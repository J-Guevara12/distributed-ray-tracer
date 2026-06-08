pub mod stream;
pub mod health;
pub mod camera;

use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
