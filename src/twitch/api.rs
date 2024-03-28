use color_eyre::{eyre::eyre, Result};
use serde::Serialize;
use twitch_api::types::UserId;

use crate::{
    twitch::DEVICE_ID,
    types::{MinuteWatched, StreamerInfo},
};

use super::{CHROME_USER_AGENT, CLIENT_ID, FIREFOX_USER_AGENT};

pub async fn get_spade_url(streamer: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("https://www.twitch.tv/{streamer}"))
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", FIREFOX_USER_AGENT)
        .send()
        .await?;

    let page_text = res.text().await?;
    match page_text.split_once("https://static.twitchcdn.net/config/settings.") {
        Some((_, after)) => match after.split_once(".js") {
            Some((pattern_js, _)) => Ok(format!(
                "https://static.twitchcdn.net/config/settings.{}.js",
                pattern_js
            )),
            None => Err(eyre!("Failed to get spade url")),
        },
        None => Err(eyre!("Failed to get spade url")),
    }
}

pub async fn set_viewership(
    user_id: u32,
    channel_id: UserId,
    info: StreamerInfo,
    spade_url: &str,
    access_token: &str,
) -> Result<()> {
    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        pub event: &'static str,
        pub properties: MinuteWatched,
    }

    let watch_event = Root {
        event: "minute-watched",
        properties: MinuteWatched::from_streamer_info(user_id, channel_id, info),
    };

    let client = reqwest::Client::new();
    let res = client
        .post(spade_url)
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", CHROME_USER_AGENT)
        .header("X-Device-Id", DEVICE_ID)
        .header("Authorization", format!("OAuth {}", access_token))
        .json(&watch_event)
        .send()
        .await?;

    res.json::<serde_json::Value>().await?;

    Ok(())
}
