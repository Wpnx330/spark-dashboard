use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::Embed;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use crate::history::HistoryDb;

/// Shared application state injected into handlers.
pub struct AppState {
    pub tx: broadcast::Sender<String>,
    pub history: HistoryDb,
}

#[derive(Embed)]
#[folder = "frontend/dist"]
struct FrontendAssets;

/// Build the HTTP router.
///
/// History API routes (`/api/history/*`) are only registered when `enable_history`
/// is true — they include destructive (`POST /prune`) and write (`settings`,
/// `toggle`) endpoints that must not be exposed unauthenticated on `0.0.0.0` by
/// default. Pass `--enable-history` (or `SPARK_DASHBOARD_ENABLE_HISTORY=1`) to
/// opt in; see `src/main.rs`.
pub fn create_router(state: Arc<AppState>, enable_history: bool) -> Router {
    let router = Router::<Arc<AppState>>::new()
        .route("/ws", get(crate::ws::ws_handler))
        // Liveness probe for container HEALTHCHECK / orchestrators. Intentionally
        // dependency-free: it reports that the HTTP server is up, not that any
        // engine/GPU is healthy (that's surfaced over /ws).
        .route("/healthz", get(healthz));

    // History API — opt-in only.
    let router = if enable_history {
        router
            .route("/api/history/summary", get(history_summary))
            .route("/api/history/settings", get(history_settings))
            .route("/api/history/settings", post(history_settings_update))
            .route("/api/history/toggle", post(history_toggle))
            .route("/api/history/prune", post(history_prune))
            .route("/api/history/size", get(history_db_size))
    } else {
        router
    };

    router
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive())
}

async fn healthz() -> &'static str {
    "ok"
}

// ---------------------------------------------------------------------------
// History API handlers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SummaryQuery {
    engine: String,
    since_ms: i64,
    until_ms: i64,
}

async fn history_summary(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SummaryQuery>,
) -> impl IntoResponse {
    match state
        .history
        .query_summary(&q.engine, q.since_ms, q.until_ms)
        .await
    {
        Ok(Some(summary)) => Json(serde_json::json!(summary)).into_response(),
        Ok(None) => Json(serde_json::json!({"error": "no data"})).into_response(),
        Err(e) => Json(serde_json::json!({"error": format!("{}", e)})).into_response(),
    }
}

async fn history_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let enabled = state.history.is_enabled();
    let utc_offset = state
        .history
        .get_setting("utc_offset")
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "-4".into());
    Json(serde_json::json!({
        "enabled": enabled,
        "utc_offset": utc_offset,
    }))
}

#[derive(Deserialize)]
struct SettingsUpdate {
    utc_offset: Option<String>,
}

async fn history_settings_update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SettingsUpdate>,
) -> impl IntoResponse {
    if let Some(v) = &body.utc_offset {
        let _ = state.history.set_setting("utc_offset", v).await;
    }
    Json(serde_json::json!({"ok": true}))
}

#[derive(Deserialize)]
struct ToggleBody {
    enabled: bool,
}

async fn history_toggle(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ToggleBody>,
) -> impl IntoResponse {
    match state.history.set_enabled(body.enabled).await {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(e) => Json(serde_json::json!({"error": format!("{}", e)})).into_response(),
    }
}

#[derive(Deserialize)]
struct PruneBody {
    older_than_ms: i64,
}

async fn history_prune(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PruneBody>,
) -> impl IntoResponse {
    match state.history.prune(body.older_than_ms).await {
        Ok((s, h, d)) => Json(serde_json::json!({"pruned_1s": s, "pruned_1h": h, "pruned_1d": d}))
            .into_response(),
        Err(e) => Json(serde_json::json!({"error": format!("{}", e)})).into_response(),
    }
}

async fn history_db_size(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.history.db_size().await {
        Ok(bytes) => Json(serde_json::json!({"bytes": bytes})).into_response(),
        Err(e) => Json(serde_json::json!({"error": format!("{}", e)})).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Static file serving (SPA)
// ---------------------------------------------------------------------------

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/');
    if path.is_empty() {
        path = "index.html";
    }

    // Try exact file match first
    if let Some(file) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, mime.as_ref().to_string())],
            file.data.into_owned(),
        )
            .into_response();
    }

    // SPA fallback: serve index.html for any unmatched route
    if let Some(index) = FrontendAssets::get("index.html") {
        return (
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/html".to_string())],
            index.data.into_owned(),
        )
            .into_response();
    }

    (axum::http::StatusCode::NOT_FOUND, "Not Found").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a router backed by an in-memory history DB for tests.
    fn test_router(enable_history: bool) -> Router {
        let history = HistoryDb::open(":memory:").unwrap();
        let (tx, _rx) = broadcast::channel::<String>(16);
        let state = Arc::new(AppState { tx, history });
        create_router(state, enable_history)
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let app = test_router(false);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let resp = reqwest::get(format!("http://{addr}/healthz"))
            .await
            .expect("request to /healthz");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        assert_eq!(resp.text().await.unwrap(), "ok");
    }

    /// With history disabled, `/api/history/summary` must not be routed — the
    /// SPA fallback serves index.html (or 404) instead of a JSON response.
    #[tokio::test]
    async fn history_routes_disabled_when_flag_off() {
        let app = test_router(false);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let resp = reqwest::get(format!(
            "http://{addr}/api/history/summary?engine=x&since_ms=0&until_ms=1"
        ))
        .await
        .expect("request to /api/history/summary");
        // Fallback serves index.html (text/html) — not a JSON API response.
        assert_ne!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .map(|v| v.to_str().unwrap_or("")),
            Some("application/json")
        );
    }

    /// With history enabled, `/api/history/summary` returns JSON (the "no data"
    /// error payload for an empty in-memory DB).
    #[tokio::test]
    async fn history_routes_enabled_when_flag_on() {
        let app = test_router(true);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let resp = reqwest::get(format!(
            "http://{addr}/api/history/summary?engine=x&since_ms=0&until_ms=1"
        ))
        .await
        .expect("request to /api/history/summary");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = resp.json().await.expect("json body");
        // Empty in-memory DB has no data for a nonexistent engine.
        assert_eq!(body.get("error").and_then(|v| v.as_str()), Some("no data"));
    }
}
