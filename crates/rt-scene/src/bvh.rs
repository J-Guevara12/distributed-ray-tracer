use rt_core::dto::ScenePayload;

use crate::geometry::{Primitive, primitives_from_scene};
use crate::{HitRecord, Hittable, Interval, Point3, Ray, Vec3};

/// Caja delimitadora alineada a los ejes (Axis-Aligned Bounding Box).
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min: Point3,
    pub max: Point3,
}

impl Aabb {
    /// Caja vacía: la unión con cualquier otra caja devuelve la otra caja.
    pub const EMPTY: Self = Self {
        min: Point3::INFINITY,
        max: Point3::NEG_INFINITY,
    };

    pub fn new(min: Point3, max: Point3) -> Self {
        Self { min, max }
    }

    /// Construye la caja mínima que contiene todos los puntos dados.
    pub fn from_points(points: &[Point3]) -> Self {
        points.iter().fold(Self::EMPTY, |acc, p| Self {
            min: acc.min.min(*p),
            max: acc.max.max(*p),
        })
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Expande cada eje cuyo grosor sea menor a `delta` (evita cajas degeneradas
    /// en geometría plana como los Quads).
    pub fn pad(self, delta: f32) -> Self {
        let thickness = self.max - self.min;
        let pad = Vec3::select(thickness.cmplt(Vec3::splat(delta)), Vec3::splat(delta * 0.5), Vec3::ZERO);
        Self {
            min: self.min - pad,
            max: self.max + pad,
        }
    }

    pub fn centroid(&self) -> Point3 {
        (self.min + self.max) * 0.5
    }

    pub fn longest_axis(&self) -> usize {
        let extent = self.max - self.min;
        if extent.x >= extent.y && extent.x >= extent.z {
            0
        } else if extent.y >= extent.z {
            1
        } else {
            2
        }
    }

    /// Test de intersección rayo-caja por el método de los "slabs".
    /// `inv_dir` es 1/direction, precalculado una vez por rayo.
    #[inline(always)]
    pub fn hit(&self, origin: Point3, inv_dir: Vec3, ray_t: Interval) -> bool {
        let t1 = (self.min - origin) * inv_dir;
        let t2 = (self.max - origin) * inv_dir;

        let t_enter = t1.min(t2).max_element().max(ray_t.min);
        let t_exit = t1.max(t2).min_element().min(ray_t.max);

        t_enter <= t_exit
    }
}

#[derive(Debug, Clone, Copy)]
struct BvhNode {
    bbox: Aabb,
    /// Si `count > 0` (hoja): índice del primer objeto.
    /// Si `count == 0` (interior): índice del hijo derecho (el izquierdo es siempre el nodo siguiente).
    right_or_first: u32,
    count: u32,
    /// Eje de partición de los nodos interiores, para visitar primero el hijo más cercano.
    axis: u32,
}

/// Jerarquía de volúmenes delimitadores (BVH) aplanada en un arreglo contiguo.
/// Reduce el costo de intersección de O(n) a O(log n) objetos por rayo.
/// Las primitivas se guardan por valor en un arreglo plano: el test de hoja
/// no persigue punteros ni pasa por una vtable.
pub struct Bvh<T: Hittable = Primitive> {
    nodes: Vec<BvhNode>,
    primitives: Vec<T>,
}

const MAX_LEAF_SIZE: usize = 2;

impl<T: Hittable> Bvh<T> {
    pub fn new(primitives: Vec<T>) -> Self {
        if primitives.is_empty() {
            return Self {
                nodes: Vec::new(),
                primitives,
            };
        }

        let mut entries: Vec<(T, Aabb, Point3)> = primitives
            .into_iter()
            .map(|primitive| {
                let bbox = primitive.bounding_box();
                let centroid = bbox.centroid();
                (primitive, bbox, centroid)
            })
            .collect();

        let mut nodes = Vec::with_capacity(2 * entries.len());
        Self::build(&mut entries, 0, &mut nodes);

        let primitives = entries.into_iter().map(|(primitive, _, _)| primitive).collect();

        Self { nodes, primitives }
    }

    /// Construye el subárbol para `entries` (cuyo primer elemento ocupa la posición
    /// global `base`) y devuelve el índice del nodo creado.
    fn build(
        entries: &mut [(T, Aabb, Point3)],
        base: u32,
        nodes: &mut Vec<BvhNode>,
    ) -> u32 {
        let index = nodes.len() as u32;
        let bbox = entries
            .iter()
            .fold(Aabb::EMPTY, |acc, (_, b, _)| acc.union(*b));

        if entries.len() <= MAX_LEAF_SIZE {
            nodes.push(BvhNode {
                bbox,
                right_or_first: base,
                count: entries.len() as u32,
                axis: 0,
            });
            return index;
        }

        // Partición por la mediana de los centroides en el eje más largo.
        let centroid_bounds = entries
            .iter()
            .fold(Aabb::EMPTY, |acc, (_, _, c)| Aabb {
                min: acc.min.min(*c),
                max: acc.max.max(*c),
            });
        let axis = centroid_bounds.longest_axis();

        let mid = entries.len() / 2;
        entries.select_nth_unstable_by(mid, |a, b| {
            a.2[axis].partial_cmp(&b.2[axis]).unwrap_or(std::cmp::Ordering::Equal)
        });

        nodes.push(BvhNode {
            bbox,
            right_or_first: 0, // se completa después de construir los hijos
            count: 0,
            axis: axis as u32,
        });

        let (left, right) = entries.split_at_mut(mid);
        Self::build(left, base, nodes);
        let right_index = Self::build(right, base + mid as u32, nodes);

        nodes[index as usize].right_or_first = right_index;

        index
    }
}

impl<T: Hittable> Hittable for Bvh<T> {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        if self.nodes.is_empty() {
            return None;
        }

        let inv_dir = ray.direction.recip();
        let neg_dir = [
            ray.direction.x < 0.0,
            ray.direction.y < 0.0,
            ray.direction.z < 0.0,
        ];

        let mut closest_so_far = ray_t.max;
        let mut hit_anything: Option<HitRecord> = None;

        let mut stack = [0u32; 64];
        let mut stack_len = 1usize;

        while stack_len > 0 {
            stack_len -= 1;
            let node_index = stack[stack_len];
            let node = &self.nodes[node_index as usize];

            let current_interval = Interval::new(ray_t.min, closest_so_far);
            if !node.bbox.hit(ray.origin, inv_dir, current_interval) {
                continue;
            }

            if node.count > 0 {
                let first = node.right_or_first as usize;
                for primitive in &self.primitives[first..first + node.count as usize] {
                    if let Some(rec) = primitive.hit(ray, Interval::new(ray_t.min, closest_so_far)) {
                        closest_so_far = rec.t;
                        hit_anything = Some(rec);
                    }
                }
            } else {
                // Visitamos primero el hijo más cercano según el sentido del rayo
                // (el lejano se apila debajo).
                let near = node_index + 1;
                let far = node.right_or_first;
                let (first_visit, second_visit) = if neg_dir[node.axis as usize] {
                    (far, near)
                } else {
                    (near, far)
                };
                stack[stack_len] = second_visit;
                stack[stack_len + 1] = first_visit;
                stack_len += 2;
            }
        }

        hit_anything
    }

    fn bounding_box(&self) -> Aabb {
        self.nodes.first().map(|n| n.bbox).unwrap_or(Aabb::EMPTY)
    }
}

impl From<&ScenePayload> for Bvh<Primitive> {
    fn from(payload: &ScenePayload) -> Self {
        Self::new(primitives_from_scene(payload))
    }
}
