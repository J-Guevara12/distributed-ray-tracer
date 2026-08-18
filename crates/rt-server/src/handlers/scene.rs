
use axum::{Json, extract::State, http::StatusCode};
use rt_core::dto::ScenePayload;
use rt_scene::{bvh::BvhNode, hittable_list::HittableList};

use crate::state::AppState;


pub async fn get_scene_handler(State(state): State<AppState>) -> Result<Json<ScenePayload>, StatusCode>{
    let scene_lock = state.scene_data.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(ref escena) = *scene_lock {
        Ok(Json(escena.clone())) // Devolvemos el espejo exacto y limpio
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn post_scene_handler(
    State(state): State<AppState>,
    Json(payload): Json<ScenePayload>,
) -> Result<StatusCode, StatusCode> {

    let hittable_list = HittableList::from(&payload);
    let world = BvhNode::build(hittable_list.objects);

    let mut world_lock = state.world.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut data_lock = state.scene_data.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    *world_lock = world;
    *data_lock = Some(payload);

    Ok(StatusCode::CREATED)
}
