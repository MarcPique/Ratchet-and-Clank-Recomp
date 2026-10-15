//! PIF images ("2FIP"): the pictures the menus stream from the global lumps (`item_images`, `help_ss`,
//! `help_controls`, `help_moves`, `help_weapons`, `help_gadgets`, `goodies_images`, `skill_images`, `options_ss`,
//! `mission_ss`, `planets`, the in-game maps `unknown_0820`), each file WAD-compressed.
//!
//! Layout (read by the texture setup `fun_00204cf0` / 0x259a38 and the map composer `fun_00204e30` / 0x259b78):
//! `+0 "2FIP"`, `+4 u32` size, `+8 u32` width, `+0xc u32` height, `+0x10 u32` format (0x13 = PSMT8 with a CT32
//! palette), `+0x20` the 256-entry palette (0x400 bytes, CSM1 order), `+0x420` the 8-bit pixels, row major. The GS
//! TEX0 the game builds: TW = log2 width, TH = log2 height, PSM 0x13, CPSM 0 (CT32), CSM 0 (CSM1).
//!
//! The in-game map files are not one PIF but a set of them (`crate`'s map module reads those: docs/plan/menus.md
//! §11); [`Pif::parse_at`] reads one PIF at an offset.

use crate::buf::{invalid, Buf, Result};
use crate::texture::Texture;

pub const MAGIC: &[u8; 4] = b"2FIP";
/// PSMT8 with a CT32 palette.
pub const FORMAT_PSMT8: u32 = 0x13;
pub const PALETTE_OFFSET: usize = 0x20;
pub const PIXELS_OFFSET: usize = 0x420;

/// One PIF image (borrowed from its file).
#[derive(Clone, Copy, Debug)]
pub struct Pif<'a> {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    /// 0x400 bytes, CSM1 order.
    pub palette: &'a [u8],
    /// `width · height` indices.
    pub pixels: &'a [u8],
}

impl<'a> Pif<'a> {
    /// The PIF at the start of `b` (a decompressed lump).
    pub fn parse(b: &'a [u8]) -> Result<Pif<'a>> { Pif::parse_at(b, 0) }

    /// The PIF at byte `at` of `b`.
    pub fn parse_at(b: &'a [u8], at: usize) -> Result<Pif<'a>> {
        let g = Buf(b);
        if g.sub(at, 4, "PIF magic")?.bytes() != MAGIC { return invalid(format!("no PIF at {at:#x}")); }
        let (width, height, format) = (g.u32(at + 8)?, g.u32(at + 0xc)?, g.u32(at + 0x10)?);
        if format != FORMAT_PSMT8 { return invalid(format!("PIF at {at:#x}: format {format:#x} is not PSMT8")); }
        if !width.is_power_of_two() || !height.is_power_of_two() || width > 1024 || height > 1024 {
            return invalid(format!("PIF at {at:#x}: {width}x{height}"));
        }
        let palette = g.sub(at + PALETTE_OFFSET, 0x400, "PIF palette")?.bytes();
        let pixels = g.sub(at + PIXELS_OFFSET, (width * height) as usize, "PIF pixels")?.bytes();
        Ok(Pif { width, height, format, palette, pixels })
    }

    /// RGBA with the GS alpha kept raw (0..0x80), as the HUD atlas samples it (`hud::decode_indexed8_raw`).
    pub fn decode_raw(&self) -> Result<Texture> { crate::hud::decode_indexed8_raw(self.pixels, self.width, self.height, self.palette) }

    /// RGBA with the alpha scaled to 0..0xff (`texture::decode_indexed8`), for inspection.
    pub fn decode(&self) -> Result<Texture> { crate::texture::decode_indexed8(self.pixels, self.width, self.height, self.palette) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_synthetic_pif() {
        let mut b = vec![0u8; PIXELS_OFFSET + 4];
        b[..4].copy_from_slice(MAGIC);
        b[8..12].copy_from_slice(&2u32.to_le_bytes());
        b[12..16].copy_from_slice(&2u32.to_le_bytes());
        b[16..20].copy_from_slice(&FORMAT_PSMT8.to_le_bytes());
        b[PALETTE_OFFSET + 4..PALETTE_OFFSET + 8].copy_from_slice(&[10, 20, 30, 0x80]);
        b[PIXELS_OFFSET] = 1;
        let p = Pif::parse(&b).unwrap();
        assert_eq!((p.width, p.height), (2, 2));
        let t = p.decode_raw().unwrap();
        assert_eq!(&t.rgba[..4], &[10, 20, 30, 0x80]);
        assert!(Pif::parse(&b[4..]).is_err());
        b[16] = 0x14;
        assert!(Pif::parse(&b).is_err());
    }
}
