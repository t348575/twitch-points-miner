use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use color_eyre::eyre::Report;
use http::StatusCode;
use serde::Deserialize;
use thiserror::Error;
use tracing::info;
use utoipa::ToSchema;

#[cfg(feature = "analytics")]
use crate::analytics::{model::*, TimelineResult};
use crate::{make_paths, pubsub::prediction_logic, twitch::auth::Token, twitch::gql};

use super::{ApiError, ApiState, RouterBuild};

pub fn build(state: ApiState, token: Arc<Token>) -> RouterBuild {
    let routes = Router::new()
        .route("/live", get(get_live_prediction))
        .route("/bet/:streamer", post(make_prediction))
        .layer(Extension(token))
        .with_state(state);

    #[allow(unused_mut)]
    let mut schemas = vec![MakePrediction::schema()];

    #[cfg(feature = "analytics")]
    schemas.extend(vec![
        Prediction::schema(),
        TimelineResult::schema(),
        Point::schema(),
        Outcomes::schema(),
        PointsInfo::schema(),
        PredictionBetWrapper::schema(),
        PredictionBet::schema(),
    ]);

    #[allow(unused_mut)]
    let mut paths = make_paths!(__path_make_prediction);

    #[cfg(feature = "analytics")]
    paths.extend(make_paths!(__path_get_live_prediction));

    (routes, schemas, paths)
}

#[derive(Debug, Error)]
pub enum PredictionError {
    #[error("Streamer does not exist")]
    StreamerDoesNotExist,
    #[error("Prediction does not exist")]
    PredictionNotFound,
    #[error("Outcome does not exist")]
    OutcomeNotFound,
    #[error("Common error")]
    ApiError(Arc<ApiError>),
}

impl IntoResponse for PredictionError {
    fn into_response(self) -> axum::response::Response {
        use PredictionError::*;
        let status_code = match self {
            OutcomeNotFound | PredictionNotFound | StreamerDoesNotExist => StatusCode::BAD_REQUEST,
            ApiError(err) => return Arc::try_unwrap(err).unwrap().into_response(),
        };

        (status_code, self.to_string()).into_response()
    }
}

impl From<Report> for PredictionError {
    fn from(value: Report) -> Self {
        PredictionError::ApiError(Arc::new(ApiError::InternalError(value.to_string())))
    }
}

#[derive(Debug, Deserialize, ToSchema)]
struct MakePrediction {
    /// ID of the prediction
    event_id: String,
    /// If specified, a bet is forcefully placed, otherwise the prediction logic specified in the configuration is used
    points: Option<u32>,
    /// The outcome to place the bet on
    outcome_id: String,
}

#[utoipa::path(
    post,
    path = "/api/predictions/bet/{streamer}",
    responses(
        (status = 201, description = "Placed a bet", body = Points),
        (status = 202, description = "Did not place a bet, but no error occurred"),
        (status = 404, description = "Could not find streamer or event ID")
    ),
    params(
        ("streamer" = String, Path, description = "Name of streamer to get state for"),
    ),
    request_body = MakePrediction
)]
async fn make_prediction(
    State(data): State<ApiState>,
    Extension(token): Extension<Arc<Token>>,
    Path(streamer): Path<String>,
    Json(payload): Json<MakePrediction>,
) -> Result<StatusCode, PredictionError> {
    info!("{payload:#?}");
    let mut data = data.write().await;
    let simulate = data.simulate;

    let s = data.get_by_name(&streamer);

    if s.is_none() {
        return Err(PredictionError::StreamerDoesNotExist);
    }

    #[cfg(feature = "analytics")]
    let analytics = data.analytics.clone();
    #[cfg(feature = "analytics")]
    let s_id = data.get_id_by_name(&streamer).unwrap().to_owned();
    let s = data.get_by_name_mut(&streamer).unwrap();

    let prediction = s.predictions.get(&payload.event_id);
    if prediction.is_none() {
        return Err(PredictionError::PredictionNotFound);
    }

    let (event, _) = prediction.unwrap();
    if let None = event.outcomes.iter().find(|o| o.id == payload.outcome_id) {
        return Err(PredictionError::OutcomeNotFound);
    }

    if payload.points.is_some() {
        place_bet(
            payload.event_id.clone(),
            payload.outcome_id,
            *payload.points.as_ref().unwrap(),
            &token,
            simulate,
            #[cfg(feature = "analytics")]
            (analytics, &s_id, &streamer),
        )
        .await?;
        s.predictions.get_mut(&payload.event_id).unwrap().1 = true;
        Ok(StatusCode::CREATED)
    } else {
        match prediction_logic(s, &payload.event_id).await {
            Ok(Some((o, p))) => {
                place_bet(
                    payload.event_id.clone(),
                    o,
                    p,
                    &token,
                    simulate,
                    #[cfg(feature = "analytics")]
                    (analytics, &s_id, &streamer),
                )
                .await?;
                s.predictions.get_mut(&payload.event_id).unwrap().1 = true;
                Ok(StatusCode::CREATED)
            }
            Ok(None) => Ok(StatusCode::ACCEPTED),
            Err(err) => Err(err.into()),
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
) -> Result<(), PredictionError> {
    info!("Prediction {} with {} points", event_id, points);
    gql::make_prediction(
        points,
        &event_id,
        &outcome_id,
        &token.access_token,
        simulate,
    )
    .await?;

    #[cfg(feature = "analytics")]
    {
        let channel_id = analytics
            .1
            .parse::<i32>()
            .map_err(|err| Into::<Report>::into(err))?;
        let points = gql::get_channel_points(&[analytics.2], &token.access_token).await?;
        analytics
            .0
            .execute(|analytics| {
                let entry_id = analytics.last_prediction_id(channel_id, &event_id)?;
                analytics.insert_points(
                    channel_id,
                    points[0].0 as i32,
                    PointsInfo::Prediction(event_id, entry_id),
                )
            })
            .await
            .map_err(|err| PredictionError::ApiError(Arc::new(ApiError::AnalyticsError(err))))?;
    }
    Ok(())
}

#[cfg(feature = "analytics")]
#[derive(Deserialize, ToSchema, utoipa::IntoParams)]
struct GetPredictionQuery {
    prediction_id: String,
    channel_id: i32,
}

#[cfg(feature = "analytics")]
#[utoipa::path(
    get,
    path = "/api/predictions/live",
    responses(
        (status = 200, description = "Get live prediction", body = Option<Prediction>),
    ),
    params(GetPredictionQuery)
)]
async fn get_live_prediction(
    axum::extract::Query(query): axum::extract::Query<GetPredictionQuery>,
    State(data): State<ApiState>,
) -> Result<Json<Option<Prediction>>, PredictionError> {
    let writer = data.write().await;
    let res = writer
        .analytics
        .execute(|analytics| analytics.get_live_prediction(query.channel_id, &query.prediction_id))
        .await
        .map_err(|err| PredictionError::ApiError(Arc::new(ApiError::AnalyticsError(err))))?;
    Ok(Json(res))
}

#[cfg(not(feature = "analytics"))]
async fn get_live_prediction() -> StatusCode {
    StatusCode::NOT_FOUND
}
