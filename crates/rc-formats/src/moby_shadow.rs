//! Moby class shadow blocks: the proxy shapes a casting class projects as its dynamic shadow. Spec and the
//! whole shadow system: `docs/plan/shadows.md` §2. The layout is read from the only consumer, *BuildShadowList*
//! (level01 0x29b948, boot 0x227740), and checked on every class of the disc (`tests/formats/moby_shadow_disc.rs`).
//!
//! **Where.** Class header byte 0x0f is the block size in quadwords; the block ends at the skeleton (class
//! +0x14): it starts at `class[0x14] − 16·class[0x0f]`. A class with 0x0f = 0 casts no shadow
//! (`InitMobyInstance` sets mode 0x400 only for the others).
//!
//! **Records**, walked until the one whose `last` byte is set (the size in the header is not read by the game;
//! the walk advances 0x20 for type 0 and 0x30 for any other type):
//!
//! | Off | Type | Sphere (type 0, 0x20 bytes) | Capsule (type 1, 0x30 bytes) |
//! |---|---|---|---|
//! | 0x00 | u8 | type 0 | type 1 |
//! | 0x01 | u8 | last (1 = final record) | last |
//! | 0x02 | u16 | size 0x20 | size 0x30 |
//! | 0x04 | | `s32` joint | `u16` joint A, `u16` joint B (+6) |
//! | 0x08 | s32 | outline segments | segments at end A |
//! | 0x0c | s32 | – | segments at end B |
//! | 0x10 | vec4 | centre xyz (joint space, packed model units), w = radius | end A (w = radius at A) |
//! | 0x20 | vec4 | – | end B (w = radius at B) |
//!
//! A capsule end with 0 segments is flat (two outline points); a negative count −k moves that end **away from**
//! the other one by `k·16/4096` of the segment and makes it flat (0x29bb98: `A += (A − B)·(−k·16/4096)`).
//!
//! Disc facts (all 19 levels, `tests/formats/moby_shadow_disc.rs`): 154 class entries with a block (79 distinct classes),
//! 1,713 records: 1,668 capsules and 45 spheres; every block ends exactly on its `last` record; every joint is below
//! the class joint count; segment counts are −10, 0 or 4..=16.

use crate::buf::{invalid, Buf, Result};
use crate::level::LevelCore;

/// One proxy shape. Points are in the joint's space, packed model units (the class scale applies at run time
/// through moby +0x2c); w is the radius in the same units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShadowPrim {
    /// Type 0: a disc of `radius = centre.w` about `centre` on `joint`, outlined with `segments` points.
    Sphere { joint: i32, segments: i32, centre: [f32; 4] },
    /// Type 1: a tapered capsule from `a` on `joints[0]` to `b` on `joints[1]` (w = the radius at each end), with
    /// `segments[k]` outline points on the half-arc of end k (0 = flat, −k = flat and pushed out, module docs).
    Capsule { joints: [u16; 2], segments: [i32; 2], a: [f32; 4], b: [f32; 4] },
}

impl ShadowPrim {
    /// Record size in the block (0x20 / 0x30).
    pub fn size(&self) -> usize {
        match self {
            ShadowPrim::Sphere { .. } => 0x20,
            ShadowPrim::Capsule { .. } => 0x30,
        }
    }

    /// The joints the record reads.
    pub fn joints(&self) -> Vec<u32> {
        match *self {
            ShadowPrim::Sphere { joint, .. } => vec![joint as u32],
            ShadowPrim::Capsule { joints, .. } => vec![joints[0] as u32, joints[1] as u32],
        }
    }

    /// The segment counts the record carries.
    pub fn segments(&self) -> Vec<i32> {
        match *self {
            ShadowPrim::Sphere { segments, .. } => vec![segments],
            ShadowPrim::Capsule { segments, .. } => segments.to_vec(),
        }
    }
}

/// A class's shadow block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShadowBlock {
    pub prims: Vec<ShadowPrim>,
}

impl ShadowBlock {
    /// Parses the records of a block (`bytes` = the block, `16·class[0x0f]` bytes). Rejects a record type other
    /// than 0 / 1, a size word that disagrees with the type, a record past the block, and a block that does not
    /// end exactly on its `last` record.
    pub fn parse(bytes: &[u8]) -> Result<ShadowBlock> {
        let b = Buf(bytes);
        let mut prims = Vec::new();
        let mut at = 0usize;
        loop {
            let (ty, last, size) = (b.u8(at)?, b.u8(at + 1)?, b.u16(at + 2)? as usize);
            let v4 = |o: usize| -> Result<[f32; 4]> { Ok([b.f32(o)?, b.f32(o + 4)?, b.f32(o + 8)?, b.f32(o + 12)?]) };
            let p = match ty {
                0 => ShadowPrim::Sphere { joint: b.i32(at + 4)?, segments: b.i32(at + 8)?, centre: v4(at + 0x10)? },
                1 => ShadowPrim::Capsule {
                    joints: [b.u16(at + 4)?, b.u16(at + 6)?],
                    segments: [b.i32(at + 8)?, b.i32(at + 0xc)?],
                    a: v4(at + 0x10)?,
                    b: v4(at + 0x20)?,
                },
                t => return invalid(format!("moby shadow: record {} at {at:#x}: type {t}", prims.len())),
            };
            if size != p.size() { return invalid(format!("moby shadow: record {} at {at:#x}: size {size:#x} for type {ty}", prims.len())); }
            at += p.size();
            prims.push(p);
            if last != 0 { break; }
        }
        if at != bytes.len() { return invalid(format!("moby shadow: last record ends at {at:#x} in a {:#x}-byte block", bytes.len())); }
        Ok(ShadowBlock { prims })
    }

    /// The block of a class blob, `None` when class byte 0x0f is 0.
    pub fn of_class(class_blob: &[u8]) -> Result<Option<ShadowBlock>> {
        let b = Buf(class_blob);
        let qw = b.u8(0xf)? as usize;
        if qw == 0 { return Ok(None); }
        let skeleton = b.i32(0x14)?;
        let Some(start) = usize::try_from(skeleton).ok().and_then(|s| s.checked_sub(16 * qw)) else {
            return invalid(format!("moby shadow: {qw} quadwords before the skeleton at {skeleton:#x}"));
        };
        Ok(Some(ShadowBlock::parse(b.sub(start, 16 * qw, "moby shadow block")?.bytes())?))
    }
}

/// Every class of a level with a shadow block, `(o_class, joint count, block)` in core-index order.
pub fn parse_level(core: &LevelCore, core_data: &[u8]) -> Result<Vec<(i32, u8, ShadowBlock)>> {
    let mut out = Vec::new();
    for e in core.moby_classes.iter().filter(|e| e.offset_in_asset_wad > 0) {
        let Some(blk) = core.blocks.iter().find(|b| b.offset == e.offset_in_asset_wad as usize && b.name.starts_with("moby_class/")) else {
            return invalid(format!("moby class {}: no core block", e.o_class));
        };
        let blob = Buf(core_data).sub(blk.offset, blk.size, "moby class blob")?;
        let s = ShadowBlock::of_class(blob.bytes()).map_err(|err| crate::FormatError::Invalid(format!("moby class {} shadow: {err}", e.o_class)))?;
        if let Some(s) = s { out.push((e.o_class, blob.u8(8)?, s)); }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(out: &mut Vec<u8>, ty: u8, last: u8, words: &[u32]) {
        out.push(ty);
        out.push(last);
        out.extend_from_slice(&(if ty == 0 { 0x20u16 } else { 0x30 }).to_le_bytes());
        for w in words { out.extend_from_slice(&w.to_le_bytes()); }
    }

    #[test]
    fn parses_spheres_and_capsules_until_last() {
        let mut b = Vec::new();
        let f = |x: f32| x.to_bits();
        rec(&mut b, 1, 0, &[0x0007_0003, 6, (-10i32) as u32, f(1.0), f(2.0), f(3.0), f(0.5), f(4.0), f(5.0), f(6.0), f(0.25)]);
        rec(&mut b, 0, 1, &[9, 5, 0, f(7.0), f(8.0), f(9.0), f(1.5)]);
        let s = ShadowBlock::parse(&b).unwrap();
        assert_eq!(s.prims.len(), 2);
        assert_eq!(s.prims[0], ShadowPrim::Capsule { joints: [3, 7], segments: [6, -10], a: [1.0, 2.0, 3.0, 0.5], b: [4.0, 5.0, 6.0, 0.25] });
        assert_eq!(s.prims[1], ShadowPrim::Sphere { joint: 9, segments: 5, centre: [7.0, 8.0, 9.0, 1.5] });
        assert_eq!(s.prims[0].joints(), [3, 7]);
        // A block with bytes past its last record, a bad type and a missing last record are rejected.
        let mut long = b.clone();
        long.extend_from_slice(&[0; 16]);
        assert!(ShadowBlock::parse(&long).is_err(), "trailing bytes");
        let mut bad = b.clone();
        bad[0] = 2;
        assert!(ShadowBlock::parse(&bad).is_err(), "type 2");
        let mut open = b.clone();
        open[0x31] = 0;
        assert!(ShadowBlock::parse(&open).is_err(), "no last record");
        let mut size = b.clone();
        size[2] = 0x20;
        assert!(ShadowBlock::parse(&size).is_err(), "capsule with size 0x20");
    }

    #[test]
    fn block_sits_before_the_skeleton() {
        // Header: 0x0f = 2 quadwords, skeleton at 0x60: the block is 0x40..0x60 (one sphere).
        let mut blob = vec![0u8; 0x60];
        blob[0xf] = 2;
        blob[0x14..0x18].copy_from_slice(&0x60i32.to_le_bytes());
        let mut s = Vec::new();
        rec(&mut s, 0, 1, &[1, 4, 0, 0, 0, 0, 2.0f32.to_bits()]);
        blob[0x40..0x60].copy_from_slice(&s);
        let blk = ShadowBlock::of_class(&blob).unwrap().unwrap();
        assert_eq!(blk.prims, [ShadowPrim::Sphere { joint: 1, segments: 4, centre: [0.0, 0.0, 0.0, 2.0] }]);
        blob[0xf] = 0;
        assert_eq!(ShadowBlock::of_class(&blob).unwrap(), None);
    }
}
