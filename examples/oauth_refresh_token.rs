//! One-time Microsoft browser sign-in that prints a new Xbox OAuth refresh token.
//!
//! Register `http://localhost:8765/callback` as a redirect URI for your app, then run:
//! `XBOX_OAUTH_REDIRECT_URI=http://localhost:8765/callback cargo run --example oauth_refresh_token`
//! The token is printed to stdout and is never saved by this example.

use std::env;
use std::error::Error;
use std::time::Duration;

use reqwest::Url;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use xbox::auth::OAuthClient;

type ExampleError = Box<dyn Error>;

fn callback_uri() -> Result<Url, ExampleError> {
    let raw = env::var("XBOX_OAUTH_REDIRECT_URI")
        .map_err(|_| "set XBOX_OAUTH_REDIRECT_URI to your registered localhost callback URI")?;
    let uri = Url::parse(&raw)?;
    if uri.scheme() != "http"
        || !matches!(uri.host_str(), Some("localhost" | "127.0.0.1"))
        || uri.port().is_none()
        || uri.query().is_some()
        || uri.fragment().is_some()
    {
        return Err(
            "redirect URI must be http://localhost:PORT/path or http://127.0.0.1:PORT/path".into(),
        );
    }
    Ok(uri)
}

fn code_from_callback(
    target: &str,
    expected_path: &str,
    state: &str,
) -> Result<String, ExampleError> {
    let uri =
        Url::parse(&format!("http://localhost{target}")).map_err(|_| "invalid OAuth callback")?;
    if uri.path() != expected_path {
        return Err("unexpected OAuth callback path".into());
    }
    let params = uri.query_pairs();
    let returned_state = params
        .clone()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned());
    if returned_state.as_deref() != Some(state) {
        return Err("OAuth state did not match".into());
    }
    params
        .filter(|(key, _)| key == "code")
        .map(|(_, value)| value.into_owned())
        .find(|value| !value.is_empty())
        .ok_or_else(|| "OAuth callback contained no authorization code".into())
}

async fn receive_code(
    listener: TcpListener,
    path: &str,
    state: &str,
) -> Result<String, ExampleError> {
    loop {
        let (mut stream, peer) = listener.accept().await?;
        if !peer.ip().is_loopback() {
            continue;
        }
        let mut request = [0_u8; 8192];
        let count = stream.read(&mut request).await?;
        let line = std::str::from_utf8(&request[..count])
            .ok()
            .and_then(|request| request.lines().next());
        let target = line
            .and_then(|line| line.strip_prefix("GET "))
            .and_then(|rest| rest.split_once(' '))
            .map(|(target, _)| target);
        let Some(target) = target else { continue };
        if !target.starts_with(path) {
            continue;
        }
        let result = code_from_callback(target, path, state);
        let body = if result.is_ok() {
            "Sign-in complete. You may close this tab."
        } else {
            "Sign-in failed. Return to the terminal for details."
        };
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(reply.as_bytes()).await?;
        return result;
    }
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let redirect = callback_uri()?;
    let client = OAuthClient::new(
        env::var("XBOX_OAUTH_CLIENT_ID")?,
        env::var("XBOX_OAUTH_CLIENT_SECRET")?,
    );
    let port = redirect.port().expect("validated port");
    let listener = TcpListener::bind(("127.0.0.1", port)).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::AddrInUse {
            format!("localhost port {port} is already in use; choose another redirect URI registered for your Microsoft app")
        } else {
            format!("failed to listen on localhost port {port}: {error}")
        }
    })?;
    let state = uuid::Uuid::new_v4().to_string();
    let authorize = client.authorization_url(redirect.as_str(), &state)?;

    eprintln!(
        "Complete Microsoft sign-in in your browser. If it does not open, visit:\n{authorize}\n"
    );
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg(authorize.as_str())
        .status();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open")
        .arg(authorize.as_str())
        .status();

    let code = tokio::time::timeout(
        Duration::from_secs(300),
        receive_code(listener, redirect.path(), &state),
    )
    .await
    .map_err(|_| "timed out waiting for Microsoft sign-in")??;
    let tokens = client.exchange_code(redirect.as_str(), &code).await?;
    println!("\nYour refresh token:\n{}", tokens.refresh_token);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::code_from_callback;

    #[test]
    fn accepts_only_matching_callback_and_state() {
        assert_eq!(
            code_from_callback(
                "/callback?state=expected&code=secret",
                "/callback",
                "expected"
            )
            .unwrap(),
            "secret"
        );
        assert!(
            code_from_callback("/callback?state=other&code=secret", "/callback", "expected")
                .is_err()
        );
        assert!(
            code_from_callback("/other?state=expected&code=secret", "/callback", "expected")
                .is_err()
        );
        assert!(code_from_callback("/callback?state=expected", "/callback", "expected").is_err());
    }
}
