use std::fmt;

#[derive(Debug)]
pub enum CircleDebugError {
    ApiError { status: u16, message: String },
    AuthenticationError(String),
    NetworkError(String),
    ParseError(String),
    ConfigurationError(String),
}

impl fmt::Display for CircleDebugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiError { status, message } => {
                write!(f, "CircleCI API error (HTTP {}): {}", status, message)
            }
            Self::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for CircleDebugError {}

impl From<reqwest::Error> for CircleDebugError {
    fn from(err: reqwest::Error) -> Self {
        CircleDebugError::NetworkError(err.to_string())
    }
}

impl From<serde_json::Error> for CircleDebugError {
    fn from(err: serde_json::Error) -> Self {
        CircleDebugError::ParseError(err.to_string())
    }
}

impl From<regex::Error> for CircleDebugError {
    fn from(err: regex::Error) -> Self {
        CircleDebugError::ParseError(err.to_string())
    }
}

