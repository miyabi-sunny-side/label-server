//! label-server — prints Brother PT-P700 labels from a textarea.
//!
//! Copyright (C) 2026 label-server contributors.
//! Licensed under the GNU General Public License version 3 (GPL-3.0-only).

pub mod ptouch;
pub mod render;

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use ab_glyph::FontVec;
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::Engine;
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;

/// The built Svelte app, compiled into the binary. Build the client
/// (`npm --prefix client run build`) before building the server.
static CLIENT: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/client/dist");

/// Blank tape kept free on each edge, as a percentage of the tape width.
pub const DEFAULT_OFFSET_PERCENT: u8 = 5;
/// Largest offset that still leaves some tape to print on.
pub const MAX_OFFSET_PERCENT: u8 = 49;

/// Full size: the largest font that fits the tape after the offset.
pub const DEFAULT_FONT_SCALE_PERCENT: u8 = 100;
pub const MIN_FONT_SCALE_PERCENT: u8 = 10;
/// Tape assumed by previews when no printer has been asked.
pub const DEFAULT_TAPE_MM: u8 = 12;
/// PT-P700 print resolution.
pub const DPI: f64 = 180.0;

/// One label to print or preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintJob {
    pub lines: Vec<String>,
    pub offset_percent: u8,
    /// Font id from [`FontCatalog`]; `None` selects the catalog default.
    pub font: Option<String>,
    /// Shrinks the auto-fitted size; 100 keeps it.
    pub font_scale_percent: u8,
}

/// The font shipped inside the binary: `BIZ UDPGothic` Regular (Morisawa,
/// SIL Open Font License 1.1, see fonts/OFL-BIZUDPGothic.txt). It is the
/// default unless `LABEL_FONT` names another one.
pub const EMBEDDED_FONT_ID: &str = "BIZUDPGothic-Regular";
pub const EMBEDDED_FONT: &[u8] = include_bytes!("../fonts/BIZUDPGothic-Regular.ttf");

/// Where a catalog font comes from.
#[derive(Debug, Clone)]
pub enum FontSource {
    /// A TTF/OTF/TTC on disk (face 0 of a collection); skipped when unreadable.
    Path(PathBuf),
    /// Bytes compiled into the binary.
    Embedded {
        id: &'static str,
        bytes: &'static [u8],
    },
}

/// Fonts loaded at startup, addressed by id (the file stem for files).
/// The first loadable source is the default.
pub struct FontCatalog {
    fonts: BTreeMap<String, FontVec>,
    default: String,
}

impl FontCatalog {
    /// Loads every loadable source in order; the first one becomes the
    /// default and duplicate ids keep their first entry.
    ///
    /// # Errors
    /// Returns the tried sources when none of them could be loaded.
    pub fn load(sources: &[FontSource]) -> Result<Self, String> {
        let mut fonts = BTreeMap::new();
        let mut default = None;
        for source in sources {
            let (id, data) = match source {
                FontSource::Path(path) => {
                    let Ok(data) = std::fs::read(path) else {
                        continue;
                    };
                    let id = path
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    (id, data)
                }
                FontSource::Embedded { id, bytes } => ((*id).to_owned(), bytes.to_vec()),
            };
            let Ok(font) = FontVec::try_from_vec_and_index(data, 0) else {
                continue;
            };
            if fonts.contains_key(&id) {
                continue;
            }
            default.get_or_insert_with(|| id.clone());
            fonts.insert(id, font);
        }
        let default = default.ok_or_else(|| {
            format!(
                "no label font could be loaded (tried {})",
                sources
                    .iter()
                    .map(|s| match s {
                        FontSource::Path(p) => p.display().to_string(),
                        FontSource::Embedded { id, .. } => format!("embedded {id}"),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        Ok(Self { fonts, default })
    }

    /// The embedded font alone.
    ///
    /// # Errors
    /// Cannot fail unless the embedded bytes are not a font.
    pub fn embedded() -> Result<Self, String> {
        Self::load(&[FontSource::Embedded {
            id: EMBEDDED_FONT_ID,
            bytes: EMBEDDED_FONT,
        }])
    }

    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.fonts.keys().map(String::as_str).collect()
    }

    #[must_use]
    pub fn default_id(&self) -> &str {
        &self.default
    }

    /// The font for a job, or the id that is not in the catalog.
    ///
    /// # Errors
    /// Returns the unknown id.
    pub fn resolve<'a>(&'a self, id: Option<&'a str>) -> Result<&'a FontVec, &'a str> {
        let id = id.unwrap_or(&self.default);
        self.fonts.get(id).ok_or(id)
    }
}

/// Renders a job for a tape of `tape_px` printable pixels — the one
/// path shared by printing and previewing.
///
/// # Errors
/// Returns a human-readable reason when the font is unknown or the text
/// cannot be laid out.
pub fn render_job(
    catalog: &FontCatalog,
    job: &PrintJob,
    tape_px: usize,
) -> Result<render::Bitmap, String> {
    let font = catalog
        .resolve(job.font.as_deref())
        .map_err(|id| format!("unknown font: {id}"))?;
    let lines: Vec<&str> = job.lines.iter().map(String::as_str).collect();
    let height = print_height(tape_px, job.offset_percent);
    render::render_lines(font, &lines, height, job.font_scale_percent).map_err(|e| e.to_string())
}

/// Tape length a bitmap occupies, before the printer's own leader and
/// cut margins.
#[must_use]
#[allow(clippy::cast_precision_loss)] // label widths are a few thousand px
pub fn length_mm(width_px: usize) -> f64 {
    (width_px as f64 * 25.4 / DPI * 10.0).round() / 10.0
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

/// Renders with the catalog and prints over USB.
pub struct UsbPrinter {
    catalog: Arc<FontCatalog>,
}

impl UsbPrinter {
    #[must_use]
    pub fn new(catalog: Arc<FontCatalog>) -> Self {
        Self { catalog }
    }
}

impl Printer for UsbPrinter {
    fn print(&self, job: &PrintJob) -> Result<String, String> {
        let mut transport = ptouch::UsbTransport::open().map_err(|e| e.to_string())?;
        let mut size = (0, 0);
        let status = ptouch::print(&mut transport, true, |status| {
            let bitmap = render_job(&self.catalog, job, status.tape_px)?;
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
    catalog: Arc<FontCatalog>,
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
    font: Option<String>,
    font_scale_percent: Option<i64>,
    /// Preview only: the tape to assume.
    tape_mm: Option<i64>,
}

#[derive(Serialize)]
struct PreviewResponse {
    png_base64: String,
    width_px: usize,
    height_px: usize,
    tape_px: usize,
    length_mm: f64,
}

#[derive(Serialize)]
struct FontsResponse {
    fonts: Vec<String>,
    default: String,
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

pub fn app(printer: Arc<dyn Printer>, catalog: Arc<FontCatalog>) -> Router {
    let api = Router::new()
        .route("/health", get(api_health))
        .route("/fonts", get(api_fonts))
        .route("/preview", post(api_preview))
        .route("/print", post(api_print))
        .fallback(api_not_found)
        .with_state(AppState { printer, catalog });

    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api", api)
        .fallback(client_asset)
        .layer(TraceLayer::new_for_http())
}

/// Serves the embedded client: a matching file with its MIME type,
/// otherwise `index.html` so client-side routes survive a reload.
async fn client_asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let file = CLIENT
        .get_file(path)
        .or_else(|| CLIENT.get_file("index.html"));
    match file {
        Some(file) => {
            let mime = mime_guess::from_path(file.path()).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], file.contents()).into_response()
        }
        None => (StatusCode::NOT_FOUND, "client is not built\n").into_response(),
    }
}

async fn healthz() -> &'static str {
    "ok\n"
}

async fn api_health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn api_fonts(State(state): State<AppState>) -> Json<FontsResponse> {
    Json(FontsResponse {
        fonts: state
            .catalog
            .ids()
            .iter()
            .map(|id| (*id).to_owned())
            .collect(),
        default: state.catalog.default_id().to_owned(),
    })
}

/// Validates an optional integer field into `min..=max`.
fn percent(value: Option<i64>, default: u8, min: u8, max: u8, name: &str) -> Result<u8, String> {
    let Some(value) = value else {
        return Ok(default);
    };
    u8::try_from(value)
        .ok()
        .filter(|&v| (min..=max).contains(&v))
        .ok_or_else(|| format!("{name} must be {min}..={max}, got {value}"))
}

/// Turns a request into a job, or the 400 message.
fn job_from(request: &PrintRequest) -> Result<PrintJob, String> {
    let lines = label_lines(&request.text).ok_or_else(|| "text is empty".to_owned())?;
    let offset_percent = percent(
        request.offset_percent,
        DEFAULT_OFFSET_PERCENT,
        0,
        MAX_OFFSET_PERCENT,
        "offset_percent",
    )?;
    let font_scale_percent = percent(
        request.font_scale_percent,
        DEFAULT_FONT_SCALE_PERCENT,
        MIN_FONT_SCALE_PERCENT,
        100,
        "font_scale_percent",
    )?;
    Ok(PrintJob {
        lines,
        offset_percent,
        font: request.font.clone(),
        font_scale_percent,
    })
}

async fn api_preview(State(state): State<AppState>, Json(request): Json<PrintRequest>) -> Response {
    let job = match job_from(&request) {
        Ok(job) => job,
        Err(message) => return error(StatusCode::BAD_REQUEST, message),
    };
    let tape_mm = match percent(request.tape_mm, DEFAULT_TAPE_MM, 1, 99, "tape_mm") {
        Ok(mm) => mm,
        Err(message) => return error(StatusCode::BAD_REQUEST, message),
    };
    let Some(tape_px) = ptouch::tape_width_px(tape_mm).filter(|&px| px <= ptouch::MAX_PX) else {
        return error(
            StatusCode::BAD_REQUEST,
            format!("tape_mm {tape_mm} is not a PT-P700 tape"),
        );
    };
    let bitmap = match render_job(&state.catalog, &job, tape_px) {
        Ok(bitmap) => bitmap,
        Err(message) => return error(StatusCode::BAD_REQUEST, message),
    };
    let png = match render::encode_png(&bitmap) {
        Ok(png) => png,
        Err(message) => return error(StatusCode::INTERNAL_SERVER_ERROR, message),
    };
    Json(PreviewResponse {
        png_base64: base64::engine::general_purpose::STANDARD.encode(png),
        width_px: bitmap.width,
        height_px: bitmap.height,
        tape_px,
        length_mm: length_mm(bitmap.width),
    })
    .into_response()
}

async fn api_print(State(state): State<AppState>, Json(request): Json<PrintRequest>) -> Response {
    let job = match job_from(&request) {
        Ok(job) => job,
        Err(message) => return error(StatusCode::BAD_REQUEST, message),
    };
    if let Err(id) = state.catalog.resolve(job.font.as_deref()) {
        return error(StatusCode::BAD_REQUEST, format!("unknown font: {id}"));
    }
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
    use base64::Engine;
    use tower::ServiceExt;

    use super::{
        EMBEDDED_FONT, EMBEDDED_FONT_ID, FontCatalog, FontSource, PrintJob, Printer, app,
        label_lines, length_mm, print_height,
    };

    fn catalog() -> Arc<FontCatalog> {
        Arc::new(FontCatalog::embedded().unwrap())
    }

    #[test]
    fn the_embedded_font_is_the_default_unless_a_file_comes_first() {
        let embedded = FontSource::Embedded {
            id: EMBEDDED_FONT_ID,
            bytes: EMBEDDED_FONT,
        };
        let dir = std::env::temp_dir().join(format!("label-server-font-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("MyLabelFont.ttf");
        std::fs::write(&file, EMBEDDED_FONT).unwrap();
        let missing = dir.join("missing.ttf");

        let catalog =
            FontCatalog::load(&[embedded.clone(), FontSource::Path(file.clone())]).unwrap();
        assert_eq!(catalog.default_id(), "BIZUDPGothic-Regular");
        assert_eq!(catalog.ids(), ["BIZUDPGothic-Regular", "MyLabelFont"]);

        let catalog =
            FontCatalog::load(&[FontSource::Path(missing), FontSource::Path(file), embedded])
                .unwrap();
        assert_eq!(catalog.default_id(), "MyLabelFont");
        assert!(catalog.resolve(Some("BIZUDPGothic-Regular")).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

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

        let response = app(printer.clone(), catalog())
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
                font: None,
                font_scale_percent: 100,
            }]
        );
    }

    #[tokio::test]
    async fn an_explicit_offset_reaches_the_printer_and_out_of_range_is_rejected() {
        let printer = Arc::new(FakePrinter::default());

        let response = app(printer.clone(), catalog())
            .oneshot(json_request(
                &serde_json::json!({ "text": "abc", "offset_percent": 20 }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(printer.jobs.lock().unwrap()[0].offset_percent, 20);

        for out_of_range in [50, -1, 256] {
            let response = app(printer.clone(), catalog())
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

    #[tokio::test]
    async fn font_and_scale_reach_the_printer_and_unknown_fonts_are_rejected() {
        let printer = Arc::new(FakePrinter::default());

        let response = app(printer.clone(), catalog())
            .oneshot(json_request(&serde_json::json!({
                "text": "abc", "font": "BIZUDPGothic-Regular", "font_scale_percent": 60
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let job = printer.jobs.lock().unwrap()[0].clone();
        assert_eq!(job.font.as_deref(), Some("BIZUDPGothic-Regular"));
        assert_eq!(job.font_scale_percent, 60);

        let response = app(printer.clone(), catalog())
            .oneshot(json_request(
                &serde_json::json!({ "text": "abc", "font": "Comic" }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(body_string(response).await.contains("unknown font: Comic"));
        assert_eq!(printer.jobs.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn fonts_endpoint_lists_the_catalog_with_its_default() {
        let response = app(Arc::new(FakePrinter::default()), catalog())
            .oneshot(
                Request::builder()
                    .uri("/api/fonts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            body_string(response).await,
            r#"{"fonts":["BIZUDPGothic-Regular"],"default":"BIZUDPGothic-Regular"}"#
        );
    }

    #[tokio::test]
    async fn preview_renders_the_same_bitmap_print_would_send_as_png() {
        let printer = Arc::new(FakePrinter::default());
        let request = serde_json::json!({ "text": "Gridfinity", "offset_percent": 5 });

        let response = app(printer, catalog())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/preview")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_str(&body_string(response).await).unwrap();

        // same renderer, same job, same assumed 12mm tape
        let expected = super::render_job(
            &catalog(),
            &PrintJob {
                lines: vec!["Gridfinity".to_owned()],
                offset_percent: 5,
                font: None,
                font_scale_percent: 100,
            },
            76,
        )
        .unwrap();
        assert_eq!(body["width_px"], expected.width);
        assert_eq!(body["height_px"], 68);
        assert_eq!(body["tape_px"], 76);
        assert_eq!(body["length_mm"], length_mm(expected.width));
        let png = base64::engine::general_purpose::STANDARD
            .decode(body["png_base64"].as_str().unwrap())
            .unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let decoded = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        assert_eq!(decoded.info().width as usize, expected.width);
        assert_eq!(decoded.info().height as usize, 68);
    }

    #[tokio::test]
    async fn preview_rejects_unknown_tapes_and_fonts() {
        for payload in [
            serde_json::json!({ "text": "abc", "tape_mm": 36 }),
            serde_json::json!({ "text": "abc", "font": "Comic" }),
            serde_json::json!({ "text": "abc", "font_scale_percent": 5 }),
        ] {
            let response = app(Arc::new(FakePrinter::default()), catalog())
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/preview")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{payload}");
        }
    }

    #[test]
    fn length_is_derived_from_180_dpi() {
        assert!((length_mm(180) - 25.4).abs() < 1e-9);
        assert!((length_mm(348) - 49.1).abs() < 1e-9);
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

        let response = app(printer.clone(), catalog())
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

        let response = app(printer, catalog())
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
        let response = app(Arc::new(FakePrinter::default()), catalog())
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
        let response = app(Arc::new(FakePrinter::default()), catalog())
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

    async fn get(uri: &str) -> axum::response::Response {
        app(Arc::new(FakePrinter::default()), catalog())
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn the_embedded_client_is_served_with_its_mime_types() {
        let response = get("/").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
        let html = body_string(response).await;
        assert!(html.contains("<title>label-server</title>"));

        // the bundle referenced by index.html is embedded too
        let script = html
            .split("src=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap()
            .to_owned();
        assert!(script.starts_with("/assets/index-"), "{script}");
        assert!(
            std::path::Path::new(&script).extension() == Some("js".as_ref()),
            "{script}"
        );
        let response = get(&script).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/javascript"
        );
        let css = html
            .split("href=\"")
            .find(|rest| rest.starts_with("/assets/"))
            .and_then(|rest| rest.split('"').next())
            .unwrap()
            .to_owned();
        let response = get(&css).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("content-type").unwrap(), "text/css");
    }

    #[tokio::test]
    async fn unknown_client_routes_return_the_spa_with_success() {
        let response = get("/anything/deep").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
        assert!(
            body_string(response)
                .await
                .contains("<title>label-server</title>")
        );
    }
}
