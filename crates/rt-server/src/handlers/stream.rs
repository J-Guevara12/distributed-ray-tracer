use std::convert::Infallible;

use axum::{extract::State, response::{Sse, sse::Event}};
use futures_util::Stream;
use rt_core::display::{DisplayParams, resolve, to_srgb8};
use rt_renderer::tiles::{Tile, TilePatch };

use crate::state::AppState;

pub async fn render_stream_handler(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>>{
    let rx = state.tx_stream.subscribe();

    let current_snapshot = state.framebuffer.get_snapshot();
    let params = DisplayParams::default();
    let current_image = tokio::task::spawn_blocking(move || {
        to_srgb8(&resolve(&current_snapshot), &params)
    }).await.unwrap_or_default();


    let width = state.framebuffer.width;
    let height = state.framebuffer.height;

    let already_finished = state.is_finished.load(std::sync::atomic::Ordering::SeqCst);

    let sse_stream = convert_to_sse_stream(rx, current_image, width, height, already_finished).await;

    Sse::new(sse_stream)
}

pub async fn convert_to_sse_stream(
    mut rx: tokio::sync::broadcast::Receiver<TilePatch>,
    initial_snapshot: Vec<u8>,
    width: u32,
    height: u32,
    already_finished: bool
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        // --- EVENTO DE INICIALIZACIÓN ---
        // Si el FrameBuffer ya contiene datos (no está completamente vacío con ceros),
        // simulamos un "Tile gigante" que cubre toda la pantalla para pintar el estado actual de golpe.
        if !initial_snapshot.iter().all(|&b| b == 0) {
            let initial_tile = TilePatch {
                original_tile: Tile {
                    id: 999999,
                    x: 0,
                    y: 0,
                    width,
                    height,
                },
                pixels: initial_snapshot,
            };

            if let Ok(json_data) = serde_json::to_string(&initial_tile) {
                yield Ok(Event::default().data(json_data));
            }
        }

        if already_finished {
            yield Ok(Event::default().event("done").data("{\"status\":\"completed\"}"));
            return
        }

        loop {
            match rx.recv().await {
                Ok(tile_result) => {
                    if let Ok(json_data) = serde_json::to_string(&tile_result) {
                        let event = Event::default().data(json_data);

                        yield Ok(event)
                    }
                }
                // Si el canal se satura (Lagged), podemos elegir ignorarlo o reintentar
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                // Si el emisor se destruyó (Closed), terminamos el stream rompiendo el bucle
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    yield Ok(Event::default().event("done").data("{\"status\":\"completed\"}"));
                    break;
                }
            }
        }
    }
}
