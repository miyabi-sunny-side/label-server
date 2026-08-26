//! label-server executable.
//!
//! Copyright (C) 2026 label-server contributors.
//! Licensed under the GNU General Public License version 3 (GPL-3.0-only).

use std::{env, error::Error, net::SocketAddr, path::PathBuf, sync::Arc};

use ab_glyph::FontVec;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3000";

/// Fonts tried in order when `LABEL_FONT` is not set. Face 0 of the Noto
/// CJK collection is the Japanese face.
const DEFAULT_FONTS: &[&str] = &[
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let bind_addr = bind_addr_from_env()?;
    let (font_path, font) = load_font()?;
    info!(font = %font_path.display(), "label font loaded");
    let printer = Arc::new(label_server::UsbPrinter::new(font));

    let listener = TcpListener::bind(bind_addr).await?;
    info!(%bind_addr, "server listening");

    axum::serve(listener, label_server::app("client/dist", printer))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("server stopped");
    Ok(())
}

fn bind_addr_from_env() -> Result<SocketAddr, Box<dyn Error>> {
    env::var("APP_BIND_ADDR")
        .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned())
        .parse()
        .map_err(Into::into)
}

fn load_font() -> Result<(PathBuf, FontVec), Box<dyn Error>> {
    let candidates: Vec<PathBuf> = match env::var_os("LABEL_FONT") {
        Some(path) => vec![PathBuf::from(path)],
        None => DEFAULT_FONTS.iter().map(PathBuf::from).collect(),
    };
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            let font = FontVec::try_from_vec_and_index(data, 0)?;
            return Ok((path.clone(), font));
        }
    }
    Err(format!(
        "no label font found; set LABEL_FONT to a TTF/OTF/TTC file (tried {})",
        candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
    .into())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    info!("shutdown signal received");
}
