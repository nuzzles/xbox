use async_trait::async_trait;

use crate::cache::CachedToken;
use crate::error::XboxError;

/// Something that can produce a fresh Xbox Live "user token" on demand.
///
/// This is the extension point for how a caller authenticates to Xbox Live in the first
/// place. v1 ships exactly one implementor, `LegacyPasswordProvider` (behind the
/// `legacy-password-login` feature); a real OAuth2 device-code provider is a planned,
/// additive follow-up behind this same trait.
#[async_trait]
pub trait XblAuthProvider: Send + Sync {
    /// Produces a fresh Xbox Live user token, including its expiry.
    ///
    /// Implementors do not need to cache the result themselves — [`crate::XboxClient`] wraps
    /// every call to this method in its own expiry-aware, single-flight cache.
    async fn user_token(&self) -> Result<CachedToken<String>, XboxError>;
}
