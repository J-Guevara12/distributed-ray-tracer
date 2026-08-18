use std::sync::Arc;
use rt_core::{Point3, Vec3, Ray, Color};
use crate::{HitRecord, Hittable, Interval, Material, aabb::Aabb, bvh::BvhNode};

// =========================================================================
// 1. COMPONENTES MOCK PARA AISLAR LAS PRUEBAS
// =========================================================================

#[derive(Clone, Debug)]
struct MockMaterial;
impl Material for MockMaterial {
    fn scatter(&self, _: &Ray, _: &HitRecord) -> Option<(Vec3, Ray)> { None }
    fn emitted(&self, _: f32, _: f32, _: Point3) -> Color { Color::ZERO }
}

// Un objeto ficticio con una posición y caja conocida para probar el árbol
struct MockHittable {
    bbox: Aabb,
    hit_point: Option<Point3>,
}

impl MockHittable {
    fn new(bbox: Aabb, hit_point: Option<Point3>) -> Self {
        Self { bbox, hit_point }
    }
}

impl Hittable for MockHittable {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        // Si el objeto está configurado para registrar un impacto, evaluamos si el rayo toca su caja
        if let Some(p) = self.hit_point && self.bbox.hit(*ray, ray_t) {
            static MAT: MockMaterial = MockMaterial;
            // El `t` tiene que corresponder al punto de impacto: el BVH
            // recorta el intervalo del hijo derecho con él, así que un `t`
            // fijo haría que el árbol descartara impactos válidos según
            // qué eje de corte le tocara al azar.
            let t = (p - ray.origin).dot(ray.direction);
            return Some(HitRecord::new(
                ray,
                t,
                Vec3::new(0.0, 0.0, 1.0),
                p,
                &MAT,
            ));
        }
        None
    }

    fn bounding_box(&self) -> Aabb {
        self.bbox
    }
}

// =========================================================================
// 2. PRUEBAS UNITARIAS PARA EL AABB (Volumen Envolvente)
// =========================================================================

#[test]
fn test_aabb_ray_intersection_hit() {
    // Caja de 2x2x2 centrada en el origen
    let bbox = Aabb {
        x: Interval::new(-1.0, 1.0),
        y: Interval::new(-1.0, 1.0),
        z: Interval::new(-1.0, 1.0),
    };

    // Un rayo que apunta directamente al centro desde el frente (Eje -Z)
    let ray_front = Ray {
        origin: Point3::new(0.0, 0.0, 5.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
    };
    let t_range = Interval::new(0.001, f32::INFINITY);
    assert!(bbox.hit(ray_front, t_range), "El rayo frontal debió impactar el centro de la caja");

    // Un rayo diagonal que raspa una esquina por dentro
    let ray_diagonal = Ray {
        origin: Point3::new(-2.0, -2.0, -2.0),
        direction: Vec3::new(1.0, 1.0, 1.0),
    };
    assert!(bbox.hit(ray_diagonal, t_range), "El rayo diagonal debió cruzar la caja de esquina a esquina");
}

#[test]
fn test_aabb_ray_intersection_miss() {
    let bbox = Aabb {
        x: Interval::new(-1.0, 1.0),
        y: Interval::new(-1.0, 1.0),
        z: Interval::new(-1.0, 1.0),
    };

    // Rayo completamente paralelo a la caja pero desplazado en X
    let ray_miss_parallel = Ray {
        origin: Point3::new(2.5, 0.0, 5.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
    };
    let t_range = Interval::new(0.001, f32::INFINITY);
    assert!(!bbox.hit(ray_miss_parallel, t_range), "Un rayo paralelo por fuera no debe impactar la caja");

    // Rayo que apunta en dirección opuesta a la ubicación de la caja
    let ray_wrong_direction = Ray {
        origin: Point3::new(0.0, 0.0, 5.0),
        direction: Vec3::new(0.0, 0.0, 1.0), // Se aleja en +Z
    };
    assert!(!bbox.hit(ray_wrong_direction, t_range), "Un rayo que se aleja de la caja no debe registrar impacto");
}

#[test]
fn test_aabb_interval_constraints() {
    let bbox = Aabb {
        x: Interval::new(1.0, 3.0),
        y: Interval::new(1.0, 3.0),
        z: Interval::new(1.0, 3.0),
    };

    let ray = Ray {
        origin: Point3::new(2.0, 2.0, 0.0),
        direction: Vec3::new(0.0, 0.0, 1.0),
    };

    // El rayo físicamente impactaría la caja en t = 1.0 (Z=1.0)
    // Forzamos un intervalo de tiempo restringido que termine antes de llegar [0.0, 0.5]
    let strict_range = Interval::new(0.0, 0.5);
    assert!(!bbox.hit(ray, strict_range), "La caja no debe registrar impacto si ocurre fuera del rango de tiempo t permitido");
}

#[test]
fn test_aabb_surrounding_box() {
    // Caja A en el cuadrante negativo
    let box_a = Aabb {
        x: Interval::new(-5.0, -2.0),
        y: Interval::new(-5.0, -2.0),
        z: Interval::new(-5.0, -2.0),
    };

    // Caja B en el cuadrante positivo
    let box_b = Aabb {
        x: Interval::new(1.0, 4.0),
        y: Interval::new(1.0, 4.0),
        z: Interval::new(1.0, 4.0),
    };

    // Generamos la caja contenedora de ambas
    let big_box = Aabb::surrounding_box(box_a, box_b);

    // Los límites de la caja grande deben expandirse para cubrir los extremos de ambas
    assert_eq!(big_box.x.min, -5.0);
    assert_eq!(big_box.x.max, 4.0);
    assert_eq!(big_box.y.min, -5.0);
    assert_eq!(big_box.y.max, 4.0);
    assert_eq!(big_box.z.min, -5.0);
    assert_eq!(big_box.z.max, 4.0);
}

// =========================================================================
// 3. PRUEBAS UNITARIAS PARA EL BVH_NODE (Estructura y Recorrido)
// =========================================================================

#[test]
fn test_bvh_construction_single_object() {
    let obj_box = Aabb {
        x: Interval::new(-1.0, 1.0),
        y: Interval::new(-1.0, 1.0),
        z: Interval::new(-1.0, 1.0),
    };
    let obj = Arc::new(MockHittable::new(obj_box, None));
    let objects: Vec<Arc<dyn Hittable>> = vec![obj];

    // Construir el nodo raíz con un único objeto
    let bvh_root = BvhNode::new(objects);

    // El algoritmo de Peter Shirley duplica la referencia en left y right cuando hay 1 solo objeto
    assert_eq!(bvh_root.bounding_box().x.min, -1.0);
    assert_eq!(bvh_root.bounding_box().x.max, 1.0);
}

#[test]
fn test_bvh_construction_multiple_objects_sorting() {
    // Creamos tres objetos separados a lo largo del eje X para forzar la división espacial
    let box_left = Aabb { x: Interval::new(-10.0, -8.0), y: Interval::new(-1.0, 1.0), z: Interval::new(-1.0, 1.0) };
    let box_center = Aabb { x: Interval::new(-1.0, 1.0), y: Interval::new(-1.0, 1.0), z: Interval::new(-1.0, 1.0) };
    let box_right = Aabb { x: Interval::new(8.0, 10.0), y: Interval::new(-1.0, 1.0), z: Interval::new(-1.0, 1.0) };

    let obj_l = Arc::new(MockHittable::new(box_left, None));
    let obj_c = Arc::new(MockHittable::new(box_center, None));
    let obj_r = Arc::new(MockHittable::new(box_right, None));

    let objects: Vec<Arc<dyn Hittable>> = vec![obj_l, obj_c, obj_r];
    let bvh_root = BvhNode::new(objects);

    // La caja contenedora de la raíz debe englobar absolutamente todo el espacio de los extremos
    let root_box = bvh_root.bounding_box();
    assert_eq!(root_box.x.min, -10.0);
    assert_eq!(root_box.x.max, 10.0);
}

#[test]
fn test_bvh_traversal_hit_and_short_circuit() {
    // Configuramos dos objetos en el eje Z
    // Objeto Cercano (Z = -2.0)
    let close_box = Aabb { x: Interval::new(-1.0, 1.0), y: Interval::new(-1.0, 1.0), z: Interval::new(-3.0, -1.0) };
    // Objeto Lejano (Z = -10.0)
    let far_box = Aabb { x: Interval::new(-1.0, 1.0), y: Interval::new(-1.0, 1.0), z: Interval::new(-11.0, -9.0) };

    let close_obj = Arc::new(MockHittable::new(close_box, Some(Point3::new(0.0, 0.0, -2.0))));
    let far_obj = Arc::new(MockHittable::new(far_box, Some(Point3::new(0.0, 0.0, -10.0))));

    // Los metemos en el árbol BVH
    let objects: Vec<Arc<dyn Hittable>> = vec![close_obj, far_obj];
    let bvh_root = BvhNode::new(objects);

    // Lanzamos un rayo desde el origen hacia el fondo (-Z). Debería cruzar ambos objetos.
    let ray = Ray {
        origin: Point3::new(0.0, 0.0, 0.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
    };

    let hit_result = bvh_root.hit(&ray, Interval::new(0.001, f32::INFINITY));
    
    assert!(hit_result.is_some(), "El BVH debió reportar un impacto");
    let record = hit_result.unwrap();
    
    // 🟢 PRUEBA CLAVE: El BVH debe retornar el impacto del objeto MÁS CERCANO (Z = -2.0)
    // porque la lógica del nodo restringe dinámicamente el `current_max` al evaluar el segundo hijo.
    assert_eq!(record.p.z, -2.0, "El árbol BVH devolvió un impacto erróneo ocultando el objeto más cercano");
}

#[test]
fn test_bvh_node_miss_optimization() {
    // Un objeto ubicado muy lejos a la derecha en el espacio
    let isolated_box = Aabb {
        x: Interval::new(100.0, 102.0),
        y: Interval::new(100.0, 102.0),
        z: Interval::new(100.0, 102.0),
    };
    let obj = Arc::new(MockHittable::new(isolated_box, Some(Point3::new(101.0, 101.0, 101.0))));
    let bvh_root = BvhNode::new(vec![obj]);

    // Lanzamos un rayo en la dirección opuesta (al vacío de la escena)
    let ray_into_void = Ray {
        origin: Point3::new(0.0, 0.0, 0.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
    };

    let hit_result = bvh_root.hit(&ray_into_void, Interval::new(0.001, f32::INFINITY));
    
    // 🟢 PRUEBA DE RENDIMIENTO INTELECTUAL: El test verifica que el método hit falle inmediatamente
    // en la línea 1 del BvhNode de forma instantánea al no cruzar la caja raíz, abortando la recursión.
    assert!(hit_result.is_none(), "El BVH debió descartar la rama y devolver None inmediatamente");
}
