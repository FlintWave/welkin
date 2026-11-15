// SPDX-License-Identifier: GPL-3.0-or-later

use crate::terrain::{TERRAIN_PATCH_SIZE, TerrainChannel};

pub const END_OF_PATCHES: u8 = 97;
pub const STRIDE: u16 = 264;

#[derive(Debug, Clone)]
pub struct PatchHeader {
    pub quant_w_bits: u8,
    pub dc_offset: f32,
    pub range: u16,
    pub patch_ids: u32,
}

pub struct PatchEncoder {
    buf: Vec<u8>,
    bit_offset: usize,
}

impl PatchEncoder {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(1024),
            bit_offset: 0,
        }
    }

    pub fn write_bits(&mut self, value: u32, bits: u8) {
        if bits == 0 {
            return;
        }
        let mut remaining = bits;
        let mut val = value;

        while remaining > 0 {
            let byte_idx = self.bit_offset / 8;
            let bit_idx = self.bit_offset % 8;

            if byte_idx >= self.buf.len() {
                self.buf.push(0);
            }

            let space = 8 - bit_idx;
            let to_write = remaining.min(space as u8);

            let shift = remaining - to_write;
            let mask = ((1u32 << to_write) - 1) as u8;
            let bits_val = ((val >> shift) & mask as u32) as u8;

            self.buf[byte_idx] |= bits_val << (space - to_write as usize);

            val &= (1u32 << shift) - if shift == 0 { 0 } else { 1 };
            remaining -= to_write;
            self.bit_offset += to_write as usize;
        }
    }

    pub fn write_f32(&mut self, v: f32) {
        let bytes = v.to_le_bytes();
        self.align_byte();
        self.buf.extend_from_slice(&bytes);
        self.bit_offset = self.buf.len() * 8;
    }

    pub fn write_u16(&mut self, v: u16) {
        let bytes = v.to_le_bytes();
        self.align_byte();
        self.buf.extend_from_slice(&bytes);
        self.bit_offset = self.buf.len() * 8;
    }

    pub fn write_u8(&mut self, v: u8) {
        self.align_byte();
        self.buf.push(v);
        self.bit_offset = self.buf.len() * 8;
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.align_byte();
        self.buf
    }

    fn align_byte(&mut self) {
        let remainder = self.bit_offset % 8;
        if remainder != 0 {
            self.bit_offset += 8 - remainder;
        }
        while self.buf.len() < self.bit_offset.div_ceil(8) {
            self.buf.push(0);
        }
    }
}

impl Default for PatchEncoder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn encode_patch_header(encoder: &mut PatchEncoder, header: &PatchHeader) {
    encoder.write_u8(header.quant_w_bits);
    encoder.write_f32(header.dc_offset);
    encoder.write_u16(header.range);
    encoder.write_bits(header.patch_ids, 10);
}

pub fn encode_patch(encoder: &mut PatchEncoder, heights: &[f32; 256], large_region: bool) {
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    for &h in heights.iter() {
        if h < min_h {
            min_h = h;
        }
        if h > max_h {
            max_h = h;
        }
    }

    let range = ((max_h - min_h).ceil() as u16).max(1);
    let word_bits: u8 = if range <= 1 {
        2
    } else {
        (range as f32).log2().ceil().max(2.0).min(16.0) as u8
    };

    let header = PatchHeader {
        quant_w_bits: word_bits,
        dc_offset: min_h,
        range,
        patch_ids: 0,
    };
    let _ = large_region;

    encoder.write_u8(header.quant_w_bits);
    encoder.write_f32(header.dc_offset);
    encoder.write_u16(header.range);

    let max_val = (1u32 << word_bits) - 1;
    let scale = if range <= 1 {
        0.0
    } else {
        max_val as f32 / range as f32
    };

    let mut quantized = [0u16; 256];
    for (i, &h) in heights.iter().enumerate() {
        let q = ((h - min_h) * scale).round() as u32;
        quantized[i] = q.min(max_val) as u16;
    }

    let mut trailing_zeros = 0usize;
    for &q in quantized.iter().rev() {
        if q == 0 {
            trailing_zeros += 1;
        } else {
            break;
        }
    }

    let significant = 256 - trailing_zeros;

    let mut run_zeros = 0u32;
    for i in 0..significant {
        let q = quantized[i];
        if q == 0 {
            run_zeros += 1;
            if run_zeros == 8 {
                encoder.write_bits(0b0, 1);
                run_zeros = 0;
            }
        } else {
            while run_zeros > 0 {
                encoder.write_bits(0b110, 3);
                encoder.write_bits(0, word_bits);
                run_zeros -= 1;
            }
            encoder.write_bits(0b110, 3);
            encoder.write_bits(q as u32, word_bits);
        }
    }

    while run_zeros > 0 {
        encoder.write_bits(0b110, 3);
        encoder.write_bits(0, word_bits);
        run_zeros -= 1;
    }

    encoder.write_bits(0b10, 2);
}

pub fn encode_layer_data(terrain: &TerrainChannel, patches: &[(u32, u32)]) -> Vec<u8> {
    let mut encoder = PatchEncoder::new();

    encoder.write_u16(STRIDE);
    encoder.write_u8(TERRAIN_PATCH_SIZE as u8);
    encoder.write_u8(1);

    for &(px, py) in patches {
        let mut heights = [0.0f32; 256];
        let base_x = px * TERRAIN_PATCH_SIZE;
        let base_y = py * TERRAIN_PATCH_SIZE;

        for ly in 0..TERRAIN_PATCH_SIZE {
            for lx in 0..TERRAIN_PATCH_SIZE {
                let gx = base_x + lx;
                let gy = base_y + ly;
                heights[(ly * TERRAIN_PATCH_SIZE + lx) as usize] = terrain.get(gx, gy);
            }
        }

        let patch_id = (px << 5) | (py & 0x1F);
        encoder.write_bits(patch_id, 10);
        encode_patch(&mut encoder, &heights, false);
    }

    encoder.write_u8(END_OF_PATCHES);
    encoder.finish()
}

pub fn decode_patch(
    data: &[u8],
    offset: &mut usize,
    _large_region: bool,
) -> Option<(u32, u32, [f32; 256])> {
    if *offset >= data.len() {
        return None;
    }

    let mut bit_pos = *offset * 8;

    let read_bits = |bp: &mut usize, count: u8| -> Option<u32> {
        if count == 0 {
            return Some(0);
        }
        let mut val = 0u32;
        for _ in 0..count {
            let byte_idx = *bp / 8;
            let bit_idx = *bp % 8;
            if byte_idx >= data.len() {
                return None;
            }
            let bit = (data[byte_idx] >> (7 - bit_idx)) & 1;
            val = (val << 1) | bit as u32;
            *bp += 1;
        }
        Some(val)
    };

    let patch_ids = read_bits(&mut bit_pos, 10)?;
    let px = (patch_ids >> 5) & 0x1F;
    let py = patch_ids & 0x1F;

    let align = |bp: &mut usize| {
        let rem = *bp % 8;
        if rem != 0 {
            *bp += 8 - rem;
        }
    };
    align(&mut bit_pos);

    let byte_idx = bit_pos / 8;
    if byte_idx + 7 > data.len() {
        return None;
    }
    let quant_w_bits = data[byte_idx];
    bit_pos += 8;

    let byte_idx = bit_pos / 8;
    let dc_offset = f32::from_le_bytes([
        data[byte_idx],
        data[byte_idx + 1],
        data[byte_idx + 2],
        data[byte_idx + 3],
    ]);
    bit_pos += 32;

    let byte_idx = bit_pos / 8;
    let range = u16::from_le_bytes([data[byte_idx], data[byte_idx + 1]]);
    bit_pos += 16;

    let max_val = if quant_w_bits >= 32 {
        u32::MAX
    } else {
        (1u32 << quant_w_bits) - 1
    };
    let scale = if range <= 1 {
        0.0
    } else {
        (range as f32) / max_val as f32
    };

    let mut quantized = [0u16; 256];
    let mut idx = 0usize;

    while idx < 256 {
        let tag = read_bits(&mut bit_pos, 1)?;
        if tag == 0 {
            let end = (idx + 8).min(256);
            while idx < end {
                quantized[idx] = 0;
                idx += 1;
            }
        } else {
            let tag2 = read_bits(&mut bit_pos, 1)?;
            if tag2 == 0 {
                while idx < 256 {
                    quantized[idx] = 0;
                    idx += 1;
                }
                break;
            } else {
                let _sign = read_bits(&mut bit_pos, 1)?;
                let val = read_bits(&mut bit_pos, quant_w_bits)? as u16;
                quantized[idx] = val;
                idx += 1;
            }
        }
    }

    let mut heights = [0.0f32; 256];
    for i in 0..256 {
        heights[i] = dc_offset + quantized[i] as f32 * scale;
    }

    align(&mut bit_pos);
    *offset = bit_pos / 8;

    Some((px, py, heights))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_terrain(height: f32) -> TerrainChannel {
        let mut t = TerrainChannel::new(256, 256);
        for y in 0..256 {
            for x in 0..256 {
                t.set(x, y, height);
            }
        }
        t
    }

    #[test]
    fn test_flat_patch_encoding() {
        let mut encoder = PatchEncoder::new();
        let heights = [25.0f32; 256];
        encode_patch(&mut encoder, &heights, false);
        let data = encoder.finish();
        assert!(
            data.len() < 100,
            "flat terrain should compress well: {} bytes",
            data.len()
        );
    }

    #[test]
    fn test_patch_header_format() {
        let mut encoder = PatchEncoder::new();
        let header = PatchHeader {
            quant_w_bits: 10,
            dc_offset: 20.0,
            range: 50,
            patch_ids: (3 << 5) | 7,
        };

        encode_patch_header(&mut encoder, &header);
        let data = encoder.finish();

        assert_eq!(data[0], 10, "quant_w_bits should be 10");

        let dc = f32::from_le_bytes([data[1], data[2], data[3], data[4]]);
        assert!((dc - 20.0).abs() < f32::EPSILON, "dc_offset should be 20.0");

        let range = u16::from_le_bytes([data[5], data[6]]);
        assert_eq!(range, 50, "range should be 50");
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let mut heights = [0.0f32; 256];
        for i in 0..256 {
            heights[i] = 20.0 + (i as f32) * 0.1;
        }

        let mut encoder = PatchEncoder::new();
        let patch_id = (2u32 << 5) | 3;
        encoder.write_bits(patch_id, 10);
        encode_patch(&mut encoder, &heights, false);

        let data = encoder.finish();
        let mut offset = 0;
        let result = decode_patch(&data, &mut offset, false);

        assert!(result.is_some(), "decode should succeed");
        let (px, py, decoded) = result.unwrap();
        assert_eq!(px, 2);
        assert_eq!(py, 3);

        let tolerance = 0.5;
        for i in 0..256 {
            let diff = (decoded[i] - heights[i]).abs();
            assert!(
                diff < tolerance,
                "height[{i}] diff {diff} exceeds tolerance: decoded={} expected={}",
                decoded[i],
                heights[i]
            );
        }
    }

    #[test]
    fn test_layer_data_format() {
        let terrain = flat_terrain(25.0);
        let patches = vec![(0, 0)];
        let data = encode_layer_data(&terrain, &patches);

        assert!(data.len() > 4, "should have group header");

        let stride = u16::from_le_bytes([data[0], data[1]]);
        assert_eq!(stride, STRIDE, "stride should match STRIDE constant");

        assert_eq!(data[2], 16, "patch size should be 16");
        assert_eq!(data[3], 1, "type should be 1");

        assert_eq!(
            *data.last().unwrap(),
            END_OF_PATCHES,
            "should end with END_OF_PATCHES"
        );
    }

    #[test]
    fn test_bit_writer() {
        let mut encoder = PatchEncoder::new();
        encoder.write_bits(0b101, 3);
        encoder.write_bits(0b1100, 4);
        let data = encoder.finish();
        assert_eq!(data[0], 0b1011_1000);
    }

    #[test]
    fn test_varied_patch_encoding() {
        let mut encoder = PatchEncoder::new();
        let mut heights = [0.0f32; 256];
        for i in 0..256 {
            heights[i] = (i as f32).sin() * 10.0 + 50.0;
        }
        encode_patch(&mut encoder, &heights, false);
        let data = encoder.finish();
        assert!(
            !data.is_empty(),
            "varied terrain should produce some output"
        );
    }
}
