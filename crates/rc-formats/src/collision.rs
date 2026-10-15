//! Level collision mesh. Spec: docs/formats/collision_rac1.md; runtime queries:
//! docs/plan/collision_queries.md. Ported from the retired C++ reference
//! extractor (git 2230812; `parse_collision`, `collision_triangles`); `tests/formats/golden.rs` checks every section against the committed snapshot table (`data/loader_snapshots.tsv`).
//!
//! RAC1 has one collision block per level, at `LevelCoreHeader::collision` in the decompressed
//! core data. It holds the baked world-space mesh of all static geometry (tfrags plus whatever
//! tie/shrub/moby collision the level build merged in), bucketed into a sparse grid of 4x4x4-unit
//! cells indexed Z -> Y -> X, followed by the hero (player-only) collision groups. Every cell has
//! its own vertices and faces: a face overlapping N cells is stored N times, so a point query
//! only ever reads one cell. Per-class moby collision is a separate format (moby class header
//! +0x10) and is not covered here.
//!
//! Tree offsets (slab, row and leaf) are byte offsets from the start of the mesh
//! ([`CollisionHeader::mesh`]), not from the block.

use crate::buf::{invalid, Buf, Result};
use crate::level::LevelCore;
use bytemuck::{Pod, Zeroable};
use std::collections::BTreeMap;

/// Edge length of a grid cell in world units. A cell with coordinates `c` spans
/// `[4c, 4c + 4)` on each axis and has its centre at `4c + 2`.
pub const CELL_SIZE: f32 = 4.0;

/// Block header (8 bytes; bytes 0x08..`mesh` are zero padding on every retail level).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct CollisionHeader {
    /// 0x0: offset of the mesh (root node) from the block start; 0x40 on every retail level.
    pub mesh: i32,
    /// 0x4: offset of the hero-collision section from the block start, or 0 for none. Every
    /// retail level has one (possibly with zero groups), and it directly follows the mesh.
    pub hero_groups: i32,
}
const _: () = assert!(std::mem::size_of::<CollisionHeader>() == 8);

/// Header shared by the three tree levels (4 bytes), followed by `count` entries.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct CollisionNodeHeader {
    /// 0x0: cell coordinate of entry 0 on this node's axis (Z for the root, Y for a slab, X for a row).
    pub base: i16,
    /// 0x2: number of entries.
    pub count: u16,
}

/// Root node at the mesh start: one `u16` entry per Z slab; slab node offset = entry * 4, 0 = empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollisionRoot {
    pub header: CollisionNodeHeader,
    pub slabs: Vec<u16>,
}

/// A non-empty Z slab: one `u32` entry per Y row, a byte offset of the row node, 0 = empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollisionSlab {
    /// Byte offset of this node from the mesh start (= root entry * 4).
    pub offset: u32,
    /// Cell Z coordinate of the slab.
    pub z: i16,
    pub header: CollisionNodeHeader,
    pub rows: Vec<u32>,
}

/// A non-empty Y row: one `u32` leaf word per X cell ([`CollisionCell::leaf_word`]), 0 = empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollisionRow {
    /// Byte offset of this node from the mesh start.
    pub offset: u32,
    /// Cell Z and Y coordinates of the row.
    pub z: i16,
    pub y: i16,
    pub header: CollisionNodeHeader,
    pub cells: Vec<u32>,
}

/// Leaf header (4 bytes). The leaf continues with `vertex_count` packed vertices, `face_count`
/// face records (quads first), `quad_count` fourth indices, and zero padding to 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct CollisionLeafHeader {
    /// 0x0: quads + triangles.
    pub face_count: u16,
    /// 0x2
    pub vertex_count: u8,
    /// 0x3: the first `quad_count` faces are quads.
    pub quad_count: u8,
}

impl CollisionLeafHeader {
    /// Payload bytes before padding: `4 + 4V + 4F + Q`.
    pub fn payload_size(&self) -> usize { 4 + 4 * self.vertex_count as usize + 4 * self.face_count as usize + self.quad_count as usize }
    /// Leaf size in quadwords, which the tree word's low byte must equal.
    pub fn size_qwords(&self) -> usize { self.payload_size().div_ceil(16) }
}

/// A packed cell-relative vertex (u32). Bit fields, all signed two's complement:
/// X = bits 0..10 at 1/16, Y = bits 10..20 at 1/16, Z = bits 20..32 at 1/64, each relative to
/// the cell centre. Extraction is by shift pairs (`(w << 22) >> 22`, `(w << 12) >> 22`,
/// `w >> 20`, arithmetic right shifts on the signed word), as in the retired C++ reference
/// and Wrench's `read_collision_mesh`; the scales are Wrench's, confirmed on the disc by the
/// collision map coinciding with the tfrag terrain (spec 6b).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct PackedCollisionVertex(pub u32);

impl PackedCollisionVertex {
    /// Raw signed fields `[x, y, z]`: x, y in -512..=511, z in -2048..=2047.
    pub fn fields(self) -> [i32; 3] {
        let w = self.0;
        [((w << 22) as i32) >> 22, ((w << 12) as i32) >> 22, (w as i32) >> 20]
    }
    /// Offset from the cell centre in world units (`x/16`, `y/16`, `z/64`; exact in f32).
    pub fn offset(self) -> [f32; 3] {
        let [x, y, z] = self.fields();
        [x as f32 / 16.0, y as f32 / 16.0, z as f32 / 64.0]
    }
    /// World position given the cell centre, computed as `centre + offset` per component in
    /// f32 like the retired C++ reference (every intermediate is exactly representable).
    pub fn world(self, centre: [f32; 3]) -> [f32; 3] {
        let o = self.offset();
        [centre[0] + o[0], centre[1] + o[1], centre[2] + o[2]]
    }
    /// Inverse of [`fields`](Self::fields) for in-range values (used by tests and tools).
    pub fn pack(x: i32, y: i32, z: i32) -> Self {
        Self((x as u32 & 0x3ff) | ((y as u32 & 0x3ff) << 10) | ((z as u32 & 0xfff) << 20))
    }
}

/// Face record (4 bytes). For a quad the fourth index is in [`CollisionCell::quad_v3`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct CollisionFace {
    /// 0x0: indices into the cell's vertex array.
    pub v: [u8; 3],
    /// 0x3: collision type byte. Bits 0-4 surface id (0x1f = none), bits 5-6 footstep sound
    /// class, bit 7 excluded by queries with flag 0x80 (docs/plan/collision_queries.md section 3).
    pub surface: u8,
}

impl CollisionFace {
    /// The game's surface-id getter (level01 0x2151d8) on this face: `type & 0x1f`, or -1 for 0x1f.
    pub fn surface_id(self) -> i32 { if self.surface & 0x1f == 0x1f { -1 } else { (self.surface & 0x1f) as i32 } }
    /// The game's footstep sound-class getter (level01 0x215208): `(type & 0x60) >> 5`, 3 maps to 0.
    pub fn sound_class(self) -> u8 { if self.surface & 0x60 == 0x60 { 0 } else { (self.surface & 0x60) >> 5 } }
    /// Bit 7: the face is skipped by queries that pass flag 0x80.
    pub fn high_bit(self) -> bool { self.surface & 0x80 != 0 }
}

/// One non-empty grid cell (a tree leaf) with its payload.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollisionCell {
    /// Cell coordinates (`base + index` on each tree level).
    pub x: i16,
    pub y: i16,
    pub z: i16,
    /// Tree entry: `leaf offset from mesh << 8 | leaf size in quadwords`.
    pub leaf_word: u32,
    pub header: CollisionLeafHeader,
    pub packed: Vec<PackedCollisionVertex>,
    /// World-space positions of `packed` (see [`PackedCollisionVertex::world`]).
    pub vertices: Vec<[f32; 3]>,
    /// Quads first (`header.quad_count` of them), then triangles.
    pub faces: Vec<CollisionFace>,
    /// Fourth index of each quad, in quad order.
    pub quad_v3: Vec<u8>,
}

impl CollisionCell {
    pub fn coords(&self) -> [i16; 3] { [self.x, self.y, self.z] }
    /// Byte offset of the leaf from the mesh start.
    pub fn leaf_offset(&self) -> u32 { self.leaf_word >> 8 }
    /// The tree word's low byte: leaf size in 16-byte units.
    pub fn leaf_qwords(&self) -> u32 { self.leaf_word & 0xff }
    /// Cell centre `4c + 2` per axis, computed as `c * 4.0 + 2.0` in f32 like the C++.
    pub fn centre(&self) -> [f32; 3] { [self.x, self.y, self.z].map(|c| c as f32 * 4.0 + 2.0) }
    /// World-space bounds `[4c, 4c + 4)` of the cell. Faces may extend past them (vertices can
    /// lie up to 32 units from the centre); the cell only owns what overlaps it.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let lo = [self.x, self.y, self.z].map(|c| c as f32 * CELL_SIZE);
        (lo, lo.map(|v| v + CELL_SIZE))
    }
    pub fn is_quad(&self, face: usize) -> bool { face < self.header.quad_count as usize }
    /// The face's four indices; a triangle repeats `v0` as the fourth, like the C++ `CollisionFace::v`.
    pub fn face_indices(&self, face: usize) -> [u8; 4] {
        let f = self.faces[face];
        [f.v[0], f.v[1], f.v[2], if self.is_quad(face) { self.quad_v3[face] } else { f.v[0] }]
    }
}

/// Hero-collision group record (0x10 bytes), relative to the hero section.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct HeroGroupHeader {
    /// 0x0: bounding sphere x, y, z, radius, unsigned at 1/64.
    pub sphere: [u16; 4],
    /// 0x8
    pub triangle_count: u16,
    /// 0xa
    pub vertex_count: u16,
    /// 0xc: offset of the group data from the hero-section start.
    pub data: u32,
}
const _: () = assert!(std::mem::size_of::<HeroGroupHeader>() == 0x10);

impl HeroGroupHeader {
    /// Bounding sphere `[x, y, z, r]` in world units (`u16 / 64`).
    pub fn sphere_world(&self) -> [f32; 4] { self.sphere.map(|v| v as f32 / 64.0) }
}

/// Hero vertex (8 bytes): unsigned world position at 1/64; `pad` is zero (validated).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct HeroVertex {
    pub xyz: [u16; 3],
    pub pad: u16,
}
impl HeroVertex {
    pub fn position(&self) -> [f32; 3] { self.xyz.map(|v| v as f32 / 64.0) }
}

/// Hero triangle (4 bytes): three group-local indices and a zero pad byte (validated). No surface id.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct HeroTriangle {
    pub v: [u8; 3],
    pub pad: u8,
}

/// Player-only collision (invisible walls, fences): a flat list of groups with bounding spheres.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeroGroup {
    pub header: HeroGroupHeader,
    pub vertices: Vec<HeroVertex>,
    pub triangles: Vec<HeroTriangle>,
}

/// The parsed collision block. Nodes and cells are in tree-walk order (Z, then Y, then X
/// ascending), which is also the order of the retired C++ reference.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collision {
    pub header: CollisionHeader,
    pub root: CollisionRoot,
    pub slabs: Vec<CollisionSlab>,
    pub rows: Vec<CollisionRow>,
    pub cells: Vec<CollisionCell>,
    /// The hero section's `s32` group count (0 when `header.hero_groups == 0`).
    pub hero_group_count: i32,
    pub hero_groups: Vec<HeroGroup>,
}

impl Collision {
    /// Index into [`cells`](Self::cells) of the cell at the given cell coordinates, found by
    /// walking the parsed tree Z -> Y -> X like [`lookup_cell_word`].
    pub fn cell_at(&self, x: i16, y: i16, z: i16) -> Option<usize> {
        self.cells.binary_search_by_key(&(z, y, x), |c| (c.z, c.y, c.x)).ok()
    }
    /// Face counts per surface id.
    pub fn surface_counts(&self) -> BTreeMap<u8, usize> {
        let mut m = BTreeMap::new();
        for c in &self.cells { for f in &c.faces { *m.entry(f.surface).or_insert(0) += 1; } }
        m
    }
}

/// Entry index `coord - base` if it lies in `0..count`. The game's lookup (level01 0x2117d0)
/// loads `base` with `lhu` (unsigned), subtracts in 32 bits and misses on `i < 0 || count - i <= 0`;
/// the one unsigned compare here is equivalent. (Every retail base is non-negative, so the
/// signed [`CollisionNodeHeader::base`] and the game's unsigned read agree on the disc.)
fn entry_index(h: CollisionNodeHeader, coord: i32) -> Option<usize> {
    let i = coord.wrapping_sub(h.base as u16 as i32) as u32;
    (i < h.count as u32).then_some(i as usize)
}

/// Walks the raw tree in `mesh` (the bytes from the root node on) for cell coordinates
/// `(x, y, z)` and returns the non-zero leaf word, or `None` when the cell is outside the grid or
/// empty, like the game's cell lookup (level01 0x2117d0). The leaf is at `mesh[word >> 8..]` and
/// is `word & 0xff` quadwords long (the game DMAs exactly that many to scratchpad).
pub fn lookup_cell_word(mesh: &[u8], x: i32, y: i32, z: i32) -> Result<Option<u32>> {
    let m = Buf(mesh);
    let root: CollisionNodeHeader = m.pod(0, "collision root")?;
    let Some(zi) = entry_index(root, z) else { return Ok(None) };
    let slab = m.u16(4 + 2 * zi)? as usize * 4;
    if slab == 0 { return Ok(None); }
    let sh: CollisionNodeHeader = m.pod(slab, "collision slab")?;
    let Some(yi) = entry_index(sh, y) else { return Ok(None) };
    let row = m.u32(slab + 4 + 4 * yi)? as usize;
    if row == 0 { return Ok(None); }
    let rh: CollisionNodeHeader = m.pod(row, "collision row")?;
    let Some(xi) = entry_index(rh, x) else { return Ok(None) };
    let word = m.u32(row + 4 + 4 * xi)?;
    Ok((word != 0).then_some(word))
}

/// Extent of the queryable world on each axis: the game rejects any query not inside
/// `[0, 1024)^3` before touching the grid (docs/plan/collision_queries.md section 2).
pub const WORLD_SIZE: f32 = 1024.0;

/// Cell coordinates containing world position `p`, or `None` outside `[0, 1024)^3`. The game
/// computes `trunc(p * 1024) >> 12` for line queries and `trunc(p) >> 2` for sphere/capsule
/// bounds; for `p` in range both equal `floor(p / 4)`, computed here exactly as the line form.
pub fn cell_coords(p: [f32; 3]) -> Option<[i32; 3]> {
    if p.iter().any(|&v| !(0.0..WORLD_SIZE).contains(&v)) { return None; }
    Some(p.map(|v| ((v * 1024.0) as i32) >> 12))
}

fn node<'a>(m: Buf<'a>, at: usize, entry_size: usize, what: &'static str) -> Result<(CollisionNodeHeader, Buf<'a>)> {
    let h: CollisionNodeHeader = m.pod(at, what)?;
    if h.count > 4096 { return invalid(format!("collision: implausible {what} count {}", h.count)); }
    let entries = m.sub(at + 4, h.count as usize * entry_size, what)?;
    Ok((h, entries))
}

fn parse_leaf(m: Buf, x: i16, y: i16, z: i16, word: u32) -> Result<CollisionCell> {
    let leaf = (word >> 8) as usize;
    let header: CollisionLeafHeader = m.pod(leaf, "collision leaf header")?;
    let (f, v, q) = (header.face_count as usize, header.vertex_count as usize, header.quad_count as usize);
    if q > f { return invalid("collision: quad count exceeds face count"); }
    if header.size_qwords() != (word & 0xff) as usize { return invalid("collision: leaf size byte does not match its contents"); }
    m.check(leaf, header.size_qwords() * 16, "collision leaf")?;
    let mut cell = CollisionCell { x, y, z, leaf_word: word, header, ..Default::default() };
    cell.packed = m.pod_slice(leaf + 4, v, "collision vertices")?;
    let centre = cell.centre();
    cell.vertices = cell.packed.iter().map(|p| p.world(centre)).collect();
    cell.faces = m.pod_slice(leaf + 4 + 4 * v, f, "collision faces")?;
    cell.quad_v3 = m.sub(leaf + 4 + 4 * v + 4 * f, q, "collision quad indices")?.bytes().to_vec();
    for i in 0..f {
        let n = if i < q { 4 } else { 3 };
        if cell.face_indices(i)[..n].iter().any(|&ix| ix as usize >= v) { return invalid("collision: face index out of range"); }
    }
    Ok(cell)
}

fn parse_hero_groups(hb: Buf, c: &mut Collision) -> Result<()> {
    c.hero_group_count = hb.i32(0)?;
    if !(0..=100_000).contains(&c.hero_group_count) { return invalid("collision: implausible hero group count"); }
    let headers: Vec<HeroGroupHeader> = hb.pod_slice(0x10, c.hero_group_count as usize, "hero group headers")?;
    for header in headers {
        let (data, nv, nt) = (header.data as usize, header.vertex_count as usize, header.triangle_count as usize);
        let vertices: Vec<HeroVertex> = hb.pod_slice(data, nv, "hero vertices")?;
        let triangles: Vec<HeroTriangle> = hb.pod_slice(data + 8 * nv, nt, "hero triangles")?;
        if vertices.iter().any(|v| v.pad != 0) { return invalid("collision: unknown hero vertex variant"); }
        for t in &triangles {
            if t.v.iter().any(|&ix| ix as usize >= nv) { return invalid("collision: hero index out of range"); }
            if t.pad != 0 { return invalid("collision: unknown hero triangle variant"); }
        }
        c.hero_groups.push(HeroGroup { header, vertices, triangles });
    }
    Ok(())
}

/// Parses a collision block (the bytes at `LevelCoreHeader::collision`).
pub fn parse_collision_block(block: &[u8]) -> Result<Collision> {
    let b = Buf(block);
    let header: CollisionHeader = b.pod(0, "collision header")?;
    if header.mesh <= 0 { return invalid("collision: no mesh pointer"); }
    if header.hero_groups != 0 && header.hero_groups < header.mesh { return invalid("collision: hero section before the mesh"); }
    // The mesh runs to the hero section (spec 2.1), or to the end of the block when there is none.
    let mesh_end = if header.hero_groups > 0 { header.hero_groups as usize } else { b.len() };
    let m = b.sub(header.mesh as usize, mesh_end.saturating_sub(header.mesh as usize), "collision mesh")?;
    let mut c = Collision { header, ..Default::default() };

    let (rh, zs) = node(m, 0, 2, "collision z")?;
    c.root = CollisionRoot { header: rh, slabs: (0..rh.count as usize).map(|i| zs.u16(2 * i)).collect::<Result<_>>()? };
    for (zi, &zoff) in c.root.slabs.iter().enumerate() {
        if zoff == 0 { continue; }
        let z = rh.base.wrapping_add(zi as i16);
        let so = zoff as usize * 4;
        let (sh, ys) = node(m, so, 4, "collision y")?;
        let rows: Vec<u32> = (0..sh.count as usize).map(|i| ys.u32(4 * i)).collect::<Result<_>>()?;
        for (yi, &ro) in rows.iter().enumerate() {
            if ro == 0 { continue; }
            let y = sh.base.wrapping_add(yi as i16);
            let (xh, xs) = node(m, ro as usize, 4, "collision x")?;
            let words: Vec<u32> = (0..xh.count as usize).map(|i| xs.u32(4 * i)).collect::<Result<_>>()?;
            for (xi, &word) in words.iter().enumerate() {
                if word == 0 { continue; }
                c.cells.push(parse_leaf(m, xh.base.wrapping_add(xi as i16), y, z, word)?);
            }
            c.rows.push(CollisionRow { offset: ro, z, y, header: xh, cells: words });
        }
        c.slabs.push(CollisionSlab { offset: so as u32, z, header: sh, rows });
    }
    if header.hero_groups > 0 { parse_hero_groups(b.tail(header.hero_groups as usize, "hero collision")?, &mut c)?; }
    Ok(c)
}

/// The level's collision block: the `"collision"` entry of `core.blocks`, sliced from the
/// decompressed core data (its size comes from the core boundary rule).
pub fn collision_block<'a>(core: &LevelCore, core_data: &'a [u8]) -> Result<&'a [u8]> {
    let Some(blk) = core.blocks.iter().find(|b| b.name == "collision") else { return invalid("level has no collision block") };
    Ok(Buf(core_data).sub(blk.offset, blk.size, "collision block")?.bytes())
}

/// Parses the level's collision from the decompressed core data.
pub fn parse_collision(core: &LevelCore, core_data: &[u8]) -> Result<Collision> {
    parse_collision_block(collision_block(core, core_data)?)
}

/// One world-space triangle of the main mesh (44 bytes; the layout of the C++ `CollisionTriangle`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CollisionTriangle {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub c: [f32; 3],
    /// Index into [`Collision::cells`].
    pub cell: u32,
    /// Index into [`CollisionCell::faces`].
    pub face: u16,
    /// The face's surface id.
    pub surface: u8,
    /// 0 = triangle face, 1 = first half of a quad `(v0, v1, v2)`, 2 = second half `(v0, v2, v3)`.
    pub part: u8,
}
const _: () = assert!(std::mem::size_of::<CollisionTriangle>() == 44);

/// Every face of every cell as world-space triangles, in cell then face order. Quads split along
/// the v0-v2 diagonal into `(v0, v1, v2)` and `(v0, v2, v3)`, as the C++ `collision_triangles`.
/// Faces duplicated across cells appear once per cell.
pub fn collision_triangles(c: &Collision) -> Vec<CollisionTriangle> {
    let mut out = Vec::new();
    for (ci, cell) in c.cells.iter().enumerate() {
        for fi in 0..cell.faces.len() {
            let v = cell.face_indices(fi);
            let quad = cell.is_quad(fi);
            for t in 0..if quad { 2 } else { 1 } {
                let p = |k: usize| cell.vertices[v[k] as usize];
                out.push(CollisionTriangle {
                    a: p(0), b: p(t + 1), c: p(t + 2),
                    cell: ci as u32, face: fi as u16, surface: cell.faces[fi].surface,
                    part: if quad { t as u8 + 1 } else { 0 },
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::{bytes_of, cast_slice};

    #[test]
    fn packed_vertex_decode_is_signed_fixed_point() {
        // Extremes of each field and a mixed word.
        let cases = [
            (0x0000_0000u32, [0, 0, 0]),
            (0x0000_01ff, [511, 0, 0]),
            (0x0000_0200, [-512, 0, 0]),
            (0x0000_03ff, [-1, 0, 0]),
            (0x0007_fc00, [0, 511, 0]),
            (0x0008_0000, [0, -512, 0]),
            (0x7ff0_0000, [0, 0, 2047]),
            (0x8000_0000, [0, 0, -2048]),
            (0xfff0_0000, [0, 0, -1]),
            (0xffff_ffff, [-1, -1, -1]),
        ];
        for (w, f) in cases {
            let p = PackedCollisionVertex(w);
            assert_eq!(p.fields(), f, "{w:#x}");
            assert_eq!(PackedCollisionVertex::pack(f[0], f[1], f[2]), p, "{w:#x}");
        }
        // x = -3 (1/16), y = 17 (1/16), z = -129 (1/64) around the centre of cell (1, -2, 3).
        let p = PackedCollisionVertex::pack(-3, 17, -129);
        assert_eq!(p.offset(), [-0.1875, 1.0625, -2.015625]);
        let cell = CollisionCell { x: 1, y: -2, z: 3, ..Default::default() };
        assert_eq!(cell.centre(), [6.0, -6.0, 14.0]);
        assert_eq!(p.world(cell.centre()), [5.8125, -4.9375, 11.984375]);
        assert_eq!(cell.bounds(), ([4.0, -8.0, 12.0], [8.0, -4.0, 16.0]));
        assert_eq!(cell_coords([5.8125, 4.9375, 11.984375]), Some([1, 1, 2]));
        assert_eq!(cell_coords([0.0, 3.999, 1023.99]), Some([0, 0, 255]));
        assert_eq!(cell_coords([5.8125, -4.9375, 11.984375]), None);
        assert_eq!(cell_coords([1024.0, 0.0, 0.0]), None);
        let f = |s| CollisionFace { v: [0; 3], surface: s };
        assert_eq!((f(31).surface_id(), f(31).sound_class()), (-1, 0));
        assert_eq!((f(0x4c).surface_id(), f(0x4c).sound_class(), f(0x4c).high_bit()), (12, 2, false));
        assert_eq!((f(0x9f).surface_id(), f(0x9f).sound_class(), f(0x9f).high_bit()), (-1, 0, true));
        assert_eq!(f(0x7f).sound_class(), 0);
    }

    /// Mesh with root z_base 5 (2 slabs: z=5 empty, z=6), slab y_base 3 (rows y=3 empty, y=4),
    /// row x_base 10 (x=10 one quad + one triangle, x=11 empty, x=12 one triangle), and a hero
    /// section with one group.
    fn synthetic() -> Vec<u8> {
        let mut b = vec![0u8; 0x200];
        let put = |b: &mut Vec<u8>, at: usize, bytes: &[u8]| b[at..at + bytes.len()].copy_from_slice(bytes);
        let mesh = 0x40;
        put(&mut b, 0, bytes_of(&CollisionHeader { mesh, hero_groups: 0x180 }));
        let m = mesh as usize;
        // root at mesh+0: base 5, count 2, entries [0, 0x10/4]
        put(&mut b, m, bytes_of(&CollisionNodeHeader { base: 5, count: 2 }));
        put(&mut b, m + 4, cast_slice(&[0u16, 0x10 / 4]));
        // slab at mesh+0x10: base 3, count 2, rows [0, 0x20]
        put(&mut b, m + 0x10, bytes_of(&CollisionNodeHeader { base: 3, count: 2 }));
        put(&mut b, m + 0x14, cast_slice(&[0u32, 0x20]));
        // row at mesh+0x20: base 10, count 3, leaves at 0x40 (2 qw) and 0x80 (2 qw)
        put(&mut b, m + 0x20, bytes_of(&CollisionNodeHeader { base: 10, count: 3 }));
        put(&mut b, m + 0x24, cast_slice(&[(0x40u32 << 8) | 2, 0, (0x80 << 8) | 2]));
        // leaf 0: 2 faces (1 quad), 4 vertices: 4 + 16 + 8 + 1 = 29 bytes -> 2 qw
        let l0 = m + 0x40;
        put(&mut b, l0, bytes_of(&CollisionLeafHeader { face_count: 2, vertex_count: 4, quad_count: 1 }));
        let v = [(-16, -16, 0), (16, -16, 0), (16, 16, 0), (-16, 16, 64)].map(|(x, y, z)| PackedCollisionVertex::pack(x, y, z));
        put(&mut b, l0 + 4, cast_slice(&v));
        put(&mut b, l0 + 20, cast_slice(&[CollisionFace { v: [0, 1, 2], surface: 31 }, CollisionFace { v: [3, 2, 0], surface: 8 }]));
        b[l0 + 28] = 3;
        // leaf 1: 1 triangle, 3 vertices: 4 + 12 + 4 = 20 bytes -> 2 qw
        let l1 = m + 0x80;
        put(&mut b, l1, bytes_of(&CollisionLeafHeader { face_count: 1, vertex_count: 3, quad_count: 0 }));
        put(&mut b, l1 + 4, cast_slice(&v[..3]));
        put(&mut b, l1 + 16, bytes_of(&CollisionFace { v: [2, 1, 0], surface: 95 }));
        // hero section at 0x180: 1 group, data at 0x20: 3 vertices then 1 triangle
        put(&mut b, 0x180, &1i32.to_le_bytes());
        put(&mut b, 0x190, bytes_of(&HeroGroupHeader { sphere: [64, 128, 192, 32], triangle_count: 1, vertex_count: 3, data: 0x20 }));
        put(&mut b, 0x1a0, cast_slice(&[HeroVertex { xyz: [64, 64, 64], pad: 0 }, HeroVertex { xyz: [128, 64, 64], pad: 0 }, HeroVertex { xyz: [64, 128, 64], pad: 0 }]));
        put(&mut b, 0x1b8, bytes_of(&HeroTriangle { v: [0, 1, 2], pad: 0 }));
        b
    }

    #[test]
    fn cell_walk_and_triangles() {
        let b = synthetic();
        let c = parse_collision_block(&b).unwrap();
        assert_eq!(c.root.slabs, vec![0, 4]);
        assert_eq!(c.slabs.len(), 1);
        assert_eq!((c.slabs[0].offset, c.slabs[0].z), (0x10, 6));
        assert_eq!(c.rows.len(), 1);
        assert_eq!((c.rows[0].offset, c.rows[0].z, c.rows[0].y), (0x20, 6, 4));
        assert_eq!(c.cells.iter().map(|c| c.coords()).collect::<Vec<_>>(), vec![[10, 4, 6], [12, 4, 6]]);
        let c0 = &c.cells[0];
        assert_eq!((c0.leaf_offset(), c0.leaf_qwords()), (0x40, 2));
        assert_eq!(c0.face_indices(0), [0, 1, 2, 3]);
        assert_eq!(c0.face_indices(1), [3, 2, 0, 3]);
        assert_eq!(c0.vertices[3], [41.0, 19.0, 27.0]);
        // Tree walk on the raw bytes agrees with the parsed cells; misses outside and on empties.
        let mesh = &b[0x40..0x180];
        assert_eq!(lookup_cell_word(mesh, 10, 4, 6).unwrap(), Some((0x40 << 8) | 2));
        assert_eq!(lookup_cell_word(mesh, 12, 4, 6).unwrap(), Some((0x80 << 8) | 2));
        for (x, y, z) in [(11, 4, 6), (13, 4, 6), (9, 4, 6), (10, 3, 6), (10, 5, 6), (10, 4, 5), (10, 4, 7), (10, 4, 4), (10, -4, 6)] {
            assert_eq!(lookup_cell_word(mesh, x, y, z).unwrap(), None, "({x},{y},{z})");
            assert_eq!(c.cell_at(x as i16, y as i16, z as i16), None);
        }
        assert_eq!(c.cell_at(12, 4, 6), Some(1));
        // Quad splits along v0-v2.
        let t = collision_triangles(&c);
        assert_eq!(t.len(), 4);
        assert_eq!((t[0].part, t[1].part, t[2].part, t[3].part), (1, 2, 0, 0));
        assert_eq!((t[0].a, t[0].b, t[0].c), (c0.vertices[0], c0.vertices[1], c0.vertices[2]));
        assert_eq!((t[1].a, t[1].b, t[1].c), (c0.vertices[0], c0.vertices[2], c0.vertices[3]));
        assert_eq!((t[3].cell, t[3].face, t[3].surface), (1, 0, 95));
        assert_eq!(c.surface_counts().into_iter().collect::<Vec<_>>(), vec![(8, 1), (31, 1), (95, 1)]);
        // Hero group.
        assert_eq!(c.hero_group_count, 1);
        assert_eq!(c.hero_groups[0].header.sphere_world(), [1.0, 2.0, 3.0, 0.5]);
        assert_eq!(c.hero_groups[0].vertices[1].position(), [2.0, 1.0, 1.0]);
    }

    #[test]
    fn rejects_corrupt_leaves() {
        let good = synthetic();
        // Wrong size byte.
        let mut b = good.clone();
        b[0x40 + 0x24] = 3;
        assert!(parse_collision_block(&b).is_err());
        // Face index past the vertex count (leaf 1 has 3 vertices).
        let mut b = good.clone();
        b[0x40 + 0x80 + 16] = 3;
        assert!(parse_collision_block(&b).is_err());
        // quad_count > face_count.
        let mut b = good.clone();
        b[0x40 + 0x80 + 3] = 2;
        assert!(parse_collision_block(&b).is_err());
        // Non-zero hero pad.
        let mut b = good;
        b[0x1bb] = 1;
        assert!(parse_collision_block(&b).is_err());
    }
}
