# Contributing to xbox

Thanks for considering a contribution. This is a solo-maintained, community library — PRs and issues are welcome.

## Getting started

```sh
git clone https://github.com/nuzzles/xbox
cd xbox
cargo build
cargo test --all-features
```

## Before opening a PR

Run the same checks CI runs:

```sh
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo doc --no-deps
```

All of these must pass. If `cargo fmt` reports diffs, run `cargo fmt` (without `--check`) to fix them.

## Coding conventions

- No `unwrap()`/`expect()` in library code outside of tests — return a typed error instead (see `src/error.rs`).
- New public types/functions should have doc comments; non-obvious behavior (auth quirks, API response shapes,
  retry semantics) should explain *why*, not just restate the signature.
- New authenticated HTTP call sites should go through the existing single-flight/expiry cache and 401-retry
  wrapper rather than hand-rolling their own `reqwest` call.
- Feature-gated code (e.g. `legacy-password-login`) must compile and pass tests both with and without the
  feature enabled — `cargo test --no-default-features` exists specifically to catch feature-gating mistakes.

## Filing issues

Bug reports should include the crate version, a minimal reproduction if possible, and what you expected vs.
what happened. Feature requests should describe the use case, not just the desired API shape.

## Branches and commits

- Branch from `main`, name branches descriptively (e.g. `fix/xsts-expiry-parsing`).
- Keep commits focused; a clear commit message beats a long diff with a vague one.

## Review and merge

Open a PR against `main`. CI must be green. A maintainer will review and may ask for changes before merging.
