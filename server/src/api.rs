//! `/api` for the web UI: what MCP does not carry. Everything else (tools, resources) goes through `/mcp`.
use crate::{AppState, ops};
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde_json::{Value, json};

pub fn router() -> Router<AppState> {
    Router::new().route("/info", get(info))
}

async fn info(State(s): State<AppState>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ops::info(s.root.as_ref()).map(Json).map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e}))))
}
