use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Deserializer};

use crate::CLIENT;

static CLIENT_CREDENTIALS_ENDPOINT: &str = "https://accounts.spotify.com/api/token";

#[derive(Deserialize, Debug)]
struct ClientCredentialsResponse {
    access_token: String,
    #[serde(deserialize_with = "deserialize_duration_seconds")]
    expires_in: Duration,
}

fn deserialize_duration_seconds<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let seconds: u64 = Deserialize::deserialize(deserializer)?;
    Ok(Duration::from_secs(seconds))
}

/// Return the Spotify API auth token and start a task to periodically refresh it
pub async fn get_token_and_refresh(client_id: &str, secret: &str) -> Result<Arc<Mutex<String>>> {
    let (access_token, expires_in) = get_token(client_id, secret).await?;
    let token = Arc::new(Mutex::new(access_token));

    // Start refresh task
    {
        let token = token.clone();
        let client_id = client_id.to_string();
        let secret = secret.to_string();
        tokio::spawn(async move {
            let mut expires_in = expires_in;
            loop {
                tokio::time::sleep(expires_in / 2).await;
                let access_token = match get_token(&client_id, &secret).await {
                    Ok((t, e)) => {
                        expires_in = e;
                        t
                    }
                    Err(_) => continue,
                };
                *token.lock().unwrap() = access_token;
            }
        });
    }

    Ok(token)
}

async fn get_token(client_id: &str, secret: &str) -> Result<(String, Duration)> {
    let params = [
        ("grant_type", "client_credentials"),
        ("client_id", client_id),
        ("client_secret", secret),
    ];
    let resp: ClientCredentialsResponse = CLIENT
        .post(CLIENT_CREDENTIALS_ENDPOINT)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await?
        .json()
        .await?;
    Ok((resp.access_token, resp.expires_in))
}
