use color_eyre::{eyre::eyre, Result};

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

pub async fn set_viewership(spade_url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let res = client
        .post(spade_url)
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", CHROME_USER_AGENT)
        .send()
        .await?;

    Ok(())
}
