use crate::tiles::{TileGenerator, Tile};

#[test]
fn test_tile_generation_perfect_division() {
    // Pantalla de 64x64 partida en bloques de 32x32. Debería dar exactamente 4 tiles uniformes.
    let generator = TileGenerator::new(64, 64, 32);
    let tiles: Vec<Tile> = generator.collect();

    assert_eq!(tiles.len(), 4, "Deberían generarse exactamente 4 tiles");
    
    // Verificar que los IDs sean secuenciales
    for (i, tile) in tiles.iter().enumerate() {
        assert_eq!(tile.id, i, "El ID del tile debe ser secuencial");
        assert_eq!(tile.width, 32);
        assert_eq!(tile.height, 32);
    }

    // Verificar coordenadas específicas del último tile (esquina inferior derecha)
    let last_tile = tiles[3];
    assert_eq!(last_tile.x, 32);
    assert_eq!(last_tile.y, 32);
}

#[test]
fn test_tile_generation_truncation_edge_cases() {
    // Pantalla de 50x50 partida en bloques de 20x20. 
    // Las dimensiones horizontales y verticales deberían truncarse a 10 en los bordes.
    let generator = TileGenerator::new(50, 50, 20);
    let tiles: Vec<Tile> = generator.collect();

    // Rejilla de 3x3 = 9 tiles totales
    assert_eq!(tiles.len(), 9);

    // Primer tile (centro/esquina superior izquierda): tamaño completo
    assert_eq!(tiles[0].width, 20);
    assert_eq!(tiles[0].height, 20);

    // Tercer tile (borde derecho superior): truncado en ancho
    assert_eq!(tiles[2].x, 40);
    assert_eq!(tiles[2].width, 10, "El ancho del borde derecho debe truncarse a 10");
    assert_eq!(tiles[2].height, 20);

    // Séptimo tile (borde inferior izquierdo): truncado en alto
    assert_eq!(tiles[6].y, 40);
    assert_eq!(tiles[6].width, 20);
    assert_eq!(tiles[6].height, 10, "El alto del borde inferior debe truncarse a 10");

    // Último tile (esquina inferior derecha): truncado en ambos ejes
    assert_eq!(tiles[8].width, 10);
    assert_eq!(tiles[8].height, 10);
}

#[test]
fn test_total_pixel_coverage() {
    let width = 801;
    let height = 601;
    let tile_size = 16;
    
    let generator = TileGenerator::new(width, height, tile_size);
    
    let mut total_pixels_processed = 0;
    for tile in generator {
        total_pixels_processed += tile.width * tile.height;
    }

    assert_eq!(total_pixels_processed, width * height, 
        "La suma del área de los tiles debe ser idéntica al total de píxeles de la imagen original");
}
