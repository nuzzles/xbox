pub mod provider;
pub mod relying_party;

#[cfg(feature = "legacy-password-login")]
pub mod legacy_password;

pub use provider::XblAuthProvider;
pub use relying_party::RelyingParty;

#[cfg(feature = "legacy-password-login")]
pub use legacy_password::LegacyPasswordProvider;
