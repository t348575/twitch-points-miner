use base64::{engine::general_purpose::URL_SAFE, Engine};
use color_eyre::{eyre::eyre, Result};
use serde::Serialize;
use twitch_api::types::UserId;

use crate::{
    twitch::DEVICE_ID,
    types::{MinuteWatched, StreamerInfo},
};

use super::{CHROME_USER_AGENT, CLIENT_ID};

pub async fn get_spade_url(streamer: &str, base_url: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{base_url}/{streamer}"))
        .header("User-Agent", CHROME_USER_AGENT)
        .send()
        .await?;

    let page_text = res.text().await?;

    async fn inner(
        client: &reqwest::Client,
        text: &str,
        uri: &str,
        #[cfg(feature = "testing")] base_url: &str,
    ) -> Result<String> {
        match text.split_once(uri) {
            Some((_, after)) => match after.split_once(".js") {
                Some((pattern_js, _)) => {
                    #[cfg(feature = "testing")]
                    let prefix = format!("{base_url}/");
                    #[cfg(not(feature = "testing"))]
                    let prefix = "";
                    let res = client
                        .get(format!("{prefix}{uri}{pattern_js}.js"))
                        .header("User-Agent", CHROME_USER_AGENT)
                        .send()
                        .await?;
                    let text = res.text().await?;
                    match text.split_once(r#""spade_url":""#) {
                        Some((_, after)) => match after.split_once('"') {
                            Some((url, _)) => Ok(url.to_string()),
                            None => Err(eyre!(r#"Failed to get spade url: ""#)),
                        },
                        None => Err(eyre!(r#"Failed to get spade url: "spade_url":""#)),
                    }
                }
                None => Err(eyre!("Failed to get spade url: .js")),
            },
            None => Err(eyre!("Failed to get spade url: {uri}")),
        }
    }

    match inner(
        &client,
        &page_text,
        #[cfg(feature = "testing")]
        "config/settings.",
        #[cfg(not(feature = "testing"))]
        "https://static.twitchcdn.net/config/settings.",
        #[cfg(feature = "testing")]
        base_url,
    )
    .await
    {
        Ok(s) => Ok(s),
        Err(_) => {
            inner(
                &client,
                &page_text,
                "https://assets.twitch.tv/config/settings.",
                #[cfg(feature = "testing")]
                base_url,
            )
            .await
        }
    }
}

pub async fn set_viewership(
    user_name: String,
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
        properties: MinuteWatched::from_streamer_info(user_name, user_id, channel_id, info),
    };

    let body = serde_json::to_string(&[watch_event])?;

    let client = reqwest::Client::new();
    let res = client
        .post(spade_url)
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", CHROME_USER_AGENT)
        .header("X-Device-Id", DEVICE_ID)
        .header("Authorization", format!("OAuth {}", access_token))
        .body(URL_SAFE.encode(body))
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(eyre!("Failed to set viewership"));
    }

    res.text().await?;
    Ok(())
}
