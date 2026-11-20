//! Manual smoke test against the real Xbox Live API using your own credentials.
//! Native platforms only.
//!
//! ```sh
//! cargo run --example whoami
//! ```
//!
//! Username and password are taken from `XBOX_USERNAME`/`XBOX_PASSWORD` if set, otherwise
//! you're prompted for them interactively (password input is hidden). Either way credentials
//! never touch argv or get committed anywhere, so they don't end up in shell history files
//! that persist or in `ps` output on shared machines.

use std::env;
use std::io::{self, Write};

use xbox::auth::LegacyPasswordProvider;
use xbox::{RelyingParty, XboxClient};

#[cfg(target_arch = "wasm32")]
compile_error!("the whoami example requires a native target");

fn prompt_line(label: &str) -> Result<String, Box<dyn std::error::Error>> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let username = match env::var("XBOX_USERNAME") {
        Ok(username) if !username.trim().is_empty() => username,
        _ => prompt_line("Xbox Live email")?,
    };
    let password = match env::var("XBOX_PASSWORD") {
        Ok(password) if !password.is_empty() => password,
        _ => rpassword::prompt_password("Xbox Live password: ")?,
    };

    let provider = LegacyPasswordProvider::new(&username, &password);
    let client = XboxClient::new(provider);

    let xsts = client.xsts_ticket(RelyingParty::XBOX).await?;
    println!(
        "logged in as {} (xuid {})",
        xsts.gamertag().unwrap_or("<unknown>"),
        xsts.xuid().map(|xuid| xuid.to_string()).unwrap_or_default()
    );
    println!("xsts ticket expires_at={}", xsts.not_after);

    Ok(())
}
