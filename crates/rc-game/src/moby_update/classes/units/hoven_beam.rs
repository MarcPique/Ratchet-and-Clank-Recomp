//! **Hoven's power beams, class 1345** (level12 `0x3081b0`, its draw `0x308350`; census U441; 3 placed, each linked
//! to a moby by pvar +0x00). A crackling curtain that holds until its moby reaches state 7 (destroyed): then its
//! mission (+0xb0) is done, sound 1 plays and it loses its collision (state 2). Already done at the start: deleted.
//! While up it plays its hum (sound 0, flags 4) and — as the game does — releases it again the same tick whenever
//! its link is valid, and it draws within 48 when in view.
//!
//! **The draw** (`0x308350`): five rows (z 1..5 in its frame) of six segments from y −3 to 3 whose joints jitter by
//! `randf(−0.2, 0.2)`, each segment a quad 0.4 tall (z ±0.2), FX 0xe, additive, colour 0x405050ff; each row's ST s
//! shifted by `randf(−0.5, 0.5)` (0..1 per segment, t 0..1). A second, thinner quad (0x40ff80ff, z ±0.02) is set up
//! and never drawn. The `rand` draws happen at draw time: the frame part ([`frame`], `Callback::UnitFrame`) draws them
//! into pvar +0x10.., the quads ([`fx_quads`]) read them.
//!
//! Read from the level12 decomp and disassembly; the tables 0x1fb6d0 / 0x1fb6f0 / 0x1fb730 and gp−0x4c70..−0x4c28.
//! Native `f32`.

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature as c;
use crate::moby_update::services::{pvar as p, Services, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x30_81b0;
pub const DRAW_FN: u32 = 0x30_8350;
pub const CLASSES: [i16; 1] = [1345];

const LINK: usize = 0x00;
const VOICE: usize = 0x04;
/// The draw's jitters: five rows of (s shift, six joints).
const JIT: usize = 0x10;
const SIZE: usize = JIT + 5 * 7 * 4;

/// Level12 `0x3081b0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, SIZE);
    match w.m(id).state {
        0 => {
            let b0 = w.m(id).mission;
            if b0 != 0xff && story::mission_done(w, b0 as i32) {
                w.delete_moby(id);
                return;
            }
            w.mm(id).state = 1;
        }
        1 => {
            let m = w.m(id);
            let seen = w.view.is_some_and(|v| !v.culled(48.0, m.bsphere.map(|x| x * (1.0 / 1024.0))));
            if seen {
                if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) {
                    w.svc.draw_callbacks.register(Callback::UnitFrame(row), id);
                    w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
                }
            }
            if c::pi32(w, id, VOICE) == -1 {
                let v = w.play_sound(0, 4, id);
                c::set_pi32(w, id, VOICE, v);
            }
            let link = c::pi32(w, id, LINK);
            if link < 0 { return; }
            let v = c::pi32(w, id, VOICE);
            if v != -1 { w.release_sound(v, id); }
            c::set_pi32(w, id, VOICE, -1);
            let dead = usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)).is_some_and(|m| m.state == 7);
            if dead {
                let b0 = w.m(id).mission;
                if b0 != 0xff { crate::cinematic::set_mission_done(w, b0); }
                w.play_sound(1, 0, id);
                let m = w.mm(id);
                m.has_collision = false;
                m.state = 2;
            }
        }
        _ => {}
    }
}

/// The draw's `rand` part (module doc).
pub fn frame(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    for row in 0..5 {
        let s = w.rng.randf(-0.5, 0.5);
        c::set_pf(w, id, JIT + row * 28, s);
        for k in 0..6 {
            let j = w.rng.randf(f32::from_bits(0xbe4c_cccd), f32::from_bits(0x3e4c_cccd));
            c::set_pf(w, id, JIT + row * 28 + 4 + 4 * k, j);
        }
    }
}

/// Level12 `0x308350`: the curtain's quads (module doc).
pub fn fx_quads(table: &MobyTable, _svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= SIZE)?;
    let (r, pos) = (m.rows, m.position);
    let to_world = |v: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| r[0][i] * v[0] + r[1][i] * v[1] + r[2][i] * v[2] + pos[i]) };
    let mut quads = Vec::new();
    let (mut lo, mut hi) = (f32::from_bits(0xbe4c_cccd), f32::from_bits(0x3e4c_cccd));
    for row in 0..5 {
        lo += 1.0;
        hi += 1.0;
        let s = p::ff(&m.pvars, JIT + row * 28);
        let st = [[s, 0.0], [s, 1.0], [1.0 + s, 0.0], [1.0 + s, 1.0]];
        let mut carry = 0.0f32;
        for (k, i) in (-3i32..3).enumerate() {
            let y0 = i as f32 + carry;
            let j = p::ff(&m.pvars, JIT + row * 28 + 4 + 4 * k);
            let y1 = (i + 1) as f32 + j;
            carry = j;
            let corners = [[0.0, y0, lo], [0.0, y0, hi], [0.0, y1, lo], [0.0, y1, hi]].map(to_world);
            quads.push(FxQuad { corners, st, rgba: [0x4050_50ff; 4] });
        }
    }
    Some(FxQuads { fx: 0xe, additive: true, subtract: false, quads })
}
