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

### OAuth

Authorize once with `OAuthClient::authorization_url` and `exchange_code`, then
save the refresh token in a durable `RefreshTokenStore` with atomic updates.
See the [Xbox authentication walkthrough](https://den.dev/blog/halo-api-authentication/)
for app registration and consent.

```rust
use std::time::Duration;
use xbox::auth::{OAuthClient, OAuthRefreshTokenProvider};
use xbox::XboxClient;

let oauth = OAuthClient::new(client_id, client_secret)
    .with_request_timeout(Duration::from_secs(20)); // Default: 10 seconds per request.
let client = XboxClient::new(OAuthRefreshTokenProvider::new(oauth, token_store));
let xuid = client.gamertag_to_xuid("Some Gamertag").await?;
```

### Legacy password

`LegacyPasswordProvider` requires the `legacy-password-login` feature, which is
enabled by default on native targets. It relies on Microsoft's undocumented login page and may
break when that page changes.

```rust
use xbox::auth::LegacyPasswordProvider;
use xbox::XboxClient;

let client = XboxClient::new(LegacyPasswordProvider::new(username, password));
let xuid = client.gamertag_to_xuid("Some Gamertag").await?;
```

## Feature flags

| Feature | Default | Description |
| --- | --- | --- |
| `legacy-password-login` | Yes | Enables `LegacyPasswordProvider` on native targets; it uses Microsoft's undocumented login page. |

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
