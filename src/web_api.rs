use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    serve::Serve,
    Extension, Json, Router,
};
use color_eyre::{
    eyre::{Context, Report},
    Result,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::info;
use twitch_api::{
    pubsub::predictions::{ByUser, Event},
    types::{Timestamp, UserId},
};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[cfg(feature = "analytics")]
use crate::analytics::model::{Outcome, Outcomes, Point, PointsInfo, Prediction, PredictionBet};
use crate::{
    config::{filters::Filter, strategy::*, StreamerConfig},
    pubsub::{prediction_logic, PubSub},
    twitch::{auth::Token, gql},
    types::*,
};

type ApiState = Arc<RwLock<PubSub>>;

pub async fn get_api_server(
    address: String,
    pubsub: ApiState,
    token: Arc<Token>,
) -> Serve<Router, Router> {
    #[derive(OpenApi)]
    #[openapi(
        paths(
            app_state,
            streamer,
            live_streamers,
            make_prediction
        ),
        components(
            schemas(
                MakePrediction, PubSub, Streamer, Points, StreamerInfo, StreamerConfigRefWrapper,
                ConfigTypeRef, StreamerConfig, Strategy, Filter, Detailed, Game, HighOdds, DefaultPrediction,
                Event, ByUser, Timestamp, UserId
            ),
        ),
        tags(
            (name = "crate", description = "Twitch points miner API")
        )
    )]
    struct ApiDoc;

    #[allow(unused_mut)]
    let mut openapi = ApiDoc::openapi();
    #[cfg(feature = "analytics")]
    {
        use utoipa::Path;
        let components = openapi.components.as_mut().unwrap();

        let schemas = [
            Point::schema(),
            PointsInfo::schema(),
            Prediction::schema(),
            PredictionBet::schema(),
            Outcomes::schema(),
            Outcome::schema(),
            Timeline::schema(),
        ];
        for s in schemas {
            components.schemas.insert(s.0.to_owned(), s.1);
        }

        let paths = [
            (
                __path_points_timeline::path(),
                __path_points_timeline::path_item(None),
            ),
            (
                __path_points_timeline::path(),
                __path_points_timeline::path_item(None),
            ),
        ];
        for p in paths {
            openapi.paths.paths.insert(p.0, p.1);
        }
    }

    let timeline_router = Router::new()
        .route("/points", get(points_timeline))
        .route("/predictions", get(predictions_timeline));

    let router = Router::new()
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", openapi))
        .route("/", get(app_state))
        .route("/:streamer", get(streamer))
        .route("/live", get(live_streamers))
        .nest("/timeline", timeline_router)
        .route("/make_prediction/:streamer", post(make_prediction))
        .layer(Extension(token))
        .with_state(pubsub);

    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    axum::serve(listener, router)
}

#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "Get the entire application state information", body = [PubSub])
    )
)]
async fn app_state(State(data): State<ApiState>) -> Json<PubSub> {
    let data = data.read().await;
    Json(data.clone())
}

#[utoipa::path(
    get,
    path = "/{streamer}",
    responses(
        (status = 200, description = "Get the entire application state information", body = [Streamer]),
        (status = 404, description = "Could not find streamer")
    ),
    params(
        ("streamer" = String, Path, description = "Name of streamer to get state for")
    )
)]
async fn streamer(State(data): State<ApiState>, Path(streamer): Path<String>) -> impl IntoResponse {
    let data = data.read().await;
    match data.get_by_name(streamer.as_str()) {
        Some(s) => Json(s.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "Streamer not found").into_response(),
    }
}

#[derive(Deserialize, ToSchema)]
struct MakePrediction {
    /// ID of the prediction
    event_id: String,
    /// If specified, a bet is forcefully placed, otherwise the prediction logic specified in the configuration is used
    points: Option<u32>,
    /// The outcome to place the bet on
    outcome_id: String,
}

#[derive(Serialize, ToSchema)]
struct Points(u32);

#[utoipa::path(
    post,
    path = "/make_prediction/{streamer}",
    responses(
        (status = 201, description = "Placed a bet", body = Points),
        (status = 202, description = "Did not place a bet, but no error occurred"),
        (status = 404, description = "Could not find streamer or event ID")
    ),
    params(
        ("streamer" = String, Path, description = "Name of streamer to get state for")
    )
)]
async fn make_prediction(
    State(data): State<ApiState>,
    Extension(token): Extension<Arc<Token>>,
    Path(streamer): Path<String>,
    Json(payload): Json<MakePrediction>,
) -> impl IntoResponse {
    let mut data = data.write().await;
    let simulate = data.simulate;

    let s = data.get_by_name(&streamer);

    if s.is_none() {
        return (StatusCode::NOT_FOUND, "Streamer not found").into_response();
    }

    #[cfg(feature = "analytics")]
    let analytics = data.analytics.clone();
    #[cfg(feature = "analytics")]
    let s_id = data.get_id_by_name(&streamer).unwrap().to_owned();
    let s = data.get_by_name_mut(&streamer).unwrap();

    let prediction = s.predictions.get(&payload.event_id);
    if prediction.is_none() {
        return (StatusCode::NOT_FOUND, "Prediction not found").into_response();
    }

    let (event, _) = prediction.unwrap();
    if let None = event.outcomes.iter().find(|o| o.id == payload.outcome_id) {
        return (StatusCode::NOT_FOUND, "Outcome not found").into_response();
    }

    let handle_err =
        |err: Report| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();

    if payload.points.is_some() {
        match place_bet(
            payload.event_id.clone(),
            payload.outcome_id,
            *payload.points.as_ref().unwrap(),
            &token,
            simulate,
            #[cfg(feature = "analytics")]
            (analytics, &s_id, &streamer),
        )
        .await
        {
            Ok(_) => {
                s.predictions.get_mut(&payload.event_id).unwrap().1 = true;
                (StatusCode::CREATED, Json(Points(payload.points.unwrap()))).into_response()
            }
            Err(err) => handle_err(err),
        }
    } else {
        match prediction_logic(s, &payload.event_id).await {
            Ok(Some((o, p))) => {
                match place_bet(
                    payload.event_id.clone(),
                    o,
                    p,
                    &token,
                    simulate,
                    #[cfg(feature = "analytics")]
                    (analytics, &s_id, &streamer),
                )
                .await
                {
                    Ok(_) => {
                        s.predictions.get_mut(&payload.event_id).unwrap().1 = true;
                        (StatusCode::CREATED, "Bet placed").into_response()
                    }
                    Err(err) => handle_err(err),
                }
            }
            Ok(None) => (StatusCode::ACCEPTED, "Did not place bet").into_response(),
            Err(err) => handle_err(err),
        }
    }
}

async fn place_bet(
    event_id: String,
    outcome_id: String,
    points: u32,
    token: &Token,
    simulate: bool,
    #[cfg(feature = "analytics")] analytics: (Arc<crate::analytics::AnalyticsWrapper>, &str, &str),
) -> Result<()> {
    info!("Prediction {} with {} points", event_id, points);
    gql::make_prediction(
        points,
        &event_id,
        &outcome_id,
        &token.access_token,
        simulate,
    )
    .await
    .context("Make prediction")?;

    #[cfg(feature = "analytics")]
    {
        let channel_id = analytics.1.parse::<i32>()?;
        let points = gql::get_channel_points(&[analytics.2], &token.access_token).await?;
        analytics
            .0
            .execute(|analytics| {
                analytics.insert_points(
                    channel_id,
                    points[0].0 as i32,
                    PointsInfo::Prediction(event_id),
                )
            })
            .await?;
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/live",
    responses(
        (status = 200, description = "List of live streamers and their state", body = Vec<Streamer>)
    )
)]
async fn live_streamers(State(data): State<ApiState>) -> Json<Vec<Streamer>> {
    let data = data.read().await;
    let items = data
        .streamers
        .values()
        .filter(|x| x.info.live)
        .cloned()
        .collect::<Vec<_>>();
    Json(items)
}

#[cfg(feature = "analytics")]
#[derive(Deserialize, ToSchema, utoipa::IntoParams)]
/// Timeline information, RFC3339 strings
struct Timeline {
    /// GE time
    from: String,
    /// LE time
    to: String,
}

#[cfg(feature = "analytics")]
#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("Could not parse RFC3339 timestamp: {0}")]
    ParseTimestamp(String),
    #[error("Analytics module error {0}")]
    AnalyticsError(crate::analytics::AnalyticsError),
}

#[cfg(feature = "analytics")]
impl From<chrono::ParseError> for ApiError {
    fn from(value: chrono::ParseError) -> Self {
        ApiError::ParseTimestamp(value.to_string())
    }
}

#[cfg(feature = "analytics")]
impl From<crate::analytics::AnalyticsError> for ApiError {
    fn from(value: crate::analytics::AnalyticsError) -> Self {
        ApiError::AnalyticsError(value)
    }
}

#[cfg(feature = "analytics")]
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status_code = match self {
            ApiError::ParseTimestamp(_) => StatusCode::BAD_REQUEST,
            ApiError::AnalyticsError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, self.to_string()).into_response()
    }
}

#[cfg(feature = "analytics")]
#[utoipa::path(
    get,
    path = "/timeline/points",
    responses(
        (status = 200, description = "Timeline of point information in the specified range", body = Vec<Point>),
    ),
    params(Timeline)
)]
async fn points_timeline(
    State(data): State<ApiState>,
    axum::extract::Query(timeline): axum::extract::Query<Timeline>,
) -> Result<Json<Vec<Point>>, ApiError> {
    use chrono::FixedOffset;

    let from = chrono::DateTime::<FixedOffset>::parse_from_rfc3339(&timeline.from)?.naive_local();
    let to = chrono::DateTime::<FixedOffset>::parse_from_rfc3339(&timeline.to)?.naive_local();

    let writer = data.write().await;
    let res = writer
        .analytics
        .execute(|analytics| analytics.points_timeline(from, to))
        .await?;
    Ok(Json(res))
}

#[cfg(not(feature = "analytics"))]
async fn points_timeline() -> StatusCode {
    StatusCode::NOT_FOUND
}

#[cfg(feature = "analytics")]
#[utoipa::path(
    get,
    path = "/timeline/predictions",
    responses(
        (status = 200, description = "Timeline of prediction information in the specified range", body = Vec<Prediction>),
    ),
    params(Timeline)
)]
async fn predictions_timeline(
    State(data): State<ApiState>,
    axum::extract::Query(timeline): axum::extract::Query<Timeline>,
) -> Result<Json<Vec<Prediction>>, ApiError> {
    use chrono::FixedOffset;

    let from = chrono::DateTime::<FixedOffset>::parse_from_rfc3339(&timeline.from)?.naive_local();
    let to = chrono::DateTime::<FixedOffset>::parse_from_rfc3339(&timeline.to)?.naive_local();

    let writer = data.write().await;
    let res = writer
        .analytics
        .execute(|analytics| analytics.predictions_timeline(from, to))
        .await?;
    Ok(Json(res))
}

#[cfg(not(feature = "analytics"))]
async fn predictions_timeline() -> StatusCode {
    StatusCode::NOT_FOUND
}
