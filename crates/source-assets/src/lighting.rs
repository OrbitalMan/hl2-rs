//! Packs each face's primary RGBExp32 baked light samples into GPU atlas pages.
use crate::{bytes, i32le, records};
use anyhow::{bail, Result};
use modkit_core::Lightmap;
const SIZE: usize = 1024;
#[derive(Clone, Copy)]
pub struct FaceLight {
    pub page: usize,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}
pub fn build(lumps: &[Vec<u8>]) -> Result<(Vec<Lightmap>, Vec<Option<FaceLight>>)> {
    let hdr = lumps[7].is_empty();
    let data = &lumps[if hdr { 53 } else { 8 }];
    let faces = records(&lumps[if hdr { 58 } else { 7 }], 56)?;
    let mut pages = Vec::<Lightmap>::new();
    let mut refs = Vec::new();
    let (mut x, mut y, mut row) = (1usize, 1usize, 0usize);
    for face in faces {
        let offset = i32le(face, 20)?;
        if offset < 0 || data.is_empty() {
            refs.push(None);
            continue;
        }
        let width = usize::try_from(i32le(face, 36)?.max(0))? + 1;
        let height = usize::try_from(i32le(face, 40)?.max(0))? + 1;
        if width + 2 > SIZE || height + 2 > SIZE {
            refs.push(None);
            continue;
        }
        let samples = bytes(data, offset as usize, width * height * 4)?;
        if x + width + 1 > SIZE {
            x = 1;
            y += row + 2;
            row = 0;
        }
        if pages.is_empty() || y + height + 1 > SIZE {
            if pages.len() >= 64 {
                bail!("lightmap atlas exceeds 64 pages");
            }
            pages.push(Lightmap {
                width: SIZE as u16,
                height: SIZE as u16,
                rgba: vec![255; SIZE * SIZE * 4],
            });
            x = 1;
            y = 1;
            row = 0;
        }
        let page = pages.len() - 1;
        let atlas = &mut pages[page];
        for dy in 0..height {
            for dx in 0..width {
                let s = &samples[(dy * width + dx) * 4..][..4];
                let scale = 2f32.powi(s[3] as i8 as i32);
                let target = ((y + dy) * SIZE + x + dx) * 4;
                for (i, &channel) in s.iter().take(3).enumerate() {
                    atlas.rgba[target + i] =
                        ((channel as f32 * scale / 255.).max(0.).powf(1. / 2.2) * 255.).min(255.)
                            as u8;
                }
                atlas.rgba[target + 3] = 255;
            }
        }
        // Duplicate edge texels to prevent filtering from leaking into neighboring faces.
        for dy in 0..height {
            for (source, dest) in [(x, x - 1), (x + width - 1, x + width)] {
                let color: [u8; 4] =
                    atlas.rgba[((y + dy) * SIZE + source) * 4..][..4].try_into()?;
                atlas.rgba[((y + dy) * SIZE + dest) * 4..][..4].copy_from_slice(&color);
            }
        }
        for dx in 0..width + 2 {
            for (source, dest) in [(y, y - 1), (y + height - 1, y + height)] {
                let color: [u8; 4] =
                    atlas.rgba[(source * SIZE + x - 1 + dx) * 4..][..4].try_into()?;
                atlas.rgba[(dest * SIZE + x - 1 + dx) * 4..][..4].copy_from_slice(&color);
            }
        }
        refs.push(Some(FaceLight {
            page,
            x,
            y,
            width,
            height,
        }));
        x += width + 2;
        row = row.max(height);
    }
    Ok((pages, refs))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgbexp_and_border() {
        let mut lumps = vec![vec![]; 64];
        let mut face = vec![0u8; 56];
        face[36..40].copy_from_slice(&1i32.to_le_bytes());
        lumps[7] = face;
        lumps[8] = vec![255, 0, 0, 0, 0, 255, 0, 0];
        let (pages, refs) = build(&lumps).unwrap();
        let r = refs[0].unwrap();
        let p = &pages[r.page];
        assert_eq!(&p.rgba[(r.y * SIZE + r.x) * 4..][..4], &[255, 0, 0, 255]);
        assert_eq!(
            &p.rgba[(r.y * SIZE + r.x - 1) * 4..][..4],
            &[255, 0, 0, 255]
        );
    }
}
