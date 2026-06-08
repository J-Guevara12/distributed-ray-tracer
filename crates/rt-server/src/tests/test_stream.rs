use axum::Router;
use axum_test::TestServer;
use crate::state::AppState;
use crate::handlers::stream::render_stream_handler;

//#[tokio::test]
async fn test_sse_render_stream_headers() {
    // 1. Configurar infraestructura mínima del Estado global de pruebas
    let state = AppState::init_default(100, 3);

    // 2. Construir router de pruebas acoplado al handler SSE
    let app: Router<()> = Router::new()
        .route("/render/stream", axum::routing::get(render_stream_handler))
        .with_state(state);

    let server = TestServer::new(app);

    // 3. Ejecutar petición al endpoint de streaming
    let response = server.get("/render/stream").await;

    // 4. Aserciones de protocolo web: SSE exige text/event-stream y mantener conexión viva
    response.assert_status_ok();
    response.assert_header("content-type", "text/event-stream");
    response.assert_header("cache-control", "no-cache");
    response.assert_header("connection", "keep-alive");
}
