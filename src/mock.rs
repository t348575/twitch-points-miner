use std::{collections::HashMap, sync::Arc};

use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use color_eyre::eyre::Result;
use http::StatusCode;
use tokio::{signal, sync::Mutex};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use twitch::gql::{GqlRequest, Variables};
use twitch_api::types::UserId;

mod config;
mod twitch;
mod types;

#[derive(Default)]
struct AppState {
    streamer_metadata: HashMap<UserId, (String, twitch::gql::User)>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::new(&format!("twitch_mock=DEBUG"))
                .add_directive(format!("tower_http::trace=DEBUG").parse()?),
        )
        .init();

    let state = Arc::new(Mutex::new(AppState::default()));
    let router = Router::new()
        .route("/gql", post(gql_handler))
        .route("/streamer_metadata", post(set_streamer_metadata))
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

    fn get_by_name(&self, name: &str) -> Option<&(String, twitch::gql::User)> {
        self.streamer_metadata.values().find(|u| u.0.eq(name))
    }
}

async fn set_streamer_metadata(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(body): Json<HashMap<UserId, (String, twitch::gql::User)>>,
) -> impl IntoResponse {
    let mut state = state.lock().await;
    state.streamer_metadata = body;
    StatusCode::ACCEPTED
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
