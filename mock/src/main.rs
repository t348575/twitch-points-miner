use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::{IntoResponse, Response as AxumResponse},
    routing::{get, post},
    Form, Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE, Engine};
use common::twitch::{
    api::SetViewership,
    gql::{self, GqlRequest, Variables},
    traverse_json,
};
use eyre::Result;
use http::StatusCode;
use serde::Deserialize;
use tokio::{signal, sync::Mutex};
use tower_http::trace::TraceLayer;
use tracing::{debug, trace, warn};
use tracing_subscriber::EnvFilter;
use twitch_api::{
    pubsub::{
        video_playback::VideoPlaybackReply, Request, Response, TopicData, Topics, TwitchResponse,
    },
    types::UserId,
};

#[derive(Default)]
struct AppState {
    streamer_metadata: HashMap<UserId, (String, gql::User)>,
    ws_test_mode: WsTest,
    test_stats: HashMap<String, serde_json::Value>,
    watching: Vec<UserId>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
enum WsTest {
    #[default]
    Listen,
    Reconnect,
    RetryCommand,
    ScaleConnections,
}

#[tokio::main]
async fn main() -> Result<()> {
    let log_level = std::env::var("LOG").unwrap_or("error".to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::new(format!("mock={log_level}"))
                .add_directive(format!("tower_http::trace={log_level}").parse()?),
        )
        .init();

    let state = Arc::new(Mutex::new(AppState::default()));

    let pubsub_router = Router::new()
        .route("/", get(ws_handler))
        .route("/test_mode", post(pubsub_test_mode))
        .route("/test_stats", get(pubsub_test_stats));

    let router = Router::new()
        .route("/gql", post(gql_handler))
        .route("/streamer_metadata", post(set_streamer_metadata))
        .route(
            "/base/:streamer",
            get(|| async { "config/settings.12345.js" }),
        )
        .route(
            "/base/config/settings.12345.js",
            get(|| async { r#""spade_url":"/spade""# }),
        )
        .route("/watching", get(get_watching).delete(clear_watching))
        .route("/spade", post(spade_handler))
        .nest("/pubsub", pubsub_router)
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("ready");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn gql_handler(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(body): Json<vec_or_one::VecOrOne<GqlRequest>>,
) -> impl IntoResponse {
    let mut state = state.lock().await;
    match body {
        vec_or_one::VecOrOne::Vec(items) => {
            let mut results = Vec::new();
            for i in items {
                results.push(state.gql_req(i).await);
            }
            Json(serde_json::Value::Array(results))
        }
        vec_or_one::VecOrOne::One(item) => Json(state.gql_req(item).await),
    }
}

async fn pubsub_test_mode(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(body): Json<WsTest>,
) -> StatusCode {
    let mut state = state.lock().await;
    match &body {
        WsTest::Listen => {}
        WsTest::RetryCommand | WsTest::Reconnect => {
            state
                .test_stats
                .entry(format!("{:?}", body))
                .or_insert(serde_json::json!({ "count": 0 }));
        }
        WsTest::ScaleConnections => {
            state
                .test_stats
                .entry(format!("{:?}", body))
                .or_insert(serde_json::json!({ "sockets": 0, "topics": 0 }));
        }
    }
    state.ws_test_mode = body;
    StatusCode::ACCEPTED
}

async fn pubsub_test_stats(
    State(state): State<Arc<Mutex<AppState>>>,
) -> Json<HashMap<String, serde_json::Value>> {
    Json(state.lock().await.test_stats.clone())
}

impl AppState {
    async fn gql_req(&mut self, item: GqlRequest) -> serde_json::Value {
        match item.variables {
            Variables::StreamMetadata(s) => match self.get_by_name(&s.channel_login) {
                Some((_, u)) => serde_json::json!({
                    "data": {
                        "user": u.clone()
                    }
                }),
                None => serde_json::json!({
                    "data": {
                        "user": null
                    }
                }),
            },
            Variables::MakePrediction(_) => todo!(),
            Variables::ChannelPointsContext(_) => todo!(),
            Variables::ClaimCommunityPoints(_) => todo!(),
            Variables::ChannelPointsPredictionContext(_) => todo!(),
            Variables::JoinRaid(_) => todo!(),
        }
    }

    fn get_by_name(&self, name: &str) -> Option<&(String, gql::User)> {
        self.streamer_metadata.values().find(|u| u.0.eq(name))
    }
}

async fn set_streamer_metadata(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(body): Json<HashMap<UserId, (String, gql::User)>>,
) -> impl IntoResponse {
    let mut state = state.lock().await;
    state.streamer_metadata = body;
    StatusCode::ACCEPTED
}

#[derive(Deserialize)]
struct SpadeData {
    data: String,
}

async fn spade_handler(
    State(state): State<Arc<Mutex<AppState>>>,
    Form(data): Form<SpadeData>,
) -> StatusCode {
    let body = String::from_utf8(URL_SAFE.decode(&data.data).unwrap()).unwrap();
    let payload: Vec<SetViewership> = serde_json::from_str(&body).unwrap();
    let mut state = state.lock().await;
    if !state.watching.contains(&payload[0].properties.channel_id) {
        state
            .watching
            .push(payload[0].properties.channel_id.clone());
        return StatusCode::ACCEPTED;
    }
    StatusCode::CREATED
}

async fn clear_watching(State(state): State<Arc<Mutex<AppState>>>) -> StatusCode {
    state.lock().await.watching.clear();
    StatusCode::OK
}

async fn get_watching(State(state): State<Arc<Mutex<AppState>>>) -> Json<Vec<UserId>> {
    Json(state.lock().await.watching.clone())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<Mutex<AppState>>>,
) -> AxumResponse {
    {
        let mut state = state.lock().await;
        if let WsTest::ScaleConnections = state.ws_test_mode {
            let field = traverse_json(
                state.test_stats.get_mut("ScaleConnections").unwrap(),
                ".sockets",
            )
            .unwrap();
            *field = serde_json::Value::Number((field.as_i64().unwrap() + 1).into());
        }
    }

    ws.on_upgrade(|socket| wrapper_handle_socket(socket, state))
}

macro_rules! send_msg {
    ($socket:tt,$data:expr) => {
        $socket
            .send(Message::Text(serde_json::to_string(&Response::Message {
                data: $data,
            })?))
            .await?
    };
}

macro_rules! success_msg {
    ($socket:tt, $nonce:tt) => {
        $socket
            .send(Message::Text(serde_json::to_string(&Response::Response(
                TwitchResponse {
                    error: None,
                    nonce: $nonce,
                },
            ))?))
            .await?
    };
}

async fn wrapper_handle_socket(socket: WebSocket, state: Arc<Mutex<AppState>>) {
    if let Err(err) = handle_socket(socket, state.clone()).await {
        let mut state = state.lock().await;

        if let WsTest::ScaleConnections = state.ws_test_mode {
            let field = traverse_json(
                state.test_stats.get_mut("ScaleConnections").unwrap(),
                ".sockets",
            )
            .unwrap();
            *field = serde_json::Value::Number((field.as_i64().unwrap() - 1).into());
        }
        warn!("{err:?}");
    }
}

async fn handle_socket(mut socket: WebSocket, state: Arc<Mutex<AppState>>) -> Result<()> {
    let test_mode = { state.lock().await.ws_test_mode.clone() };
    debug!("connected, test_mode={test_mode:?}");

    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(msg) = msg {
            trace!("{msg}");
            match Request::parse(&msg) {
                Ok(msg) => match msg {
                    Request::Listen { data, nonce } => match test_mode {
                        WsTest::Listen => {
                            success_msg!(socket, nonce);

                            if let Topics::VideoPlaybackById(data) = &data.topics[0] {
                                send_msg!(
                                    socket,
                                    TopicData::VideoPlaybackById {
                                        topic: data.clone(),
                                        reply: Box::new(VideoPlaybackReply::StreamUp {
                                            server_time: 0.0,
                                            play_delay: 0,
                                        }),
                                    }
                                );
                            }
                        }
                        WsTest::Reconnect => {
                            let mut state = state.lock().await;
                            socket
                                .send(Message::Text(serde_json::to_string(&Response::Reconnect)?))
                                .await?;
                            let field = traverse_json(
                                state.test_stats.get_mut("Reconnect").unwrap(),
                                ".count",
                            )
                            .unwrap();
                            *field =
                                serde_json::Value::Number((field.as_i64().unwrap() + 1).into());
                        }
                        WsTest::RetryCommand => {
                            let mut state = state.lock().await;

                            let field = traverse_json(
                                state.test_stats.get_mut("RetryCommand").unwrap(),
                                ".count",
                            )
                            .unwrap();
                            *field =
                                serde_json::Value::Number((field.as_i64().unwrap() + 1).into());

                            if field == 1 {
                                socket
                                    .send(Message::Text(serde_json::to_string(
                                        &Response::Response(TwitchResponse {
                                            error: Some("retrying mode".to_owned()),
                                            nonce,
                                        }),
                                    )?))
                                    .await?;
                            } else {
                                success_msg!(socket, nonce);
                            }
                        }
                        WsTest::ScaleConnections => {
                            let mut state = state.lock().await;

                            let field = traverse_json(
                                state.test_stats.get_mut("ScaleConnections").unwrap(),
                                ".topics",
                            )
                            .unwrap();
                            *field =
                                serde_json::Value::Number((field.as_i64().unwrap() + 1).into());
                            trace!("{field:#?}");
                        }
                    },
                    Request::UnListen { data, nonce } => match test_mode {
                        WsTest::Listen => {
                            success_msg!(socket, nonce);

                            if let Topics::VideoPlaybackById(data) = &data.topics[0] {
                                send_msg!(
                                    socket,
                                    TopicData::VideoPlaybackById {
                                        topic: data.clone(),
                                        reply: Box::new(VideoPlaybackReply::StreamDown {
                                            server_time: 0.0
                                        }),
                                    }
                                );
                            }
                        }
                        WsTest::Reconnect => {}
                        WsTest::RetryCommand => {}
                        WsTest::ScaleConnections => {
                            let mut state = state.lock().await;

                            let field = traverse_json(
                                state.test_stats.get_mut("ScaleConnections").unwrap(),
                                ".topics",
                            )
                            .unwrap();
                            *field =
                                serde_json::Value::Number((field.as_i64().unwrap() - 1).into());
                        }
                    },
                    Request::Ping => {
                        socket
                            .send(Message::Text(serde_json::to_string(&Response::Pong)?))
                            .await?
                    }
                    _ => unreachable!(),
                },
                Err(err) => {
                    debug!("{err:#?}")
                }
            }
        }
    }
    debug!("disconnected");
    Ok(())
}

pub mod vec_or_one {
    use serde::{self, de, Deserialize, Serialize, Serializer};
    #[derive(Deserialize, Debug)]
    #[serde(untagged)]
    pub enum VecOrOne<T> {
        Vec(Vec<T>),
        One(T),
    }

    pub fn deserialize<'de, D: de::Deserializer<'de>, T: Deserialize<'de>>(
        de: D,
    ) -> Result<Vec<T>, D::Error> {
        use de::Deserialize as _;
        match VecOrOne::deserialize(de)? {
            VecOrOne::Vec(v) => Ok(v),
            VecOrOne::One(i) => Ok(vec![i]),
        }
    }

    pub fn serialize<S: Serializer, T: Serialize>(v: &Vec<T>, s: S) -> Result<S::Ok, S::Error> {
        match v.len() {
            1 => T::serialize(v.first().unwrap(), s),
            _ => Vec::<T>::serialize(v, s),
        }
    }
}
