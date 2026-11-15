// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{Read, Write};

use crate::terrain::TerrainChannel;

const LLRAW_PIXEL_SIZE: usize = 13;
const LLRAW_REGION_SIZE: u32 = 256;
const LLRAW_TOTAL_BYTES: usize = (LLRAW_REGION_SIZE * LLRAW_REGION_SIZE) as usize * LLRAW_PIXEL_SIZE;

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
    for (i, chunk) in buf.chunks_exact(4).enumerate() {
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
    for (i, chunk) in buf.chunks_exact(LLRAW_PIXEL_SIZE).enumerate() {
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
