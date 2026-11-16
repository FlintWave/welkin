// SPDX-License-Identifier: LGPL-3.0-or-later

use std::io::{Read, Write};

use crate::terrain::TerrainChannel;

const LLRAW_PIXEL_SIZE: usize = 13;
const LLRAW_REGION_SIZE: u32 = 256;
const LLRAW_TOTAL_BYTES: usize =
    (LLRAW_REGION_SIZE * LLRAW_REGION_SIZE) as usize * LLRAW_PIXEL_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainFormat {
    Raw32,
    LlRaw,
}

#[derive(Debug, thiserror::Error)]
pub enum TerrainIoError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("invalid size: expected {expected} bytes, got {got}")]
    InvalidSize { expected: usize, got: usize },

    #[error("unsupported dimensions: {width}x{height}")]
    UnsupportedDimensions { width: u32, height: u32 },
}

/// OpenSim terrain database revision codes (`DBTerrainRevision`).
pub const DB_REVISION_VARIABLE_2D: i32 = 22;
pub const DB_REVISION_VARIABLE_2D_GZIP: i32 = 23;

/// Serialise heights as OpenSim's `Variable2DGzip` (revision 23): gzip of
/// `i32 size_x, i32 size_y, f32 heights` (y outer, x inner), heights rounded to
/// 3 decimals.
pub fn to_db_blob(size_x: u32, size_y: u32, heights: &[f32]) -> Result<Vec<u8>, TerrainIoError> {
    let expected = (size_x as usize) * (size_y as usize);
    if heights.len() != expected {
        return Err(TerrainIoError::InvalidSize { expected: expected * 4, got: heights.len() * 4 });
    }
    let mut raw = Vec::with_capacity(8 + expected * 4);
    raw.extend_from_slice(&(size_x as i32).to_le_bytes());
    raw.extend_from_slice(&(size_y as i32).to_le_bytes());
    for &h in heights {
        raw.extend_from_slice(&((h * 1000.0).round() / 1000.0).to_le_bytes());
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&raw)?;
    Ok(gz.finish()?)
}

/// Parse a terrain database blob of revision 22 (plain) or 23 (gzip). Returns
/// (size_x, size_y, heights, row-major y*size_x+x).
pub fn from_db_blob(revision: i32, blob: &[u8]) -> Result<(u32, u32, Vec<f32>), TerrainIoError> {
    let raw: Vec<u8> = match revision {
        DB_REVISION_VARIABLE_2D => blob.to_vec(),
        DB_REVISION_VARIABLE_2D_GZIP => {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(blob).read_to_end(&mut out)?;
            out
        }
        _ => return Err(TerrainIoError::UnsupportedDimensions { width: 0, height: revision as u32 }),
    };
    if raw.len() < 8 {
        return Err(TerrainIoError::InvalidSize { expected: 8, got: raw.len() });
    }
    let w = i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let h = i32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
    if w <= 0 || h <= 0 || w > 4096 || h > 4096 {
        return Err(TerrainIoError::UnsupportedDimensions { width: w as u32, height: h as u32 });
    }
    let count = (w as usize) * (h as usize);
    if raw.len() != 8 + count * 4 {
        return Err(TerrainIoError::InvalidSize { expected: 8 + count * 4, got: raw.len() });
    }
    let heights = raw[8..]
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .map(|v| if v.is_finite() { v } else { 0.0 })
        .collect();
    Ok((w as u32, h as u32, heights))
}

pub fn save_raw32(terrain: &TerrainChannel, writer: &mut impl Write) -> Result<(), TerrainIoError> {
    for &h in terrain.heights() {
        writer.write_all(&h.to_le_bytes())?;
    }
    Ok(())
}

pub fn load_raw32(
    reader: &mut impl Read,
    size_x: u32,
    size_y: u32,
) -> Result<TerrainChannel, TerrainIoError> {
    let count = (size_x * size_y) as usize;
    let expected = count * 4;
    let mut buf = vec![0u8; expected];
    let got = read_full(reader, &mut buf)?;
    if got != expected {
        return Err(TerrainIoError::InvalidSize { expected, got });
    }

    let mut terrain = TerrainChannel::new(size_x, size_y);
    let heights = terrain.heights_mut();
    for (i, chunk) in buf.as_chunks::<4>().0.iter().enumerate() {
        heights[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    Ok(terrain)
}

pub fn save_llraw(terrain: &TerrainChannel, writer: &mut impl Write) -> Result<(), TerrainIoError> {
    if terrain.size_x() != LLRAW_REGION_SIZE || terrain.size_y() != LLRAW_REGION_SIZE {
        return Err(TerrainIoError::UnsupportedDimensions {
            width: terrain.size_x(),
            height: terrain.size_y(),
        });
    }

    for &h in terrain.heights() {
        let val = h * 128.0;
        let high = (val / 256.0) as u8;
        let low = (val % 256.0) as u8;
        let pixel: [u8; LLRAW_PIXEL_SIZE] = [
            low, low, low, high, high, high, 0xFF, high, high, high, low, low, low,
        ];
        writer.write_all(&pixel)?;
    }
    Ok(())
}

pub fn load_llraw(reader: &mut impl Read) -> Result<TerrainChannel, TerrainIoError> {
    let mut buf = vec![0u8; LLRAW_TOTAL_BYTES];
    let got = read_full(reader, &mut buf)?;
    if got != LLRAW_TOTAL_BYTES {
        return Err(TerrainIoError::InvalidSize {
            expected: LLRAW_TOTAL_BYTES,
            got,
        });
    }

    let mut terrain = TerrainChannel::new(LLRAW_REGION_SIZE, LLRAW_REGION_SIZE);
    let heights = terrain.heights_mut();
    for (i, chunk) in buf.as_chunks::<LLRAW_PIXEL_SIZE>().0.iter().enumerate() {
        heights[i] = (chunk[3] as f32 * 256.0 + chunk[0] as f32) / 128.0;
    }
    Ok(terrain)
}

fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> Result<usize, std::io::Error> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..])? {
            0 => break,
            n => total += n,
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    #[test]
    fn db_blob_round_trips_in_opensim_format() {
        let heights: Vec<f32> = (0..32 * 16).map(|i| i as f32 * 0.37).collect();
        let blob = super::to_db_blob(32, 16, &heights).unwrap();
        let (w, h, back) = super::from_db_blob(super::DB_REVISION_VARIABLE_2D_GZIP, &blob).unwrap();
        assert_eq!((w, h), (32, 16));
        for (a, b) in heights.iter().zip(&back) {
            assert!((a - b).abs() <= 0.0006, "{a} vs {b}");
        }
    }

    #[test]
    fn db_blob_rejects_bad_input() {
        assert!(super::from_db_blob(99, &[1, 2, 3]).is_err());
        assert!(super::from_db_blob(super::DB_REVISION_VARIABLE_2D, &[0; 4]).is_err());
        // wrong payload length for the declared size
        let mut raw = Vec::new();
        raw.extend_from_slice(&16i32.to_le_bytes());
        raw.extend_from_slice(&16i32.to_le_bytes());
        raw.extend_from_slice(&[0u8; 12]);
        assert!(super::from_db_blob(super::DB_REVISION_VARIABLE_2D, &raw).is_err());
        assert!(super::to_db_blob(16, 16, &[0.0; 5]).is_err());
    }

    #[test]
    fn db_blob_replaces_non_finite_values() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&1i32.to_le_bytes());
        raw.extend_from_slice(&1i32.to_le_bytes());
        raw.extend_from_slice(&f32::NAN.to_le_bytes());
        let (_, _, h) = super::from_db_blob(super::DB_REVISION_VARIABLE_2D, &raw).unwrap();
        assert_eq!(h, vec![0.0]);
    }

    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_raw32_roundtrip() {
        let mut t = TerrainChannel::new_default();
        t.set(10, 20, 42.5);
        t.set(100, 200, 123.75);

        let mut buf = Vec::new();
        save_raw32(&t, &mut buf).unwrap();

        let mut cursor = Cursor::new(buf);
        let loaded = load_raw32(&mut cursor, 256, 256).unwrap();

        assert_eq!(loaded.get(10, 20), 42.5);
        assert_eq!(loaded.get(100, 200), 123.75);
        assert_eq!(loaded.get(0, 0), 21.0);
    }

    #[test]
    fn test_raw32_variable_size() {
        let mut t = TerrainChannel::new(64, 64);
        t.set(32, 32, 55.0);

        let mut buf = Vec::new();
        save_raw32(&t, &mut buf).unwrap();
        assert_eq!(buf.len(), 64 * 64 * 4);

        let mut cursor = Cursor::new(buf);
        let loaded = load_raw32(&mut cursor, 64, 64).unwrap();
        assert_eq!(loaded.size_x(), 64);
        assert_eq!(loaded.size_y(), 64);
        assert_eq!(loaded.get(32, 32), 55.0);
    }

    #[test]
    fn test_llraw_roundtrip() {
        let mut t = TerrainChannel::new_default();
        t.set(0, 0, 21.0);
        t.set(128, 128, 50.0);
        t.set(255, 255, 100.0);

        let mut buf = Vec::new();
        save_llraw(&t, &mut buf).unwrap();
        assert_eq!(buf.len(), LLRAW_TOTAL_BYTES);

        let mut cursor = Cursor::new(buf);
        let loaded = load_llraw(&mut cursor).unwrap();

        assert!((loaded.get(0, 0) - 21.0).abs() < 0.01);
        assert!((loaded.get(128, 128) - 50.0).abs() < 0.01);
        assert!((loaded.get(255, 255) - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_llraw_wrong_size() {
        let short_buf = vec![0u8; 1000];
        let mut cursor = Cursor::new(short_buf);
        let result = load_llraw(&mut cursor);
        assert!(result.is_err());
        match result.unwrap_err() {
            TerrainIoError::InvalidSize { expected, got } => {
                assert_eq!(expected, LLRAW_TOTAL_BYTES);
                assert_eq!(got, 1000);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn test_llraw_rejects_non_256() {
        let t = TerrainChannel::new(512, 512);
        let mut buf = Vec::new();
        let result = save_llraw(&t, &mut buf);
        assert!(result.is_err());
        match result.unwrap_err() {
            TerrainIoError::UnsupportedDimensions { width, height } => {
                assert_eq!(width, 512);
                assert_eq!(height, 512);
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
