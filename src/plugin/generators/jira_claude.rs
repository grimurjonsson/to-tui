use crate::plugin::subprocess::{check_command_exists, run_command};
use crate::plugin::TodoGenerator;
use crate::todo::TodoItem;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

pub struct JiraClaudeGenerator;

impl JiraClaudeGenerator {
    pub fn new() -> Self {
        Self
    }

    fn fetch_jira_ticket(&self, ticket_id: &str) -> Result<JiraTicket> {
        let output = run_command(
            "acli",
            &[
                "jira",
                "workitem",
                "view",
                ticket_id,
                "--fields",
                "key,summary,description,comment",
                "--json",
            ],
        )?;

        let ticket: AcliTicket = serde_json::from_str(&output).with_context(|| {
            format!(
                "Failed to parse acli JSON output.\n\nRaw output:\n{}",
                truncate_string(&output, 2000)
            )
        })?;

        Ok(JiraTicket::from(ticket))
    }

    fn fetch_jira_base_url(&self) -> Result<String> {
        let output = run_command("acli", &["jira", "auth", "status"])?;

        for line in output.lines() {
            if let Some(site) = line.trim().strip_prefix("Site:") {
                let domain = site.trim();
                return Ok(format!("https://{domain}"));
            }
        }

        Err(anyhow!(
            "Could not find Site in acli jira auth status output"
        ))
    }

    fn generate_todos_with_claude(&self, ticket: &JiraTicket) -> Result<Vec<GeneratedTodo>> {
        let prompt = self.build_prompt(ticket);

        let output = run_command("claude", &["-p", &prompt])?;

        self.parse_claude_output(&output)
    }

    fn build_prompt(&self, ticket: &JiraTicket) -> String {
        let comments_section = if ticket.comments.is_empty() {
            String::from("No comments")
        } else {
            ticket
                .comments
                .iter()
                .map(|c| {
                    let author = c.author.as_deref().unwrap_or("Unknown");
                    let date = c
                        .created
                        .as_ref()
                        .and_then(|d| d.split('T').next())
                        .unwrap_or("Unknown date");
                    format!("- {} ({}): {}", author, date, c.body)
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            r#"You are a task breakdown assistant. Given a Jira ticket, generate actionable todo items.

TICKET: {}
SUMMARY: {}
DESCRIPTION:
{}

COMMENTS:
{}

Generate a list of specific, actionable todos to complete this ticket.
Each todo should be a concrete task that can be checked off.
Use nested todos for subtasks (indent_level > 0).
Consider any additional context or requirements mentioned in the comments.

IMPORTANT: Respond ONLY with a JSON array, no other text. Format:
[
  {{"content": "Main task description", "indent_level": 0}},
  {{"content": "Subtask description", "indent_level": 1}}
]"#,
            ticket.key,
            ticket.summary,
            ticket.description.as_deref().unwrap_or("No description"),
            comments_section
        )
    }

    fn parse_claude_output(&self, output: &str) -> Result<Vec<GeneratedTodo>> {
        let trimmed = output.trim();

        let json_start = trimmed.find('[').ok_or_else(|| {
            anyhow!(
                "Claude output doesn't contain JSON array. Output: {}",
                truncate_string(trimmed, 200)
            )
        })?;

        let json_end = trimmed.rfind(']').ok_or_else(|| {
            anyhow!(
                "Claude output doesn't contain valid JSON array end. Output: {}",
                truncate_string(trimmed, 200)
            )
        })?;

        let json_str = &trimmed[json_start..=json_end];

        serde_json::from_str(json_str).with_context(|| {
            format!(
                "Failed to parse Claude's JSON output: {}",
                truncate_string(json_str, 200)
            )
        })
    }
}

impl Default for JiraClaudeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TodoGenerator for JiraClaudeGenerator {
    fn name(&self) -> &str {
        "jira"
    }

    fn description(&self) -> &str {
        "Generate todos from a Jira ticket using Claude AI"
    }

    fn check_available(&self) -> Result<(), String> {
        check_command_exists("acli")?;
        check_command_exists("claude")?;
        Ok(())
    }

    fn generate(&self, input: &str) -> Result<Vec<TodoItem>> {
        let ticket_id = input.trim().to_uppercase();

        let ticket = self
            .fetch_jira_ticket(&ticket_id)
            .with_context(|| format!("Failed to fetch Jira ticket '{ticket_id}'"))?;

        let generated = self
            .generate_todos_with_claude(&ticket)
            .with_context(|| "Failed to generate todos with Claude")?;

        if generated.is_empty() {
            return Err(anyhow!(
                "Claude generated no todos for ticket '{ticket_id}'"
            ));
        }

        let ticket_url = self
            .fetch_jira_base_url()
            .map(|base| format!("{base}/browse/{}", ticket.key))
            .ok();

        let description = match (&ticket_url, &ticket.description) {
            (Some(url), Some(desc)) => Some(format!("{url}\n---\n{desc}")),
            (Some(url), None) => Some(url.clone()),
            (None, Some(desc)) => Some(desc.clone()),
            (None, None) => None,
        };

        let mut root = TodoItem::new(format!("{} : {}", ticket.key, ticket.summary), 0);
        root.description = description;

        let children: Vec<TodoItem> = generated
            .into_iter()
            .map(|g| TodoItem::new(g.content, g.indent_level + 1))
            .collect();

        let mut items = vec![root];
        items.extend(children);

        Ok(items)
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[derive(Debug, Deserialize)]
struct AcliTicket {
    key: String,
    fields: JiraFields,
}

#[derive(Debug, Deserialize)]
struct JiraFields {
    summary: String,
    description: Option<serde_json::Value>,
    comment: Option<CommentWrapper>,
}

struct JiraTicket {
    key: String,
    summary: String,
    description: Option<String>,
    comments: Vec<JiraComment>,
}

impl From<AcliTicket> for JiraTicket {
    fn from(ticket: AcliTicket) -> Self {
        let description = ticket
            .fields
            .description
            .and_then(|v| extract_text_from_adf(&v));

        let comments = ticket
            .fields
            .comment
            .map(|wrapper| {
                wrapper
                    .comments
                    .into_iter()
                    .filter_map(|c| {
                        let body = c.body.and_then(|v| extract_text_from_adf(&v))?;
                        Some(JiraComment {
                            author: c.author.and_then(|a| a.display_name),
                            body,
                            created: c.created,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self {
            key: ticket.key,
            summary: ticket.fields.summary,
            description,
            comments,
        }
    }
}

fn extract_text_from_adf(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(obj) => {
            if let Some(content) = obj.get("content") {
                extract_text_from_adf(content)
            } else if let Some(text) = obj.get("text") {
                text.as_str().map(|s| s.to_string())
            } else {
                None
            }
        }
        serde_json::Value::Array(arr) => {
            let texts: Vec<String> = arr.iter().filter_map(extract_text_from_adf).collect();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        }
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct GeneratedTodo {
    content: String,
    indent_level: usize,
}

#[derive(Debug, Deserialize)]
struct CommentWrapper {
    comments: Vec<AcliComment>,
}

#[derive(Debug, Deserialize)]
struct AcliComment {
    author: Option<CommentAuthor>,
    body: Option<serde_json::Value>,
    created: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentAuthor {
    display_name: Option<String>,
}

#[derive(Debug, Clone)]
struct JiraComment {
    author: Option<String>,
    body: String,
    created: Option<String>,
}
