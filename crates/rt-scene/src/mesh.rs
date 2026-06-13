use std::path::Path;
use std::sync::Arc;

use crate::bvh::Bvh;
use crate::{Aabb, HitRecord, Hittable, Interval, Material, Point3, Ray, Vec3};

/// Triángulo con atributos por vértice (normales, UVs) y tangente precomputada.
#[derive(Clone)]
pub struct Triangle {
    v0: Point3,
    e1: Vec3, // v1 - v0
    e2: Vec3, // v2 - v0
    n0: Vec3,
    n1: Vec3,
    n2: Vec3,
    uv0: [f32; 2],
    uv1: [f32; 2],
    uv2: [f32; 2],
    tangent: Vec3,
    pub material: Arc<dyn Material>,
}

impl Triangle {
    /// Triángulo plano: normal de cara y UVs canónicos (0,0) (1,0) (0,1).
    pub fn new(v0: Point3, v1: Point3, v2: Point3, material: Arc<dyn Material>) -> Self {
        Self::with_attributes(v0, v1, v2, None, None, material)
    }

    pub fn with_attributes(
        v0: Point3,
        v1: Point3,
        v2: Point3,
        normals: Option<[Vec3; 3]>,
        uvs: Option<[[f32; 2]; 3]>,
        material: Arc<dyn Material>,
    ) -> Self {
        let e1 = v1 - v0;
        let e2 = v2 - v0;

        let face_normal = e1.cross(e2);
        let face_normal = if face_normal.length_squared() > 1e-12 {
            face_normal.normalize()
        } else {
            Vec3::Y // triángulo degenerado: normal arbitraria
        };

        let [n0, n1, n2] = normals.unwrap_or([face_normal; 3]);
        let [uv0, uv1, uv2] = uvs.unwrap_or([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);

        // Tangente a partir de los deltas de UV (dirección de u creciente)
        let duv1 = [uv1[0] - uv0[0], uv1[1] - uv0[1]];
        let duv2 = [uv2[0] - uv0[0], uv2[1] - uv0[1]];
        let det = duv1[0] * duv2[1] - duv1[1] * duv2[0];

        let tangent = if det.abs() > 1e-12 {
            ((e1 * duv2[1] - e2 * duv1[1]) / det).normalize_or_zero()
        } else {
            e1.normalize_or_zero()
        };

        Self {
            v0,
            e1,
            e2,
            n0,
            n1,
            n2,
            uv0,
            uv1,
            uv2,
            tangent,
            material,
        }
    }
}

impl Hittable for Triangle {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        // Möller-Trumbore (válido para direcciones no unitarias)
        let pvec = ray.direction.cross(self.e2);
        let det = self.e1.dot(pvec);

        if det.abs() < 1e-9 {
            return None; // Rayo paralelo al plano del triángulo
        }

        let inv_det = 1.0 / det;
        let tvec = ray.origin - self.v0;

        let u = tvec.dot(pvec) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let qvec = tvec.cross(self.e1);
        let v = ray.direction.dot(qvec) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = self.e2.dot(qvec) * inv_det;
        if !ray_t.surrounds(t) {
            return None;
        }

        let w = 1.0 - u - v;

        // Interpolación baricéntrica de atributos
        let normal = (self.n0 * w + self.n1 * u + self.n2 * v).normalize();
        let tex_u = self.uv0[0] * w + self.uv1[0] * u + self.uv2[0] * v;
        let tex_v = self.uv0[1] * w + self.uv1[1] * u + self.uv2[1] * v;

        Some(HitRecord::with_uv(
            ray,
            t,
            normal,
            ray.at(t),
            self.material.as_ref(),
            tex_u,
            tex_v,
            self.tangent,
        ))
    }

    fn bounding_box(&self) -> Aabb {
        Aabb::from_points(&[self.v0, self.v0 + self.e1, self.v0 + self.e2]).pad(1e-4)
    }
}

/// Malla de triángulos con su propio BVH interno (segundo nivel de la jerarquía).
pub struct Mesh {
    bvh: Bvh<Triangle>,
    pub n_triangles: usize,
}

impl Mesh {
    pub fn new(triangles: Vec<Triangle>) -> Self {
        let n_triangles = triangles.len();
        Self {
            bvh: Bvh::new(triangles),
            n_triangles,
        }
    }

    /// Carga un OBJ (triangulado por tobj) aplicando un material único a toda la malla.
    /// `scale` y `translate` permiten ubicar la malla sin necesidad de una instancia.
    pub fn load_obj<P: AsRef<Path>>(
        path: P,
        material: Arc<dyn Material>,
        scale: f32,
        translate: Vec3,
    ) -> Result<Self, String> {
        let (models, _) = tobj::load_obj(
            path.as_ref(),
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
        )
        .map_err(|e| format!("Error cargando OBJ {:?}: {}", path.as_ref(), e))?;

        let mut triangles = Vec::new();

        for model in &models {
            let mesh = &model.mesh;

            let position = |i: usize| -> Point3 {
                Point3::new(
                    mesh.positions[3 * i],
                    mesh.positions[3 * i + 1],
                    mesh.positions[3 * i + 2],
                ) * scale
                    + translate
            };

            let normal = |i: usize| -> Option<Vec3> {
                if mesh.normals.is_empty() {
                    None
                } else {
                    Some(Vec3::new(
                        mesh.normals[3 * i],
                        mesh.normals[3 * i + 1],
                        mesh.normals[3 * i + 2],
                    ))
                }
            };

            let texcoord = |i: usize| -> Option<[f32; 2]> {
                if mesh.texcoords.is_empty() {
                    None
                } else {
                    Some([mesh.texcoords[2 * i], mesh.texcoords[2 * i + 1]])
                }
            };

            for face in mesh.indices.chunks_exact(3) {
                let (i0, i1, i2) = (face[0] as usize, face[1] as usize, face[2] as usize);

                let normals = match (normal(i0), normal(i1), normal(i2)) {
                    (Some(a), Some(b), Some(c)) => Some([a, b, c]),
                    _ => None,
                };
                let uvs = match (texcoord(i0), texcoord(i1), texcoord(i2)) {
                    (Some(a), Some(b), Some(c)) => Some([a, b, c]),
                    _ => None,
                };

                triangles.push(Triangle::with_attributes(
                    position(i0),
                    position(i1),
                    position(i2),
                    normals,
                    uvs,
                    Arc::clone(&material),
                ));
            }
        }

        if triangles.is_empty() {
            return Err(format!("El OBJ {:?} no contiene triángulos", path.as_ref()));
        }

        Ok(Self::new(triangles))
    }
}

impl Hittable for Mesh {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        self.bvh.hit(ray, ray_t)
    }

    fn bounding_box(&self) -> Aabb {
        self.bvh.bounding_box()
    }
}
