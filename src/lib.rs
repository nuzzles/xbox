//! Unofficial Xbox Live authentication, gamertag/XUID resolution, and profile client for Rust.
//!
//! This crate is not affiliated with, endorsed by or supported by Microsoft.
//!
//! # Quick start
//!
//! Requires the default `legacy-password-login` feature.
//!
//! ```no_run
//! # #[cfg(feature = "legacy-password-login")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use xbox::auth::LegacyPasswordProvider;
//! use xbox::{RelyingParty, XboxClient};
//!
//! let provider = LegacyPasswordProvider::new("my-username", "my-password");
//! let client = XboxClient::new(provider);
//!
//! let xuid = client.gamertag_to_xuid("Some Gamertag").await?;
//! let xsts = client.xsts_ticket(RelyingParty::Xbox).await?;
//!
//! println!("xuid={xuid} xsts_expires_at={}", xsts.not_after);
//! # Ok(())
//! # }
//! ```

pub mod auth;
pub mod cache;
pub mod client;
pub mod error;
pub mod models;
pub mod people;
pub mod util;

pub use auth::RelyingParty;
pub use client::{XboxClient, XboxEndpoints};
pub use error::XboxError;
