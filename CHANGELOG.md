# Changelog

<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/1.0.0/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

Changes for each release are documented below.

## [Unreleased]

This release has an [MSRV][] of 1.96.

## [0.3.1] - 2026-11-19

This release has an [MSRV][] of 1.96.

### Fixed

- Allow OAuth authentication futures to compile for WebAssembly targets.
- Exclude native-only legacy password login from WebAssembly builds, including builds with all features enabled.

## [0.3.0] - 2026-11-19

This release has an [MSRV][] of 1.96.

### Added

- Added Microsoft OAuth authorization-code and refresh-token support with `OAuthClient` and
  `OAuthRefreshTokenProvider` for unattended Xbox Live authentication.
- Added `RefreshTokenStore` for durable, atomic refresh-token rotation across service instances.
- Added `OAuthClient::with_request_timeout` to configure Microsoft token and Xbox user-authentication
  request timeouts (10 seconds by default).

## [0.2.0] - 2026-07-27

This release has an [MSRV][] of 1.96.

### Changed

- Replaced the closed `RelyingParty` enum with an extensible URI-backed type. Use
  `RelyingParty::XBOX` for Xbox Live or `RelyingParty::new` for downstream services.

### Fixed

- Report invalid Microsoft account usernames or passwords as `XboxError::InvalidCredentials`
  instead of `XboxError::MissingRedirectLocation` during legacy password login.

## [0.1.0] - 2026-07-27

This release has an [MSRV][] of 1.96.

### Added

- Initial release: Xbox Live authentication via `LegacyPasswordProvider`, XSTS ticket exchange, gamertag/XUID
  resolution, and expiry-aware single-flight token caching.

[MSRV]: README.md#msrv

[Unreleased]: https://github.com/nuzzles/xbox/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/nuzzles/xbox/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/nuzzles/xbox/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/nuzzles/xbox/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/nuzzles/xbox/releases/tag/v0.1.0
