//! Microsoft authorization-code and refresh-token authentication for Xbox Live.
//!
//! Register an application that accepts personal Microsoft accounts and grants
//! `XboxLive.signin` and `XboxLive.offline_access`. Complete authorization once
//! in a browser, then persist the refresh token in a secret store. The provider
//! rotates that token whenever Microsoft returns a replacement. Access and
//! refresh tokens are never included in errors or `Debug` output.

use async_trait::async_trait;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::time::Duration;

use crate::auth::XblAuthProvider;
use crate::cache::CachedToken;
use crate::error::XboxError;
use crate::models::UserToken;

const SCOPES: &str = "XboxLive.signin XboxLive.offline_access";
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Microsoft and Xbox endpoints. Override these for tests or private gateways.
#[derive(Clone)]
pub struct OAuthEndpoints {
    pub authorize_url: String,
    pub token_url: String,
    pub xbox_user_authenticate_url: String,
}

impl Default for OAuthEndpoints {
    fn default() -> Self {
        Self {
            authorize_url: "https://login.live.com/oauth20_authorize.srf".into(),
            token_url: "https://login.live.com/oauth20_token.srf".into(),
            xbox_user_authenticate_url: "https://user.auth.xboxlive.com/user/authenticate".into(),
        }
    }
}

/// A successful Microsoft OAuth exchange. Treat both fields as secrets.
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

/// Authorization URL generation and Microsoft/Xbox token exchanges.
pub struct OAuthClient {
    client_id: String,
    client_secret: String,
    http: Client,
    endpoints: OAuthEndpoints,
    request_timeout: Duration,
}

impl OAuthClient {
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self::with_endpoints(client_id, client_secret, OAuthEndpoints::default())
    }

    pub fn with_endpoints(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        endpoints: OAuthEndpoints,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            http: Client::new(),
            endpoints,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// Set the timeout for each Microsoft token and Xbox user-authentication
    /// request. The default is 10 seconds per request.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Build a browser URL for the one-time authorization-code grant. The caller
    /// must generate and verify a random `state`, and use a registered redirect URI.
    pub fn authorization_url(&self, redirect_uri: &str, state: &str) -> Result<Url, XboxError> {
        if self.client_id.is_empty() || redirect_uri.is_empty() || state.is_empty() {
            return Err(XboxError::OAuthConfiguration);
        }
        let mut url =
            Url::parse(&self.endpoints.authorize_url).map_err(|_| XboxError::OAuthConfiguration)?;
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("response_type", "code")
            .append_pair("scope", SCOPES)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", state);
        Ok(url)
    }

    /// Exchange the one-time authorization code for tokens. Persist
    /// `refresh_token` securely; the authorization code cannot be reused.
    pub async fn exchange_code(
        &self,
        redirect_uri: &str,
        code: &str,
    ) -> Result<OAuthTokens, XboxError> {
        if redirect_uri.is_empty() || code.is_empty() {
            return Err(XboxError::OAuthConfiguration);
        }
        let fields = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("scope", SCOPES),
        ];
        let response = self.request_token(&fields).await?;
        let refresh_token = response
            .refresh_token
            .filter(|token| !token.is_empty())
            .ok_or(XboxError::OAuthIncompleteResponse)?;
        Ok(OAuthTokens {
            access_token: response.access_token,
            refresh_token,
        })
    }

    /// Refresh an OAuth access token and keep the prior refresh token if the
    /// provider does not send a replacement.
    pub async fn refresh(&self, refresh_token: &str) -> Result<OAuthTokens, XboxError> {
        if refresh_token.is_empty() || self.client_id.is_empty() {
            return Err(XboxError::OAuthConfiguration);
        }
        let fields = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("scope", SCOPES),
        ];
        let response = self.request_token(&fields).await?;
        Ok(OAuthTokens {
            access_token: response.access_token,
            refresh_token: response
                .refresh_token
                .filter(|token| !token.is_empty())
                .unwrap_or_else(|| refresh_token.to_owned()),
        })
    }

    async fn request_token(&self, fields: &[(&str, &str)]) -> Result<TokenResponse, XboxError> {
        let response = self
            .http
            .post(&self.endpoints.token_url)
            .form(fields)
            .timeout(self.request_timeout)
            .send()
            .await?;
        if !response.status().is_success() {
            let code = response
                .json::<ErrorResponse>()
                .await
                .ok()
                .map(|body| body.error);
            let category = match code.as_deref() {
                Some("invalid_grant") => "invalid_grant",
                Some("invalid_client") => "invalid_client",
                Some("interaction_required") => "interaction_required",
                _ => "other",
            };
            return Err(XboxError::OAuthRejected(category));
        }
        let token = response
            .json::<TokenResponse>()
            .await
            .map_err(|_| XboxError::OAuthIncompleteResponse)?;
        if token.access_token.is_empty() {
            return Err(XboxError::OAuthIncompleteResponse);
        }
        Ok(token)
    }

    async fn xbox_user_token(&self, access_token: &str) -> Result<UserToken, XboxError> {
        let url = &self.endpoints.xbox_user_authenticate_url;
        let response = self.http.post(url)
            .header("x-xbl-contract-version", "1")
            .json(&serde_json::json!({
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT",
                "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={access_token}")}
            }))
            .timeout(self.request_timeout).send().await?;
        if !response.status().is_success() {
            return Err(XboxError::HttpStatus {
                url: url.clone(),
                status: response.status(),
            });
        }
        response
            .json::<UserToken>()
            .await
            .map_err(|_| XboxError::OAuthIncompleteResponse)
    }
}

/// Persist refresh tokens in a durable, restricted store. The conditional write
/// must be atomic across every process using the same Microsoft account.
#[async_trait]
pub trait RefreshTokenStore: Send + Sync {
    async fn load(&self) -> Result<String, XboxError>;
    /// Replace `expected` with `replacement` only when `expected` is still
    /// current. Return `false` when another process has already replaced it.
    async fn compare_and_swap(&self, expected: &str, replacement: &str) -> Result<bool, XboxError>;
}

/// Supplies Xbox user tokens through Microsoft OAuth refresh grants.
pub struct OAuthRefreshTokenProvider<S> {
    oauth: OAuthClient,
    store: S,
}

impl<S: RefreshTokenStore> OAuthRefreshTokenProvider<S> {
    pub fn new(oauth: OAuthClient, store: S) -> Self {
        Self { oauth, store }
    }
}

#[async_trait]
impl<S: RefreshTokenStore> XblAuthProvider for OAuthRefreshTokenProvider<S> {
    async fn user_token(&self) -> Result<CachedToken<String>, XboxError> {
        let mut current = self.store.load().await?;
        let tokens = match self.oauth.refresh(&current).await {
            Ok(tokens) => tokens,
            Err(XboxError::OAuthRejected("invalid_grant")) => {
                // Another process may have rotated the shared secret first.
                let latest = self.store.load().await?;
                if latest == current {
                    return Err(XboxError::OAuthRejected("invalid_grant"));
                }
                let refreshed = self.oauth.refresh(&latest).await?;
                current = latest;
                refreshed
            }
            Err(error) => return Err(error),
        };
        if tokens.refresh_token != current {
            // A losing writer can still use its short-lived access token. Its
            // replacement refresh token must not overwrite a newer one.
            self.store
                .compare_and_swap(&current, &tokens.refresh_token)
                .await?;
        }
        let user = self.oauth.xbox_user_token(&tokens.access_token).await?;
        Ok(CachedToken::new(user.token, user.not_after))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as ChronoDuration, Utc};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Clone)]
    struct Store(Arc<Mutex<String>>);

    #[async_trait]
    impl RefreshTokenStore for Store {
        async fn load(&self) -> Result<String, XboxError> {
            Ok(self.0.lock().await.clone())
        }

        async fn compare_and_swap(
            &self,
            expected: &str,
            replacement: &str,
        ) -> Result<bool, XboxError> {
            let mut token = self.0.lock().await;
            if *token != expected {
                return Ok(false);
            }
            *token = replacement.to_owned();
            Ok(true)
        }
    }

    fn client(server: &MockServer) -> OAuthClient {
        let base = server.uri();
        OAuthClient::with_endpoints(
            "test-client",
            "test-secret",
            OAuthEndpoints {
                authorize_url: format!("{base}/authorize"),
                token_url: format!("{base}/token"),
                xbox_user_authenticate_url: format!("{base}/user/authenticate"),
            },
        )
    }

    #[test]
    fn authorization_url_has_code_grant_scopes_and_state() {
        let oauth = OAuthClient::new("test-client", "test-secret");
        let url = oauth
            .authorization_url("https://example.test/callback", "random-state")
            .unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query["response_type"], "code");
        assert_eq!(query["scope"], SCOPES);
        assert_eq!(query["redirect_uri"], "https://example.test/callback");
        assert_eq!(query["state"], "random-state");
    }

    #[tokio::test]
    async fn code_exchange_sends_expected_form() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("code=one-time-code"))
            .and(body_string_contains("client_secret=test-secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "access", "refresh_token": "refresh"
            })))
            .expect(1)
            .mount(&server)
            .await;
        let tokens = client(&server)
            .exchange_code("https://example.test/callback", "one-time-code")
            .await
            .unwrap();
        assert_eq!(tokens.access_token, "access");
        assert_eq!(tokens.refresh_token, "refresh");
    }

    #[tokio::test]
    async fn configured_timeout_applies_to_microsoft_token_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(200)))
            .mount(&server)
            .await;

        let error = client(&server)
            .with_request_timeout(Duration::from_millis(20))
            .refresh("refresh")
            .await
            .err()
            .expect("request should time out");
        assert!(matches!(error, XboxError::Network(error) if error.is_timeout()));
    }

    #[tokio::test]
    async fn configured_timeout_applies_to_xbox_user_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/user/authenticate"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(200)))
            .mount(&server)
            .await;

        let error = client(&server)
            .with_request_timeout(Duration::from_millis(20))
            .xbox_user_token("access")
            .await
            .expect_err("request should time out");
        assert!(matches!(error, XboxError::Network(error) if error.is_timeout()));
    }

    #[tokio::test]
    async fn refresh_rotates_token_and_uses_d_ticket() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains("refresh_token=old"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "access", "refresh_token": "new"
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/user/authenticate"))
            .and(body_string_contains("\"RpsTicket\":\"d=access\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Token": "user-token", "NotAfter": (Utc::now() + ChronoDuration::hours(1)).to_rfc3339()
            })))
            .expect(1)
            .mount(&server).await;
        let store = Store(Arc::new(Mutex::new("old".into())));
        let provider = OAuthRefreshTokenProvider::new(client(&server), store.clone());
        let user = provider.user_token().await.unwrap();
        assert_eq!(user.value, "user-token");
        assert_eq!(store.load().await.unwrap(), "new");
    }

    #[tokio::test]
    async fn invalid_grant_does_not_expose_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant", "error_description": "sensitive-response"
            })))
            .mount(&server)
            .await;
        let error = client(&server)
            .refresh("sensitive-refresh")
            .await
            .err()
            .unwrap();
        let rendered = format!("{error:?} {error}");
        assert!(matches!(error, XboxError::OAuthRejected("invalid_grant")));
        assert!(!rendered.contains("sensitive"));
    }

    #[tokio::test]
    async fn retries_with_a_token_rotated_by_another_process() {
        struct RotatedStore(Arc<Mutex<usize>>);

        #[async_trait]
        impl RefreshTokenStore for RotatedStore {
            async fn load(&self) -> Result<String, XboxError> {
                let mut calls = self.0.lock().await;
                *calls += 1;
                Ok(if *calls == 1 { "old" } else { "current" }.into())
            }

            async fn compare_and_swap(
                &self,
                expected: &str,
                replacement: &str,
            ) -> Result<bool, XboxError> {
                assert_eq!(expected, "current");
                assert_eq!(replacement, "replacement");
                Ok(true)
            }
        }

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("refresh_token=old"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"error": "invalid_grant"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("refresh_token=current"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "access", "refresh_token": "replacement"
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/user/authenticate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Token": "user-token", "NotAfter": (Utc::now() + ChronoDuration::hours(1)).to_rfc3339()
            })))
            .mount(&server).await;
        let provider =
            OAuthRefreshTokenProvider::new(client(&server), RotatedStore(Arc::new(Mutex::new(0))));
        assert_eq!(provider.user_token().await.unwrap().value, "user-token");
    }
}
