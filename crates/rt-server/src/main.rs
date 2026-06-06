use axum::{routing::get, Router};
use std::net::SocketAddr;

use crate::state::AppState;
mod state;
mod handlers;

async fn health_handler() -> &'static str {
    "hello ray tracer"
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/health", get(health_handler));

    let addr = SocketAddr::from(([127, 0, 1, 1], 3000));
    println!("Servidor de pruebas corriendo en http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests;
