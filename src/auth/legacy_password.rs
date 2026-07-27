//! Username/password sign-in to Xbox Live by scraping the Microsoft login page, the same way
//! the Xbox Live web login flow works under the hood.
//!
//! This is **not an officially documented API** — it depends on the exact markup/JS shape of
//! `login.live.com`'s login page and may break if Microsoft changes it. It exists to support
//! use cases where an interactive OAuth2 device-code flow isn't practical (e.g. a fully
//! unattended service account). Prefer a real OAuth2 provider once one exists for anything
//! else.

use std::sync::LazyLock;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use reqwest::redirect::Policy;
use serde::Serialize;

use crate::auth::provider::XblAuthProvider;
use crate::cache::CachedToken;
use crate::error::XboxError;
use crate::models::UserToken;

static PPFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"<input type=\\?"hidden\\?" name=\\?"PPFT\\?" id=\\?"i0327\\?" value=\\?"(.*?)\\?"/>"#,
    )
    .expect("PPFT_RE is a valid regex")
});
static POST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\\?"urlPost\\?":\\?"(.*?)\\?""#).expect("POST_RE is a valid regex")
});
static RPS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r".*access_token=(.*?)&").expect("RPS_RE is a valid regex"));

#[derive(Debug, Serialize)]
struct LoginForm {
    login: String,
    passwd: String,
    #[serde(rename = "PPFT")]
    ppft: String,
    #[serde(rename = "loginoptions")]
    login_options: String,
}

/// Signs in to Xbox Live using a plain username and password, via the same login flow the
/// Xbox Live website itself uses.
///
/// Ported from a hand-rolled scrape flow; see the module-level docs for caveats.
pub struct LegacyPasswordProvider {
    username: String,
    password: String,
}

impl LegacyPasswordProvider {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

#[async_trait]
impl XblAuthProvider for LegacyPasswordProvider {
    async fn user_token(&self) -> Result<CachedToken<String>, XboxError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .cookie_store(true)
            .build()?;

        let (ppft, post_url) = ppft_and_post(&client).await?;
        let rps_ticket =
            live_login(&client, post_url, &self.username, &self.password, ppft).await?;
        let user_token = get_user_token(&client, &rps_ticket).await?;

        Ok(CachedToken::new(user_token.token, user_token.not_after))
    }
}

async fn ppft_and_post(client: &Client) -> Result<(String, String), XboxError> {
    let url = "https://login.live.com/oauth20_authorize.srf?response_type=token&\
        redirect_uri=https://login.live.com/oauth20_desktop.srf&\
        scope=service::user.auth.xboxlive.com::MBI_SSL&client_id=000000004C12AE6F";

    let response = client
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(XboxError::HttpStatus {
            url: url.to_string(),
            status: response.status(),
        });
    }
    let body = response.text().await?;

    let ppft = PPFT_RE
        .captures(&body)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or(XboxError::PpftNotFound)?;

    let post_url = POST_RE
        .captures(&body)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or(XboxError::PostUrlNotFound)?;

    Ok((ppft, post_url))
}

async fn live_login(
    client: &Client,
    post_url: String,
    username: &str,
    password: &str,
    ppft: String,
) -> Result<String, XboxError> {
    let form = LoginForm {
        login: username.to_string(),
        passwd: password.to_string(),
        ppft,
        login_options: "3".to_string(),
    };

    let response = client
        .post(&post_url)
        .form(&form)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    let location = response
        .headers()
        .get("Location")
        .and_then(|v| v.to_str().ok())
        .ok_or(XboxError::MissingRedirectLocation)?;

    RPS_RE
        .captures(location)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or(XboxError::RpsTicketNotFound)
}

async fn get_user_token(client: &Client, rps_ticket: &str) -> Result<UserToken, XboxError> {
    let url = "https://user.auth.xboxlive.com/user/authenticate";
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("x-xbl-contract-version", "1")
        .json(&serde_json::json!({
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={rps_ticket}"),
            }
        }))
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(XboxError::HttpStatus {
            url: url.to_string(),
            status: response.status(),
        });
    }

    Ok(response.json::<UserToken>().await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppft() {
        let example = r#""sFTTag":"<input type="hidden" name="PPFT" id="i0327" value="-DuVDERK6Xc4y!W6TFISm6lRFQ2fsCIP1vhyC9hyCT62Ecw4SqPht3EGC1KcUx8vNKhwDDO5yJ!bO!ErUA8VhLUtIii6xG6uxTkzaoOdXBV4rMQweK6ii!dOR*mhTdQqzTxCbkf2oBsvQQVTgGR99TWAlHbNFtS3LRwVIGSKJehlgKk9eLVdSTfO1YwTa51qczv!38Lqh*kbBcUfQ4Zfz0a!OqFTdGTl8q*6cm7P9LEeVhs3q0L92uP5Cu*!AV*lNEg$$"/>"#;
        let ppft = PPFT_RE.captures(example);
        assert!(ppft.is_some(), "PPFT should be captured");
        let ppft = ppft.unwrap().get(1);
        assert!(ppft.is_some(), "PPFT should be 1 token");
        let ppft = ppft.unwrap().as_str();
        assert_eq!(
            ppft,
            "-DuVDERK6Xc4y!W6TFISm6lRFQ2fsCIP1vhyC9hyCT62Ecw4SqPht3EGC1KcUx8vNKhwDDO5yJ!bO!ErUA8VhLUtIii6xG6uxTkzaoOdXBV4rMQweK6ii!dOR*mhTdQqzTxCbkf2oBsvQQVTgGR99TWAlHbNFtS3LRwVIGSKJehlgKk9eLVdSTfO1YwTa51qczv!38Lqh*kbBcUfQ4Zfz0a!OqFTdGTl8q*6cm7P9LEeVhs3q0L92uP5Cu*!AV*lNEg$$"
        );
    }

    #[test]
    fn test_post() {
        let example = r#""urlPost":"https://login.live.com/ppsecure/post.srf?client_id=000000004C12AE6F&contextid=5B318DF9700F9B69&opid=CF610FC6C48CE9DC&bk=1757300168&uaid=01b2c4913fdd4f6bb10d5253ee938646&pid=15216""#;
        let post = POST_RE.captures(example);
        assert!(post.is_some(), "urlPost should be captured");
        let post = post.unwrap().get(1);
        assert!(post.is_some(), "urlPost should be 1 token");
        let post = post.unwrap().as_str();
        assert_eq!(
            post,
            "https://login.live.com/ppsecure/post.srf?client_id=000000004C12AE6F&contextid=5B318DF9700F9B69&opid=CF610FC6C48CE9DC&bk=1757300168&uaid=01b2c4913fdd4f6bb10d5253ee938646&pid=15216"
        );
    }

    #[test]
    fn test_rps_ticket() {
        let location = "https://login.live.com/oauth20_desktop.srf?access_token=EwAAA...&token_type=bearer&expires_in=3600";
        let captures = RPS_RE.captures(location);
        assert!(captures.is_some());
        assert_eq!(captures.unwrap().get(1).unwrap().as_str(), "EwAAA...");
    }
}
