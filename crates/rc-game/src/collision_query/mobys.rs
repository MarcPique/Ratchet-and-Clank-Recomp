//! The moby pass of the collision kernels (level01): `CollLine_Fix` 0x211fd0..0x21295c, sphere
//! 0x212fb8..0x21350c, capsule 0x213df8..0x2143c4, and `coll_sphere_mobys` 0x214468; the moby grid
//! `UpdateMobyGrids` 0x265900 (from `MobyBuildMatrix` 0x265bd8 and `DeleteMoby` 0x2636c0). Spec:
//! `docs/plan/collision_queries.md` ("Moby collision in the port"). Same arithmetic model as the world
//! kernels ([`crate::ps2v`], game operation order).
//!
//! **Grid** ([`MobyGrid`], 0x19bc60): 64×64 cells of 16 units, x/y only; cell `(x, y)` at `+x·4 + y·0x100`
//! holds `{u16 block, s8 count, u8 capacity}`, the u16 moby indices (`moby+0xac`) at `0x19fc60 + block·32`.
//! A moby is in the cells of the rectangle `+0xa0..+0xa3` = `{x0, y0, x1, y1}` bytes, set by
//! `MobyBuildMatrix` (every matrix rebuild of a moby with `+0x94 ≠ 0` and state < 0x80) from its bounding
//! sphere: `(vftoi0(c) ∓ vftoi0(r)) >> 14` per axis — unless the new rectangle equals the old one or its x0
//! or y0 byte has bit 6 or 7 set (off the grid: the old registration stays). `UpdateMobyGrids` removes the
//! moby from the old cells not in the new rectangle (swap with the cell's last entry) and appends it to the
//! new cells not in the old one, x inner, y outer; `DeleteMoby` passes `0x80807f7f` (empty), which is also
//! the value `InitMobyInstance` leaves. The list storage (capacity doubling / halving, block allocator
//! 0x265820 / 0x265880) does not change the order and is not modelled.
//!
//! **Candidates.** Line: the (x>>2, y>>2) of the DDA cell list (up to the first entry with t = 1.0,
//! consecutive duplicates dropped; all of it, even past the world walk's early out). Sphere / capsule:
//! `vftoi0(c·1024 ∓ r·1024) >> 14`, y outer, x inner; `coll_sphere_mobys`: `vftoi0(c ∓ r) >> 4`. Each
//! cell's list in order. A moby is skipped when it is the `ignore` argument, has no blob (`+0x94`),
//! was already tested by this query (stamp `+0x9c`), or fails the bounding-sphere test:
//! * line: `t = clamp((bs − a)·(d/|d|²), 0, best)`, reject when `((|a + d·t − bs|²ₓ − R²) + y) + z > +0`;
//!   with query flag 0x1 or 0x8, also every moby without `mode & 0x4000`;
//! * sphere: `((dx² − (r+R)²) + dy²) + dz² > +0`;
//! * capsule (a cylinder test): the same in x/y only, and `bs.z + (r+R) − c.z > 0`,
//!   `bs.z − (r+R) − c.z − h < 0`.
//!
//! **Per moby.** With a non-zero joint count (blob +0 with flag 0x4, else +2) the pose is fetched through
//! the 8-entry cache ([`PoseCache`], key `moby+0xa8`) — except that with flag 0x2 a moby without a mesh is
//! skipped first. Then the mesh: vertices `((r0·s)·x + (r1·s)·y) + (r2·s)·z` relative to `pos·1024`
//! (rows `+0xc0/d0/e0`, scale `+0x2c`), outcodes as float sign tests against the query box (line: the
//! whole segment's min/max; sphere: `c ± r`; capsule: `c ± r`, top + h) with no margin, and the world
//! kernel's triangle test on every face (no quads), sharing the running best. Then, unless flag 0x2, the
//! primitives (see `rc_formats::moby_collision`), each tested as a sphere of radius `R·s` about the
//! primitive's point nearest the query:
//! * sphere / capsule: accept when the closest point of that sphere (`C + (c − C)/|c − C|·min(|c − C|, R)`)
//!   is within `best` of the centre (capsule: of the axis point at that height), then `best = d²`
//!   (**not** shrunk, unlike a triangle); `+0x40` = that offset, `+0x50..0x70` left as they were;
//! * line: the entry root `t = (2w·d − √((2w·d)² − 4|d|²(|w|² − R²)))/(2|d|²)` clamped to [0, 1], accepted
//!   when `best − t > +0`; `+0x40 = hit − C`. Kind 3 first intersects the infinite cylinder in x/y
//!   (|d.xy|² clamped to ≥ 2⁻⁶), kind 4 the infinite capsule axis, to pick the axis point.
//!
//! **Output.** `+0x18` = the moby of the last accepted hit (world hits: 0), `+0x1c = 0x1000 | type` for a
//! moby triangle, `-(record address)` for a primitive (here `-(0x10 + 0x20·i)` and
//! [`CollOutput::primitive`](super::CollOutput::primitive)).

use super::{capsule_tri, face_excluded, line_tri, record, sphere_tri, Best, Vert, Volume, BEST_SHRINK};
use crate::moby_runtime::{Moby, MobyTable};
use crate::ps2v::*;
use rc_formats::moby_anim::{self, AnimState, MobyAnimClass, MobyFrame};
use rc_formats::moby_collision::{MobyCollPrim, MobyCollision};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Cells per grid side (16 units each).
pub const GRID_DIM: u32 = 64;
/// `+0xa0` of a moby in no cell (`InitMobyInstance`, `DeleteMoby`): x0 = y0 = 0x7f, x1 = y1 = 0x80.
pub const GRID_NONE: u32 = 0x8080_7f7f;
/// Joint slots of one pose-cache entry (0x800 bytes).
pub const POSE_SLOTS: usize = 0x80;

// ---------------------------------------------------------------------------------------------------
// Moby records

/// The fields of a runtime moby the moby pass reads, as raw float bits.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CollMoby {
    /// +0x00: bounding sphere, x1024 world (`MobyBuildMatrix`).
    pub bsphere: [F; 4],
    /// +0x10: position (world units).
    pub position: V3,
    /// +0xc0 / +0xd0 / +0xe0: rotation rows (xyz).
    pub rows: [V3; 3],
    /// +0x2c: scale.
    pub scale: F,
    /// +0x34: mode bits (0x4000: tested by line queries with flag 0x1 / 0x8).
    pub mode: u16,
    /// +0x98: per-primitive disable bits (bit i: primitive i).
    pub disable: u32,
    /// +0xa8: `index << 16 | rebuild count`, the pose-cache key.
    pub key: u32,
    pub o_class: i16,
    /// +0x94 ≠ 0: the class has a collision blob.
    pub collision: bool,
}

impl CollMoby {
    pub fn of(m: &Moby) -> CollMoby {
        let r3 = |r: [f32; 4]| [r[0].to_bits(), r[1].to_bits(), r[2].to_bits()];
        CollMoby {
            bsphere: m.bsphere.map(f32::to_bits),
            position: r3(m.position),
            rows: [r3(m.rows[0]), r3(m.rows[1]), r3(m.rows[2])],
            scale: m.scale.to_bits(),
            mode: m.mode,
            disable: m.coll_disable,
            key: m.uid_hi,
            o_class: m.o_class,
            collision: m.has_collision,
        }
    }
}

/// Where the moby pass reads mobys from (the live table in the moby loop, a snapshot for the hero).
pub trait MobySource {
    /// Moby `id` (the grid's u16 index), `None` when there is no such slot.
    fn moby(&self, id: usize) -> Option<CollMoby>;
    /// Positions of joints `0..count` of the moby's current pose (translation rows of the posed joint
    /// matrices, model units, lanes xyzw): what `0x267fc0` leaves at 0x70000000 + j·16.
    fn joints(&self, id: usize, count: usize) -> Vec<[F; 4]>;
}

/// `0x267fc0`'s joint positions from the animation state: the translation rows `P_j.r3` of joints
/// `0..count` through `rc_formats::moby_anim::joint_translations` (the partial evaluator `fun_002109b8`,
/// all joints below `count` marked). **Not instruction-compared** with 0x267fc0 (a third evaluator variant
/// with the same inputs); only joint primitives (kinds 2 / 4) read it. Zeros without class data.
pub fn pose_joints(class: Option<&MobyAnimClass>, anim: &AnimState, snap: Option<&MobyFrame>, count: usize) -> Vec<[F; 4]> {
    let Some(class) = class.filter(|_| count > 0) else { return vec![[0; 4]; count] };
    let chains: Vec<Vec<u8>> = (0..count.min(256)).map(|j| (0..=j as u8).collect()).collect();
    let refs: Vec<&[u8]> = chains.iter().map(Vec::as_slice).collect();
    moby_anim::joint_translations(class, anim, snap, &refs)
}

/// A moby table as a [`MobySource`], with the class animation data and the snapshot frames for the poses.
pub struct TableMobys<'a> {
    pub table: &'a MobyTable,
    pub classes: &'a dyn crate::moby_update::ClassData,
    pub snapshots: &'a [Option<MobyFrame>],
}

impl MobySource for TableMobys<'_> {
    fn moby(&self, id: usize) -> Option<CollMoby> { self.table.mobys.get(id).map(CollMoby::of) }
    fn joints(&self, id: usize, count: usize) -> Vec<[F; 4]> {
        let Some(m) = self.table.mobys.get(id) else { return vec![[0; 4]; count] };
        let snap = self.snapshots.get(id).and_then(Option::as_ref);
        pose_joints(self.classes.anim(m.o_class), &m.anim, snap, count)
    }
}

// ---------------------------------------------------------------------------------------------------
// Pose cache (CollOutput +4 / +8 / +0xc)

/// The 8-entry joint-position cache the kernels keep for posed classes: keys at 0x174340 (8 × {key, word}),
/// buffers at 0x174380 (8 × 0x800), round-robin index `CollOutput+0xc`. A query looks the moby's `+0xa8`
/// up in entry order; on a miss the next entry is overwritten with the fresh pose. So a moby whose matrix is
/// not rebuilt (mode 4) keeps being tested with the pose cached for its key.
#[derive(Clone, Debug)]
pub struct PoseCache {
    pub keys: [u32; 8],
    pub slots: Vec<Vec<[F; 4]>>,
    pub next: usize,
}

impl Default for PoseCache {
    fn default() -> Self { PoseCache { keys: [0; 8], slots: vec![vec![[0; 4]; POSE_SLOTS]; 8], next: 0 } }
}

impl PoseCache {
    /// The joint positions for `key` (cached, or `fill(count)` stored into the next entry).
    pub fn get(&mut self, key: u32, count: usize, fill: impl FnOnce(usize) -> Vec<[F; 4]>) -> Vec<[F; 4]> {
        if let Some(i) = self.keys.iter().position(|&k| k == key) { return self.slots[i].clone(); }
        let i = self.next;
        self.next = (i + 1) & 7;
        self.keys[i] = key;
        let j = fill(count);
        for (d, s) in self.slots[i].iter_mut().zip(&j) { *d = *s; }
        let mut out = self.slots[i].clone();
        // After a miss the kernel reads the scratchpad copy: exactly `count` fresh joints.
        if j.len() > out.len() { out = j; }
        out
    }
}

// ---------------------------------------------------------------------------------------------------
// Grid (0x19bc60)

/// The moby grid: per cell, the moby indices in list order.
#[derive(Clone, Debug, PartialEq)]
pub struct MobyGrid {
    pub cells: Vec<Vec<u16>>,
}

impl Default for MobyGrid {
    fn default() -> Self { MobyGrid { cells: vec![Vec::new(); (GRID_DIM * GRID_DIM) as usize] } }
}

/// `psubsb` of the cell `(x, y, x, y)` bytes and the rectangle: inside when `x ≥ x0, y ≥ y0, x ≤ x1, y ≤ y1`
/// as signed bytes.
fn rect_contains(rect: u32, x: u8, y: u8) -> bool {
    let b = rect.to_le_bytes().map(|v| v as i8);
    let (x, y) = (x as i8, y as i8);
    !(x.saturating_sub(b[0]) < 0 || y.saturating_sub(b[1]) < 0 || x.saturating_sub(b[2]) > 0 || y.saturating_sub(b[3]) > 0)
}

impl MobyGrid {
    pub fn new() -> MobyGrid { MobyGrid::default() }

    /// The moby list of cell `(x, y)` (x at `+x·4`, y at `+y·0x100`; out of the 64×64 table: empty — the game
    /// would read past it).
    pub fn cell(&self, x: u32, y: u32) -> &[u16] {
        let i = y.wrapping_mul(GRID_DIM).wrapping_add(x) as usize;
        self.cells.get(i).map_or(&[], Vec::as_slice)
    }

    fn cell_mut(&mut self, x: u8, y: u8) -> Option<&mut Vec<u16>> { self.cells.get_mut(y as usize * GRID_DIM as usize + x as usize) }

    /// The rectangle word `MobyBuildMatrix` computes from a bounding sphere (x1024 world, radius w):
    /// bytes `{x0, y0, x1, y1}` = low bytes of `(vftoi0(c) ∓ vftoi0(r)) >> 14` (arithmetic).
    pub fn rect_for(bsphere: [F; 4]) -> u32 {
        let [x, y, _, r] = [ftoi0(bsphere[0]), ftoi0(bsphere[1]), ftoi0(bsphere[2]), ftoi0(bsphere[3])];
        let c = |v: i32| (v >> 14) as u32 & 0xff;
        c(x.wrapping_sub(r)) | c(y.wrapping_sub(r)) << 8 | c(x.wrapping_add(r)) << 16 | c(y.wrapping_add(r)) << 24
    }

    /// `UpdateMobyGrids(moby, new)` 0x265900 for moby index `index` whose `+0xa0` word is `old`: leaves the
    /// old cells outside `new` (the entry is replaced by the cell's last one), joins the new cells outside
    /// `old` (appended), both x inner, y outer. Negative words (byte 3 ≥ 0x80) are "no cells".
    pub fn update(&mut self, index: u16, old: u32, new: u32) {
        let each = |rect: u32, f: &mut dyn FnMut(u8, u8)| {
            let [x0, y0, x1, y1] = rect.to_le_bytes();
            let mut y = y0;
            loop {
                let mut x = x0;
                loop {
                    f(x, y);
                    if x == x1 { break; }
                    x = x.wrapping_add(1);
                }
                if y == y1 { break; }
                y = y.wrapping_add(1);
            }
        };
        if (old as i32) >= 0 {
            each(old, &mut |x, y| {
                if rect_contains(new, x, y) { return; }
                if let Some(c) = self.cell_mut(x, y) {
                    // (The game traps when the index is missing.)
                    if let Some(p) = c.iter().position(|&e| e == index) { c.swap_remove(p); }
                }
            });
        }
        if (new as i32) < 0 { return; }
        each(new, &mut |x, y| {
            if rect_contains(old, x, y) { return; }
            if let Some(c) = self.cell_mut(x, y) { c.push(index); }
        });
    }

    /// The grid part of `MobyBuildMatrix` (0x265c74..0x265cd4) for a moby whose bounding sphere was just
    /// rebuilt: only with a collision blob (`+0x94`) and state < 0x80; the new rectangle replaces `+0xa0`
    /// unless it is equal or off the grid (`x0 | y0 << 8` & 0xc0c0).
    pub fn register(&mut self, m: &mut Moby) {
        if !m.has_collision || m.state >= 0x80 { return; }
        let new = MobyGrid::rect_for(m.bsphere.map(f32::to_bits));
        let old = u32::from_le_bytes(m.ba0);
        if new == old || new & 0xc0c0 != 0 { return; }
        self.update(m.index as u16, old, new);
        m.ba0 = new.to_le_bytes();
    }

    /// `DeleteMoby`'s `UpdateMobyGrids(moby, 0x80807f7f)`.
    pub fn remove(&mut self, m: &mut Moby) {
        let old = u32::from_le_bytes(m.ba0);
        self.update(m.index as u16, old, GRID_NONE);
        m.ba0 = GRID_NONE.to_le_bytes();
    }

    /// The level loader's registrations: `MobyBuildMatrix` on every instance in array order (the table as
    /// `load_static_mobys` leaves it; each moby's `+0xa0` is updated).
    pub fn build(table: &mut MobyTable) -> MobyGrid {
        let mut g = MobyGrid::new();
        for m in &mut table.mobys {
            if m.state == crate::moby_runtime::state::END { break; }
            g.register(m);
        }
        g
    }

    /// The cells holding `index` (tests, statistics).
    pub fn cells_of(&self, index: u16) -> Vec<(u32, u32)> {
        (0..self.cells.len()).filter(|&i| self.cells[i].contains(&index)).map(|i| (i as u32 % GRID_DIM, i as u32 / GRID_DIM)).collect()
    }
}

// ---------------------------------------------------------------------------------------------------
// Scene

/// Everything the moby pass reads: the mobys, the grid, the class blobs by `o_class` and the pose cache.
pub struct MobyScene<'a> {
    pub mobys: &'a dyn MobySource,
    pub grid: &'a MobyGrid,
    pub classes: &'a HashMap<i16, MobyCollision>,
    pub cache: &'a Mutex<PoseCache>,
}

impl MobyScene<'_> {
    fn joints(&self, id: usize, key: u32, count: usize) -> Vec<[F; 4]> {
        let mut c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        c.get(key, count, |n| self.mobys.joints(id, n))
    }
}

/// An owned [`MobyScene`] (`moby_update::Services::hero_scene`, `crate::tick::MobySystem::scene`).
pub struct OwnedScene {
    pub mobys: Box<dyn MobySource>,
    pub grid: Arc<MobyGrid>,
    pub classes: Arc<HashMap<i16, MobyCollision>>,
    pub cache: Arc<Mutex<PoseCache>>,
    /// A moby its users may pass as `ignore` (informational; `crate::tick` passes Ratchet's moby `0x1413d0`
    /// through `hero::Env::hero_moby` / `CamInput::hero_moby`).
    pub ignore: Option<usize>,
}

impl OwnedScene {
    pub fn scene(&self) -> MobyScene<'_> { MobyScene { mobys: &*self.mobys, grid: &self.grid, classes: &self.classes, cache: &self.cache } }
}


// ---------------------------------------------------------------------------------------------------
// Shared per-moby machinery

/// `vmulax ACC, r0, v; vmadday ACC, r1, v; vmaddz d, r2, v`.
fn rot(r: &[V3; 3], v: V3) -> V3 { std::array::from_fn(|l| add(add(mul(r[0][l], v[0]), mul(r[1][l], v[1])), mul(r[2][l], v[2]))) }

/// Mesh vertices relative to `pos·1024` and their outcodes against `[lo, hi]` (float sign tests).
fn mesh_verts(m: &CollMoby, blob: &MobyCollision, lo: V3, hi: V3) -> Vec<Vert> {
    let rs = m.rows.map(|r| vmuls(r, m.scale));
    blob.vertices
        .iter()
        .map(|v| {
            let p = rot(&rs, [itof0(v[0] as i32), itof0(v[1] as i32), itof0(v[2] as i32)]);
            let mut code = 0u32;
            for k in 0..3 {
                let below = neg(sub(p[k], lo[k]));
                let above = neg(sub(hi[k], p[k]));
                code |= (below as u32 | (above as u32) << 1) << (8 * k);
            }
            Vert { p, code }
        })
        .collect()
}

/// The faces as `(vertices, type)`, skipping outcode-culled and flag-excluded ones.
fn mesh_faces<'v>(blob: &'v MobyCollision, verts: &'v [Vert], fl: u32) -> impl Iterator<Item = ([Vert; 3], u8)> + 'v {
    blob.faces.iter().filter_map(move |f| {
        let v = [verts[f[0] as usize], verts[f[1] as usize], verts[f[2] as usize]];
        (v[0].code & v[1].code & v[2].code == 0 && !face_excluded(fl, f[3])).then_some((v, f[3]))
    })
}

/// The primitives a query tests, in order: `(index, record)` for each one not disabled by `moby+0x98` and
/// with the query's mask bit (1 with flag 0x4, else 2); the walk ends after the record with mask bit 15.
fn active_prims<'b>(blob: &'b MobyCollision, m: &CollMoby, fl: u32) -> impl Iterator<Item = (usize, &'b MobyCollPrim)> + 'b {
    let sel: i32 = if fl & 0x4 == 0 { 2 } else { 1 };
    let disable = m.disable;
    let end = blob.prims.iter().position(|p| p.mask() < 0).map_or(blob.prims.len(), |i| i + 1);
    blob.prims[..end].iter().enumerate().filter(move |(i, p)| {
        let off = if *i < 32 { (disable >> i) & 1 } else { 0 };
        off == 0 && sel & p.mask() as i32 != 0
    })
}

/// The joint count the query poses (+0 with flag 0x4, else +2).
fn joint_count(blob: &MobyCollision, fl: u32) -> usize { blob.joint_counts[if fl & 0x4 != 0 { 0 } else { 1 }] as usize }

fn joint(joints: &[[F; 4]], j: i32) -> [F; 4] { usize::try_from(j).ok().and_then(|j| joints.get(j)).copied().unwrap_or([0; 4]) }

/// The sphere / capsule kernels' primitive point (0x213358..0x21346c; capsule 0x2141d0..0x21430c) nearest the
/// query point `c` (capsule: its base) and radius, x1024 world.
fn vol_prim(p: &MobyCollPrim, m: &CollMoby, pos: V3, joints: &[[F; 4]], c: V3, capsule: bool) -> (V3, F) {
    let s = m.scale;
    let place = |v: V3| vadd(rot(&m.rows, v), pos); // ... vmaddaz ACC; vmaddx d, pos, vf21 (1.0)
    match p.kind() {
        1 => {
            let q = p.q1();
            (place([mul(q[0], s), mul(q[1], s), mul(q[2], s)]), mul(q[3], s))
        }
        2 => {
            let j = joint(joints, p.word4());
            let o = p.q1();
            let v = [add(j[0], o[0]), add(j[1], o[1]), add(j[2], o[2])];
            (place(vmuls(v, s)), mul(p.q0()[3], s))
        }
        3 => {
            let q = p.q1();
            let mut cc = place([mul(q[0], s), mul(q[1], s), mul(q[2], s)]);
            let top = add(cc[2], mul(p.q0()[1], s));
            cc[2] = min(top, max(cc[2], c[2]));
            (cc, mul(q[3], s))
        }
        _ => {
            let [j0, j1] = p.joints();
            let (a, b) = (joint(joints, j0 as i32), joint(joints, j1 as i32));
            let r = mul(p.q0()[3], s);
            let a = place(vmuls([a[0], a[1], a[2]], s));
            let b = place(vmuls([b[0], b[1], b[2]], s));
            let e = vsub(b, a);
            let w = vsub(c, a);
            if !capsule {
                let t = min(max(add(0, div(dot_x(w, e), dot_x(e, e))), 0), ONE);
                return (vadd(a, vmuls(e, t)), r);
            }
            // Capsule: 2-D closest point, or the base z clamped to the segment when |e.xy|² <= 1.
            let wd = add(mul(w[0], e[0]), mul(w[1], e[1]));
            let ee = add(mul(e[0], e[0]), mul(e[1], e[1]));
            let q = div(wd, ee);
            if !neg(sub(ONE, ee & !SIGN)) {
                let b = vadd(e, a);
                (vec_z(a, min(b[2], max(a[2], c[2]))), r)
            } else {
                let t = min(max(add(0, q), 0), ONE);
                (vmadds(a, e, t), r)
            }
        }
    }
}

fn vec_z(v: V3, z: F) -> V3 { [v[0], v[1], z] }

/// The sphere / capsule kernels' primitive test (0x213470..0x2134ec; capsule from 0x214318 with `c` the axis
/// point): the closest point of the primitive sphere `(cc, rad)` to `c`, its offset from `cc` and `d²`, when
/// `r + rad` reaches `c` and `d² ≤ best`.
fn vol_prim_test(c: V3, r: F, cc: V3, rad: F, best: F) -> Option<(V3, V3, F)> {
    let rr = add(r, rad);
    let e = vsub(cc, c);
    let rr2 = mul(rr, rr);
    let d2 = dot_x(e, e);
    let s = sub(rr2, d2);
    let dist = add(0, sqrt(d2)); // vsqrt Q; vaddq.x vf2, vf0, Q
    if neg(s) { return None; }
    let q = add(0, div(ONE, dist)); // vdiv Q, vf0w, vf2x
    let k = min(mul(dist, ONE), rad); // vminibcw.x
    let off = vmuls(vmuls(vsub(c, cc), q), k);
    let p = vadd(cc, off);
    let e = vsub(p, c);
    let d2 = dot_x(e, e);
    if neg(sub(best, d2)) { return None; }
    Some((p, off, d2))
}

fn prim_best(i: usize, point: V3, normal: V3, prev: Option<&Best>, id: usize) -> Best {
    Best { kind: -(0x10 + 0x20 * i as i32), point, normal, tri: prev.map_or([[0; 3]; 3], |b| b.tri), moby: Some(id), prim: Some(i) }
}

// ---------------------------------------------------------------------------------------------------
// Sphere / capsule (0x212fb8 / 0x213df8)

/// The moby pass of the sphere (`h = None`) and capsule kernels, after the world pass.
#[allow(clippy::too_many_arguments)]
pub(super) fn volume_pass(sc: &MobyScene, fl: u32, vol: &Volume, h: Option<F>, ignore: Option<usize>, best_d: &mut F, mut best: Option<Best>) -> Option<Best> {
    let c = vol.c1024;
    let r = vol.r1024;
    let lo = [0, 1].map(|k| (ftoi0(vol.lo1024[k]) as u32) >> 14);
    let hi = [0, 1].map(|k| (ftoi0(vol.hi1024[k]) as u32) >> 14);
    let mut stamped: Vec<usize> = Vec::new();
    for y in lo[1]..=hi[1] {
        for x in lo[0]..=hi[0] {
            for &idx in sc.grid.cell(x, y) {
                let id = idx as usize;
                if Some(id) == ignore { continue; }
                let Some(m) = sc.mobys.moby(id) else { continue };
                if stamped.contains(&id) || !m.collision { continue; }
                let bs = m.bsphere;
                let rr = add(r, bs[3]);
                match h {
                    None => {
                        let e = vsub([bs[0], bs[1], bs[2]], c);
                        let sq = vmul(e, e);
                        let v = add(add(sub(sq[0], mul(rr, rr)), mul(ONE, sq[1])), mul(ONE, sq[2]));
                        if v as i32 > 0 { continue; }
                    }
                    Some(h) => {
                        let (ex, ey) = (sub(bs[0], c[0]), sub(bs[1], c[1]));
                        let z4 = sub(sub(sub(bs[2], rr), c[2]), h);
                        let z5 = sub(add(bs[2], rr), c[2]);
                        let v = add(sub(mul(ex, ex), mul(rr, rr)), mul(ONE, mul(ey, ey)));
                        if v as i32 > 0 { continue; }
                        // blez on (z : stale y): +0 counts as "not above" here.
                        if z5 as i32 <= 0 { continue; }
                        if !neg(z4) { continue; }
                    }
                }
                stamped.push(id);
                let Some(blob) = sc.classes.get(&m.o_class) else { continue };
                let n = joint_count(blob, fl);
                let joints = if n != 0 {
                    if fl & 0x2 != 0 && blob.vertices.is_empty() { continue; }
                    sc.joints(id, m.key, n)
                } else {
                    Vec::new()
                };
                let pos = vmuls(m.position, K1024); // vf15 / vf23
                if !blob.vertices.is_empty() {
                    let c_rel = vsub(c, pos); // vf22
                    let lo = c_rel.map(|v| sub(v, r));
                    let mut hi = c_rel.map(|v| add(v, r));
                    if let Some(h) = h { hi[2] = add(hi[2], h); }
                    let verts = mesh_verts(&m, blob, lo, hi);
                    for ([v0, v1, v2], ty) in mesh_faces(blob, &verts, fl) {
                        let hit = match h {
                            None => sphere_tri(fl, c_rel, v0.p, v1.p, v2.p, *best_d),
                            Some(h) => capsule_tri(fl, c_rel, h, v0.p, v1.p, v2.p, *best_d),
                        };
                        let Some((pt, d2, nrm, e1, e2)) = hit else { continue };
                        *best_d = mul(d2, BEST_SHRINK);
                        best = Some(record(ty, pt, nrm, v0.p, e1, e2, pos, Some(id)));
                    }
                }
                if fl & 0x2 != 0 { continue; }
                for (i, p) in active_prims(blob, &m, fl) {
                    let (cc, rad) = vol_prim(p, &m, pos, &joints, c, h.is_some());
                    let from = match h {
                        None => c,
                        Some(h) => vec_z(c, add(c[2], max(min(sub(cc[2], c[2]), h), 0))),
                    };
                    let Some((pt, off, d2)) = vol_prim_test(from, r, cc, rad, *best_d) else { continue };
                    *best_d = mul(d2, ONE);
                    best = Some(prim_best(i, pt, off, best.as_ref(), id));
                }
            }
        }
    }
    best
}

/// `coll_sphere_mobys(r, centre, flags, ignore, tmpl)` (level01 0x214468): every moby the sphere touches,
/// in grid order (y outer, x inner, cell lists in order, each moby once). Bounds as the sphere kernel
/// (`c − r ≥ 0`, `c + r < 1024`, `r > +0`; else nothing). Per moby (same candidates and bounding-sphere test
/// as the sphere kernel): the first mesh triangle within `r` (the sphere kernel's triangle test against
/// the **fixed** best `(r·1024)²`, not shrunk) or else the first primitive within `r` lists it. Flag 0x2
/// is not read (primitives are always tested); flags 0x4 / 0x10 / 0x20 / 0x80 act as in the sphere kernel.
///
/// The game also writes `CollOutput` (+0x18 = the first listed moby, +0x1c = 0x3f, +0x20 = (0,0,0,1)) and,
/// with a hit template, a hit record for every listed moby with `mode & 0x4000` (done by the caller:
/// `moby_update::services::sphere_mobys_in`). Returns the list (the kernel returns its length).
pub fn coll_sphere_mobys(sc: &MobyScene, centre: [f32; 3], radius: f32, flags: super::QueryFlags, ignore: Option<usize>) -> Vec<usize> {
    let fl = flags.0;
    let Some(vol) = Volume::new(centre, radius, None) else { return Vec::new() };
    let c = vol.c1024;
    let r = vol.r1024;
    let best = vol.rr; // vf24.x = (r*1024)^2, never updated
    let lo = [0, 1].map(|k| (ftoi0(vol.lo[k]) as u32) >> 4);
    let hi = [0, 1].map(|k| (ftoi0(vol.hi[k]) as u32) >> 4);
    let mut stamped: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for y in lo[1]..=hi[1] {
        for x in lo[0]..=hi[0] {
            for &idx in sc.grid.cell(x, y) {
                let id = idx as usize;
                if Some(id) == ignore { continue; }
                let Some(m) = sc.mobys.moby(id) else { continue };
                if stamped.contains(&id) || !m.collision { continue; }
                let bs = m.bsphere;
                let rr = add(r, bs[3]);
                let e = vsub([bs[0], bs[1], bs[2]], c);
                let sq = vmul(e, e);
                let v = add(add(sub(sq[0], mul(rr, rr)), mul(ONE, sq[1])), mul(ONE, sq[2]));
                if v as i32 > 0 { continue; }
                stamped.push(id);
                let Some(blob) = sc.classes.get(&m.o_class) else { continue };
                let n = joint_count(blob, fl);
                let joints = if n != 0 { sc.joints(id, m.key, n) } else { Vec::new() };
                let pos = vmuls(m.position, K1024);
                let mut hit = false;
                if !blob.vertices.is_empty() {
                    let c_rel = vsub(c, pos);
                    let verts = mesh_verts(&m, blob, c_rel.map(|v| sub(v, r)), c_rel.map(|v| add(v, r)));
                    hit = mesh_faces(blob, &verts, fl).any(|([v0, v1, v2], _)| sphere_tri(fl, c_rel, v0.p, v1.p, v2.p, best).is_some());
                }
                if !hit {
                    hit = active_prims(blob, &m, fl).any(|(_, p)| {
                        let (cc, rad) = vol_prim(p, &m, pos, &joints, c, false);
                        vol_prim_test(c, r, cc, rad, best).is_some()
                    });
                }
                if hit { out.push(id); }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------
// Line (0x211fd0)

/// The moby pass of `CollLine_Fix`, after the world walk (which it follows even past its early out).
#[allow(clippy::too_many_arguments)]
pub(super) fn line_pass(sc: &MobyScene, fl: u32, a: V3, b: V3, d: V3, list: &[(F, [i32; 3])], ignore: Option<usize>, best_t: &mut F, mut best: Option<Best>) -> Option<Best> {
    // 0x211fd8: the DDA cells' (x>>2, y>>2), consecutive duplicates dropped, up to t == 1.0.
    let key = |c: [i32; 3]| (((c[0] & 0xff) >> 2) as u32, ((c[1] & 0xff) >> 2) as u32);
    let Some(first) = list.first() else { return best };
    let mut cells = vec![key(first.1)];
    for &(t, c) in &list[1..] {
        if t == ONE { break; }
        let k = key(c);
        if *cells.last().unwrap() != k { cells.push(k); }
    }
    let dn = vmuls(d, div(ONE, dot_x(d, d))); // vf21 = d / |d|^2
    let mut stamped: Vec<usize> = Vec::new();
    for (x, y) in cells {
        for &idx in sc.grid.cell(x, y) {
            let id = idx as usize;
            if Some(id) == ignore { continue; }
            let Some(m) = sc.mobys.moby(id) else { continue };
            if fl & 0x9 != 0 && m.mode & 0x4000 == 0 { continue; }
            let bs = m.bsphere;
            let bc = [bs[0], bs[1], bs[2]];
            let t = min(max(dot_x(vsub(bc, a), dn), 0), *best_t);
            let e = vsub(vmadds(a, d, t), bc);
            let sq = vmul(e, e);
            let v = add(add(sub(sq[0], mul(bs[3], bs[3])), mul(ONE, sq[1])), mul(ONE, sq[2]));
            if !m.collision || v as i32 > 0 || stamped.contains(&id) { continue; }
            stamped.push(id);
            let Some(blob) = sc.classes.get(&m.o_class) else { continue };
            let n = joint_count(blob, fl);
            let joints = if n != 0 {
                if fl & 0x2 != 0 && blob.vertices.is_empty() { continue; }
                sc.joints(id, m.key, n)
            } else {
                Vec::new()
            };
            let pos = vmuls(m.position, K1024); // vf16
            if !blob.vertices.is_empty() {
                let a_rel = vsub(a, pos); // vf14
                let b_rel = vsub(b, pos); // vf15
                let lo: V3 = std::array::from_fn(|k| min(a_rel[k], b_rel[k]));
                let hi: V3 = std::array::from_fn(|k| max(a_rel[k], b_rel[k]));
                let verts = mesh_verts(&m, blob, lo, hi);
                for ([v0, v1, v2], ty) in mesh_faces(blob, &verts, fl) {
                    let Some((q, p, nrm, e1, e2)) = line_tri(fl, d, a_rel, b_rel, v0.p, v1.p, v2.p, *best_t) else { continue };
                    *best_t = q;
                    best = Some(record(ty, p, nrm, v0.p, e1, e2, pos, Some(id)));
                }
            }
            if fl & 0x2 != 0 { continue; }
            for (i, p) in active_prims(blob, &m, fl) {
                let Some((t, hit, nrm)) = line_prim(p, &m, pos, &joints, a, b, d, *best_t) else { continue };
                *best_t = add(0, t);
                best = Some(prim_best(i, hit, nrm, best.as_ref(), id));
            }
        }
    }
    best
}

/// One primitive against the segment (0x2123e0..0x212818): `(t, hit, hit − C)`.
#[allow(clippy::too_many_arguments)]
fn line_prim(p: &MobyCollPrim, m: &CollMoby, pos: V3, joints: &[[F; 4]], a: V3, b: V3, d: V3, best_t: F) -> Option<(F, V3, V3)> {
    let s = m.scale;
    let four: F = 0x4080_0000;
    let (cc, rad) = match p.kind() {
        1 => {
            let q = p.q1();
            (vadd(rot(&m.rows, [mul(q[0], s), mul(q[1], s), mul(q[2], s)]), pos), mul(q[3], s))
        }
        2 => {
            let j = joint(joints, p.word4());
            let o = p.q1();
            let v = vmuls([add(j[0], o[0]), add(j[1], o[1]), add(j[2], o[2])], s);
            (vadd(rot(&m.rows, v), pos), mul(p.q0()[3], s))
        }
        3 => {
            // Infinite vertical cylinder in x/y first.
            let q = p.q1();
            let cc = vadd(rot(&m.rows, [mul(q[0], s), mul(q[1], s), mul(q[2], s)]), pos);
            let rad = mul(q[3], s);
            let d2 = vsub(b, a);
            let w = vsub(cc, a);
            let dd = max(add(mul(d2[0], d2[0]), mul(d2[1], d2[1])), 0x3c80_0000);
            let wd = add(mul(w[0], d2[0]), mul(w[1], d2[1]));
            let ww = add(mul(w[0], w[0]), mul(w[1], w[1]));
            let q = div(wd, dd);
            let rr = mul(rad, rad);
            let t = min(max(add(0, q), 0), ONE);
            let e = [sub(add(a[0], mul(d2[0], t)), cc[0]), sub(add(a[1], mul(d2[1], t)), cc[1])];
            let v = add(sub(mul(e[0], e[0]), rr), mul(ONE, mul(e[1], e[1])));
            if v as i32 > 0 { return None; }
            let t = entry_root(wd, ww, dd, rr, four);
            let p6 = vmadds(a, d2, t);
            let top = add(cc[2], mul(p.q0()[1], s));
            (vec_z(cc, min(max(cc[2], p6[2]), top)), rad)
        }
        _ => {
            let [j0, j1] = p.joints();
            let (ja, jb) = (joint(joints, j0 as i32), joint(joints, j1 as i32));
            let rad = mul(p.q0()[3], s);
            let a0 = rot(&m.rows, vmuls([ja[0], ja[1], ja[2]], s));
            let b0 = rot(&m.rows, vmuls([jb[0], jb[1], jb[2]], s));
            let rx = add(0, rad);
            let rr = mul(rx, rx);
            let e = vsub(b0, a0);
            let pa = vadd(a0, pos);
            let n3 = cross(e, d);
            let n4 = cross(d, e);
            let n3sq = vmul(n3, n3);
            let n7 = cross(d, n3);
            let n8 = cross(e, n4);
            let parallel = (0..3).all(|k| neg(sub(n3sq[k], ONE)));
            let mut x = a;
            if !parallel {
                let nn = add(add(n3sq[0], n3sq[1]), mul(ONE, n3sq[2]));
                let s7 = dot_x(n7, vsub(a, pa));
                let s8 = dot_x(n8, vsub(pa, a));
                let c3 = vmadds(pa, e, div(s7, nn)); // closest point on the capsule axis line
                let c4 = vmadds(a, d, div(s8, nn)); // closest point on the query line
                let ee = dot_x(e, e);
                let dd = dot_x(d, d);
                let g = vsub(c4, c3);
                let gs = vmul(g, g);
                let v = add(add(add(sub(0, rr), gs[0]), mul(ONE, gs[1])), mul(ONE, gs[2]));
                if !neg(v) { return None; }
                let nv = sub(0, v);
                let eh = vmuls(e, div(ONE, sqrt(ee)));
                let dh = vmuls(d, div(ONE, sqrt(dd)));
                let cs = dot_x(eh, dh);
                let sin2 = sub(ONE, mul(cs, cs));
                let k = max(add(0, div(nv, sin2)), 0);
                let entry = vsub(c4, vmuls(dh, sqrt(k)));
                if !neg(dot_x(vsub(entry, a), d)) { x = entry; }
            }
            let u = vsub(x, pa);
            let t = min(max(add(0, div(dot_x(u, e), dot_x(e, e))), 0), ONE);
            (vadd(pa, vmuls(e, t)), add(rad, 0))
        }
    };
    // Common tail 0x212720: sphere (cc, rad) against the segment.
    let d2 = vsub(b, a);
    let w = vsub(cc, a);
    let dd = dot_x(d2, d2);
    let wd = dot_x(w, d2);
    let ww = dot_x(w, w);
    let rr = mul(rad, rad);
    let t = min(max(add(0, div(wd, dd)), 0), ONE);
    let e = vsub(vmadds(a, d2, t), cc);
    let sq = vmul(e, e);
    let v = add(add(sub(sq[0], rr), mul(ONE, sq[1])), mul(ONE, sq[2]));
    if v as i32 > 0 { return None; }
    let t = entry_root(wd, ww, dd, rr, four);
    let hit = vmadds(a, d2, t);
    let nrm = vsub(hit, cc);
    if sub(best_t, t) as i32 <= 0 { return None; }
    Some((t, hit, nrm))
}

/// `t = −((−2wd + √max((2wd)² − (4·dd)·(ww − R²), 0)) / (dd + dd))`, clamped to [0, 1]: the segment's entry
/// into the sphere (or the cylinder, in x/y).
fn entry_root(wd: F, ww: F, dd: F, rr: F, four: F) -> F {
    let k = add(wd, wd);
    let wr = sub(ww, rr);
    let disc = max(sub(mul(k, k), mul(mul(four, dd), wr)), 0);
    let root = sqrt(disc);
    let q = div(add(sub(0, k), root), add(dd, dd));
    min(max(sub(0, q), 0), ONE)
}
