//! Gamertag ⇄ XUID resolution via the Xbox Live people-search API.

use std::time::Duration;

use reqwest::Client;

use crate::error::XboxError;
use crate::models::{PeopleSearchResponse, Xuid};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Resolves `gamertag` to its XUID by searching Xbox Live's people-search API (rooted at
/// `peoplehub_base_url`) and matching case-insensitively against the (potentially fuzzy)
/// results, using `authorization` as the `XBL3.0 x=<uhs>;<token>` header value.
pub(crate) async fn search_gamertag(
    http: &Client,
    peoplehub_base_url: &str,
    authorization: &str,
    gamertag: &str,
) -> Result<Xuid, XboxError> {
    // Xbox Live encodes the gamertag discriminator's `#` twice: once as `%23`, then that
    // literal percent-sign again as `%25`, giving `%2523`.
    let sanitized = gamertag.replace('#', "%2523");
    let url = format!(
        "{peoplehub_base_url}/users/me/people/search/decoration/detail?q={sanitized}&maxItems=5"
    );

    let response = http
        .get(&url)
        .header("x-xbl-contract-version", "4")
        .header("Accept-Language", "en-US")
        .header("Accept", "application/json")
        .header("Authorization", authorization)
        .timeout(DEFAULT_TIMEOUT)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(XboxError::HttpStatus {
            url,
            status: response.status(),
        });
    }

    let payload = response.json::<PeopleSearchResponse>().await?;
    payload
        .people
        .into_iter()
        .find(|person| person.gamertag.eq_ignore_ascii_case(gamertag))
        .map(|person| Xuid(person.xuid))
        .ok_or_else(|| XboxError::PersonNotFound(gamertag.to_string()))
}
