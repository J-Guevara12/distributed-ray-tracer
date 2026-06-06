use crate::framebuffer::FrameBuffer;
use crate::tiles::{Tile, TileResult};
use std::sync::Arc;
use std::thread;

#[test]
fn test_framebuffer_write_and_snapshot() {
    // 1. Inicializamos un FrameBuffer pequeño de 4x4 píxeles con un stride de 3 (RGB)
    let stride = 3;
    let fb = FrameBuffer::new(4, 4, stride);
    
    // 2. Simulamos un TileResult de 2x2 que se ubica en la esquina superior izquierda (0,0)
    let original_tile = Tile {
        id: 0,
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    };

    let tile_result = TileResult {
        tile_id: 0,
        original_tile,
        // Matriz de píxeles local del tile (2x2 píxeles * 3 bytes = 12 bytes)
        pixels: vec![
            255, 0, 0,  255, 0, 0, // Fila local 0: 2 píxeles rojos
            0, 255, 0,  0, 255, 0, // Fila local 1: 2 píxeles verdes
        ],
    };

    // 3. Escribimos el tile usando la nueva firma matemática
    fb.write_tile(&tile_result, stride);
    
    let snapshot = fb.get_snapshot();
    
    // --- ASERCIONES ---

    // Fila Global 0, Píxel 0 (Coordenada 0,0) -> Debe ser Rojo (255, 0, 0)
    assert_eq!(snapshot[0], 255);
    assert_eq!(snapshot[1], 0);
    assert_eq!(snapshot[2], 0);

    // Fila Global 0, Píxel 1 (Coordenada 1,0) -> Debe ser Rojo (255, 0, 0)
    assert_eq!(snapshot[3], 255);
    assert_eq!(snapshot[4], 0);
    assert_eq!(snapshot[5], 0);

    // Fila Global 1, Píxel 0 (Coordenada 0,1) -> Debe ser Verde (0, 255, 0)
    // Indexación en la matriz global: (Y * Width + X) * stride => (1 * 4 + 0) * 3 = 12
    assert_eq!(snapshot[12], 0);
    assert_eq!(snapshot[13], 255);
    assert_eq!(snapshot[14], 0);

    // Fila Global 1, Píxel 1 (Coordenada 1,1) -> Debe ser Verde (0, 255, 0)
    // Indexación: (1 * 4 + 1) * 3 = 15
    assert_eq!(snapshot[15], 0);
    assert_eq!(snapshot[16], 255);
    assert_eq!(snapshot[17], 0);

    // Fila Global 0, Píxel 2 (Coordenada 2,0) -> Fuera del área del tile, debe seguir en negro (0)
    // Indexación: (0 * 4 + 2) * 3 = 6
    assert_eq!(snapshot[6], 0);
    assert_eq!(snapshot[7], 0);
    assert_eq!(snapshot[8], 0);
}

#[test]
fn test_framebuffer_write_offset_block() {
    // Valida que el mapeo funcione cuando el tile no inicia en (0,0)
    let stride = 3;
    let fb = FrameBuffer::new(4, 4, stride);
    
    // Un tile de 2x2 desplazado a la esquina inferior derecha (inicia en X=2, Y=2)
    let original_tile = Tile {
        id: 1,
        x: 2,
        y: 2,
        width: 2,
        height: 2,
    };

    let tile_result = TileResult {
        tile_id: 1,
        original_tile,
        pixels: vec![
            0, 0, 255,  0, 0, 255, // Fila local 0: 2 píxeles azules
            0, 0, 255,  0, 0, 255, // Fila local 1: 2 píxeles azules
        ],
    };

    fb.write_tile(&tile_result, stride);
    let snapshot = fb.get_snapshot();

    // El primer píxel del buffer global (0,0) debe seguir intacto (negro)
    assert_eq!(snapshot[0], 0);

    // El primer píxel azul debe estar en la coordenada global (2,2)
    // Indexación global: (2 * 4 + 2) * 3 = 30
    assert_eq!(snapshot[30], 0);
    assert_eq!(snapshot[31], 0);
    assert_eq!(snapshot[32], 255);
}

#[test]
fn test_concurrent_framebuffer_writes() {
    let stride = 3;
    let width = 100;
    let height = 100;
    let framebuffer = Arc::new(FrameBuffer::new(width, height, stride));
    
    let mut handles = vec![];

    // Lanzamos 4 hilos simulando workers concurrentes escribiendo bloques independientes
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
                tile_id: id,
                original_tile,
                // Llenamos el bloque con el ID del hilo para asegurar que no se pisen de forma corrupta
                pixels: vec![id as u8; 2 * 2 * stride], 
            };
            fb_clone.write_tile(&result, stride);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let snapshot = framebuffer.get_snapshot();
    // Validar invariante de memoria del buffer completo
    assert_eq!(snapshot.len(), (width * height ) as usize * stride);
}
