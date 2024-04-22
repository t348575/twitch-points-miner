use color_eyre::{eyre::eyre, Result};
use rand::distributions::{Alphanumeric, DistString};
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use twitch_api::{pubsub, types::UserId};

use super::{CLIENT_ID, DEVICE_ID, USER_AGENT};
use crate::{
    twitch::traverse_json,
    types::{Game, StreamerInfo},
};

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
    channels: &[&str],
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
    pub struct User {
        pub id: UserId,
        pub stream: Option<Stream>,
    }

    #[derive(Deserialize, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct Stream {
        pub id: UserId,
        pub game: Option<Game>,
    }

    impl User {
        fn into(self, channel_name: String) -> StreamerInfo {
            StreamerInfo {
                live: self.stream.is_some(),
                broadcast_id: self.stream.clone().map(|x| x.id),
                channel_name,
                game: self.stream.map(|x| x.game).map_or(None, |x| x),
            }
        }
    }

    let users = channels
        .iter()
        .map(|user| {
            let mut req = GqlRequest::<Variables>::default();
            req.variables.channel_login = user;
            req
        })
        .collect::<Vec<_>>();

    let res = gql_req(access_token).json(&users).send().await?;
    let items = res.json::<serde_json::Value>().await?;
    if !items.is_array() {
        return Err(eyre!("Failed to get streamer metadata"));
    }

    let items = items.as_array().unwrap().clone();
    let items = items
        .into_iter()
        .zip(channels)
        .map(|(mut x, channel_name)| {
            let x = traverse_json(&mut x, ".data.user").unwrap();
            if x.is_null() {
                None
            } else {
                let user = serde_json::from_value::<User>(x.clone()).unwrap();
                Some((user.id.clone(), user.into(channel_name.to_string())))
            }
        })
        .collect();
    Ok(items)
}

pub async fn make_prediction(
    points: u32,
    event_id: &str,
    outcome_id: &str,
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
        outcome_id: &'a str,
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
    pred.variables = Variables {
        input: Input {
            event_id,
            outcome_id,
            points,
            transaction_id: Alphanumeric.sample_string(&mut rand::thread_rng(), 16),
        },
    };

    let res = gql_req(access_token).json(&pred).send().await?;

    if !res.status().is_success() {
        return Err(eyre!("Failed to place prediction"));
    }

    let mut res = res.json::<serde_json::Value>().await?;
    let res = traverse_json(&mut res, ".data.makePrediction.error").unwrap();
    if !res.is_null() {
        tracing::error!("Failed to make prediction: {:#?}", res);
        return Err(eyre!("Failed to make prediction: {:#?}", res));
    }
    Ok(())
}

/// (Points, Available points claim ID)
pub async fn get_channel_points(
    channel_names: &[&str],
    access_token: &str,
) -> Result<Vec<(u32, Option<String>)>> {
    #[derive(Clone, Serialize, Default, Debug)]
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
    let reqs = vec![GqlRequest::<Variables>::default(); channel_names.len()]
        .into_iter()
        .zip(channel_names)
        .map(|(mut req, name)| {
            req.variables.channel_login = name;
            req
        })
        .collect::<Vec<_>>();

    let res = gql_req(access_token).json(&reqs).send().await?;
    if !res.status().is_success() {
        return Err(eyre!("Failed to get channel points"));
    }

    let json = res.json::<serde_json::Value>().await?;
    if !json.is_array() {
        return Err(eyre!(
            "Failed to get channel points, expected array as response"
        ));
    }

    let arr = json.as_array().unwrap().clone();
    let items = arr
        .into_iter()
        .map(|mut result| {
            let balance = traverse_json(
                &mut result,
                ".data.community.channel.self.communityPoints.balance",
            )
            .unwrap()
            .as_u64()
            .unwrap() as u32;
            let available_claim = traverse_json(
                &mut result,
                ".data.community.channel.self.communityPoints.availableClaim.id",
            )
            .map(|x| x.as_str().unwrap().to_owned());

            (balance, available_claim)
        })
        .collect();

    Ok(items)
}

/// (UserID, UserName)
pub async fn get_user_id(access_token: &str) -> Result<(String, String)> {
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

    let mut data = res.json::<serde_json::Value>().await?;

    let user_id = traverse_json(&mut data, ".data.currentUser.id")
        .map(|x| x.as_str().unwrap().to_owned())
        .ok_or(eyre!("Failed to get user ID"))?;
    let user_name = traverse_json(&mut data, ".data.currentUser.login")
        .map(|x| x.as_str().unwrap().to_owned())
        .ok_or(eyre!("Failed to get user name"))?;

    Ok((user_id, user_name))
}

pub async fn claim_points(channel_id: &str, claim_id: &str, access_token: &str) -> Result<u32> {
    #[derive(Serialize, Default, Debug)]
    struct Variables<'a> {
        input: Input<'a>,
    }

    #[derive(Serialize, Default, Debug)]
    struct Input<'a> {
        #[serde(rename = "claimID")]
        claim_id: &'a str,
        #[serde(rename = "channelID")]
        channel_id: &'a str,
    }

    impl<'a> Default for GqlRequest<Variables<'a>> {
        fn default() -> Self {
            Self {
                operation_name: "ClaimCommunityPoints".to_string(),
                extensions: json!({
                    "persistedQuery": {
                        "version": 1,
                        "sha256Hash": "46aaeebe02c99afdf4fc97c7c0cba964124bf6b0af229395f1f6d1feed05b3d0",
                    }
                }),
                variables: Default::default(),
            }
        }
    }
    let mut claim = GqlRequest::<Variables>::default();
    claim.variables = Variables {
        input: Input {
            claim_id,
            channel_id,
        },
    };

    let res = gql_req(access_token).json(&claim).send().await?;

    if !res.status().is_success() {
        return Err(eyre!("Failed to claim points"));
    }

    let mut res = res.json::<serde_json::Value>().await?;
    let current_points = traverse_json(&mut res, ".data.claimCommunityPoints.currentPoints")
        .unwrap()
        .as_u64()
        .unwrap();

    Ok(current_points as u32)
}

#[derive(Serialize, Default, Debug)]
struct Variables<'a> {
    count: u8,
    #[serde(rename = "channelLogin")]
    channel_login: &'a str,
}

impl<'a> GqlRequest<Variables<'a>> {
    fn default(channel_name: &'a str) -> Self {
        Self {
            operation_name: "ChannelPointsPredictionContext".to_string(),
            extensions: json!({
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "beb846598256b75bd7c1fe54a80431335996153e358ca9c7837ce7bb83d7d383",
                }
            }),
            variables: Variables {
                count: 1,
                channel_login: channel_name,
            },
        }
    }
}

pub async fn channel_points_context(
    channel_names: &[&str],
    access_token: &str,
) -> Result<Vec<Vec<(pubsub::predictions::Event, bool)>>> {
    let request = channel_names
        .into_iter()
        .map(|x| GqlRequest::<Variables>::default(*x))
        .collect::<Vec<_>>();
    let res = gql_req(access_token).json(&request).send().await?;
    if !res.status().is_success() {
        return Err(eyre!("Failed to claim points"));
    }

    let res = res.json::<Vec<serde_json::Value>>().await?;
    let active_predictions = res
        .into_iter()
        .filter_map(|mut x| {
            let channel_id = traverse_json(&mut x, ".data.community.channel.id")
                .unwrap()
                .clone();
            let mut v = traverse_json(&mut x, ".data.community.channel.activePredictionEvents")
                .unwrap()
                .clone();
            super::camel_to_snake_case_json(&mut v);

            for item in v.as_array_mut().unwrap() {
                item.as_object_mut()
                    .unwrap()
                    .insert("channel_id".to_owned(), channel_id.clone());
                for outcome in traverse_json(item, ".outcomes")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                {
                    let x = outcome.as_object_mut().unwrap();
                    *x.get_mut("top_predictors").unwrap() = serde_json::Value::Array(Vec::new());
                }
            }

            match serde_json::from_value::<Vec<pubsub::predictions::Event>>(v) {
                Ok(s) => {
                    match traverse_json(&mut x, ".data.community.channel.self.recentPredictions") {
                        Some(recent) => {
                            let recent = recent
                                .as_array()
                                .unwrap()
                                .clone()
                                .into_iter()
                                .filter_map(|mut x| match traverse_json(&mut x, ".event.id") {
                                    Some(s) => Some(s.as_str().unwrap().to_owned()),
                                    None => None,
                                })
                                .collect::<Vec<_>>();
                            let items = s
                                .into_iter()
                                .map(|x| {
                                    let bet_placed = recent
                                        .iter()
                                        .find(|y| (**y).eq(x.id.as_str()))
                                        .and(Some(true))
                                        .or(Some(false))
                                        .unwrap();
                                    (x, bet_placed)
                                })
                                .collect();
                            Some(items)
                        }
                        None => Some(s.into_iter().map(|x| (x, false)).collect()),
                    }
                }
                Err(_) => None,
            }
        })
        .collect::<Vec<_>>();
    Ok(active_predictions)
}

pub async fn join_raid(raid_id: &str, access_token: &str) -> Result<()> {
    #[derive(Serialize, Default, Debug)]
    struct Variables<'a> {
        input: Input<'a>,
    }

    #[derive(Serialize, Default, Debug)]
    struct Input<'a> {
        #[serde(rename = "raidID")]
        raid_id: &'a str,
    }

    impl<'a> Default for GqlRequest<Variables<'a>> {
        fn default() -> Self {
            Self {
                operation_name: "JoinRaid".to_string(),
                extensions: json!({
                    "persistedQuery": {
                        "version": 1,
                        "sha256Hash": "c6a332a86d1087fbbb1a8623aa01bd1313d2386e7c63be60fdb2d1901f01a4ae",
                    }
                }),
                variables: Default::default(),
            }
        }
    }
    let mut claim = GqlRequest::<Variables>::default();
    claim.variables = Variables {
        input: Input { raid_id },
    };

    let res = gql_req(access_token).json(&claim).send().await?;

    if !res.status().is_success() {
        return Err(eyre!("Failed to claim points"));
    }
    Ok(())
}
