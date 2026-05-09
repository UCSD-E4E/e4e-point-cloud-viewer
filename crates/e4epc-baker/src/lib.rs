//! Baker pipeline: LAZ → normalized splat cloud.
//!
//! M1 lands LAZ ingestion. The `summarize_laz` entry point produces a
//! lightweight `Summary` (counts, bbox, channel presence) suitable for
//! showing the user immediately after they pick a file, well before the
//! splat bake is run.

use std::path::Path;

use e4epc_formats::SplatCloud;

pub mod bake;
pub mod knn;
pub mod normal;
pub mod normalize;
pub use bake::{bake_cloud, bake_cloud_from_points, bake_cloud_from_points_with_progress};
pub use normalize::auto_normalize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawPoint {
    pub position: [f64; 3],
    pub color: Option<[u8; 4]>,
}

pub struct PointIter {
    reader: las::Reader,
    has_color: bool,
    /// Bit-shift to apply when converting LAS u16 color channels to u8.
    /// LAS spec says channels are full 16-bit; many real files store 8-bit
    /// values in the low byte. We pick `8` (high byte) when any channel
    /// exceeds 255 in a sample, otherwise `0` (low byte).
    color_shift: u32,
}

impl Iterator for PointIter {
    type Item = Result<RawPoint, BakerError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.read_point() {
            Ok(Some(p)) => {
                let shift = self.color_shift;
                let color = if self.has_color {
                    p.color.map(|c| {
                        [
                            (c.red >> shift) as u8,
                            (c.green >> shift) as u8,
                            (c.blue >> shift) as u8,
                            255,
                        ]
                    })
                } else {
                    None
                };
                Some(Ok(RawPoint {
                    position: [p.x, p.y, p.z],
                    color,
                }))
            }
            Ok(None) => None,
            Err(e) => Some(Err(BakerError::Open(e))),
        }
    }
}

const COLOR_DETECT_SAMPLE: usize = 1024;

fn detect_color_shift(path: &Path) -> Result<u32, BakerError> {
    let mut reader = las::Reader::from_path(path)?;
    if !reader.header().point_format().has_color {
        return Ok(0);
    }
    let mut max_seen: u16 = 0;
    for _ in 0..COLOR_DETECT_SAMPLE {
        match reader.read_point()? {
            Some(p) => {
                if let Some(c) = p.color {
                    max_seen = max_seen.max(c.red).max(c.green).max(c.blue);
                }
            }
            None => break,
        }
    }
    Ok(if max_seen > 255 { 8 } else { 0 })
}

pub fn iter_points<P: AsRef<Path>>(path: P) -> Result<PointIter, BakerError> {
    let path_ref = path.as_ref();
    let color_shift = detect_color_shift(path_ref)?;
    let reader = las::Reader::from_path(path_ref)?;
    let has_color = reader.header().point_format().has_color;
    Ok(PointIter {
        reader,
        has_color,
        color_shift,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum BakerError {
    #[error("failed to open LAZ file: {0}")]
    Open(#[from] las::Error),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Summary {
    pub point_count: u64,
    pub bbox_min: [f64; 3],
    pub bbox_max: [f64; 3],
    pub has_color: bool,
}

pub fn summarize_laz<P: AsRef<Path>>(path: P) -> Result<Summary, BakerError> {
    let reader = las::Reader::from_path(path)?;
    let header = reader.header();
    let bounds = header.bounds();
    Ok(Summary {
        point_count: header.number_of_points(),
        bbox_min: [bounds.min.x, bounds.min.y, bounds.min.z],
        bbox_max: [bounds.max.x, bounds.max.y, bounds.max.z],
        has_color: header.point_format().has_color,
    })
}

#[must_use]
pub fn empty_cloud() -> SplatCloud {
    SplatCloud::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_returns_empty() {
        assert!(empty_cloud().is_empty());
    }
}
