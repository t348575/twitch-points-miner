use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use flume::{Receiver, Sender};
use futures::{
    future::try_join_all,
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{net::TcpStream, time::interval};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use twitch_api::{
    helix::streams::get_streams,
    types::{UserId, UserIdRef},
    HelixClient,
};

use crate::auth::{Token, TwitchApiToken, CLIENT_ID, DEVICE_ID, USER_AGENT};

pub async fn get_channel_ids(
    users: &[&str],
    twitch_api_token: &TwitchApiToken,
) -> Result<Vec<Option<UserId>>> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();

    let items = try_join_all(
        users
            .iter()
            .map(|user| client.get_channel_from_login(*user, twitch_api_token)),
    )
    .await
    .wrap_err_with(|| "Failed to get channel ids")?
    .into_iter()
    .map(|x| x.map(|x| x.broadcaster_id))
    .collect();
    Ok(items)
}

pub async fn live_channels(
    channels: &[UserId],
    twitch_api_token: &TwitchApiToken,
) -> Result<Vec<(UserId, bool)>> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();

    let ids: Vec<&UserIdRef> = channels.iter().map(|ch| ch.into()).collect();
    let req = get_streams::GetStreamsRequest::builder()
        .user_id(ids)
        .build();
    let res: Vec<get_streams::Stream> = client
        .req_get(req, twitch_api_token)
        .await
        .context("Live channels")?
        .data;
    Ok(channels
        .iter()
        .map(|ch| {
            (
                ch.clone(),
                res.iter()
                    .find(|stream| stream.user_id == *ch)
                    .and(Some(true))
                    .or(Some(false))
                    .unwrap(),
            )
        })
        .collect())
}

pub async fn make_prediction(
    points: u32,
    event_id: String,
    outcome_id: String,
    token: &Token,
) -> Result<()> {
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct MakePrediction {
        #[serde(rename = "operationName")]
        operation_name: String,
        extensions: serde_json::Value,
        variables: Variables,
    }

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    struct Variables {
        input: Input,
    }

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    struct Input {
        #[serde(rename = "eventID")]
        event_id: String,
        #[serde(rename = "outcomeID")]
        outcome_id: String,
        points: u32,
        #[serde(rename = "transactionID")]
        transaction_id: String,
    }

    impl Default for MakePrediction {
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

    let mut pred = MakePrediction::default();
    pred.variables.input.event_id = event_id;
    pred.variables.input.outcome_id = outcome_id;
    pred.variables.input.points = points;
    pred.variables.input.transaction_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);

    let client = reqwest::Client::new();
    let res = client
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", USER_AGENT)
        .header("X-Device-Id", DEVICE_ID)
        .header("Authorization", format!("OAuth {}", token.access_token))
        .json(&pred)
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(eyre!("Failed to place prediction"));
    }
    Ok(())
}

pub async fn get_channel_points(channel: String, token: &Token) -> Result<u32> {
    #[derive(Serialize, Debug)]
    struct GetChannelPoints {
        #[serde(rename = "operationName")]
        operation_name: String,
        extensions: serde_json::Value,
        variables: Variables,
    }

    #[derive(Serialize, Default, Debug)]
    struct Variables {
        #[serde(rename = "channelLogin")]
        channel_login: String,
    }

    impl Default for GetChannelPoints {
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

    let mut points = GetChannelPoints::default();
    points.variables.channel_login = channel;

    let client = reqwest::Client::new();
    let res = client
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", CLIENT_ID)
        .header("User-Agent", USER_AGENT)
        .header("X-Device-Id", DEVICE_ID)
        .header("Authorization", format!("OAuth {}", token.access_token))
        .json(&points)
        .send()
        .await?;

    if !res.status().is_success() {
        println!("{:#?}", res);
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

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub async fn connect_twitch_ws(
    url: &str,
    access_token: &str,
) -> Result<(SplitSink<WsStream, Message>, SplitStream<WsStream>)> {
    let request = http::Request::builder()
        .uri(url)
        .header("Authorization", format!("OAuth {}", access_token))
        .header("Host", "localhost")
        .header("upgrade", "websocket")
        .header("connection", "upgrade")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header("sec-websocket-version", 13)
        .body(())
        .context(format!("Couldn't build request for {}", url))?;
    let (socket, _) = connect_async(request).await?;

    Ok(socket.split())
}

pub async fn writer(rx: Receiver<String>, mut write: SplitSink<WsStream, Message>) -> Result<()> {
    while let Ok(msg) = rx.recv_async().await {
        write.send(Message::Text(msg)).await?;
    }
    Ok(())
}

pub async fn ping_loop(tx: Sender<String>) -> Result<()> {
    let mut interval = interval(std::time::Duration::from_secs(3 * 60));
    let ping = json!({"type": "PING"}).to_string();
    loop {
        interval.tick().await;
        if tx.is_disconnected() {
            break;
        }

        tx.send_async(ping.clone()).await?;
    }
    Ok(())
}

#[cfg(feature = "api")]
pub async fn start_axum_server(address: String) -> axum::serve::Serve<axum::Router, axum::Router> {
    use axum::{routing::get, Router};

    let app = Router::new().route("/", get(|| async { "Hello, World!" }));
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    axum::serve(listener, app)
}
