//! label-server — prints Brother PT-P700 labels from a textarea.
//!
//! Copyright (C) 2026 label-server contributors.
//! Licensed under the GNU General Public License version 3 (GPL-3.0-only).

pub mod ptouch;
pub mod render;

use std::{path::Path, sync::Arc};

use ab_glyph::FontVec;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

/// Prints one label. The USB implementation is [`UsbPrinter`]; tests
/// substitute a recorder.
pub trait Printer: Send + Sync {
    /// Prints `lines` top to bottom and returns a one-line summary.
    ///
    /// # Errors
    /// Returns a human-readable reason when the label could not be printed.
    fn print(&self, lines: &[String]) -> Result<String, String>;
}

/// Renders with a font and prints over USB.
pub struct UsbPrinter {
    font: FontVec,
}

impl UsbPrinter {
    #[must_use]
    pub fn new(font: FontVec) -> Self {
        Self { font }
    }
}

impl Printer for UsbPrinter {
    fn print(&self, lines: &[String]) -> Result<String, String> {
        let mut transport = ptouch::UsbTransport::open().map_err(|e| e.to_string())?;
        let borrowed: Vec<&str> = lines.iter().map(String::as_str).collect();
        let mut size = (0, 0);
        let status = ptouch::print(&mut transport, true, |status| {
            let bitmap = render::render_lines(&self.font, &borrowed, status.tape_px)
                .map_err(|e| e.to_string())?;
            size = (bitmap.width, bitmap.height);
            Ok(bitmap)
        })
        .map_err(|e| e.to_string())?;
        Ok(format!(
            "printed {}x{}px on {}mm tape",
            size.0, size.1, status.media_width_mm
        ))
    }
}

#[derive(Clone)]
struct AppState {
    printer: Arc<dyn Printer>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Deserialize)]
struct PrintRequest {
    text: String,
}

#[derive(Serialize)]
struct PrintResponse {
    output: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// Splits textarea text into label lines, dropping leading and trailing
/// blank lines. Returns `None` when there is nothing printable.
#[must_use]
pub fn label_lines(text: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let first = lines.iter().position(|line| !line.trim().is_empty())?;
    let last = lines.iter().rposition(|line| !line.trim().is_empty())?;
    Some(
        lines[first..=last]
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
    )
}

pub fn app(static_dir: impl AsRef<Path>, printer: Arc<dyn Printer>) -> Router {
    let static_dir = static_dir.as_ref().to_path_buf();
    let api = Router::new()
        .route("/health", get(api_health))
        .route("/print", post(api_print))
        .fallback(api_not_found)
        .with_state(AppState { printer });

    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api", api)
        .fallback_service(
            ServeDir::new(&static_dir).fallback(ServeFile::new(static_dir.join("index.html"))),
        )
        .layer(TraceLayer::new_for_http())
}

async fn healthz() -> &'static str {
    "ok\n"
}

async fn api_health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn api_print(State(state): State<AppState>, Json(request): Json<PrintRequest>) -> Response {
    let Some(lines) = label_lines(&request.text) else {
        return error(StatusCode::BAD_REQUEST, "text is empty".to_owned());
    };
    let printer = state.printer;
    match tokio::task::spawn_blocking(move || printer.print(&lines)).await {
        Ok(Ok(output)) => Json(PrintResponse { output }).into_response(),
        Ok(Err(message)) => error(StatusCode::BAD_GATEWAY, message),
        Err(join) => error(StatusCode::BAD_GATEWAY, join.to_string()),
    }
}

fn error(status: StatusCode, message: String) -> Response {
    (status, Json(ErrorResponse { error: message })).into_response()
}

async fn api_not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "API route not found\n")
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use tower::ServiceExt;

    use super::{Printer, app, label_lines};

    /// Records every job; fails every job when `failure` is set.
    #[derive(Default)]
    struct FakePrinter {
        jobs: Mutex<Vec<Vec<String>>>,
        failure: Option<String>,
    }

    impl Printer for FakePrinter {
        fn print(&self, lines: &[String]) -> Result<String, String> {
            self.jobs.lock().unwrap().push(lines.to_vec());
            match &self.failure {
                Some(message) => Err(message.clone()),
                None => Ok("printed".to_owned()),
            }
        }
    }

    fn print_request(text: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/api/print")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({ "text": text })).unwrap(),
            ))
            .unwrap()
    }

    async fn body_string(response: axum::response::Response) -> String {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    #[test]
    fn label_lines_keeps_inner_blank_lines_and_trims_the_edges() {
        assert_eq!(
            label_lines("abc\n\n12mm テスト  \n").unwrap(),
            ["abc", "", "12mm テスト"]
        );
    }

    #[test]
    fn label_lines_rejects_blank_text() {
        assert_eq!(label_lines(""), None);
        assert_eq!(label_lines(" \n\t\n"), None);
    }

    #[tokio::test]
    async fn multi_line_text_is_handed_to_the_printer_as_lines() {
        let printer = Arc::new(FakePrinter::default());

        let response = app("client/dist", printer.clone())
            .oneshot(print_request("abc\ndef"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_string(response).await, r#"{"output":"printed"}"#);
        assert_eq!(*printer.jobs.lock().unwrap(), [["abc", "def"]]);
    }

    #[tokio::test]
    async fn blank_text_is_rejected_without_touching_the_printer() {
        let printer = Arc::new(FakePrinter::default());

        let response = app("client/dist", printer.clone())
            .oneshot(print_request(" \n"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(printer.jobs.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn printer_failure_is_reported_with_its_message() {
        let printer = Arc::new(FakePrinter {
            failure: Some("no PT-P700 found on USB".to_owned()),
            ..FakePrinter::default()
        });

        let response = app("client/dist", printer)
            .oneshot(print_request("abc"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            body_string(response).await,
            r#"{"error":"no PT-P700 found on USB"}"#
        );
    }

    #[tokio::test]
    async fn liveness_is_lightweight_plain_text() {
        let response = app("client/dist", Arc::new(FakePrinter::default()))
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_string(response).await, "ok\n");
    }

    #[tokio::test]
    async fn unknown_api_routes_do_not_fall_back_to_the_spa() {
        let response = app("client/dist", Arc::new(FakePrinter::default()))
            .oneshot(
                Request::builder()
                    .uri("/api/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_client_routes_return_the_spa_with_success() {
        let response = app("client", Arc::new(FakePrinter::default()))
            .oneshot(
                Request::builder()
                    .uri("/anything")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
    }
}
