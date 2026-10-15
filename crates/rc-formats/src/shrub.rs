//! Shrub (small instanced prop) classes and gameplay shrub instances. Spec: docs/formats/shrub_sky_rac1.md
//! part 1. Ported from the retired C++ reference extractor (git 2230812;
//! `parse_shrub_class`, `shrub_triangles`, `parse_shrub_instances`); `tests/formats/golden.rs` checks every section against the committed snapshot table (`data/loader_snapshots.tsv`).
//!
//! The packet semantics follow the VU1 shrub program 56467 (EE 0x101768, uploaded by `ShrubProc`,
//! level01 0x29cdf0; spec 1.3b), not only Wrench:
//! * the packet's three UNPACKs (FLG, TOPS-relative) fill a 0x76-quadword input buffer; VU1 reads the
//!   [`ShrubPacketHeader`] at qw 0, the GIF tags from qw 1, the ad-gif blocks after them, vertex part 1 at
//!   `vertex_offset` and part 2 at `vertex_offset + vertex_count`, by address;
//! * VU1 copies every GIF tag to its GS-packet slot, then every ad-gif block (its own A+D GIF tag + the
//!   4 stored quadwords), then writes ST / RGBAQ / XYZF2 of vertices `0 ..= stop + 3` to slots
//!   `off .. off + 3`, where `stop` is the first vertex from index 2 on with bit 15 of `n_and_stop` set
//!   (the software-pipelined loop never tests vertices 0 and 1 and drains three vertices after the flag);
//!   later writes win (padding vertices rewrite their original's slots);
//! * XGKICK starts the GIF at slot 0 of the 0xa8-qw output buffer; it reads A+D blocks (5 qw) and vertex
//!   tags (1 + 3 * NLOOP qw) until the vertex tag with EOP.
//!
//! Lighting inputs are kept: every vertex carries its normal index ([`ShrubVertex::normal`], 0..23), which
//! selects one of the class's 24 normals ([`ShrubClass::normals`]) and the matching entry of the instance's
//! 24-colour palette that the EE lighting pass (level01 `FUN_0029e7e8`, docs/plan/shrub_lighting.md)
//! computes from [`ShrubInstance::colour`] and [`ShrubInstance::dir_lights`]. VU1 looks each vertex's colour
//! up by that index; there is no per-vertex colour on the disc.

use crate::buf::{invalid, Buf, Result};
use crate::level::{LevelCore, ShrubBillboardInfo, ShrubClassEntry};
use crate::tfrag::AdGif;
use bytemuck::{Pod, Zeroable};

/// VU1 input buffer size (quadwords; double buffered at VU 0x02 / 0x78).
pub const INPUT_QWC: usize = 0x76;
/// One GS-packet output buffer (quadwords; triple buffered at VU 0x208 / 0x2b0 / 0x358).
pub const OUTPUT_QWC: usize = 0xa8;

/// Shrub class header at byte 0 of a class blob (0x40 bytes). Offsets are relative to the blob.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ShrubClassHeader {
    /// 0x00: bounding sphere (xyz, radius).
    pub bsphere: [f32; 4],
    /// 0x10: mip distance (Wrench: source of the ad-gif TEX1 K).
    pub mip_distance: f32,
    /// 0x14: render mode bits (unknown).
    pub mode_bits: u16,
    /// 0x16: 0 on the disc; the level loader counts the class's instances here.
    pub instance_count: i16,
    /// 0x18: 0 on the disc; the loader stores the first runtime instance record here.
    pub instances_pointer: i32,
    /// 0x1c: [`ShrubBillboard`] offset, 0 = none (the loader relocates it to a pointer).
    pub billboard_offset: i32,
    /// 0x20: class-space position = packed s16 * `scale` / 1024 (the loader also copies it into the
    /// runtime matrix `[3][3]`).
    pub scale: f32,
    /// 0x24: the class's own number.
    pub o_class: i16,
    /// 0x26
    pub s_class: i16,
    /// 0x28: entries of the packet table at 0x40.
    pub packet_count: i16,
    /// 0x2a
    pub pad_2a: i16,
    /// 0x2c: 24 x [`ShrubNormal`] (read by the lighting pass through the relocated pointer).
    pub normals_offset: i32,
    /// 0x30
    pub pad_30: i32,
    /// 0x34: runtime counters, 0 on the disc.
    pub drawn_count: i16,
    pub scis_count: i16,
    pub billboard_count: i16,
    pub pad_3a: [i16; 3],
}
const _: () = assert!(std::mem::size_of::<ShrubClassHeader>() == 0x40);

/// Packet table entry (8 bytes) at blob + 0x40.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ShrubPacketEntry {
    /// VIF command list offset (blob-relative); `ShrubProc` sends it with a REF tag of `size / 16` qw.
    pub offset: i32,
    pub size: i32,
}

/// Input-buffer quadword 0.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ShrubPacketHeader {
    /// Ad-gif blocks (>= 1: VU1's copy loop is a do-while).
    pub texture_count: i32,
    /// Vertex GIF tags (>= 1).
    pub gif_tag_count: i32,
    /// Entries in each vertex table.
    pub vertex_count: i32,
    /// Input-buffer qw of vertex part 1; part 2 starts `vertex_count` later.
    pub vertex_offset: i32,
}

/// A vertex GIF tag: the first 12 bytes of the GIFtag VU1 copies, plus its GS slot in the 4th word.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ShrubGifTag {
    /// GIFtag bits 0..63 (NLOOP, EOP, PRE, PRIM, FLG, NREG).
    pub tag: u64,
    /// REGS bits 0..31 (0x412: ST, RGBAQ, XYZF2).
    pub tag_hi: u32,
    /// GS-packet quadword the tag is copied to.
    pub gs_packet_offset: i32,
}

impl ShrubGifTag {
    pub fn nloop(&self) -> u32 { (self.tag & 0x7fff) as u32 }
    pub fn eop(&self) -> bool { (self.tag >> 15) & 1 != 0 }
    pub fn pre(&self) -> bool { (self.tag >> 46) & 1 != 0 }
    /// GS primitive type (PRIM bits 0..2): 3 = triangle list, 4 = triangle strip.
    pub fn prim(&self) -> u8 { ((self.tag >> 47) & 7) as u8 }
    pub fn flg(&self) -> u8 { ((self.tag >> 58) & 3) as u8 }
    pub fn nreg(&self) -> u8 { (self.tag >> 60) as u8 }
}

/// Ad-gif block (4 A+D quadwords). VU1 writes its own A+D GIF tag (`NLOOP 1, NREG 4, REGS 0xeeee`) in
/// front of it, so a block takes 5 GS-packet quadwords.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ShrubAdGifs {
    pub tex1: AdGif,
    pub clamp: AdGif,
    pub miptbp1: AdGif,
    /// `data_lo` = texture slot in [`ShrubClassEntry`]`::base.textures` (patched to a real TEX0 at load).
    pub tex0: AdGif,
}

impl ShrubAdGifs {
    /// GS-packet quadword of the block's A+D tag (the w lane of the TEX1 quadword).
    pub fn gs_packet_offset(&self) -> i32 { i32::from_le_bytes(self.tex1.pad[3..7].try_into().unwrap()) }

    /// TEX1_1 K field as the signed 12-bit raw value, 1/16 units (retail -142..-110).
    pub fn lod_k_raw(&self) -> i16 { (((self.tex1.data_lo as u32 & 0xfff) << 4) as u16 as i16) >> 4 }
    /// TEX1_1 K in mip levels: GS LOD = log2(1/|Q|) + K.
    pub fn lod_k(&self) -> f32 { self.lod_k_raw() as f32 / 16.0 }
    /// CLAMP_1 WMS / WMT (on-disc `clamp.data_lo` / `data_hi`): true = CLAMP, false = REPEAT.
    pub fn clamp_st(&self) -> (bool, bool) { (self.clamp.data_lo & 1 != 0, self.clamp.data_hi & 1 != 0) }

    /// The register values the class init writes over the block (boot `fun_00203b08`, the loop over every
    /// packet's ad-gif blocks, texture-table branch). `texture_index` = the class entry's `textures[tex0.data_lo]`
    /// (a `LevelCore::shrub_textures` index), `texture` = that entry, `gs_base` = `DAT_0015ee8c` (GS byte address).
    /// Bit for bit the tfrag rule (`TfragAdGifs::gs_registers`, boot `fun_002040e0`):
    /// * TEX1 = MXL (ty − 1) << 2 | MMIN hi << 6 | MMAG 1 << 5 | K lo << 32;
    /// * CLAMP = WMS lo | WMT hi << 2 | texture_index << 24 (the byte `DrawShrubs`' patcher `fun_00228a30` reads);
    /// * MIPTBP1 = TBW1 max(1, w/128) << 14 | TBP2 (mipmap + base) << 20 | 1 << 34 | TBP3 (pad + base) << 40 | 1 << 54;
    /// * TEX0 = TBW max(1, w/64) << 14 | PSMT8 << 20 | log2 w << 26 | log2 h << 30 | TCC << 34 | CBP (palette + base) << 37 | CLD 4 << 61.
    ///
    /// TBP0 (TEX0 bits 0..13) and TBP1 (MIPTBP1 bits 0..13) stay 0; `fun_00228a30` ORs them in every frame from the
    /// shrub texture-paging table 0x1d88b0 (`fun_0022a330`: base level square `2^n`, mip 1 at `+ 4·(n/2)²`).
    pub fn gs_registers(&self, texture_index: u8, texture: &TextureEntry, gs_base: u32) -> ShrubGsRegs {
        let base = gs_base as i32 >> 8;
        let w = texture.width as i32;
        let tbw = (w >> 6).max(1) as i64;
        let tbw1 = (w >> 7).max(1) as i64;
        let tex1 = ((texture.ty as i64 - 1) << 2 | (self.tex1.data_hi as i64) << 6 | 0x20) as u64 | (self.tex1.data_lo as u32 as u64) << 32;
        let clamp = (self.clamp.data_lo as i64 | (self.clamp.data_hi as i64) << 2 | (texture_index as i64) << 24) as u64;
        let miptbp1 = tbw1 << 14 | ((texture.mipmap as i32 + base) as i64) << 20 | 1 << 34 | ((texture.pad as i32 + base) as i64) << 40 | 1 << 54;
        let tex0 = tbw << 14 | 0x0130_0000 | ee_log2(w) << 26 | ee_log2(texture.height as i32) << 30 | 1 << 34
            | ((texture.palette as i32 + base) as i64) << 37 | (-1i64) << 63;
        ShrubGsRegs { tex1, clamp, miptbp1: miptbp1 as u64, tex0: tex0 as u64 }
    }
}

/// The four register values of a converted shrub ad-gif block (data words only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShrubGsRegs { pub tex1: u64, pub clamp: u64, pub miptbp1: u64, pub tex0: u64 }

/// `fun_001f97a0`: `30 - PLZCW(x)`, floor(log2 x) for x > 0 (same helper as the tfrag init).
fn ee_log2(x: i32) -> i64 {
    let lz = if x >= 0 { x.leading_zeros() } else { (!x).leading_zeros() };
    30 - (lz as i64 - 1)
}

use crate::level::TextureEntry;

/// Vertex part 1 as VU1 sees it (V4_16 signed).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ShrubVertexPart1 { pub x: i16, pub y: i16, pub z: i16, pub gs_packet_offset: i16 }

/// Vertex part 2 as VU1 sees it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ShrubVertexPart2 { pub s: i16, pub t: i16, pub q: i16, pub n_and_stop: u16 }

/// One vertex as VU1 processes it (index `i` reads part-1 / part-2 entry `i`); 16 bytes, the C++
/// `rc::ShrubVertex` layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ShrubVertex {
    /// Class-space position; world units = value * `scale` / 1024.
    pub position: [i16; 3],
    /// GS-packet quadword of the vertex's ST (RGBAQ +1, XYZF2 +2).
    pub gs_packet_offset: i16,
    /// Texture coordinates, 1/4096.
    pub st: [i16; 2],
    /// Multiplies the perspective Q in the ST write (0x1000 = 1.0 on every retail vertex).
    pub q: i16,
    /// 0..23: class normal and instance palette entry.
    pub normal: u8,
    /// 1 on the vertex whose stop bit ends VU1's loop (three more follow it).
    pub stop: u8,
}
const _: () = assert!(std::mem::size_of::<ShrubVertex>() == 16);

impl ShrubVertex {
    /// Class-space position in world units.
    pub fn class_position(&self, class_scale: f32) -> [f32; 3] {
        let k = class_scale / 1024.0;
        self.position.map(|c| c as f32 * k)
    }
    /// Texture coordinates as floats (s16 / 4096).
    pub fn uv(&self) -> [f32; 2] { self.st.map(|c| c as f32 / 4096.0) }
}

/// One vertex GIF tag in GS order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShrubDraw {
    /// Texture slot in effect (tex0 `data_lo` of the last ad-gif block the GIF read; GS state carries
    /// across packets): index into `ShrubClassEntry::base.textures`.
    pub texture: u8,
    /// GS primitive: 3 = triangle list, 4 = triangle strip (every retail draw).
    pub prim: u8,
    /// Indices into [`ShrubPacket::vertices`].
    pub vertices: Vec<u16>,
}

/// Triangle of [`ShrubPacket::vertices`] indices plus the texture slot.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ShrubTriangle { pub a: u16, pub b: u16, pub c: u16, pub texture: u16 }

/// One packet (VU1 batch).
#[derive(Clone, Debug, Default)]
pub struct ShrubPacket {
    pub entry: ShrubPacketEntry,
    pub header: ShrubPacketHeader,
    pub gif_tags: Vec<ShrubGifTag>,
    pub ad_gifs: Vec<ShrubAdGifs>,
    /// `vertex_count` entries read from the input buffer.
    pub part1: Vec<ShrubVertexPart1>,
    pub part2: Vec<ShrubVertexPart2>,
    /// The `stop + 4` vertices VU1 writes, in processing order.
    pub vertices: Vec<ShrubVertex>,
    /// Vertex GIF tags in GS order.
    pub draws: Vec<ShrubDraw>,
}

/// Class normal (x, y, z, 0), unit length in 1/32767.
pub type ShrubNormal = [i16; 4];

/// Far-LOD billboard record at `billboard_offset` (0x40 bytes). Use (docs/plan/shrub_lighting.md §6):
/// the class init (boot `fun_00203b08`, called by `transition_load_wad`) rewrites the three A+D data words
/// ([`ShrubBillboard::gs_registers`]); `ShrubProc` loads qw 0 into `vf16` and sends qw 1..3 verbatim.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ShrubBillboard {
    /// 0x00: F = `trunc(fade_distance)` as a byte (runtime record +0x17; 256 wraps to 0 = billboard only);
    /// the loader raises the instance draw distance to at least `F + 24`.
    pub fade_distance: f32,
    /// 0x04: quad width in class units (world = `width · |c0,c1| · scale / 1024`, [`billboard_extent`]).
    pub width: f32,
    /// 0x08: quad height, class units, scaled by the instance z-column length.
    pub height: f32,
    /// 0x0c: z offset of the quad's bottom edge from the instance origin, class units (same scale as `height`).
    pub z_ofs: f32,
    /// 0x10: TEX1_1 (address 0x14 on disc); on disc `data_lo` = K (s16, 1/16), `data_hi` = MMIN (4).
    pub tex1: AdGif,
    /// 0x20: TEX0_1 (address 0x06); the on-disc data (1, 2, 8) is overwritten at load.
    pub tex0: AdGif,
    /// 0x30: MIPTBP1_1 (address 0x34); built at load from the class entry's texture descriptor.
    pub miptbp1: AdGif,
}
const _: () = assert!(std::mem::size_of::<ShrubBillboard>() == 0x40);

/// A parsed shrub class blob.
#[derive(Clone, Debug, Default)]
pub struct ShrubClass {
    pub header: ShrubClassHeader,
    pub packets: Vec<ShrubPacket>,
    /// The 24 normals the lighting pass lights per instance.
    pub normals: Vec<ShrubNormal>,
    pub billboard: Option<ShrubBillboard>,
}

impl ShrubClass {
    /// Unit normal of a normal / palette index.
    pub fn normal(&self, index: u8) -> Option<[f32; 3]> {
        let n = self.normals.get(index as usize)?;
        Some([n[0] as f32 / 32767.0, n[1] as f32 / 32767.0, n[2] as f32 / 32767.0])
    }
}

fn fail<T>(msg: impl AsRef<str>) -> Result<T> { invalid(format!("shrub packet: {}", msg.as_ref())) }

/// The VU1 input buffer after the packet's UNPACKs (32-bit lanes, as VU memory holds them).
struct InputBuffer {
    qw: [[u32; 4]; INPUT_QWC],
    written: [bool; INPUT_QWC],
}

impl InputBuffer {
    fn at(&self, a: usize, what: &str) -> Result<[u32; 4]> {
        if a >= INPUT_QWC { return fail(format!("{what} outside the input buffer")); }
        if !self.written[a] { return fail(format!("{what} reads an input quadword no unpack wrote")); }
        Ok(self.qw[a])
    }
    fn record<T: Pod>(&self, a: usize, what: &str) -> Result<T> {
        Ok(bytemuck::pod_read_unaligned(bytemuck::bytes_of(&self.at(a, what)?)))
    }
    /// The four lanes truncated to 16 bits (the V4_16 view of a vertex quadword).
    fn halves(&self, a: usize, what: &str) -> Result<[i16; 4]> { Ok(self.at(a, what)?.map(|w| w as i16)) }
}

fn run_vif(list: &[u8]) -> Result<InputBuffer> {
    use crate::vif::{parse_vif, NOP, STCYCL, STMOD};
    let mut buf = InputBuffer { qw: [[0; 4]; INPUT_QWC], written: [false; INPUT_QWC] };
    let mut unpacks = 0;
    for c in parse_vif(list)? {
        if c.is_unpack() {
            unpacks += 1;
            let (v4_32, v4_16) = (c.vn() == 3 && c.vl() == 0, c.vn() == 3 && c.vl() == 1);
            if !v4_32 && !v4_16 { return fail("unpack is neither V4_32 nor V4_16"); }
            if c.imm & 0x8000 == 0 { return fail("unpack without FLG (not TOPS-relative)"); }
            if c.addr() as usize + c.count() as usize > INPUT_QWC { return fail("unpack runs past the 0x76-qw input buffer"); }
            for i in 0..c.count() as usize {
                let q = &mut buf.qw[c.addr() as usize + i];
                for (k, lane) in q.iter_mut().enumerate() {
                    *lane = if v4_32 {
                        u32::from_le_bytes(c.data[i * 16 + k * 4..i * 16 + k * 4 + 4].try_into().unwrap())
                    } else {
                        let h = u16::from_le_bytes([c.data[i * 8 + k * 2], c.data[i * 8 + k * 2 + 1]]);
                        if c.usn() { h as u32 } else { h as i16 as i32 as u32 }
                    };
                }
                buf.written[c.addr() as usize + i] = true;
            }
        } else if c.cmd == STCYCL {
            if c.imm & 0xff != c.imm >> 8 { return fail("STCYCL with CL != WL"); }
        } else if c.cmd == STMOD {
            if c.imm & 3 != 0 { return fail("STMOD mode != 0"); }
        } else if c.cmd != NOP {
            return fail(format!("unexpected VIF code {}", c.cmd));
        }
    }
    if unpacks != 3 { return fail(format!("expected 3 unpacks, got {unpacks}")); }
    Ok(buf)
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Slot {
    #[default]
    Unwritten,
    Tag(u16),
    AdGif(u16, u8),
    Vertex(u16, u8),
}

/// Parses one packet's VIF list and walks it the way VU1 program 56467 and the GIF consume it.
/// `texture` is the GS texture slot in effect (carried from the previous packet; -1 = none yet).
fn read_packet(list: &[u8], entry: ShrubPacketEntry, texture: &mut i32) -> Result<ShrubPacket> {
    let inp = run_vif(list)?;
    let h: ShrubPacketHeader = inp.record(0, "packet header")?;
    if h.texture_count < 1 || h.gif_tag_count < 1 { return fail("texture_count and gif_tag_count must be >= 1"); }
    if h.vertex_count < 0 || h.vertex_offset < 0 || h.vertex_offset as usize + 2 * h.vertex_count as usize > INPUT_QWC {
        return fail("vertex tables outside the input buffer");
    }
    let mut pk = ShrubPacket { entry, header: h, ..Default::default() };
    for k in 0..h.gif_tag_count as usize { pk.gif_tags.push(inp.record(1 + k, "GIF tag")?); }
    for k in 0..h.texture_count as usize {
        let base = 1 + h.gif_tag_count as usize + 4 * k;
        pk.ad_gifs.push(ShrubAdGifs {
            tex1: inp.record(base, "ad-gif")?, clamp: inp.record(base + 1, "ad-gif")?,
            miptbp1: inp.record(base + 2, "ad-gif")?, tex0: inp.record(base + 3, "ad-gif")?,
        });
    }
    let (p1, p2) = (h.vertex_offset as usize, (h.vertex_offset + h.vertex_count) as usize);
    for i in 0..h.vertex_count as usize {
        let a = inp.halves(p1 + i, "vertex part 1")?;
        let b = inp.halves(p2 + i, "vertex part 2")?;
        pk.part1.push(ShrubVertexPart1 { x: a[0], y: a[1], z: a[2], gs_packet_offset: a[3] });
        pk.part2.push(ShrubVertexPart2 { s: b[0], t: b[1], q: b[2], n_and_stop: b[3] as u16 });
    }
    // The vertex loop tests the stop bit from vertex 2 on and drains three more vertices after it.
    let mut stop = 2;
    while inp.at(p2 + stop, "vertex stop scan")?[3] & 0x8000 == 0 { stop += 1; }
    for i in 0..stop + 4 {
        let a = inp.halves(p1 + i, "vertex part 1")?;
        let b = inp.halves(p2 + i, "vertex part 2")?;
        let n = b[3] as u16;
        if n & 0x7fff > 23 { return fail("normal index > 23"); }
        pk.vertices.push(ShrubVertex {
            position: [a[0], a[1], a[2]], gs_packet_offset: a[3], st: [b[0], b[1]], q: b[2], normal: (n & 0x7fff) as u8, stop: (n >> 15) as u8,
        });
    }

    // GS-packet slots in VU1 write order: tags, then ad-gif blocks, then vertices (later writes win).
    let mut slot = [Slot::Unwritten; OUTPUT_QWC];
    let mut put = |at: i32, s: Slot| -> Result<()> {
        if at < 0 || at as usize >= OUTPUT_QWC { return fail("GS slot outside the 0xa8-qw output buffer"); }
        slot[at as usize] = s;
        Ok(())
    };
    for (k, t) in pk.gif_tags.iter().enumerate() { put(t.gs_packet_offset, Slot::Tag(k as u16))?; }
    for (k, a) in pk.ad_gifs.iter().enumerate() {
        for j in 0..5 { put(a.gs_packet_offset() + j as i32, Slot::AdGif(k as u16, j))?; }
    }
    for (i, v) in pk.vertices.iter().enumerate() {
        for j in 0..3 { put(v.gs_packet_offset as i32 + j as i32, Slot::Vertex(i as u16, j))?; }
    }

    // GIF from slot 0 until the EOP tag.
    let mut cursor = 0usize;
    loop {
        if cursor >= OUTPUT_QWC { return fail("GIF runs past the output buffer"); }
        match slot[cursor] {
            Slot::AdGif(k, 0) => {
                if (1..5u8).any(|j| slot.get(cursor + j as usize) != Some(&Slot::AdGif(k, j))) { return fail("ad-gif block partly overwritten"); }
                let tex = pk.ad_gifs[k as usize].tex0.data_lo;
                if !(0..=15).contains(&tex) { return fail("ad-gif texture slot out of range"); }
                *texture = tex;
                cursor += 5;
            }
            Slot::Tag(k) => {
                let g = pk.gif_tags[k as usize];
                if g.flg() != 0 || g.nreg() != 3 || g.tag_hi & 0xfff != 0x412 || !g.pre() {
                    return fail("vertex GIF tag is not PACKED ST/RGBAQ/XYZF2 with PRIM");
                }
                if g.prim() != 3 && g.prim() != 4 { return fail(format!("unexpected GS primitive type {}", g.prim())); }
                if *texture < 0 { return fail("vertices drawn before any ad-gif"); }
                let mut d = ShrubDraw { texture: *texture as u8, prim: g.prim(), vertices: Vec::with_capacity(g.nloop() as usize) };
                for m in 0..g.nloop() as usize {
                    let q = cursor + 1 + 3 * m;
                    if q + 2 >= OUTPUT_QWC { return fail("GIF runs past the output buffer"); }
                    match (slot[q], slot[q + 1], slot[q + 2]) {
                        (Slot::Vertex(a, 0), Slot::Vertex(b, 1), Slot::Vertex(c, 2)) if a == b && b == c => d.vertices.push(a),
                        _ => return fail(format!("GIF reads a vertex slot not written by one vertex at {q}")),
                    }
                }
                pk.draws.push(d);
                cursor += 1 + 3 * g.nloop() as usize;
                if g.eop() { break; }
            }
            _ => return fail(format!("no GIF tag at GS slot {cursor}")),
        }
    }
    Ok(pk)
}

/// Parses one shrub class blob (as split out of the core data by the block boundary rule).
pub fn parse_shrub_class(blob: &[u8]) -> Result<ShrubClass> {
    let b = Buf(blob);
    let h: ShrubClassHeader = b.pod(0, "shrub class header")?;
    if !(0..=1000).contains(&h.packet_count) { return invalid("implausible shrub packet count"); }
    let entries: Vec<ShrubPacketEntry> = b.pod_slice(0x40, h.packet_count as usize, "shrub packet table")?;
    let mut sc = ShrubClass { header: h, ..Default::default() };
    let mut texture = -1; // GS texture state carries from packet to packet
    for e in entries {
        if e.offset < 0 || e.size < 0 { return invalid("negative shrub packet offset/size"); }
        let list = b.sub(e.offset as usize, e.size as usize, "shrub packet")?;
        sc.packets.push(read_packet(list.bytes(), e, &mut texture)?);
    }
    if h.normals_offset <= 0 { return invalid("shrub class without normals"); }
    sc.normals = b.pod_slice(h.normals_offset as usize, 24, "shrub normals")?;
    if h.billboard_offset > 0 { sc.billboard = Some(b.pod(h.billboard_offset as usize, "shrub billboard")?); }
    Ok(sc)
}

/// Triangulates a packet: strips as `(i-2, i-1, i)` for even `i`, `(i, i-1, i-2)` for odd `i` (the GS
/// draws both; this only gives consistent facing, which the disc does not guarantee), lists three at a time.
pub fn shrub_triangles(p: &ShrubPacket) -> Vec<ShrubTriangle> {
    let mut out = Vec::new();
    for d in &p.draws {
        let (v, texture) = (&d.vertices, d.texture as u16);
        if d.prim == 4 {
            for i in 2..v.len() {
                out.push(if i % 2 == 0 {
                    ShrubTriangle { a: v[i - 2], b: v[i - 1], c: v[i], texture }
                } else {
                    ShrubTriangle { a: v[i], b: v[i - 1], c: v[i - 2], texture }
                });
            }
        } else {
            for &[a, b, c] in v.as_chunks::<3>().0 { out.push(ShrubTriangle { a, b, c, texture }); }
        }
    }
    out
}

/// A shrub class of a level with its core-index entry.
#[derive(Clone, Debug)]
pub struct LevelShrubClass {
    pub o_class: i32,
    pub entry: ShrubClassEntry,
    pub class: ShrubClass,
}

impl LevelShrubClass {
    /// Shrub texture table index (`LevelCore::shrub_textures`) for a draw's texture slot, None for an
    /// unused slot (0xff).
    pub fn texture_table_index(&self, texture: u16) -> Option<usize> {
        let slot = *self.entry.base.textures.get(texture as usize)?;
        (slot != 0xff).then_some(slot as usize)
    }
    /// The billboard texture descriptor (GS RAM addresses; decoded by `texture::parse_textures`), if the
    /// class has one (`width != 0`).
    pub fn billboard_texture(&self) -> Option<&ShrubBillboardInfo> { (self.entry.billboard.width != 0).then_some(&self.entry.billboard) }
}

/// Parses every shrub class of a level (core-index order).
pub fn parse_level_shrubs(core: &LevelCore, core_data: &[u8]) -> Result<Vec<LevelShrubClass>> {
    let mut out = Vec::new();
    for e in core.shrub_classes.iter().filter(|e| e.base.offset_in_asset_wad > 0) {
        let Some(blk) = core.blocks.iter().find(|b| b.offset == e.base.offset_in_asset_wad as usize && b.name.starts_with("shrub_class/")) else {
            return invalid(format!("shrub class {}: no core block", e.base.o_class));
        };
        let blob = Buf(core_data).sub(blk.offset, blk.size, "shrub class blob")?;
        let class = parse_shrub_class(blob.bytes()).map_err(|err| crate::FormatError::Invalid(format!("shrub class {}: {err}", e.base.o_class)))?;
        out.push(LevelShrubClass { o_class: e.base.o_class, entry: *e, class });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------
// Billboards (level01 `ShrubProc` 0x29cdf0 = boot `fun_00228be8`; docs/plan/shrub_lighting.md §6)
// ---------------------------------------------------------------------------------------------------------

/// Billboard A+D data words after the class init (boot `fun_00203b08`, billboard branch with the class
/// entry's [`ShrubBillboardInfo`]; `b` = `(TBW_k = max(1, width >> (6 + k)))`, base = `DAT_0015ee8c >> 8`):
/// * TEX1 = MXL (max_mip − 1) << 2 | MMIN (record tex1 hi) << 6 | MMAG 1 << 5 | K (record tex1 lo) << 32;
/// * TEX0 = TBP0 (texture_offset + base) | TBW0 << 14 | PSMT8 << 20 | log2 w << 26 | log2 h << 30 | TCC << 34
///   | CBP (palette_offset + base) << 37 | CLD 4 << 61 (TBP0 set here: billboard textures are resident);
/// * MIPTBP1 = TBP1 (mip1 + base) | TBW1 << 14 | TBP2 (mip2 + base) << 20 | TBW2 << 34 | TBP3 (mip3 + base) << 40 | TBW3 << 54.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShrubBillboardRegs { pub tex1: u64, pub tex0: u64, pub miptbp1: u64 }

impl ShrubBillboardRegs {
    /// MXL: highest mip level (`max_mip - 1`; `max_mip` is the level count).
    pub fn mxl(&self) -> u32 { (self.tex1 >> 2 & 7) as u32 }
    /// K in mip levels (TEX1 bits 32..43, s12 in 1/16).
    pub fn lod_k(&self) -> f32 { ((((self.tex1 >> 32) as u32 & 0xfff) << 4) as u16 as i16 >> 4) as f32 / 16.0 }
}

impl ShrubBillboard {
    /// The class init's conversion of qw 1..3 (see [`ShrubBillboardRegs`]).
    pub fn gs_registers(&self, info: &ShrubBillboardInfo, gs_base: u32) -> ShrubBillboardRegs {
        let base = gs_base as i32 >> 8;
        let tbw = |k: i32| ((info.width as i32) >> (k + 6)).max(1) as i64;
        let tbp = |v: i16| (v as i32 + base) as i64;
        let tex1 = ((info.max_mip as i64 - 1) << 2 | (self.tex1.data_hi as i64) << 6 | 0x20) as u64 | (self.tex1.data_lo as u32 as u64) << 32;
        let tex0 = tbp(info.texture_offset) | tbw(0) << 14 | ee_log2(info.width as i32) << 26 | 0x0130_0000
            | ee_log2(info.height as i32) << 30 | tbp(info.palette_offset) << 37 | 1 << 34 | (-1i64) << 63;
        let miptbp1 = tbp(info.mip1) | tbw(1) << 14 | tbp(info.mip2) << 20 | tbw(2) << 34 | tbp(info.mip3) << 40 | tbw(3) << 54;
        ShrubBillboardRegs { tex1, tex0: tex0 as u64, miptbp1: miptbp1 as u64 }
    }
}

/// Matrix-block column 2 w (level-load `FUN_00255958`): `lo | hi << 16` with
/// lo = `min(trunc((|c0| + |c1|) · 0.5 · 4096), 0x10000)` and hi = `trunc(|c2| · 4096)`, or 0 when above 0x10000
/// (lo = 0x10000 spills into bit 16, as in the game). `m` = the instance matrix (columns).
pub fn packed_column_lengths(m: &[[f32; 4]; 4]) -> u32 {
    let len = |c: &[f32; 4]| (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    let lo = (((len(&m[0]) + len(&m[1])) * 0.5 * 4096.0) as i32).min(0x10000);
    let hi = (len(&m[2]) * 4096.0) as i32;
    let hi = if hi < 0x10001 { hi } else { 0 };
    (lo | hi << 16) as u32
}

/// The billboard quad of one instance in world units: `(width, height, z_ofs)`. `ShrubProc` (level01
/// 0x29dcd4..0x29dd40, boot 0x229acc..0x229b38): `vf3.xy = itof12(lo, hi)` of [`packed_column_lengths`], `vf15.y = width · lo`,
/// `vf15.zw = (height, z_ofs) · hi`, all `· scale` (col3.w); the view-projection 0x186f80 takes 1024-scaled
/// coordinates, so world = that / 1024.
pub fn billboard_extent(b: &ShrubBillboard, class_scale: f32, packed_lengths: u32) -> [f32; 3] {
    let lo = (packed_lengths & 0xffff) as f32 / 4096.0;
    let hi = (packed_lengths >> 16) as f32 / 4096.0;
    [b.width * lo * class_scale / 1024.0, b.height * hi * class_scale / 1024.0, b.z_ofs * hi * class_scale / 1024.0]
}

/// Quad corners in GS order (a 4-vertex triangle strip, PRIM 0x7c), `(y, z, s, t)`: boot 0x1debb0 / level01
/// 0x1c3130, positions `(0, y, z, 1)` and ST `(s, t, 1)`. y scales the horizontal axis, z the height.
pub const BILLBOARD_CORNERS: [[f32; 4]; 4] = [[-0.5, 1.0, 0.0, 0.0], [0.5, 1.0, 1.0, 0.0], [-0.5, 0.0, 0.0, 1.0], [0.5, 0.0, 1.0, 1.0]];

/// World-space corners (game axes, Z up) of an instance's billboard: origin `t` (matrix column 3), eye = camera
/// position. `ShrubProc`: `d = (t − eye) · rsqrt(|t − eye|²)`; axes `(d.y, −d.x, 0)` (horizontal, length
/// cos(elevation): the quad narrows seen from above) and `(0, 0, 1)`; corner = `t + y·W·(d.y, −d.x, 0) + (z·H + Z)·ẑ`.
pub fn billboard_corners(t: [f32; 3], eye: [f32; 3], extent: [f32; 3]) -> [[f32; 3]; 4] {
    let d = [t[0] - eye[0], t[1] - eye[1], t[2] - eye[2]];
    let q = 1.0 / ((d[0] * d[0] + d[1] * d[1]) + d[2] * d[2]).sqrt();
    let (dx, dy) = (d[0] * q, d[1] * q);
    BILLBOARD_CORNERS.map(|[y, z, _, _]| {
        let (h, v) = (y * extent[0], z * extent[1] + extent[2]);
        [t[0] + h * dy, t[1] - h * dx, t[2] + v]
    })
}

/// `ShrubProc`'s per-instance mesh / billboard alpha (level01 0x29d274..0x29d32c, boot 0x22906c..0x229124) for
/// an instance that passed the distance and frustum tests. `z` = view depth of the bounding-sphere centre, `d` =
/// run-time draw distance, `f` = the class's billboard byte F (None = no billboard). Alphas are GS 0..0x80;
/// a billboard alpha of 0 is listed but never drawn (the passes select `& 0x80` / `& 0x7f`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShrubFade { pub mesh: Option<u8>, pub billboard: Option<u8> }

pub fn shrub_fade(z: f32, d: f32, f: Option<u8>) -> ShrubFade {
    let dz = ((d - z) * 4096.0) as i32; // vftoi12 (D − z), lane y of vf3
    let far = || ((dz >> 1).min(0x8000) >> 8) as u8;
    match f {
        None => ShrubFade { mesh: Some((dz.min(0x8000) >> 8) as u8), billboard: None },
        Some(0) => ShrubFade { mesh: None, billboard: Some(far()) },
        Some(f) => {
            let iz = (z.max(0.0) * 4096.0) as i32 - ((f as i32) << 12);
            if iz < 0 { return ShrubFade { mesh: Some(0x80), billboard: None }; }
            let a = (iz.min(0x8000) >> 8) as u8;
            if a == 0x80 { ShrubFade { mesh: None, billboard: Some(far()) } } else { ShrubFade { mesh: Some(((0x8000 - iz) >> 8) as u8), billboard: Some(a) } }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Wind sway (`mode_bits & 6`): level01 `ShrubProc` 0x29d4f8..0x29d670 = boot 0x2292f0..0x229468
// ---------------------------------------------------------------------------------------------------------

/// `vf8` (boot 0x1dfef0, level01 0x1c4470): gust scale, sway amplitude, x lean weight, y lean weight.
pub const SWAY_VF8: [f32; 4] = [0.1, 0.02, 1.0, 0.0];
/// `vf9` (boot 0x1dff00, level01 0x1c4480): gust depth, range² (world units²), 1 / range², mode-1 factor.
pub const SWAY_VF9: [f32; 4] = [0.2, 6000.0, f32::from_bits(0x392e_c33e), 0.5];

/// The 256-entry signed-byte table `ShrubProc` copies to scratchpad 0x70003b00 (boot 0x1dfdf0, level01 0x1c4370):
/// entry i = `trunc(−127 · sin(2πi / 256))` (every entry checked against the ELF data).
pub fn sway_table(i: u32) -> i8 { (-127.0 * (std::f64::consts::TAU * (i & 0xff) as f64 / 256.0).sin()).trunc() as i8 }

/// `lb` + `sll 5` + `vitof12`: table entry / 128.
fn sway_sin(i: u32) -> f32 { sway_table(i) as f32 / 128.0 }

/// The wind shear of one instance this frame, `(sx, sy)`: `ShrubProc` replaces the scaled instance columns
/// `c` by `(c.x + sx·c.z, c.y + sy·c.z, c.z)` (world Z-up shear about the instance origin) before building the
/// VU1 matrix. `mode` = `(mode_bits & 6) >> 1` (0 = none), `block` = EE address of the instance's 0x40-byte
/// matrix block, `tick` = the frame counter (boot 0x15f438 / level01 0x15f3f8, +1 per frame in the main
/// loop), `rel` = origin − camera position (world units). None: no sway (mode 0, or `|rel|² > 6000`).
///
/// ```text
/// u = block·67 + tick,  w = block·123 + tick·(mode ≥ 2 ? 2 : 1)          (u32, wrapping)
/// s(i) = table[i & 0xff] / 128
/// g = ((s(u)·s(u >> 1))·0.2 + (1 − 0.2))·0.1
/// k = 1 − |rel|²·(1/6000)
/// sx = (s(w + 64)·0.02 + g·1.0)·k,  sy = (s(w >> 1)·0.02 + g·0.0)·k,  both ·0.5 when mode = 1
/// ```
pub fn wind_sway(mode: u16, block: u32, tick: u32, rel: [f32; 3]) -> Option<[f32; 2]> {
    if mode == 0 { return None; }
    let d2 = (rel[0] * rel[0] + rel[1] * rel[1]) + 1.0 * (rel[2] * rel[2]);
    if SWAY_VF9[1] - d2 < 0.0 { return None; }
    let k = 1.0 - d2 * SWAY_VF9[2];
    let double = mode >> 1 != 0;
    let u = block.wrapping_mul(67).wrapping_add(tick);
    let w = block.wrapping_mul(123).wrapping_add(tick).wrapping_add(if double { tick } else { 0 });
    let g = ((sway_sin(u) * sway_sin(u >> 1)) * SWAY_VF9[0] + (1.0 - SWAY_VF9[0])) * SWAY_VF8[0];
    let mut sx = (sway_sin(w.wrapping_add(0x40)) * SWAY_VF8[1] + g * SWAY_VF8[2]) * k;
    let mut sy = (sway_sin(w >> 1) * SWAY_VF8[1] + g * SWAY_VF8[3]) * k;
    if !double { sx *= SWAY_VF9[3]; sy *= SWAY_VF9[3]; }
    Some([sx, sy])
}

/// Gameplay-file header offset of the shrub class-number list (`s32 count`, `count` x `s32`).
pub const GAMEPLAY_SHRUB_CLASSES: usize = 0x38;
/// Gameplay-file header offset of the shrub instance section pointer.
pub const GAMEPLAY_SHRUB_INSTANCES: usize = 0x3c;

/// Gameplay shrub instance (0x70 bytes on disc). Spec 1.8; field use from the level loader
/// (level01 `FUN_00255958`, the loop after pointer 0x3c), which builds a 0x20-byte runtime record, a
/// 0x40-byte matrix block and a 0x60-byte palette slot per instance. There is no uid: an instance's
/// identity is its index in the section (the loader's runtime index, record +0x18).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ShrubInstance {
    /// 0x00: class number, resolved against [`LevelShrubClass::o_class`].
    pub o_class: i32,
    /// 0x04: draw distance (f32, world units); the loader clamps it to >= 16 and, for billboard classes,
    /// to >= `trunc(fade_distance) + 24`.
    pub draw_distance: f32,
    /// 0x08: zero.
    pub unused_08: i32,
    /// 0x0c: zero (no occlusion index: shrubs are not occlusion-culled).
    pub unused_0c: i32,
    /// 0x10: class-to-world matrix, column-major (`matrix[col][row]`), translation in column 3, world units.
    /// `[3][3]` is 0.01 on the disc and kept raw; use [`ShrubInstance::world_matrix`].
    pub matrix: [[f32; 4]; 4],
    /// 0x50: base (ambient) colour r, g, b, 0..255; the loader packs `r | g << 8 | b << 16 | 0x80 << 24`.
    pub colour: [i32; 3],
    /// 0x5c: zero.
    pub unused_5c: i32,
    /// 0x60: light-set selector (low 16 bits → runtime record +0x1c): bits 0..3 light set A, 4..7 set B,
    /// 8..15 blend weight of B (0 = A only). See [`ShrubInstance::light_sets`].
    pub dir_lights: i32,
    /// 0x64..0x6f: zero.
    pub unused_64: [i32; 3],
}
const _: () = assert!(std::mem::size_of::<ShrubInstance>() == 0x70);

impl ShrubInstance {
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
    /// Ambient RGBA exactly as the loader packs it for the lighting pass (alpha 0x80).
    pub fn ambient_rgba(&self) -> [u8; 4] { [self.colour[0] as u8, self.colour[1] as u8, self.colour[2] as u8, 0x80] }
    /// Light sets and blend: `(a, b, weight_of_b)`, weight in 1/256. With weight 0 the lighting pass uses
    /// set `a` only.
    pub fn light_sets(&self) -> (u8, u8, u8) {
        let s = self.dir_lights as u16;
        ((s & 0xf) as u8, ((s >> 4) & 0xf) as u8, (s >> 8) as u8)
    }
}

/// Parses a shrub instance section (`s32 count`, 12 bytes pad, `count` x 0x70 records).
pub fn parse_shrub_instance_section(section: &[u8]) -> Result<Vec<ShrubInstance>> {
    let b = Buf(section);
    let count = b.i32(0)?;
    if !(0..=100_000).contains(&count) { return invalid("implausible shrub instance count"); }
    b.pod_slice(0x10, count as usize, "shrub instances")
}

/// Parses the shrub instances of a decompressed gameplay file (section pointer at 0x3c).
pub fn parse_shrub_instances(gameplay: &[u8]) -> Result<Vec<ShrubInstance>> {
    let b = Buf(gameplay);
    let ofs = b.u32(GAMEPLAY_SHRUB_INSTANCES)? as usize;
    if ofs == 0 { return Ok(Vec::new()); }
    parse_shrub_instance_section(b.tail(ofs, "shrub instance section")?.bytes())
}

/// Parses the gameplay shrub class-number list (pointer at 0x38; the distinct instance classes).
pub fn parse_shrub_class_list(gameplay: &[u8]) -> Result<Vec<i32>> {
    let b = Buf(gameplay);
    let ofs = b.u32(GAMEPLAY_SHRUB_CLASSES)? as usize;
    if ofs == 0 { return Ok(Vec::new()); }
    let count = b.i32(ofs)?;
    if !(0..=10_000).contains(&count) { return invalid("implausible shrub class count"); }
    b.pod_slice(ofs + 4, count as usize, "shrub class list")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::bytes_of;

    fn w(v: &mut Vec<u8>, words: &[u32]) { for x in words { v.extend(x.to_le_bytes()); } }
    fn tag(nloop: u32, eop: bool, prim: u64, slot: i32) -> ShrubGifTag {
        ShrubGifTag { tag: nloop as u64 | (eop as u64) << 15 | 1 << 46 | prim << 47 | 3 << 60, tag_hi: 0x412, gs_packet_offset: slot }
    }
    fn adgif(slot: i32, tex: i32) -> ShrubAdGifs {
        let mut a = ShrubAdGifs::default();
        a.tex1.pad[3..7].copy_from_slice(&slot.to_le_bytes());
        a.tex0.data_lo = tex;
        a
    }
    /// A packet VIF list: header block (V4_32 at 0), part 1 and part 2 (V4_16) at the header's addresses.
    fn packet(tags: &[ShrubGifTag], ads: &[ShrubAdGifs], verts: &[(i16, u16)], vertex_offset: Option<i32>) -> Vec<u8> {
        let vo = vertex_offset.unwrap_or(1 + tags.len() as i32 + 4 * ads.len() as i32);
        let n = verts.len();
        let mut v = Vec::new();
        w(&mut v, &[0x0100_0404, 0, 0x0500_0000]);
        let hq = 1 + tags.len() + 4 * ads.len();
        w(&mut v, &[0x6c00_8000 | (hq as u32) << 16]);
        v.extend(bytes_of(&ShrubPacketHeader { texture_count: ads.len() as i32, gif_tag_count: tags.len() as i32, vertex_count: n as i32, vertex_offset: vo }));
        for t in tags { v.extend(bytes_of(t)); }
        for a in ads { v.extend(bytes_of(a)); }
        w(&mut v, &[0x0500_0000, 0x6d00_8000 | (n as u32) << 16 | vo as u32]);
        for (i, &(slot, _)) in verts.iter().enumerate() { v.extend(bytes_of(&ShrubVertexPart1 { x: i as i16, y: 10, z: 20, gs_packet_offset: slot })); }
        w(&mut v, &[0x0500_0000, 0x6d00_8000 | (n as u32) << 16 | (vo as u32 + n as u32)]);
        for (i, &(_, nst)) in verts.iter().enumerate() { v.extend(bytes_of(&ShrubVertexPart2 { s: 4096, t: -(i as i16), q: 0x1000, n_and_stop: nst })); }
        v
    }
    fn class_blob(packets: &[Vec<u8>], billboard: bool) -> Vec<u8> {
        let mut table = Vec::new();
        let mut data = Vec::new();
        let first = 0x40 + 8 * packets.len();
        for p in packets {
            table.push(ShrubPacketEntry { offset: (first + data.len()) as i32, size: p.len() as i32 });
            data.extend(p);
            while data.len() % 16 != 0 { data.push(0); }
        }
        let bb = first + data.len();
        let normals = bb + if billboard { 0x40 } else { 0 };
        let h = ShrubClassHeader { scale: 2.0, o_class: 5, packet_count: packets.len() as i16, normals_offset: normals as i32,
            billboard_offset: if billboard { bb as i32 } else { 0 }, ..Default::default() };
        let mut b = bytes_of(&h).to_vec();
        for e in &table { b.extend(bytes_of(e)); }
        b.extend(data);
        if billboard { b.extend(bytes_of(&ShrubBillboard { fade_distance: 40.0, width: 2.0, height: 3.0, z_ofs: 1.0, ..Default::default() })); }
        for i in 0..24i16 { b.extend(bytes_of(&[i, 0, 0, 0])); }
        b
    }

    /// Ad-gif at 0, strip A (3 verts) at 5, strip B (4 verts, EOP) at 15; 8 vertices: 7 real + 1 padding
    /// vertex that rewrites vertex 6's slot. Stop bit on vertex 4 (= 8 - 4).
    fn sample() -> Vec<u8> {
        let tags = [tag(3, false, 4, 5), tag(4, true, 4, 15)];
        let verts = [(6, 3), (9, 1), (12, 2), (16, 7), (19, 8), (22, 9), (25, 10), (25, 11)];
        let mut verts = verts.to_vec();
        verts[4].1 |= 0x8000;
        packet(&tags, &[adgif(0, 2)], &verts, None)
    }

    #[test]
    fn walker_follows_vu1_rules() {
        let sc = parse_shrub_class(&class_blob(&[sample()], true)).unwrap();
        assert_eq!(sc.normal(3), Some([3.0 / 32767.0, 0.0, 0.0]));
        assert_eq!(sc.billboard.unwrap().height, 3.0);
        let pk = &sc.packets[0];
        assert_eq!(pk.vertices.len(), 8);
        assert_eq!((pk.vertices[4].stop, pk.vertices[4].normal, pk.vertices[3].normal), (1, 8, 7));
        assert_eq!(pk.vertices[2].uv(), [1.0, -2.0 / 4096.0]);
        assert_eq!(pk.vertices[1].class_position(2.0), [1.0 / 512.0, 20.0 / 1024.0, 40.0 / 1024.0]);
        // The padding vertex 7 wins slot 25 over vertex 6.
        assert_eq!(pk.draws, [
            ShrubDraw { texture: 2, prim: 4, vertices: vec![0, 1, 2] },
            ShrubDraw { texture: 2, prim: 4, vertices: vec![3, 4, 5, 7] },
        ]);
        assert_eq!(shrub_triangles(pk), [
            ShrubTriangle { a: 0, b: 1, c: 2, texture: 2 },
            ShrubTriangle { a: 3, b: 4, c: 5, texture: 2 },
            ShrubTriangle { a: 7, b: 5, c: 4, texture: 2 },
        ]);
    }

    #[test]
    fn stop_bit_rules() {
        let tags = [tag(6, true, 4, 5)];
        let base: Vec<(i16, u16)> = (0..6).map(|i| (6 + 3 * i as i16, 0)).collect();
        let run = |v: &[(i16, u16)]| read_packet(&packet(&tags, &[adgif(0, 1)], v, None), ShrubPacketEntry::default(), &mut -1);
        // A stop bit on vertex 0 or 1 is never tested: the flag on vertex 2 ends the loop.
        let mut v = base.clone();
        v[1].1 = 0x8000;
        v[2].1 = 0x8000;
        let pk = run(&v).unwrap();
        assert_eq!(pk.vertices.len(), 6);
        assert_eq!(pk.draws[0].vertices, [0, 1, 2, 3, 4, 5]);
        // Without a flag from index 2 on, VU1 reads past the tables into quadwords no unpack wrote.
        let mut v = base.clone();
        v[0].1 = 0x8000;
        assert!(run(&v).is_err());
        // A flag on vertex 3 of 6 makes VU1 process 7 vertices; the 7th reads unwritten memory.
        let mut v = base.clone();
        v[3].1 = 0x8000;
        assert!(run(&v).is_err());
    }

    #[test]
    fn texture_state_carries_across_packets() {
        let tags = [tag(6, true, 4, 0)];
        let verts: Vec<(i16, u16)> = (0..6).map(|i| (1 + 3 * i as i16, if i == 2 { 0x8000 } else { 0 })).collect();
        // Second packet: its ad-gif block sits after the EOP tag, so the GIF never reads it.
        let p2 = packet(&tags, &[adgif(0x40, 9)], &verts, None);
        let sc = parse_shrub_class(&class_blob(&[sample(), p2.clone()], false)).unwrap();
        assert_eq!(sc.packets[1].draws[0].texture, 2);
        assert!(sc.billboard.is_none());
        // Alone, the same packet draws before any texture was set.
        assert!(parse_shrub_class(&class_blob(&[p2], false)).is_err());
    }

    #[test]
    fn malformed_packets_are_rejected() {
        let good = sample();
        let mut t = -1;
        assert!(read_packet(&good, ShrubPacketEntry::default(), &mut t).is_ok());
        // A strip longer than the vertices written.
        let tags = [tag(3, false, 4, 5), tag(5, true, 4, 15)];
        let mut verts = vec![(6, 3), (9, 1), (12, 2), (16, 7), (19, 8), (22, 9), (25, 10), (25, 11)];
        verts[4].1 |= 0x8000;
        assert!(read_packet(&packet(&tags, &[adgif(0, 2)], &verts, None), ShrubPacketEntry::default(), &mut t).is_err());
        // A vertex overwriting the ad-gif block.
        let mut v2 = verts.clone();
        v2[0].0 = 2;
        assert!(read_packet(&packet(&[tag(3, false, 4, 5), tag(4, true, 4, 15)], &[adgif(0, 2)], &v2, None), ShrubPacketEntry::default(), &mut t).is_err());
        // Triangle fans are not a shrub primitive.
        assert!(read_packet(&packet(&[tag(3, false, 5, 5), tag(4, true, 4, 15)], &[adgif(0, 2)], &verts, None), ShrubPacketEntry::default(), &mut t).is_err());
        // Vertex tables are read by address: elsewhere is fine when the unpacks go there too ...
        assert!(read_packet(&packet(&[tag(3, false, 4, 5), tag(4, true, 4, 15)], &[adgif(0, 2)], &verts, Some(40)), ShrubPacketEntry::default(), &mut t).is_ok());
        // ... but not where no unpack wrote.
        let mut bad = good.clone();
        bad[4 * 4 + 12..4 * 4 + 16].copy_from_slice(&50i32.to_le_bytes()); // header vertex_offset -> 50
        assert!(read_packet(&bad, ShrubPacketEntry::default(), &mut t).is_err());
    }

    #[test]
    fn ad_gif_conversion_is_the_tfrag_rule() {
        use crate::tfrag::TfragAdGifs;
        let e = TextureEntry { data_offset: 0x1000, width: 128, height: 64, ty: 4, palette: 231, mipmap: 2879, pad: 2878 };
        for (k, hi, wms, wmt) in [(-110i32, 4, 0, 0), (-142, 4, 0, 1), (-126, 2, 1, 1)] {
            let mut s = ShrubAdGifs::default();
            s.tex1.data_lo = k & 0xffff;
            s.tex1.data_hi = hi;
            s.clamp.data_lo = wms;
            s.clamp.data_hi = wmt;
            s.tex0.data_lo = 3; // class slot; the table index (7) comes from the class entry
            let r = s.gs_registers(7, &e, 0x2_0000);
            let mut t = TfragAdGifs::default();
            t.tex0.data_lo = 7;
            t.tex1 = s.tex1;
            t.clamp = s.clamp;
            let tr = t.gs_registers(&e, 0x2_0000);
            assert_eq!((r.tex0, r.tex1, r.clamp, r.miptbp1), (tr.tex0, tr.tex1, tr.clamp, tr.miptbp1));
            assert_eq!(s.lod_k_raw(), k as i16);
        }
    }

    #[test]
    fn billboard_registers_and_quad() {
        let info = ShrubBillboardInfo { width: 32, height: 32, max_mip: 3, palette_offset: 3061, texture_offset: 3065, mip1: 3069, mip2: 3070, mip3: 0 };
        let mut b = ShrubBillboard { fade_distance: 15.0, width: 27724.238, height: 27724.238, z_ofs: 0.0, ..Default::default() };
        b.tex1.data_lo = 0xff56;
        b.tex1.data_hi = 4;
        let r = b.gs_registers(&info, 0);
        assert_eq!((r.mxl(), r.lod_k()), (2, -170.0 / 16.0));
        assert_eq!(r.tex1 & 0xffff_ffff, 2 << 2 | 4 << 6 | 0x20);
        assert_eq!(r.tex0 & 0x3fff, 3065);
        assert_eq!(r.tex0 >> 14 & 0x3f, 1); // TBW0 = max(1, 32/64)
        assert_eq!((r.tex0 >> 26 & 0xf, r.tex0 >> 30 & 0xf, r.tex0 >> 37 & 0x3fff), (5, 5, 3061));
        assert_eq!((r.miptbp1 & 0x3fff, r.miptbp1 >> 20 & 0x3fff, r.miptbp1 >> 40 & 0x3fff), (3069, 3070, 0));
        // An unscaled instance of scale 0.036935188: width · scale = 1024 → a 1-unit quad from the origin up.
        let mut m = [[0.0f32; 4]; 4];
        m[0][0] = 1.0; m[1][1] = 1.0; m[2][2] = 2.0;
        let packed = packed_column_lengths(&m);
        assert_eq!(packed, 0x1000 | 0x2000 << 16);
        let ext = billboard_extent(&b, 0.036935188, packed);
        assert!((ext[0] - 1.0).abs() < 1e-5 && (ext[1] - 2.0).abs() < 1e-5 && ext[2] == 0.0);
        // Camera due -y of the origin, level with it: the quad spans x = ±0.5 (s = 0 on the left, -x), z = 0..2.
        let c = billboard_corners([10.0, 20.0, 5.0], [10.0, 0.0, 5.0], ext);
        assert!((c[0][0] - 9.5).abs() < 1e-5 && (c[0][2] - 7.0).abs() < 1e-5);
        assert!((c[3][0] - 10.5).abs() < 1e-5 && (c[3][2] - 5.0).abs() < 1e-5);
        // Seen from 45° above, the width shrinks by cos 45°.
        let c = billboard_corners([0.0, 10.0, 0.0], [0.0, 0.0, 10.0], ext);
        assert!(((c[1][0] - c[0][0]) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        // A column length of exactly 16 (0x10000) spills into bit 16.
        m[0][0] = 16.0; m[1][1] = 16.0; m[2][2] = 17.0;
        assert_eq!(packed_column_lengths(&m), 0x10000);
    }

    #[test]
    fn fade_rules() {
        assert_eq!(shrub_fade(10.0, 32.0, None), ShrubFade { mesh: Some(0x80), billboard: None });
        assert_eq!(shrub_fade(28.0, 32.0, None), ShrubFade { mesh: Some(64), billboard: None });
        assert_eq!(shrub_fade(10.0, 200.0, Some(15)), ShrubFade { mesh: Some(0x80), billboard: None });
        assert_eq!(shrub_fade(17.0, 200.0, Some(15)), ShrubFade { mesh: Some(96), billboard: Some(32) });
        assert_eq!(shrub_fade(23.0, 200.0, Some(15)), ShrubFade { mesh: None, billboard: Some(0x80) });
        assert_eq!(shrub_fade(196.0, 200.0, Some(15)), ShrubFade { mesh: None, billboard: Some(32) });
        assert_eq!(shrub_fade(1.0, 24.0, Some(0)), ShrubFade { mesh: None, billboard: Some(0x80) });
    }

    #[test]
    fn sway() {
        // The table is a negated sine.
        assert_eq!([sway_table(0), sway_table(20), sway_table(64), sway_table(128), sway_table(192)], [0, -59, -127, 0, 127]);
        assert_eq!(wind_sway(0, 0x100000, 5, [1.0, 1.0, 0.0]), None);
        assert_eq!(wind_sway(2, 0x100000, 5, [70.0, 40.0, 0.0]), None); // 4900 + 1600 > 6000
        // Block 0, tick 0: s(0) = 0 → g = 0.08; w = 0: s(64) = -127/128, s(0) = 0.
        let [sx, sy] = wind_sway(2, 0, 0, [0.0; 3]).unwrap();
        assert!((sx - (0.08 - 0.02 * 127.0 / 128.0)).abs() < 1e-6 && sy == 0.0);
        let [sx1, _] = wind_sway(1, 0, 0, [0.0; 3]).unwrap();
        assert!((sx1 - sx * 0.5).abs() < 1e-7);
        // Attenuation is linear in the squared distance.
        let [sxa, _] = wind_sway(2, 0, 0, [0.0, 0.0, 3000f32.sqrt()]).unwrap();
        assert!((sxa - sx * 0.5).abs() < 1e-4);
    }

    #[test]
    fn instance_helpers() {
        let mut inst = ShrubInstance { o_class: 3, draw_distance: 64.0, colour: [10, 20, 30], dir_lights: 0x4021, ..Default::default() };
        inst.matrix = [[2.0, 0.0, 0.0, 0.0], [0.0, 3.0, 0.0, 0.0], [0.0, 0.0, 4.0, 0.0], [10.0, 20.0, 30.0, 0.01]];
        assert_eq!(inst.world_matrix()[3], [10.0, 20.0, 30.0, 1.0]);
        assert_eq!(inst.transform_point([1.0, 1.0, 1.0]), [12.0, 23.0, 34.0]);
        assert_eq!(inst.ambient_rgba(), [10, 20, 30, 0x80]);
        assert_eq!(inst.light_sets(), (1, 2, 0x40));
        let mut gp = vec![0u8; 0x40];
        gp[GAMEPLAY_SHRUB_CLASSES..GAMEPLAY_SHRUB_CLASSES + 4].copy_from_slice(&0x40u32.to_le_bytes());
        gp.extend([1, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let at = gp.len() as u32;
        gp[GAMEPLAY_SHRUB_INSTANCES..GAMEPLAY_SHRUB_INSTANCES + 4].copy_from_slice(&at.to_le_bytes());
        gp.extend([1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        gp.extend(bytes_of(&inst));
        assert_eq!(parse_shrub_class_list(&gp).unwrap(), [3]);
        let got = parse_shrub_instances(&gp).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(bytes_of(&got[0]), bytes_of(&inst));
    }
}
