use std::sync::Arc;

use axum::{
    extract::State, http::StatusCode, response::IntoResponse, routing::get, serve::Serve, Json,
    Router,
};
use color_eyre::eyre::{Context, Report};
use common::{
    config::{filters::Filter, strategy::*, PredictionConfig, StreamerConfig},
    twitch::auth::Token,
    types::*,
};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use twitch_api::{
    pubsub::predictions::Event,
    types::{Timestamp, UserId},
};
use utoipa::{
    openapi::{PathItem, RefOr, Schema},
    OpenApi,
};
use utoipa_swagger_ui::SwaggerUi;

use crate::pubsub::PubSub;

mod analytics;
mod config;
mod predictions;
mod streamer;

type ApiState = Arc<RwLock<PubSub>>;
type RouterBuild = (
    Router,
    Vec<(&'static str, RefOr<Schema>)>,
    Vec<(String, PathItem)>,
);

#[macro_export]
macro_rules! make_paths {
    ($($path:tt),*) => {
        {
            use utoipa::Path;
            vec![
                $(
                    (
                        $path::path(),
                        $path::path_item(None)
                    ),
                )*
            ]
        }
    };
}

#[macro_export]
macro_rules! sub_error {
    ($rule:expr) => {
        Err(ApiError::SubError(Box::new($rule)))
    };
}

pub async fn get_api_server(
    address: String,
    pubsub: ApiState,
    token: Arc<Token>,
) -> Serve<Router, Router> {
    #[derive(OpenApi)]
    #[openapi(
        paths(
            app_state
        ),
        components(
            schemas(
                PubSub, StreamerState, StreamerConfigRefWrapper, ConfigTypeRef, StreamerConfig, PredictionConfig, StreamerInfo, Event,
                Filter, Strategy, UserId, Game, Detailed, Timestamp, DefaultPrediction, DetailedOdds, Points, OddsComparisonType
            ),
        ),
        tags(
            (name = "crate", description = "Twitch points miner API")
        )
    )]
    struct ApiDoc;

    let mut openapi = ApiDoc::openapi();
    let components = openapi.components.as_mut().unwrap();

    let mut paths = Vec::new();
    let mut schemas = Vec::new();

    let streamer = streamer::build(pubsub.clone(), token.clone());
    schemas.extend(streamer.1);
    paths.extend(streamer.2);

    let predictions = predictions::build(pubsub.clone(), token.clone());
    schemas.extend(predictions.1);
    paths.extend(predictions.2);

    let config = config::build(pubsub.clone());
    schemas.extend(config.1);
    paths.extend(config.2);

    let analytics = {
        let analytics = analytics::build(pubsub.clone(), token.clone());
        schemas.extend(analytics.1);
        paths.extend(analytics.2);
        analytics.0
    };

    for p in paths {
        openapi.paths.paths.insert(p.0, p.1);
    }
    for s in schemas {
        components.schemas.insert(s.0.to_owned(), s.1);
    }

    #[allow(unused_mut)]
    let mut api = Router::new()
        .nest("/streamers", streamer.0)
        .nest("/predictions", predictions.0)
        .nest("/config", config.0)
        .nest("/analytics", analytics)
        .route("/", get(app_state).with_state(pubsub.clone()));

    let router = Router::new()
        .merge(SwaggerUi::new("/docs").url("/docs/openapi.json", openapi))
        .nest_service("/", ServeDir::new("dist"))
        .nest("/api", api)
        .layer(CorsLayer::very_permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    axum::serve(listener, router)
}

#[utoipa::path(
    get,
    path = "/api",
    responses(
        (status = 200, description = "Get the entire application state information", body = PubSub)
    )
)]
async fn app_state(State(data): State<ApiState>) -> Json<PubSub> {
    let data = data.read().await;
    Json(data.clone())
}

#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("Streamer does not exist")]
    StreamerDoesNotExist,
    #[error("Could not parse RFC3339 timestamp: {0}")]
    ParseTimestamp(String),
    #[error("Analytics module error {0}")]
    AnalyticsError(crate::analytics::AnalyticsError),
    #[error("Error sending request to the twitch API {0}")]
    TwitchAPIError(String),
    #[error("SubError")]
    SubError(Box<dyn WebApiError>),
    #[error("Internal server error {0}")]
    InternalError(String),
}

trait WebApiError: std::fmt::Debug + Send {
    fn make_response(&self) -> axum::response::Response;
}

impl From<chrono::ParseError> for ApiError {
    fn from(value: chrono::ParseError) -> Self {
        ApiError::ParseTimestamp(value.to_string())
    }
}

impl From<crate::analytics::AnalyticsError> for ApiError {
    fn from(value: crate::analytics::AnalyticsError) -> Self {
        ApiError::AnalyticsError(value)
    }
}

impl ApiError {
    fn twitch_api_error(err: Report) -> ApiError {
        ApiError::TwitchAPIError(err.to_string())
    }

    fn internal_error(err: Report) -> ApiError {
        ApiError::InternalError(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status_code = match self {
            ApiError::ParseTimestamp(_) => StatusCode::BAD_REQUEST,
            ApiError::StreamerDoesNotExist => StatusCode::BAD_REQUEST,
            ApiError::TwitchAPIError(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::AnalyticsError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::SubError(s) => return s.make_response(),
        };

        (status_code, self.to_string()).into_response()
    }
}

impl PubSub {
    async fn save_config(&mut self, context: &str) -> Result<(), ApiError> {
        tokio::fs::write(
            &self.config_path,
            serde_yaml::to_string(&self.config)
                .context(format!("Serializing config {context}"))
                .map_err(ApiError::internal_error)?,
        )
        .await
        .context(format!("Writing config file {context}"))
        .map_err(ApiError::internal_error)?;
        Ok(())
    }
}
