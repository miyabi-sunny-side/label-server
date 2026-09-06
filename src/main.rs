//! label-server executable.
//!
//! Copyright (C) 2026 label-server contributors.
//! Licensed under the GNU General Public License version 3 (GPL-3.0-only).

use std::{env, error::Error, net::SocketAddr, path::PathBuf, sync::Arc};

use label_server::{EMBEDDED_FONT, EMBEDDED_FONT_ID, FontCatalog, FontSource};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_PORT: u16 = 3000;

/// System fonts offered next to the embedded one when present. Face 0 of
/// the Noto CJK collection is the Japanese face.
const SYSTEM_FONTS: &[&str] = &[
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // The release updater checks that a downloaded binary names its tag.
    if env::args_os().nth(1).is_some_and(|arg| arg == "--version") {
        println!("label-server {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

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
    let raw = match env::var("PORT") {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            return Err("PORT must be a valid Unicode integer".into());
        }
    };
    Ok(SocketAddr::from((
        [0, 0, 0, 0],
        parse_port(raw.as_deref())?,
    )))
}

fn parse_port(raw: Option<&str>) -> Result<u16, Box<dyn Error>> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_PORT);
    };
    raw.parse::<u16>()
        .ok()
        .filter(|port| *port != 0 && raw.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or_else(|| "PORT must be an integer from 1 to 65535".into())
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

#[cfg(test)]
mod tests {
    use super::parse_port;

    #[test]
    fn port_defaults_only_when_unset() {
        assert_eq!(parse_port(None).unwrap(), 3000);
        for (raw, expected) in [("1", 1), ("43127", 43127), ("65535", 65535)] {
            assert_eq!(parse_port(Some(raw)).unwrap(), expected);
        }
    }

    #[test]
    fn invalid_port_is_an_explicit_error() {
        for raw in [
            "",
            "0",
            "65536",
            "-1",
            "+3000",
            "3000 ",
            " 3000",
            "abc",
            "127.0.0.1:3000",
        ] {
            let error = parse_port(Some(raw)).unwrap_err();
            assert!(error.to_string().contains("PORT"), "{raw:?}: {error}");
        }
    }
}
