use std::sync::Arc;

use thiserror::Error;

/// Errors produced by this crate.
#[derive(Debug, Clone, Error)]
pub enum XboxError {
    /// An HTTP request completed but returned a non-2xx status.
    #[error("http request to {url} failed with status {status}")]
    HttpStatus {
        url: String,
        status: reqwest::StatusCode,
    },

    /// The underlying HTTP request itself failed (connection, timeout, decode, ...).
    #[error("network error: {0}")]
    Network(Arc<reqwest::Error>),

    /// No Xbox Live user matching the given gamertag was found.
    #[error("no Xbox Live user found matching gamertag \"{0}\"")]
    PersonNotFound(String),

    /// Failed to locate the PPFT hidden form field in the Microsoft login page.
    #[cfg(feature = "legacy-password-login")]
    #[error("failed to parse PPFT token from Microsoft login page")]
    PpftNotFound,

    /// Failed to locate the login POST URL in the Microsoft login page.
    #[cfg(feature = "legacy-password-login")]
    #[error("failed to parse login post URL from Microsoft login page")]
    PostUrlNotFound,

    /// Microsoft rejected the supplied username or password.
    #[cfg(feature = "legacy-password-login")]
    #[error("invalid Microsoft account username or password")]
    InvalidCredentials,

    /// The login redirect response was missing a `Location` header.
    #[cfg(feature = "legacy-password-login")]
    #[error("login redirect response was missing a Location header")]
    MissingRedirectLocation,

    /// Failed to extract the RPS ticket (`access_token=...`) from the login redirect URL.
    #[cfg(feature = "legacy-password-login")]
    #[error("failed to extract access token from login redirect URL")]
    RpsTicketNotFound,

    /// The Microsoft OAuth exchange was rejected. Response bodies are deliberately omitted.
    #[error("Microsoft OAuth token request was rejected ({0})")]
    OAuthRejected(&'static str),

    /// Microsoft returned an unexpected OAuth response without a usable token.
    #[error("Microsoft OAuth token response was incomplete")]
    OAuthIncompleteResponse,

    /// Invalid OAuth endpoint or authorization URL configuration.
    #[error("Microsoft OAuth configuration is invalid")]
    OAuthConfiguration,

    /// A caller-supplied refresh-token store failed without exposing token contents.
    #[error("OAuth refresh-token store failed ({0})")]
    OAuthTokenStore(&'static str),
}

impl From<reqwest::Error> for XboxError {
    fn from(err: reqwest::Error) -> Self {
        XboxError::Network(Arc::new(err))
    }
}

impl XboxError {
    /// Whether this error represents an HTTP 401 (Unauthorized) response,
    /// indicating a cached credential has expired or been revoked server-side.
    pub fn is_unauthorized(&self) -> bool {
        matches!(
            self,
            XboxError::HttpStatus { status, .. } if *status == reqwest::StatusCode::UNAUTHORIZED
        )
    }
}
