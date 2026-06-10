use axum::Router;
use std::net::SocketAddr;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::{state::AppState, router::setup_app};
mod state;
mod handlers;
mod router;

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let (tx, rx) = tokio::sync::mpsc::channel(1000);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_max_level(tracing::Level::INFO)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let stride = 3;
    let state = AppState::init_default(100, stride, tx);

    let app = setup_app(Router::new(), state);

    let addr = SocketAddr::from(([127, 0, 1, 1], 3000));
    println!("Servidor de pruebas corriendo en http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests;
