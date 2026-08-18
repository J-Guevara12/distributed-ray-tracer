use crate::framebuffer::FrameBuffer;
use crate::tiles::{Tile, TileResult};
use rt_core::Vec4;
use std::sync::Arc;
use std::thread;

/// Suma de radiancia con w = número de muestras, que es lo que guarda el buffer.
fn px(r: f32, g: f32, b: f32) -> Vec4 {
    Vec4::new(r, g, b, 1.0)
}

#[test]
fn test_framebuffer_write_and_snapshot() {
    let fb = FrameBuffer::new(4, 4);

    let original_tile = Tile {
        id: 0,
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    };

    let red = px(1.0, 0.0, 0.0);
    let green = px(0.0, 1.0, 0.0);

    let tile_result = TileResult {
        original_tile,
        pixels: vec![red, red, green, green],
    };

    fb.write_tile(&tile_result);

    let snapshot = fb.get_snapshot();

    // Fila global 0: índice = y * width + x
    assert_eq!(snapshot[0], red);
    assert_eq!(snapshot[1], red);

    // Fila global 1 => 1 * 4 + 0 = 4
    assert_eq!(snapshot[4], green);
    assert_eq!(snapshot[5], green);

    // Fuera del tile: sin muestras, así que `resolve` lo devuelve en negro.
    assert_eq!(snapshot[2], Vec4::ZERO);
    assert_eq!(snapshot[2].w, 0.0);
}

#[test]
fn test_framebuffer_write_offset_block() {
    let fb = FrameBuffer::new(4, 4);

    let original_tile = Tile {
        id: 1,
        x: 2,
        y: 2,
        width: 2,
        height: 2,
    };

    let blue = px(0.0, 0.0, 1.0);

    let tile_result = TileResult {
        original_tile,
        pixels: vec![blue; 4],
    };

    fb.write_tile(&tile_result);
    let snapshot = fb.get_snapshot();

    assert_eq!(snapshot[0], Vec4::ZERO);

    // (2,2) => 2 * 4 + 2 = 10
    assert_eq!(snapshot[10], blue);
    assert_eq!(snapshot[11], blue);
    assert_eq!(snapshot[14], blue);
    assert_eq!(snapshot[15], blue);
}

#[test]
fn test_concurrent_framebuffer_writes() {
    let width = 100;
    let height = 100;
    let framebuffer = Arc::new(FrameBuffer::new(width, height));

    let mut handles = vec![];

    for id in 0..4 {
        let fb_clone = Arc::clone(&framebuffer);
        let handle = thread::spawn(move || {
            let original_tile = Tile {
                id,
                x: (id * 2) as u32,
                y: 0,
                width: 2,
                height: 2,
            };

            let result = TileResult {
                original_tile,
                pixels: vec![px(id as f32, 0.0, 0.0); 4],
            };
            fb_clone.write_tile(&result);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let snapshot = framebuffer.get_snapshot();
    assert_eq!(snapshot.len(), (width * height) as usize);

    // Cada hilo escribió un bloque disjunto; ninguno debió pisar al otro.
    for id in 0..4u32 {
        let index = (id * 2) as usize;
        assert_eq!(snapshot[index], px(id as f32, 0.0, 0.0));
    }
}
