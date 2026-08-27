//! label-server executable.
//!
//! Copyright (C) 2026 label-server contributors.
//! Licensed under the GNU General Public License version 3 (GPL-3.0-only).

use std::{env, error::Error, net::SocketAddr, path::PathBuf, sync::Arc};

use label_server::{EMBEDDED_FONT, EMBEDDED_FONT_ID, FontCatalog, FontSource};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3000";

/// System fonts offered next to the embedded one when present. Face 0 of
/// the Noto CJK collection is the Japanese face.
const SYSTEM_FONTS: &[&str] = &[
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
    let catalog = Arc::new(FontCatalog::load(&font_sources())?);
    info!(fonts = ?catalog.ids(), default = catalog.default_id(), "label fonts loaded");
    let printer = Arc::new(label_server::UsbPrinter::new(Arc::clone(&catalog)));

    let listener = TcpListener::bind(bind_addr).await?;
    info!(%bind_addr, "server listening");

    axum::serve(listener, label_server::app(printer, catalog))
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

/// `LABEL_FONT` first (so it becomes the default), then the embedded
/// font, then the system candidates; every loadable one joins the catalog.
fn font_sources() -> Vec<FontSource> {
    let mut sources: Vec<FontSource> = env::var_os("LABEL_FONT")
        .map(|path| FontSource::Path(PathBuf::from(path)))
        .into_iter()
        .collect();
    sources.push(FontSource::Embedded {
        id: EMBEDDED_FONT_ID,
        bytes: EMBEDDED_FONT,
    });
    sources.extend(
        SYSTEM_FONTS
            .iter()
            .map(|path| FontSource::Path(PathBuf::from(path))),
    );
    sources
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
