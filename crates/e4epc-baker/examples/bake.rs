//! Dev utility: bake a LAZ file to the flat `.e4epc` splat buffer the browser
//! viewer renders. Used to produce test/demo fixtures until the desktop "Save
//! .e4epc" command (M7) makes this a UI button.
//!
//! ```text
//! cargo run -q -p e4epc-baker --example bake -- input.laz output.e4epc
//! ```
//!
//! Applies the same default normalization the desktop UI starts from: recenter
//! the bounding box to the origin, scale the longest axis to 5 m, +Z up.

use std::path::PathBuf;

use e4epc_baker::{auto_normalize, bake_cloud_from_points, iter_points, summarize_laz};
use e4epc_formats::pack_splat_cloud;

const TARGET_LONGEST_EXTENT_M: f64 = 5.0;
const BAKE_K: usize = 12;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let usage = "usage: bake <input.laz> <output.e4epc>";
    let input = PathBuf::from(args.next().ok_or(usage)?);
    let output = PathBuf::from(args.next().ok_or(usage)?);

    let summary = summarize_laz(&input)?;
    let normalization = auto_normalize(&summary, TARGET_LONGEST_EXTENT_M);
    let points = iter_points(&input)?.collect::<Result<Vec<_>, _>>()?;
    let cloud = bake_cloud_from_points(points, &normalization, BAKE_K);
    std::fs::write(&output, pack_splat_cloud(&cloud))?;

    eprintln!("baked {} splats -> {}", cloud.len(), output.display());
    Ok(())
}
