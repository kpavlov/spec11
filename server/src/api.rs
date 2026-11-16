//! `/api` for the web UI: the same tools the MCP server exposes, as `POST /api/call/{name}`.
use crate::{AppState, ops};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde_json::{Value, json};

pub fn router() -> Router<AppState> {
    Router::new().route("/info", get(info)).route("/call/{name}", post(call))
}

async fn info(State(s): State<AppState>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ops::info(s.root.as_ref()).map(Json).map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e}))))
}

async fn call(State(s): State<AppState>, Path(name): Path<String>, body: Option<Json<Value>>) -> (StatusCode, Json<Value>) {
    let args = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let r = tokio::task::spawn_blocking(move || ops::call(s.root.as_deref(), &name, &args, &s.dashboard)).await;
    match r {
        Ok(Ok((text, data))) => (StatusCode::OK, Json(json!({"text": text, "data": data}))),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}
