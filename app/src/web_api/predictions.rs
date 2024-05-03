use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use common::twitch::gql;
use eyre::{eyre, Context, ContextCompat};
use flume::Sender;
use http::StatusCode;
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::RwLockWriteGuard;
use tracing::info;
use utoipa::ToSchema;

use crate::{
    analytics::{self, model::*, Analytics, AnalyticsError, AnalyticsWrapper, TimelineResult},
    pubsub::PubSub,
};
use crate::{make_paths, pubsub::prediction_logic, sub_error};

use super::{ApiError, ApiState, RouterBuild, WebApiError};

pub fn build(
    state: ApiState,
    analytics: Arc<AnalyticsWrapper>,
    tx: Sender<analytics::Request>,
) -> RouterBuild {
    let routes = Router::new()
        .route("/live", get(get_live_prediction))
        .route("/bet/:streamer", post(make_prediction))
        .with_state((state, analytics, tx));

    #[allow(unused_mut)]
    let mut schemas = vec![MakePrediction::schema()];

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
    paths.extend(make_paths!(__path_get_live_prediction));

    (routes, schemas, paths)
}

#[derive(Debug, Error)]
pub enum PredictionError {
    #[error("Prediction does not exist")]
    PredictionNotFound,
    #[error("Outcome does not exist")]
    OutcomeNotFound,
}

impl WebApiError for PredictionError {
    fn make_response(&self) -> axum::response::Response {
        use PredictionError::*;
        let status_code = match self {
            OutcomeNotFound | PredictionNotFound => StatusCode::BAD_REQUEST,
        };

        (status_code, self.to_string()).into_response()
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
    State((data, _analytics, tx)): State<(
        ApiState,
        Arc<AnalyticsWrapper>,
        Sender<analytics::Request>,
    )>,
    Path(streamer): Path<String>,
    Json(payload): Json<MakePrediction>,
) -> Result<StatusCode, ApiError> {
    let mut state = data.write().await;
    let simulate = state.simulate;

    let gql = state.gql.clone();
    let s = state.get_by_name(&streamer);
    if s.is_none() {
        return Err(ApiError::StreamerDoesNotExist);
    }

    let s_id = state.get_id_by_name(&streamer).unwrap().to_owned();
    let s = state.get_by_name_mut(&streamer).unwrap().clone();

    let prediction = s.predictions.get(&payload.event_id);
    if prediction.is_none() {
        return sub_error!(PredictionError::PredictionNotFound);
    }

    let (event, _) = prediction.unwrap().clone();
    if !event.outcomes.iter().any(|o| o.id == payload.outcome_id) {
        return sub_error!(PredictionError::OutcomeNotFound);
    }
    drop(state);

    let update_placed_state = |mut state: RwLockWriteGuard<PubSub>| {
        state
            .get_by_name_mut(&streamer)
            .context("Streamer not found")
            .unwrap()
            .predictions
            .get_mut(&payload.event_id)
            .unwrap()
            .1 = true;
    };

    if payload.points.is_some() && *payload.points.as_ref().unwrap() > 0 {
        place_bet(
            payload.event_id.clone(),
            payload.outcome_id,
            *payload.points.as_ref().unwrap(),
            simulate,
            &streamer,
            &gql,
            &s_id,
            tx,
        )
        .await?;
        update_placed_state(data.write().await);
        Ok(StatusCode::CREATED)
    } else {
        match prediction_logic(&s, &payload.event_id) {
            Ok(Some((o, p))) => {
                place_bet(
                    payload.event_id.clone(),
                    o,
                    p,
                    simulate,
                    &streamer,
                    &gql,
                    &s_id,
                    tx,
                )
                .await?;
                update_placed_state(data.write().await);
                Ok(StatusCode::CREATED)
            }
            Ok(None) => Ok(StatusCode::ACCEPTED),
            Err(err) => Err(ApiError::internal_error(err)),
        }
    }
}

async fn place_bet(
    event_id: String,
    outcome_id: String,
    points: u32,
    simulate: bool,
    streamer_name: &str,
    gql: &gql::Client,
    streamer_id: &str,
    tx: Sender<analytics::Request>,
) -> Result<(), ApiError> {
    info!(
        "{}: predicting {}, with points {}",
        streamer_name, event_id, points
    );

    gql.make_prediction(points, &event_id, &outcome_id, simulate)
        .await
        .map_err(ApiError::twitch_api_error)?;

    let channel_id = streamer_id
        .parse::<i32>()
        .context("Could not parse streamer ID")?;
    let channel_points = gql
        .get_channel_points(&[streamer_name])
        .await
        .map_err(ApiError::twitch_api_error)?;

    tx.send_async(Box::new(
        move |analytics: &mut Analytics| -> Result<(), AnalyticsError> {
            let entry_id = analytics.last_prediction_id(channel_id, &event_id)?;
            analytics.insert_points(
                channel_id,
                channel_points[0].0 as i32,
                PointsInfo::Prediction(event_id.clone(), entry_id),
            )?;
            analytics.place_bet(&event_id, channel_id, &outcome_id, points)
        },
    ))
    .await
    .map_err(|_| eyre!("Could not send analytics request"))?;
    Ok(())
}

#[derive(Deserialize, ToSchema, utoipa::IntoParams)]
struct GetPredictionQuery {
    prediction_id: String,
    channel_id: i32,
}

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
    State(state): State<(ApiState, Arc<AnalyticsWrapper>, Sender<analytics::Request>)>,
) -> Result<Json<Option<Prediction>>, ApiError> {
    let res = state
        .1
        .execute(|analytics| analytics.get_live_prediction(query.channel_id, &query.prediction_id))
        .await?;
    Ok(Json(res))
}
