use color_eyre::{eyre::eyre, Result};
use rand::distributions::{Alphanumeric, DistString};
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use twitch_api::types::UserId;

use super::{CLIENT_ID, DEVICE_ID, USER_AGENT};
use crate::types::{Game, StreamerInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GqlRequest<T> {
    #[serde(rename = "operationName")]
    operation_name: String,
    extensions: serde_json::Value,
    variables: T,
}

fn gql_req(access_token: &str) -> RequestBuilder {
    let client = reqwest::Client::new();
    client
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", USER_AGENT)
        .header("X-Device-Id", DEVICE_ID)
        .header("Authorization", format!("OAuth {}", access_token))
}

pub async fn streamer_metadata(
    users: &[&str],
    access_token: &str,
) -> Result<Vec<Option<(UserId, StreamerInfo)>>> {
    #[derive(Debug, Default, Serialize)]
    struct Variables<'a> {
        #[serde(rename = "channelLogin")]
        channel_login: &'a str,
    }

    impl<'a> Default for GqlRequest<Variables<'a>> {
        fn default() -> Self {
            Self {
                operation_name: "StreamMetadata".to_string(),
                extensions: json!({
                    "persistedQuery": {
                        "sha256Hash": "676ee2f834ede42eb4514cdb432b3134fefc12590080c9a2c9bb44a2a4a63266",
                        "version": 1
                    }
                }),
                variables: Default::default(),
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        pub data: Data,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Data {
        pub user: Option<User>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct User {
        pub id: UserId,
        pub stream: Option<Stream>,
    }

    #[derive(Deserialize, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct Stream {
        pub id: UserId,
        pub game: Game,
    }

    impl From<User> for StreamerInfo {
        fn from(value: User) -> Self {
            Self {
                live: value.stream.is_some(),
                broadcast_id: value.stream.clone().map(|x| x.id),
                channel_name: String::new(),
                game: value.stream.map(|x| x.game),
            }
        }
    }

    let users = users
        .iter()
        .map(|user| {
            let mut req = GqlRequest::<Variables>::default();
            req.variables.channel_login = user;
            req
        })
        .collect::<Vec<_>>();

    let res = gql_req(access_token).json(&users).send().await?;
    let items = res.json::<Vec<Root>>().await?;

    Ok(items
        .into_iter()
        .map(|x| match x.data.user {
            Some(s) => Some((s.id.clone(), s.into())),
            None => None,
        })
        .collect())
}

/// Assumes all the channels exist
/// (Channel ID, Broadcast ID)
pub async fn live_channels(
    channels: &[UserId],
    access_token: &str,
) -> Result<Vec<(UserId, Option<UserId>)>> {
    let channels = streamer_metadata(
        &channels.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
        access_token,
    )
    .await?;
    Ok(channels
        .into_iter()
        .filter_map(|x| x)
        .map(|x| (x.0, x.1.broadcast_id.map(|x| x.into())))
        .collect())
}

pub async fn make_prediction(
    points: u32,
    event_id: &str,
    outcome_id: String,
    access_token: &str,
    simulate: bool,
) -> Result<()> {
    if simulate {
        return Ok(());
    }

    #[derive(Debug, Default, Serialize)]
    struct Variables<'a> {
        #[serde(borrow)]
        input: Input<'a>,
    }

    #[derive(Debug, Default, Serialize)]
    struct Input<'a> {
        #[serde(rename = "eventID")]
        event_id: &'a str,
        #[serde(rename = "outcomeID")]
        outcome_id: String,
        points: u32,
        #[serde(rename = "transactionID")]
        transaction_id: String,
    }

    impl<'a> Default for GqlRequest<Variables<'a>> {
        fn default() -> Self {
            Self {
                operation_name: "MakePrediction".to_string(),
                extensions: json!({
                    "persistedQuery": {
                        "version": 1,
                        "sha256Hash": "b44682ecc88358817009f20e69d75081b1e58825bb40aa53d5dbadcc17c881d8",
                    }
                }),
                variables: Default::default(),
            }
        }
    }

    let mut pred = GqlRequest::<Variables>::default();
    pred.variables.input.event_id = event_id;
    pred.variables.input.outcome_id = outcome_id;
    pred.variables.input.points = points;
    pred.variables.input.transaction_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);

    let res = gql_req(access_token).json(&pred).send().await?;

    if !res.status().is_success() {
        return Err(eyre!("Failed to place prediction"));
    }
    Ok(())
}

pub async fn get_channel_points(channel: &str, access_token: &str) -> Result<u32> {
    #[derive(Serialize, Default, Debug)]
    struct Variables<'a> {
        #[serde(rename = "channelLogin")]
        channel_login: &'a str,
    }

    impl<'a> Default for GqlRequest<Variables<'a>> {
        fn default() -> Self {
            Self {
                operation_name: "ChannelPointsContext".to_string(),
                extensions: json!({
                    "persistedQuery": {
                        "version": 1,
                        "sha256Hash": "1530a003a7d374b0380b79db0be0534f30ff46e61cffa2bc0e2468a909fbc024",
                    }
                }),
                variables: Default::default(),
            }
        }
    }
    let mut points = GqlRequest::<Variables>::default();
    points.variables.channel_login = channel;

    let res = gql_req(access_token).json(&points).send().await?;

    if !res.status().is_success() {
        return Err(eyre!("Failed to get channel points"));
    }

    let json = res.json::<serde_json::Value>().await?;
    if !json.is_object() {
        return Err(eyre!("Returned data is not an object"));
    }

    let data = json
        .as_object()
        .unwrap()
        .get("data")
        .ok_or(eyre!("Failed to get data"))?;
    let community = data
        .as_object()
        .ok_or(eyre!("Failed to get data as object"))?
        .get("community")
        .ok_or(eyre!("Streamer does not exist"))?;
    let _self = community
        .as_object()
        .unwrap()
        .get("channel")
        .unwrap()
        .get("self")
        .unwrap();
    let balance = _self
        .as_object()
        .unwrap()
        .get("communityPoints")
        .unwrap()
        .get("balance")
        .unwrap()
        .as_u64()
        .unwrap();

    Ok(balance as u32)
}

pub async fn get_user_id(access_token: &str) -> Result<String> {
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        pub data: Data,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Data {
        pub current_user: CurrentUser,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct CurrentUser {
        pub id: String,
    }

    let res = gql_req(access_token)
        .json(&json!({
            "operationName": "CoreActionsCurrentUser",
            "variables": {},
            "extensions": {
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "6b5b63a013cf66a995d61f71a508ab5c8e4473350c5d4136f846ba65e8101e95"
                }
            }
        }))
        .send()
        .await?;

    Ok(res.json::<Root>().await?.data.current_user.id)
}
