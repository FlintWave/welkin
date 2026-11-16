// SPDX-License-Identifier: LGPL-3.0-or-later

pub fn encode_land_layer(terrain: &crate::terrain::TerrainChannel, patches: &[(u32, u32)]) -> Vec<u8> {
    if terrain.size_x() != 256 || terrain.size_y() != 256 {
        return Vec::new();
    }

    let mut writer = BitWriter::new();
    writer.pack_bits(264, 16);
    writer.pack_bits(16, 8);
    writer.pack_bits(0x4C, 8);

    for &(px, py) in patches {
        let (zmin, zmax) = terrain.patch_min_max(px, py);
        let frange = (zmax - zmin) + 1.0f32;

        if (frange * 100.0).round() == 100.0 {
            writer.pack_bits(0, 8);
            writer.pack_bits(f32::to_bits(zmin - 0.5f32), 32);
            writer.pack_bits(1, 8);
            writer.pack_bits(0, 8);
            writer.pack_bits((px << 5) | (py & 0x1F), 10);
            writer.pack_bits(2, 2);
            continue;
        }

        let dc_offset = zmin;
        let range = frange as i32;
        let range_f = range as f32; // C# uses the integer header range here

        let mut block = vec![0.0f32; 256];
        let premult = 1024.0f32 / range_f;
        let sub = 0.5f32 * range_f + dc_offset;
        for ly in 0..16u32 {
            for lx in 0..16u32 {
                let gx = px * 16 + lx;
                let gy = py * 16 + ly;
                let h = terrain.get(gx, gy);
                block[(ly * 16 + lx) as usize] = (h - sub) * premult;
            }
        }

        let (patch_coeffs, mut wbits) = dct_quantize(&block);

        if wbits > 17 {
            wbits = 17;
        } else if wbits < 2 {
            wbits = 2;
        }
        let quant_wbits = 0x80u8 | ((wbits - 2) as u8);

        writer.pack_bits(quant_wbits as u32, 8);
        writer.pack_bits(f32::to_bits(dc_offset), 32);
        writer.pack_bits(range as u32, 16);
        writer.pack_bits((px << 5) | (py & 0x1F), 10);

        let maxwbitssize = (1i32 << wbits) - 1;
        let last_zero_indx = 256usize;
        let mut i = 0usize;
        while i < 256 {
            let mut temp = patch_coeffs[i];
            if temp == 0 {
                let mut j = i + 1;
                while j < last_zero_indx {
                    if patch_coeffs[j] != 0 {
                        break;
                    }
                    j += 1;
                }
                if j == last_zero_indx {
                    writer.pack_bits(2, 2);
                    break;
                }
                let mut zeros = j - i;
                while zeros > 8 {
                    writer.pack_bits(0, 8);
                    zeros -= 8;
                }
                if zeros > 0 {
                    writer.pack_bits(0, zeros as u32);
                }
                i = j;
                continue;
            } else if temp < 0 {
                temp = -temp;
                writer.pack_bits(7, 3);
            } else {
                writer.pack_bits(6, 3);
            }
            let mag = if temp > maxwbitssize {
                maxwbitssize
            } else {
                temp
            };
            writer.pack_bits(mag as u32, wbits as u32);
            i += 1;
        }
    }

    writer.pack_bits(97, 8);
    writer.finish()
}

pub fn decode_land_layer(data: &[u8]) -> Option<Vec<(u32, u32, Vec<f32>)>> {
    let mut reader = BitReader::new(data);

    let _stride = reader.read_bits(16)?;
    let _patch_size = reader.read_bits(8)?;
    let _layer = reader.read_bits(8)?;

    let copy = build_copy_matrix();
    let dequant = build_dequant();
    let cos = build_cos();

    let mut out = Vec::new();

    loop {
        let quant_wbits = reader.read_bits(8)? as u32;
        if quant_wbits == 97 {
            break;
        }

        let dc_offset = f32::from_bits(reader.read_bits(32)?);
        let range = reader.read_bits(16)? as f32;
        let ids = reader.read_bits(10)?;
        let px = (ids >> 5) & 0x1F;
        let py = ids & 0x1F;

        let prequant = (quant_wbits >> 4) + 2;
        let wbits = (quant_wbits & 0x0F) + 2;

        let mut decoded = vec![0i32; 256];
        let mut i = 0usize;
        while i < 256 {
            let first = reader.read_bit()?;
            if first == 0 {
                decoded[i] = 0;
                i += 1;
            } else {
                let second = reader.read_bit()?;
                if second == 0 {
                    break;
                } else {
                    let sign = reader.read_bit()?;
                    let mag = reader.read_bits(wbits)? as i32;
                    decoded[i] = if sign == 1 { -mag } else { mag };
                    i += 1;
                }
            }
        }

        let mut block = vec![0.0f32; 256];
        for indx in 0..256 {
            block[indx] = decoded[copy[indx] as usize] as f32 * dequant[indx];
        }

        let mut tmp = vec![0.0f32; 256];
        let oo = std::f32::consts::FRAC_1_SQRT_2;
        for c in 0..16usize {
            for n in 0..16usize {
                let mut total = oo * block[c];
                for u in 1..16usize {
                    total += block[u * 16 + c] * cos[u * 16 + n];
                }
                tmp[n * 16 + c] = total;
            }
        }
        let mut idct = vec![0.0f32; 256];
        for l in 0..16usize {
            for n in 0..16usize {
                let mut total = oo * tmp[l * 16];
                for u in 1..16usize {
                    total += tmp[l * 16 + u] * cos[u * 16 + n];
                }
                idct[l * 16 + n] = total * (2.0 / 16.0);
            }
        }

        let mult = (1.0 / ((1u32 << prequant) as f32)) * range;
        let addval = mult * ((1u32 << (prequant - 1)) as f32) + dc_offset;
        let mut heights = vec![0.0f32; 256];
        for idx in 0..256 {
            heights[idx] = idct[idx] * mult + addval;
        }

        out.push((px, py, heights));
    }

    Some(out)
}

struct BitWriter {
    data: Vec<u8>,
    current: u8,
    bits: u8,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            data: Vec::new(),
            current: 0,
            bits: 0,
        }
    }

    fn push_bit(&mut self, bit: bool) {
        if self.bits == 8 {
            self.data.push(self.current);
            self.current = 0;
            self.bits = 0;
        }
        if bit {
            self.current |= 1 << (7 - self.bits);
        }
        self.bits += 1;
        if self.bits == 8 {
            self.data.push(self.current);
            self.current = 0;
            self.bits = 0;
        }
    }

    fn pack_bits(&mut self, value: u32, count: u32) {
        if count == 0 {
            return;
        }
        let mut remaining = count;
        for byte in value.to_le_bytes() {
            if remaining == 0 {
                break;
            }
            let n = remaining.min(8);
            let mask = if n == 8 {
                0xFFu8
            } else {
                ((1u32 << n) - 1) as u8
            };
            let bits_val = byte & mask;
            for shift in (0..n).rev() {
                self.push_bit(((bits_val >> shift) & 1) == 1);
            }
            remaining -= n;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.data.push(self.current);
        }
        self.data
    }
}

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            bit: 0,
        }
    }

    fn read_bit(&mut self) -> Option<u8> {
        if self.pos >= self.data.len() {
            return None;
        }
        if self.bit == 8 {
            self.pos += 1;
            self.bit = 0;
            if self.pos >= self.data.len() {
                return None;
            }
        }
        let shift = 7 - self.bit;
        let b = (self.data[self.pos] >> shift) & 1;
        self.bit += 1;
        Some(b)
    }

    fn read_bits(&mut self, count: u32) -> Option<u32> {
        let mut remaining = count;
        let mut bytes = [0u8; 4];
        let mut bi = 0usize;

        while remaining > 0 {
            let n = remaining.min(8);
            let mut chunk = 0u32;
            for _ in 0..n {
                let b = self.read_bit()?;
                chunk = (chunk << 1) | (b as u32);
            }
            let mask = if n == 8 {
                0xFFu8
            } else {
                ((1u32 << n) - 1) as u8
            };
            if bi >= bytes.len() {
                return None;
            }
            bytes[bi] = (chunk as u8) & mask;
            remaining -= n;
            bi += 1;
        }

        Some(u32::from_le_bytes(bytes))
    }
}

fn build_copy_matrix() -> [i32; 256] {
    let mut copy = [0i32; 256];
    let mut diag = false;
    let mut right = true;
    let mut i = 0i32;
    let mut j = 0i32;
    let mut count = 0i32;

    while i < 16 && j < 16 {
        copy[(j * 16 + i) as usize] = count;
        count += 1;

        if !diag {
            if right {
                if i < 15 {
                    i += 1;
                } else {
                    j += 1;
                }
                right = false;
                diag = true;
            } else {
                if j < 15 {
                    j += 1;
                } else {
                    i += 1;
                }
                right = true;
                diag = true;
            }
        } else {
            if right {
                i += 1;
                j -= 1;
                if i == 15 || j == 0 {
                    diag = false;
                }
            } else {
                i -= 1;
                j += 1;
                if j == 15 || i == 0 {
                    diag = false;
                }
            }
        }
    }

    copy
}

fn build_quant() -> [f32; 256] {
    let mut quant = [0.0f32; 256];
    for j in 0..16usize {
        for i in 0..16usize {
            let tmp = 1.0f32 + 2.0f32 * ((i + j) as f32);
            quant[j * 16 + i] = (2.0f32 / 16.0f32) / tmp;
        }
    }
    quant
}

fn build_dequant() -> [f32; 256] {
    let mut dequant = [0.0f32; 256];
    for j in 0..16usize {
        for i in 0..16usize {
            dequant[j * 16 + i] = 1.0f32 + 2.0f32 * ((i + j) as f32);
        }
    }
    dequant
}

fn build_cos() -> [f32; 256] {
    let mut cos = [0.0f32; 256];
    for u in 0..16usize {
        for n in 0..16usize {
            let val = (core::f64::consts::PI
                * ((2 * n + 1) as f64)
                * (u as f64)
                / 32.0)
            .cos() as f32;
            cos[u * 16 + n] = val;
        }
    }
    cos
}

fn dct_quantize(block: &[f32]) -> (Vec<i32>, i32) {
    let quant = build_quant();
    let copy = build_copy_matrix();

    // Separable 2-D DCT with a precomputed basis: basis[k*16+n] = cos(pi*(n+0.5)*k/16).
    let mut basis = [0.0f32; 256];
    for k in 0..16usize {
        for n in 0..16usize {
            basis[k * 16 + n] =
                (core::f64::consts::PI * (n as f64 + 0.5) * (k as f64) / 16.0).cos() as f32;
        }
    }
    // Rows first: tmp[y*16+kx] = sum_x a[y*16+x] * basis[kx*16+x]
    let mut tmp = [0.0f32; 256];
    for y in 0..16usize {
        for kx in 0..16usize {
            let mut sum = 0.0f32;
            for x in 0..16usize {
                sum += block[y * 16 + x] * basis[kx * 16 + x];
            }
            tmp[y * 16 + kx] = sum;
        }
    }
    let mut coeff = vec![0.0f32; 256];
    let s0 = 1.0f32 / (2.0f32).sqrt();
    for ky in 0..16usize {
        for kx in 0..16usize {
            let mut sum = 0.0f32;
            for y in 0..16usize {
                sum += tmp[y * 16 + kx] * basis[ky * 16 + y];
            }
            let s_kx = if kx == 0 { s0 } else { 1.0 };
            let s_ky = if ky == 0 { s0 } else { 1.0 };
            // The 1/8 of the Ooura definition lives in QuantizeTable16 (2/16 per coefficient).
            coeff[ky * 16 + kx] = s_kx * s_ky * sum;
        }
    }

    let mut iout = vec![0i32; 256];
    let mut wbits = 5i32;
    for ky in 0..16u32 {
        for kx in 0..16u32 {
            let indx = (ky * 16 + kx) as usize;
            let itmp = (coeff[indx] * quant[indx]) as i32;
            iout[copy[indx] as usize] = itmp;
            let ab = itmp.abs();
            while wbits < 17 && ab > (1 << wbits) {
                wbits += 1;
            }
        }
    }

    (iout, wbits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::TerrainChannel;

    #[test]
    fn roundtrip_smooth() {
        let mut t = TerrainChannel::new(256, 256);
        let px = 2u32;
        let py = 3u32;
        for ly in 0..16u32 {
            for lx in 0..16u32 {
                let h = 5.0 + 0.25 * (lx as f32) + 0.125 * (ly as f32);
                t.set(px * 16 + lx, py * 16 + ly, h);
            }
        }
        let encoded = encode_land_layer(&t, &[(px, py)]);
        let decoded = decode_land_layer(&encoded).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, px);
        assert_eq!(decoded[0].1, py);
        let mut maxerr = 0.0f32;
        for i in 0..256 {
            let err = (decoded[0].2[i] - (5.0 + 0.25 * ((i % 16) as f32) + 0.125 * ((i / 16) as f32))).abs();
            maxerr = maxerr.max(err);
        }
        assert!(maxerr <= 0.1, "maxerr={}", maxerr);
    }

    #[test]
    fn flat_patch() {
        let mut t = TerrainChannel::new(256, 256);
        let px = 1u32;
        let py = 1u32;
        for ly in 0..16u32 {
            for lx in 0..16u32 {
                t.set(px * 16 + lx, py * 16 + ly, 7.0);
            }
        }
        let encoded = encode_land_layer(&t, &[(px, py)]);
        let decoded = decode_land_layer(&encoded).unwrap();
        assert_eq!(decoded.len(), 1);
        for h in &decoded[0].2 {
            assert!((h - 7.0).abs() <= 0.01, "h={}", h);
        }
    }

    #[test]
    fn patch_ids() {
        let t = TerrainChannel::new(256, 256);
        let patches = vec![(15u32, 15u32), (0u32, 0u32), (3u32, 7u32)];
        let encoded = encode_land_layer(&t, &patches);
        let decoded = decode_land_layer(&encoded).unwrap();
        assert_eq!(decoded.len(), patches.len());
        for (i, (px, py)) in patches.iter().enumerate() {
            assert_eq!(decoded[i].0, *px);
            assert_eq!(decoded[i].1, *py);
        }
    }

    #[test]
    fn multiple_roundtrip() {
        let mut t = TerrainChannel::new(256, 256);
        let patches = vec![(0u32, 0u32), (1u32, 0u32)];
        for (px, py) in &patches {
            for ly in 0..16u32 {
                for lx in 0..16u32 {
                    let h = *px as f32 * 10.0 + *py as f32 * 5.0 + lx as f32 * 0.25 + ly as f32 * 0.5;
                    t.set(px * 16 + lx, py * 16 + ly, h);
                }
            }
        }
        let encoded = encode_land_layer(&t, &patches);
        let decoded = decode_land_layer(&encoded).unwrap();
        assert_eq!(decoded.len(), patches.len());
        for (i, (px, py)) in patches.iter().enumerate() {
            assert_eq!(decoded[i].0, *px);
            assert_eq!(decoded[i].1, *py);
            let mut maxerr = 0.0f32;
            for ly in 0..16u32 {
                for lx in 0..16u32 {
                    let idx = (ly * 16 + lx) as usize;
                    let orig = *px as f32 * 10.0
                        + *py as f32 * 5.0
                        + lx as f32 * 0.25
                        + ly as f32 * 0.5;
                    maxerr = maxerr.max((decoded[i].2[idx] - orig).abs());
                }
            }
            assert!(maxerr <= 0.1, "maxerr={}", maxerr);
        }
    }

    #[test]
    fn steep_random() {
        let mut t = TerrainChannel::new(256, 256);
        let px = 4u32;
        let py = 5u32;
        for ly in 0..16u32 {
            for lx in 0..16u32 {
                let h = (((lx as i32) * 53 + (ly as i32) * 97 + (lx as i32) * (ly as i32) * 17) % 201) as f32;
                t.set(px * 16 + lx, py * 16 + ly, h);
            }
        }
        let encoded = encode_land_layer(&t, &[(px, py)]);
        let decoded = decode_land_layer(&encoded).unwrap();
        let mut maxerr = 0.0f32;
        for ly in 0..16u32 {
            for lx in 0..16u32 {
                let idx = (ly * 16 + lx) as usize;
                let orig = (((lx as i32) * 53 + (ly as i32) * 97 + (lx as i32) * (ly as i32) * 17) % 201) as f32;
                maxerr = maxerr.max((decoded[0].2[idx] - orig).abs());
            }
        }
        // The format is lossy: error scales with the patch height range (200 m here).
        assert!(maxerr <= 12.0, "maxerr={}", maxerr);
    }

    #[test]
    fn non_256_returns_empty() {
        let t = TerrainChannel::new(128, 128);
        let out = encode_land_layer(&t, &[(0, 0)]);
        assert!(out.is_empty());
    }
}

