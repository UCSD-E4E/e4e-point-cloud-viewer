//! Surface normal estimation via PCA on a point's k-nearest neighbors.
//!
//! Build the 3×3 covariance matrix of the neighbor positions about their
//! centroid; the eigenvector of the smallest eigenvalue is the direction
//! of least variance — i.e. perpendicular to the local surface.

use nalgebra::Matrix3;

#[must_use]
pub fn estimate_normal(neighbors: &[[f64; 3]]) -> [f64; 3] {
    let n = neighbors.len() as f64;
    let centroid = [
        neighbors.iter().map(|p| p[0]).sum::<f64>() / n,
        neighbors.iter().map(|p| p[1]).sum::<f64>() / n,
        neighbors.iter().map(|p| p[2]).sum::<f64>() / n,
    ];

    let mut cov = Matrix3::<f64>::zeros();
    for p in neighbors {
        let d = [p[0] - centroid[0], p[1] - centroid[1], p[2] - centroid[2]];
        for i in 0..3 {
            for j in 0..3 {
                cov[(i, j)] += d[i] * d[j];
            }
        }
    }

    let eigen = cov.symmetric_eigen();
    let mut min_idx = 0;
    for i in 1..3 {
        if eigen.eigenvalues[i] < eigen.eigenvalues[min_idx] {
            min_idx = i;
        }
    }
    let v = eigen.eigenvectors.column(min_idx);
    [v[0], v[1], v[2]]
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn assert_unit_along(actual: [f64; 3], axis: usize) {
        let len = (actual[0].powi(2) + actual[1].powi(2) + actual[2].powi(2)).sqrt();
        assert!(
            (len - 1.0).abs() < 1e-9,
            "not unit length: {actual:?} (|n|={len})"
        );
        for i in 0..3 {
            if i == axis {
                assert!(
                    (actual[i].abs() - 1.0).abs() < 1e-9,
                    "expected ±1 on axis {i}, got {}",
                    actual[i],
                );
            } else {
                assert!(
                    actual[i].abs() < 1e-9,
                    "expected 0 on axis {i}, got {}",
                    actual[i],
                );
            }
        }
    }

    #[test]
    fn normal_of_xy_plane_cluster_is_z_axis() {
        let neighbors = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
        ];
        assert_unit_along(estimate_normal(&neighbors), 2);
    }

    #[test]
    fn normal_of_yz_plane_cluster_is_x_axis() {
        let neighbors = [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];
        assert_unit_along(estimate_normal(&neighbors), 0);
    }

    #[test]
    fn normal_of_xz_plane_cluster_is_y_axis() {
        let neighbors = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];
        assert_unit_along(estimate_normal(&neighbors), 1);
    }

    #[test]
    fn normal_is_unit_length_for_arbitrary_neighbors() {
        let neighbors = [
            [0.0, 0.0, 0.0],
            [1.5, 0.7, -0.3],
            [-0.4, 1.1, 0.2],
            [0.8, -0.5, 1.3],
            [-1.2, 0.3, -0.6],
        ];
        let n = estimate_normal(&neighbors);
        let len = (n[0].powi(2) + n[1].powi(2) + n[2].powi(2)).sqrt();
        assert!((len - 1.0).abs() < 1e-9, "|n| = {len}");
    }

    #[test]
    fn normal_is_perpendicular_to_a_tilted_plane() {
        // Plane with normal (1, 1, 1) / sqrt(3): points on this plane
        // satisfy x + y + z = 0. Build a few such points.
        let neighbors = [
            [0.0, 0.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 0.0, -1.0],
            [0.0, 1.0, -1.0],
            [-1.0, 1.0, 0.0],
            [-1.0, 0.0, 1.0],
        ];
        let n = estimate_normal(&neighbors);
        let inv_sqrt3 = 1.0 / 3.0_f64.sqrt();
        // Sign ambiguous, so check |n · expected| ≈ 1
        let dot = n[0] * inv_sqrt3 + n[1] * inv_sqrt3 + n[2] * inv_sqrt3;
        assert!((dot.abs() - 1.0).abs() < 1e-9, "n={n:?}, dot={dot}");
    }
}
