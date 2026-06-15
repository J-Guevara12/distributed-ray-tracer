use serde::{Deserialize, Serialize};

use crate::{Color, Ray};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Background {
    #[serde(rename = "solid")]
    Solid { color: Color },
    #[serde(rename = "gradient")]
    Gradient { top: Color, bottom: Color },
}

impl Background {
    pub fn new_solid(color: Color) -> Self {
        Self::Solid { color }
    }

    pub fn new_gradient(top: Color, bottom: Color) -> Self {
        Self::Gradient { top, bottom }
    }

    pub fn emit(&self, ray: &Ray) -> Color {
        match self {
            Background::Solid { color } => *color,
            Background::Gradient { top, bottom } => {
                let t = 0.5 * (ray.direction[1] + 1.0);
                (1.0 - t) * bottom + t * top
            }
        }
    }
}
