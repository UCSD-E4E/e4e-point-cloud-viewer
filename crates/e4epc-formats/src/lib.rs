//! Shared data formats for the E4E point cloud viewer.
//!
//! For the MVP this exposes only the in-memory splat representation that the
//! baker produces and the desktop renderer consumes. The on-wire octree tile
//! format (and matching WASM decoder) lands in v2.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Splat {
    pub position: [f32; 3],
    pub color: [u8; 4],
    pub normal: [f32; 3],
    pub radius: f32,
}

/// 3x3 row-major rotation matrix.
type Rot3 = [[f64; 3]; 3];

const ROT_IDENTITY: Rot3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A rigid+uniform-scale transform from source coordinates to viewer coordinates.
///
/// `apply(p)` computes `R · (s · (p + t))`: translate to recenter, scale uniformly,
/// then rotate. Storing the components separately (rather than a single 4x4) keeps
/// it cheap to serialize for the IPC boundary and easy to reason about for the
/// snap-to-axis UI.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Normalization {
    pub translation: [f64; 3],
    pub scale: f64,
    pub rotation: Rot3,
}

impl Normalization {
    #[must_use]
    pub fn identity() -> Self {
        Self {
            translation: [0.0; 3],
            scale: 1.0,
            rotation: ROT_IDENTITY,
        }
    }

    #[must_use]
    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let recentered = [
            (p[0] + self.translation[0]) * self.scale,
            (p[1] + self.translation[1]) * self.scale,
            (p[2] + self.translation[2]) * self.scale,
        ];
        let r = &self.rotation;
        [
            r[0][0] * recentered[0] + r[0][1] * recentered[1] + r[0][2] * recentered[2],
            r[1][0] * recentered[0] + r[1][1] * recentered[1] + r[1][2] * recentered[2],
            r[2][0] * recentered[0] + r[2][1] * recentered[1] + r[2][2] * recentered[2],
        ]
    }
}

/// Pack a `SplatCloud` into a flat little-endian binary buffer suitable for
/// IPC and direct upload to typed arrays in the frontend.
///
/// Layout (structure-of-arrays so each attribute can be a typed-array view):
/// ```text
/// u32  count
/// f32  positions[count * 3]
/// u8   colors[count * 4]
/// f32  normals[count * 3]
/// f32  radii[count]
/// ```
///
/// Every typed-array slice begins on a 4-byte boundary so the JS side can
/// construct `Float32Array` views without copying.
///
/// A packed buffer is exactly `4 + count * PACKED_SPLAT_SIZE` bytes long.
pub const PACKED_SPLAT_SIZE: usize = 12 /* position */ + 4 /* color */ + 12 /* normal */ + 4 /* radius */;

#[must_use]
pub fn pack_splat_cloud(cloud: &SplatCloud) -> Vec<u8> {
    let n = cloud.splats.len();
    let total = 4 + n * PACKED_SPLAT_SIZE;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&u32::try_from(n).unwrap_or(u32::MAX).to_le_bytes());
    for s in &cloud.splats {
        for &v in &s.position {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    for s in &cloud.splats {
        out.extend_from_slice(&s.color);
    }
    for s in &cloud.splats {
        for &v in &s.normal {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    for s in &cloud.splats {
        out.extend_from_slice(&s.radius.to_le_bytes());
    }
    out
}

/// The splat count from a packed `.e4epc` buffer, but only if the buffer's
/// length is consistent with that count. Returns `None` for a truncated or
/// otherwise malformed buffer — a cheap "is this really an `.e4epc`?" gate for
/// upload endpoints, without decoding the whole thing.
#[must_use]
pub fn packed_cloud_splat_count(bytes: &[u8]) -> Option<usize> {
    let header: [u8; 4] = bytes.get(0..4)?.try_into().ok()?;
    let n = u32::from_le_bytes(header) as usize;
    let expected = n.checked_mul(PACKED_SPLAT_SIZE)?.checked_add(4)?;
    (bytes.len() == expected).then_some(n)
}

#[derive(Debug, Default, Clone)]
pub struct SplatCloud {
    pub splats: Vec<Splat>,
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
}

impl SplatCloud {
    #[must_use]
    pub fn len(&self) -> usize {
        self.splats.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.splats.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn empty_cloud_is_empty() {
        let c = SplatCloud::default();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    fn read_f32_le(b: &[u8]) -> f32 {
        f32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    #[test]
    fn pack_splat_cloud_layout_matches_spec() {
        let cloud = SplatCloud {
            splats: vec![
                Splat {
                    position: [1.0, 2.0, 3.0],
                    color: [10, 20, 30, 40],
                    normal: [0.0, 0.0, 1.0],
                    radius: 0.5,
                },
                Splat {
                    position: [4.0, 5.0, 6.0],
                    color: [50, 60, 70, 80],
                    normal: [0.0, 1.0, 0.0],
                    radius: 0.7,
                },
            ],
            bbox_min: [0.0, 0.0, 0.0],
            bbox_max: [0.0, 0.0, 0.0],
        };
        let bytes = pack_splat_cloud(&cloud);
        assert_eq!(bytes.len(), 4 + 24 + 8 + 24 + 8);

        // Header
        assert_eq!(&bytes[0..4], &[2, 0, 0, 0]);
        // Positions
        assert_eq!(read_f32_le(&bytes[4..8]), 1.0);
        assert_eq!(read_f32_le(&bytes[8..12]), 2.0);
        assert_eq!(read_f32_le(&bytes[12..16]), 3.0);
        assert_eq!(read_f32_le(&bytes[16..20]), 4.0);
        assert_eq!(read_f32_le(&bytes[20..24]), 5.0);
        assert_eq!(read_f32_le(&bytes[24..28]), 6.0);
        // Colors
        assert_eq!(&bytes[28..36], &[10, 20, 30, 40, 50, 60, 70, 80]);
        // Normals
        assert_eq!(read_f32_le(&bytes[36..40]), 0.0);
        assert_eq!(read_f32_le(&bytes[40..44]), 0.0);
        assert_eq!(read_f32_le(&bytes[44..48]), 1.0);
        assert_eq!(read_f32_le(&bytes[48..52]), 0.0);
        assert_eq!(read_f32_le(&bytes[52..56]), 1.0);
        assert_eq!(read_f32_le(&bytes[56..60]), 0.0);
        // Radii
        assert_eq!(read_f32_le(&bytes[60..64]), 0.5);
        assert_eq!(read_f32_le(&bytes[64..68]), 0.7);
    }

    #[test]
    fn pack_empty_cloud_is_just_header() {
        let bytes = pack_splat_cloud(&SplatCloud::default());
        assert_eq!(bytes, &[0, 0, 0, 0]);
    }

    fn sample_cloud(n: usize) -> SplatCloud {
        SplatCloud {
            splats: vec![
                Splat {
                    position: [1.0, 2.0, 3.0],
                    color: [10, 20, 30, 255],
                    normal: [0.0, 0.0, 1.0],
                    radius: 0.5,
                };
                n
            ],
            bbox_min: [0.0; 3],
            bbox_max: [0.0; 3],
        }
    }

    #[test]
    fn packed_cloud_splat_count_reads_a_well_formed_buffer() {
        for n in [0usize, 1, 2, 1000] {
            let bytes = pack_splat_cloud(&sample_cloud(n));
            assert_eq!(packed_cloud_splat_count(&bytes), Some(n));
        }
    }

    #[test]
    fn packed_cloud_splat_count_rejects_a_short_header() {
        assert_eq!(packed_cloud_splat_count(&[]), None);
        assert_eq!(packed_cloud_splat_count(&[0, 0, 0]), None);
    }

    #[test]
    fn packed_cloud_splat_count_rejects_a_length_mismatch() {
        let mut bytes = pack_splat_cloud(&sample_cloud(2));
        bytes.pop(); // truncate one byte
        assert_eq!(packed_cloud_splat_count(&bytes), None);

        // Header claims 2 splats but the buffer is just the header.
        assert_eq!(packed_cloud_splat_count(&[2, 0, 0, 0]), None);
    }

    #[test]
    fn identity_normalization_is_a_noop() {
        let n = Normalization::identity();
        assert_eq!(n.apply([1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);
        assert_eq!(n.apply([-7.5, 0.0, 999.0]), [-7.5, 0.0, 999.0]);
    }

    #[test]
    fn translation_recenters() {
        let n = Normalization {
            translation: [-10.0, -20.0, -30.0],
            ..Normalization::identity()
        };
        assert_eq!(n.apply([10.0, 20.0, 30.0]), [0.0, 0.0, 0.0]);
        assert_eq!(n.apply([12.0, 22.0, 33.0]), [2.0, 2.0, 3.0]);
    }

    #[test]
    fn scale_is_applied_after_translation() {
        let n = Normalization {
            translation: [-100.0, 0.0, 0.0],
            scale: 0.5,
            ..Normalization::identity()
        };
        // (110 - 100) * 0.5 = 5
        assert_eq!(n.apply([110.0, 4.0, 8.0]), [5.0, 2.0, 4.0]);
    }

    #[test]
    fn rotation_is_applied_last() {
        // 90° rotation about Z: [x, y, z] -> [-y, x, z]
        let n = Normalization {
            rotation: [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            ..Normalization::identity()
        };
        let out = n.apply([1.0, 0.0, 7.0]);
        assert!((out[0] - 0.0).abs() < 1e-12);
        assert!((out[1] - 1.0).abs() < 1e-12);
        assert_eq!(out[2], 7.0);
    }
}
