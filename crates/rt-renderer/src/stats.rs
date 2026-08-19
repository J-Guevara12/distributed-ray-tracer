use rt_scene::TraversalStats;

#[derive(Default, Clone)]
pub struct RayStats {
    pub rays: u64,
    pub traversal: TraversalStats,
}

pub struct RenderStats {
    pub rays: u64,
    pub traversal: TraversalStats,
    pub tile_ms: Vec<f64>,
}

impl RenderStats {
    /// Nodos de BVH cuyo AABB se testeó, por segmento de rayo. Es la métrica
    /// de calidad del árbol: separa "el árbol mejoró" de "cada nodo cuesta
    /// menos", que en el reloj se ven igual.
    pub fn nodes_per_ray(&self) -> f64 {
        if self.rays == 0 {
            0.0
        } else {
            self.traversal.node_visits as f64 / self.rays as f64
        }
    }

    pub fn prims_per_ray(&self) -> f64 {
        if self.rays == 0 {
            0.0
        } else {
            self.traversal.prim_tests as f64 / self.rays as f64
        }
    }
}
