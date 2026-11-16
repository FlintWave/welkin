// SPDX-License-Identifier: LGPL-3.0-or-later

//! Golden terrain vectors: LayerData bytes produced by OpenSim 0.9.3.0's own
//! TerrainCompressor for 19 height maps (see tests/golden/terrain/README.md).
//! The vectors are data; they contain no OpenSim code.

use std::io::Read;
use std::path::{Path, PathBuf};

use welkin_scene::terrain::TerrainChannel;
use welkin_scene::terrain_layer::{decode_land_layer, encode_land_layer};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/terrain")
}

fn read_input(file: &str) -> Vec<f32> {
    let gz = std::fs::read(golden_dir().join(file)).expect("input file");
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(&gz[..]).read_to_end(&mut raw).expect("gunzip");
    assert_eq!(raw.len(), 65536 * 4);
    raw.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
}

fn channel(heights: &[f32]) -> TerrainChannel {
    let mut t = TerrainChannel::new(256, 256);
    for y in 0..256u32 {
        for x in 0..256u32 {
            t.set(x, y, heights[(y * 256 + x) as usize]);
        }
    }
    t
}

fn all_patches() -> Vec<(u32, u32)> {
    (0..16u32).flat_map(|py| (0..16u32).map(move |px| (px, py))).collect()
}

/// (max, mean) absolute error of a LayerData stream against the input heights.
fn decode_error(data: &[u8], input: &[f32]) -> (f64, f64) {
    let patches = decode_land_layer(data).expect("decodes");
    assert_eq!(patches.len(), 256, "all patches decode");
    let (mut max, mut sum) = (0.0f64, 0.0f64);
    for (px, py, heights) in patches {
        for ly in 0..16u32 {
            for lx in 0..16u32 {
                let want = input[((py * 16 + ly) * 256 + px * 16 + lx) as usize];
                let err = (heights[(ly * 16 + lx) as usize] as f64 - want as f64).abs();
                max = max.max(err);
                sum += err;
            }
        }
    }
    (max, sum / 65536.0)
}

#[test]
fn encoder_matches_opensim_on_golden_terrains() {
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(golden_dir().join("manifest.json")).unwrap()).unwrap();
    let terrains = manifest["terrains"].as_array().expect("terrains");
    assert_eq!(terrains.len(), 19);

    for t in terrains {
        let name = t["name"].as_str().unwrap();
        let input = read_input(t["input_file"].as_str().unwrap());
        let expected = std::fs::read(golden_dir().join(t["opensim_output"].as_str().unwrap())).unwrap();
        let ref_max = t["reference_decode_error"]["max"].as_f64().unwrap();
        let ref_mean = t["reference_decode_error"]["mean"].as_f64().unwrap();
        let ours = encode_land_layer(&channel(&input), &all_patches());

        // Our decoder reproduces the error OpenSim's output has (checks the decoder).
        let (m, a) = decode_error(&expected, &input);
        assert!((m - ref_max).abs() <= ref_max * 0.01 + 0.001, "{name}: OpenSim stream max error {m} vs {ref_max}");
        assert!((a - ref_mean).abs() <= ref_mean * 0.01 + 0.001, "{name}: OpenSim stream mean error {a} vs {ref_mean}");

        match t["compare"].as_str().unwrap() {
            "bytes_identical" => assert_eq!(ours, expected, "{name}: differs from OpenSim output"),
            "decode_only" => {
                let (m, a) = decode_error(&ours, &input);
                assert!(m <= ref_max * 1.01 + 0.001, "{name}: max error {m} > reference {ref_max}");
                assert!(a <= ref_mean * 1.01 + 0.001, "{name}: mean error {a} > reference {ref_mean}");
            }
            other => panic!("{name}: unknown compare mode {other}"),
        }
    }
}
