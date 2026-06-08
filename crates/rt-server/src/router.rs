use axum::{Router, routing::{get, post, put}};
use tower_http::cors::{CorsLayer, Any};
use crate::{handlers, state::AppState};


pub fn setup_app(app: Router<AppState>, state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any);
    
    return app
        .route("/health", get(handlers::health::health_handler))
        .route("/camera", get(handlers::camera::get_camera_handler))
        .route("/camera", put(handlers::camera::update_camera_handler))
        .route("/render", post(handlers::render::post_render))
        .route("/render/stream", get(handlers::stream::render_stream_handler))
        .with_state(state)
        .layer(cors)

}
