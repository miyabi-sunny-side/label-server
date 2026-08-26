use std::path::{Path, PathBuf};

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

/// fontconfig name of the font handed to ptouch-print. A TTC path would
/// leave Latin glyphs blank, so the name form is deliberate.
pub const FONT: &str = "Noto Sans CJK JP";

#[derive(Clone)]
struct AppState {
    print_command: PathBuf,
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

/// Turns textarea text into ptouch-print arguments: one `-t` per line.
/// Returns `None` when there is nothing printable.
#[must_use]
pub fn print_args(text: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let first = lines.iter().position(|line| !line.trim().is_empty())?;
    let last = lines.iter().rposition(|line| !line.trim().is_empty())?;

    let mut args = vec!["--font".to_owned(), FONT.to_owned(), "--precut".to_owned()];
    for line in &lines[first..=last] {
        args.push("-t".to_owned());
        args.push((*line).to_owned());
    }
    Some(args)
}

pub fn app(static_dir: impl AsRef<Path>, print_command: impl Into<PathBuf>) -> Router {
    let static_dir = static_dir.as_ref().to_path_buf();
    let state = AppState {
        print_command: print_command.into(),
    };
    let api = Router::new()
        .route("/health", get(api_health))
        .route("/print", post(api_print))
        .fallback(api_not_found)
        .with_state(state);

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
    let Some(args) = print_args(&request.text) else {
        return error(StatusCode::BAD_REQUEST, "text is empty".to_owned());
    };

    let output = match tokio::process::Command::new(&state.print_command)
        .args(&args)
        .output()
        .await
    {
        Ok(output) => output,
        Err(err) => {
            return error(
                StatusCode::BAD_GATEWAY,
                format!("{}: {err}", state.print_command.display()),
            );
        }
    };

    if output.status.success() {
        Json(PrintResponse {
            output: String::from_utf8_lossy(&output.stdout).into_owned(),
        })
        .into_response()
    } else {
        error(
            StatusCode::BAD_GATEWAY,
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
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
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use tower::ServiceExt;

    use super::{app, print_args};

    /// A stand-in for ptouch-print: records its arguments (one per line)
    /// next to itself and exits with the given status.
    struct Stub {
        dir: PathBuf,
        command: PathBuf,
    }

    impl Stub {
        fn new(name: &str, exit_code: i32) -> Self {
            let dir =
                std::env::temp_dir().join(format!("label-server-{name}-{}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            let command = dir.join("ptouch-print");
            fs::write(
                &command,
                format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\n\
                     echo 'stub stdout'\necho 'stub stderr' >&2\nexit {exit_code}\n"
                ),
            )
            .unwrap();
            fs::set_permissions(&command, fs::Permissions::from_mode(0o755)).unwrap();
            Self { dir, command }
        }

        fn recorded_args(&self) -> Option<Vec<String>> {
            let recorded = fs::read_to_string(self.dir.join("ptouch-print.args")).ok()?;
            Some(recorded.lines().map(str::to_owned).collect())
        }
    }

    impl Drop for Stub {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
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
    fn print_args_maps_each_line_to_a_text_argument() {
        assert_eq!(
            print_args("abc\n\n12mm テスト  \n").unwrap(),
            [
                "--font",
                super::FONT,
                "--precut",
                "-t",
                "abc",
                "-t",
                "",
                "-t",
                "12mm テスト"
            ]
        );
    }

    #[test]
    fn print_args_rejects_blank_text() {
        assert_eq!(print_args(""), None);
        assert_eq!(print_args(" \n\t\n"), None);
    }

    #[tokio::test]
    async fn multi_line_text_is_printed_one_text_argument_per_line() {
        let stub = Stub::new("multi-line", 0);

        let response = app("client/dist", &stub.command)
            .oneshot(print_request("abc\ndef"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_string(response).await, r#"{"output":"stub stdout\n"}"#);
        assert_eq!(
            stub.recorded_args().unwrap(),
            ["--font", super::FONT, "--precut", "-t", "abc", "-t", "def"]
        );
    }

    #[tokio::test]
    async fn blank_text_is_rejected_without_touching_the_printer() {
        let stub = Stub::new("blank", 0);

        let response = app("client/dist", &stub.command)
            .oneshot(print_request(" \n"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(stub.recorded_args(), None);
    }

    #[tokio::test]
    async fn printer_failure_is_reported_with_its_stderr() {
        let stub = Stub::new("failure", 3);

        let response = app("client/dist", &stub.command)
            .oneshot(print_request("abc"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(body_string(response).await, r#"{"error":"stub stderr\n"}"#);
    }

    #[tokio::test]
    async fn missing_printer_command_is_reported() {
        let response = app("client/dist", "/nonexistent/ptouch-print")
            .oneshot(print_request("abc"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(
            body_string(response)
                .await
                .contains("/nonexistent/ptouch-print")
        );
    }

    #[tokio::test]
    async fn liveness_is_lightweight_plain_text() {
        let response = app("client/dist", "ptouch-print")
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
        let response = app("client/dist", "ptouch-print")
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
        let response = app("client", "ptouch-print")
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
