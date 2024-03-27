use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use twitch_api::{
    helix::Scope,
    twitch_oauth2::{
        client::Client,
        tokens::{errors::RefreshTokenError, BearerTokenType},
        AccessToken, ClientId, TwitchToken,
    },
    types::{UserIdRef, UserNameRef},
};

use crate::common::{CLIENT_ID, DEVICE_ID, USER_AGENT};

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

pub struct TwitchApiToken {
    client_id: ClientId,
    access_token: AccessToken,
    scopes: [Scope; 0],
}

impl From<Token> for TwitchApiToken {
    fn from(value: Token) -> Self {
        TwitchApiToken {
            client_id: ClientId::new(CLIENT_ID.to_owned()),
            access_token: AccessToken::new(value.access_token),
            scopes: [],
        }
    }
}

#[async_trait::async_trait]
impl TwitchToken for TwitchApiToken {
    fn token_type() -> BearerTokenType {
        BearerTokenType::UserToken
    }

    fn client_id(&self) -> &ClientId {
        &self.client_id
    }

    fn token(&self) -> &AccessToken {
        &self.access_token
    }

    fn login(&self) -> Option<&UserNameRef> {
        None
    }

    fn user_id(&self) -> Option<&UserIdRef> {
        None
    }

    fn expires_in(&self) -> std::time::Duration {
        std::time::Duration::MAX
    }

    fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    async fn refresh_token<'a, C>(
        &mut self,
        _: &'a C,
    ) -> Result<(), RefreshTokenError<<C as Client>::Error>>
    where
        Self: Sized,
        C: Client,
    {
        Ok(())
    }
}

pub async fn login(tokens: &str) -> Result<()> {
    let client = reqwest::Client::new();

    let res = client.post("https://id.twitch.tv/oauth2/device")
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", USER_AGENT)
        .header("X-Device-Id", DEVICE_ID)
        .form(&std::collections::HashMap::from([
            ("client_id", CLIENT_ID),
            ("scopes", "channel_read chat:read user_blocks_edit user_blocks_read user_follows_edit user_read")
        ])).send().await.unwrap();

    let flow = res.json::<LoginFlowStart>().await.unwrap();

    println!(
        "Open https://www.twitch.tv/activate and enter this code: {}\nPress enter once done!",
        flow.user_code
    );
    let mut buf = [0u8; 1];
    tokio::io::stdin().read_exact(&mut buf).await?;

    let res = client
        .post("https://id.twitch.tv/oauth2/token")
        .header("Client-Id", CLIENT_ID)
        .header("Host", "id.twitch.tv")
        .header("Origin", "https://android.tv.twitch.tv")
        .header("Refer", "https://android.tv.twitch.tv")
        .header("User-Agent", USER_AGENT)
        .header("X-Device-Id", DEVICE_ID)
        .form(&std::collections::HashMap::from([
            ("client_id", CLIENT_ID),
            ("device_code", &flow.device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ]))
        .send()
        .await
        .unwrap();

    tokio::fs::write(
        tokens,
        serde_json::to_string(&res.json::<Token>().await.context("Parsing tokens")?)?,
    )
    .await
    .context("Writing tokens file")?;
    Ok(())
}
