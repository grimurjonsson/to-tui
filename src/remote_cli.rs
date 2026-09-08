use crate::cli::RemoteCommand;
use anyhow::{Context, Result, ensure};
use std::io::Read;
use to_tui::config::Config;
use to_tui::remote::{self, Client, RemoteConfig};

pub fn run(command: RemoteCommand) -> Result<()> {
    let mut config = Config::load()?;
    std::fs::create_dir_all(to_tui::utils::paths::get_to_tui_dir()?)?;
    match command {
        RemoteCommand::Add { name, url } => {
            remote::validate_name(&name)?;
            ensure!(
                !config.remotes.contains_key(&name),
                "Remote already exists; remove it before replacing its URL"
            );
            config.remotes.insert(
                name.clone(),
                RemoteConfig {
                    url: remote::validate_url(&url)?,
                    user_id: None,
                },
            );
            config.save()?;
            println!("Added {name}. Run totui remote login {name}, then totui remote use {name}.");
        }
        RemoteCommand::Login { name, cookie_stdin } => {
            let mut profile = config.remotes.get(&name).context("Unknown remote")?.clone();
            profile.user_id = None;
            let (client, browser_token, cookie) = if cookie_stdin {
                let mut cookie = String::new();
                std::io::stdin().take(65537).read_to_string(&mut cookie)?;
                ensure!(cookie.len() <= 65536, "Cookie is too large");
                (
                    Client::new(profile.clone(), Some(&cookie))?,
                    None,
                    Some(cookie),
                )
            } else {
                let flow = remote::login::BrowserLogin::new(&profile)?;
                println!("Open this page in your browser to connect:\n{}", flow.url());
                if let Err(error) = open::that(flow.url()) {
                    eprintln!(
                        "Could not open your browser automatically: {error}. Open the link above."
                    );
                }
                println!("Waiting for browser login…");
                let token = flow.finish()?;
                (
                    Client::with_token(profile.clone(), &token.access_token)?,
                    Some(token),
                    None,
                )
            };
            let user = client.user()?;
            profile.user_id = Some(user.id.clone());
            client.check()?;
            if let Some(token) = browser_token {
                remote::save_token(&name, &token)?;
            } else if let Some(cookie) = cookie {
                remote::save_cookie(&name, &cookie)?;
                match std::fs::remove_file(remote::token_path(&name)?) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            config.remotes.insert(name.clone(), profile);
            config.save()?;
            println!("Logged into {name} as {}.", user.email.unwrap_or(user.id));
        }
        RemoteCommand::Use { name } => {
            let mut profile = config.remotes.get(&name).context("Unknown remote")?.clone();
            let client = Client::configured(&name, profile.clone())?;
            let user = client.user()?;
            client.check()?;
            profile.user_id = Some(user.id);
            config.remotes.insert(name.clone(), profile);
            config.default_remote = Some(name.clone());
            config.save()?;
            println!("Default workspace: {name}. Run totui to open it.");
        }
        RemoteCommand::Local => {
            config.default_remote = None;
            config.save()?;
            println!("Default workspace: local.");
        }
        RemoteCommand::Status { name } => {
            let name = name
                .or(config.default_remote.clone())
                .context("No default remote; supply a name")?;
            let profile = config.remotes.get(&name).context("Unknown remote")?;
            let client = Client::configured(&name, profile.clone())?;
            let user = client.user()?;
            client.check()?;
            println!(
                "{name}: {} — connected as {}",
                profile.url,
                user.email.unwrap_or(user.id)
            );
        }
        RemoteCommand::List => {
            for (name, profile) in &config.remotes {
                println!(
                    "{} {name}: {}",
                    if config.default_remote.as_ref() == Some(name) {
                        "*"
                    } else {
                        " "
                    },
                    profile.url
                );
            }
        }
        RemoteCommand::Logout { name } => {
            ensure!(config.remotes.contains_key(&name), "Unknown remote");
            if remote::token_path(&name)?.exists() {
                let token: to_tui::api::client_auth::Token =
                    serde_json::from_slice(&std::fs::read(remote::token_path(&name)?)?)?;
                Client::with_token(config.remotes[&name].clone(), &token.access_token)?.revoke()?;
            }
            remote::forget_credentials(&name)?;
            println!("Logged out of {name}.");
        }

        RemoteCommand::Remove { name } => {
            ensure!(config.remotes.contains_key(&name), "Unknown remote");
            if remote::token_path(&name)?.exists() {
                let token: to_tui::api::client_auth::Token =
                    serde_json::from_slice(&std::fs::read(remote::token_path(&name)?)?)?;
                Client::with_token(config.remotes[&name].clone(), &token.access_token)?.revoke()?;
            }
            remote::forget_credentials(&name)?;
            config.remotes.remove(&name);
            if config.default_remote.as_ref() == Some(&name) {
                config.default_remote = None;
            }
            config.save()?;
            println!("Removed {name} and its saved session.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn test_remote_cli_selection_and_login_arguments() {
        assert!(Cli::try_parse_from(["totui", "--remote", "home"]).is_ok());
        assert!(Cli::try_parse_from(["totui", "--remote", "home", "--local"]).is_err());
        assert!(
            Cli::try_parse_from(["totui", "remote", "login", "home", "--cookie-stdin"]).is_ok()
        );
        assert!(Cli::try_parse_from(["totui", "remote", "add", "home"]).is_err());
    }
}
