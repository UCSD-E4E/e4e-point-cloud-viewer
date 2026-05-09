//! Compute the recenter+autoscale transform for a freshly-imported cloud.
//!
//! The orientation half (snap-to-axis, invert-Z) lives in sibling functions
//! and is composed onto whatever `auto_normalize` returns.

use e4epc_formats::Normalization;
use serde::{Deserialize, Serialize};

use crate::Summary;

/// Which axis of the source cloud should point "up" (viewer +Z) after normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpAxis {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

#[must_use]
pub fn with_up_axis(n: Normalization, up: UpAxis) -> Normalization {
    Normalization {
        rotation: rotation_for(up),
        ..n
    }
}

#[must_use]
pub fn with_z_inverted(mut n: Normalization) -> Normalization {
    for v in &mut n.rotation[2] {
        *v = -*v;
    }
    n
}

/// The axis-aligned bbox of the source cloud after normalization is applied.
///
/// Computed by transforming all 8 corners and taking the component-wise
/// min/max — necessary because rotation can rotate corners outside the
/// naive (apply min, apply max) box.
#[must_use]
pub fn transformed_bbox(summary: &Summary, n: &Normalization) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for cx in [summary.bbox_min[0], summary.bbox_max[0]] {
        for cy in [summary.bbox_min[1], summary.bbox_max[1]] {
            for cz in [summary.bbox_min[2], summary.bbox_max[2]] {
                let p = n.apply([cx, cy, cz]);
                for i in 0..3 {
                    min[i] = min[i].min(p[i]);
                    max[i] = max[i].max(p[i]);
                }
            }
        }
    }
    (min, max)
}

fn rotation_for(up: UpAxis) -> [[f64; 3]; 3] {
    match up {
        UpAxis::PosZ => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        UpAxis::NegZ => [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]],
        UpAxis::PosY => [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
        UpAxis::NegY => [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]],
        UpAxis::PosX => [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
        UpAxis::NegX => [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]],
    }
}

#[must_use]
pub fn auto_normalize(summary: &Summary, target_longest_extent: f64) -> Normalization {
    let mid = |i: usize| 0.5 * (summary.bbox_min[i] + summary.bbox_max[i]);
    let extent = |i: usize| summary.bbox_max[i] - summary.bbox_min[i];
    let longest = extent(0).max(extent(1)).max(extent(2));
    let scale = if longest > 0.0 {
        target_longest_extent / longest
    } else {
        1.0
    };
    Normalization {
        translation: [-mid(0), -mid(1), -mid(2)],
        scale,
        ..Normalization::identity()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn fake_summary(min: [f64; 3], max: [f64; 3]) -> Summary {
        Summary {
            point_count: 1,
            bbox_min: min,
            bbox_max: max,
            has_color: false,
        }
    }

    fn assert_close(a: [f64; 3], b: [f64; 3], eps: f64) {
        for i in 0..3 {
            assert!(
                (a[i] - b[i]).abs() < eps,
                "axis {i}: {} vs {} (eps {eps})",
                a[i],
                b[i],
            );
        }
    }

    #[test]
    fn recenters_bbox_midpoint_to_origin() {
        let s = fake_summary([100.0, 200.0, 300.0], [110.0, 220.0, 330.0]);
        let n = auto_normalize(&s, 5.0);
        let center = [105.0, 210.0, 315.0];
        assert_close(n.apply(center), [0.0, 0.0, 0.0], 1e-9);
    }

    #[test]
    fn scales_longest_axis_to_target_extent() {
        let s = fake_summary([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);
        let n = auto_normalize(&s, 5.0);
        let lo = n.apply([0.0, 0.0, 0.0]);
        let hi = n.apply([10.0, 20.0, 30.0]);
        let extent_z = (hi[2] - lo[2]).abs();
        assert!(
            (extent_z - 5.0).abs() < 1e-9,
            "expected longest extent 5.0, got {extent_z}",
        );
    }

    #[test]
    fn shorter_axes_scale_proportionally() {
        // Longest axis is X = 100. After scaling to target=10, scale = 0.1.
        // Y extent 50 should become 5, Z extent 25 should become 2.5.
        let s = fake_summary([0.0, 0.0, 0.0], [100.0, 50.0, 25.0]);
        let n = auto_normalize(&s, 10.0);
        let lo = n.apply([0.0, 0.0, 0.0]);
        let hi = n.apply([100.0, 50.0, 25.0]);
        assert!(((hi[0] - lo[0]).abs() - 10.0).abs() < 1e-9);
        assert!(((hi[1] - lo[1]).abs() - 5.0).abs() < 1e-9);
        assert!(((hi[2] - lo[2]).abs() - 2.5).abs() < 1e-9);
    }

    #[test]
    fn up_axis_maps_chosen_source_axis_to_viewer_z() {
        let cases = [
            (UpAxis::PosX, [1.0, 0.0, 0.0]),
            (UpAxis::NegX, [-1.0, 0.0, 0.0]),
            (UpAxis::PosY, [0.0, 1.0, 0.0]),
            (UpAxis::NegY, [0.0, -1.0, 0.0]),
            (UpAxis::PosZ, [0.0, 0.0, 1.0]),
            (UpAxis::NegZ, [0.0, 0.0, -1.0]),
        ];
        for (axis, source_vec) in cases {
            let n = with_up_axis(Normalization::identity(), axis);
            let out = n.apply(source_vec);
            assert_close(out, [0.0, 0.0, 1.0], 1e-12);
        }
    }

    #[test]
    fn pos_z_up_axis_is_identity_rotation() {
        // The default case shouldn't disturb a cloud that's already +Z up.
        let n = with_up_axis(Normalization::identity(), UpAxis::PosZ);
        assert_close(n.apply([1.0, 0.0, 0.0]), [1.0, 0.0, 0.0], 1e-12);
        assert_close(n.apply([0.0, 1.0, 0.0]), [0.0, 1.0, 0.0], 1e-12);
        assert_close(n.apply([0.0, 0.0, 1.0]), [0.0, 0.0, 1.0], 1e-12);
    }

    #[test]
    fn up_axis_preserves_orthonormal_basis() {
        // Whatever rotation we pick, the three viewer axes must remain
        // unit-length and pairwise orthogonal — otherwise we're shearing.
        for axis in [
            UpAxis::PosX,
            UpAxis::NegX,
            UpAxis::PosY,
            UpAxis::NegY,
            UpAxis::PosZ,
            UpAxis::NegZ,
        ] {
            let n = with_up_axis(Normalization::identity(), axis);
            let cols = [
                n.apply([1.0, 0.0, 0.0]),
                n.apply([0.0, 1.0, 0.0]),
                n.apply([0.0, 0.0, 1.0]),
            ];
            for c in cols {
                let len2 = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
                assert!((len2 - 1.0).abs() < 1e-12, "axis {axis:?}: len2 {len2}");
            }
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            assert!(dot(cols[0], cols[1]).abs() < 1e-12);
            assert!(dot(cols[0], cols[2]).abs() < 1e-12);
            assert!(dot(cols[1], cols[2]).abs() < 1e-12);
        }
    }

    #[test]
    fn transformed_bbox_with_identity_is_source_bbox() {
        let s = fake_summary([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);
        let (min, max) = transformed_bbox(&s, &Normalization::identity());
        assert_close(min, [0.0, 0.0, 0.0], 1e-12);
        assert_close(max, [10.0, 20.0, 30.0], 1e-12);
    }

    #[test]
    fn transformed_bbox_uses_all_eight_corners_after_rotation() {
        // PosY rotation maps source (x,y,z) -> (x,-z,y).
        // Source bbox [-1,-2,-3]..[1,2,3]; after rotation, axis-aligned bbox
        // must come from min/max over all 8 corners → x: [-1,1], y: [-3,3], z: [-2,2].
        let s = fake_summary([-1.0, -2.0, -3.0], [1.0, 2.0, 3.0]);
        let n = with_up_axis(Normalization::identity(), UpAxis::PosY);
        let (min, max) = transformed_bbox(&s, &n);
        assert_close(min, [-1.0, -3.0, -2.0], 1e-12);
        assert_close(max, [1.0, 3.0, 2.0], 1e-12);
    }

    #[test]
    fn invert_z_flips_viewer_z_only() {
        let n = with_z_inverted(Normalization::identity());
        assert_eq!(n.apply([5.0, 7.0, 9.0]), [5.0, 7.0, -9.0]);
    }

    #[test]
    fn invert_z_composes_after_up_axis() {
        // PosY sends source +Y to viewer +Z; invert-Z then sends it to viewer -Z.
        let n = with_z_inverted(with_up_axis(Normalization::identity(), UpAxis::PosY));
        assert_close(n.apply([0.0, 1.0, 0.0]), [0.0, 0.0, -1.0], 1e-12);
    }

    #[test]
    fn invert_twice_is_a_noop() {
        let n = with_z_inverted(with_z_inverted(Normalization::identity()));
        assert_eq!(n.apply([1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn zero_extent_cloud_falls_back_to_unit_scale() {
        // A degenerate single-point cloud has zero extent. Don't divide by zero.
        let s = fake_summary([42.0, 42.0, 42.0], [42.0, 42.0, 42.0]);
        let n = auto_normalize(&s, 5.0);
        // We don't care what scale is, just that it's finite and the recenter still works.
        assert!(n.scale.is_finite());
        assert_close(n.apply([42.0, 42.0, 42.0]), [0.0, 0.0, 0.0], 1e-9);
    }
}
