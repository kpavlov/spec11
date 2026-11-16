//! The UI. Release: the embedded single-file build. Debug: a proxy to the Vite dev server, so the browser still sees one port.
use axum::{
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

/// `web/dist` (built by build.rs). In debug builds rust-embed reads the folder from disk, so a missing build is fine there.
#[derive(Embed)]
#[folder = "../web/dist"]
#[allow_missing = true]
pub struct Dist;

pub fn index_html() -> Option<String> {
    Dist::get("index.html").map(|f| String::from_utf8_lossy(&f.data).into_owned())
}

#[cfg(not(debug_assertions))]
pub async fn handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(f) = Dist::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let cache = if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
        return ([(header::CONTENT_TYPE, mime.as_ref().to_string()), (header::CACHE_CONTROL, cache.to_string())], f.data).into_response();
    }
    if path.starts_with("assets/") {
        return StatusCode::NOT_FOUND.into_response(); // a missing asset, not a page
    }
    match Dist::get("index.html") {
        // unknown path: the app's own client-side view
        Some(f) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")], f.data).into_response(),
        None => (StatusCode::INTERNAL_SERVER_ERROR, "web/dist is not built: run `npm run build` in web/").into_response(),
    }
}

#[cfg(debug_assertions)]
pub async fn handler(axum::extract::State(s): axum::extract::State<crate::AppState>, uri: Uri) -> Response {
    let pq = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    match s.http.get(format!("http://localhost:5173{pq}")).send().await {
        Ok(r) => {
            let (status, headers) = (r.status(), r.headers().clone());
            let body = r.bytes().await.unwrap_or_default();
            let mut out = (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY), body).into_response();
            for (k, v) in headers.iter() {
                if !matches!(k.as_str(), "transfer-encoding" | "connection" | "content-length") {
                    if let (Ok(k), Ok(v)) = (header::HeaderName::from_bytes(k.as_str().as_bytes()), header::HeaderValue::from_bytes(v.as_bytes())) {
                        out.headers_mut().insert(k, v);
                    }
                }
            }
            out
        }
        Err(_) => (StatusCode::BAD_GATEWAY, "Vite is not running: `make dev` (cd web && npm run dev)").into_response(),
    }
}
