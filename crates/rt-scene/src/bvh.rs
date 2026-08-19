use rt_core::{Interval, Ray, Vec3};

use crate::{Aabb, HitRecord, Hittable, primitive::Primitive};

/// Máximo de primitivas por hoja. Con 1 el árbol tiene el doble de nodos y el
/// recorrido paga más saltos de los que ahorra.
const MAX_LEAF_PRIMITIVES: usize = 4;

/// Profundidad máxima de la pila de recorrido. Un árbol balanceado de 10M de
/// primitivas con hojas de 4 llega a ~21 niveles.
const MAX_STACK: usize = 64;

/// Nodo del BVH en representación plana.
///
/// El hijo izquierdo es siempre `índice + 1`: al construir en orden
/// depth-first queda pegado al padre en memoria, así que no hay que guardar su
/// índice y suele estar ya en caché. `offset` apunta al hijo derecho en los
/// nodos internos y al primer primitivo en las hojas.
#[derive(Clone, Copy)]
struct FlatNode {
    bounds: Aabb,
    offset: u32,
    /// 0 = nodo interno. >0 = hoja con esa cantidad de primitivas.
    count: u16,
    axis: u8,
}

pub struct Bvh {
    nodes: Vec<FlatNode>,
    primitives: Vec<Primitive>,
    bounds: Aabb,
}

fn union(primitives: &[Primitive]) -> Aabb {
    let mut out = primitives[0].bounding_box();
    for primitive in &primitives[1..] {
        out = Aabb::surrounding_box(out, primitive.bounding_box());
    }
    out
}

fn longest_axis(primitives: &[Primitive]) -> usize {
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);

    for primitive in primitives {
        let b = primitive.bounding_box();
        let centroid = 0.5 * (b.min + b.max);
        low = low.min(centroid);
        high = high.max(centroid);
    }

    let extent = high - low;
    if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    }
}

impl Bvh {
    pub fn build(mut primitives: Vec<Primitive>) -> Self {
        if primitives.is_empty() {
            return Self {
                nodes: Vec::new(),
                primitives,
                bounds: Aabb::default(),
            };
        }

        let mut nodes = Vec::with_capacity(2 * primitives.len() / MAX_LEAF_PRIMITIVES + 1);
        build_recursive(&mut nodes, &mut primitives, 0);
        let bounds = nodes[0].bounds;

        Self {
            nodes,
            primitives,
            bounds,
        }
    }

    pub fn primitive_count(&self) -> usize {
        self.primitives.len()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// Emite los nodos en orden depth-first y devuelve el índice del que acaba de
/// escribir. `first` es el desplazamiento de este trozo dentro del array
/// completo de primitivas, que es lo que las hojas guardan en `offset`.
fn build_recursive(nodes: &mut Vec<FlatNode>, primitives: &mut [Primitive], first: usize) -> usize {
    let index = nodes.len();
    let bounds = union(primitives);

    nodes.push(FlatNode {
        bounds,
        offset: 0,
        count: 0,
        axis: 0,
    });

    if primitives.len() <= MAX_LEAF_PRIMITIVES {
        nodes[index] = FlatNode {
            bounds,
            offset: first as u32,
            count: primitives.len() as u16,
            axis: 0,
        };
        return index;
    }

    let axis = longest_axis(primitives);
    primitives.sort_by(|a, b| a.sort_key(axis).total_cmp(&b.sort_key(axis)));

    let mid = primitives.len() / 2;
    let (left, right) = primitives.split_at_mut(mid);

    build_recursive(nodes, left, first);
    let right_index = build_recursive(nodes, right, first + mid);

    nodes[index] = FlatNode {
        bounds,
        offset: right_index as u32,
        count: 0,
        axis: axis as u8,
    };
    index
}

impl Hittable for Bvh {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord> {
        if self.nodes.is_empty() {
            return None;
        }

        let mut stack = [0u32; MAX_STACK];
        let mut depth = 0usize;
        let mut current = 0u32;

        let mut closest = ray_t.max;
        let mut best: Option<HitRecord> = None;

        loop {
            let node = &self.nodes[current as usize];

            if node.bounds.hit(ray, Interval::new(ray_t.min, closest)) {
                if node.count > 0 {
                    let start = node.offset as usize;
                    for primitive in &self.primitives[start..start + node.count as usize] {
                        if let Some(rec) = primitive.hit(ray, Interval::new(ray_t.min, closest)) {
                            closest = rec.t;
                            best = Some(rec);
                        }
                    }
                } else {
                    // Front-to-back: visitar primero el lado hacia el que
                    // apunta el rayo hace que `closest` se recorte antes y el
                    // otro hijo falle su AABB más seguido.
                    let (near, far) = if ray.direction[node.axis as usize] < 0.0 {
                        (node.offset, current + 1)
                    } else {
                        (current + 1, node.offset)
                    };

                    debug_assert!(depth < MAX_STACK, "pila de recorrido desbordada");
                    stack[depth] = far;
                    depth += 1;
                    current = near;
                    continue;
                }
            }

            if depth == 0 {
                break;
            }
            depth -= 1;
            current = stack[depth];
        }

        best
    }

    fn bounding_box(&self) -> Aabb {
        self.bounds
    }
}
