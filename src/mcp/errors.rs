use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct McpErrorDetail {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl McpErrorDetail {
    pub fn invalid_input(message: impl Into<String>, suggestion: impl Into<String>) -> Self {
        Self {
            code: "INVALID_INPUT".to_string(),
            message: message.into(),
            retryable: true,
            suggestion: Some(suggestion.into()),
        }
    }

    pub fn not_found(message: impl Into<String>, suggestion: impl Into<String>) -> Self {
        Self {
            code: "NOT_FOUND".to_string(),
            message: message.into(),
            retryable: true,
            suggestion: Some(suggestion.into()),
        }
    }

    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_STATE".to_string(),
            message: message.into(),
            retryable: true,
            suggestion: Some(
                "Valid states: ' ' (empty), 'x' (done), '?' (question), '!' (important)"
                    .to_string(),
            ),
        }
    }

    pub fn validation_error(message: impl Into<String>, suggestion: impl Into<String>) -> Self {
        Self {
            code: "VALIDATION_ERROR".to_string(),
            message: message.into(),
            retryable: true,
            suggestion: Some(suggestion.into()),
        }
    }

    pub fn storage_error(message: impl Into<String>) -> Self {
        Self {
            code: "STORAGE_ERROR".to_string(),
            message: message.into(),
            retryable: false,
            suggestion: None,
        }
    }
}

/// Extension trait to simplify converting anyhow::Result to McpErrorDetail
pub trait IntoMcpError<T> {
    fn into_mcp_storage_error(self) -> Result<T, McpErrorDetail>;
}

impl<T, E: std::fmt::Display> IntoMcpError<T> for Result<T, E> {
    fn into_mcp_storage_error(self) -> Result<T, McpErrorDetail> {
        self.map_err(|e| McpErrorDetail::storage_error(e.to_string()))
    }
}
