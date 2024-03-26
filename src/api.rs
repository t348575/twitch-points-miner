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
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::info;

use crate::{
    auth::Token,
    common,
    pubsub::{self, prediction_logic},
};

type ApiState = Arc<RwLock<pubsub::PubSub>>;

pub async fn get_api_server(
    address: String,
    pubsub: ApiState,
    token: Arc<Token>,
) -> Serve<Router, Router> {
    let app = Router::new()
        .route("/", get(get_all_streamers))
        .route("/:streamer", get(get_streamer))
        .route("/make_prediction/:streamer", post(make_prediction))
        .layer(Extension(token))
        .with_state(pubsub);

    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    axum::serve(listener, app)
}

async fn get_all_streamers(State(data): State<ApiState>) -> Json<pubsub::PubSub> {
    let data = data.read().await;
    Json(data.clone())
}

async fn get_streamer(
    State(data): State<ApiState>,
    Path(streamer): Path<String>,
) -> impl IntoResponse {
    let data = data.read().await;
    match data.get_by_name(streamer.as_str()) {
        Some(s) => Json(s.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "Streamer not found").into_response(),
    }
}

#[derive(Deserialize)]
struct MakePrediction {
    event_id: String,
    points: Option<u32>,
    outcome_id: String,
}

async fn make_prediction(
    State(data): State<ApiState>,
    Extension(token): Extension<Arc<Token>>,
    Path(streamer): Path<String>,
    Json(payload): Json<MakePrediction>,
) -> impl IntoResponse {
    let mut data = data.write().await;
    let simulate = data.simulate;
    let s = data.get_by_name_mut(streamer.as_str());

    if s.is_none() {
        return (StatusCode::NOT_FOUND, "Streamer not found").into_response();
    }
    let s = s.unwrap();

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
            payload.points.unwrap(),
            &token,
            simulate,
        )
        .await
        {
            Ok(_) => {
                s.predictions.get_mut(&payload.event_id).unwrap().1 = true;
                (StatusCode::CREATED, "Bet placed").into_response()
            }
            Err(err) => handle_err(err),
        }
    } else {
        match prediction_logic(s, &payload.event_id).await {
            Ok(Some((o, p))) => {
                match place_bet(payload.event_id.clone(), o, p, &token, simulate).await {
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
) -> Result<()> {
    info!("Prediction {} with {} points", event_id, points);
    common::make_prediction(points, &event_id, outcome_id, token, simulate)
        .await
        .context("Make prediction")?;
    Ok(())
}
