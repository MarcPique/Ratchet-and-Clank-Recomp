//! **Blarg's laser gates, class 1035** (level06 `0x2f6470` with its beams' tick `0x2f7000` and its draw callback
//! `0x2f6dd0` through the strip emitter `0x216b88`; 5 placed; census U228; the name is descriptive [L]). A slowly
//! turning web of crackling beams across the doorway: the shared four point sets of every gate run along y (`0x2f7000`
//! writes the run at +4) and are drawn through nine frames, three groups at 0 / ±60° about x (the door's normal), one
//! on the axis and two on a hexagon around it, so the beams cross as a star. It fades in within 48 of the camera and
//! hums. Two lines across it hurt for 5; touching it or
//! hitting it counts against Ratchet (a 30-tick hold between counts), and after three a hint plays (help 0x1774, record
//! 0x2b). It goes down when its four links (+0x00..+0x0c) are all done: a generator 1302 in state 2, or any other moby
//! with its command byte set; then the beams burst into sparks and it is deleted. Brought down with no generator
//! among its links, it counts in the hint's record (the hint is then never given). Read from the level06 decomp; its words
//! gp−0x4e30..−0x4dc8. Native `f32`.
//!
//! **Pvars**: +0x00..+0x0c the links (−1 none), +0x10 the hum's voice, +0x14 (s16) the touches, +0x16 (s16) the hold,
//! +0x18 the fade (0..1).
//!
//! | state | what | port |
//! |---|---|---|
//! | 0 | the shared countdowns 0, 11, 23, 34 (0x161e40..), the counts 45 / 22; no voice; → 1; mode \|= 0x4000 | [`update`] |
//! | 1 | the hold ticks; the hum; `0x2f7000`: rot.x += 5°·dt, the shared scroll (gp−0x4e04) += 2·dt (wrapped), scale = class scale · 1.75, the shared beams' tick (every gate steps them) | [`update`] (`blarg_barrier::Beams::step`) |
//! | 1 | within 48 of the camera: the fade `Approach`ed to 1 by 2·dt, else to 0; within 48 two lines (from 2.25 below, 0.5 along its facing ± 3 across) with the template `0x268698(5, tmpl, m, 0x10001, 0.5·facing)` (= `0x26e808`), each one that hits Ratchet's moby (0x1745d8) counts; Ratchet's capsule on it (0x13f58c) → `0x268898(Ratchet, ±row 0)` (= `0x26e968`, away from it) and a count | [`update`] |
//! | 1 | fade ≠ 0 → the draw; the hint's record (0x141ac0) at 0: no hold and a hit on it (0x330000) → a count; +0xa4 0xff; more than two → `Help_Request(0x1774, 0x2b)` | [`update`] |
//! | 1 | every link done → no collision, 2, the hum released, sound 1; the hint's record (help 0x2b): no generator among the links → its count + 1 (unless 0xffff); its time and level mask touched | [`update`] |
//! | 2 | per frame (0x1db060 Euler, 0x1dafd0 offset through Euler · 1.5): 20 sparks along its y (`randf_sym(0, 2.5)` on the frame's row 1), `FastTweenColor(randf(0, 1), 0x80802020, 0x80802080)`, size `randf(0.125, 0.333)`, `PartType60Spawn(…, rand_vec(1.2·dt, 2.1·dt), ticks(trunc(randf(30, 60))), randi(255), 0)`; → 3 | [`update`] |
//! | 3 | deleted | [`update`] |
//! | `0x2f6dd0` | FX 0xe, additive; per frame the four strips, colours `FastTweenColor(fade, 0x80c04070 & 0xffffff, 0x80c04070)` / 0x00ff0000, S 0.25 per point from the scroll | [`fx_quads`] |

use super::blarg_barrier::frame;
use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::creature::{self as c, turn, V};
use crate::moby_update::services::{pf, pv as v4, pvar as p, HitTemplate, Services, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 6;
pub const UPDATE_FN: u32 = 0x2f_6470;
pub const DRAW_FN: u32 = 0x2f_6dd0;
pub const CLASSES: [i16; 1] = [1035];
/// The generators (0x516) that hold a gate up until destroyed (state 2).
pub const GENERATOR: i16 = 0x516;
/// gp−0x4e04 (0x161dfc): the gates' scroll.
const SCROLL_KEY: u32 = 0x16_1dfc;
/// The gate hint's help record (0x141ac0 = 0x141968 + 0x2b·8) and its message.
const HINT_REC: usize = 0x2b;
const HINT_MSG: i32 = 0x1774;
/// gp−0x4e00 (2), −0x4dfc (0.4), −0x4df8 (5), −0x4df4 (1.5), −0x4df0 (5°/s), −0x4dec (1.75), −0x4de8 (2.25).
const LENGTH: f32 = 5.0;
const DRIFT: f32 = f32::from_bits(0x3ecc_cccd);
const FRAME_SCALE: f32 = 1.5;
const ROLL: f32 = 5.0;
const DROP: f32 = 2.25;
const COLOUR: u32 = 0x80c0_4070;
const COLOUR_B: u32 = 0x00ff_0000;
/// 0x1db060 / 0x1dafd0: the nine frames' Euler x and offsets (y, z).
/// (The x angles are the data's 1.0472, not π/3.)
#[allow(clippy::approx_constant)]
const FRAMES: [(f32, f32, f32); 9] = [
    (0.0, 0.0, 0.0),
    (-1.0472, 0.0, 0.0),
    (1.0472, 0.0, 0.0),
    (0.0, 0.0, 1.21),
    (-1.0472, 0.866, 1.0),
    (1.0472, -0.866, 1.0),
    (0.0, 0.0, -1.21),
    (-1.0472, -0.866, -1.0),
    (1.0472, 0.866, -1.0),
];

fn frame_k(rot: V, pos: V, k: usize) -> ([[f32; 3]; 3], [f32; 3]) {
    let (ex, oy, oz) = FRAMES[k];
    let m = frame(rot, FRAME_SCALE, [0.0; 3]);
    let off = [0.0, oy, oz];
    let at = std::array::from_fn(|i| m[0][i] * off[0] + m[1][i] * off[1] + m[2][i] * off[2] + pos[i]);
    (frame(rot, FRAME_SCALE, [ex, 0.0, 0.0]), at)
}

/// A count against Ratchet: +1, the hold `ticks(30)`.
fn count(w: &mut World, id: MobyId) {
    let n = c::pi16(w, id, 0x14) + 1;
    c::set_pi16(w, id, 0x14, n);
    let t = w.ticks(30);
    c::set_pi16(w, id, 0x16, t as i16);
}
/// The hint not given yet (its record's count 0).
fn stats_open(w: &World) -> bool { w.svc.help.records.help[HINT_REC].count == 0 }
fn may_count(w: &World, id: MobyId) -> bool { stats_open(w) && c::pi16(w, id, 0x16) == 0 }

/// Level06 `0x2f6470` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x1c { w.mm(id).pvars.resize(0x1c, 0); }
    match w.m(id).state {
        0 => {
            w.svc.units.blarg_gates.count = [0, 0xb, 0x17, 0x22];
            c::set_pi32(w, id, 0x10, -1);
            w.mm(id).state = 1;
            w.mm(id).mode |= 0x4000;
        }
        1 => tick(w, id),
        2 => {
            let (rot, pos) = (w.m(id).rotation, w.m(id).position);
            for k in 0..9 {
                let (rows, at) = frame_k(rot, pos, k);
                // Along the frame's y row (`FastVecNormalize` of +0x10): the gates' beams run along y.
                let axis = c::set_len3([rows[1][0], rows[1][1], rows[1][2], 0.0], 1.0);
                for _ in 0..20 {
                    let s = w.rng.randf_sym(0.0, LENGTH * 0.5);
                    let pt = c::add(c::scale(axis, s), [at[0], at[1], at[2], 1.0]);
                    let f = w.rng.randf(0.0, 1.0);
                    let col = crate::particles::tween_color(f.to_bits(), 0x8080_2020, 0x8080_2080);
                    let size = w.rng.randf(0.125, f32::from_bits(0x3eaa_7efa));
                    let life = w.rng.randf(30.0, 60.0) as i32;
                    let v = w.rng.rand_vec(1.2 * c::DT, f32::from_bits(0x4006_6666) * c::DT);
                    let life = w.ticks(life);
                    let seed = w.rng.randi(0xff);
                    w.part60(size, pt, [v[0], v[1], v[2], 0.0], col, life as u16, seed as u8, 0);
                }
            }
            w.mm(id).state = 3;
        }
        3 => w.delete_moby(id),
        _ => {}
    }
}

fn tick(w: &mut World, id: MobyId) {
    c::dec_timer_pvar_s16(w, id, 0x16);
    let v = c::pi32(w, id, 0x10);
    if !w.sound_alive(v, id) {
        let s = w.play_sound(0, 4, id);
        c::set_pi32(w, id, 0x10, s);
    }
    w.mm(id).rotation[0] = c::add_rot(w.m(id).rotation[0], ROLL * 0.017_453_292 * c::DT);
    let mut sc = f32::from_bits(w.svc.units.word(SCROLL_KEY)) + 2.0 * c::DT;
    if 1.0 < sc { sc -= 1.0; }
    w.svc.units.set_word(SCROLL_KEY, sc.to_bits());
    let o = w.m(id).o_class;
    w.mm(id).scale = super::class_scale(w, o) * 1.75;
    let mut b = std::mem::take(&mut w.svc.units.blarg_gates);
    b.step(w, LENGTH, DRIFT * c::DT, super::blarg_barrier::PERIOD, 1);
    w.svc.units.blarg_gates = b;
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let mut fade = c::pf(w, id, 0x18);
    if c::dist3(c::pos(w, id), cam) < 48.0 {
        turn::approach(1.0, c::DT + c::DT, &mut fade);
        c::set_pf(w, id, 0x18, fade);
        let yaw = c::yaw(w, id);
        for off in [0.0, std::f32::consts::PI] {
            // The game's two lines: 0.5 along yaw (then yaw + π), ±3 along yaw + π/2.
            let a = c::add_rot(yaw, off);
            let along = [a.cos() * 0.5, a.sin() * 0.5, 0.0, 0.0];
            let b2 = c::add_rot(yaw, std::f32::consts::FRAC_PI_2);
            let across = [b2.cos() * 3.0, b2.sin() * 3.0, 0.0, 0.0];
            let p0 = c::pos(w, id);
            let base = [p0[0], p0[1], p0[2] - DROP, p0[3]];
            let from = c::add(c::add(base, along), across);
            let to = c::sub(c::add(base, along), across);
            let t = HitTemplate { dir: [pf(along[0]), pf(along[1]), Pf::ZERO, Pf::ZERO], attacker: Some(id), flags: 0x1_0001, b18: 0, b19: 0, h1a: 0, damage: pf(5.0), w20: 1 };
            let hit = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, v4(from), v4(to), 0, Some(id), &t);
            if may_count(w, id) && hit.and_then(|h| h.moby).is_some() && hit.and_then(|h| h.moby) == w.hero_moby { count(w, id); }
        }
        if w.hero.cap_moby == Some(id) {
            if let Some(h) = w.hero_moby {
                let d = c::sub(super::hero_pos(w), c::pos(w, id));
                let r0 = w.m(id).rows[0];
                let dir = c::set_len3(r0, if 0.0 < c::dot3(d, r0) { 1.0 } else { -1.0 });
                let t = HitTemplate { dir: [pf(dir[0]), pf(dir[1]), pf(dir[2]), pf(dir[3])], attacker: Some(id), flags: 0x1_0001, b18: 0, b19: 0, h1a: 0, damage: pf(5.0), w20: 1 };
                w.deliver_hit(h, &t);
                if may_count(w, id) { count(w, id); }
            }
        }
    } else {
        turn::approach(0.0, c::DT + c::DT, &mut fade);
        c::set_pf(w, id, 0x18, fade);
    }
    if c::pf(w, id, 0x18) != 0.0 {
        if let Some(r) = super::row(REFERENCE_LEVEL, DRAW_FN) { w.svc.draw_callbacks.register(crate::moby_update::classes::draw_callbacks::Callback::UnitQuads(r), id); }
    }
    if stats_open(w) {
        if c::pi16(w, id, 0x16) == 0 && w.get_hit(id, 0x33_0000, false).is_some() { count(w, id); }
        w.mm(id).hit_slot = 0xff;
        if 2 < c::pi16(w, id, 0x14) { w.svc.help.request(HINT_MSG, HINT_REC as i32); }
    }
    let (mut down, mut no_gen) = (true, true);
    for k in 0..4 {
        let l = c::pi32(w, id, 4 * k);
        let Some(m) = usize::try_from(l).ok().filter(|&m| m < w.table.mobys.len()) else { continue };
        if w.m(m).o_class == GENERATOR {
            if w.m(m).state == 2 { no_gen = false; } else { down = false; }
        } else if w.m(m).cmd == 0 {
            down = false;
        }
    }
    if !down { return; }
    w.mm(id).has_collision = false;
    w.mm(id).state = 2;
    let v = c::pi32(w, id, 0x10);
    if w.sound_alive(v, id) { w.release_sound(v, id); }
    c::set_pi32(w, id, 0x10, -1);
    w.play_sound(1, 0, id);
    // The hint's record: counted (unless 0xffff) when no generator held it, touched either way.
    let (lv, pt) = (w.svc.help.level, w.svc.help.play_time);
    let r = &mut w.svc.help.records.help[HINT_REC];
    if no_gen && r.count != 0xffff { r.count += 1; }
    crate::help::touch(r, lv, pt);
}

/// `0x2f6dd0` (module doc; draw only).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < 0x1c { return None; }
    let fade = p::ff(&m.pvars, 0x18);
    let a = crate::particles::tween_color(fade.to_bits(), COLOUR & 0x00ff_ffff, COLOUR);
    let s_off = f32::from_bits(svc.units.word(SCROLL_KEY));
    let mut quads: Vec<FxQuad> = Vec::new();
    for k in 0..9 {
        let (rows, at) = frame_k(m.rotation, m.position, k);
        svc.units.blarg_gates.quads(rows, at, (a, COLOUR_B), 0.25, s_off, &mut quads);
    }
    Some(FxQuads { fx: super::blarg_barrier::FX, additive: true, subtract: false, quads })
}
