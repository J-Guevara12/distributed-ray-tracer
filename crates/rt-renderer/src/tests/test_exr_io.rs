use rt_core::Vec3;

use crate::exr_io::{self, ExrError};

fn scratch(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("rt-exr-test-{name}.exr"));
    path
}

/// Lo único que una referencia necesita: que lo que se lee sea bit a bit lo que
/// se escribió. Si el crate guardara `f16` o comprimiera con pérdida, el MSE
/// medido contra ella tendría un piso artificial.
#[test]
fn test_roundtrip_is_bit_exact() {
    let (width, height) = (7u32, 5u32);
    let pixels: Vec<Vec3> = (0..width * height)
        .map(|i| {
            let i = i as f32;
            // Valores HDR fuera de [0,1] y un denormal, que es donde una
            // conversión a f16 se notaría.
            Vec3::new(i * 137.5, 1.0 / (i + 1.0), i * 1e-7)
        })
        .collect();

    let path = scratch("roundtrip");
    exr_io::write(&path, &pixels, width, height).expect("no se pudo escribir");
    let read = exr_io::read(&path).expect("no se pudo leer");
    let _ = std::fs::remove_file(&path);

    assert_eq!(read.dimensions(), (width, height));
    assert_eq!(read.len(), pixels.len());

    for (index, (original, recovered)) in pixels.iter().zip(read.pixels.iter()).enumerate() {
        assert_eq!(
            original, recovered,
            "el píxel {index} no sobrevivió el round-trip: {original:?} vs {recovered:?}"
        );
    }
}

/// El orden de las filas es donde se rompen los codecs de imagen. Un buffer
/// asimétrico lo detecta; uno cuadrado o uniforme no.
#[test]
fn test_roundtrip_preserves_row_order() {
    let (width, height) = (4u32, 3u32);
    let pixels: Vec<Vec3> = (0..width * height)
        .map(|i| Vec3::splat(i as f32))
        .collect();

    let path = scratch("order");
    exr_io::write(&path, &pixels, width, height).unwrap();
    let read = exr_io::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(read.pixels, pixels, "las filas volvieron en otro orden");
}

#[test]
fn test_write_rejects_wrong_pixel_count() {
    let pixels = vec![Vec3::ZERO; 10];
    let error = exr_io::write(scratch("bad"), &pixels, 4, 4).expect_err("debió rechazar 10 != 16");

    assert!(matches!(error, ExrError::Mismatch(_)));
}

#[test]
fn test_compare_identical_images_has_zero_error() {
    let pixels: Vec<Vec3> = (0..32).map(|i| Vec3::splat(i as f32 * 0.25)).collect();
    let result = exr_io::compare(&pixels, &pixels).unwrap();

    assert_eq!(result.mse, 0.0);
    assert_eq!(result.rmse, 0.0);
    assert_eq!(result.relative_mse, 0.0);
    assert_eq!(result.max_abs, 0.0);
}

#[test]
fn test_compare_computes_mse_over_channels() {
    // Un solo píxel con error de 2.0 en un canal: MSE = 4 / 3 canales.
    let render = vec![Vec3::new(3.0, 1.0, 1.0)];
    let reference = vec![Vec3::new(1.0, 1.0, 1.0)];

    let result = exr_io::compare(&render, &reference).unwrap();

    assert!((result.mse - 4.0 / 3.0).abs() < 1e-12, "mse = {}", result.mse);
    assert!((result.max_abs - 2.0).abs() < 1e-6);
    // El relativo divide por referencia² + eps = 1 + 0.01
    assert!((result.relative_mse - (4.0 / 1.01) / 3.0).abs() < 1e-9);
}

#[test]
fn test_compare_rejects_mismatched_sizes() {
    let error = exr_io::compare(&[Vec3::ZERO; 4], &[Vec3::ZERO; 5]).expect_err("distinto tamaño");
    assert!(matches!(error, ExrError::Mismatch(_)));
}

/// Un cambio que parte el tiempo y duplica el error no mejora nada, y la
/// eficiencia tiene que decirlo.
#[test]
fn test_efficiency_treats_time_and_error_symmetrically() {
    let base = exr_io::efficiency(0.01, 10.0);

    assert!((exr_io::efficiency(0.02, 5.0) - base).abs() < 1e-12, "empate no detectado");
    assert!(exr_io::efficiency(0.01, 5.0) > base, "mitad de tiempo, mismo error");
    assert!(exr_io::efficiency(0.02, 10.0) < base, "mismo tiempo, doble error");
    assert_eq!(exr_io::efficiency(0.0, 10.0), 0.0);
}
