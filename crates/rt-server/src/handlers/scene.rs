
use axum::{Json, extract::State, http::StatusCode};
use rt_core::dto::ScenePayload;
use rt_scene::{Scene, bvh::BvhNode, hittable_list::SceneData};
use std::sync::Arc;

use crate::state::AppState;


pub async fn get_scene_handler(State(state): State<AppState>) -> Result<Json<ScenePayload>, StatusCode>{
    let scene_lock = state.scene_data.read();

    match *scene_lock {
        Some(ref escena) => {
            Ok(Json(escena.clone())) // Devolvemos el espejo exacto y limpio
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn post_scene_handler(
    State(state): State<AppState>,
    Json(payload): Json<ScenePayload>,
) -> Result<StatusCode, StatusCode> {

    let data = SceneData::from(&payload);
    let world = Arc::new(Scene {
        world: BvhNode::build(data.objects),
        materials: data.materials,
        background: payload.background.clone(),
    });

    let mut world_lock = state.world.write();
    let mut data_lock = state.scene_data.write();

    *world_lock = world;
    *data_lock = Some(payload);

    Ok(StatusCode::CREATED)
}
