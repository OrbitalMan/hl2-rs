//! VTF 7.0-7.5 texture reader; RGBA/BGRA/RGB/BGR/BGRX and DXT1/3/5.
use crate::{bytes, u16le, u32le};
use anyhow::{bail, Context, Result};
#[derive(Debug)]
pub struct Image {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
    pub format: u32,
}
fn size(format: u32, w: usize, h: usize) -> Result<usize> {
    Ok(match format {
        0 | 1 | 12 | 16 => w * h * 4,
        2 | 3 => w * h * 3,
        4 => w * h * 2,
        13 | 20 => w.div_ceil(4) * h.div_ceil(4) * 8,
        14 | 15 => w.div_ceil(4) * h.div_ceil(4) * 16,
        _ => bail!("unsupported VTF image format {format}"),
    })
}
fn rgb565(v: u16) -> [u8; 4] {
    let r = ((v >> 11) & 31) as u32;
    let g = ((v >> 5) & 63) as u32;
    let b = (v & 31) as u32;
    [
        (r * 255 / 31) as u8,
        (g * 255 / 63) as u8,
        (b * 255 / 31) as u8,
        255,
    ]
}
fn color_block(block: &[u8], force_four: bool) -> Result<[[u8; 4]; 16]> {
    let c0 = u16le(block, 0)?;
    let c1 = u16le(block, 2)?;
    let mut colors = [rgb565(c0), rgb565(c1), [0; 4], [0; 4]];
    let a = colors[0];
    let b = colors[1];
    if c0 > c1 || force_four {
        for (i, (&x, &y)) in a[..3].iter().zip(&b[..3]).enumerate() {
            colors[2][i] = ((2 * x as u16 + y as u16) / 3) as u8;
            colors[3][i] = ((x as u16 + 2 * y as u16) / 3) as u8;
        }
        colors[2][3] = 255;
        colors[3][3] = 255;
    } else {
        for (i, (&x, &y)) in a[..3].iter().zip(&b[..3]).enumerate() {
            colors[2][i] = ((x as u16 + y as u16) / 2) as u8;
        }
        colors[2][3] = 255;
    }
    let bits = u32le(block, 4)?;
    let mut pixels = [[0; 4]; 16];
    for i in 0..16 {
        pixels[i] = colors[((bits >> (2 * i)) & 3) as usize];
    }
    Ok(pixels)
}
pub fn decode(data: &[u8], max_dimension: usize) -> Result<Image> {
    if bytes(data, 0, 4)? != b"VTF\0" {
        bail!("not a VTF texture");
    }
    let major = u32le(data, 4)?;
    let minor = u32le(data, 8)?;
    if major != 7 || minor > 5 {
        bail!("unsupported VTF {major}.{minor}");
    }
    let header = u32le(data, 12)? as usize;
    bytes(data, 0, header)?;
    let full_w = u16le(data, 16)? as usize;
    let full_h = u16le(data, 18)? as usize;
    if full_w == 0 || full_h == 0 || full_w > 16384 || full_h > 16384 {
        bail!("invalid VTF dimensions");
    }
    let flags = u32le(data, 20)?;
    let frames = u16le(data, 24)? as usize;
    if frames == 0 || frames > 4096 {
        bail!("invalid VTF frame count");
    }
    let format = u32le(data, 52)?;
    let mip_count = *bytes(data, 56, 1)?.first().unwrap() as usize;
    if mip_count == 0 || mip_count > 15 {
        bail!("invalid VTF mip count");
    }
    let depth = if minor >= 2 {
        u16le(data, 63)? as usize
    } else {
        1
    };
    if depth != 1 {
        bail!("volume VTF is unsupported");
    }
    let faces = if flags & 0x4000 != 0 {
        if minor < 5 && u16le(data, 26)? != 0xffff {
            7
        } else {
            6
        }
    } else {
        1
    };
    let mut image_offset = None;
    if minor >= 3 {
        let resources = u32le(data, 68)? as usize;
        if resources > 32 {
            bail!("invalid VTF resource count");
        }
        for i in 0..resources {
            let o = 80 + i * 8;
            let id = bytes(data, o, 4)?;
            if id[..3] == [0x30, 0, 0] {
                if id[3] & 2 != 0 {
                    bail!("inline image resource unsupported");
                }
                image_offset = Some(u32le(data, o + 4)? as usize);
            }
        }
    }
    let mut offset = if let Some(o) = image_offset {
        o
    } else {
        let lowformat = u32le(data, 57)?;
        let w = bytes(data, 61, 1)?[0] as usize;
        let h = bytes(data, 62, 1)?[0] as usize;
        header
            + if w == 0 || h == 0 {
                0
            } else {
                size(lowformat, w, h)?
            }
    };
    let mut mip = 0;
    while mip + 1 < mip_count && (full_w >> mip).max(full_h >> mip) > max_dimension.max(1) {
        mip += 1;
    }
    for level in ((mip + 1)..mip_count).rev() {
        let w = (full_w >> level).max(1);
        let h = (full_h >> level).max(1);
        offset = offset
            .checked_add(
                size(format, w, h)?
                    .checked_mul(frames * faces)
                    .context("VTF mip size overflow")?,
            )
            .context("VTF offset overflow")?;
    }
    let w = (full_w >> mip).max(1);
    let h = (full_h >> mip).max(1);
    let image = bytes(data, offset, size(format, w, h)?)?;
    let mut rgba = vec![0; w * h * 4];
    match format {
        0 | 1 | 12 | 16 | 2 | 3 | 4 => {
            let stride = match format {
                2 | 3 => 3,
                4 => 2,
                _ => 4,
            };
            for i in 0..w * h {
                let p = &image[i * stride..(i + 1) * stride];
                let v = match format {
                    0 => [p[0], p[1], p[2], p[3]],
                    1 => [p[3], p[2], p[1], p[0]],
                    12 => [p[2], p[1], p[0], p[3]],
                    16 => [p[2], p[1], p[0], 255],
                    2 => [p[0], p[1], p[2], 255],
                    3 => [p[2], p[1], p[0], 255],
                    4 => rgb565(u16::from_le_bytes([p[0], p[1]])),
                    _ => unreachable!(),
                };
                rgba[i * 4..i * 4 + 4].copy_from_slice(&v);
            }
        }
        13 | 20 | 14 | 15 => {
            let blocksize = if format == 13 || format == 20 { 8 } else { 16 };
            for by in 0..h.div_ceil(4) {
                for bx in 0..w.div_ceil(4) {
                    let b = &image[(by * w.div_ceil(4) + bx) * blocksize..][..blocksize];
                    let mut pixels =
                        color_block(if blocksize == 8 { b } else { &b[8..] }, blocksize == 16)?;
                    if format == 14 {
                        for i in 0..16 {
                            pixels[i][3] = ((b[i / 2] >> (4 * (i % 2))) & 15) * 17;
                        }
                    }
                    if format == 15 {
                        let mut a = [0u8; 8];
                        a[0] = b[0];
                        a[1] = b[1];
                        if a[0] > a[1] {
                            for i in 2..8 {
                                a[i] =
                                    (((8 - i) * a[0] as usize + (i - 1) * a[1] as usize) / 7) as u8;
                            }
                        } else {
                            for i in 2..6 {
                                a[i] =
                                    (((6 - i) * a[0] as usize + (i - 1) * a[1] as usize) / 5) as u8;
                            }
                            a[6] = 0;
                            a[7] = 255;
                        }
                        let mut bits = 0u64;
                        for i in 0..6 {
                            bits |= (b[i + 2] as u64) << (8 * i);
                        }
                        for i in 0..16 {
                            pixels[i][3] = a[((bits >> (3 * i)) & 7) as usize];
                        }
                    }
                    for dy in 0..4 {
                        for dx in 0..4 {
                            let x = bx * 4 + dx;
                            let y = by * 4 + dy;
                            if x < w && y < h {
                                rgba[(y * w + x) * 4..(y * w + x) * 4 + 4]
                                    .copy_from_slice(&pixels[dy * 4 + dx]);
                            }
                        }
                    }
                }
            }
        }
        _ => bail!("unsupported VTF format {format}"),
    }
    Ok(Image {
        width: w as u16,
        height: h as u16,
        rgba,
        format,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dxt1_red_block() {
        let b = [0, 248, 0, 0, 0, 0, 0, 0];
        let p = color_block(&b, false).unwrap();
        assert_eq!(p[0], [255, 0, 0, 255]);
    }
    #[test]
    fn dxt1_transparency() {
        let b = [0, 0, 0, 248, 255, 255, 255, 255];
        let p = color_block(&b, false).unwrap();
        assert_eq!(p[0][3], 0);
    }
    #[test]
    fn truncated_vtf_is_error() {
        assert!(decode(b"VTF\0", 512).is_err());
    }
    #[test]
    fn rgba_fixture() {
        let mut d = vec![0; 68];
        d[..4].copy_from_slice(b"VTF\0");
        d[4..8].copy_from_slice(&7u32.to_le_bytes());
        d[8..12].copy_from_slice(&1u32.to_le_bytes());
        d[12..16].copy_from_slice(&64u32.to_le_bytes());
        d[16..18].copy_from_slice(&1u16.to_le_bytes());
        d[18..20].copy_from_slice(&1u16.to_le_bytes());
        d[24..26].copy_from_slice(&1u16.to_le_bytes());
        d[56] = 1;
        d[64..68].copy_from_slice(&[11, 22, 33, 255]);
        assert_eq!(decode(&d, 512).unwrap().rgba, vec![11, 22, 33, 255]);
    }
}
