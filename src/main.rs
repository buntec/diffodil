mod git;
mod messages;
mod server;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use clap::Parser;
use rust_embed::Embed;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::git::find_git_repos;
use crate::server::{AppState, ws_handler};

#[derive(Embed)]
#[folder = "frontend_dist/"]
struct Assets;

#[derive(Parser)]
#[command(name = "diffodil", about = "Git diffs in your browser")]
struct Cli {
    /// Only git repos below the root will be considered
    root: String,

    /// The port on which the server will listen
    #[arg(short, long, default_value_t = 8765)]
    port: u16,

    /// Increase verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

async fn serve_embedded_file(path: &str) -> Response {
    let path = if path.is_empty() || path == "/" {
        "index.html"
    } else {
        path.strip_prefix('/').unwrap_or(path)
    };

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: serve index.html for all non-asset routes
            match Assets::get("index.html") {
                Some(content) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html".to_string())],
                    content.data.into_owned(),
                )
                    .into_response(),
                None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
            }
        }
    }
}

async fn static_handler(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    serve_embedded_file(&path).await
}

async fn index_handler() -> Response {
    serve_embedded_file("index.html").await
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .init();

    let root = PathBuf::from(&cli.root).canonicalize().unwrap_or_else(|_| {
        eprintln!("Error: path '{}' does not exist", cli.root);
        std::process::exit(1);
    });

    let repos = find_git_repos(&root);
    info!("Found {} git repos under {}", repos.len(), root.display());

    let app_state = Arc::new(AppState {
        repos,
        root,
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/", get(index_handler))
        .route("/{*path}", get(static_handler))
        .with_state(app_state);

    let addr = format!("0.0.0.0:{}", cli.port);
    info!("Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
