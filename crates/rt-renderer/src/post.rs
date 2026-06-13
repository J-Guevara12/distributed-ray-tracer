use rt_core::Color;
use serde::{Deserialize, Serialize};

/// Operador de tone mapping aplicado al pasar de radiancia lineal HDR a 8 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ToneMap {
    /// Recorte duro a [0,1]: comportamiento clásico LDR (pierde altas luces).
    Clamp,
    /// Compresión global de Reinhard: c / (1 + c).
    Reinhard,
    /// Curva fílmica ACES (ajuste de Narkowicz). Recomendada con materiales emisivos.
    #[default]
    Aces,
}

/// Parámetros de post-procesado aplicados al convertir el framebuffer HDR a 8 bits.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PostProcess {
    #[serde(default)]
    pub tone_map: ToneMap,
    #[serde(default = "default_exposure")]
    pub exposure: f32,
}

fn default_exposure() -> f32 {
    1.0
}

impl Default for PostProcess {
    fn default() -> Self {
        Self {
            tone_map: ToneMap::Aces,
            exposure: 1.0,
        }
    }
}

const GAMMA: f32 = 2.2;

impl PostProcess {
    /// Radiancia lineal -> espacio de display ([0,1], con gamma aplicada).
    pub fn map(&self, color: Color) -> Color {
        let exposed = color * self.exposure;

        let mapped = match self.tone_map {
            ToneMap::Clamp => exposed.clamp(Color::ZERO, Color::ONE),
            ToneMap::Reinhard => exposed / (Color::ONE + exposed),
            ToneMap::Aces => aces_narkowicz(exposed),
        };

        mapped.powf(1.0 / GAMMA)
    }

    pub fn to_rgb8(&self, color: Color) -> [u8; 3] {
        let display = self.map(color) * 255.0 + Color::splat(0.5);

        // `as u8` satura en [0, 255], no hay wrap-around
        [display.x as u8, display.y as u8, display.z as u8]
    }
}

/// Ajuste racional de la curva ACES (Krzysztof Narkowicz, 2015).
fn aces_narkowicz(c: Color) -> Color {
    const A: f32 = 2.51;
    const B: f32 = 0.03;
    const C: f32 = 2.43;
    const D: f32 = 0.59;
    const E: f32 = 0.14;

    let numerator = c * (A * c + Color::splat(B));
    let denominator = c * (C * c + Color::splat(D)) + Color::splat(E);

    (numerator / denominator).clamp(Color::ZERO, Color::ONE)
}
