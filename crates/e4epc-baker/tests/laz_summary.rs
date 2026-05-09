use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn assert_close(actual: [f64; 3], expected: [f64; 3], eps: f64) {
    for i in 0..3 {
        assert!(
            (actual[i] - expected[i]).abs() < eps,
            "axis {i}: expected {} ± {eps}, got {}",
            expected[i],
            actual[i],
        );
    }
}

#[test]
fn autzen_trim_has_known_summary() {
    let s = e4epc_baker::summarize_laz(fixture("autzen_trim.laz"))
        .expect("autzen_trim.laz must be readable");
    assert_eq!(s.point_count, 110_000);
    assert_close(s.bbox_min, [636_001.76, 848_935.20, 406.26], 1e-3);
    assert_close(s.bbox_max, [637_179.22, 849_497.90, 520.51], 1e-3);
}

#[test]
fn autzen_trim_reports_color_present() {
    let s = e4epc_baker::summarize_laz(fixture("autzen_trim.laz")).unwrap();
    assert!(s.has_color, "autzen_trim is known to carry RGB");
}

#[test]
fn missing_file_returns_error() {
    let result = e4epc_baker::summarize_laz(fixture("definitely-not-a-real-file.laz"));
    assert!(
        result.is_err(),
        "expected Err for missing file, got {result:?}"
    );
}
