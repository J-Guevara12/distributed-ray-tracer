use rt_core::{Vec4, display::{DisplayParams, resolve, to_srgb8}};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Tile {
    pub id: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Serialize, Clone)]
pub struct TilePatch {
    pub pixels: Vec<u8>,
    pub original_tile: Tile,
}

#[derive(Clone)]
pub struct TileResult {
    pub pixels: Vec<Vec4>,
    pub original_tile: Tile,
}

pub struct TileGenerator {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,

    current_x: u32,
    current_y: u32,
    current_id: usize,
}

impl TileGenerator {
    pub fn new(width: u32, height: u32, tile_size: u32) -> Self {
        Self { width, height, tile_size , current_x: 0, current_y: 0, current_id: 0}
    }
}

impl Iterator for TileGenerator {
    type Item = Tile;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_y >= self.height {
            return None
        }
        let x = self.current_x;
        let y = self.current_y;
        let id = self.current_id;

        let width = std::cmp::min(self.tile_size, self.width - x);
        let height = std::cmp::min(self.tile_size, self.height - y);

        let tile = Tile { id, x, y, width, height };

        self.current_id += 1;
        self.current_x += self.tile_size;

        if self.current_x >= self.width {
            self.current_x = 0;
            self.current_y += self.tile_size;
        }
    
        Some(tile)
    }
}

impl TilePatch {
    pub fn from_tile_result(value: &TileResult, params: &DisplayParams) -> Self {
        let original_tile = value.original_tile;
        let pixels = to_srgb8(&resolve(&value.pixels), params);
        Self { pixels , original_tile }
    }
}

