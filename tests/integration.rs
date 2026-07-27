use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use xbox::auth::{RelyingParty, XblAuthProvider};
use xbox::cache::CachedToken;
use xbox::{XboxClient, XboxEndpoints, XboxError};

/// Hands out a fixed, never-expiring user token without touching the network — this crate's
/// login-page-scraping flow is exercised separately by unit tests on its own regexes, not by
/// this integration suite.
struct FakeAuthProvider {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl XblAuthProvider for FakeAuthProvider {
    async fn user_token(&self) -> Result<CachedToken<String>, XboxError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CachedToken::new(
            "fake-user-token".to_string(),
            Utc::now() + Duration::hours(1),
        ))
    }
}

fn xsts_body(not_after: chrono::DateTime<Utc>) -> serde_json::Value {
    serde_json::json!({
        "Token": "fake-xsts-token",
        "NotAfter": not_after.to_rfc3339(),
        "DisplayClaims": { "xui": [{ "uhs": "fake-uhs" }] }
    })
}

fn people_search_body(gamertag: &str, xuid: &str) -> serde_json::Value {
    serde_json::json!({
        "people": [{ "gamertag": gamertag, "xuid": xuid }]
    })
}

#[tokio::test]
async fn resolves_gamertag_to_xuid_end_to_end() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/xsts/authorize"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(xsts_body(Utc::now() + Duration::hours(1))),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/users/me/people/search/decoration/detail"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(people_search_body("Some Gamertag", "123456789")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let calls = Arc::new(AtomicUsize::new(0));
    let client = XboxClient::with_endpoints(
        FakeAuthProvider {
            calls: calls.clone(),
        },
        reqwest::Client::new(),
        XboxEndpoints {
            xsts_authorize_url: format!("{}/xsts/authorize", server.uri()),
            peoplehub_base_url: server.uri(),
        },
    );

    let xuid = client.gamertag_to_xuid("Some Gamertag").await.unwrap();
    assert_eq!(xuid.as_str(), "123456789");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn xuid_resolution_is_cached_on_second_call() {
    let server = MockServer::start().await;

    // `.expect(1)` on each mock asserts the underlying HTTP call happens exactly once even
    // though we resolve the same gamertag twice below — this is the caching fix this crate
    // exists to provide.
    Mock::given(method("POST"))
        .and(path("/xsts/authorize"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(xsts_body(Utc::now() + Duration::hours(1))),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/users/me/people/search/decoration/detail"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(people_search_body("Some Gamertag", "123456789")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let calls = Arc::new(AtomicUsize::new(0));
    let client = XboxClient::with_endpoints(
        FakeAuthProvider {
            calls: calls.clone(),
        },
        reqwest::Client::new(),
        XboxEndpoints {
            xsts_authorize_url: format!("{}/xsts/authorize", server.uri()),
            peoplehub_base_url: server.uri(),
        },
    );

    let first = client.gamertag_to_xuid("Some Gamertag").await.unwrap();
    let second = client.gamertag_to_xuid("Some Gamertag").await.unwrap();

    assert_eq!(first.as_str(), "123456789");
    assert_eq!(second.as_str(), "123456789");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "user token should only be fetched once across both gamertag lookups"
    );
}

#[tokio::test]
async fn person_not_found_when_no_exact_match() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/xsts/authorize"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(xsts_body(Utc::now() + Duration::hours(1))),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/users/me/people/search/decoration/detail"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(people_search_body("Someone Else", "999")),
        )
        .mount(&server)
        .await;

    let client = XboxClient::with_endpoints(
        FakeAuthProvider {
            calls: Arc::new(AtomicUsize::new(0)),
        },
        reqwest::Client::new(),
        XboxEndpoints {
            xsts_authorize_url: format!("{}/xsts/authorize", server.uri()),
            peoplehub_base_url: server.uri(),
        },
    );

    let err = client.gamertag_to_xuid("Some Gamertag").await.unwrap_err();
    assert!(matches!(err, XboxError::PersonNotFound(gt) if gt == "Some Gamertag"));
}

#[tokio::test]
async fn xsts_ticket_is_scoped_per_relying_party() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/xsts/authorize"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(xsts_body(Utc::now() + Duration::hours(1))),
        )
        .mount(&server)
        .await;

    let client = XboxClient::with_endpoints(
        FakeAuthProvider {
            calls: Arc::new(AtomicUsize::new(0)),
        },
        reqwest::Client::new(),
        XboxEndpoints {
            xsts_authorize_url: format!("{}/xsts/authorize", server.uri()),
            peoplehub_base_url: server.uri(),
        },
    );

    let xbox_ticket = client.xsts_ticket(RelyingParty::Xbox).await.unwrap();
    let halo_ticket = client.xsts_ticket(RelyingParty::Halo).await.unwrap();

    assert_eq!(xbox_ticket.token, "fake-xsts-token");
    assert_eq!(halo_ticket.token, "fake-xsts-token");
}
