use eyre::{eyre, Context, Result};
use serde::{Deserialize, Serialize};

use super::{CLIENT_ID, DEVICE_ID, USER_AGENT};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginFlowStart {
    pub device_code: String,
    pub expires_in: i64,
    pub interval: i64,
    pub user_code: String,
    pub verification_uri: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
    pub scope: Vec<String>,
    pub token_type: String,
}

pub async fn login(tokens: &str) -> Result<()> {
    let flow: LoginFlowStart = ureq::post("https://id.twitch.tv/oauth2/device")
        .set("Client-Id", CLIENT_ID)
        .set("User-Agent", USER_AGENT)
        .set("X-Device-Id", DEVICE_ID)
        .send_form(&[
            ("client_id", CLIENT_ID),
            ("scopes", "channel_read chat:read user_blocks_edit user_blocks_read user_follows_edit user_read")
        ])?.into_json()?;

    if !dialoguer::Confirm::new()
        .with_prompt(format!(
            "Open https://www.twitch.tv/activate and enter this code: {}",
            flow.user_code
        ))
        .interact()?
    {
        return Err(eyre!("User cancelled login"));
    }

    let res: Token = ureq::post("https://id.twitch.tv/oauth2/token")
        .set("Client-Id", CLIENT_ID)
        .set("Host", "id.twitch.tv")
        .set("Origin", "https://android.tv.twitch.tv")
        .set("Refer", "https://android.tv.twitch.tv")
        .set("User-Agent", USER_AGENT)
        .set("X-Device-Id", DEVICE_ID)
        .send_form(&[
            ("client_id", CLIENT_ID),
            ("device_code", &flow.device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])?
        .into_json()?;

    tokio::fs::write(tokens, serde_json::to_string(&res)?)
        .await
        .context("Writing tokens file")?;
    Ok(())
}
