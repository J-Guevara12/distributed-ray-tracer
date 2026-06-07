use std::sync::Arc;

use rt_core::{Point3, Ray, Vec3};
use rt_scene::{geometry::Sphere, hittable_list::HittableList, materials::Lambertian};

use crate::tracers::{NormalTracer, RayTracer};

#[test]
fn test_tracer_fallback_to_gradient_on_miss() {
    let world = Arc::new(HittableList::new());
    let tracer = NormalTracer{};

    // Un rayo apuntando hacia arriba (Y = 1.0), debería dar el color azul del cielo puro
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let color = tracer.trace_ray(ray, world.as_ref());

    // El azul del gradiente es [128, 179, 255] aprox (t = 1.0)
    assert_eq!(color[2], 1.0); // El canal azul debe estar al tope
}

#[test]
fn test_tracer_renders_normal_on_hit() {
    let mut world = HittableList::new();
    // Esfera en frente de la cámara
    let material = Arc::new(Lambertian::new(Vec3::new(0.0, 0.0, 0.0)));
    world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.0, -1.0), 0.5, material)));
    
    let tracer = NormalTracer{};
    // Disparamos un rayo al centro exacto de la esfera
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));
    
    let color = tracer.trace_ray(ray, &world);
    println!("{}: color", color);
    
    // En el centro exacto, la normal apunta directo a la cámara (Z = 1.0)
    // Mapeo Z: 0.5 * (1.0 + 1.0) = 1.0 -> 255 en el canal Azul (color[2])
    // Mapeo X/Y: 0.5 * (0.0 + 1.0) = 0.5 -> ~128 en canales Rojo y Verde
    assert!((color[0] - 0.5).abs() <= 2.0/255.0);
    assert!((color[1] - 0.5).abs() <= 2.0/255.0);
    assert!(color[2] >= 254.0/255.0);
}
