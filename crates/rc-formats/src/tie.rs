//! Tie (instanced static prop) classes and gameplay tie instances. Spec: docs/formats/tie_rac1.md.
//! Ported from the retired C++ reference extractor (git 2230812; `parse_tie_class`, `tie_triangles`,
//! `parse_tie_instances`); `tests/formats/golden.rs` checks every section against the committed snapshot table (`data/loader_snapshots.tsv`).
//!
//! The packet semantics follow the game, not only Wrench: the EE DMA builder (`TieProc`,
//! level01 0x2a9a90) fixes what each packet-header byte uploads, and the VU1 program 13507
//! (EE 0x108e28) fixes how stored vertices become GS-packet writes (spec 3.4):
//! * a packet uploads its ad-gifs, the unpack header + strips, the vertices (VU 0x32), and a
//!   per-vertex colour-index array into the per-instance double buffer;
//! * VU1 writes each vertex's ST / RGBA / XYZ to its `gs_slot` and, in the double-write phases
//!   delimited by GS-slot markers in the unpack header, also to `gs_slot_2`;
//! * the GIF unit then reads ad-gif blocks (placed at 0 and `ad_gif_dest[k-1]`) and strip GIF
//!   tags with `NLOOP = vertex_count` in address order, EOP on the last strip.
//!
//! Lighting inputs are kept, not dropped: every vertex carries its light slot ([`TieVertex::color`],
//! 0..63), which indexes the class's 64 normals ([`TieClass::normals`]) and the instance's 64
//! RGBA5551 ambient colours ([`TieInstance::ambient_rgbas`]); `LightTies` (boot 0x2ab218) lights the
//! 64 slots per instance and VU1 looks each vertex's colour up by slot.

use crate::buf::{invalid, Buf, Result};
use crate::level::{ClassEntry, LevelCore};
use crate::tfrag::AdGif;
use bytemuck::{Pod, Zeroable};

/// Per-LOD totals in the class header (0x10 bytes). All three counts match the packets on every
/// retail class.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TieLodInfo {
    /// 0x0: sum of [`TieStrip::vertex_count`] over the LOD's packets.
    pub strip_vertex_count: u32,
    /// 0x4: sum of `vertex_count - 2` (triangles).
    pub triangle_count: u32,
    /// 0x8: number of strips.
    pub strip_count: u32,
    /// 0xc: zero.
    pub pad: u32,
}

/// RAC1 tie class header at byte 0 of a class blob (0x80 bytes). Spec 2.1. Offsets are relative
/// to the start of the blob.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct TieClassHeader {
    /// 0x00: packet-header table of LOD 0 (highest detail), 1, 2.
    pub packets: [i32; 3],
    /// 0x0c: 64 light-slot normals (`[i16; 4]`, 0x200 bytes), immediately before the ad-gifs.
    pub normals: u32,
    /// 0x10: LOD distance thresholds.
    pub near_dist: f32,
    /// 0x14
    pub mid_dist: f32,
    /// 0x18
    pub far_dist: f32,
    /// 0x1c: equals `unknown_48` on the disc; `TieProc` reuses the word as per-frame LOD counters.
    pub unknown_1c: f32,
    /// 0x20: packets per LOD.
    pub packet_count: [u8; 3],
    /// 0x23: ad-gif records at `ad_gif_ofs`.
    pub texture_count: u8,
    /// 0x24: render mode bits (`TieProc`: `& 9` skips the class, `(& 6) >> 1` selects the path).
    pub flags_24: u16,
    /// 0x26: 0 on the disc (run-time instance count).
    pub instance_count_26: u16,
    /// 0x28: 0 on the disc (run-time instance list).
    pub instance_list_28: u32,
    /// 0x2c: ad-gif table, `texture_count` x 0x50 bytes.
    pub ad_gif_ofs: u32,
    /// 0x30: bounding sphere (xyz, radius).
    pub bsphere: [f32; 4],
    /// 0x40: class-space position = packed s16 * `scale` / 1024.
    pub scale: f32,
    /// 0x44: the class's own number (equals the core-index `o_class` on every retail class).
    pub o_class: i32,
    /// 0x48: unknown distance-like float (5, 10, 20 ...).
    pub unknown_48: f32,
    /// 0x4c: zero.
    pub unknown_4c: u32,
    /// 0x50: per-LOD totals.
    pub lod_info: [TieLodInfo; 3],
}
const _: () = assert!(std::mem::size_of::<TieClassHeader>() == 0x80);

/// Packet header (0x10 bytes). `_ofs`/`_size` fields are quadwords relative to the packet data.
/// Semantics from the DMA chain `TieProc` builds per packet (spec 3.2).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TiePacketHeader {
    /// 0x0: packet data offset, relative to the LOD's packet table.
    pub data: i32,
    /// 0x4: ad-gif blocks uploaded to VU 1, 6, 11, 16 (= 1 + leading positive `ad_gif_dest`).
    pub shader_count: u8,
    /// 0x5: `5 * shader_count`.
    pub ad_gif_qwc: u8,
    /// 0x6: `3 + strip_count`, the V4_8 elements of the control region that matter.
    pub control_count: u8,
    /// 0x7: quadwords of unpack header + strips (uploaded V4_8 USN after the ad-gifs).
    pub control_size: u8,
    /// 0x8: vertex region (uploaded V4_16 to VU 0x32).
    pub vert_ofs: u8,
    /// 0x9
    pub vert_size: u8,
    /// 0xa: colour-index region: two copies (VU buffers A and B) of `color_count` x 4 bytes, each padded to a quadword.
    pub color_ofs: u8,
    /// 0xb
    pub color_count: u8,
    /// 0xc: per-strip-vertex GS slot steps (`strip_vertex_count + 1` bytes; Wrench's "scissor").
    pub slot_table_ofs: u8,
    /// 0xd
    pub slot_table_size: u8,
    /// 0xe: strip count (= [`TieUnpackHeader::strip_count`]).
    pub strip_count: u8,
    /// 0xf: sum of [`TieStrip::vertex_count`].
    pub strip_vertex_count: u8,
}

/// Unpack header at packet data + 0x20 (12 bytes, uploaded V4_8: one VU quadword per 4 bytes).
/// The four `*_end` markers are GS slots at which VU1's vertex loops change phase (spec 3.4).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TieUnpackHeader {
    /// 0: non-zero = no double-write phase for dinky vertices.
    pub dinky_single_only: u8,
    /// 1: non-zero = no fat vertices.
    pub no_fat: u8,
    /// 2: unknown (often the first strip's GIF tag offset).
    pub unknown_2: u8,
    /// 3: strips following at 0x2c.
    pub strip_count: u8,
    /// 4: slot of the dinky vertex that ends the single-write loop (3 more follow in its pipeline).
    pub dinky_single_end: u8,
    /// 5: slot of the dinky vertex that ends the double-write loop (2 more follow).
    pub dinky_double_end: u8,
    /// 6: slot of the last single-write fat vertex.
    pub fat_single_end: u8,
    /// 7: slot of the last fat vertex.
    pub fat_double_end: u8,
    /// 8: `2 * dinky_count + 4` (VU1 conversion-loop bound).
    pub dinky_qwc_plus_four: u8,
    /// 9: `3 * fat_count + 6`.
    pub fat_qwc_plus_six: u8,
    /// 10: dinky vertices.
    pub dinky_count: u8,
    /// 11: fat vertices.
    pub fat_count: u8,
}

/// Strip record (4 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TieStrip {
    /// 0: vertices in the strip; VU1 writes it as the strip GIF tag's NLOOP.
    pub vertex_count: u8,
    /// 1
    pub pad: u8,
    /// 2: GS-packet quadword of the strip's GIF tag.
    pub gif_tag_offset: u8,
    /// 3: non-zero flips the triangle parity (0 on every retail strip).
    pub winding: u8,
}

/// Stored "dinky" vertex (0x10 bytes, V4_16 -> 2 VU quadwords).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TieDinkyVertex {
    /// 0x0: class-space position.
    pub x: i16,
    pub y: i16,
    pub z: i16,
    /// 0x6: GS slot written.
    pub gs_slot: u16,
    /// 0x8: texture coordinates, 1/4096.
    pub s: i16,
    pub t: i16,
    /// 0xc
    pub q: u16,
    /// 0xe: second GS slot (used only in the double-write phase).
    pub gs_slot_2: u16,
}

/// Stored "fat" (LOD-morphing) vertex (0x18 bytes, V4_16 -> 3 VU quadwords). VU1 (L25..L39)
/// draws it at `position + k * delta` with a per-instance morph factor `k`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TieFatVertex {
    /// 0x00: morph delta (same units as the position). Wrench's `unknown_0/2/4`.
    pub dx: i16,
    pub dy: i16,
    pub dz: i16,
    /// 0x06
    pub gs_slot: u16,
    /// 0x08: base position.
    pub x: i16,
    pub y: i16,
    pub z: i16,
    /// 0x0e: zero.
    pub pad: u16,
    /// 0x10
    pub s: i16,
    pub t: i16,
    pub q: u16,
    /// 0x16
    pub gs_slot_2: u16,
}

/// A class's ad-gif block (0x50 bytes): TEX0, TEX1, MIPTBP1, CLAMP, MIPTBP2 A+D quadwords.
/// TEX0's TBP/CBP are 0 on the disc (`PatchTieGifs` fills them in).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TieAdGifs {
    pub tex0: AdGif,
    pub tex1: AdGif,
    pub miptbp1: AdGif,
    pub clamp: AdGif,
    pub miptbp2: AdGif,
}

/// One vertex as VU1 processes it (dinky vertices first, then fat), resolved (26 bytes, the
/// C++ `rc::TieVertex` layout).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TieVertex {
    /// Class-space position (fat: the base position); world units = value * `scale` / 1024.
    pub position: [i16; 3],
    /// Fat: LOD-morph delta, drawn at `position + k * morph_delta`; 0 for dinky.
    pub morph_delta: [i16; 3],
    /// Texture coordinates, 1/4096 fixed point (`s16`).
    pub st: [i16; 2],
    /// Q of the ST/RGBAQ pair as stored (0x1000 = 1.0 on the disc).
    pub q: u16,
    /// GS-packet quadword this vertex is written to.
    pub gs_slot: u16,
    /// Second slot in the double-write phases, 0 otherwise.
    pub gs_slot_2: u16,
    /// Light slot 0..63: the instance ambient colour / class normal this vertex uses.
    pub color: u8,
    /// Fat: the two slots whose average VU1 blends in as the vertex morphs; equal to `color` for dinky.
    pub morph_colors: [u8; 2],
    /// 1 for a fat vertex.
    pub fat: u8,
}
const _: () = assert!(std::mem::size_of::<TieVertex>() == 26);

impl TieVertex {
    pub fn is_fat(&self) -> bool { self.fat != 0 }
    /// Class-space position in world units at morph factor 0 (full detail).
    pub fn class_position(&self, class_scale: f32) -> [f32; 3] {
        let k = class_scale / 1024.0;
        self.position.map(|c| c as f32 * k)
    }
    /// Texture coordinates as floats (s16 / 4096).
    pub fn uv(&self) -> [f32; 2] { self.st.map(|c| c as f32 / 4096.0) }
    /// Class-space morph delta in world units (same scale as [`Self::class_position`]); VU1 draws a fat
    /// vertex at `class_position + k * class_morph_delta` (tie_rac1.md §3.5).
    pub fn class_morph_delta(&self, class_scale: f32) -> [f32; 3] {
        let k = class_scale / 1024.0;
        self.morph_delta.map(|c| c as f32 * k)
    }
}

/// One GS triangle strip in GS-packet order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TieDraw {
    /// Active ad-gif: index into [`TieClass::ad_gifs`] and `ClassEntry::textures`.
    pub ad_gif: u8,
    pub winding: u8,
    /// Indices into [`TiePacket::vertices`].
    pub vertices: Vec<u16>,
}

/// Triangle of [`TiePacket::vertices`] indices plus the ad-gif index.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TieTriangle {
    pub a: u16,
    pub b: u16,
    pub c: u16,
    pub ad_gif: u16,
}

/// One packet (VU1 batch) of a LOD.
#[derive(Clone, Debug, Default)]
pub struct TiePacket {
    pub header: TiePacketHeader,
    /// GS-packet quadword of ad-gif block k+1 (block 0 is at 0); VU1 stops at the first entry <= 0.
    pub ad_gif_dest: [i32; 4],
    /// Byte offset of ad-gif block k in the class ad-gif table (/ 0x50 = ad-gif index).
    pub ad_gif_src: [i32; 4],
    pub unpack: TieUnpackHeader,
    pub strips: Vec<TieStrip>,
    pub dinky: Vec<TieDinkyVertex>,
    pub fat: Vec<TieFatVertex>,
    /// Colour-index bytes for VU buffer A: one per dinky vertex, then (c0, c1, c2, 0xff) per fat vertex from the next 4-byte boundary.
    pub colors: Vec<u8>,
    /// The same for buffer B (every used index + 0x40, the second palette).
    pub colors_b: Vec<u8>,
    /// Raw `slot_table_size` quadwords (per-strip-vertex GS slot steps; not decoded).
    pub slot_table: Vec<u8>,
    /// Resolved vertices in VU1 processing order.
    pub vertices: Vec<TieVertex>,
    /// Strips in GS order.
    pub draws: Vec<TieDraw>,
}

/// A parsed tie class blob.
#[derive(Clone, Debug, Default)]
pub struct TieClass {
    pub header: TieClassHeader,
    /// Bytes 0x80 .. first packet table: bbox min / max (vec4, w = 1) then, when present, 8 corner points.
    pub header_ext: Vec<u8>,
    /// 64 light-slot normals (x, y, z, 0), unit length in 1/32767.
    pub normals: Vec<[i16; 4]>,
    pub lods: [Vec<TiePacket>; 3],
    pub ad_gifs: Vec<TieAdGifs>,
}

impl TieClass {
    /// Unit normal of a light slot.
    pub fn normal(&self, slot: u8) -> Option<[f32; 3]> {
        let n = self.normals.get(slot as usize)?;
        Some([n[0] as f32 / 32767.0, n[1] as f32 / 32767.0, n[2] as f32 / 32767.0])
    }
}

fn fail<T>(msg: impl AsRef<str>) -> Result<T> { invalid(format!("tie packet: {}", msg.as_ref())) }

/// Resolves stored vertices the way VU1 program 13507 writes them (spec 3.4): dinky, then fat;
/// double-write phases delimited by the unpack-header GS-slot markers.
fn resolve_vertices(pk: &mut TiePacket) -> Result<()> {
    let u = pk.unpack;
    let (d, f) = (pk.dinky.len(), pk.fat.len());
    fn find(slots: impl Iterator<Item = u16>, from: usize, slot: u8) -> Option<usize> {
        slots.enumerate().skip(from).find(|&(_, s)| s == slot as u16).map(|(i, _)| i)
    }
    // Dinky: the single-write loop (L10) exits after storing its marker vertex; the three already in
    // its pipeline are stored single-write (L13 / L21). The double-write loop (L14..L17) exits on its
    // marker and flushes two more (L18..L20).
    let mut d_single = 0;
    if d > 0 {
        let Some(i1) = find(pk.dinky.iter().map(|v| v.gs_slot), 0, u.dinky_single_end) else { return fail("dinky single-write marker not found") };
        d_single = i1 + 4;
        let d_end = if u.dinky_single_only != 0 {
            d_single
        } else {
            let Some(i5) = find(pk.dinky.iter().map(|v| v.gs_slot), d_single, u.dinky_double_end) else { return fail("dinky double-write marker not found") };
            i5 + 3
        };
        if d_end != d { return fail("dinky phase markers do not end at the last dinky vertex"); }
    }
    // Fat: single-write loop (L29..L32) up to and including its marker, then the double-write loop
    // (L33..L39) up to and including the `fat_double_end` vertex.
    let mut f_single = 0;
    if u.no_fat != 0 {
        if f > 0 { return fail("fat vertices present with no_fat set"); }
    } else {
        if f == 0 { return fail("no fat vertices but no_fat clear"); }
        let Some(j6) = find(pk.fat.iter().map(|v| v.gs_slot), 0, u.fat_single_end) else { return fail("fat single-write marker not found") };
        f_single = j6 + 1;
        if find(pk.fat.iter().map(|v| v.gs_slot), f_single, u.fat_double_end) != Some(f - 1) { return fail("fat double-write marker is not the last fat vertex"); }
    }
    let fat_base = d.div_ceil(4) * 4;
    if fat_base + 4 * f > pk.colors.len() || d > pk.colors.len() { return fail("colour indices too short"); }
    let mut out = Vec::with_capacity(d + f);
    for (i, v) in pk.dinky.iter().enumerate() {
        let dbl = i >= d_single;
        if dbl && v.gs_slot_2 == 0 { return fail("double-write dinky vertex with gs_slot_2 = 0"); }
        let c = pk.colors[i];
        out.push(TieVertex {
            position: [v.x, v.y, v.z], morph_delta: [0; 3], st: [v.s, v.t], q: v.q,
            gs_slot: v.gs_slot, gs_slot_2: if dbl { v.gs_slot_2 } else { 0 }, color: c, morph_colors: [c, c], fat: 0,
        });
    }
    for (j, v) in pk.fat.iter().enumerate() {
        let dbl = j >= f_single;
        if dbl && v.gs_slot_2 == 0 { return fail("double-write fat vertex with gs_slot_2 = 0"); }
        let c = &pk.colors[fat_base + 4 * j..fat_base + 4 * j + 3];
        out.push(TieVertex {
            position: [v.x, v.y, v.z], morph_delta: [v.dx, v.dy, v.dz], st: [v.s, v.t], q: v.q,
            gs_slot: v.gs_slot, gs_slot_2: if dbl { v.gs_slot_2 } else { 0 }, color: c[0], morph_colors: [c[1], c[2]], fat: 1,
        });
    }
    pk.vertices = out;
    Ok(())
}

/// Walks the GS packet as the GIF unit consumes it (spec 3.4): ad-gif blocks where VU1's L2 loop put
/// them, strip GIF tags with NLOOP = `vertex_count`, EOP on the last strip.
fn walk_gs_packet(pk: &mut TiePacket) -> Result<()> {
    // L1/L2: ad-gif 0 at slot 0, ad-gif k at ad_gif_dest[k-1] while that entry is > 0.
    let mut adgif_pos = vec![0i32];
    while adgif_pos.len() <= 4 && pk.ad_gif_dest[adgif_pos.len() - 1] > 0 { adgif_pos.push(pk.ad_gif_dest[adgif_pos.len() - 1]); }
    if adgif_pos.len() > 4 { return fail("ad_gif_dest[3] > 0 (VU1 L2 would never terminate)"); }
    if adgif_pos.len() != pk.header.shader_count as usize { return fail("placed ad-gif count != shader_count"); }
    if pk.ad_gif_src[..adgif_pos.len()].iter().any(|&s| s < 0 || s % 0x50 != 0) { return fail("ad_gif_src not a multiple of 0x50"); }
    // Slot -> vertex index, last writer in VU processing order wins.
    let mut slot = vec![-1i32; 0x400];
    let mut consumed = vec![false; 0x400];
    for (i, v) in pk.vertices.iter().enumerate() {
        if v.gs_slot as usize >= slot.len() || v.gs_slot_2 as usize >= slot.len() { return fail("GS slot out of range"); }
        slot[v.gs_slot as usize] = i as i32;
        if v.gs_slot_2 != 0 { slot[v.gs_slot_2 as usize] = i as i32; }
    }
    let (mut cursor, mut next_ad, mut si, mut material) = (0usize, 0usize, 0usize, 0u8);
    let mut draws = Vec::with_capacity(pk.strips.len());
    while si < pk.strips.len() {
        let st = pk.strips[si];
        if next_ad < adgif_pos.len() && adgif_pos[next_ad] as usize == cursor {
            material = (pk.ad_gif_src[next_ad] / 0x50) as u8;
            next_ad += 1;
            cursor += 6;
        } else if st.gif_tag_offset as usize == cursor {
            let mut vertices = Vec::with_capacity(st.vertex_count as usize);
            for n in 0..st.vertex_count as usize {
                let s = cursor + 1 + 3 * n;
                if s >= slot.len() || slot[s] < 0 { return fail(format!("strip reads unwritten GS slot {s}")); }
                consumed[s] = true;
                vertices.push(slot[s] as u16);
            }
            cursor += 1 + 3 * st.vertex_count as usize;
            draws.push(TieDraw { ad_gif: material, winding: st.winding, vertices });
            si += 1;
        } else {
            return fail(format!("no GIF tag at GS packet offset {cursor}"));
        }
    }
    if next_ad != adgif_pos.len() { return fail("ad-gif block after the last strip"); }
    if pk.vertices.iter().any(|v| !consumed[v.gs_slot as usize] || (v.gs_slot_2 != 0 && !consumed[v.gs_slot_2 as usize])) {
        return fail("vertex written to a GS slot no strip reads");
    }
    pk.draws = draws;
    Ok(())
}

fn read_packet(blob: Buf, lod_table: usize, ph: TiePacketHeader) -> Result<TiePacket> {
    let base = lod_table + ph.data as usize;
    let mut pk = TiePacket {
        header: ph,
        ad_gif_dest: blob.pod(base, "tie ad_gif_dest")?,
        ad_gif_src: blob.pod(base + 0x10, "tie ad_gif_src")?,
        unpack: blob.pod(base + 0x20, "tie unpack header")?,
        ..Default::default()
    };
    pk.strips = blob.pod_slice(base + 0x2c, pk.unpack.strip_count as usize, "tie strips")?;
    if (ph.control_size as usize) * 16 < 12 + 4 * pk.strips.len() { return fail("control region smaller than unpack header + strips"); }

    let vert_start = base + ph.vert_ofs as usize * 0x10;
    let (d, f) = (pk.unpack.dinky_count as usize, pk.unpack.fat_count as usize);
    if d * 0x10 + f * 0x18 > ph.vert_size as usize * 0x10 { return fail("vertices overrun vert_size"); }
    pk.dinky = blob.pod_slice(vert_start, d, "tie dinky vertices")?;
    pk.fat = blob.pod_slice(vert_start + d * 0x10, f, "tie fat vertices")?;

    let color_start = base + ph.color_ofs as usize * 0x10;
    let color_bytes = ph.color_count as usize * 4;
    let color_qw = (ph.color_count as usize).div_ceil(4);
    pk.colors = blob.sub(color_start, color_bytes, "tie colour indices")?.bytes().to_vec();
    pk.colors_b = blob.sub(color_start + color_qw * 0x10, color_bytes, "tie colour indices B")?.bytes().to_vec();
    pk.slot_table = blob.sub(base + ph.slot_table_ofs as usize * 0x10, ph.slot_table_size as usize * 0x10, "tie slot table")?.bytes().to_vec();

    resolve_vertices(&mut pk)?;
    walk_gs_packet(&mut pk)?;
    Ok(pk)
}

/// Parses one tie class blob (as split out of the core data by the block boundary rule).
pub fn parse_tie_class(blob: &[u8]) -> Result<TieClass> {
    let b = Buf(blob);
    let h: TieClassHeader = b.pod(0, "tie class header")?;
    let first = (0..3).filter(|&l| h.packet_count[l] != 0 && h.packets[l] > 0).map(|l| h.packets[l] as usize).min().unwrap_or(blob.len());
    if first < 0x80 { return invalid("tie packet table overlaps the class header"); }
    let mut tc = TieClass {
        header: h,
        header_ext: b.sub(0x80, first - 0x80, "tie header extension")?.bytes().to_vec(),
        normals: b.pod_slice(h.normals as usize, 64, "tie normals")?,
        ..Default::default()
    };
    for lod in 0..3 {
        if h.packet_count[lod] == 0 { continue; }
        if h.packets[lod] <= 0 { return invalid("tie LOD with packets but no packet table"); }
        let table = h.packets[lod] as usize;
        let phs: Vec<TiePacketHeader> = b.pod_slice(table, h.packet_count[lod] as usize, "tie packet headers")?;
        for ph in phs { tc.lods[lod].push(read_packet(b, table, ph)?); }
    }
    if h.texture_count != 0 { tc.ad_gifs = b.pod_slice(h.ad_gif_ofs as usize, h.texture_count as usize, "tie ad-gifs")?; }
    Ok(tc)
}

/// Triangulates a packet's strips: for strip vertex `i >= 2`, `(i-2, i-1, i)` when `i % 2 == winding`,
/// else `(i, i-1, i-2)` (the GS draws both; the order only gives consistent facing).
pub fn tie_triangles(p: &TiePacket) -> Vec<TieTriangle> {
    let mut out = Vec::new();
    for d in &p.draws {
        let parity = usize::from(d.winding != 0);
        for i in 2..d.vertices.len() {
            let (a, b, c) = (d.vertices[i - 2], d.vertices[i - 1], d.vertices[i]);
            out.push(if i % 2 == parity { TieTriangle { a, b, c, ad_gif: d.ad_gif as u16 } } else { TieTriangle { a: c, b, c: a, ad_gif: d.ad_gif as u16 } });
        }
    }
    out
}

/// A tie class of a level with its core-index entry.
#[derive(Clone, Debug)]
pub struct LevelTieClass {
    pub o_class: i32,
    pub entry: ClassEntry,
    pub class: TieClass,
}

impl LevelTieClass {
    /// Tie texture table index (`LevelCore::tie_textures`) for an ad-gif index, None for an unused slot (0xff).
    pub fn texture_table_index(&self, ad_gif: u16) -> Option<usize> {
        let slot = *self.entry.textures.get(ad_gif as usize)?;
        (slot != 0xff).then_some(slot as usize)
    }
}

/// Parses every tie class of a level (core-index order).
pub fn parse_level_ties(core: &LevelCore, core_data: &[u8]) -> Result<Vec<LevelTieClass>> {
    let mut out = Vec::new();
    for e in core.tie_classes.iter().filter(|e| e.offset_in_asset_wad > 0) {
        let Some(blk) = core.blocks.iter().find(|b| b.offset == e.offset_in_asset_wad as usize && b.name.starts_with("tie_class/")) else {
            return invalid(format!("tie class {}: no core block", e.o_class));
        };
        let blob = Buf(core_data).sub(blk.offset, blk.size, "tie class blob")?;
        let class = parse_tie_class(blob.bytes()).map_err(|err| crate::FormatError::Invalid(format!("tie class {}: {err}", e.o_class)))?;
        out.push(LevelTieClass { o_class: e.o_class, entry: *e, class });
    }
    Ok(out)
}

/// Gameplay-file header offset of the tie instance section pointer.
pub const GAMEPLAY_TIE_INSTANCES: usize = 0x34;

/// Gameplay tie instance (0xe0 bytes on disc). Spec 7.2.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TieInstance {
    /// 0x00: class number, resolved against `ClassEntry::o_class` / [`LevelTieClass::o_class`].
    pub o_class: i32,
    /// 0x04: draw distance, an **s32** in world units (not an f32 as Wrench has it): `TieProc` (boot 0x235be8,
    /// level01 0x2a9a90) converts it with `cvt.s.w` and `pminw`s it with the level loader's 720.0 cap. Retail:
    /// 0 or 84..=720 (720 on every Novalis tie); 0 = never drawn.
    pub draw_distance: i32,
    /// 0x08: zero.
    pub pad_8: i32,
    /// 0x0c: occlusion index.
    pub occlusion_index: i32,
    /// 0x10: class-to-world matrix, column-major (`matrix[col][row]`), translation in column 3, world units.
    /// `[3][3]` is 0.01 (or 0) on the disc and kept raw; use [`TieInstance::world_matrix`].
    pub matrix: [[f32; 4]; 4],
    /// 0x50: RGBA5551 ambient colour per light slot (`LightTies` expands them with PEXT5).
    pub ambient_rgbas: [u16; 64],
    /// 0xd0: directional-light selector (values 0..8, 15, 0xff00 on the disc).
    pub directional_lights: i32,
    /// 0xd4
    pub uid: i32,
    /// 0xd8: zero.
    pub pad_d8: i32,
    /// 0xdc: zero.
    pub pad_dc: i32,
}
const _: () = assert!(std::mem::size_of::<TieInstance>() == 0xe0);

impl TieInstance {
    /// The instance matrix with `[3][3]` = 1.
    pub fn world_matrix(&self) -> [[f32; 4]; 4] {
        let mut m = self.matrix;
        m[3][3] = 1.0;
        m
    }
    /// Transforms a class-space point (world units, i.e. already * scale / 1024) to world space.
    pub fn transform_point(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.matrix;
        std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r])
    }
    /// Ambient colour of a light slot expanded like PEXT5: 5-bit channels to bits 7..3, alpha bit to 0x80.
    pub fn ambient_rgba(&self, slot: u8) -> [u8; 4] { rgba5551(self.ambient_rgbas[slot as usize & 63]) }
}

/// PEXT5 expansion of one RGBA5551 halfword.
pub fn rgba5551(c: u16) -> [u8; 4] {
    [((c & 0x1f) << 3) as u8, (((c >> 5) & 0x1f) << 3) as u8, (((c >> 10) & 0x1f) << 3) as u8, if c & 0x8000 != 0 { 0x80 } else { 0 }]
}

/// Parses a tie instance section (`s32 count`, 12 bytes pad, `count` x 0xe0 records).
pub fn parse_tie_instance_section(section: &[u8]) -> Result<Vec<TieInstance>> {
    let b = Buf(section);
    let count = b.i32(0)?;
    if !(0..=100_000).contains(&count) { return invalid("implausible tie instance count"); }
    b.pod_slice(0x10, count as usize, "tie instances")
}

/// Parses the tie instances of a decompressed gameplay file (section pointer at 0x34).
pub fn parse_tie_instances(gameplay: &[u8]) -> Result<Vec<TieInstance>> {
    let b = Buf(gameplay);
    let ofs = b.u32(GAMEPLAY_TIE_INSTANCES)? as usize;
    if ofs == 0 { return Ok(Vec::new()); }
    parse_tie_instance_section(b.tail(ofs, "tie instance section")?.bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::bytes_of;

    /// Builds a one-LOD class blob around `pk` (packet data, header fields filled in by the caller).
    fn class_blob(ph: TiePacketHeader, data: &[u8], ad_gifs: usize) -> Vec<u8> {
        let table = 0x80usize;
        let pdata = table + 0x10;
        let normals = pdata + data.len();
        let adg = normals + 0x200;
        let mut h = TieClassHeader { scale: 1.0, o_class: 7, ..Default::default() };
        h.packets = [table as i32, 0, 0];
        h.packet_count = [1, 0, 0];
        h.normals = normals as u32;
        h.ad_gif_ofs = adg as u32;
        h.texture_count = ad_gifs as u8;
        let mut b = bytes_of(&h).to_vec();
        b.extend_from_slice(bytes_of(&TiePacketHeader { data: 0x10, ..ph }));
        b.extend_from_slice(data);
        for i in 0..64i16 { b.extend_from_slice(bytes_of(&[i, 0, 0, 0])); }
        b.resize(b.len() + ad_gifs * 0x50, 0);
        b
    }

    fn pad16(v: &mut Vec<u8>) { while !v.len().is_multiple_of(16) { v.push(0); } }

    /// Packet: ad-gif 0 at slot 0, strip A (3 verts) at 6, ad-gif 1 at 16, strip B (4 verts) at 22.
    /// Slots A: 7, 10, 13; B: 23, 26, 29, 32. Four dinky (single-only, marker = index 0) and two fat
    /// vertices (single phase = index 0, double phase = index 1 which also writes slot 26).
    fn sample() -> (TiePacketHeader, Vec<u8>) {
        let mut d = Vec::new();
        for x in [16i32, 0, 0, 0] { d.extend(x.to_le_bytes()); }                 // ad_gif_dest
        for x in [0x50i32, 0, 0, 0] { d.extend(x.to_le_bytes()); }               // ad_gif_src: block 0 -> ad-gif 1, block 1 -> ad-gif 0
        // unpack header: single_only, fat present, ?, 2 strips, markers (dinky 29 is index 0), counts 4 dinky + 2 fat
        d.extend([1, 0, 6, 2, 29, 0, 7, 23, 12, 12, 4, 2]);
        d.extend([3, 0, 6, 0, 4, 0, 22, 0]);                                     // strips
        pad16(&mut d);
        let vert_ofs = d.len() / 16;
        let dinky = [(29u16, 0u16), (13, 0), (32, 0), (10, 0)];
        for (i, (s, s2)) in dinky.iter().enumerate() {
            d.extend(bytes_of(&TieDinkyVertex { x: i as i16, y: 100, z: 200, gs_slot: *s, s: 4096, t: -4096, q: 0x1000, gs_slot_2: *s2 }));
        }
        d.extend(bytes_of(&TieFatVertex { dx: 1, dy: 2, dz: 3, gs_slot: 7, x: 10, y: 11, z: 12, pad: 0, s: 0, t: 0, q: 0x1000, gs_slot_2: 0 }));
        d.extend(bytes_of(&TieFatVertex { dx: -1, dy: -2, dz: -3, gs_slot: 23, x: 20, y: 21, z: 22, pad: 0, s: 0, t: 0, q: 0x1000, gs_slot_2: 26 }));
        pad16(&mut d);
        let vert_size = d.len() / 16 - vert_ofs;
        let color_ofs = d.len() / 16;
        let colors = [5u8, 6, 7, 8, 9, 10, 11, 0xff, 12, 13, 14, 0xff];      // 4 dinky, 2 fat
        d.extend(colors);
        pad16(&mut d);
        d.extend(colors.map(|c| if c == 0xff { c } else { c + 0x40 }));
        pad16(&mut d);
        let slot_ofs = d.len() / 16;
        d.extend([7u8, 3, 3, 0xfc, 3, 3, 3, 0xf6]);
        pad16(&mut d);
        let ph = TiePacketHeader {
            data: 0, shader_count: 2, ad_gif_qwc: 10, control_count: 5, control_size: 2, vert_ofs: vert_ofs as u8, vert_size: vert_size as u8,
            color_ofs: color_ofs as u8, color_count: 3, slot_table_ofs: slot_ofs as u8, slot_table_size: 1, strip_count: 2, strip_vertex_count: 7,
        };
        (ph, d)
    }

    #[test]
    fn walker_follows_vu1_rules() {
        let (ph, d) = sample();
        let tc = parse_tie_class(&class_blob(ph, &d, 2)).unwrap();
        assert_eq!(tc.header_ext.len(), 0);
        assert_eq!(tc.normal(3), Some([3.0 / 32767.0, 0.0, 0.0]));
        let pk = &tc.lods[0][0];
        // Processing order: dinky 0..4 then fat 0..2; only fat 1 is in a double-write phase.
        let slots: Vec<(u16, u16)> = pk.vertices.iter().map(|v| (v.gs_slot, v.gs_slot_2)).collect();
        assert_eq!(slots, [(29, 0), (13, 0), (32, 0), (10, 0), (7, 0), (23, 26)]);
        assert_eq!(pk.vertices.iter().map(|v| v.color).collect::<Vec<_>>(), [5, 6, 7, 8, 9, 12]);
        assert_eq!(pk.vertices[4].morph_colors, [10, 11]);
        assert_eq!(pk.vertices[0].morph_colors, [5, 5]);
        assert_eq!((pk.vertices[5].position, pk.vertices[5].morph_delta, pk.vertices[5].fat), ([20, 21, 22], [-1, -2, -3], 1));
        assert_eq!(pk.vertices[1].uv(), [1.0, -1.0]);
        // Strip A uses ad-gif 1 (block 0's source is 0x50), strip B ad-gif 0; fat 1 fills slots 23 and 26.
        assert_eq!(pk.draws, [TieDraw { ad_gif: 1, winding: 0, vertices: vec![4, 3, 1] }, TieDraw { ad_gif: 0, winding: 0, vertices: vec![5, 5, 0, 2] }]);
        let tris = tie_triangles(pk);
        assert_eq!(tris, [
            TieTriangle { a: 4, b: 3, c: 1, ad_gif: 1 },
            TieTriangle { a: 5, b: 5, c: 0, ad_gif: 0 },
            TieTriangle { a: 2, b: 0, c: 5, ad_gif: 0 },
        ]);
    }

    #[test]
    fn winding_flag_flips_parity() {
        let p = TiePacket { draws: vec![TieDraw { ad_gif: 3, winding: 1, vertices: vec![0, 1, 2, 3] }], ..Default::default() };
        assert_eq!(tie_triangles(&p), [TieTriangle { a: 2, b: 1, c: 0, ad_gif: 3 }, TieTriangle { a: 1, b: 2, c: 3, ad_gif: 3 }]);
    }

    #[test]
    fn dinky_double_phase_uses_marker_plus_four() {
        // 8 dinky, no fat: single marker at index 0 -> indices 0..3 single, 4..7 double; double marker at index 5 (= 8 - 3).
        let mut pk = TiePacket {
            unpack: TieUnpackHeader { dinky_single_end: 40, dinky_double_end: 45, no_fat: 1, ..Default::default() },
            colors: vec![0; 8],
            ..Default::default()
        };
        pk.dinky = (0..8u16).map(|i| TieDinkyVertex { gs_slot: 40 + i, gs_slot_2: 100 + i, ..Default::default() }).collect();
        resolve_vertices(&mut pk).unwrap();
        let s2: Vec<u16> = pk.vertices.iter().map(|v| v.gs_slot_2).collect();
        assert_eq!(s2, [0, 0, 0, 0, 104, 105, 106, 107]);
        // Double marker not at D - 3: the VU would run past the dinky vertices.
        pk.unpack.dinky_double_end = 44;
        assert!(resolve_vertices(&mut pk).is_err());
        // Single-only: the marker must be D - 4.
        pk.unpack.dinky_single_only = 1;
        pk.unpack.dinky_single_end = 44;
        resolve_vertices(&mut pk).unwrap();
        assert!(pk.vertices.iter().all(|v| v.gs_slot_2 == 0));
        pk.unpack.dinky_single_end = 45;
        assert!(resolve_vertices(&mut pk).is_err());
    }

    #[test]
    fn malformed_packets_are_rejected() {
        let (ph, d) = sample();
        // A strip whose slots are not all written.
        let mut bad = d.clone();
        bad[0x2c] = 4;
        assert!(parse_tie_class(&class_blob(ph, &bad, 2)).is_err());
        // ad_gif_dest[3] > 0 never terminates the VU1 placement loop.
        let mut bad = d.clone();
        bad[0..16].copy_from_slice(&[16, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
        assert!(parse_tie_class(&class_blob(ph, &bad, 2)).is_err());
        // shader_count disagreeing with the placed ad-gifs.
        assert!(parse_tie_class(&class_blob(TiePacketHeader { shader_count: 1, ..ph }, &d, 2)).is_err());
        // A missing marker.
        let mut bad = d.clone();
        bad[0x20 + 6] = 99;
        assert!(parse_tie_class(&class_blob(ph, &bad, 2)).is_err());
    }

    #[test]
    fn instance_helpers() {
        let mut inst: TieInstance = Zeroable::zeroed();
        inst.matrix = [[2.0, 0.0, 0.0, 0.0], [0.0, 3.0, 0.0, 0.0], [0.0, 0.0, 4.0, 0.0], [10.0, 20.0, 30.0, 0.01]];
        assert_eq!(inst.world_matrix()[3], [10.0, 20.0, 30.0, 1.0]);
        assert_eq!(inst.transform_point([1.0, 1.0, 1.0]), [12.0, 23.0, 34.0]);
        inst.ambient_rgbas[5] = 0x8000 | (31 << 10) | (1 << 5) | 2;
        assert_eq!(inst.ambient_rgba(5), [16, 8, 248, 0x80]);
        assert_eq!(rgba5551(0x7fff), [248, 248, 248, 0]);
        let mut section = vec![0u8; 0x10];
        section[0] = 1;
        section.extend_from_slice(bytes_of(&inst));
        let mut gp = vec![0u8; 0x40];
        gp[GAMEPLAY_TIE_INSTANCES..GAMEPLAY_TIE_INSTANCES + 4].copy_from_slice(&0x40u32.to_le_bytes());
        gp.extend_from_slice(&section);
        let got = parse_tie_instances(&gp).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(bytes_of(&got[0]), bytes_of(&inst));
        // +0x04 is read as an integer: 720 = 0x2d0, not the f32 bits.
        let mut raw = bytes_of(&inst).to_vec();
        raw[4..8].copy_from_slice(&720i32.to_le_bytes());
        assert_eq!(bytemuck::pod_read_unaligned::<TieInstance>(&raw).draw_distance, 720);
    }

    /// Novalis (skipped without `extracted/`): a fat vertex at morph factor k = 1 (`position + delta`) lies on
    /// the next LOD's surface (within 2.5 s16 units, the position quantisation), while at k = 0 it mostly does
    /// not. So `TieProc`'s k = (depth − near) / (mid − near) turns LOD 0 into the LOD-1 surface exactly where it
    /// switches to LOD 1 (same for LOD 1 → 2 over mid..far): no pop at the switch (tie_rac1.md §3.5).
    #[test]
    fn novalis_fat_vertices_morph_onto_the_next_lod() {
        type V = [f64; 3];
        fn sub(a: V, b: V) -> V { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
        fn dot(a: V, b: V) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
        fn lerp(a: V, d: V, t: f64) -> V { [a[0] + t * d[0], a[1] + t * d[1], a[2] + t * d[2]] }
        // Distance from p to triangle abc (closest-point regions, Ericson 5.1.5).
        fn dist(p: V, a: V, b: V, c: V) -> f64 {
            let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
            let q = {
                let (d1, d2) = (dot(ab, ap), dot(ac, ap));
                let bp = sub(p, b);
                let (d3, d4) = (dot(ab, bp), dot(ac, bp));
                let cp = sub(p, c);
                let (d5, d6) = (dot(ab, cp), dot(ac, cp));
                let (va, vb, vc) = (d3 * d6 - d5 * d4, d5 * d2 - d1 * d6, d1 * d4 - d3 * d2);
                if d1 <= 0.0 && d2 <= 0.0 { a }
                else if d3 >= 0.0 && d4 <= d3 { b }
                else if d6 >= 0.0 && d5 <= d6 { c }
                else if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 { lerp(a, ab, d1 / (d1 - d3)) }
                else if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 { lerp(a, ac, d2 / (d2 - d6)) }
                else if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 { lerp(b, sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))) }
                else { let den = 1.0 / (va + vb + vc); lerp(lerp(a, ab, vb * den), ac, vc * den) }
            };
            dot(sub(p, q), sub(p, q)).sqrt()
        }
        let dir = crate::test_data::root().join("levels/01");
        let (Ok(idx), Ok(comp)) = (std::fs::read(dir.join("core_index.bin")), std::fs::read(dir.join("core_data.bin"))) else {
            eprintln!("skipped: no extracted/");
            return;
        };
        let data = crate::wad::decompress(&comp).unwrap();
        let core = crate::level::parse_level_core(&idx, data.len()).unwrap();
        let classes = parse_level_ties(&core, &data).unwrap();
        for l in 0..2 {
            let (mut n, mut on_target, mut on_base, mut worst) = (0usize, 0usize, 0usize, 0.0f64);
            for c in &classes {
                let tris: Vec<[V; 3]> = c.class.lods[l + 1]
                    .iter()
                    .flat_map(|p| tie_triangles(p).into_iter().map(move |t| [t.a, t.b, t.c].map(|i| p.vertices[i as usize].position.map(f64::from))))
                    .collect();
                if tris.is_empty() { continue; }
                for v in c.class.lods[l].iter().flat_map(|p| p.vertices.iter()).filter(|v| v.is_fat()) {
                    let base = v.position.map(f64::from);
                    let target = lerp(base, v.morph_delta.map(f64::from), 1.0);
                    let near = |p: V| tris.iter().map(|t| dist(p, t[0], t[1], t[2])).fold(f64::MAX, f64::min);
                    let d = near(target);
                    n += 1;
                    worst = worst.max(d);
                    on_target += (d < 2.5) as usize;
                    on_base += (near(base) < 2.5) as usize;
                }
            }
            eprintln!("LOD {l} fat vertices: {n}, at k = 1 on LOD {} surface: {on_target} (worst {worst:.2}), at k = 0: {on_base}", l + 1);
            assert!(n > 0 && on_target == n, "LOD {l}: {on_target}/{n} (worst {worst})");
            assert!(on_base < n / 2);
        }
    }

    /// Novalis (skipped without `extracted/`): every tie draw distance is an integer in 0..=720 (the retail
    /// data has 720 on all of them).
    #[test]
    fn novalis_draw_distances_are_integers_up_to_720() {
        let Some(gameplay) = crate::test_data::gameplay(1) else { eprintln!("skipped: no extracted/"); return; };
        let insts = parse_tie_instances(&gameplay).unwrap();
        assert!(!insts.is_empty());
        for inst in &insts { assert!((0..=720).contains(&inst.draw_distance), "draw distance {}", inst.draw_distance); }
        assert!(insts.iter().all(|i| i.draw_distance == 720));
    }
}
