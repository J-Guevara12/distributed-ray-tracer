use crate::{camera::Camera, dto::ScenePayload};

pub struct Job {
    pub id: String,
    pub status: JobStatus,
    pub camera: Option<Camera>,
    pub scene: Option<ScenePayload>,
    pub created_at: u64,        // UNIX Timestamp
}

pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed(String)
} 
