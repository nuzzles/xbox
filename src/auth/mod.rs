pub mod oauth;
pub mod provider;
pub mod relying_party;

#[cfg(all(feature = "legacy-password-login", not(target_arch = "wasm32")))]
pub mod legacy_password;

pub use oauth::{
    OAuthClient, OAuthEndpoints, OAuthRefreshTokenProvider, OAuthTokens, RefreshTokenStore,
};
pub use provider::XblAuthProvider;
pub use relying_party::RelyingParty;

#[cfg(all(feature = "legacy-password-login", not(target_arch = "wasm32")))]
pub use legacy_password::LegacyPasswordProvider;
