use std::future::Future;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use reqwest::Client;

use crate::auth::{RelyingParty, XblAuthProvider};
use crate::cache::{CachedToken, ExpiryTokenCache, KeyedExpiryTokenCache};
use crate::error::XboxError;
use crate::models::{XstsToken, Xuid};
use crate::people;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
/// XUID resolution isn't a real expiring credential, but gamertags are effectively immutable
/// on the timescale a bot/service cares about, so we reuse the same cache shape with a long TTL.
const XUID_CACHE_TTL: ChronoDuration = ChronoDuration::hours(24);

const DEFAULT_XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const DEFAULT_PEOPLEHUB_BASE_URL: &str = "https://peoplehub.xboxlive.com";

/// Overridable base URLs for the Xbox Live endpoints this crate calls.
///
/// Exists primarily so tests (and callers proxying/mocking Xbox Live) can point this crate at
/// something other than the real service. [`Default`] points at the real endpoints.
#[derive(Debug, Clone)]
pub struct XboxEndpoints {
    pub xsts_authorize_url: String,
    pub peoplehub_base_url: String,
}

impl Default for XboxEndpoints {
    fn default() -> Self {
        Self {
            xsts_authorize_url: DEFAULT_XSTS_URL.to_string(),
            peoplehub_base_url: DEFAULT_PEOPLEHUB_BASE_URL.to_string(),
        }
    }
}

/// The top-level entry point for talking to Xbox Live.
///
/// Wraps an [`XblAuthProvider`] with expiry-aware, single-flight caching for the user token,
/// per-[`RelyingParty`] XSTS tickets, and gamertag→XUID resolution, plus automatic
/// invalidate-and-retry-once behavior on HTTP 401 responses.
pub struct XboxClient<P: XblAuthProvider> {
    provider: P,
    http: Client,
    endpoints: XboxEndpoints,
    user_token_cache: ExpiryTokenCache<String, XboxError>,
    xsts_cache: KeyedExpiryTokenCache<RelyingParty, XstsToken, XboxError>,
    xuid_cache: KeyedExpiryTokenCache<String, Xuid, XboxError>,
}

impl<P: XblAuthProvider> XboxClient<P> {
    pub fn new(provider: P) -> Self {
        Self::with_http_client(provider, Client::new())
    }

    /// Constructs a client using a caller-supplied `reqwest::Client`, e.g. one with custom TLS
    /// or proxy configuration. This crate does not modify certificate validation on your
    /// behalf — configure that on the client you pass in if you need to.
    pub fn with_http_client(provider: P, http: Client) -> Self {
        Self::with_endpoints(provider, http, XboxEndpoints::default())
    }

    /// Constructs a client with overridden endpoint URLs, e.g. to point at a mock server in
    /// tests. Most callers should use [`XboxClient::new`] or
    /// [`XboxClient::with_http_client`] instead.
    pub fn with_endpoints(provider: P, http: Client, endpoints: XboxEndpoints) -> Self {
        Self {
            provider,
            http,
            endpoints,
            user_token_cache: ExpiryTokenCache::new(),
            xsts_cache: KeyedExpiryTokenCache::new(),
            xuid_cache: KeyedExpiryTokenCache::new(),
        }
    }

    async fn user_token(&self) -> Result<String, XboxError> {
        self.user_token_cache
            .get_or_refresh(|| self.provider.user_token())
            .await
    }

    /// Returns a cached XSTS ticket for `relying_party`, fetching and caching a fresh one if
    /// necessary.
    pub async fn xsts_ticket(&self, relying_party: RelyingParty) -> Result<XstsToken, XboxError> {
        self.xsts_cache
            .get_or_refresh(relying_party, || async {
                let user_token = self.user_token().await?;
                let ticket = fetch_xsts_ticket(
                    &self.http,
                    &self.endpoints.xsts_authorize_url,
                    &user_token,
                    relying_party,
                )
                .await?;
                Ok(CachedToken::new(ticket.clone(), ticket.not_after))
            })
            .await
    }

    /// Resolves a gamertag to its XUID via the Xbox Live people-search API, matching
    /// case-insensitively against the (potentially fuzzy) search results. The result is
    /// cached for a long TTL since gamertags rarely change.
    pub async fn gamertag_to_xuid(&self, gamertag: &str) -> Result<Xuid, XboxError> {
        let gamertag_owned = gamertag.to_string();
        self.xuid_cache
            .get_or_refresh(gamertag_owned.clone(), || async {
                let xuid = self
                    .with_single_retry(RelyingParty::XBOX, || async {
                        let xsts = self.xsts_ticket(RelyingParty::XBOX).await?;
                        let authorization = xsts
                            .authorization_header()
                            .ok_or_else(|| XboxError::PersonNotFound(gamertag_owned.clone()))?;
                        people::search_gamertag(
                            &self.http,
                            &self.endpoints.peoplehub_base_url,
                            &authorization,
                            &gamertag_owned,
                        )
                        .await
                    })
                    .await?;
                Ok(CachedToken::new(xuid, Utc::now() + XUID_CACHE_TTL))
            })
            .await
    }

    /// Runs `call`, and on an HTTP 401 response invalidates the XSTS ticket for
    /// `relying_party` (and the underlying user token) before retrying exactly once.
    async fn with_single_retry<T, F, Fut>(
        &self,
        relying_party: RelyingParty,
        mut call: F,
    ) -> Result<T, XboxError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, XboxError>>,
    {
        match call().await {
            Err(err) if err.is_unauthorized() => {
                self.xsts_cache.invalidate(&relying_party).await;
                self.user_token_cache.invalidate().await;
                call().await
            }
            other => other,
        }
    }
}

async fn fetch_xsts_ticket(
    http: &Client,
    url: &str,
    user_token: &str,
    relying_party: RelyingParty,
) -> Result<XstsToken, XboxError> {
    let response = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("x-xbl-contract-version", "1")
        .json(&serde_json::json!({
            "RelyingParty": relying_party.as_str(),
            "TokenType": "JWT",
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [user_token],
            }
        }))
        .timeout(DEFAULT_TIMEOUT)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(XboxError::HttpStatus {
            url: url.to_string(),
            status: response.status(),
        });
    }

    Ok(response.json::<XstsToken>().await?)
}
