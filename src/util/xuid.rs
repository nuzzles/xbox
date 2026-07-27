//! Helpers for the `xuid(123)`-wrapped player-identifier format used by several Xbox Live
//! and Halo Waypoint endpoints, alongside plain gamertags or raw XUID strings.

/// Wraps a raw XUID (e.g. `"2535405290989773"`) as `xuid(2535405290989773)`, the form several
/// endpoints require in path/query parameters. If `player_id` already looks like a wrapped
/// identifier (`word(digits)`), it is returned unchanged.
pub fn wrap_xuid(player_id: &str) -> String {
    if is_wrapped(player_id) {
        player_id.to_string()
    } else {
        format!("xuid({player_id})")
    }
}

/// Unwraps a wrapped identifier (`xuid(123)`) back to its raw digits (`"123"`). If `player_id`
/// is not in wrapped form, it is returned unchanged (e.g. a plain gamertag passes through).
pub fn unwrap_xuid(player_id: &str) -> String {
    match wrapped_digits(player_id) {
        Some(digits) => digits.to_string(),
        None => player_id.to_string(),
    }
}

fn is_wrapped(player_id: &str) -> bool {
    wrapped_digits(player_id).is_some()
}

/// If `player_id` matches `word(digits)`, returns the digits substring.
fn wrapped_digits(player_id: &str) -> Option<&str> {
    let open = player_id.find('(')?;
    if !player_id.ends_with(')') {
        return None;
    }
    let prefix = &player_id[..open];
    if prefix.is_empty()
        || !prefix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    let digits = &player_id[open + 1..player_id.len() - 1];
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        Some(digits)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_raw_xuid() {
        assert_eq!(wrap_xuid("2535405290989773"), "xuid(2535405290989773)");
    }

    #[test]
    fn wrap_is_idempotent() {
        assert_eq!(wrap_xuid("xuid(123)"), "xuid(123)");
    }

    #[test]
    fn unwraps_wrapped_xuid() {
        assert_eq!(unwrap_xuid("xuid(123)"), "123");
    }

    #[test]
    fn unwrap_passes_through_gamertag() {
        assert_eq!(unwrap_xuid("Some Gamertag"), "Some Gamertag");
    }

    #[test]
    fn unwrap_passes_through_raw_xuid() {
        assert_eq!(unwrap_xuid("123"), "123");
    }

    #[test]
    fn wrap_passes_through_gamertag_with_parens() {
        // Not a valid wrapped form (non-digit inside parens), so wrap_xuid should still wrap it
        // as a literal xuid(...) — this is an edge case callers shouldn't hit in practice, but
        // the function must not panic or misparse.
        assert_eq!(wrap_xuid("gt(abc)"), "xuid(gt(abc))");
    }
}
