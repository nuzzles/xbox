# xbox

[![CI](https://github.com/nuzzles/xbox/actions/workflows/ci.yml/badge.svg)](https://github.com/nuzzles/xbox/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/xbox.svg)](https://crates.io/crates/xbox)
[![docs.rs](https://docs.rs/xbox/badge.svg)](https://docs.rs/xbox)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Unofficial Xbox Live authentication, gamertag/XUID resolution, and profile client for Rust.

> [!IMPORTANT]
> This is an unofficial, community-maintained library. It is not affiliated with, endorsed by Microsoft.

## What this crate does

- Signs in to Xbox Live and exchanges credentials for a Xbox Live "user token".
- Exchanges a user token for an XSTS ticket, scoped to a given [`RelyingParty`](src/auth/relying_party.rs) (e.g.
  Xbox Live itself, or a third-party service like Halo Waypoint).
- Resolves gamertags to XUIDs (and back) via the Xbox Live people-search API.
- Caches every credential with expiry-awareness and single-flight de-duplication, so concurrent callers never
  trigger redundant network round trips, and callers never pay full re-authentication cost on every request.

## Quick start

```rust,no_run
use xbox::auth::LegacyPasswordProvider;
use xbox::{RelyingParty, XboxClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = LegacyPasswordProvider::new("my-username", "my-password");
    let client = XboxClient::new(provider);

    let xuid = client.gamertag_to_xuid("Some Gamertag").await?;
    let xsts = client.xsts_ticket(RelyingParty::Xbox).await?;

    println!("xuid={xuid} xsts_expires_at={}", xsts.expires_at);
    Ok(())
}
```

## Feature flags

| Feature | Default | Description |
| --- | --- | --- |
| `legacy-password-login` | ✅ | Enables `LegacyPasswordProvider`, which signs in via the same username/password flow the Xbox Live web login page uses. This is not an officially documented API and may break if Microsoft changes their login page. |

A real OAuth2 device-code provider (the officially documented path, with refresh-token support) is planned as an
additive, non-breaking follow-up behind the same [`XblAuthProvider`](src/auth/provider.rs) trait.

## MSRV

This crate has a [Minimum Supported Rust Version (MSRV)][MSRV] of 1.96.

[MSRV]: CHANGELOG.md

## License

Licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option

## Contribution

See [CONTRIBUTING.md](CONTRIBUTING.md).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
