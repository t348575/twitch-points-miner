use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE, Engine};
use eyre::{eyre, Context, ContextCompat, Result};
use serde_json::json;
use twitch_api::types::UserId;

use crate::{twitch::DEVICE_ID, types::StreamerInfo};

use super::{traverse_json, CHROME_USER_AGENT, CLIENT_ID};

pub async fn get_spade_info(#[cfg(feature = "testing")] base_url: &str) -> Result<SpadeInfo> {
    #[cfg(feature = "testing")]
    let manifest_url = format!("{base_url}/manifest.json");
    #[cfg(not(feature = "testing"))]
    let manifest_url = "https://assets.twitch.tv/config/manifest.json".to_owned();

    let client = reqwest::Client::new();
    let mut manifest: serde_json::Value = client
        .get(&manifest_url)
        .header("User-Agent", CHROME_USER_AGENT)
        .send()
        .await?
        .json()
        .await?;

    let app_version = traverse_json(&mut manifest, ".channels[0].releases[0].buildId")
        .context("buildId does not exist")?
        .as_str()
        .unwrap()
        .to_owned();

    #[cfg(feature = "testing")]
    let uri = format!("{base_url}/");
    #[cfg(not(feature = "testing"))]
    let uri = "https://assets.twitch.tv/config/";
    let client = reqwest::Client::new();
    let text = client
        .get(&format!("{uri}settings.js"))
        .header("User-Agent", CHROME_USER_AGENT)
        .send()
        .await?
        .text()
        .await?;
    match text.split_once(r#""spade_url":""#) {
        Some((_, after)) => match after.split_once('"') {
            Some((url, _)) => Ok(SpadeInfo {
                url: url.to_string(),
                app_version,
            }),
            None => Err(eyre!(r#"Failed to get spade url: ""#)),
        },
        None => Err(eyre!(r#"Failed to get spade url: "spade_url":""#)),
    }
}

#[derive(Debug, Default, Clone)]
pub struct SpadeInfo {
    pub url: String,
    pub app_version: String,
}

pub async fn set_viewership(
    user_name: String,
    user_id: u32,
    channel_id: UserId,
    info: StreamerInfo,
    spade_info: &SpadeInfo,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let body = serde_json::to_string(&[json!({
        "event": "minute-watched",
        "properties": {
            "app_version": spade_info.app_version,
            "batch_time": now,
            "client_time": now,
            "device_id": CLIENT_ID,
            "domain": "www.twitch.tv",
            "host": "www.twitch.tv",
            "platform": "web",
            "url": format!("https://www.twitch.tv/{}", info.channel_name),
            "browser": CHROME_USER_AGENT,
            "location": "channel",
            "session_device_id": CLIENT_ID,
            "channel": info.channel_name,
            "channel_id": channel_id,
            "is_live": true,
            "game": info.game.clone().map(|x| x.name),
            "game_id": info.game.map(|x| x.id),
            "backend": "mediaplayer",
            "player": "site",
            "broadcast_id": info.broadcast_id,
            "hidden": false,
            "live": true,
            "low_latency": true,
            "muted": false,
            "logged_in": true,
            "login": user_name,
            "user_id": user_id,
            "client_build_id": spade_info.app_version,
            "distinct_id": CLIENT_ID,
            "client_app": "twilight"
        }
    })]).context("Building body")?;

    let client = reqwest::Client::new();
    let res = client
        .post(&spade_info.url)
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", CHROME_USER_AGENT)
        .header("X-Device-Id", DEVICE_ID)
        .form(&[("data", &URL_SAFE.encode(body))])
        .send()
        .await.context("Sending request")?;

    if !res.status().is_success() {
        return Err(eyre!("Failed to set viewership {}. Response: {}", res.status(), res.text().await?));
    }

    Ok(())
}
