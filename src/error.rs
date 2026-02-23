use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

/// Replica of the tauri::plugin::mobile::ErrorResponse for desktop platforms.
#[cfg(desktop)]
#[derive(Debug, thiserror::Error, Clone, serde::Deserialize)]
pub struct ErrorResponse<T = ()> {
    /// Error code.
    pub code: Option<String>,
    /// Error message.
    pub message: Option<String>,
    /// Optional error data.
    #[serde(flatten)]
    pub data: T,
}

#[cfg(desktop)]
impl<T> std::fmt::Display for ErrorResponse<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(code) = &self.code {
            write!(f, "[{code}]")?;
            if self.message.is_some() {
                write!(f, " - ")?;
            }
        }
        if let Some(message) = &self.message {
            write!(f, "{message}")?;
        }
        Ok(())
    }
}

/// Replica of the tauri::plugin::mobile::PluginInvokeError for desktop platforms.
#[cfg(desktop)]
#[derive(Debug, thiserror::Error)]
pub enum PluginInvokeError {
    /// Error returned from direct desktop plugin.
    #[error(transparent)]
    InvokeRejected(#[from] ErrorResponse),
    /// Failed to deserialize response.
    #[error("failed to deserialize response: {0}")]
    CannotDeserializeResponse(serde_json::Error),
    /// Failed to serialize request payload.
    #[error("failed to serialize payload: {0}")]
    CannotSerializePayload(serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
    #[cfg(desktop)]
    #[error(transparent)]
    PluginInvoke(#[from] crate::error::PluginInvokeError),
    #[cfg(target_os = "windows")]
    #[error(transparent)]
    WindowsApi(#[from] windows::core::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_io_display() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = Error::Io(io_error);
        let display = error.to_string();
        assert!(display.contains("file not found"));
    }

    #[test]
    fn test_error_serialize() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test error");
        let error = Error::Io(io_error);
        let serialized = serde_json::to_string(&error).expect("Failed to serialize Error");
        assert!(serialized.contains("test error"));
    }

    #[test]
    fn test_error_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let error: Error = io_error.into();
        assert!(error.to_string().contains("access denied"));
    }

    #[cfg(desktop)]
    mod desktop_tests {
        use super::*;

        #[test]
        fn test_error_response_display_code_only() {
            let response = ErrorResponse {
                code: Some("ERR001".to_string()),
                message: None,
                data: (),
            };
            assert_eq!(response.to_string(), "[ERR001]");
        }

        #[test]
        fn test_error_response_display_message_only() {
            let response = ErrorResponse {
                code: None,
                message: Some("Something went wrong".to_string()),
                data: (),
            };
            assert_eq!(response.to_string(), "Something went wrong");
        }

        #[test]
        fn test_error_response_display_both() {
            let response = ErrorResponse {
                code: Some("ERR001".to_string()),
                message: Some("Something went wrong".to_string()),
                data: (),
            };
            assert_eq!(response.to_string(), "[ERR001] - Something went wrong");
        }

        #[test]
        fn test_error_response_display_neither() {
            let response: ErrorResponse = ErrorResponse {
                code: None,
                message: None,
                data: (),
            };
            assert_eq!(response.to_string(), "");
        }

        #[test]
        fn test_error_response_deserialize() {
            let json = r#"{"code":"testCode","message":"test message"}"#;
            let response: ErrorResponse =
                serde_json::from_str(json).expect("Failed to deserialize ErrorResponse");
            assert_eq!(response.code, Some("testCode".to_string()));
            assert_eq!(response.message, Some("test message".to_string()));
        }

        #[test]
        fn test_error_response_deserialize_partial() {
            let json = r#"{"code":"testCode"}"#;
            let response: ErrorResponse =
                serde_json::from_str(json).expect("Failed to deserialize partial ErrorResponse");
            assert_eq!(response.code, Some("testCode".to_string()));
            assert_eq!(response.message, None);
        }

        #[test]
        fn test_plugin_invoke_error_invoke_rejected() {
            let response = ErrorResponse {
                code: Some("rejected".to_string()),
                message: Some("Request rejected".to_string()),
                data: (),
            };
            let error = PluginInvokeError::InvokeRejected(response);
            let display = error.to_string();
            assert!(display.contains("rejected"));
            assert!(display.contains("Request rejected"));
        }

        #[test]
        fn test_plugin_invoke_error_deserialize_failed() {
            let json_error =
                serde_json::from_str::<i32>("not a number").expect_err("Expected JSON parse error");
            let error = PluginInvokeError::CannotDeserializeResponse(json_error);
            let display = error.to_string();
            assert!(display.contains("failed to deserialize response"));
        }

        #[test]
        fn test_plugin_invoke_error_serialize_failed() {
            // Create a serialization error by trying to serialize from invalid JSON
            // We can reuse a deserialization error type since serde_json::Error is the same type
            let json_error = serde_json::from_str::<serde_json::Value>("{invalid}")
                .expect_err("Expected JSON parse error");
            let error = PluginInvokeError::CannotSerializePayload(json_error);
            let display = error.to_string();
            assert!(display.contains("failed to serialize payload"));
        }

        #[test]
        fn test_error_from_plugin_invoke_error() {
            let response = ErrorResponse {
                code: Some("test".to_string()),
                message: Some("test".to_string()),
                data: (),
            };
            let plugin_error = PluginInvokeError::InvokeRejected(response);
            let error: Error = plugin_error.into();
            assert!(error.to_string().contains("test"));
        }
    }
}
