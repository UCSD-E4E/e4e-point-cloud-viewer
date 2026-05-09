//! End-to-end bake: raw points + normalization → `SplatCloud`.
//!
//! Bake order:
//! 1. Apply the normalization to every point's position.
//! 2. Build a kNN index over the normalized positions.
//! 3. For each point, take its `k` nearest neighbors → estimate the surface
//!    normal via PCA, and the splat radius from mean neighbor distance
//!    (excluding self).
//! 4. Carry per-point color through (default grey if the source has none).

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use e4epc_formats::{Normalization, Splat, SplatCloud};
use rayon::prelude::*;

use crate::knn::{KnnIndex, splat_radius};
use crate::normal::estimate_normal;
use crate::{BakerError, RawPoint, iter_points};

const DEFAULT_K: usize = 12;
const DEFAULT_COLOR: [u8; 4] = [200, 200, 200, 255];

/// Scale factor applied to the kNN-mean distance to get the splat radius.
/// 1.0 looks fluffy because mean-of-12-NN is wider than the typical local
/// spacing; 0.6 gives a tight surface with just enough overlap to avoid gaps.
/// The user can fine-tune at runtime via the size-multiplier UI slider.
const RADIUS_SCALE: f64 = 0.6;

/// Throttle ratio: emit progress at most this many times across the bake.
const PROGRESS_BUCKETS: usize = 100;

#[must_use]
pub fn bake_cloud_from_points(
    points: Vec<RawPoint>,
    normalization: &Normalization,
    k: usize,
) -> SplatCloud {
    bake_cloud_from_points_with_progress(points, normalization, k, |_, _| {})
}

#[must_use]
pub fn bake_cloud_from_points_with_progress<F>(
    points: Vec<RawPoint>,
    normalization: &Normalization,
    k: usize,
    on_progress: F,
) -> SplatCloud
where
    F: Fn(usize, usize) + Send + Sync,
{
    let positions: Vec<[f64; 3]> = points
        .iter()
        .map(|p| normalization.apply(p.position))
        .collect();

    let knn = KnnIndex::build(&positions);

    let total = points.len();
    let bucket = (total / PROGRESS_BUCKETS).max(1);
    let counter = AtomicUsize::new(0);

    let splats: Vec<Splat> = (0..total)
        .into_par_iter()
        .map(|i| {
            let pos = positions[i];
            let nn = knn.nearest_k(pos, k + 1);
            let neighbor_positions: Vec<[f64; 3]> =
                nn.iter().map(|(idx, _)| positions[*idx]).collect();
            let normal = estimate_normal(&neighbor_positions);
            let dists: Vec<f64> = nn
                .iter()
                .filter(|(idx, _)| *idx != i)
                .map(|(_, d)| *d)
                .collect();
            let radius = splat_radius(&dists) * RADIUS_SCALE;
            let splat = Splat {
                position: [pos[0] as f32, pos[1] as f32, pos[2] as f32],
                color: points[i].color.unwrap_or(DEFAULT_COLOR),
                normal: [normal[0] as f32, normal[1] as f32, normal[2] as f32],
                radius: radius as f32,
            };

            let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
            if done == total || done.is_multiple_of(bucket) {
                on_progress(done, total);
            }
            splat
        })
        .collect();

    let mut bbox_min = [f32::INFINITY; 3];
    let mut bbox_max = [f32::NEG_INFINITY; 3];
    for s in &splats {
        for i in 0..3 {
            bbox_min[i] = bbox_min[i].min(s.position[i]);
            bbox_max[i] = bbox_max[i].max(s.position[i]);
        }
    }

    SplatCloud {
        splats,
        bbox_min,
        bbox_max,
    }
}

pub fn bake_cloud<P: AsRef<Path>>(
    path: P,
    normalization: &Normalization,
) -> Result<SplatCloud, BakerError> {
    let raw: Result<Vec<RawPoint>, BakerError> = iter_points(path)?.collect();
    Ok(bake_cloud_from_points(raw?, normalization, DEFAULT_K))
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn raw(p: [f64; 3]) -> RawPoint {
        RawPoint {
            position: p,
            color: None,
        }
    }

    fn small_grid(n: usize, spacing: f64) -> Vec<RawPoint> {
        let mut v = Vec::with_capacity(n * n * n);
        for x in 0..n {
            for y in 0..n {
                for z in 0..n {
                    v.push(raw([
                        x as f64 * spacing,
                        y as f64 * spacing,
                        z as f64 * spacing,
                    ]));
                }
            }
        }
        v
    }

    #[test]
    fn produces_one_splat_per_input_point() {
        let pts = small_grid(5, 1.0);
        assert_eq!(pts.len(), 125);
        let cloud = bake_cloud_from_points(pts, &Normalization::identity(), 8);
        assert_eq!(cloud.splats.len(), 125);
    }

    #[test]
    fn identity_normalization_preserves_positions() {
        let pts = vec![
            raw([1.0, 2.0, 3.0]),
            raw([4.0, 5.0, 6.0]),
            raw([7.0, 8.0, 9.0]),
        ];
        let cloud = bake_cloud_from_points(pts, &Normalization::identity(), 2);
        assert_eq!(cloud.splats[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(cloud.splats[1].position, [4.0, 5.0, 6.0]);
        assert_eq!(cloud.splats[2].position, [7.0, 8.0, 9.0]);
    }

    #[test]
    fn applies_normalization_to_positions() {
        let pts = vec![raw([10.0, 20.0, 30.0]), raw([12.0, 22.0, 33.0])];
        let n = Normalization {
            translation: [-10.0, -20.0, -30.0],
            scale: 0.5,
            ..Normalization::identity()
        };
        let cloud = bake_cloud_from_points(pts, &n, 1);
        assert_eq!(cloud.splats[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(cloud.splats[1].position, [1.0, 1.0, 1.5]);
    }

    #[test]
    fn passes_color_through_when_present() {
        let pts = vec![
            RawPoint {
                position: [0.0; 3],
                color: Some([100, 150, 200, 255]),
            },
            RawPoint {
                position: [1.0, 0.0, 0.0],
                color: None,
            },
        ];
        let cloud = bake_cloud_from_points(pts, &Normalization::identity(), 1);
        assert_eq!(cloud.splats[0].color, [100, 150, 200, 255]);
        assert_eq!(cloud.splats[1].color, DEFAULT_COLOR);
    }

    #[test]
    fn splat_normals_are_unit_length() {
        let pts = small_grid(5, 1.0);
        let cloud = bake_cloud_from_points(pts, &Normalization::identity(), 8);
        for (i, s) in cloud.splats.iter().enumerate() {
            let len = (s.normal[0].powi(2) + s.normal[1].powi(2) + s.normal[2].powi(2)).sqrt();
            assert!(
                (len - 1.0).abs() < 1e-4,
                "splat {i}: |n|={len}, normal={:?}",
                s.normal,
            );
        }
    }

    #[test]
    fn splat_radii_are_finite_and_non_negative() {
        let pts = small_grid(5, 1.0);
        let cloud = bake_cloud_from_points(pts, &Normalization::identity(), 8);
        for (i, s) in cloud.splats.iter().enumerate() {
            assert!(
                s.radius.is_finite() && s.radius >= 0.0,
                "splat {i}: r={}",
                s.radius
            );
        }
    }

    #[test]
    fn progress_callback_signals_completion() {
        use std::sync::Mutex;
        let last = std::sync::Arc::new(Mutex::new((0usize, 0usize)));
        let last_clone = std::sync::Arc::clone(&last);

        let pts = small_grid(5, 1.0); // 125 points
        let _ = bake_cloud_from_points_with_progress(
            pts,
            &Normalization::identity(),
            8,
            move |done, total| {
                *last_clone.lock().unwrap() = (done, total);
            },
        );

        let (done, total) = *last.lock().unwrap();
        assert_eq!(total, 125);
        assert_eq!(done, 125, "final progress callback should report all done");
    }

    #[test]
    fn progress_callback_fires_multiple_times_for_large_input() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let calls_clone = std::sync::Arc::clone(&calls);

        let pts = small_grid(10, 1.0); // 1000 points → throttled into ~100 buckets
        let _ = bake_cloud_from_points_with_progress(
            pts,
            &Normalization::identity(),
            8,
            move |_d, _t| {
                calls_clone.fetch_add(1, Ordering::Relaxed);
            },
        );

        assert!(
            calls.load(Ordering::Relaxed) > 1,
            "expected throttled progress to fire multiple times for 1000 points",
        );
    }

    #[test]
    fn bake_cloud_path_variant_round_trips_autzen_count() {
        // Slow-ish (110k points) but proves the path-taking variant is wired up.
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/autzen_trim.laz");
        let cloud = bake_cloud(&path, &Normalization::identity()).expect("bake");
        assert_eq!(cloud.splats.len(), 110_000);
    }
}
