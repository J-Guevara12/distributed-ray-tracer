use std::convert::Infallible;

use axum::{extract::State, response::{Sse, sse::Event}};
use futures_util::Stream;
use rt_renderer::tiles::TileResult;

use crate::state::AppState;

pub async fn render_stream_handler(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>>{
    let rx = state.tx_stream.subscribe();

    let sse_stream = convert_to_sse_stream(rx).await;

    Sse::new(sse_stream)
}

pub async fn convert_to_sse_stream(mut rx: tokio::sync::broadcast::Receiver<TileResult>) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(tile_result) => {
                    if let Ok(json_data) = serde_json::to_string(&tile_result) {
                        let event = Event::default().data(json_data);

                        yield Ok(event)
                    }
                }
                // Si el canal se satura (Lagged), podemos elegir ignorarlo o reintentar
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    continue;
                }
                // Si el emisor se destruyó (Closed), terminamos el stream rompiendo el bucle
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    }
}
