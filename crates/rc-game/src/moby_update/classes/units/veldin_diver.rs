//! Veldin's divers, class 1355 (level 18, 36 created instances): level18 0x2efb88 (census U567; was U561), with the
//! group activation 0x2f0030, the wander 0x2f0120 / 0x2f02e8, the trail ring 0x2f0848 and the group's two draw
//! callbacks 0x2f0390 (the trails) and 0x2f06c0 (the glows). A diver waits hidden until the boss 1422 releases its
//! group (`activate`, 0x2f0030: the boss's state 0x14 calls it, `units::veldin_boss`); then it flies about a point 5–15 from its home at 4–16 up, around the boss, spinning, with
//! a fading green trail and a pulsing glow, and 1 in 1000 ticks dives to the ground and blows up (a beam explosion
//! that hurts, damage 1 in radius 1, credited to the boss), reappearing 20 above the camera. Read from the level18
//! decomp and the disassembly of the explosion call (0x2eff2c..0x2effa0: its stack arguments) and the overlay's data
//! (gp−0x49f0 .. −0x4994, 0x1da2f0). Native `f32`; the `rand` draws in the game's order.
//!
//! **Pvar block**: +0x10 + 0x10·i the trail ring (16 points, w = 1), +0x110 the wander target, +0x120 home (its z the
//! ground), +0x130 the dive velocity, +0x140 the yaw spring velocity, +0x144 the height spring velocity, +0x148 a yaw
//! offset (0), +0x14c the dive blend, +0x150 / +0x154 the yaw spring's rate and top, +0x158 / +0x15c the height
//! spring's, +0x160 s32 the timer, +0x164 s16 the trail length, +0x166 s16 the ring head, +0x168 the glow phase,
//! +0x16c the boss that released it.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2efb88 | state ≠ 4: rot.x += 360°·dt (gp−0x49f0) | [`update`] |
//! | | game mode (0x15f5c4) 2 and the level's scene id 0x16d290 = 3 → `DeleteMoby` | [`update`] (the scene id: the running scene the scene player publishes, `Cinematic::scene`; level18's starter 0x2983e8 is `DialogStreamStart`, called by the boss 1422) |
//! | state 0 | +0x15c = 10·dt, +0x150 = π·dt², +0x154 = 4π·dt, +0x158 = 10·dt², +0x168 = `rand_angle`; draw distance 0xff; home = position, z + 5, then z = `GroundHeight(0.5, home, 0)` (0x25b9d8 = L01 0x26e618); 0x2f02e8; → 1; collision off (+0x94 = 0), hidden (mode \| 1, +0x31 = 0) | [`update`], [`new_target`] |
//! | state 1 | waits for [`activate`] | n/a |
//! | state 2 | 0x2f0120 ≠ 0 → 0x2f02e8; 0x2f0848; `randi(1000)` (gp−0x49cc; 0x259e38 = L01 0x26c930) = 0 → state 3, +0x130 = (cos yaw, sin yaw)·20·dt (gp−0x49ec), +0x14c = 0, +0x138 = +0x144 | [`update`] |
//! | 0x2f0120 | d = target − position; yaw target = `atan(d)` + +0x148; `0x25e140(that, +0x150, +0x150, +0x154, &yaw, &+0x140)` (L01 0x270cc0); x / y += (cos, sin) yaw · 20·dt; `0x25dcb0(target z, +0x158, +0x158, +0x15c, &z, &+0x144)` (L01 0x270830); pitch +0x44 = −4 (gp−0x49d4) · +0x144 | [`wander`] (`turn::turn_toward`, `turn::spring`) |
//! | | \|d\| < 2 → 1; `FastDecTimer(+0x160)` out → 1; else with a boss: its xy distance ≥ 4.5 → 0, else target xy = boss + (position − boss) at xy length `randf(15, 20)` → 0; without → 0 | [`wander`] |
//! | 0x2f02e8 | r = `randf(5, 15)` (gp−0x49dc / −0x49d8), a = `rand_angle`, z = `randf(4, 16)` (gp−0x49e4 / −0x49e0); target = home + (cos a·r, sin a·r, z); timer = `ticks(180)` | [`new_target`] |
//! | 0x2f0848 | frame & 3 = 0: head = (head + 1) & 15, length < 4 → length + 1; ring[head] = position, w = 1; first call this frame (gp−0x49d0): `RegisterDrawCallback(0x2f0390)` and `(0x2f06c0)` with this moby (0x2020c8 = L01 0x21afe0) | [`trail`] (`Callback::UnitQuads` / `Callback::UnitGlow`) |
//! | state 3 | v = unit(home − position)·40·dt (gp−0x49e8); `Approach(1, dt, &+0x14c)`; v = lerp(v, +0x130, +0x14c); position += v; yaw = `atan(v)`, pitch = −`atan(\|v\|xy, v.z)`; nose = position + unit(row 0); 0x2f0848; nose.z < home z → `PlayClassSound(1, 0, m)`, → 4, timer = `ticks(90)`, z raised to home z + 0.7 if below | [`update`] |
//! | state 4 | `0x25df40(max(pitch, 50°), 4π·dt, &pitch, 0)` (L01 0x270ac0); timer running → 0x2f0848; out → state 2, length 0, `SpawnBeamExplosion(1, 1, 1.5, 0.5, 9, 1, 5, boss, 0x15f580, position, 10, 3, 16, sound 2, no shake, no debris, −1, 0)` (0x260790 = L01 0x273310), position = the camera (0x1677c0 on 18 [L]) + 20 up, ring[0..3] = it | [`update`] (`fx::beam_explosion`) |
//! | 0x2f0030 | the group list (0x1ac240 on 18) absent → 1; every member of class 1355 in state 1: position and ring[0..3] = pos, +0x16c = the caller, drawn (+0x31 = 1), shown (mode & ~1), → 2, collision on (class +0x10); returns 0 if any | [`activate`] |
//! | 0x2f0390 | (gp−0x49c8 = 0: on) per quad k = 0..15: ST (0, ½), (1, ½), (0, ½), (1, ½) (0x1da2f0), FX 0x13, ALPHA (0, 2, 0, 1) = 0x48 (additive), corner c's colour `FastTweenColor(clamp((k + 1 − c/2)·¼ + ((frame + 3) & 3)/16, 0, 1), 0x4000ff00, 0x0000ff80)` (gp−0x49b0 / −0x49ac) | [`fx_quads`] |
//! | | each member (class 1355 only: the game spins forever on another class), k = 0..length − 1: corners ring[(head − k − 1) & 15] ± (0, 0, 0.15), ring[(head − k) & 15] ± (0, 0, 0.15) (gp−0x49c4 = 0: drawn) | [`fx_quads`] |
//! | 0x2f06c0 | each member of class 1355 in states 2..4: colour = `FastTweenColor(sin(+0x168)·½ + ½, 0x80208020, 0x80208080)` (gp−0x49a0 / −0x499c); glow quad (0x265e28 = L01 0x2781d0) size 0.333 (gp−0x4998), pull 0, at position + unit(row 0)·(−0.1) (gp−0x4994) | [`glow_quads`] |
//! | | +0x168 += 360°·dt (states 2, 3; gp−0x49a8) or 720°·dt (state 4; gp−0x49a4): in the callback in the game; the port advances it at the registration, once a drawn frame, so the drawn colour leads by one step [L] | [`trail`] |
//! | | no light (the explosion's own), save flag, bolt | n/a |

use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::classes::gold_bolt::fast_dec_timer;
use crate::moby_update::creature::{self as c, fx, turn, DT, DT2};
use crate::moby_update::scheduler::group_ids;
use crate::moby_update::services::{self as sv, Services, World};

use super::{FxQuad, FxQuads, GlowQuad};

/// The update in the level18 class table.
pub const UPDATE_FN: u32 = 0x2e_fb88;
/// The trail callback: the port's second row (draw only).
pub const TRAIL_FN: u32 = 0x2f_0390;
pub const REFERENCE_LEVEL: u32 = 18;
pub const CLASSES: [i16; 1] = [1355];
/// The once-a-frame registration word (gp−0x49d0) and the frame counter 0x15f5cc for the draws (`Services::units`).
pub const FRAME_WORD: u32 = 0x16_2230;
pub const COUNTER_WORD: u32 = 0x15_f5cc;

pub mod pv {
    pub const RING: usize = 0x10;
    pub const TARGET: usize = 0x110;
    pub const HOME: usize = 0x120;
    pub const DIVE_V: usize = 0x130;
    pub const YAW_V: usize = 0x140;
    pub const Z_V: usize = 0x144;
    pub const YAW_OFS: usize = 0x148;
    pub const BLEND: usize = 0x14c;
    pub const YAW_RATE: usize = 0x150;
    pub const YAW_MAX: usize = 0x154;
    pub const Z_RATE: usize = 0x158;
    pub const Z_MAX: usize = 0x15c;
    pub const TIMER: usize = 0x160;
    pub const LEN: usize = 0x164;
    pub const HEAD: usize = 0x166;
    pub const PHASE: usize = 0x168;
    pub const BOSS: usize = 0x16c;
    pub const SIZE: usize = 0x170;
}

const DEG: f32 = 0.017_453_292;
/// gp−0x49f0 / −0x49ec / −0x49e8: the spin (°/s), the flying and the diving speed.
pub const SPIN: f32 = 360.0;
pub const SPEED: f32 = 20.0;
pub const DIVE: f32 = 40.0;
/// gp−0x49cc: 1 in this many ticks a diver dives.
pub const DIVE_ODDS: i32 = 1000;
pub const TRAIL_RGBA: [u32; 2] = [0x4000_ff00, 0x0000_ff80];
pub const TRAIL_FX: usize = 0x13;
pub const TRAIL_ST: [[f32; 2]; 4] = [[0.0, 0.5], [1.0, 0.5], [0.0, 0.5], [1.0, 0.5]];
pub const GLOW_RGBA: [u32; 2] = [0x8020_8020, 0x8020_8080];
pub const GLOW_SIZE: f32 = 0.333;
/// The beam explosion of the crash (module doc).
pub const CRASH: fx::Beam = fx::Beam { damage_r: 1.0, damage: 1.0, flash: 1.5, flash2: 0.5, flash_dist: 9.0, scale: 1.0, light: 5.0, streaks: 10, sparks: 3, puffs: 0x10, debris: 0, sound: 2, shake: false };

fn ok(m: &crate::moby_runtime::Moby) -> bool { m.o_class == CLASSES[0] && m.pvars.len() >= pv::SIZE }

fn timer(w: &mut World, id: MobyId) -> i32 {
    let mut t = c::pi32(w, id, pv::TIMER);
    let r = fast_dec_timer(&mut t);
    c::set_pi32(w, id, pv::TIMER, t);
    r
}

fn boss(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, pv::BOSS) - 1).ok().filter(|&b| w.table.mobys.get(b).is_some()) }

fn set_ring4(w: &mut World, id: MobyId, p: [f32; 4]) {
    for i in 0..4 { c::set_pv4(w, id, pv::RING + 0x10 * i, p); }
}

/// 0x2f0030(boss, group, pos): every member of `group` of class 1355 in state 1 starts flying at `pos`; the game's 1
/// when none did. The boss 1422 (`units::veldin_boss`, state 0x14) is its caller.
pub fn activate(w: &mut World, boss: Option<MobyId>, group: i8, pos: [f32; 4]) -> i32 {
    let ids = group_ids(w, group);
    if ids.is_empty() { return 1; }
    let mut r = 1;
    for m in ids {
        if !w.table.mobys.get(m).is_some_and(|o| ok(o) && o.state == 1) { continue; }
        r = 0;
        w.mm(m).position = pos;
        set_ring4(w, m, pos);
        c::set_pi32(w, m, pv::BOSS, boss.map_or(0, |b| b as i32 + 1));
        let col = super::class_collision(w, CLASSES[0]);
        let mo = w.mm(m);
        mo.visible = 1;
        mo.mode &= !mode::HIDDEN;
        mo.state = 2;
        mo.has_collision = col;
    }
    r
}

/// Level18 0x2efb88 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    if w.m(id).state != 4 {
        let r = c::add_rot(w.m(id).rotation[0], SPIN * DEG * DT);
        w.mm(id).rotation[0] = r;
    }
    if w.svc.game_mode == 2 && w.svc.cinematic.scene.as_ref().is_some_and(|sc| sc.id == 3) {
        w.delete_moby(id);
        return;
    }
    match w.m(id).state {
        0 => {
            c::set_pf(w, id, pv::Z_MAX, DT * 10.0);
            c::set_pf(w, id, pv::YAW_RATE, DT2 * std::f32::consts::PI);
            c::set_pf(w, id, pv::YAW_MAX, DT * 12.566_371);
            c::set_pf(w, id, pv::Z_RATE, DT2 * 10.0);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::PHASE, a);
            w.mm(id).draw_dist = 0xff;
            let mut home = w.m(id).position;
            home[2] += 5.0;
            home[2] = sv::fl(w.ground_height(sv::pf(0.5), sv::pv(home), 0));
            c::set_pv4(w, id, pv::HOME, home);
            new_target(w, id);
            let m = w.mm(id);
            m.state = 1;
            m.has_collision = false;
            m.mode |= mode::HIDDEN;
            m.visible = 0;
        }
        2 => {
            if wander(w, id) != 0 { new_target(w, id); }
            trail(w, id);
            if w.rng.randi(DIVE_ODDS) == 0 {
                let yaw = w.m(id).rotation[2];
                let zv = c::pf(w, id, pv::Z_V);
                w.mm(id).state = 3;
                c::set_pv4(w, id, pv::DIVE_V, [yaw.cos() * SPEED * DT, yaw.sin() * SPEED * DT, zv, c::pv4(w, id, pv::DIVE_V)[3]]);
                c::set_pf(w, id, pv::BLEND, 0.0);
            }
        }
        3 => {
            let pos = w.m(id).position;
            let home = c::pv4(w, id, pv::HOME);
            let d = c::sub(home, pos);
            let n = c::len3(d);
            let mut v = if n == 0.0 { [0.0, 0.0, 0.0, 2.0] } else { c::scale(d, DIVE * DT / n) };
            let mut t = c::pf(w, id, pv::BLEND);
            turn::approach(1.0, DT, &mut t);
            c::set_pf(w, id, pv::BLEND, t);
            let dv = c::pv4(w, id, pv::DIVE_V);
            v = std::array::from_fn(|k| v[k] + (dv[k] - v[k]) * t);
            let pos = c::add(pos, v);
            {
                let m = w.mm(id);
                m.position = pos;
                m.rotation[2] = c::atan(v[0], v[1]);
                m.rotation[1] = -c::atan(c::len2(v), v[2]);
            }
            let r0 = w.m(id).rows[0];
            let nose = c::add(c::set_len3(r0, 1.0), pos);
            trail(w, id);
            if nose[2] < home[2] {
                w.play_sound(1, 0, id);
                w.mm(id).state = 4;
                let t = w.ticks(0x5a);
                c::set_pi32(w, id, pv::TIMER, t);
                if w.m(id).position[2] < home[2] + 0.7 { w.mm(id).position[2] = home[2] + 0.7; }
            }
        }
        4 => {
            let p = w.m(id).rotation[1].max(0.872_664_6);
            let mut x = w.m(id).rotation[1];
            turn::approach_rot(p, DT * 12.566_371, &mut x);
            w.mm(id).rotation[1] = x;
            if timer(w, id) == 0 {
                trail(w, id);
            } else {
                w.mm(id).state = 2;
                c::set_pi16(w, id, pv::LEN, 0);
                let b = boss(w, id);
                let pos = w.m(id).position;
                fx::beam_explosion(w, &CRASH, b, pos);
                let cam = w.camera_point();
                let p = [cam[0], cam[1], cam[2] + 20.0, w.m(id).position[3]];
                w.mm(id).position = p;
                set_ring4(w, id, p);
            }
        }
        _ => {}
    }
}

/// 0x2f0120: one step toward the wander target; 1 = pick a new one.
fn wander(w: &mut World, id: MobyId) -> i32 {
    let tgt = c::pv4(w, id, pv::TARGET);
    let d = c::sub(tgt, w.m(id).position);
    let h = c::add_rot(c::atan(d[0], d[1]), c::pf(w, id, pv::YAW_OFS));
    let (rate, top) = (c::pf(w, id, pv::YAW_RATE), c::pf(w, id, pv::YAW_MAX));
    let mut yaw = w.m(id).rotation[2];
    let mut yv = c::pf(w, id, pv::YAW_V);
    turn::turn_toward(h, rate, rate, top, &mut yaw, &mut yv);
    c::set_pf(w, id, pv::YAW_V, yv);
    let (zr, zm) = (c::pf(w, id, pv::Z_RATE), c::pf(w, id, pv::Z_MAX));
    let mut zv = c::pf(w, id, pv::Z_V);
    {
        let m = w.mm(id);
        m.rotation[2] = yaw;
        m.position[0] += yaw.cos() * SPEED * DT;
        m.position[1] += yaw.sin() * SPEED * DT;
        let mut z = m.position[2];
        turn::spring(tgt[2], zr, zr, zm, &mut z, &mut zv);
        m.position[2] = z;
        m.rotation[1] = -4.0 * zv;
    }
    c::set_pf(w, id, pv::Z_V, zv);
    if c::len3(d) < 2.0 || timer(w, id) != 0 { return 1; }
    if let Some(b) = boss(w, id) {
        let bp = w.m(b).position;
        let e = c::sub(w.m(id).position, bp);
        if 4.5 <= c::len2(e) { return 0; }
        let r = w.rng.randf(15.0, 20.0);
        let e = c::add(c::set_len2(e, r), bp);
        let t = c::pv4(w, id, pv::TARGET);
        c::set_pv4(w, id, pv::TARGET, [e[0], e[1], t[2], e[3]]);
    }
    0
}

/// 0x2f02e8: a new wander target about home and the timer `ticks(180)`.
fn new_target(w: &mut World, id: MobyId) {
    let r = w.rng.randf(5.0, 15.0);
    let a = w.rng.rand_angle();
    let z = w.rng.randf(4.0, 16.0);
    let home = c::pv4(w, id, pv::HOME);
    c::set_pv4(w, id, pv::TARGET, [home[0] + a.cos() * r, home[1] + a.sin() * r, home[2] + z, home[3]]);
    let t = w.ticks(0xb4);
    c::set_pi32(w, id, pv::TIMER, t);
}

/// 0x2f0848: the trail ring and the group's draw callbacks (module doc).
fn trail(w: &mut World, id: MobyId) {
    let frame = w.counter as u32;
    if frame & 3 == 0 {
        let h = (c::pi16(w, id, pv::HEAD) + 1) & 0xf;
        c::set_pi16(w, id, pv::HEAD, h);
        let n = c::pi16(w, id, pv::LEN);
        if n < 4 { c::set_pi16(w, id, pv::LEN, n + 1); }
    }
    let h = c::pi16(w, id, pv::HEAD) as usize;
    let mut p = w.m(id).position;
    p[3] = 1.0;
    c::set_pv4(w, id, pv::RING + 0x10 * h, p);
    if w.svc.units.word(FRAME_WORD) == frame { return; }
    w.svc.units.set_word(FRAME_WORD, frame);
    w.svc.units.set_word(COUNTER_WORD, frame);
    for (f, glow) in [(TRAIL_FN, false), (UPDATE_FN, true)] {
        let Some(i) = super::row(REFERENCE_LEVEL, f) else { continue };
        w.svc.draw_callbacks.register(if glow { Callback::UnitGlow(i) } else { Callback::UnitQuads(i) }, id);
    }
    // The glow callback's phase step (0x2f06c0's tail), here [L].
    for m in group_ids(w, w.m(id).group) {
        let Some(o) = w.table.mobys.get(m) else { continue };
        if !ok(o) || !(2..=4).contains(&o.state) { continue; }
        let step = if o.state == 4 { 720.0 } else { 360.0 } * DEG * DT;
        let a = c::add_rot(sv::pvar::ff(&o.pvars, pv::PHASE), step);
        sv::pvar::set_ff(&mut w.mm(m).pvars, pv::PHASE, a);
    }
}

/// 0x2f0390 for the group of `id`: the trails.
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    let ids = svc.groups.lists.get(usize::try_from(m.group).ok()?).and_then(|l| l.clone())?;
    let f = ((svc.units.word(COUNTER_WORD).wrapping_add(3)) & 3) as f32;
    let colour: [[u32; 4]; 16] = std::array::from_fn(|k| {
        std::array::from_fn(|cn| {
            let t = (((k as i32) - ((cn as i32 >> 1) - 1)) as f32 * 0.25 + f * 0.0625).clamp(0.0, 1.0);
            crate::hud::tween_color(t, TRAIL_RGBA[0], TRAIL_RGBA[1])
        })
    });
    let mut quads = Vec::new();
    for mi in ids {
        let Some(o) = table.mobys.get(mi as usize) else { continue };
        if !ok(o) { continue; }
        let n = sv::pvar::i16(&o.pvars, pv::LEN).max(0) as usize;
        let h = sv::pvar::i16(&o.pvars, pv::HEAD) as i32;
        for (k, rgba) in colour.iter().enumerate().take(n.min(16)) {
            let corners = std::array::from_fn(|cn| {
                let j = (((h - k as i32 + 0xf) & 0xf) + (cn as i32 >> 1)) & 0xf;
                let p = sv::pvar::v4f(&o.pvars, pv::RING + 0x10 * j as usize);
                let dz = if cn & 1 == 0 { 0.15 } else { -0.15 };
                [p[0], p[1], p[2] + dz]
            });
            quads.push(FxQuad { corners, st: TRAIL_ST, rgba: *rgba });
        }
    }
    Some(FxQuads { fx: TRAIL_FX, additive: true, subtract: false, quads })
}

/// 0x2f06c0 for the group of `id`: the glows.
pub fn glow_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Vec<GlowQuad> {
    let Some(m) = table.mobys.get(id) else { return Vec::new() };
    let Some(ids) = usize::try_from(m.group).ok().and_then(|g| svc.groups.lists.get(g)).and_then(|l| l.clone()) else { return Vec::new() };
    let mut out = Vec::new();
    for mi in ids {
        let Some(o) = table.mobys.get(mi as usize) else { continue };
        if !ok(o) || !(2..=4).contains(&o.state) { continue; }
        let ph = sv::pvar::ff(&o.pvars, pv::PHASE);
        let rgba = crate::hud::tween_color(ph.sin() * 0.5 + 0.5, GLOW_RGBA[0], GLOW_RGBA[1]);
        let r = o.rows[0];
        let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        let k = if n == 0.0 { 0.0 } else { -0.1 / n };
        out.push(GlowQuad { size: GLOW_SIZE, pull: 0.0, point: [o.position[0] + r[0] * k, o.position[1] + r[1] * k, o.position[2] + r[2] * k], rgba });
    }
    out
}
