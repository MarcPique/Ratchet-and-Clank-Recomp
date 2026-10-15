//! **The ships' fighters: Pokitaru's 1319** (level11 `0x319838`, census U389, 52 instances) **and the fleet's 1843**
//! (level17 `0x2f40d8`, census U576, 60 instances; the same code but for the pickups they drop (the floating 224 /
//! 228, [`super::ship_pickup_float`]), the help record they leave alone and the launch's +0x98; their escorts are placed
//! by the fleet's turrets 347, [`super::fleet_turret`], which also launch them). Pokitaru's: the escorts of the convoys
//! 1264 ([`super::pokitaru_convoy`] spaces them along its path) and the ambushers the jet 1242 calls in
//! ([`super::pokitaru_jet`]). An escort flies its path (forward or back, at a random rate) on a random orbit around
//! it; shot down it bursts, drops a ship pickup ([`super::ship_pickup`]) and waits hidden until it is off screen
//! and away from the jet for a second, then flies again. An ambusher waits hidden until the jet launches it
//! (`0x31a7d8`) into one of four slots around the view, closes in, then turns on the jet and fires the laser
//! shot 1017 ([`super::fighter_shot`]) every 10 ticks, leading its motion; shot down or touching anything it
//! bursts and drops a pickup; off screen it retires. Every fighter leaves two wingtip contrails (the draw
//! callback `0x3192c8`). Read from the level11 decomp and disassembly; the level data (the paths, the gun
//! offsets) from the overlay (gp 0x166c00). Native `f32`.
//!
//! **System or not.** Per-class code (a cluster of one), on the shared pieces: the paths (`Services::splines`), the
//! vehicle record ([`crate::vehicle`]), the shots, the creature death burst (`fx::death_explosion`), the frustum test
//! ([`crate::particles::BSphereView`]), the draw-callback quads (`Callback::UnitQuads`).
//!
//! **Pvar block** (0x2cc; the slot / path words come with the instance or from the convoy): +0x60 the path, +0x64 its
//! point, +0x68 the way to the next (0..1), +0x6c the path rate (an attack run: the approach speed), +0x70 / +0x74 /
//! +0x78 the orbit's angle, rate (never applied) and radius, +0x80 the approach speed on the path, +0x84 the hidden
//! timer, +0x88 the ambush slot (0: an escort), +0x8c / +0x90 the yaw / pitch turn rates, +0x94 the aim's lead, +0x98
//! the approach timer, +0xb0 the jet's position last tick, +0xc0 / +0x1c0 the two contrails' 16 points, +0x2c0 their
//! length, +0x2c4 their head, +0x2c8 the shared frame stamp (shared data), +0x2cc (the port) the tick of the
//! contrails' registration.
//!
//! ## Coverage (`0x319838`)
//! | address | what | port |
//! |---|---|---|
//! | entry | the hits (`0x31a1c0`) | [`hits`] |
//! | 0 | distances 0xff; an escort (+0x88 = 0) → 1, +0x6c = `randf_sym(0.3, 0.5)`, +0x70 = a random angle, +0x74 = `randf_sym(60, 120)`·π/180·dt, +0x78 = `randf(8, 12)`; an ambusher → 4, hidden, no collision | [`update`] |
//! | 1, 2 | the contrails (`0x319670`); +0x68 += +0x6c, past 1 / below 0 the point index steps forward / back (mod the path's count); `fast_add_rotations(+0x70, +0x74)` (result unused); the path step (`0x31a438`) | [`update`], [`trails`], [`path_step`] |
//! | 2 | shot down: `0x273f50(2, 13, m, position, −1)`, a pickup (`0x30f5a8(position, 0, randi(2) = 0 ? 1218 : 1220)`), `PlayClassSound(1, 0)`, help record 0x56's count = 0xffff, → 3, hidden, no collision | [`shot_down`] (`fx::death_explosion`, `ship_pickup::spawn`) |
//! | 1 | not drawn and within 32 of Ratchet → 3, hidden, no collision | [`update`] |
//! | 3 | the sphere (position, 3) in view (`FastBSphereCheck(512)` ≠ −1) or within 32 of Ratchet → +0x84 = 0; else +0x84 + 1 past `ticks(60)` → 1, sequence 0 (blend `ticks(10)`), shown, the class collision | [`update`] |
//! | 5 | the view rows from (0, the camera pitch clamped to ±15°, its yaw); the jet's motion (0x13f3d0 − +0xb0) added to the position, +0xb0 = it; the slot point = the camera + row 1·sin(45° + slot·90°)·8 + row 2·cos(…)·8 + row 0·20 | [`attack`] |
//! | 5 | the approach speed `0x270830(\|slot − position\|, 2·dt², 20·dt², 5·dt` (closing in: 15·dt; at the jet's edge, 0x14095f & 2: dt) `, 0, +0x6c)`; position += toward the slot at it | [`attack`] (`turn::spring`) |
//! | 5 | closing in (+0xbc): drawn → +0x98 + 1; within 2 of the slot or +0x98 past `ticks(120)` → +0xbc = 0; done | [`attack`] |
//! | 5 | else turn to Ratchet (`0x270cc0(·, 4π·dt², 4π·dt², 4π·dt)` on rot.z / rot.y); every 10th tick: `PlayClassSound(2, 0)`, +0x94 → 0 by 0.25, the aim = Ratchet + his motion at +0x94; the gun = the rows · (0x1f14b0 every 20th tick, else 0x1f14c0) + position; `0x308f48(128, −1, 10, m, unit(aim − gun)·32·dt + motion, gun)`, the shot's Euler along it (without the motion) | [`attack`] ([`super::fighter_shot::spawn`]) |
//! | 5 | not drawn → 4, hidden, no collision | [`attack`] |
//! | 6 | help record 0x56's count = 0xffff; the jet's motion added, +0xb0; the burst, a pickup, `PlayClassSound(1, 0)`; → 4, hidden, no collision | [`shot_down`] |
//!
//! ## Coverage (the helpers)
//! | address | what | port |
//! |---|---|---|
//! | 0x31a1c0 | `MobyGetHitMessage(m, 0x330000, 0)`; +0xa4 = 0xff; targetable (0x1000) only while Ratchet is in 0x32 | [`hits`] |
//! | 0x31a1c0 | no hit in state 5: Ratchet not in 0x32 → 4, hidden, no collision; `coll_sphere(1, position, 0, m)` → 6, sequence 1 (blend `ticks(10)`) | [`hits`] |
//! | 0x31a1c0 | a hit: the record bump of 0x157150 (no reader found: NOT kept [L]); on level 11 a hit by the Visibomb (172): 0x15ee00 + 1, past 2 → skill point 0x13d41a; → 2 (an escort) / 6 (an ambusher) when not already, sequence 1 (blend `ticks(10)`) | [`hits`] (`story::award_skill_point`; 0x15ee00: a unit word [L: not in the save block the port keeps]) |
//! | 0x31a438 | the path's points i, i+1, i+2; the Euler (0, pitch, yaw) between the two segments' (pointing back along the path) by +0x68; forward (+0x6c > 0) → yaw + π, −pitch; the target = lerp(p_i, p_i+1) + row 1·r·sin(+0x70) + row 2·r·cos(+0x70) (the unflipped rows); `0x270830(\|target − position\|, 10·dt², 10·dt², 20·dt, 0, +0x80)`; position += toward the target at +0x80 | [`path_step`] |
//! | 0x319670 | every 4th tick the head + 1 (mod 16), the length + 1 up to 5 (16 with cheat 2: 0x15edb2); the points rows · (−1.2, ±0.7, 0.3) + position (w = 1); once a frame (the stamp) `RegisterDrawCallback(0x3192c8)` | [`trails`] (the stamp: a unit word per shared-data offset; the length cap: computed where the draw sets it [L: one frame earlier when the cheat changes]) |
//! | 0x3192c8 | the group's (+0x21) drawn fighters: each contrail point pair a quad 0.25 above / below each point, FX 0x13, ALPHA 0x48, ST table 0x1f1490, colours `FastTweenColor(clamp((e + 1 − k/2)/n + ((tick + 3) & 3)/4n, 0, 1), 0x8c28aa28, 0x144646)` (e the segment, k the corner) | [`trail_quads`] (`Callback::UnitQuads`) |
//! | 0x31a758 | the group's fighters in state 5 | [`attacking`] |
//! | 0x31a7d8 | the group's fighters in state 4, each `randi(10)` = 0 launched: the view rows (camera pitch clamped to ±10°); position = the camera + row 1·sin(45° + slot·90°)·12 + row 2·cos(…)·12 + row 0·6; +0xb0 = Ratchet; +0xbc = 1, → 5, +0x98 = 0, +0x94 = 5, +0x6c = 20·dt; sequence 0 (blend `ticks(10)`), shown, the class collision | [`launch`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::turn::{spring, turn_toward};
use crate::moby_update::creature::{self as c, add, add_rot, atan, dist2, dist3, fx, set_len3, sub, sub_rot, DT, DT2};
use crate::moby_update::services::{fl, pf as to_pf, pv, World};
use crate::moby_update::{scheduler, story};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x31_9838;
pub const TRAIL_FN: u32 = 0x31_92c8;
pub const CLASS: i16 = 0x527;
pub const CLASSES: [i16; 1] = [CLASS];
/// The fleet's copy (module doc).
pub const FLEET_LEVEL: u32 = 17;
pub const FLEET_UPDATE_FN: u32 = 0x2f_40d8;
pub const FLEET_TRAIL_FN: u32 = 0x2f_3b68;
pub const FLEET_CLASS: i16 = 0x733;
pub const FLEET_CLASSES: [i16; 1] = [FLEET_CLASS];

pub mod pvo {
    pub const PATH: usize = 0x60;
    pub const POINT: usize = 0x64;
    pub const T: usize = 0x68;
    pub const RATE: usize = 0x6c;
    pub const ORBIT: usize = 0x70;
    pub const ORBIT_V: usize = 0x74;
    pub const ORBIT_R: usize = 0x78;
    pub const APPROACH: usize = 0x80;
    pub const HIDDEN_T: usize = 0x84;
    pub const SLOT: usize = 0x88;
    pub const YAW_V: usize = 0x8c;
    pub const PITCH_V: usize = 0x90;
    pub const LEAD: usize = 0x94;
    pub const CLOSE_T: usize = 0x98;
    pub const JET: usize = 0xb0;
    pub const TRAIL_A: usize = 0xc0;
    pub const TRAIL_B: usize = 0x1c0;
    pub const TRAIL_N: usize = 0x2c0;
    pub const TRAIL_HEAD: usize = 0x2c4;
    pub const STAMP: usize = 0x2c8;
    pub const DRAW_TICK: usize = 0x2cc;
    pub const LEN: usize = 0x2d0;
}

/// 0x1f14b0 / 0x1f14c0 (level 17: 0x1de110 / 0x1de120, the same): the two gun offsets (model space).
const GUNS: [[f32; 3]; 2] = [[0.3, 0.4, -1.15], [0.3, -0.4, -1.15]];
/// The fleet's 1843 (else Pokitaru's 1319).
fn fleet(w: &World, id: MobyId) -> bool { w.m(id).o_class == FLEET_CLASS }

/// gp−0x4830 (level 17: gp−0x4818): the shot's speed (32·dt).
const SHOT_SPEED: f32 = 32.0;
/// gp−0x4810 (0x1623f0): the contrails' length (5; 16 with cheat 2).
const TRAIL_LEN: f32 = 5.0;
const TRAIL_LEN_CHEAT: f32 = 16.0;
/// 0x1f1490: the contrail quads' ST (corner order).
const TRAIL_ST: [[f32; 2]; 4] = [[0.0, 0.5], [1.0, 0.5], [0.0, 0.5], [1.0, 0.5]];
const TRAIL_A_RGBA: u32 = 0x8c28_aa28;
const TRAIL_B_RGBA: u32 = 0x0014_4646;
/// The unit words: the shared frame stamps (by shared-data offset) and the Visibomb count 0x15ee00.
const STAMP_BASE: u32 = 0x5f00_0000;
const VISIBOMB_HITS: u32 = 0x15_ee00;
/// Skill point 0x13d41a: three fighters downed with the Visibomb.
const SKILL: usize = 0x12;
/// The Visibomb's class.
const VISIBOMB: i16 = 0xac;
/// Help record 0x56 (0x141c18): the fighters' hint (the jet's HUD asks for it while its count is 0).
pub const HELP_REC: usize = 0x56;

fn pf(w: &World, id: MobyId, o: usize) -> f32 { c::pf(w, id, o) }
fn set(w: &mut World, id: MobyId, o: usize, v: f32) { c::set_pf(w, id, o, v) }
fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn seti(w: &mut World, id: MobyId, o: usize, v: i32) { c::set_pi32(w, id, o, v) }

/// Path `i` of the level (empty for −1 / missing).
fn path(w: &World, i: i32) -> Vec<[f32; 4]> {
    usize::try_from(i).ok().and_then(|i| w.svc.splines.get(i)).map_or(Vec::new(), |p| p.iter().map(|q| q.map(f32::from_bits)).collect())
}

/// The rows (forward, left, up) of Euler `e` (`fun_001fa030` / `fun_001fa050`).
pub fn rows_of(e: [f32; 3]) -> [[f32; 3]; 3] {
    let r = crate::moby_update::services::euler_rows([to_pf(e[0]), to_pf(e[1]), to_pf(e[2]), Pf::ZERO]);
    std::array::from_fn(|k| [fl(r[k][0]), fl(r[k][1]), fl(r[k][2])])
}

fn along(v: [f32; 3], k: f32) -> [f32; 4] { [v[0] * k, v[1] * k, v[2] * k, 0.0] }

fn hide(w: &mut World, id: MobyId, state: u8) {
    let m = w.mm(id);
    m.state = state;
    m.has_collision = false;
    m.mode |= 1;
}

fn show(w: &mut World, id: MobyId) {
    if w.m(id).anim.seq_b != 0 {
        let t = w.ticks(10);
        w.anim_blend(id, 0, 0, t);
    }
    let coll = super::class_collision(w, w.m(id).o_class);
    let m = w.mm(id);
    m.mode &= !1;
    m.has_collision = coll;
}

/// The view rows of the camera Euler with its pitch clamped to ±`lim`.
fn view_rows(w: &World, lim: f32) -> [[f32; 3]; 3] {
    let e = w.hero.loop_in.cam_euler;
    rows_of([0.0, e[1].clamp(-lim, lim), e[2]])
}

/// A slot point around the view: the camera + row 1·sin(a)·r + row 2·cos(a)·r + row 0·ahead.
fn slot_point(w: &World, rows: &[[f32; 3]; 3], slot: i32, r: f32, ahead: f32) -> [f32; 4] {
    let a = add_rot(std::f32::consts::FRAC_PI_4, slot as f32 * std::f32::consts::FRAC_PI_2);
    let cam = w.camera_point();
    let p = add([cam[0], cam[1], cam[2], 0.0], along(rows[1], a.sin() * r));
    let p = add(p, along(rows[2], a.cos() * r));
    add(p, along(rows[0], ahead))
}

/// Level11 `0x319838` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pvo::LEN);
    hits(w, id);
    match w.m(id).state {
        0 => {
            {
                let m = w.mm(id);
                m.update_dist = 0xff;
                m.draw_dist = 0xff;
            }
            if pi(w, id, pvo::SLOT) == 0 {
                w.mm(id).state = 1;
                let r = w.rng.randf_sym(0.3, 0.5);
                set(w, id, pvo::RATE, r);
                let a = w.rng.rand_angle();
                set(w, id, pvo::ORBIT, a);
                let v = w.rng.randf_sym(60.0, 120.0);
                set(w, id, pvo::ORBIT_V, v * 0.017_453_292 * DT);
                let r = w.rng.randf(8.0, 12.0);
                set(w, id, pvo::ORBIT_R, r);
                return;
            }
            hide(w, id, 4);
        }
        1 | 2 => {
            trails(w, id);
            let n = path(w, pi(w, id, pvo::PATH)).len() as i32;
            let t = pf(w, id, pvo::T) + pf(w, id, pvo::RATE);
            set(w, id, pvo::T, t);
            if 1.0 < t {
                set(w, id, pvo::T, t - 1.0);
                if n != 0 { seti(w, id, pvo::POINT, (pi(w, id, pvo::POINT) + 1) % n); }
            } else if t < 0.0 {
                set(w, id, pvo::T, t + 1.0);
                if n != 0 { seti(w, id, pvo::POINT, (pi(w, id, pvo::POINT) + n - 1) % n); }
            }
            path_step(w, id);
            if w.m(id).state == 2 {
                shot_down(w, id, 3);
                return;
            }
            if w.m(id).visible != 0 { return; }
            if dist3(w.m(id).position, super::hero_pos(w)) < 32.0 { hide(w, id, 3); }
        }
        3 => {
            let p = w.m(id).position;
            let in_view = w.view.is_some_and(|v| !v.culled(512.0, [p[0], p[1], p[2], 3.0])) || w.view.is_none();
            if in_view || dist3(p, super::hero_pos(w)) <= 32.0 {
                seti(w, id, pvo::HIDDEN_T, 0);
                return;
            }
            let n = pi(w, id, pvo::HIDDEN_T) + 1;
            seti(w, id, pvo::HIDDEN_T, n);
            if n <= w.ticks(0x3c) { return; }
            w.mm(id).state = 1;
            show(w, id);
        }
        5 => attack(w, id),
        6 => {
            shot_down(w, id, 4);
        }
        _ => {}
    }
}

/// The jet's motion since last tick added to the position (+0xb0 kept).
fn follow_jet(w: &mut World, id: MobyId) -> [f32; 4] {
    let h = w.hero_point();
    let h = [h[0], h[1], h[2], 0.0];
    let d = sub(h, c::pv4(w, id, pvo::JET));
    c::set_pv4(w, id, pvo::JET, h);
    let p = add(w.m(id).position, d);
    w.mm(id).position = p;
    d
}

/// States 2 / 6: the burst, the pickup, the sound; → `next`, hidden (module doc).
fn shot_down(w: &mut World, id: MobyId, next: u8) {
    let fleet = fleet(w, id);
    if next == 4 {
        if !fleet { w.svc.help.records.help[HELP_REC].count = 0xffff; }
        follow_jet(w, id);
    }
    let p = w.m(id).position;
    fx::death_explosion(w, 2.0, 13.0, Some(id), p, -1);
    let first = w.rng.randi(2) == 0;
    if fleet {
        let kind = if first { super::ship_pickup_float::HEALTH } else { super::ship_pickup_float::MISSILES };
        super::ship_pickup_float::spawn(w, p, kind);
    } else {
        let kind = if first { super::ship_pickup::MISSILES } else { super::ship_pickup::HEALTH };
        super::ship_pickup::spawn(w, p, [0.0; 4], kind);
    }
    w.play_sound(1, 0, id);
    if next == 3 && !fleet { w.svc.help.records.help[HELP_REC].count = 0xffff; }
    hide(w, id, next);
}

/// State 5: the attack run (module doc).
fn attack(w: &mut World, id: MobyId) {
    let rows = view_rows(w, f32::from_bits(0x3e86_0a92));
    let motion = follow_jet(w, id);
    let spot = slot_point(w, &rows, pi(w, id, pvo::SLOT), 8.0, 20.0);
    let pos = w.m(id).position;
    let to = sub(spot, pos);
    let l = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
    let closing = w.m(id).cmd != 0;
    let (mut x, mut v) = (0.0f32, pf(w, id, pvo::RATE));
    if w.svc.vehicle.quit & 2 == 0 {
        if !closing { spring(l, DT2 * 2.0, DT2 * 20.0, DT * 5.0, &mut x, &mut v); } else { spring(l, DT2 * 2.0, DT2 * 20.0, DT * 15.0, &mut x, &mut v); }
    } else {
        spring(l, DT2 * 2.0, DT2 * 20.0, DT, &mut x, &mut v);
    }
    set(w, id, pvo::RATE, v);
    let p = add(pos, set_len3(to, v));
    w.mm(id).position = p;
    if closing {
        if w.m(id).visible != 0 { seti(w, id, pvo::CLOSE_T, pi(w, id, pvo::CLOSE_T) + 1); }
        if dist3(spot, w.m(id).position) < 2.0 || w.ticks(0x78) < pi(w, id, pvo::CLOSE_T) { w.mm(id).cmd = 0; }
        return;
    }
    let h = w.hero_point();
    let h4 = [h[0], h[1], h[2], 0.0];
    let me = w.m(id).position;
    let aim = sub(h4, me);
    let k = DT2 * 12.566_371;
    let (mut a, mut va) = (w.m(id).rotation[2], pf(w, id, pvo::YAW_V));
    turn_toward(atan(aim[0], aim[1]), k, k, DT * 12.566_371, &mut a, &mut va);
    w.mm(id).rotation[2] = a;
    set(w, id, pvo::YAW_V, va);
    let xy = (aim[0] * aim[0] + aim[1] * aim[1]).sqrt();
    let (mut b, mut vb) = (w.m(id).rotation[1], pf(w, id, pvo::PITCH_V));
    turn_toward(-atan(xy, aim[2]), k, k, DT * 12.566_371, &mut b, &mut vb);
    w.mm(id).rotation[1] = b;
    set(w, id, pvo::PITCH_V, vb);
    if w.counter.is_multiple_of(10) {
        w.play_sound(2, 0, id);
        let mut lead = pf(w, id, pvo::LEAD);
        c::turn::approach(0.0, 0.25, &mut lead);
        set(w, id, pvo::LEAD, lead);
        let target = add(set_len3(motion, lead), h4);
        let g = if w.counter.is_multiple_of(20) { GUNS[0] } else { GUNS[1] };
        let r = w.m(id).rows;
        let gun: [f32; 4] = std::array::from_fn(|k| if k < 3 { r[0][k] * g[0] + r[1][k] * g[1] + r[2][k] * g[2] + me[k] } else { 0.0 });
        let dir = add(set_len3(sub(target, gun), SHOT_SPEED * DT), motion);
        if let Some(s) = super::fighter_shot::spawn(w, 128.0, -1.0, 10.0, id, dir, gun) {
            let d = sub(dir, motion);
            let dxy = (d[0] * d[0] + d[1] * d[1]).sqrt();
            let m = w.mm(s);
            m.rotation[1] = -atan(dxy, d[2]);
            m.rotation[2] = atan(d[0], d[1]);
        }
    }
    if w.m(id).visible != 0 { return; }
    hide(w, id, 4);
}

/// Level11 `0x31a1c0` (module doc).
fn hits(w: &mut World, id: MobyId) {
    let hit = w.get_hit(id, 0x33_0000, false);
    w.mm(id).hit_slot = 0xff;
    let flying = w.hero.state == 0x32;
    if flying { w.mm(id).mode |= mode::TARGETABLE } else { w.mm(id).mode &= !mode::TARGETABLE }
    let Some(h) = hit else {
        if w.m(id).state != 5 { return; }
        if !flying { hide(w, id, 4); }
        let p = pv(w.m(id).position);
        if w.coll_sphere(p, Pf::ONE, 0, Some(id)).is_none() { return; }
        w.mm(id).state = 6;
        if w.m(id).anim.seq_b != 1 {
            let t = w.ticks(10);
            w.anim_blend(id, 1, 0, t);
        }
        return;
    };
    if w.svc.level == 11 && h.attacker.is_some_and(|a| w.m(a).o_class == VISIBOMB) {
        let n = w.svc.units.word(VISIBOMB_HITS).wrapping_add(1);
        w.svc.units.set_word(VISIBOMB_HITS, n);
        if 2 < n as i32 { story::award_skill_point(w, SKILL); }
    }
    let next = if pi(w, id, pvo::SLOT) == 0 { 2 } else { 6 };
    if w.m(id).state != next {
        w.mm(id).state = next;
        if w.m(id).anim.seq_b != 1 {
            let t = w.ticks(10);
            w.anim_blend(id, 1, 0, t);
        }
    }
}

/// Level11 `0x31a438`: the step along the path (module doc).
fn path_step(w: &mut World, id: MobyId) {
    let pts = path(w, pi(w, id, pvo::PATH));
    let n = pts.len();
    if n == 0 { return; }
    let i = (pi(w, id, pvo::POINT).max(0) as usize) % n;
    let (p0, p1, p2) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
    let t = pf(w, id, pvo::T);
    let y01 = atan(p0[0] - p1[0], p0[1] - p1[1]);
    let y12 = atan(p1[0] - p2[0], p1[1] - p2[1]);
    let e01 = atan(dist2(p0, p1), p1[2] - p0[2]);
    let e12 = atan(dist2(p1, p2), p2[2] - p1[2]);
    let e = [0.0, add_rot(sub_rot(e12, e01) * t, e01), add_rot(sub_rot(y12, y01) * t, y01)];
    {
        let m = w.mm(id);
        m.rotation[0] = e[0];
        m.rotation[1] = e[1];
        m.rotation[2] = e[2];
    }
    if 0.0 < pf(w, id, pvo::RATE) {
        let m = w.mm(id);
        m.rotation[2] = add_rot(m.rotation[2], f32::from_bits(0x4049_0fd0));
        m.rotation[1] = -m.rotation[1];
    }
    let base: [f32; 4] = std::array::from_fn(|k| p0[k] + (p1[k] - p0[k]) * t);
    let rows = rows_of(e);
    let (a, r) = (pf(w, id, pvo::ORBIT), pf(w, id, pvo::ORBIT_R));
    let target = add(add(base, along(rows[1], r * a.sin())), along(rows[2], r * a.cos()));
    let pos = w.m(id).position;
    let (mut x, mut v) = (0.0f32, pf(w, id, pvo::APPROACH));
    spring(dist3(pos, target), DT2 * 10.0, DT2 * 10.0, DT * 20.0, &mut x, &mut v);
    set(w, id, pvo::APPROACH, v);
    let p = add(pos, set_len3(sub(target, pos), v));
    w.mm(id).position = p;
}

/// Level11 `0x319670`: the contrails' points and the draw registration (module doc).
fn trails(w: &mut World, id: MobyId) {
    if w.counter & 3 == 0 {
        let h = (pi(w, id, pvo::TRAIL_HEAD) + 1).rem_euclid(16);
        seti(w, id, pvo::TRAIL_HEAD, h);
        let cap = if w.svc.cheats.on(2) { TRAIL_LEN_CHEAT } else { TRAIL_LEN };
        if (pi(w, id, pvo::TRAIL_N) as f32) < cap { seti(w, id, pvo::TRAIL_N, pi(w, id, pvo::TRAIL_N) + 1); }
    }
    let h = pi(w, id, pvo::TRAIL_HEAD).rem_euclid(16) as usize;
    let r = w.m(id).rows;
    let p = w.m(id).position;
    for (base, side) in [(pvo::TRAIL_A, 0.7f32), (pvo::TRAIL_B, -0.7)] {
        let o = [-1.2f32, side, 0.3];
        let q: [f32; 4] = std::array::from_fn(|k| if k < 3 { r[0][k] * o[0] + r[1][k] * o[1] + r[2][k] * o[2] + p[k] } else { 1.0 });
        c::set_pv4(w, id, base + h * 0x10, q);
    }
    let key = STAMP_BASE.wrapping_add(pi(w, id, pvo::STAMP) as u32);
    let tick = w.counter as u32;
    if tick != 0 && w.svc.units.word(key) != tick {
        w.svc.units.set_word(key, tick);
        seti(w, id, pvo::DRAW_TICK, tick as i32);
        let row = if fleet(w, id) { super::row(FLEET_LEVEL, FLEET_TRAIL_FN) } else { super::row(REFERENCE_LEVEL, TRAIL_FN) };
        if let Some(i) = row { w.svc.draw_callbacks.register(Callback::UnitQuads(i), id); }
    }
}

/// Level11 `0x3192c8`: the contrails of the registering fighter's group (module doc).
pub fn trail_quads(table: &crate::moby_runtime::MobyTable, svc: &crate::moby_update::Services, id: MobyId) -> Option<super::FxQuads> {
    let me = table.mobys.get(id).filter(|m| (m.o_class == CLASS || m.o_class == FLEET_CLASS) && m.pvars.len() >= pvo::LEN)?;
    if me.group < 0 { return None; }
    let n = if svc.cheats.on(2) { TRAIL_LEN_CHEAT } else { TRAIL_LEN };
    let tick = crate::moby_update::services::pvar::i32(&me.pvars, pvo::DRAW_TICK) as u32;
    let phase = (tick.wrapping_add(3) & 3) as f32;
    let colour = |e: usize, k: usize| {
        let f = ((e as i32 - ((k as i32 >> 1) - 1)) as f32 / n + phase / (n * 4.0)).clamp(0.0, 1.0);
        crate::particles::tween_color(f.to_bits(), TRAIL_A_RGBA, TRAIL_B_RGBA)
    };
    let list = svc.groups.lists.get(me.group as usize).and_then(|l| l.as_ref())?;
    let mut quads = Vec::new();
    for &e in list {
        let Some(m) = table.mobys.get((e & 0x7fff) as usize) else { continue };
        if m.visible == 0 || m.o_class != me.o_class || m.pvars.len() < pvo::LEN { continue; }
        let pv4 = |o: usize| crate::moby_update::services::pvar::v4f(&m.pvars, o);
        let count = crate::moby_update::services::pvar::i32(&m.pvars, pvo::TRAIL_N);
        let head = crate::moby_update::services::pvar::i32(&m.pvars, pvo::TRAIL_HEAD);
        for s in 0..count.max(0) {
            let i0 = (head - s + 15).rem_euclid(16);
            for base in [pvo::TRAIL_A, pvo::TRAIL_B] {
                let corners: [[f32; 3]; 4] = std::array::from_fn(|k| {
                    let j = (i0 + (k as i32 >> 1)).rem_euclid(16) as usize;
                    let p = pv4(base + j * 0x10);
                    let dz = if k & 1 == 0 { 0.25 } else { -0.25 };
                    [p[0], p[1], p[2] + dz]
                });
                quads.push(super::FxQuad { corners, st: TRAIL_ST, rgba: std::array::from_fn(|k| colour(s as usize, k)) });
            }
        }
    }
    Some(super::FxQuads { fx: 0x13, additive: true, subtract: false, quads })
}

/// Level11 `0x31a758(g)`: the group's fighters (`class`) on an attack run.
pub fn attacking(w: &World, g: i32, class: i16) -> i32 {
    let Ok(g) = i8::try_from(g) else { return 0 };
    scheduler::group_ids(w, g).into_iter().filter(|&m| m < w.table.mobys.len() && w.m(m).o_class == class && w.m(m).state == 5).count() as i32
}

/// Level11 `0x31a7d8(g)` / level17 `0x2f4fb8(g)`: one waiting fighter (`class`) of the group launched at random
/// (module doc; the fleet's copy leaves +0x98).
pub fn launch(w: &mut World, g: i32, class: i16) -> Option<MobyId> {
    let g = i8::try_from(g).ok()?;
    for m in scheduler::group_ids(w, g) {
        if m >= w.table.mobys.len() || w.m(m).o_class != class || w.m(m).state != 4 { continue; }
        if w.rng.randi(10) != 0 { continue; }
        story::pvars(w, m, pvo::LEN);
        let rows = view_rows(w, 0.174_532_92);
        let p = slot_point(w, &rows, pi(w, m, pvo::SLOT), 12.0, 6.0);
        let h = w.hero_point();
        c::set_pv4(w, m, pvo::JET, [h[0], h[1], h[2], 0.0]);
        {
            let mo = w.mm(m);
            mo.position = [p[0], p[1], p[2], mo.position[3]];
            mo.cmd = 1;
            mo.state = 5;
        }
        set(w, m, pvo::RATE, DT * 20.0);
        if class != FLEET_CLASS { seti(w, m, pvo::CLOSE_T, 0); }
        set(w, m, pvo::LEAD, 5.0);
        show(w, m);
        return Some(m);
    }
    None
}
