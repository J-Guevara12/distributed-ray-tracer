use crate::{handlers::health_handler, *};
use axum_test::TestServer;

#[tokio::test]
async fn test_health_endpoint() {
    let app = Router::new().route("/health", get(health_handler));
    let server = TestServer::new(app);

    let response = server.get("/health").await;
    response.assert_status_ok();
    response.assert_text("hello ray tracer");
}
