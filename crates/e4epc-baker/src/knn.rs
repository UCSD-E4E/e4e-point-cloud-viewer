//! k-nearest-neighbor lookup for splat baking.
//!
//! Thin wrapper over `kiddo` so that the rest of the baker can speak in our
//! own types and so we can swap implementations later without touching
//! callers (e.g. an approximate-NN backend if exact gets too slow at 10M+).

use kiddo::{ImmutableKdTree, SquaredEuclidean};

/// Index over a fixed set of 3D positions, supporting k-nearest queries.
///
/// Uses kiddo's immutable variant — we build once per cloud and query many
/// times, so the balanced static layout pays for itself and avoids the
/// bucket-overflow that the mutable variant hits on LiDAR scan lines.
pub struct KnnIndex {
    tree: ImmutableKdTree<f64, 3>,
}

impl KnnIndex {
    #[must_use]
    pub fn build(positions: &[[f64; 3]]) -> Self {
        Self {
            tree: ImmutableKdTree::new_from_slice(positions),
        }
    }

    /// Returns the `k` indices closest to `query`, paired with their
    /// Euclidean distance from it, sorted nearest-first.
    #[must_use]
    pub fn nearest_k(&self, query: [f64; 3], k: usize) -> Vec<(usize, f64)> {
        let Some(k_nz) = std::num::NonZero::new(k) else {
            return Vec::new();
        };
        self.tree
            .nearest_n::<SquaredEuclidean>(&query, k_nz)
            .into_iter()
            .map(|n| (n.item as usize, n.distance.sqrt()))
            .collect()
    }
}

/// Splat radius for a point given its neighbor distances — the mean.
///
/// Caller is responsible for excluding the query point itself from
/// `distances` (otherwise the zero pulls the mean down).
#[must_use]
pub fn splat_radius(distances: &[f64]) -> f64 {
    if distances.is_empty() {
        return 0.0;
    }
    distances.iter().sum::<f64>() / distances.len() as f64
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn assert_close(a: f64, b: f64, eps: f64) {
        assert!((a - b).abs() < eps, "{a} vs {b} (eps {eps})");
    }

    #[test]
    fn finds_self_at_distance_zero_for_indexed_point() {
        let pts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let idx = KnnIndex::build(&pts);
        let nn = idx.nearest_k([0.0, 0.0, 0.0], 1);
        assert_eq!(nn.len(), 1);
        assert_eq!(nn[0].0, 0);
        assert_close(nn[0].1, 0.0, 1e-12);
    }

    #[test]
    fn returns_k_nearest_in_distance_order() {
        let pts = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
        ];
        let idx = KnnIndex::build(&pts);
        let nn = idx.nearest_k([0.0, 0.0, 0.0], 3);
        assert_eq!(nn.len(), 3);
        assert_eq!(nn[0].0, 0);
        assert_eq!(nn[1].0, 2);
        assert_eq!(nn[2].0, 3);
    }

    #[test]
    fn distances_are_euclidean_not_squared() {
        let pts = vec![[0.0, 0.0, 0.0], [3.0, 4.0, 0.0]];
        let idx = KnnIndex::build(&pts);
        let nn = idx.nearest_k([0.0, 0.0, 0.0], 2);
        // Distance from origin to [3,4,0] is sqrt(9+16) = 5
        assert_close(nn[1].1, 5.0, 1e-12);
    }

    #[test]
    fn radius_is_mean_of_distances() {
        assert_eq!(splat_radius(&[1.0, 2.0, 3.0, 4.0]), 2.5);
    }

    #[test]
    fn radius_with_single_distance_is_that_distance() {
        assert_eq!(splat_radius(&[3.7]), 3.7);
    }

    #[test]
    fn radius_of_empty_is_zero_not_nan() {
        assert_eq!(splat_radius(&[]), 0.0);
    }
}
