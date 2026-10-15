//! Moby (animated character / dynamic object) classes. Spec: docs/formats/moby_rac1.md, with the
//! skinning/lighting corrections from docs/plan/moby_skinning_lighting.md. Ported
//! from the retired C++ reference extractor (git 2230812; `parse_moby_class`, `moby_normal`, the index walk);
//! `tests/formats/golden.rs` checks every section against the committed snapshot table (`data/loader_snapshots.tsv`).
//!
//! What the loader resolves at load time, so a renderer never has to replay the PS2 machinery:
//! * the 9-bit vertex-cache ids (stored 7 vertices late, spec 2.9) and duplicate vertices (2.8);
//! * the VU0 matrix-slot machine (skinning doc 4-5) into per-vertex [`MobySkin`]
//!   (up to 3 joint-palette indices, integer weights summing to 256);
//! * the index stream (secret indices, texture switches, flush trailer) into [`MobyTriangle`]s.
//!
//! Not decoded here: animation sequences/keyframes (spec 4; the keyframe decode in the game's
//! `fun_0020e0e0` is not reversed yet), collision (5), sound defs, shadow, joints lists, bangles
//! (unused on the RAC1 disc) and the separately compressed RAC1 gadget classes.

use crate::buf::{invalid, Buf, Result};
use crate::level::{ClassEntry, LevelCore};
use crate::vif::parse_vif;
use bytemuck::{Pod, Zeroable};

/// Moby class header at byte 0 of a class blob (0x48 bytes). Spec 1.2. Pointers are byte offsets
/// from the start of the class blob; 0 means absent.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MobyClassHeader {
    /// 0x00: packet (submesh) table; 0 = no mesh.
    pub packet_table_offset: i32,
    /// 0x04: high-LOD packet count (first in the table).
    pub high_lod_count: u8,
    /// 0x05: low-LOD packet count (after the high-LOD entries).
    pub low_lod_count: u8,
    /// 0x06: metal ("shine") packet count.
    pub metal_count: u8,
    /// 0x07: table index of the first metal packet (= high + low on the disc).
    pub metal_begin: u8,
    /// 0x08: joint count used with the high LOD (size of `skeleton` / `common_trans`).
    pub joint_count: u8,
    /// 0x09: joint count used with the low LOD (skinning doc 2).
    pub low_lod_joint_count: u8,
    /// 0x0a: first glow packet of the high-LOD list (0xff / past the end: none); docs/plan/moby_skinning_lighting.md §10.
    pub rac1_byte_a: u8,
    /// 0x0b: first glow packet of the low-LOD list (0xff / past the end: none).
    pub rac1_byte_b: u8,
    /// 0x0c: animation sequence count; the pointer list is at 0x48.
    pub sequence_count: u8,
    /// 0x0d: sound definition count.
    pub sound_count: u8,
    /// 0x0e: LOD switch distance; the game uses the low LOD beyond `lod_trans * 1024` depth.
    pub lod_trans: u8,
    /// 0x0f: number of 16-byte shadow entries just before `skeleton`.
    pub shadow: u8,
    /// 0x10: collision data: class-relative offset of the collision blob (0 = none), copied to `moby+0x94` by
    /// `InitMobyInstance`; parsed by [`crate::moby_collision::MobyCollision::of_class`].
    pub collision: i32,
    /// 0x14: `joint_count` Mat4 skeleton matrices (the game uses them as inverse bind matrices).
    pub skeleton: i32,
    /// 0x18: `joint_count` x [`MobyTrans`].
    pub common_trans: i32,
    /// 0x1c: joints lists (spec 3.3).
    pub joints: i32,
    /// 0x20: GIF usage table (spec 6.2).
    pub gif_usage: i32,
    /// 0x24: world units = packed position * scale / 1024.
    pub scale: f32,
    /// 0x28: sound definitions.
    pub sound_defs: i32,
    /// 0x2c: bangles block, in quadwords (0 on every RAC1 class).
    pub bangles: u8,
    /// 0x2d: mipmap distance.
    pub mip_dist: u8,
    /// 0x2e: unknown in RAC1.
    pub rac1_short_2e: i16,
    /// 0x30: bounding sphere centre xyz and radius, world units.
    pub bsphere: [f32; 4],
    /// 0x40: glow colour (RGBA, R low); non-zero → the moby's mode 0x10 and +0x90 (`InitMobyInstance`).
    pub glow_rgba: i32,
    /// 0x44: class mode bits, unknown.
    pub mode_bits: i16,
    /// 0x46: unknown type tag.
    pub ty: u8,
    /// 0x47: more mode bits, unknown.
    pub mode_bits2: u8,
}
const _: () = assert!(std::mem::size_of::<MobyClassHeader>() == 0x48);

/// Packet table entry (0x10 bytes). Spec 2.1.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MobyPacketEntry {
    /// 0x0: class-relative offset of the VIF list.
    pub vif_list_offset: u32,
    /// 0x4: VIF list size in quadwords.
    pub vif_list_size: u16,
    /// 0x6: quadword offset of the ad-gif unpack in the list (0 = none).
    pub vif_list_texture_unpack_offset: u16,
    /// 0x8: class-relative offset of the vertex table header.
    pub vertex_offset: u32,
    /// 0xc: vertex data size in quadwords, header included.
    pub vertex_data_size: u8,
    /// 0xd: `(0xf + 6 * transfer_vertex_count) / 16`, the qwc of the positions DMA.
    pub unknown_d: u8,
    /// 0xe: `(3 + transfer_vertex_count) / 4`, the qwc of the colours DMA.
    pub unknown_e: u8,
    /// 0xf: vertices sent to VU1 (in-file + duplicates).
    pub transfer_vertex_count: u8,
}
const _: () = assert!(std::mem::size_of::<MobyPacketEntry>() == 0x10);

/// RAC1 vertex table header (0x20 bytes) of a regular packet. Spec 2.7.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MobyVertexTableHeader {
    /// 0x00: pre-loop matrix transfers following this header.
    pub matrix_transfer_count: u32,
    /// 0x04: two-way blend vertices (first in the vertex array).
    pub two_way_blend_vertex_count: u32,
    /// 0x08: three-way blend vertices (next).
    pub three_way_blend_vertex_count: u32,
    /// 0x0c: single-matrix vertices (last).
    pub main_vertex_count: u32,
    /// 0x10: duplicate vertex entries.
    pub duplicate_vertex_count: u32,
    /// 0x14: in-file + duplicate vertices.
    pub transfer_vertex_count: u32,
    /// 0x18: byte offset of the vertex array from this header.
    pub vertex_table_offset: u32,
    /// 0x1c: byte offset of the RGBA multiplier blob from this header (also ends the epilogue).
    pub unknown_e: u32,
}
const _: () = assert!(std::mem::size_of::<MobyVertexTableHeader>() == 0x20);

/// Metal packet vertex table header (0x10 bytes). Skinning doc 6.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MobyMetalVertexTableHeader {
    /// 0x0: vertex count; the 16-byte metal vertices follow the header.
    pub vertex_count: i32,
    /// 0x4: output offset of the positions (EE driver output buffer).
    pub unknown_4: i32,
    /// 0x8: output offset of the colours.
    pub unknown_8: i32,
    /// 0xc: total output bytes (ST at 0).
    pub unknown_c: i32,
}
const _: () = assert!(std::mem::size_of::<MobyMetalVertexTableHeader>() == 0x10);

/// Pre-loop matrix transfer: `VU0[vu0_dest_addr] = palette[spr_joint_index]`. Spec 2.10.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct MobyMatrixTransfer {
    pub spr_joint_index: u8,
    /// VU0 quadword address (multiple of 4).
    pub vu0_dest_addr: u8,
}

/// Per-joint `common_trans` record (0x10 bytes). Spec 3.2.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MobyTrans {
    /// 0x0: per-joint vector, role unknown.
    pub vector: [f32; 3],
    /// 0xc: parent joint index * 0x40 (byte offset into the skeleton table).
    pub parent_offset: u16,
    /// 0xe: unknown, usually 0x70.
    pub seventy: u16,
}
const _: () = assert!(std::mem::size_of::<MobyTrans>() == 0x10);

/// Resolved skin attributes of one vertex: position = sum_k weights[k]/256 * (F[joints[k]] * p),
/// with F the per-moby joint palette (F_j = pose_j * skeleton_j; identity F gives the stored pose).
/// Only the first `count` entries are meaningful; unused entries are 0.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MobySkin {
    /// 1..=3.
    pub count: u8,
    /// Joint-palette indices (the game's SPR joint slots; joint 0 is identity for unanimated classes).
    pub joints: [u8; 3],
    /// Integer weights, sum exactly 256.
    pub weights: [u16; 3],
}

impl MobySkin {
    pub fn joint(j: u8) -> Self { MobySkin { count: 1, joints: [j, 0, 0], weights: [256, 0, 0] } }
    /// Weights as fractions (`w / 256`), unused entries 0.
    pub fn weights_f32(&self) -> [f32; 3] { self.weights.map(|w| w as f32 / 256.0) }
}

/// Vertex kind (spec 2.9 types; metal from skinning doc 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MobyVertexKind { TwoWay = 1, ThreeWay = 2, Single = 3, Metal = 4 }

/// One vertex as sent to VU1.
#[derive(Clone, Copy, Debug)]
pub struct MobyVertex {
    /// Regular: bytes 0-7 of the record (skinning bytes, spec 2.9). Metal: bytes 8-15
    /// (joint[3], count, weight[3], pad). Duplicates carry their source vertex's bytes.
    pub raw: [u8; 8],
    pub normal_azimuth: u8,
    pub normal_elevation: u8,
    /// Packed position; world units = value * class scale / 1024.
    pub x: i16,
    pub y: i16,
    pub z: i16,
    /// 9-bit vertex-cache id after undoing the 7-vertex shift; metal: the vertex number.
    pub id: u16,
    pub kind: MobyVertexKind,
    /// Copied from the vertex cache (spec 2.8): position, normal and skin of an earlier vertex.
    pub duplicate: bool,
    pub skin: MobySkin,
    /// Texture coordinate, 4.12 fixed point (`/ 4096.0`); 0 for metal (sphere-mapped at runtime).
    pub st: [i16; 2],
}

impl MobyVertex {
    pub fn packed_position(&self) -> [i16; 3] { [self.x, self.y, self.z] }
}

/// One triangle: indices into the packet's `vertices`, in index-stream order (strip positions
/// n-2, n-1, n; winding is not normalised), and the texture in effect when it was kicked.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct MobyTriangle {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    /// TEX0 `data_lo`: index into the class's 16-entry texture list
    /// (`ClassEntry::textures`), or -1 none, -2 chrome, -3 glass.
    pub texture: i32,
}

/// One packet ("submesh"): up to ~0x60 vertices that VU1 processes in one go.
#[derive(Clone, Debug)]
pub struct MobySubmesh {
    pub entry: MobyPacketEntry,
    /// Zeroed for metal packets.
    pub vertex_table: MobyVertexTableHeader,
    /// Zeroed for regular packets.
    pub metal_header: MobyMetalVertexTableHeader,
    pub transfers: Vec<MobyMatrixTransfer>,
    /// Raw duplicate entries (`cache id << 7`).
    pub duplicates: Vec<u16>,
    /// The vertex records as stored: in-file + epilogue (regular) or the metal records.
    pub raw_vertices: Vec<u8>,
    /// The RAC1 `unknown_e` blob as stored: RGBA lighting multiplier per transfer vertex
    /// (0x80 = 1.0), zero-padded to 16 bytes. Every multiplier on the disc is 0x80. Empty for metal.
    pub rgba_multipliers: Vec<u8>,
    /// In-file vertices, then duplicates.
    pub vertices: Vec<MobyVertex>,
    /// Raw ST unpack (s, t pairs), may be longer than `vertices`; empty for metal.
    pub st: Vec<[i16; 2]>,
    /// Index bytes after the 4-byte index header, including the terminator and padding.
    pub index_bytes: Vec<u8>,
    /// Index header byte 2, then byte 0xc of the first `ad_gif count` quadwords of the ad-gif unpack.
    pub secret_indices: Vec<u8>,
    /// TEX0 `data_lo` of each ad-gif block, in switch order.
    pub texture_indices: Vec<i32>,
    /// Texture in effect at packet start (GS state left by the previous packet of the list; -1 for the first).
    pub initial_texture: i32,
    /// Duplicates whose cache slot was never written (always 0 on the disc).
    pub unresolved_duplicates: u32,
    pub triangles: Vec<MobyTriangle>,
    pub is_metal: bool,
}

impl MobySubmesh {
    /// One RGBA multiplier per vertex of `vertices` (in-file then duplicates; duplicates are not
    /// re-lit by the game, they copy their source's colour). Empty for metal packets.
    pub fn rgba_multiplier_records(&self) -> &[[u8; 4]] {
        let n = if self.is_metal { 0 } else { self.vertex_table.transfer_vertex_count as usize * 4 };
        bytemuck::cast_slice(&self.rgba_multipliers[..n.min(self.rgba_multipliers.len())])
    }
}

/// Skeleton: `joint_count` matrices and hierarchy records.
#[derive(Clone, Debug, Default)]
pub struct MobySkeleton {
    /// Row-major Mat4 as stored (rows 0-2 rotation, row 3 translation in packed units).
    pub matrices: Vec<[[f32; 4]; 4]>,
    pub trans: Vec<MobyTrans>,
}

impl MobySkeleton {
    /// Parent joint (`parent_offset / 0x40`); joint 0 is the root.
    pub fn parent(&self, joint: usize) -> Option<usize> {
        if joint == 0 { return None; }
        self.trans.get(joint).map(|t| t.parent_offset as usize / 0x40)
    }
}

#[derive(Clone, Debug)]
pub struct MobyClass {
    pub header: MobyClassHeader,
    pub sequence_pointers: Vec<i32>,
    pub high_lod: Vec<MobySubmesh>,
    pub low_lod: Vec<MobySubmesh>,
    pub metal: Vec<MobySubmesh>,
    pub skeleton: MobySkeleton,
}

impl MobyClass {
    /// Unit normal of a vertex, game convention (see [`moby_normal`]).
    pub fn normal(&self, v: &MobyVertex) -> [f32; 3] { moby_normal(v.normal_azimuth, v.normal_elevation) }
    /// Bind-pose position in world units (no skinning, no instance transform).
    pub fn position(&self, v: &MobyVertex) -> [f32; 3] {
        let k = self.header.scale / 1024.0;
        [v.x as f32 * k, v.y as f32 * k, v.z as f32 * k]
    }
}

/// Normal from the packed spherical angles as VU0 program 104691 builds it from the (cos, sin)
/// table at 0x165500: `x = cos a cos e, y = sin a cos e, z = sin e`, a/e in 2pi/256 steps.
/// Wrench (and moby_rac1.md before 2026-09-26) swap x and y. Computed in f64 then rounded, like
/// the C++ `moby_normal`; the game's f32 table differs from this by at most 3.5e-7 per entry.
pub fn moby_normal(azimuth: u8, elevation: u8) -> [f32; 3] {
    let k = std::f64::consts::PI / 128.0;
    let (a, e) = (azimuth as f64 * k, elevation as f64 * k);
    [(a.cos() * e.cos()) as f32, (a.sin() * e.cos()) as f32, e.sin() as f32]
}

/// 9-bit vertex-cache id of a duplicate entry (`index << 7`). Spec 2.8.
pub fn duplicate_cache_id(entry: u16) -> u16 { (entry >> 7) & 0x1ff }

/// Undoes the 7-vertex id shift (spec 2.9): record i carries the id of vertex i-7; the ids of the
/// last vertices continue in the epilogue records and then in the six u16 at bytes 4..16 of the
/// last epilogue record. `records` are the in-file + epilogue 16-byte records.
pub fn vertex_cache_ids(records: &[[u8; 16]], in_file: usize) -> Result<Vec<u16>> {
    let mut ids: Vec<u16> = records.iter().skip(7).map(|r| u16::from_le_bytes([r[0], r[1]]) & 0x1ff).collect();
    if let Some(last) = records.last() {
        for k in 0..6 {
            if ids.len() >= in_file { break; }
            ids.push(u16::from_le_bytes([last[4 + k * 2], last[5 + k * 2]]) & 0x1ff);
        }
    }
    if ids.len() < in_file { return invalid("moby packet: not enough vertex ids"); }
    ids.truncate(in_file);
    Ok(ids)
}

/// VU0 data memory of program 104691 as matrix slots (skinning doc 4-5): each quadword address a
/// matrix was stored at holds one palette matrix (a transfer) or a 2-/3-way blend of them.
/// Carried across the packets of one LOD list.
pub struct Vu0Slots { slot: [Option<MobySkin>; 256] }

impl Default for Vu0Slots { fn default() -> Self { Vu0Slots { slot: [None; 256] } } }

impl Vu0Slots {
    pub fn store(&mut self, addr: u8, s: MobySkin) -> Result<()> {
        if !addr.is_multiple_of(4) { return invalid("moby skin: unaligned VU0 store address"); }
        self.slot[addr as usize] = Some(s);
        Ok(())
    }
    pub fn load(&self, addr: u8) -> Result<MobySkin> {
        if !addr.is_multiple_of(4) { return invalid("moby skin: unaligned VU0 load address"); }
        if addr >= 0xf4 { return invalid("moby skin: load from the VU0 sink/constant area"); }
        match self.slot[addr as usize] {
            Some(s) => Ok(s),
            None => invalid("moby skin: load from a VU0 slot that was never written"),
        }
    }
    fn load_joint(&self, addr: u8) -> Result<u8> {
        let s = self.load(addr)?;
        if s.count != 1 { return invalid("moby skin: blend input is itself a blend"); }
        Ok(s.joints[0])
    }

    /// Runs one regular vertex record (bytes 0-7) of the given kind through the slot machine and
    /// returns its skin. Two-way: store palette[b1>>1] at b6, blend b2/b3 with weights b4/b5, store
    /// at b7. Three-way: blend b2/b3/(b1 & 0xfe) with b4/b5/b6, store at b7. Single: store
    /// palette[b1>>1] at b3, then load b2. Stores happen before loads.
    pub fn vertex(&mut self, kind: MobyVertexKind, r: &[u8; 8]) -> Result<MobySkin> {
        let joint = r[1] >> 1;
        match kind {
            MobyVertexKind::TwoWay => {
                self.store(r[6], MobySkin::joint(joint))?;
                let (a, b) = (self.load_joint(r[2])?, self.load_joint(r[3])?);
                if r[4] as u16 + r[5] as u16 != 256 { return invalid("moby skin: 2-way weights do not sum to 256"); }
                let m = MobySkin { count: 2, joints: [a, b, 0], weights: [r[4] as u16, r[5] as u16, 0] };
                self.store(r[7], m)?;
                Ok(m)
            }
            MobyVertexKind::ThreeWay => {
                let (a, b, c) = (self.load_joint(r[2])?, self.load_joint(r[3])?, self.load_joint(r[1] & 0xfe)?);
                if r[4] as u16 + r[5] as u16 + r[6] as u16 != 256 { return invalid("moby skin: 3-way weights do not sum to 256"); }
                let m = MobySkin { count: 3, joints: [a, b, c], weights: [r[4] as u16, r[5] as u16, r[6] as u16] };
                self.store(r[7], m)?;
                Ok(m)
            }
            MobyVertexKind::Single => {
                self.store(r[3], MobySkin::joint(joint))?;
                self.load(r[2])
            }
            MobyVertexKind::Metal => invalid("moby skin: metal vertices do not use VU0 slots"),
        }
    }
}

/// Skin of a metal vertex from its bytes 8-15 (joint[3], count, weight[3], pad): count <= 1 is a
/// single joint (byte 8), otherwise `count` joints whose weights sum to 256. Skinning doc 6.
pub fn metal_skin(r: &[u8; 8]) -> Result<MobySkin> {
    let n = r[3];
    if n <= 1 { return Ok(MobySkin::joint(r[0])); }
    if n > 3 { return invalid("moby metal vertex: joint count > 3"); }
    let mut s = MobySkin { count: n, ..Default::default() };
    for k in 0..n as usize { s.joints[k] = r[k]; s.weights[k] = r[4 + k] as u16; }
    if s.weights.iter().sum::<u16>() != 256 { return invalid("moby metal vertex: weights do not sum to 256"); }
    Ok(s)
}

/// Walks the index stream into triangles (spec 2.4, corrected). Indices are 1-based, bit 7 means
/// "no GS kick"; a 0 byte switches to the next ad-gif block's texture and pushes the next secret
/// index (never kicked); a 0 secret index ends the packet, and the last three indices pushed (the
/// 1,1,1 flush trailer) never reach the GS. A kicked push n draws (n-2, n-1, n). Starts with the
/// submesh's `initial_texture`. Mirrors C++ `walk_indices`.
pub fn moby_triangles(sub: &MobySubmesh) -> Result<Vec<MobyTriangle>> { Ok(walk_indices(sub)?.0) }

fn walk_indices(sub: &MobySubmesh) -> Result<(Vec<MobyTriangle>, i32)> {
    struct Push { idx: u32, nokick: bool, texture: i32 }
    let mut texture = sub.initial_texture;
    let mut pushes: Vec<Push> = Vec::new();
    let (mut secret_i, mut adgif_i, mut ended) = (0usize, 0usize, false);
    for &b in &sub.index_bytes {
        let (idx, nokick) = if b == 0 {
            let Some(&s) = sub.secret_indices.get(secret_i) else { return invalid("moby packet: ran out of secret indices") };
            secret_i += 1;
            if s == 0 { ended = true; break; }
            let Some(&t) = sub.texture_indices.get(adgif_i) else { return invalid("moby packet: texture switch without an ad-gif block") };
            texture = t;
            adgif_i += 1;
            ((s & 0x7f) as u32, true)
        } else {
            ((b & 0x7f) as u32, b & 0x80 != 0)
        };
        if idx == 0 || idx as usize > sub.vertices.len() { return invalid("moby packet: vertex index out of range"); }
        pushes.push(Push { idx: idx - 1, nokick, texture });
    }
    if !ended { return invalid("moby packet: index stream not terminated"); }
    if pushes.len() < 3 { return invalid("moby packet: fewer than 3 indices before the terminator"); }
    pushes.truncate(pushes.len() - 3);
    let tris = (2..pushes.len())
        .filter(|&n| !pushes[n].nokick)
        .map(|n| MobyTriangle { a: pushes[n - 2].idx, b: pushes[n - 1].idx, c: pushes[n].idx, texture: pushes[n].texture })
        .collect();
    Ok((tris, texture))
}

/// State carried across the packets of one LOD list: vertex cache, VU0 slots, GS texture.
struct ListState { cache: Vec<Option<MobyVertex>>, slots: Vu0Slots, texture: i32 }

impl ListState { fn new() -> Self { ListState { cache: vec![None; 512], slots: Vu0Slots::default(), texture: -1 } } }

fn read_packet(blob: Buf, e: &MobyPacketEntry, metal: bool, state: &mut ListState) -> Result<MobySubmesh> {
    // VIF list: ST (regular only), indices, optional ad-gifs.
    let list = blob.sub(e.vif_list_offset as usize, e.vif_list_size as usize * 0x10, "moby VIF list")?;
    let codes = parse_vif(list.bytes())?;
    let mut unpacks = codes.iter().filter(|c| c.is_unpack());
    let mut sub = MobySubmesh {
        entry: *e,
        vertex_table: Default::default(),
        metal_header: Default::default(),
        transfers: Vec::new(),
        duplicates: Vec::new(),
        raw_vertices: Vec::new(),
        rgba_multipliers: Vec::new(),
        vertices: Vec::new(),
        st: Vec::new(),
        index_bytes: Vec::new(),
        secret_indices: Vec::new(),
        texture_indices: Vec::new(),
        initial_texture: -1,
        unresolved_duplicates: 0,
        triangles: Vec::new(),
        is_metal: metal,
    };
    if !metal {
        let Some(st) = unpacks.next() else { return invalid("moby packet: fewer than 2 unpacks") };
        if st.vn() != 1 || st.vl() != 1 { return invalid("moby packet: first unpack is not V2_16"); }
        sub.st = bytemuck::pod_collect_to_vec(&st.data[..st.count() as usize * 4]);
    }
    let Some(ix) = unpacks.next() else { return invalid(if metal { "moby packet: missing index unpack" } else { "moby packet: fewer than 2 unpacks" }) };
    if ix.vn() != 3 || ix.vl() != 2 { return invalid("moby packet: index unpack is not V4_8"); }
    let ix_bytes = ix.count() as usize * 4;
    sub.index_bytes = ix.data[4..ix_bytes].to_vec();
    sub.secret_indices.push(ix.data[2]);
    if let Some(ad) = unpacks.next() {
        if ad.vn() != 3 || ad.vl() != 0 { return invalid("moby packet: texture unpack is not V4_32"); }
        for b in 0..ad.count() as usize / 4 {
            sub.texture_indices.push(i32::from_le_bytes(ad.data[b * 0x40 + 0x20..b * 0x40 + 0x24].try_into().unwrap()));
            // One extra index per ad-gif block, from successive quadwords (block 0 holds the first four).
            sub.secret_indices.push(ad.data[b * 0x10 + 0x0c]);
        }
    }

    let vo = e.vertex_offset as usize;
    if metal {
        sub.metal_header = blob.pod(vo, "moby metal vertex table header")?;
        let count = sub.metal_header.vertex_count;
        if !(0..=4096).contains(&count) { return invalid("moby metal packet: implausible vertex count"); }
        sub.raw_vertices = blob.sub(vo + 0x10, count as usize * 0x10, "moby metal vertices")?.bytes().to_vec();
        for (i, r) in sub.raw_vertices.as_chunks::<16>().0.iter().enumerate() {
            let raw: [u8; 8] = r[8..16].try_into().unwrap();
            sub.vertices.push(MobyVertex {
                raw,
                normal_azimuth: r[6],
                normal_elevation: r[7],
                x: i16::from_le_bytes([r[0], r[1]]),
                y: i16::from_le_bytes([r[2], r[3]]),
                z: i16::from_le_bytes([r[4], r[5]]),
                id: i as u16,
                kind: MobyVertexKind::Metal,
                duplicate: false,
                skin: metal_skin(&raw)?,
                st: [0, 0],
            });
        }
    } else {
        let h: MobyVertexTableHeader = blob.pod(vo, "moby vertex table header")?;
        sub.vertex_table = h;
        let (n2, n3) = (h.two_way_blend_vertex_count as usize, h.three_way_blend_vertex_count as usize);
        let in_file = n2 + n3 + h.main_vertex_count as usize;
        if h.transfer_vertex_count as usize != in_file + h.duplicate_vertex_count as usize { return invalid("moby packet: transfer count mismatch"); }
        if h.transfer_vertex_count != e.transfer_vertex_count as u32 { return invalid("moby packet: entry/table transfer count mismatch"); }
        let tvc = e.transfer_vertex_count as u32;
        if e.unknown_d as u32 != (tvc * 6).div_ceil(0x10) || e.unknown_e as u32 != tvc.div_ceil(4) {
            return invalid("moby packet: redundant entry fields do not match");
        }
        sub.transfers = blob.pod_slice(vo + 0x20, h.matrix_transfer_count as usize, "moby matrix transfers")?;
        let mut ofs = vo + 0x20 + h.matrix_transfer_count as usize * 2;
        if !ofs.is_multiple_of(4) { ofs += 2; }
        if !ofs.is_multiple_of(8) { ofs += 4; }
        sub.duplicates = blob.pod_slice(ofs, h.duplicate_vertex_count as usize, "moby duplicate vertices")?;
        if h.unknown_e < h.vertex_table_offset { return invalid("moby packet: unknown_e before vertex table"); }
        let epilogue = ((h.unknown_e - h.vertex_table_offset) / 0x10) as usize;
        let epilogue = epilogue.wrapping_sub(in_file);
        if !(1..7).contains(&epilogue) { return invalid(format!("moby packet: epilogue vertex count {}", epilogue as isize)); }
        let mult_size = e.vertex_data_size as usize * 0x10;
        if mult_size < h.unknown_e as usize { return invalid("moby packet: multiplier blob starts past the vertex data"); }
        sub.rgba_multipliers = blob.sub(vo + h.unknown_e as usize, mult_size - h.unknown_e as usize, "moby RGBA multipliers")?.bytes().to_vec();
        if sub.rgba_multipliers.len() != (h.transfer_vertex_count as usize * 4).div_ceil(16) * 16 {
            return invalid("moby packet: multiplier blob is not align16(4 * transfer_vertex_count)");
        }
        let vbase = vo + h.vertex_table_offset as usize;
        sub.raw_vertices = blob.sub(vbase, (in_file + epilogue) * 0x10, "moby vertices")?.bytes().to_vec();
        let records: Vec<[u8; 16]> = sub.raw_vertices.as_chunks::<16>().0.to_vec();
        let ids = vertex_cache_ids(&records, in_file)?;

        for t in &sub.transfers { state.slots.store(t.vu0_dest_addr, MobySkin::joint(t.spr_joint_index))?; }
        for (i, r) in records[..in_file].iter().enumerate() {
            let raw: [u8; 8] = r[..8].try_into().unwrap();
            let kind = if i < n2 { MobyVertexKind::TwoWay } else if i < n2 + n3 { MobyVertexKind::ThreeWay } else { MobyVertexKind::Single };
            let skin = state.slots.vertex(kind, &raw)?;
            sub.vertices.push(MobyVertex {
                raw,
                normal_azimuth: r[8],
                normal_elevation: r[9],
                x: i16::from_le_bytes([r[10], r[11]]),
                y: i16::from_le_bytes([r[12], r[13]]),
                z: i16::from_le_bytes([r[14], r[15]]),
                id: ids[i],
                kind,
                duplicate: false,
                skin,
                st: [0, 0],
            });
        }
        // Duplicates copy an earlier vertex (position, normal, skin) from the 512-entry cache.
        for v in &sub.vertices { state.cache[(v.id & 0x1ff) as usize] = Some(*v); }
        for &d in &sub.duplicates {
            let id = duplicate_cache_id(d);
            let v = match state.cache[id as usize] {
                Some(v) => v,
                None => {
                    sub.unresolved_duplicates += 1;
                    MobyVertex { raw: [0; 8], normal_azimuth: 0, normal_elevation: 0, x: 0, y: 0, z: 0, id: 0, kind: MobyVertexKind::Single, duplicate: false, skin: MobySkin::default(), st: [0, 0] }
                }
            };
            sub.vertices.push(MobyVertex { id, duplicate: true, ..v });
        }
        if sub.st.len() < sub.vertices.len() { return invalid("moby packet: ST array shorter than vertex list"); }
        for (v, st) in sub.vertices.iter_mut().zip(&sub.st) { v.st = *st; }
    }

    sub.initial_texture = state.texture;
    let (tris, texture) = walk_indices(&sub)?;
    sub.triangles = tris;
    state.texture = texture;
    Ok(sub)
}

/// Parses one moby class blob (a `moby_class/NNNN` block of the level core data).
pub fn parse_moby_class(blob: &[u8]) -> Result<MobyClass> {
    let b = Buf(blob);
    let h: MobyClassHeader = b.pod(0, "moby class header")?;
    let sequence_pointers = if h.sequence_count > 0 { b.pod_slice(0x48, h.sequence_count as usize, "moby sequence pointers")? } else { Vec::new() };
    let (mut high_lod, mut low_lod, mut metal) = (Vec::new(), Vec::new(), Vec::new());
    if h.packet_table_offset > 0 {
        let total = h.high_lod_count as usize + h.low_lod_count as usize + h.metal_count as usize;
        if h.metal_count > 0 && h.metal_begin as usize + h.metal_count as usize > total { return invalid("moby class: metal packets past the packet table"); }
        let entries: Vec<MobyPacketEntry> = b.pod_slice(h.packet_table_offset as usize, total, "moby packet table")?;
        // Vertex cache, VU0 slots and GS texture state carry across the packets of one list.
        let list = |first: usize, count: usize, is_metal: bool, out: &mut Vec<MobySubmesh>| -> Result<()> {
            let mut st = ListState::new();
            for e in &entries[first..first + count] { out.push(read_packet(b, e, is_metal, &mut st)?); }
            Ok(())
        };
        list(0, h.high_lod_count as usize, false, &mut high_lod)?;
        list(h.high_lod_count as usize, h.low_lod_count as usize, false, &mut low_lod)?;
        list(h.metal_begin as usize, h.metal_count as usize, true, &mut metal)?;
    }
    let jc = h.joint_count as usize;
    let mut skeleton = MobySkeleton::default();
    if h.skeleton > 0 && jc > 0 { skeleton.matrices = b.pod_slice::<[[f32; 4]; 4]>(h.skeleton as usize, jc, "moby skeleton")?; }
    if h.common_trans > 0 && jc > 0 { skeleton.trans = b.pod_slice(h.common_trans as usize, jc, "moby common trans")?; }
    Ok(MobyClass { header: h, sequence_pointers, high_lod, low_lod, metal, skeleton })
}

/// A level's moby class with its core-index entry (whose `textures[16]` maps a triangle's
/// `texture` >= 0 to an index into `LevelCore::moby_textures`).
#[derive(Clone, Debug)]
pub struct LevelMobyClass {
    pub o_class: i32,
    pub entry: ClassEntry,
    pub class: MobyClass,
}

impl LevelMobyClass {
    /// Moby texture table index (`LevelCore::moby_textures`) for a triangle texture, None for
    /// -1/-2/-3 (none/chrome/glass) or an unused slot (0xff).
    pub fn texture_table_index(&self, texture: i32) -> Option<usize> {
        let slot = *self.entry.textures.get(usize::try_from(texture).ok()?)?;
        (slot != 0xff).then_some(slot as usize)
    }
}

/// Parses every moby class of a level that has a geometry blob (entries with offset 0 are
/// skipped), in core-index order. `core_data` is the decompressed core data.
pub fn parse_level_mobys(core: &LevelCore, core_data: &[u8]) -> Result<Vec<LevelMobyClass>> {
    let mut out = Vec::new();
    for e in core.moby_classes.iter().filter(|e| e.offset_in_asset_wad > 0) {
        let Some(blk) = core.blocks.iter().find(|b| b.offset == e.offset_in_asset_wad as usize && b.name.starts_with("moby_class/")) else {
            return invalid(format!("moby class {}: no core block", e.o_class));
        };
        let blob = Buf(core_data).sub(blk.offset, blk.size, "moby class blob")?;
        let class = parse_moby_class(blob.bytes()).map_err(|err| crate::FormatError::Invalid(format!("moby class {}: {err}", e.o_class)))?;
        out.push(LevelMobyClass { o_class: e.o_class, entry: *e, class });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool { a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6) }

    #[test]
    fn normal_axis_convention() {
        // a = 0, e = 0 -> +x; a = 90 deg (64), e = 0 -> +y; e = 90 deg (64) -> +z; a = 180 deg -> -x.
        assert!(close(moby_normal(0, 0), [1.0, 0.0, 0.0]));
        assert!(close(moby_normal(64, 0), [0.0, 1.0, 0.0]));
        assert!(close(moby_normal(0, 64), [0.0, 0.0, 1.0]));
        assert!(close(moby_normal(128, 0), [-1.0, 0.0, 0.0]));
        assert!(close(moby_normal(192, 0), [0.0, -1.0, 0.0]));
        // a = 45 deg, e = 45 deg: (1/2, 1/2, sqrt(2)/2).
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!(close(moby_normal(32, 32), [0.5, 0.5, h]));
        let n = moby_normal(17, 200);
        assert!(((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn vertex_cache_ids_shift_by_seven() {
        // 10 in-file vertices + 1 epilogue: records 7..=10 carry ids of vertices 0..=3, the
        // epilogue's bytes 4.. carry the remaining six (upper bits 9-15 are not part of the id).
        let mut recs = vec![[0u8; 16]; 11];
        for (i, r) in recs.iter_mut().enumerate().skip(7) { r[..2].copy_from_slice(&((0x100 + i as u16) | 0xfe00).to_le_bytes()); }
        for k in 0..6 { recs[10][4 + k * 2..6 + k * 2].copy_from_slice(&((0x1f0u16 + k as u16) | 0x200).to_le_bytes()); }
        let ids = vertex_cache_ids(&recs, 10).unwrap();
        assert_eq!(ids, [0x107, 0x108, 0x109, 0x10a, 0x1f0, 0x1f1, 0x1f2, 0x1f3, 0x1f4, 0x1f5]);
        assert!(vertex_cache_ids(&recs[..8], 8).is_err(), "1 shifted id + 6 epilogue ids < 8");
        // Duplicate entries store the cache id shifted left by 7.
        assert_eq!(duplicate_cache_id(0x1ab << 7), 0x1ab);
        assert_eq!(duplicate_cache_id(0xffff), 0x1ff);
    }

    #[test]
    fn slot_machine_resolves_weights_summing_to_256() {
        let mut s = Vu0Slots::default();
        // Pre-loop transfers: joint 5 -> slot 0, joint 9 -> slot 4.
        s.store(0, MobySkin::joint(5)).unwrap();
        s.store(4, MobySkin::joint(9)).unwrap();
        // Two-way: transfer joint 2 (b1 = 2 << 1) to 8, blend slots 0/4 with 100/156, store at 0x40.
        let two = s.vertex(MobyVertexKind::TwoWay, &[0, 2 << 1, 0, 4, 100, 156, 8, 0x40]).unwrap();
        assert_eq!(two, MobySkin { count: 2, joints: [5, 9, 0], weights: [100, 156, 0] });
        // Three-way: slots 0, 4 and b1 & 0xfe = 8 (joint 2), weights 50/60/146, store at 0x44.
        let three = s.vertex(MobyVertexKind::ThreeWay, &[0x77, 8 | 1, 0, 4, 50, 60, 146, 0x44]).unwrap();
        assert_eq!(three, MobySkin { count: 3, joints: [5, 9, 2], weights: [50, 60, 146] });
        for sk in [two, three] { assert_eq!(sk.weights.iter().sum::<u16>(), 256); }
        // Single: transfer joint 7 to 0xc (store before load), then load it; loading a blend slot
        // gives the blend; 0xf4 is the no-store sink.
        assert_eq!(s.vertex(MobyVertexKind::Single, &[0, 7 << 1, 0xc, 0xc, 0, 0, 0, 0]).unwrap(), MobySkin::joint(7));
        assert_eq!(s.vertex(MobyVertexKind::Single, &[0, 0, 0x40, 0xf4, 0, 0, 0, 0]).unwrap(), two);
        // Invariants the disc satisfies: aligned addresses, no blend of blends, defined slots, sum 256.
        assert!(s.vertex(MobyVertexKind::TwoWay, &[0, 0, 0x40, 0, 128, 128, 0xf4, 0xf4]).is_err(), "blend of a blend");
        assert!(s.vertex(MobyVertexKind::Single, &[0, 0, 0x80, 0xf4, 0, 0, 0, 0]).is_err(), "undefined slot");
        assert!(s.vertex(MobyVertexKind::Single, &[0, 0, 2, 0xf4, 0, 0, 0, 0]).is_err(), "unaligned");
        assert!(s.vertex(MobyVertexKind::TwoWay, &[0, 0, 0, 4, 100, 100, 0xf4, 0xf4]).is_err(), "weights 200");
        assert!(s.vertex(MobyVertexKind::Single, &[0, 0, 0xf4, 0xf4, 0, 0, 0, 0]).is_err(), "sink is write-only");
        // Metal vertices carry their skin inline.
        assert_eq!(metal_skin(&[3, 4, 5, 1, 0, 0, 0, 0]).unwrap(), MobySkin::joint(3));
        assert_eq!(metal_skin(&[3, 4, 5, 3, 16, 32, 208, 0]).unwrap(), MobySkin { count: 3, joints: [3, 4, 5], weights: [16, 32, 208] });
        assert!(metal_skin(&[3, 4, 5, 2, 16, 32, 208, 0]).is_err());
    }

    fn walk_sub(indices: &[u8], secrets: &[u8], textures: &[i32], nverts: usize) -> MobySubmesh {
        let v = MobyVertex { raw: [0; 8], normal_azimuth: 0, normal_elevation: 0, x: 0, y: 0, z: 0, id: 0, kind: MobyVertexKind::Single, duplicate: false, skin: MobySkin::default(), st: [0, 0] };
        MobySubmesh {
            entry: Default::default(), vertex_table: Default::default(), metal_header: Default::default(),
            transfers: vec![], duplicates: vec![], raw_vertices: vec![], rgba_multipliers: vec![],
            vertices: vec![v; nverts], st: vec![], index_bytes: indices.to_vec(), secret_indices: secrets.to_vec(),
            texture_indices: textures.to_vec(), initial_texture: 7, unresolved_duplicates: 0, triangles: vec![], is_metal: false,
        }
    }

    #[test]
    fn index_walk_secret_indices_and_trailer() {
        // 0 -> texture 3 + secret vertex 2 (no kick); 0x83 no kick; 4 kicks (1,2,3); 5 kicks (2,3,4);
        // 0 -> texture 1 + secret 6; 0x81 no kick; 2 kicks (5,0,1) with texture 1; trailer 1,1,1; 0 ends.
        let sub = walk_sub(&[0, 0x83, 4, 5, 0, 0x81, 2, 1, 1, 1, 0, 0], &[0x82, 0x86, 0], &[3, 1], 6);
        let t = moby_triangles(&sub).unwrap();
        let abc: Vec<[u32; 4]> = t.iter().map(|t| [t.a, t.b, t.c, t.texture as u32]).collect();
        assert_eq!(abc, [[1, 2, 3, 3], [2, 3, 4, 3], [5, 0, 1, 1]]);
        // Without a texture switch the packet inherits the previous packet's texture.
        let t = moby_triangles(&walk_sub(&[0x81, 0x82, 3, 1, 1, 1, 0], &[0], &[], 3)).unwrap();
        assert_eq!(t, [MobyTriangle { a: 0, b: 1, c: 2, texture: 7 }]);
        // Unterminated, or out of range.
        assert!(moby_triangles(&walk_sub(&[0x81, 0x82, 3, 1, 1, 1], &[0], &[], 3)).is_err());
        assert!(moby_triangles(&walk_sub(&[0x81, 0x82, 4, 1, 1, 1, 0], &[0], &[], 3)).is_err());
    }
}
