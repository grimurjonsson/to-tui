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
            // Sourced from ops::VALID_STATES so this cannot drift from what the
            // parser actually accepts. It previously omitted '*' and '-'.
            suggestion: Some(format!("Valid states: {}", crate::todo::ops::VALID_STATES)),
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

/// Bridge the shared ops layer's error kinds onto the MCP wire contract.
///
/// The codes below are what MCP clients branch on, so this mapping is pinned by
/// tests in `todo::ops`.
impl From<crate::todo::ops::OpsError> for McpErrorDetail {
    fn from(err: crate::todo::ops::OpsError) -> Self {
        use crate::todo::ops::OpsError;
        match err {
            OpsError::NotFound {
                message,
                suggestion,
            } => Self::not_found(message, suggestion),
            OpsError::InvalidInput {
                message,
                suggestion,
            } => Self::invalid_input(message, suggestion),
            OpsError::InvalidState { message } => Self::invalid_state(message),
            OpsError::Validation {
                message,
                suggestion,
            } => Self::validation_error(message, suggestion),
            OpsError::Storage { message } => Self::storage_error(message),
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
