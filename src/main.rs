mod git;
mod messages;
mod server;
mod service;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use clap::{Parser, Subcommand};
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
    #[command(subcommand)]
    command: Option<Command>,

    /// Only git repos below the root will be considered
    root: Option<String>,

    /// The port on which the server will listen
    #[arg(short, long, default_value_t = 8765)]
    port: u16,

    /// Increase verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Command {
    /// Install diffodil as a background service (launchd on macOS)
    Install {
        /// Root directory to watch
        root: String,
        /// Port to listen on
        #[arg(short, long, default_value_t = 8765)]
        port: u16,
        /// Print the generated plist to stdout instead of installing
        #[arg(long)]
        print: bool,
    },
    /// Uninstall the background service
    Uninstall,
    /// Restart the background service
    Restart,
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
        None => match Assets::get("index.html") {
            Some(content) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html".to_string())],
                content.data.into_owned(),
            )
                .into_response(),
            None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
        },
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

    match cli.command {
        Some(Command::Install { root, port, print }) => {
            service::install(&PathBuf::from(&root), port, print);
            return;
        }
        Some(Command::Uninstall) => {
            service::uninstall();
            return;
        }
        Some(Command::Restart) => {
            service::restart();
            return;
        }
        None => {}
    }

    let root_arg = cli.root.unwrap_or_else(|| {
        eprintln!("Error: a root directory is required when running the server");
        eprintln!("Usage: diffodil <ROOT> [--port <PORT>]");
        std::process::exit(1);
    });

    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .init();

    let root = PathBuf::from(&root_arg).canonicalize().unwrap_or_else(|_| {
        eprintln!("Error: path '{}' does not exist", root_arg);
        std::process::exit(1);
    });

    let repos = find_git_repos(&root);
    info!("Found {} git repos under {}", repos.len(), root.display());

    let app_state = Arc::new(AppState { repos, root });

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
