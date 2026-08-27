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

/// Blank tape kept free on each edge, as a percentage of the tape width.
pub const DEFAULT_OFFSET_PERCENT: u8 = 5;
/// Largest offset that still leaves some tape to print on.
pub const MAX_OFFSET_PERCENT: u8 = 49;

/// One label to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintJob {
    pub lines: Vec<String>,
    pub offset_percent: u8,
}

/// Pixels available for text once `offset_percent` of the tape width is
/// left blank on each edge.
#[must_use]
pub fn print_height(tape_px: usize, offset_percent: u8) -> usize {
    let edge = (tape_px * usize::from(offset_percent) + 50) / 100;
    tape_px.saturating_sub(2 * edge)
}

/// Prints one label. The USB implementation is [`UsbPrinter`]; tests
/// substitute a recorder.
pub trait Printer: Send + Sync {
    /// Prints the job's lines top to bottom and returns a one-line summary.
    ///
    /// # Errors
    /// Returns a human-readable reason when the label could not be printed.
    fn print(&self, job: &PrintJob) -> Result<String, String>;
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
    fn print(&self, job: &PrintJob) -> Result<String, String> {
        let mut transport = ptouch::UsbTransport::open().map_err(|e| e.to_string())?;
        let borrowed: Vec<&str> = job.lines.iter().map(String::as_str).collect();
        let mut size = (0, 0);
        let status = ptouch::print(&mut transport, true, |status| {
            let height = print_height(status.tape_px, job.offset_percent);
            let bitmap =
                render::render_lines(&self.font, &borrowed, height).map_err(|e| e.to_string())?;
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
    /// Wide on purpose so that -1 or 256 reach our range check (400)
    /// instead of failing JSON extraction (422).
    offset_percent: Option<i64>,
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
    let offset_percent = request
        .offset_percent
        .map_or(Ok(DEFAULT_OFFSET_PERCENT), |value| {
            u8::try_from(value)
                .ok()
                .filter(|&percent| percent <= MAX_OFFSET_PERCENT)
                .ok_or(value)
        });
    let offset_percent = match offset_percent {
        Ok(percent) => percent,
        Err(value) => {
            return error(
                StatusCode::BAD_REQUEST,
                format!("offset_percent must be 0..={MAX_OFFSET_PERCENT}, got {value}"),
            );
        }
    };
    let job = PrintJob {
        lines,
        offset_percent,
    };
    let printer = state.printer;
    match tokio::task::spawn_blocking(move || printer.print(&job)).await {
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

    use super::{PrintJob, Printer, app, label_lines, print_height};

    /// Records every job; fails every job when `failure` is set.
    #[derive(Default)]
    struct FakePrinter {
        jobs: Mutex<Vec<PrintJob>>,
        failure: Option<String>,
    }

    impl Printer for FakePrinter {
        fn print(&self, job: &PrintJob) -> Result<String, String> {
            self.jobs.lock().unwrap().push(job.clone());
            match &self.failure {
                Some(message) => Err(message.clone()),
                None => Ok("printed".to_owned()),
            }
        }
    }

    fn print_request(text: &str) -> Request<Body> {
        json_request(&serde_json::json!({ "text": text }))
    }

    fn json_request(payload: &serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/api/print")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(payload).unwrap()))
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
        assert_eq!(
            *printer.jobs.lock().unwrap(),
            [PrintJob {
                lines: vec!["abc".to_owned(), "def".to_owned()],
                offset_percent: 5,
            }]
        );
    }

    #[tokio::test]
    async fn an_explicit_offset_reaches_the_printer_and_out_of_range_is_rejected() {
        let printer = Arc::new(FakePrinter::default());

        let response = app("client/dist", printer.clone())
            .oneshot(json_request(
                &serde_json::json!({ "text": "abc", "offset_percent": 20 }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(printer.jobs.lock().unwrap()[0].offset_percent, 20);

        for out_of_range in [50, -1, 256] {
            let response = app("client/dist", printer.clone())
                .oneshot(json_request(
                    &serde_json::json!({ "text": "abc", "offset_percent": out_of_range }),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{out_of_range}");
            assert!(
                body_string(response)
                    .await
                    .contains("offset_percent must be 0..=49")
            );
        }
        assert_eq!(printer.jobs.lock().unwrap().len(), 1);
    }

    #[test]
    fn print_height_leaves_the_offset_blank_on_both_edges() {
        // 12mm tape: 5% of 76px rounds to 4px per edge
        assert_eq!(print_height(76, 5), 68);
        assert_eq!(print_height(76, 0), 76);
        assert_eq!(print_height(128, 10), 102);
        assert_eq!(print_height(76, 49), 2);
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
