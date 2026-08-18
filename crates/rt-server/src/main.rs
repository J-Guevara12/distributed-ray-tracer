use axum::Router;
use std::net::SocketAddr;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::{router::setup_app, state::AppState};
mod handlers;
mod router;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()>{
    let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let (tx, _) = tokio::sync::mpsc::channel(1000);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_max_level(tracing::Level::INFO)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let state = AppState::init_default(100, tx);

    let app = setup_app(Router::new(), state);

    let addr = SocketAddr::from(([127, 0, 1, 1], 3000));

    let listener = tokio::net::TcpListener::bind(addr).await.expect("Port 3000 is already being used");
    println!("Rendering server ready at http://{}", addr);

    axum::serve(listener, app).await.expect("Could not start axum web server");
    Ok(())
}

#[cfg(test)]
mod tests;
