//! Cluster map — pre-computed k-means clustering of feature vectors.

/// Pre-computed clustering of feature vectors for fast semantic grouping.
pub struct ClusterMap {
    /// Cluster centroids.
    centroids: Vec<Vec<f32>>,
    /// cluster_index -> sorted Vec of node IDs in that cluster.
    assignments: Vec<Vec<u64>>,
    /// Feature vector dimension.
    dimension: usize,
}

impl ClusterMap {
    /// Create a new, empty cluster map.
    pub fn new(dimension: usize) -> Self {
        Self {
            centroids: Vec::new(),
            assignments: Vec::new(),
            dimension,
        }
    }

    /// Run k-means clustering on all feature vectors in the graph.
    /// k = min(sqrt(node_count), 256). Skips if node_count < 4.
    pub fn build(&mut self, nodes: &[(u64, &[f32])], max_iterations: usize) {
        // Filter out zero vectors
        let non_zero: Vec<(u64, &[f32])> = nodes
            .iter()
            .filter(|(_, v)| v.iter().any(|&x| x != 0.0))
            .copied()
            .collect();

        if non_zero.len() < 4 {
            self.centroids.clear();
            self.assignments.clear();
            return;
        }

        let k = ((non_zero.len() as f64).sqrt().ceil() as usize).min(256);

        // Initialize centroids: pick k evenly-spaced nodes
        let step = non_zero.len() / k;
        self.centroids = (0..k)
            .map(|i| {
                let idx = (i * step).min(non_zero.len() - 1);
                non_zero[idx].1.to_vec()
            })
            .collect();

        self.assignments = vec![Vec::new(); k];

        for _ in 0..max_iterations {
            // Clear assignments
            for a in &mut self.assignments {
                a.clear();
            }

            // Assign each node to nearest centroid
            for &(id, vec) in &non_zero {
                let nearest = self.find_nearest_centroid(vec);
                self.assignments[nearest].push(id);
            }

            // Update centroids
            let mut changed = false;
            for (ci, cluster_ids) in self.assignments.iter().enumerate() {
                if cluster_ids.is_empty() {
                    continue;
                }
                let mut new_centroid = vec![0.0f32; self.dimension];
                let count = cluster_ids.len() as f32;
                for &node_id in cluster_ids {
                    if let Some((_, vec)) = non_zero.iter().find(|(id, _)| *id == node_id) {
                        for (j, &val) in vec.iter().enumerate() {
                            new_centroid[j] += val;
                        }
                    }
                }
                for val in &mut new_centroid {
                    *val /= count;
                }
                if new_centroid != self.centroids[ci] {
                    changed = true;
                    self.centroids[ci] = new_centroid;
                }
            }

            if !changed {
                break;
            }
        }

        // Sort assignments
        for a in &mut self.assignments {
            a.sort_unstable();
        }
    }

    fn find_nearest_centroid(&self, vec: &[f32]) -> usize {
        let mut best = 0;
        let mut best_sim = f32::NEG_INFINITY;
        for (i, centroid) in self.centroids.iter().enumerate() {
            let sim = cosine_similarity(vec, centroid);
            if sim > best_sim {
                best_sim = sim;
                best = i;
            }
        }
        best
    }

    /// Find the nearest cluster for a query vector.
    pub fn nearest_cluster(&self, query: &[f32]) -> Option<usize> {
        if self.centroids.is_empty() {
            return None;
        }
        Some(self.find_nearest_centroid(query))
    }

    /// Get all node IDs in a specific cluster.
    pub fn get_cluster(&self, cluster_index: usize) -> &[u64] {
        self.assignments
            .get(cluster_index)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get the centroid for a cluster.
    pub fn centroid(&self, cluster_index: usize) -> Option<&[f32]> {
        self.centroids.get(cluster_index).map(|v| v.as_slice())
    }

    /// Number of clusters.
    pub fn cluster_count(&self) -> usize {
        self.centroids.len()
    }

    /// Assign a new node to the nearest cluster without rebuilding.
    pub fn assign_node(&mut self, node_id: u64, feature_vec: &[f32]) {
        if self.centroids.is_empty() {
            return;
        }
        if feature_vec.iter().all(|&x| x == 0.0) {
            return;
        }
        let nearest = self.find_nearest_centroid(feature_vec);
        let list = &mut self.assignments[nearest];
        let pos = list.binary_search(&node_id).unwrap_or_else(|p| p);
        list.insert(pos, node_id);
    }

    /// Clear the cluster map.
    pub fn clear(&mut self) {
        self.centroids.clear();
        self.assignments.clear();
    }

    /// Whether the cluster map is empty.
    pub fn is_empty(&self) -> bool {
        self.centroids.is_empty()
    }

    /// Get the dimension.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Get centroids (for serialization).
    pub fn centroids(&self) -> &[Vec<f32>] {
        &self.centroids
    }

    /// Get assignments (for serialization).
    pub fn assignments(&self) -> &[Vec<u64>] {
        &self.assignments
    }
}

/// Compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len().min(b.len()) {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

impl Default for ClusterMap {
    fn default() -> Self {
        Self::new(128)
    }
}
