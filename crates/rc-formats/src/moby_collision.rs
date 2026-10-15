//! Moby class collision blobs (class header +0x10, copied to `moby+0x94` by `InitMobyInstance`). Spec and
//! kernel semantics: `docs/plan/collision_queries.md` ("Moby collision in the port"); the layout below is
//! read from the level01 query kernels (`CollLine_Fix` 0x211870, sphere 0x212960, capsule 0x2135a0,
//! `coll_sphere_mobys` 0x214468), which are the only readers.
//!
//! | Off | Type | Meaning |
//! |---|---|---|
//! | 0x00 | u16 | joints to pose for queries **with** flag 0x4 (0 = none; primitives of mask bit 0 read them) |
//! | 0x02 | u16 | joints to pose for queries **without** flag 0x4 (primitives of mask bit 1) |
//! | 0x04 | s32 | primitive section bytes (0x20 per primitive) |
//! | 0x08 | s32 | face section bytes (4 per triangle) |
//! | 0x0c | s32 | vertex section bytes (8 per vertex) |
//! | 0x10 | | primitives, then vertices, then faces |
//!
//! **Primitive** (0x20 bytes): `s8 kind` at +0, a byte at +1 (4 on every retail primitive; not read),
//! `s16 mask` at +2 (bit 0: tested by flag-0x4 queries, bit 1: by the others; **bit 15 ends the list** —
//! the kernels walk primitives until one with a negative mask, not by the section size), then per kind:
//!
//! | kind | shape | fields |
//! |---|---|---|
//! | 1 | sphere | centre xyz / radius w at +0x10 (model units) |
//! | 2 | sphere on a joint | joint `s32` at +4 (`lw`), radius f32 at +0xc, offset xyz at +0x10 added to the joint |
//! | 3 | vertical cylinder | base centre xyz / radius w at +0x10, height f32 at +4 (along world z, not rotated) |
//! | 4 | capsule between two joints | joints `s16` at +4 / +6, radius f32 at +0xc |
//!
//! Anything else is treated by the kernels as kind 4 (none on the disc). Joint positions are the
//! translation rows of the posed joint matrices (`0x267fc0`, the count from +0/+2).
//!
//! **Vertex**: 4 × s16 (x, y, z, pad), model units (the class scale applies at run time through `moby+0x2c`).
//! **Face**: the world mesh's 4-byte record `v0, v1, v2, type` (no quads); normal `(v2−v0)×(v1−v0)`.
//!
//! Disc facts (all 19 levels, `tests/formats/moby_collision_disc.rs`): 1064 blobs, 464 / 31 / 350 / 302 primitives of
//! kinds 1 / 2 / 3 / 4, 23369 vertices, 38359 faces; every joint primitive's selected joint count is non-zero
//! and above its joint indices; one class has 257 vertices (level 3 class 825, just past the 256-slot
//! scratch area the kernels transform into).

use crate::buf::{invalid, Buf, Result};
use crate::level::LevelCore;

/// One 0x20-byte primitive record, kept as raw words so floats keep their bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MobyCollPrim {
    pub raw: [u32; 8],
}

impl MobyCollPrim {
    /// +0 `s8` kind (1 sphere, 2 joint sphere, 3 vertical cylinder, 4 joint capsule).
    pub fn kind(&self) -> i8 { self.raw[0] as u8 as i8 }
    /// +1 byte (4 on the disc; unused).
    pub fn byte1(&self) -> u8 { (self.raw[0] >> 8) as u8 }
    /// +2 `s16` mask: bit 0 / bit 1 select the query class (flag 0x4 set / clear), bit 15 ends the list.
    pub fn mask(&self) -> i16 { (self.raw[0] >> 16) as u16 as i16 }
    /// +4 as `s32` (kind 2's joint, read with `lw`).
    pub fn word4(&self) -> i32 { self.raw[1] as i32 }
    /// +4 / +6 as `s16` (kind 4's joints).
    pub fn joints(&self) -> [i16; 2] { [self.raw[1] as u16 as i16, (self.raw[1] >> 16) as u16 as i16] }
    /// The quadword at +0 (lanes as raw bits): kind 3 reads lane y (+4, height), kinds 2 / 4 lane w (+0xc, radius).
    pub fn q0(&self) -> [u32; 4] { [self.raw[0], self.raw[1], self.raw[2], self.raw[3]] }
    /// The quadword at +0x10: kinds 1 / 3 centre and radius, kind 2 joint offset.
    pub fn q1(&self) -> [u32; 4] { [self.raw[4], self.raw[5], self.raw[6], self.raw[7]] }
}

/// A class's collision blob.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MobyCollision {
    /// +0 / +2: joints posed for flag-0x4 queries / for the others (0 = no pose).
    pub joint_counts: [u16; 2],
    pub prims: Vec<MobyCollPrim>,
    /// s16 x, y, z, pad.
    pub vertices: Vec<[i16; 4]>,
    /// v0, v1, v2, type.
    pub faces: Vec<[u8; 4]>,
}

impl MobyCollision {
    /// Parses the blob at `blob` (the bytes from class + class[0x10] on). Rejects sizes that are not a
    /// multiple of their record, a primitive list without its bit-15 terminator (or with an early one), and
    /// face indices past the vertex count.
    pub fn parse(blob: &[u8]) -> Result<MobyCollision> {
        let b = Buf(blob);
        let joint_counts = [b.u16(0)?, b.u16(2)?];
        let (pb, fb, vb) = (b.i32(4)?, b.i32(8)?, b.i32(0xc)?);
        if pb < 0 || fb < 0 || vb < 0 || pb % 0x20 != 0 || fb % 4 != 0 || vb % 8 != 0 {
            return invalid(format!("moby collision: section sizes {pb:#x} / {fb:#x} / {vb:#x}"));
        }
        let (pb, fb, vb) = (pb as usize, fb as usize, vb as usize);
        let raw: Vec<[u32; 8]> = b.pod_slice(0x10, pb / 0x20, "moby collision primitives")?;
        let prims: Vec<MobyCollPrim> = raw.into_iter().map(|raw| MobyCollPrim { raw }).collect();
        if let Some(i) = prims.iter().position(|p| p.mask() < 0) {
            if i + 1 != prims.len() { return invalid(format!("moby collision: primitive {i} ends the list early")); }
        } else if !prims.is_empty() {
            return invalid("moby collision: primitive list without its end bit");
        }
        let vertices: Vec<[i16; 4]> = b.pod_slice(0x10 + pb, vb / 8, "moby collision vertices")?;
        let faces: Vec<[u8; 4]> = b.pod_slice(0x10 + pb + vb, fb / 4, "moby collision faces")?;
        if let Some(f) = faces.iter().find(|f| f[..3].iter().any(|&v| v as usize >= vertices.len())) {
            return invalid(format!("moby collision: face {f:?} past {} vertices", vertices.len()));
        }
        Ok(MobyCollision { joint_counts, prims, vertices, faces })
    }

    /// Byte size of the blob (header + sections).
    pub fn size(&self) -> usize { 0x10 + 0x20 * self.prims.len() + 8 * self.vertices.len() + 4 * self.faces.len() }

    /// The blob of a class (`class_blob` = the class's core block), `None` when class +0x10 is 0.
    pub fn of_class(class_blob: &[u8]) -> Result<Option<MobyCollision>> {
        let off = Buf(class_blob).i32(0x10)?;
        if off == 0 { return Ok(None); }
        if off < 0 { return invalid(format!("moby collision offset {off:#x}")); }
        Ok(Some(MobyCollision::parse(Buf(class_blob).tail(off as usize, "moby collision blob")?.bytes())?))
    }
}

/// Every class of a level with a collision blob, `(o_class, blob)` in core-index order.
pub fn parse_level(core: &LevelCore, core_data: &[u8]) -> Result<Vec<(i32, MobyCollision)>> {
    let mut out = Vec::new();
    for e in core.moby_classes.iter().filter(|e| e.offset_in_asset_wad > 0) {
        let Some(blk) = core.blocks.iter().find(|b| b.offset == e.offset_in_asset_wad as usize && b.name.starts_with("moby_class/")) else {
            return invalid(format!("moby class {}: no core block", e.o_class));
        };
        let blob = Buf(core_data).sub(blk.offset, blk.size, "moby class blob")?;
        let c = MobyCollision::of_class(blob.bytes()).map_err(|err| crate::FormatError::Invalid(format!("moby class {} collision: {err}", e.o_class)))?;
        if let Some(c) = c { out.push((e.o_class, c)); }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(prims: &[[u32; 8]], verts: &[[i16; 4]], faces: &[[u8; 4]], counts: [u16; 2]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&counts[0].to_le_bytes());
        b.extend_from_slice(&counts[1].to_le_bytes());
        for n in [prims.len() * 0x20, faces.len() * 4, verts.len() * 8] { b.extend_from_slice(&(n as i32).to_le_bytes()); }
        for p in prims { for w in p { b.extend_from_slice(&w.to_le_bytes()); } }
        for v in verts { for c in v { b.extend_from_slice(&c.to_le_bytes()); } }
        for f in faces { b.extend_from_slice(f); }
        b
    }

    #[test]
    fn parses_sections_and_primitive_fields() {
        let sphere = [0x8003_0401, 0, 0, 0, 0, 0, 1.5f32.to_bits(), 2.0f32.to_bits()];
        let cyl = [0x0002_0403, 3.0f32.to_bits(), 0, 0, 0, 0, 0, 0.5f32.to_bits()];
        let b = blob(&[cyl, sphere], &[[0, 0, 0, 0], [1024, 0, 0, 0], [0, 1024, 0, 0]], &[[0, 1, 2, 0x1f]], [0, 6]);
        let c = MobyCollision::parse(&b).unwrap();
        assert_eq!(c.size(), b.len());
        assert_eq!(c.joint_counts, [0, 6]);
        assert_eq!((c.prims[0].kind(), c.prims[0].mask(), c.prims[0].byte1()), (3, 2, 4));
        assert_eq!(f32::from_bits(c.prims[0].q0()[1]), 3.0);
        assert_eq!((c.prims[1].kind(), c.prims[1].mask()), (1, -0x7ffd));
        assert_eq!(f32::from_bits(c.prims[1].q1()[3]), 2.0);
        assert_eq!(c.vertices[1], [1024, 0, 0, 0]);
        assert_eq!(c.faces, [[0, 1, 2, 0x1f]]);
        let joint = MobyCollPrim { raw: [0x8002_0404, 0x0007_0005, 0, 0, 0, 0, 0, 0] };
        assert_eq!((joint.joints(), joint.word4()), ([5, 7], 0x0007_0005));
    }

    #[test]
    fn rejects_bad_lists_and_indices() {
        let open = [0x0002_0401, 0, 0, 0, 0, 0, 0, 0];
        let end = [0x8002_0401, 0, 0, 0, 0, 0, 0, 0];
        assert!(MobyCollision::parse(&blob(&[open], &[], &[], [0, 0])).is_err(), "no end bit");
        assert!(MobyCollision::parse(&blob(&[end, open], &[], &[], [0, 0])).is_err(), "early end");
        assert!(MobyCollision::parse(&blob(&[], &[[0; 4]], &[[0, 0, 1, 0]], [0, 0])).is_err(), "index past the vertices");
        assert!(MobyCollision::parse(&blob(&[open, end], &[], &[], [0, 0])).is_ok());
    }
}
