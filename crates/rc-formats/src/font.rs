//! The game's three bitmap fonts: glyph tables in the level overlay and the FX textures they index.
//! Spec: docs/plan/hud_text.md §3.1. Snapshot-tested in `tests/formats/golden.rs` (HUD test).
//!
//! A font is an FX texture (`GetEffectTex__Fii(n)`, level01 0x21ae98: n = 1 regular, 2 small, 3 large; 256×128
//! PSMT8, decoded by [`crate::particle_tex`]) plus a 232-entry [`Glyph`] table indexed by the byte value.
//! The tables live in the level overlay's `.data`, **at a different address in every overlay** (level 01:
//! 0x1c35d0 / 0x1c3970 / 0x1c3d10; level 00: 0x1c3150 / …); their bytes are identical in all 19 overlays.
//! The game reaches them only through the font wrappers (`font_print_*` 0x21cf70 / 0x21cff0 / 0x21d070 and
//! their right / centre / window variants), which load the FX texture and the table address and call
//! `FontPrint` (0x21ccf0):
//!
//! ```text
//! jal  GetEffectTex        ; li a0, n  (delay slot)
//! lui  t2, hi(table)
//! ...                      ; argument moves
//! jal  FontPrint           ; addiu t2, t2, lo(table)  (delay slot)
//! ```
//!
//! [`find_glyph_tables`] finds the addresses by that call pattern (the same references the game uses), so no
//! per-level address list is needed.

use crate::buf::{invalid, Buf, Result};
use bytemuck::{Pod, Zeroable};

/// Entries per glyph table (byte values 0x00..=0xe7; accents live at 0xc0..0xe7).
pub const GLYPHS: usize = 232;

/// The fonts, in FX-texture order (`GetEffectTex(n)` = `font as usize + 1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Font {
    /// FX 1: HUD numbers, banners' second line, most menus (Lombyte "font_print_large").
    Regular = 0,
    /// FX 2: help boxes, subtitles.
    Small = 1,
    /// FX 3: banners.
    Large = 2,
}

impl Font {
    /// The FX texture index `GetEffectTex` is called with.
    pub fn fx_texture(self) -> usize { self as usize + 1 }
}

/// `{u8 u, u8 v, s8 y_off, s8 advance}`: atlas cell (16×16 texels; 24×16 for the pad icons 0x10..0x1f) at
/// texel (u, v) of the font's FX texture, drawn at `y + y_off`; the pen then moves by `advance`.
/// `advance == 0` means "no glyph": `FontPrint` skips the byte.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct Glyph {
    pub u: u8,
    pub v: u8,
    pub y_off: i8,
    pub advance: i8,
}

pub type GlyphTable = [Glyph; GLYPHS];

/// `0x16ccb8` (level 01; also per-overlay .data): `FontPrint`'s colour table for the codes 0x08..0x0f, R in the
/// low byte. Entry 0 is overwritten at run time with the caller's colour; a code keeps the current alpha and
/// takes the low 24 bits: 0x09 blue (R 0x70 G 0x70 B 0xe0), 0x0a green, 0x0b purple, 0x0c orange (names in
/// help text), 0x0d..0x0f black. Identical in every overlay (checked by the golden test).
pub const COLOUR_TABLE: [u32; 8] = [0x8000_0000, 0x80e0_7070, 0x8040_a040, 0x80a0_60a0, 0x8040_80c0, 0x8000_0000, 0x8000_0000, 0x8000_0000];

/// One section of a level overlay (`ratchet-executable` layout, docs/formats/wad_layouts_rac1.md §4):
/// `{u32 dest, u32 size, u32 type, u32 entry}` then `size` bytes; the list ends at the first header whose
/// entry point differs from the first one (the game's own termination rule).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlaySection {
    pub dest: u32,
    /// ELF `sh_type`: 1 PROGBITS, 8 NOBITS.
    pub kind: u32,
    pub entry: u32,
    pub data: Vec<u8>,
}

/// Splits the raw `overlay` lump (`LevelFiles::overlay`, `extracted/levels/NN/overlay.bin`) into its sections.
pub fn parse_overlay_sections(overlay: &[u8]) -> Result<Vec<OverlaySection>> {
    let b = Buf(overlay);
    let mut out: Vec<OverlaySection> = Vec::new();
    let mut pos = 0usize;
    while pos + 16 <= overlay.len() {
        let (dest, size, kind, entry) = (b.u32(pos)?, b.u32(pos + 4)? as usize, b.u32(pos + 8)?, b.u32(pos + 12)?);
        if out.first().is_some_and(|f| f.entry != entry) { break; }
        let data = b.sub(pos + 16, size, "overlay section")?.bytes().to_vec();
        out.push(OverlaySection { dest, kind, entry, data });
        pos += 16 + size;
    }
    if out.is_empty() { return invalid("overlay has no sections"); }
    Ok(out)
}

/// `n` bytes at EE address `addr` of the loaded overlay.
pub fn read_overlay(sections: &[OverlaySection], addr: u32, n: usize) -> Option<&[u8]> {
    sections.iter().filter(|s| s.kind != 8).find_map(|s| {
        let off = addr.checked_sub(s.dest)? as usize;
        s.data.get(off..off.checked_add(n)?)
    })
}

const OP_JAL: u32 = 3;

fn is_jal(w: u32) -> bool { w >> 26 == OP_JAL }

/// `addiu a0, zero, n` or `ori a0, zero, n` → n.
fn li_a0(w: u32) -> Option<u32> { (w & 0xffff_0000 == 0x2404_0000 || w & 0xffff_0000 == 0x3404_0000).then_some(w & 0xffff) }

/// The glyph-table address of every font, found from the `FontPrint` call sites (module doc). Errors if a
/// font has no call site or two call sites disagree.
pub fn find_glyph_tables(sections: &[OverlaySection]) -> Result<[u32; 3]> {
    let mut found: [Option<u32>; 3] = [None; 3];
    for s in sections.iter().filter(|s| s.kind == 1) {
        let words: Vec<u32> = s.data.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect();
        for i in 1..words.len() {
            let Some(n) = li_a0(words[i]).filter(|n| (1..=3).contains(n) && is_jal(words[i - 1])) else { continue };
            // lui t2, hi within the next few words, then a jal whose delay slot is addiu t2, t2, lo.
            let Some(l) = (i + 1..(i + 4).min(words.len())).find(|&k| words[k] & 0xffff_0000 == 0x3c0a_0000) else { continue };
            let Some(a) = (l + 1..(l + 12).min(words.len())).find(|&k| words[k] & 0xffff_0000 == 0x254a_0000 && is_jal(words[k - 1])) else {
                continue;
            };
            let addr = ((words[l] & 0xffff) << 16).wrapping_add(words[a] as i16 as i32 as u32);
            let slot = &mut found[n as usize - 1];
            match *slot {
                None => *slot = Some(addr),
                Some(prev) if prev != addr => return invalid(format!("FX {n} call sites disagree on the glyph table ({prev:#x} vs {addr:#x})")),
                _ => {}
            }
        }
    }
    match found {
        [Some(a), Some(b), Some(c)] => Ok([a, b, c]),
        _ => invalid(format!("glyph table call sites not found: {found:x?}")),
    }
}

/// The three glyph tables ([`Font`] order) and their addresses, from the raw overlay lump.
pub fn parse_glyph_tables(overlay: &[u8]) -> Result<([GlyphTable; 3], [u32; 3])> {
    let sections = parse_overlay_sections(overlay)?;
    let addrs = find_glyph_tables(&sections)?;
    let mut out = [[Glyph::default(); GLYPHS]; 3];
    for (t, &a) in out.iter_mut().zip(&addrs) {
        let Some(bytes) = read_overlay(&sections, a, GLYPHS * 4) else { return invalid(format!("glyph table {a:#x} outside the overlay")) };
        t.copy_from_slice(bytemuck::cast_slice(bytes));
    }
    // 'A' has a glyph in every font: a cheap check that the pattern found real tables.
    if out.iter().any(|t| t[b'A' as usize].advance <= 0) { return invalid("glyph tables without an 'A'"); }
    Ok((out, addrs))
}

/// `measure_text_width` (0x21cc40): sum of the non-zero advances of the first `len` bytes (all of them up
/// to the NUL when `len < 0`), colour codes and pad icons included. Bytes ≥ 232 (never in the game's text)
/// would read past the table in the game; here they count 0.
pub fn measure_text_width(text: &[u8], len: i32, table: &GlyphTable) -> i32 {
    let mut w = 0i32;
    for (i, &c) in text.iter().enumerate() {
        if c == 0 || (len >= 0 && i as i32 >= len) || len == 0 { break; }
        w += table.get(c as usize).map_or(0, |g| g.advance as i32);
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(dest: u32, kind: u32, words: &[u32]) -> Vec<u8> {
        let mut v = Vec::new();
        for x in [dest, (words.len() * 4) as u32, kind, 0x1234] { v.extend_from_slice(&x.to_le_bytes()); }
        for w in words { v.extend_from_slice(&w.to_le_bytes()); }
        v
    }

    fn wrapper(n: u32, table: u32) -> Vec<u32> {
        let lo = table & 0xffff;
        let hi = (table >> 16) + (lo >= 0x8000) as u32;
        vec![0x0c08_6ba6, 0x2404_0000 | n, 0x3c0a_0000 | hi, 0x0200_202d, 0x0220_282d, 0x0c08_733c, 0x254a_0000 | lo]
    }

    #[test]
    fn call_pattern_finds_tables_and_sign_extends_lo() {
        let mut code = Vec::new();
        for (n, t) in [(1, 0x1c35d0), (2, 0x1c3970), (3, 0x1c9d10)] { code.extend(wrapper(n, t)); }
        let mut ov = section(0x100000, 1, &code);
        ov.extend(section(0x200000, 8, &[0; 4]));
        let secs = parse_overlay_sections(&ov).unwrap();
        assert_eq!(secs.len(), 2);
        assert_eq!(find_glyph_tables(&secs).unwrap(), [0x1c35d0, 0x1c3970, 0x1c9d10]);
        code.extend(wrapper(1, 0x1c0000));
        assert!(find_glyph_tables(&parse_overlay_sections(&section(0x100000, 1, &code)).unwrap()).is_err(), "disagreeing call sites");
    }

    #[test]
    fn measure_counts_nonzero_advances() {
        let mut t = [Glyph::default(); GLYPHS];
        t[b'a' as usize].advance = 10;
        t[b' ' as usize].advance = 4;
        t[0x0c].advance = 0;
        assert_eq!(measure_text_width(b"a a\x0ca\0aaa", -1, &t), 34);
        assert_eq!(measure_text_width(b"aaaa", 2, &t), 20);
        assert_eq!(measure_text_width(b"aaaa", 0, &t), 0);
    }
}
