//! **The Hydrodisplacer pools of Veldin's last level, class 1402** (level18 `0x2f2310`, its group carry `0x2f2598` and
//! draw callbacks `0x2f2970` / `0x2f2620` / `0x2f27c8`; census U594; three placed, #872..#874, each the link of a
//! Hydrodisplacer pad 341). Each pool sits at one of two heights, pvar +0x00 (full) and +0x04 (drained), and springs
//! toward it; every moby of its group (the water and what floats on it) follows its height. The Hydrodisplacer
//! drains a full pool (the pool's command +0xbc bit 1 says it can give, the gadget sends 8) and fills a drained one
//! (bit 0, the gadget sends 4); the first time Ratchet pours into a pool whose start word +0x08 is 0, global flag 0x7e
//! is set, which swaps every pool's start for good. A gadget still full at a restart makes the pools start drained,
//! and a full gadget remembered by the level (gp−0x4904) is given back. The visible surface is the pool's own mesh
//! (one of three, chosen by +0x14), drawn shifted by the height change, its texture scrolling. Read from the level18
//! decomp and data (gp−0x4908..−0x48d8, the meshes 0x1da3c0..0x1db3a0). Native `f32`.
//!
//! **Pvars** (0x20): +0x00 the full height, +0x04 the drained height, +0x08 the start word (1: start full), +0x0c /
//! +0x10 (unread), +0x14 the mesh (0, 1, 2), +0x18 the spring's velocity, +0x1c the pool's number (the order of their
//! first updates).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | `0x2f2310` state 0 | gp−0x4904 ≠ 0 → 0x141400 (the Hydrodisplacer full) = it; +0x1c = gp−0x4908, which counts up; +0x30 = 0xff, → 1, mode \| 1 (the moby itself not drawn), +0x32 = 0, collision off | [`update`] (`HeroFields::hydro_full`) |
//! | state 1 | flag 0x13d406 (0x7e) set: +0x08 = (+0x08 + 1) & 1; gp−0x4908 = 0; +0x08 = 0 or 0x141400 → 3, z = +0x04; else → 2, z = +0x00 | [`update`] |
//! | state 2 (full) | +0xbc & 8 → 3, gp−0x4904 = 1; +0xbc = 2; `0x2f2598`; `0x270830(+0x00, 4·dt², 4·dt², 4·dt, &z, &+0x18)` | [`update`] (`turn::spring`), [`carry`] |
//! | state 3 (drained) | +0xbc & 4 → gp−0x4904 = 0, 2, +0x08 = 0 → flag 0x7e = 1; the spring toward +0x04; `0x2f2598`; +0xbc = 1 | [`update`] |
//! | every state | +0x14: 1 → gp−0x48f8 += 0.05·dt (gp−0x48f4), −1 past 1, `RegisterDrawCallback(0x2f2620)`; 0 → `(0x2f2970)`; 2 → `(0x2f27c8)` | [`update`] (`Callback::UnitQuads`) |
//! | `0x2f2598` | every moby of the group (0x1ac240[+0x21]): z = the pool's z; a 1402 among them: the ripple patch level `0x1db508 + its +0x1c·0x1190` = z | [`carry`] (the patch word has no reader on level 18: no ripple manager sets up 0x1612d0 there [L]) |
//! | draws | the frame: identity, translation (0, 0, z − +0x00) (`0x1f9fc8`); quads of the mesh, each corner a (vertex, ST) index pair; ST s + gp−0x48f8; colour 0x40505050 (gp−0x48d8); FX 0x2b (gp−0x48dc), TEX1 0xff9000000260, ALPHA A 0 B 1 C 0 D 1 FIX 0x80 (blended) | [`fx_quads`] + `rc-engine` fx_draw |
//! | the meshes | `0x2f2970`: 22 quads (vertices 0x1db030, quads 0x1db240, ST 0x1db3a0); `0x2f2620`: 25 (0x1da3c0, 0x1da610, 0x1da7a0); `0x2f27c8`: 38 (0x1da8d0, 0x1dac20, 0x1dae80) | [`Meshes::parse`] (read from the overlay at the level load) |

use std::sync::Arc;

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{turn, DT, DT2};
use crate::moby_update::scheduler::group_ids;
use crate::moby_update::services::{pvar as p, Services, World};

use super::{FxQuad, FxQuads};

pub const REFERENCE_LEVEL: u32 = 18;
pub const UPDATE_FN: u32 = 0x2f_2310;
/// The draw callbacks of meshes 0, 1, 2.
pub const DRAW_FNS: [u32; 3] = [0x2f_2970, 0x2f_2620, 0x2f_27c8];
pub const CLASSES: [i16; 1] = [1402];

const LEN: usize = 0x20;
const FULL: usize = 0x00;
const DRAINED: usize = 0x04;
const START: usize = 0x08;
const MESH: usize = 0x14;
const VEL: usize = 0x18;
const NUMBER: usize = 0x1c;
/// The level words: the pools counted at their first update, the gadget full, the texture scroll.
const COUNT_WORD: u32 = 0x16_22f8;
const GADGET_WORD: u32 = 0x16_22fc;
const SCROLL_WORD: u32 = 0x16_2308;
const SCROLL_RATE: f32 = 0.05;
/// Global flag 0x13d406.
const SWAP_FLAG: usize = 0x7e;
const RGBA: u32 = 0x4050_5050;
const FX: usize = 0x2b;
/// The meshes' tables (vertices, quads, ST) and quad counts, in mesh order.
const TABLES: [(u32, u32, u32, usize); 3] = [
    (0x1d_b030, 0x1d_b240, 0x1d_b3a0, 22),
    (0x1d_a3c0, 0x1d_a610, 0x1d_a7a0, 25),
    (0x1d_a8d0, 0x1d_ac20, 0x1d_ae80, 38),
];

/// One pool surface mesh: vertices, ST pairs, and quads of (vertex, ST) index pairs in GS strip order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub verts: Vec<[f32; 3]>,
    pub st: Vec<[f32; 2]>,
    pub quads: Vec<[(u16, u16); 4]>,
}

/// The three pool meshes of level 18 (`Globals::veldin_pools`, set by the engine at the level load).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meshes {
    pub meshes: Vec<Mesh>,
}

impl Meshes {
    /// Reads the meshes from level 18's overlay (None on another level or when a table does not read).
    pub fn parse(ov: &rc_formats::water::Overlay, level: u32) -> Option<Arc<Meshes>> {
        if level != REFERENCE_LEVEL { return None; }
        let mut meshes = Vec::new();
        for &(va, qa, sa, n) in &TABLES {
            let b = ov.read(qa, 16 * n).ok()?;
            let h = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
            let quads: Vec<[(u16, u16); 4]> = (0..n).map(|q| std::array::from_fn(|k| (h(16 * q + 4 * k), h(16 * q + 4 * k + 2)))).collect();
            let nv = quads.iter().flatten().map(|c| c.0 as u32 + 1).max()?;
            let ns = quads.iter().flatten().map(|c| c.1 as u32 + 1).max()?;
            let verts = (0..nv).map(|k| Some([ov.f32(va + 16 * k).ok()?, ov.f32(va + 16 * k + 4).ok()?, ov.f32(va + 16 * k + 8).ok()?])).collect::<Option<Vec<_>>>()?;
            let st = (0..ns).map(|k| Some([ov.f32(sa + 8 * k).ok()?, ov.f32(sa + 8 * k + 4).ok()?])).collect::<Option<Vec<_>>>()?;
            meshes.push(Mesh { verts, st, quads });
        }
        Some(Arc::new(Meshes { meshes }))
    }
}

/// Level18 `0x2f2310` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, LEN);
    let st = w.m(id).state;
    match st {
        0 => {
            let g = w.svc.units.word(GADGET_WORD);
            if g != 0 { w.hero_fields_mut().hydro_full = Some(g as u8 != 0); }
            let n = w.svc.units.word(COUNT_WORD);
            w.svc.units.set_word(COUNT_WORD, n.wrapping_add(1));
            let m = w.mm(id);
            p::set_u32(&mut m.pvars, NUMBER, n);
            m.update_dist = 0xff;
            m.state = 1;
            m.mode |= 1;
            m.draw_dist = 0;
            m.has_collision = false;
        }
        1 => {
            let mut start = p::i32(&w.m(id).pvars, START);
            if crate::moby_update::story::flag(w, SWAP_FLAG) != 0 {
                start = (start + 1) & 1;
                p::set_i32(&mut w.mm(id).pvars, START, start);
            }
            w.svc.units.set_word(COUNT_WORD, 0);
            let pv = &w.m(id).pvars;
            let (full, drained) = (p::ff(pv, FULL), p::ff(pv, DRAINED));
            // 0x141400 as state 0 left it: its store is a queued hero write the tick applies after the moby loop, and
            // after a death reload the load pass's store is still queued when this runs (read as empty, the pool
            // started full under a full gadget, the way on blocked).
            let gadget_full = w.hero_fields().hydro_full.unwrap_or(w.hero.gadgets.hydro_full);
            let drain = start == 0 || gadget_full;
            let m = w.mm(id);
            if drain {
                m.state = 3;
                m.position[2] = drained;
            } else {
                m.state = 2;
                m.position[2] = full;
            }
        }
        2 => {
            if w.m(id).cmd & 8 != 0 {
                w.mm(id).state = 3;
                w.svc.units.set_word(GADGET_WORD, 1);
            }
            w.mm(id).cmd = 2;
            carry(w, id);
            spring(w, id, FULL);
        }
        3 => {
            if w.m(id).cmd & 4 != 0 {
                w.svc.units.set_word(GADGET_WORD, 0);
                w.mm(id).state = 2;
                if p::i32(&w.m(id).pvars, START) == 0 { crate::moby_update::story::set_flag(w, SWAP_FLAG, 1); }
            }
            spring(w, id, DRAINED);
            carry(w, id);
            w.mm(id).cmd = 1;
        }
        _ => {}
    }
    let mesh = p::i32(&w.m(id).pvars, MESH);
    if mesh == 1 {
        let mut s = f32::from_bits(w.svc.units.word(SCROLL_WORD)) + SCROLL_RATE * DT;
        if 1.0 < s { s -= 1.0; }
        w.svc.units.set_word(SCROLL_WORD, s.to_bits());
    }
    let Some(&f) = usize::try_from(mesh).ok().and_then(|k| DRAW_FNS.get(k)) else { return };
    if let Some(i) = super::row(REFERENCE_LEVEL, f) { w.svc.draw_callbacks.register(Callback::UnitQuads(i), id); }
}

/// `0x270830(target, 4·dt², 4·dt², 4·dt, &z, &+0x18)`: the height springs toward pvar `target`.
fn spring(w: &mut World, id: MobyId, target: usize) {
    let pv = &w.m(id).pvars;
    let (t, mut v) = (p::ff(pv, target), p::ff(pv, VEL));
    let mut z = w.m(id).position[2];
    turn::spring(t, 4.0 * DT2, 4.0 * DT2, 4.0 * DT, &mut z, &mut v);
    let m = w.mm(id);
    m.position[2] = z;
    p::set_ff(&mut m.pvars, VEL, v);
}

/// `0x2f2598`: the group follows the pool's height (module table).
fn carry(w: &mut World, id: MobyId) {
    let z = w.m(id).position[2];
    for k in group_ids(w, w.m(id).group) {
        if let Some(m) = w.table.mobys.get_mut(k) { m.position[2] = z; }
    }
}

/// The draw callback's quads for pool `id` (module table).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < LEN { return None; }
    let mesh = svc.units.veldin_pools.as_ref()?.meshes.get(usize::try_from(p::i32(&m.pvars, MESH)).ok()?)?;
    let dz = m.position[2] - p::ff(&m.pvars, FULL);
    let scroll = f32::from_bits(svc.units.word(SCROLL_WORD));
    let quads = mesh.quads.iter().filter_map(|q| {
        let corners = q.iter().map(|&(v, _)| mesh.verts.get(v as usize).map(|v| [v[0], v[1], v[2] + dz])).collect::<Option<Vec<_>>>()?;
        let st = q.iter().map(|&(_, s)| mesh.st.get(s as usize).map(|s| [s[0] + scroll, s[1]])).collect::<Option<Vec<_>>>()?;
        Some(FxQuad { corners: corners.try_into().ok()?, st: st.try_into().ok()?, rgba: [RGBA; 4] })
    }).collect();
    Some(FxQuads { fx: FX, additive: false, subtract: false, quads })
}
