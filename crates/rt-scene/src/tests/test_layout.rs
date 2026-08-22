//! Struct sizes the roofline model depends on.
//!
//! `scripts/plot_roofline.py` needs bytes per node and per primitive to compute
//! arithmetic intensity, and to size the working set that decides which
//! bandwidth ceiling binds. Both were originally assumed from alignment rules,
//! which is exactly the kind of assumption that drifts silently: adding a field
//! to `FlatNode` would move the whole roofline point without anything failing.
//!
//! These are also the numbers behind the cache-line arithmetic in
//! LEARNED_LESSONS, so a change here invalidates a documented conclusion.

use std::mem::size_of;

use crate::bvh::Bvh;
use crate::primitive::Primitive;

const CACHE_LINE: usize = 64;

#[test]
fn test_flat_node_is_48_bytes() {
    assert_eq!(
        Bvh::NODE_BYTES, 48,
        "el tamaño del nodo cambió; hay que actualizar BYTES_PER_NODE en \
         scripts/plot_roofline.py y revisar la aritmética de líneas de caché \
         en LEARNED_LESSONS"
    );
}

/// 48 y no 32: `Sphere` lleva un `Vec3A` de centro, que alinea a 16 y estira
/// los 24 bytes de datos a 32; el discriminante del enum vuelve a redondear a
/// 48. O sea que cada esfera de B2 ocupa el doble de lo que informa.
///
/// Bajarlo pediría `Vec3` en el centro, y eso saca la intersección del camino
/// SIMD. Queda como está, pero medido en vez de supuesto.
#[test]
fn test_primitive_is_48_bytes() {
    assert_eq!(
        size_of::<Primitive>(), 48,
        "el tamaño de la primitiva cambió; hay que actualizar \
         BYTES_PER_PRIMITIVE en scripts/plot_roofline.py"
    );
}

/// Con 48 bytes caben 1.33 nodos por línea, no 2. Se eligió a sabiendas: dejar
/// el `Aabb` en `Vec3A` cuesta ese padding y compra cargas alineadas de una
/// instrucción, y a la escala de estas escenas el array entero vive en L1.
/// El día que la escena no quepa, la salida es empacar en dos `Vec4` y meter
/// `offset`/`count`/`axis` en los lanes `w` que hoy se desperdician.
#[test]
fn test_node_cache_line_occupancy_is_understood() {
    let per_line = CACHE_LINE as f32 / Bvh::NODE_BYTES as f32;

    assert!(
        (1.0..2.0).contains(&per_line),
        "con {} bytes por nodo caben {per_line:.2} por línea de caché; si llegó \
         a 2 o bajó de 1, el layout cambió de categoría",
        Bvh::NODE_BYTES
    );
}
