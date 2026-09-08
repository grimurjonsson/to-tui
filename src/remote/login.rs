use super::{RemoteConfig, validate_url};
use crate::api::client_auth::{Exchange, Token, challenge, secret};
use anyhow::{Context, Result, bail, ensure};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

pub struct BrowserLogin {
    listener: TcpListener,
    verifier: String,
    state: String,
    url: reqwest::Url,
    server: String,
}

impl BrowserLogin {
    pub fn new(config: &RemoteConfig) -> Result<Self> {
        let server = validate_url(&config.url)?;
        let listener =
            TcpListener::bind("127.0.0.1:0").context("Could not start browser login callback")?;
        listener.set_nonblocking(true)?;
        let verifier = secret();
        let state = secret();
        let mut url = reqwest::Url::parse(&format!("{server}/remote/login"))?;
        url.query_pairs_mut()
            .append_pair("challenge", &challenge(&verifier))
            .append_pair("state", &state)
            .append_pair("port", &listener.local_addr()?.port().to_string());
        Ok(Self {
            listener,
            verifier,
            state,
            url,
            server,
        })
    }

    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    pub fn finish(self) -> Result<Token> {
        let deadline = Instant::now() + Duration::from_secs(300);
        while Instant::now() < deadline {
            let (mut stream, _) = match self.listener.accept() {
                Ok(connection) => connection,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            stream.set_write_timeout(Some(Duration::from_secs(2)))?;
            let callback = match self.callback(&stream) {
                Ok(Some(callback)) => callback,
                _ => {
                    let _ = reply(
                        &mut stream,
                        "400 Bad Request",
                        "This is not a valid to-tui login callback.",
                    );
                    continue;
                }
            };
            if callback.is_empty() {
                let _ = reply(
                    &mut stream,
                    "200 OK",
                    "Connection cancelled. You can close this tab.",
                );
                bail!("Browser login cancelled");
            }
            let result = self.exchange(callback);
            match &result {
                Ok(_) => {
                    let _ = reply(
                        &mut stream,
                        "200 OK",
                        "Connected to to-tui. Return to your terminal; you can close this tab.",
                    );
                }
                Err(_) => {
                    let _ = reply(
                        &mut stream,
                        "400 Bad Request",
                        "Login could not be completed. Return to your terminal and try again.",
                    );
                }
            }
            return result;
        }
        bail!("Browser login timed out after five minutes. Run totui remote login again")
    }

    fn callback(&self, stream: &TcpStream) -> Result<Option<String>> {
        let mut reader = BufReader::new(stream).take(8192);
        let mut first = String::new();
        reader.read_line(&mut first)?;
        let fields = first.split_whitespace().collect::<Vec<_>>();
        ensure!(
            fields.len() == 3 && fields[0] == "GET" && fields[1].starts_with("/callback?"),
            "Invalid callback request"
        );
        let mut host = None;
        loop {
            let mut line = String::new();
            ensure!(reader.read_line(&mut line)? > 0, "Incomplete callback");
            if line == "\r\n" {
                break;
            }
            if let Some((key, value)) = line.split_once(':')
                && key.eq_ignore_ascii_case("host")
            {
                ensure!(host.is_none(), "Duplicate Host");
                host = Some(value.trim().to_owned());
            }
        }
        ensure!(
            host.as_deref() == Some(&format!("127.0.0.1:{}", self.listener.local_addr()?.port())),
            "Invalid callback host"
        );
        let url = reqwest::Url::parse(&format!("http://127.0.0.1{}", fields[1]))?;
        let mut pairs = std::collections::HashMap::new();
        for (key, value) in url.query_pairs() {
            ensure!(
                pairs.insert(key.into_owned(), value.into_owned()).is_none(),
                "Duplicate callback parameter"
            );
        }
        if pairs.get("state") != Some(&self.state) {
            return Ok(None);
        }
        if pairs.get("error").is_some_and(|e| e == "access_denied") {
            return Ok(Some(String::new()));
        }
        let code = pairs.remove("code").context("Missing authorization code")?;
        ensure!(
            code.len() == 96 && code.bytes().all(|c| c.is_ascii_hexdigit()),
            "Invalid authorization code"
        );
        Ok(Some(code))
    }

    fn exchange(&self, code: String) -> Result<Token> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client
            .post(format!("{}/api/remote/login/exchange", self.server))
            .json(&Exchange {
                code,
                verifier: self.verifier.clone(),
            })
            .send()
            .context("Could not exchange browser login code")?;
        ensure!(
            response.status().is_success(),
            "Browser login exchange failed (HTTP {}). Check that the server and reverse proxy support browser login",
            response.status()
        );
        response.json().context("Invalid login token response")
    }
}

fn reply(stream: &mut TcpStream, status: &str, message: &str) -> Result<()> {
    let body = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>to-tui login</title><h1>to-tui</h1><p>{message}</p></html>"
    );
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    Ok(())
}
