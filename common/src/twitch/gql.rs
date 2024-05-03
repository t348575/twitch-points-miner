use eyre::{eyre, Result};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use serde_json::json;
use strum_macros::EnumDiscriminants;
use twitch_api::{pubsub, types::UserId};

use super::{CLIENT_ID, DEVICE_ID, USER_AGENT};
use crate::{
    twitch::traverse_json,
    types::{Game, StreamerInfo},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "GqlPlaceHolder")]
pub struct GqlRequest {
    #[serde(rename = "operationName")]
    pub operation_name: OperationName,
    extensions: serde_json::Value,
    pub variables: Variables,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GqlPlaceHolder {
    #[serde(rename = "operationName")]
    operation_name: OperationName,
    extensions: serde_json::Value,
    variables: Variables,
}

impl TryFrom<GqlPlaceHolder> for GqlRequest {
    type Error = String;
    fn try_from(data: GqlPlaceHolder) -> Result<Self, Self::Error> {
        use OperationName::*;
        let variables = match (data.operation_name, data.variables) {
            (StreamMetadata, content @ Variables::StreamMetadata(_)) => content,
            (ChannelPointsContext, Variables::StreamMetadata(content)) => {
                Variables::ChannelPointsContext(content)
            }
            (
                MakePrediction | ClaimCommunityPoints | ChannelPointsPredictionContext | JoinRaid,
                content,
            ) => content,
            (operation_name, _) => {
                return Err(format!(
                    "Operation name and variables do not match: {operation_name:#?}"
                ))
            }
        };

        Ok(GqlRequest {
            operation_name: data.operation_name,
            extensions: data.extensions,
            variables,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, EnumDiscriminants)]
#[strum_discriminants(name(OperationName))]
#[strum_discriminants(derive(Serialize, Deserialize))]
#[serde(untagged)]
pub enum Variables {
    StreamMetadata(ChanelLogin),
    MakePrediction(MakePrediction),
    ChannelPointsContext(ChanelLogin),
    ClaimCommunityPoints(ClaimCommunityPoints),
    ChannelPointsPredictionContext(ChannelPointsPredictionContext),
    JoinRaid(JoinRaid),
}

#[derive(Debug, Clone, Default)]
pub struct Client {
    access_token: String,
    url: String,
}

impl Client {
    pub fn new(access_token: String, url: String) -> Client {
        Client { access_token, url }
    }

    fn gql_req(&self) -> ureq::Request {
        ureq::post(&self.url)
            .set("Client-Id", CLIENT_ID)
            .set("User-Agent", USER_AGENT)
            .set("X-Device-Id", DEVICE_ID)
            .set("Authorization", &format!("OAuth {}", self.access_token))
    }

    pub async fn streamer_metadata(
        &self,
        channels: &[&str],
    ) -> Result<Vec<Option<(UserId, StreamerInfo)>>> {
        let users = channels
            .iter()
            .map(|user| GqlRequest::stream_metadata(user))
            .collect::<Vec<_>>();

        let items: serde_json::Value = self.gql_req().send_json(&users)?.into_json()?;
        if !items.is_array() {
            return Err(eyre!("Failed to get streamer metadata"));
        }

        let items = items.as_array().unwrap().clone();
        let items = items
            .into_iter()
            .zip(channels)
            .map(
                |(mut x, channel_name)| match traverse_json(&mut x, ".data.user") {
                    Some(x) => {
                        if x.is_null() {
                            None
                        } else {
                            let user = serde_json::from_value::<User>(x.clone()).unwrap();
                            Some((user.id.clone(), user.into(channel_name.to_string())))
                        }
                    }
                    None => None,
                },
            )
            .collect();
        Ok(items)
    }

    pub async fn make_prediction(
        &self,
        points: u32,
        event_id: &str,
        outcome_id: &str,
        simulate: bool,
    ) -> Result<()> {
        if simulate {
            return Ok(());
        }

        let pred = GqlRequest::make_prediction(event_id, outcome_id, points);
        let res = self.gql_req().send_json(&pred)?;

        if res.status() > 299 {
            return Err(eyre!("Failed to place prediction"));
        }

        let mut res = res.into_json()?;
        let res = traverse_json(&mut res, ".data.makePrediction.error").unwrap();
        if !res.is_null() {
            return Err(eyre!("Failed to make prediction: {:#?}", res));
        }
        Ok(())
    }

    /// (Points, Available points claim ID)
    pub async fn get_channel_points(
        &self,
        channel_names: &[&str],
    ) -> Result<Vec<(u32, Option<String>)>> {
        let reqs = channel_names
            .iter()
            .map(|name| GqlRequest::channel_points_context(name))
            .collect::<Vec<_>>();

        let res = self.gql_req().send_json(&reqs)?;
        if res.status() > 299 {
            return Err(eyre!("Failed to get channel points"));
        }

        let json: serde_json::Value = res.into_json()?;
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
    pub async fn get_user_id(&self) -> Result<(String, String)> {
        let mut data = self.gql_req()
            .send_json(&json!({
                "operationName": "CoreActionsCurrentUser",
                "variables": {},
                "extensions": {
                    "persistedQuery": {
                        "version": 1,
                        "sha256Hash": "6b5b63a013cf66a995d61f71a508ab5c8e4473350c5d4136f846ba65e8101e95"
                    }
                }
            }))?.into_json()?;

        let user_id = traverse_json(&mut data, ".data.currentUser.id")
            .map(|x| x.as_str().unwrap().to_owned())
            .ok_or(eyre!("Failed to get user ID"))?;
        let user_name = traverse_json(&mut data, ".data.currentUser.login")
            .map(|x| x.as_str().unwrap().to_owned())
            .ok_or(eyre!("Failed to get user name"))?;

        Ok((user_id, user_name))
    }

    pub async fn claim_points(&self, channel_id: &str, claim_id: &str) -> Result<u32> {
        let claim = GqlRequest::claim_community_points(claim_id, channel_id);
        let res = self.gql_req().send_json(&claim)?;

        if res.status() > 299 {
            return Err(eyre!("Failed to claim points"));
        }

        let mut res = res.into_json()?;
        let current_points = traverse_json(&mut res, ".data.claimCommunityPoints.currentPoints")
            .unwrap()
            .as_u64()
            .unwrap();

        Ok(current_points as u32)
    }

    pub async fn channel_points_context(
        &self,
        channel_names: &[&str],
    ) -> Result<Vec<Vec<(pubsub::predictions::Event, bool)>>> {
        let request = channel_names
            .iter()
            .map(|x| GqlRequest::channel_points_prediction_context(x))
            .collect::<Vec<_>>();
        let res = self.gql_req().send_json(&request)?;
        if res.status() > 299 {
            return Err(eyre!("Failed to claim points"));
        }

        let res: Vec<serde_json::Value> = res.into_json()?;
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
                        *x.get_mut("top_predictors").unwrap() =
                            serde_json::Value::Array(Vec::new());
                    }
                }

                match serde_json::from_value::<Vec<pubsub::predictions::Event>>(v) {
                    Ok(s) => {
                        match traverse_json(
                            &mut x,
                            ".data.community.channel.self.recentPredictions",
                        ) {
                            Some(recent) => {
                                let recent = recent
                                    .as_array()
                                    .unwrap()
                                    .clone()
                                    .into_iter()
                                    .filter_map(|mut x| {
                                        traverse_json(&mut x, ".event.id")
                                            .map(|s| s.as_str().unwrap().to_owned())
                                    })
                                    .collect::<Vec<_>>();
                                let items = s
                                    .into_iter()
                                    .map(|x| {
                                        let bet_placed = recent
                                            .iter()
                                            .find(|y| (**y).eq(x.id.as_str()))
                                            .and(Some(true))
                                            .unwrap_or(false);
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

    pub async fn join_raid(&self, raid_id: &str) -> Result<()> {
        let claim = GqlRequest::join_raid(raid_id);
        let res = self.gql_req().send_json(&claim)?;

        if res.status() > 299 {
            return Err(eyre!("Failed to join raid"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: UserId,
    pub stream: Option<Stream>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            game: self.stream.map(|x| x.game).and_then(|x| x),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChanelLogin {
    #[serde(rename = "channelLogin")]
    pub channel_login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakePrediction {
    #[serde(rename = "input")]
    input: MakePredictionInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakePredictionInput {
    #[serde(rename = "eventID")]
    event_id: String,
    #[serde(rename = "outcomeID")]
    outcome_id: String,
    points: u32,
    #[serde(rename = "transactionID")]
    transaction_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimCommunityPoints {
    input: ClaimCommunityPointsInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimCommunityPointsInput {
    #[serde(rename = "claimID")]
    claim_id: String,
    #[serde(rename = "channelID")]
    channel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPointsPredictionContext {
    count: u8,
    #[serde(rename = "channelLogin")]
    channel_login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRaid {
    input: JoinRaidInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRaidInput {
    #[serde(rename = "raidID")]
    raid_id: String,
}

impl GqlRequest {
    fn stream_metadata(channel_login: &str) -> Self {
        Self {
            operation_name: OperationName::StreamMetadata,
            extensions: json!({
                "persistedQuery": {
                    "sha256Hash": "676ee2f834ede42eb4514cdb432b3134fefc12590080c9a2c9bb44a2a4a63266",
                    "version": 1
                }
            }),
            variables: Variables::StreamMetadata(ChanelLogin {
                channel_login: channel_login.to_owned(),
            }),
        }
    }

    fn make_prediction(event_id: &str, outcome_id: &str, points: u32) -> Self {
        Self {
            operation_name: OperationName::MakePrediction,
            extensions: json!({
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "b44682ecc88358817009f20e69d75081b1e58825bb40aa53d5dbadcc17c881d8",
                }
            }),
            variables: Variables::MakePrediction(MakePrediction {
                input: MakePredictionInput {
                    event_id: event_id.to_owned(),
                    outcome_id: outcome_id.to_owned(),
                    points,
                    transaction_id: Alphanumeric.sample_string(&mut rand::thread_rng(), 32),
                },
            }),
        }
    }

    fn channel_points_context(channel_login: &str) -> Self {
        Self {
            operation_name: OperationName::ChannelPointsContext,
            extensions: json!({
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "1530a003a7d374b0380b79db0be0534f30ff46e61cffa2bc0e2468a909fbc024",
                }
            }),
            variables: Variables::ChannelPointsContext(ChanelLogin {
                channel_login: channel_login.to_owned(),
            }),
        }
    }

    fn claim_community_points(claim_id: &str, channel_id: &str) -> Self {
        Self {
            operation_name: OperationName::ClaimCommunityPoints,
            extensions: json!({
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "46aaeebe02c99afdf4fc97c7c0cba964124bf6b0af229395f1f6d1feed05b3d0",
                }
            }),
            variables: Variables::ClaimCommunityPoints(ClaimCommunityPoints {
                input: ClaimCommunityPointsInput {
                    claim_id: claim_id.to_owned(),
                    channel_id: channel_id.to_owned(),
                },
            }),
        }
    }

    fn channel_points_prediction_context(channel_login: &str) -> Self {
        Self {
            operation_name: OperationName::ChannelPointsPredictionContext,
            extensions: json!({
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "beb846598256b75bd7c1fe54a80431335996153e358ca9c7837ce7bb83d7d383",
                }
            }),
            variables: Variables::ChannelPointsPredictionContext(ChannelPointsPredictionContext {
                count: 1,
                channel_login: channel_login.to_owned(),
            }),
        }
    }

    fn join_raid(raid_id: &str) -> Self {
        Self {
            operation_name: OperationName::JoinRaid,
            extensions: json!({
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "c6a332a86d1087fbbb1a8623aa01bd1313d2386e7c63be60fdb2d1901f01a4ae",
                }
            }),
            variables: Variables::JoinRaid(JoinRaid {
                input: JoinRaidInput {
                    raid_id: raid_id.to_owned(),
                },
            }),
        }
    }
}
