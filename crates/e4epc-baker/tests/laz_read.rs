use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn iter_points_yields_summary_count_for_autzen_trim() {
    let mut iter = e4epc_baker::iter_points(fixture("autzen_trim.laz")).unwrap();
    let count: u64 = iter
        .try_fold(0, |n, p| p.map(|_| n + 1))
        .expect("all points should decode cleanly");
    assert_eq!(count, 110_000);
}

#[test]
fn iter_points_surfaces_color_when_format_has_it() {
    let mut iter = e4epc_baker::iter_points(fixture("autzen_trim.laz")).unwrap();
    let first = iter.next().unwrap().unwrap();
    assert!(
        first.color.is_some(),
        "autzen_trim is RGB; first point should report color",
    );
}

#[test]
fn iter_points_returns_non_zero_colors_for_autzen_trim() {
    // autzen stores 8-bit color values in u16 fields (low byte). A naive
    // `>> 8` would zero everything out and the rendering would be all-black.
    let iter = e4epc_baker::iter_points(fixture("autzen_trim.laz")).unwrap();
    let any_nonzero = iter.take(1000).any(|p| {
        let c = p.unwrap().color.unwrap();
        c[0] > 0 || c[1] > 0 || c[2] > 0
    });
    assert!(
        any_nonzero,
        "expected at least one non-zero color in first 1000 points",
    );
}

#[test]
fn iter_points_returns_positions_inside_bbox() {
    let summary = e4epc_baker::summarize_laz(fixture("autzen_trim.laz")).unwrap();
    let iter = e4epc_baker::iter_points(fixture("autzen_trim.laz")).unwrap();
    // Sample the first ~1000 points; checking all 110k is overkill for a property test.
    for p in iter.take(1000) {
        let p = p.unwrap();
        for axis in 0..3 {
            assert!(
                p.position[axis] >= summary.bbox_min[axis]
                    && p.position[axis] <= summary.bbox_max[axis],
                "axis {axis}: {} not in [{}, {}]",
                p.position[axis],
                summary.bbox_min[axis],
                summary.bbox_max[axis],
            );
        }
    }
}
