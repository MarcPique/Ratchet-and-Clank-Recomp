//! Classes 572 / 865 / 866, the amoeboids (`AmoeboidUpdate` 0x2edca0 with its pre-update `0x2eec68`, the split
//! `0x2ef350`, the goo `0x2ef560` / `0x2ef770`; level01, the same function on levels 5 and 11). A thin state machine
//! over the shared creature layer; spec `docs/plan/creatures.md` "Amoeboids".
//!
//! **Sizes.** 572 (big, 1.0), 865 (medium, 0.7), 866 (small, 0.49): +0x250 and the walker radius. Only the big ones
//! (pvar+0x230 = 1) start active (state 1); the smaller ones wait hidden 20 units down in state 0xc (0xd with a trigger
//! cuboid +0x25c) as the **split children** of their group: a hit that leaves health (3 at start, −1 per hit) wakes
//! the next size's two waiting members (`0x2ef350`: position, rotation, health and range copied), throws them ±45°
//! off the push (`15·dt` out, `5·dt` up, state 8), drops the parent's bolts (`SetDeathBits`) and deletes the parent.
//! The last hit (health 0) only bursts it into goo (`0x2ef770`: 20 × 10 type-2 blobs) and deletes it.
//!
//! **Behaviour.** Keep to the arena (path +0x240; its walls, [`region`]) and move through the waypoint graph (path
//! +0x244) towards Ratchet while he is inside the arena polygon (path +0x24c) and within the range (+0x224, +20
//! after an alert) of home: wander (2, 1-in-4 line-of-sight checks), chase (4, `4π` turn rate, ±20° offset, doubled
//! walker steps), strike (6: lunges between key frames 10 and 16, hits Ratchet for 1 with an exact push at key frame
//! 15.5 when within 2 xy and twice its size in height), recover (7 / 3), go home (5), path chase (0xf / 0x10 while
//! alerted). The glow (+0x90) pulses between 0x80008080 and 0x80008040 at `3π` rad/s from a random phase.
//!
//! Pvars (688 bytes; header at 0: damage +0x20, knockback +0x60, extra +0xc0, walker +0x170): +0x1c0 the target record
//! of `0x274df8` (its +0x40 is +0x200 the target moby, here the runtime index + 1; its +0x44 is +0x204 the kind, 2 =
//! none / lost), +0x210 home, +0x220 turn velocity, +0x224
//! range, +0x228 current range, +0x22c alert timer, +0x230 big, +0x234 heading offset, +0x23c speed, +0x240 arena path,
//! +0x244 waypoint graph, +0x248 alternate-target group (class 623 members; −1 on Novalis), +0x24c target polygon, +0x250
//! size, +0x254 glow phase, +0x258 fall-out ripple patch (0 on Novalis), +0x25c trigger cuboid, +0x260 gate moby.
//! The fall-out rule (`0x2eec68`): with +0x258 ≠ 0 (a ripple patch index; Rilgar / level 11) an amoeboid below that
//! patch's height (`water::world::WaterWorld::patch_level`) bursts into goo and is deleted without bolts. The big-head cheat
//! `0x278720(2.9, m, 0, +0x270)` is `manip::big_head`. Not ported (counted): the shadow probe.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, attack, damage, fx, ground, knock, region, target, turn, walker};
use crate::moby_update::services::World;
use std::f32::consts::PI;

pub const UPDATE_FN: u32 = 0x2edca0;
pub const CLASSES: [i16; 3] = [0x23c, 0x361, 0x362];

const D: usize = 0x20;
const ALERT: usize = 0x38;
const STEP_OUT: usize = 0x40;
const K: usize = 0x60;
const J: usize = 0x170;
const J_VMAX: usize = 0x194;
const TGT_REC: usize = 0x1c0;
const TGT: usize = 0x200;
const TGT_STATE: usize = 0x204;
const HOME: usize = 0x210;
const TURN_V: usize = 0x220;
const RANGE0: usize = 0x224;
const RANGE: usize = 0x228;
const ALERT_T: usize = 0x22c;
const BIG: usize = 0x230;
const OFFSET: usize = 0x234;
const SPEED: usize = 0x23c;
const ARENA: usize = 0x240;
const GRAPH: usize = 0x244;
const ALT_GROUP: usize = 0x248;
const POLY: usize = 0x24c;
const SIZE: usize = 0x250;
const GLOW: usize = 0x254;
const FALLOUT: usize = 0x258;
const CUBOID: usize = 0x25c;
const GATE: usize = 0x260;

/// `gp−0x5260` (0x1619a0) 15 / `gp−0x525c` 5: the split children's throw (per second); `gp−0x5254` 2.5 the strike range.
const SPLIT_OUT: f32 = 15.0;
const SPLIT_UP: f32 = 5.0;
const STRIKE: f32 = 2.5;
/// The glow colours `gp−0x5268` / `gp−0x5264`.
const GLOW_A: u32 = 0x8000_8080;
const GLOW_B: u32 = 0x8000_8040;

fn st(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_st(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn seq_b(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
/// `MobyAnimBlend(m, seq, frame, n)` with `n` raw ticks.
fn blend_raw(w: &mut World, id: MobyId, seq: u8, frame: i32, n: i32) { w.anim_blend(id, seq, frame, n); }
fn blend_t(w: &mut World, id: MobyId, seq: u8, frame: i32, n: i32) { let t = w.ticks(n); w.anim_blend(id, seq, frame, t); }

fn size(w: &World, id: MobyId) -> f32 { c::pf(w, id, SIZE) }

fn target(w: &World, id: MobyId) -> Option<MobyId> {
    let t = c::pi32(w, id, TGT);
    (t > 0).then(|| (t - 1) as usize).filter(|&t| t < w.table.mobys.len())
}
fn target_pos(w: &World, id: MobyId) -> c::V { target(w, id).map(|t| w.m(t).position).unwrap_or([0.0; 4]) }

fn paths(w: &World, id: MobyId) -> ([usize; 1], usize) {
    (
        [c::pi32(w, id, ARENA).max(0) as usize],
        c::pi32(w, id, GRAPH).max(0) as usize,
    )
}

/// `LineOfSightTest(r, &+0x240, 1, +0x244, from, to)`.
fn los(w: &World, id: MobyId, r: f32, from: c::V, to: c::V) -> Option<c::V> {
    let (a, g) = paths(w, id);
    region::line_of_sight(w, r, &a, g, from, to)
}

fn walk(w: &mut World, id: MobyId, k: f32) -> u32 {
    let (cy, sy) = c::cs(c::yaw(w, id));
    let mut out = [0.0; 4];
    let r = walker::step(w, id, J, 1.0, [cy * k, sy * k, 0.0, 0.0], &mut out);
    c::set_pv4(w, id, STEP_OUT, out);
    r
}

fn slow_turn(w: &mut World, id: MobyId, h: f32) { turn::turn_toward_pvar(w, id, h, c::DT2 * std::f32::consts::FRAC_PI_3, c::DT2 * std::f32::consts::FRAC_PI_3, c::DT * PI, TURN_V); }
fn fast_turn(w: &mut World, id: MobyId, h: f32) { turn::turn_toward_pvar(w, id, h, c::DT2 * 12.566_371, c::DT2 * 12.566_371, c::DT * 25.132_742, TURN_V); }

fn new_offset(w: &mut World, id: MobyId) {
    c::set_pv4(w, id, STEP_OUT, [0.0; 4]);
    let o = w.rng.randf(f32::from_bits(0xbeb2_b8c2), f32::from_bits(0x3eb2_b8c2));
    c::set_pf(w, id, OFFSET, o);
}

/// `AmoeboidUpdate` 0x2edca0.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x270 { return; }
    if st(w, id) == 0xc {
        w.mm(id).mode |= 1;
        return;
    }
    // 0x278720(2.9, m, 0, +0x270): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, f32::from_bits(0x4039_999a), id, 0, id, 0x270);
    // Drawn and within 28 units of the camera: the shadow probe 0x26f020, +0x7f = 0x16.
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 28.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x16;
        }
    }
    if pre(w, id) { return; }
    let s = size(w, id);
    let pos = c::pos(w, id);
    match st(w, id) {
        0 => init(w, id),
        1 => {
            let mut t = c::pi32(w, id, ALERT_T);
            if c::pi32(w, id, TGT_STATE) != 2 {
                if w.rng.randi(4) == 0 {
                    let tp = target_pos(w, id);
                    if los(w, id, s * 0.8, pos, tp).is_none() {
                        c::set_pi32(w, id, TGT_STATE, 2);
                    } else {
                        set_st(w, id, 4);
                        if seq_b(w, id) != 1 { blend_t(w, id, 1, 0, 10); }
                        new_offset(w, id);
                    }
                }
                t = c::pi32(w, id, ALERT_T);
            }
            if t != 0 && c::pi32(w, id, TGT_STATE) == 2 {
                set_st(w, id, 0xf);
                if seq_b(w, id) != 1 { blend_raw(w, id, 1, 0, 5); }
                return;
            }
            let v = c::pf(w, id, SPEED) * 0.5 * c::DT;
            c::set_pf(w, id, J_VMAX, v);
            if seq_b(w, id) != 1 { blend_raw(w, id, 1, 0, 5); }
            set_st(w, id, 2);
        }
        2 => {
            let home = c::pv4(w, id, HOME);
            slow_turn(w, id, c::atan(home[0] - pos[0], home[1] - pos[1]));
            walk(w, id, 1.0);
            let mut t = c::pi32(w, id, ALERT_T);
            if c::pi32(w, id, TGT_STATE) != 2 {
                if w.rng.randi(4) == 0 {
                    let p = c::pos(w, id);
                    let tp = target_pos(w, id);
                    if los(w, id, s * 0.8, p, tp).is_none() {
                        c::set_pi32(w, id, TGT_STATE, 2);
                    } else {
                        set_st(w, id, 3);
                        if seq_b(w, id) != 0 { blend_t(w, id, 0, 0, 10); }
                        new_offset(w, id);
                        let v = c::pf(w, id, SPEED) * c::DT;
                        c::set_pf(w, id, J_VMAX, v);
                    }
                }
                t = c::pi32(w, id, ALERT_T);
            }
            if t != 0 && c::pi32(w, id, TGT_STATE) == 2 {
                set_st(w, id, 0xf);
                if seq_b(w, id) != 1 { blend_raw(w, id, 1, 0, 5); }
            }
            drip(w, id);
        }
        3 | 7 => {
            if w.m(id).anim.flags & 2 == 0 { return; }
            set_st(w, id, 4);
            if seq_b(w, id) != 1 { blend_t(w, id, 1, 0, 10); }
        }
        4 => {
            let tp = target_pos(w, id);
            if STRIKE <= c::dist2(pos, tp) {
                let a = los(w, id, s * 0.8, pos, tp).or_else(|| los(w, id, 0.0, pos, tp));
                match a {
                    Some(a) => {
                        let h = c::add_rot(c::atan(a[0] - pos[0], a[1] - pos[1]), c::pf(w, id, OFFSET));
                        fast_turn(w, id, h);
                        walk(w, id, 2.0);
                    }
                    None => c::set_pi32(w, id, TGT_STATE, 2),
                }
                if c::pi32(w, id, TGT_STATE) == 2 { set_st(w, id, 5); }
                drip(w, id);
            } else {
                set_st(w, id, 6);
                let v = (c::pf(w, id, SPEED) + c::pf(w, id, SPEED)) * c::DT;
                c::set_pf(w, id, J_VMAX, v);
                if seq_b(w, id) != 2 { blend_t(w, id, 2, 0, 10); }
            }
        }
        5 => {
            let home = c::pv4(w, id, HOME);
            let mut aim = None;
            if c::dist2(pos, home) < 1.0 {
                set_st(w, id, 1);
            } else {
                aim = los(w, id, s * 0.8, pos, home).or_else(|| los(w, id, 0.0, home, home));
            }
            if let Some(a) = aim {
                let h = c::add_rot(c::atan(a[0] - pos[0], a[1] - pos[1]), c::pf(w, id, OFFSET));
                fast_turn(w, id, h);
                walk(w, id, 2.0);
            }
            if c::pi32(w, id, TGT_STATE) != 2 {
                set_st(w, id, 4);
                if seq_b(w, id) != 1 { blend_t(w, id, 1, 0, 3); }
            }
            drip(w, id);
        }
        6 => {
            let t = target(w, id);
            let tp = target_pos(w, id);
            fast_turn(w, id, c::atan(tp[0] - pos[0], tp[1] - pos[1]));
            let k = ground::key_time(w, id);
            let an = w.m(id).anim;
            if an.seq_a == an.seq_b && (10.0..=16.0).contains(&k) { walk(w, id, 2.0); }
            if ground::passed_frame(w, id, 15.5) {
                let p = c::pos(w, id);
                if c::dist2(p, tp) < 2.0 && (tp[2] - p[2]).abs() < s + s {
                    let (cy, sy) = c::cs(c::yaw(w, id));
                    let dir = [cy, sy, 0.0, crate::hero::damage::EXACT_PUSH_W];
                    let at = [tp[0], tp[1], tp[2] + 0.75, tp[3]];
                    if let Some(t) = t { attack::hit_moby(w, t, id, 1.0, 0x1_0001, at, dir); }
                    return;
                }
            }
            if w.m(id).anim.flags & 2 != 0 {
                set_st(w, id, 7);
                if seq_b(w, id) != 3 { blend_t(w, id, 3, 0, 10); }
                let v = c::pf(w, id, SPEED) * c::DT;
                c::set_pf(w, id, J_VMAX, v);
            }
        }
        8 => {
            let r = knock::update(w, id, K);
            if r & knock::res::LANDED != 0 {
                let p = c::pos(w, id);
                let home = c::pv4(w, id, HOME);
                if los(w, id, s * 0.8, p, home).is_some() {
                    w.play_sound(4, 0, id);
                    let v = c::pf(w, id, SPEED) * c::DT;
                    c::set_pf(w, id, J_VMAX, v);
                    set_st(w, id, 9);
                    if seq_b(w, id) != 5 { blend_t(w, id, 5, 4, 10); }
                    c::set_pv4(w, id, HOME, c::pos(w, id));
                    return;
                }
                goo_burst(w, id, [0.0; 4]);
                set_death_bits(w, id, 0, -1);
            } else if r & 0x120 == 0 {
                return;
            }
            w.delete_moby(id);
        }
        9 => {
            if w.m(id).anim.flags & 2 != 0 {
                set_st(w, id, 4);
                if seq_b(w, id) != 1 { blend_t(w, id, 1, 0, 10); }
                new_offset(w, id);
                let v = c::pf(w, id, SPEED) * c::DT;
                c::set_pf(w, id, J_VMAX, v);
            }
        }
        0xd => {
            w.mm(id).mode |= 1;
            let body = w.hero.body_point.map(|x| f32::from_bits(x.0));
            if crate::moby_update::triggers::point_in_cuboid(&w.svc.volumes, [body[0], body[1], body[2]], c::pi32(w, id, CUBOID)) {
                wake(w, id, 20.0);
                set_st(w, id, 1);
            }
        }
        // FUN_00305260: carried by the Suck Cannon (the shared reaction layer); landed after a let-go → state 1, the
        // suck record's state (+0x128) cleared.
        0xe => {
            if c::react::carried(w, id, K) != 0 {
                set_st(w, id, 1);
                c::set_pi16(w, id, 0xc0 + c::react::rec::STATE, 0);
            }
        }
        0xf => {
            let tp = target_pos(w, id);
            slow_turn(w, id, c::atan(tp[0] - pos[0], tp[1] - pos[1]));
            walk(w, id, 1.0);
            let p = c::pos(w, id);
            let (crossed, cp) = region::clamp(w, c::pi32(w, id, ARENA).max(0) as usize, p, tp);
            let at_wall = crossed && c::dist3(p, cp) < 1.5;
            if c::pi32(w, id, ALERT_T) == 0 {
                set_st(w, id, 5);
            } else if at_wall {
                set_st(w, id, 0x10);
                if seq_b(w, id) != 0 { blend_t(w, id, 0, 0, 10); }
            } else if c::dist2(p, tp) < 2.0 {
                set_st(w, id, 6);
                if seq_b(w, id) != 2 { blend_t(w, id, 2, 5, 10); }
            }
        }
        0x10 => {
            if w.m(id).anim.flags & 2 == 0 || c::pi32(w, id, ALERT_T) != 0 { return; }
            set_st(w, id, 5);
            if seq_b(w, id) != 1 { blend_t(w, id, 1, 0, 20); }
        }
        _ => {}
    }
}

/// Unhide a waiting amoeboid: mode −1 | 0x1000, anim speed 1, z + `dz`, drawn, collision on.
fn wake(w: &mut World, id: MobyId, dz: f32) {
    let has = w.classes.info(w.m(id).o_class).map(|i| i.has_collision && !i.no_header).unwrap_or(false);
    let m = w.mm(id);
    m.anim.speed = 1.0;
    m.position[2] += dz;
    m.visible = 1;
    m.has_collision = has;
    m.mode = (m.mode & !1) | mode::TARGETABLE;
}

/// State 0: the init (module doc; 1 draw: the glow phase).
fn init(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    c::set_pv4(w, id, HOME, p);
    let (b58, b5a, sz) = match w.m(id).o_class {
        0x361 => (14.0f32, 5.6f32, 0.7f32),
        0x23c => (14.0, 10.4, 1.0),
        0x362 => (14.0, 4.0, 0.49),
        _ => (0.0, 0.0, c::pf(w, id, SIZE)),
    };
    if b58 != 0.0 {
        c::set_pu8(w, id, 0x58, b58 as i32 as u8);
        c::set_pu8(w, id, 0x5a, b5a as i32 as u8);
        c::set_pf(w, id, SIZE, sz);
    }
    c::set_pf(w, id, D + 0x10, c::pf(w, id, SIZE));
    let g = w.rng.rand_angle();
    c::set_pf(w, id, GLOW, g);
    c::set_pu8(w, id, D + 9, 1);
    let h = if c::pi32(w, id, FALLOUT) == 0 { 3.0 } else {
        c::set_pi32(w, id, BIG, 1);
        1.0
    };
    c::set_pf(w, id, D, h);
    if c::pf(w, id, RANGE0) == 0.0 { c::set_pf(w, id, RANGE0, 15.0); }
    walker::seed(&mut w.mm(id).pvars, J);
    c::set_pf(w, id, J + 8, 2.0);
    c::set_pf(w, id, J + 0xc, 2.0);
    let r = (size(w, id) * 0.8 * 1024.0) as i32;
    c::set_pi32(w, id, J, r);
    let v = c::pf(w, id, SPEED) * c::DT;
    c::set_pf(w, id, J_VMAX, v);
    if c::pi32(w, id, BIG) == 0 {
        let s = if c::pi32(w, id, CUBOID) == -1 { 0xc } else { 0xd };
        set_st(w, id, s);
        let m = w.mm(id);
        m.has_collision = false;
        m.anim.speed = 0.0;
        m.visible = 0;
        m.position[2] -= 20.0;
        m.mode = (m.mode & !mode::TARGETABLE) | 1;
    } else {
        set_st(w, id, 1);
        w.mm(id).mode |= mode::TARGETABLE;
    }
}

/// `0x2eec68`: hits (and the split), the alert and range, the target, the glow. True when the moby was deleted.
fn pre(w: &mut World, id: MobyId) -> bool {
    let mut hit = w.get_hit(id, 0x33_0000, false);
    if let Some(h) = hit {
        let p = c::pos(w, id);
        let z0 = [p[0], p[1], p[2] + 0.5, p[3]];
        let src: c::V = match h.attacker {
            None => h.pos.map(|x| f32::from_bits(x.0)),
            Some(a) if w.m(a).o_class == 0x47 => w.hero.body_point.map(|x| f32::from_bits(x.0)),
            Some(a) if w.m(a).o_class == 0xba => z0,
            Some(a) => w.m(a).position,
        };
        if c::dist3(z0, src) < 24.0 && w.line(crate::moby_update::services::pv(z0), crate::moby_update::services::pv(src), 2, Some(id)).is_some() {
            hit = None;
        }
    }
    let res = damage::resolve(w, id, hit, D, 0, 4);
    let s = st(w, id);
    if res.out5 != 1 && res.damage != 0.0 && !matches!(s, 0xb | 0xa | 8) {
        let h = res.hit.unwrap();
        w.play_sound(0, 0, id);
        let sz = size(w, id);
        c::set_pu8(w, id, K + 0x3d, 0);
        c::set_pf(w, id, K + knock::k::GRAVITY, 0.008);
        c::set_pi32(w, id, K + knock::k::FLAGS, 8);
        let health = c::pf(w, id, D) - 1.0;
        c::set_pf(w, id, D, health);
        c::set_pi32(w, id, K + knock::k::RADIUS, (sz * 0.8 * 1024.0) as i32);
        c::set_pf(w, id, K + knock::k::ZOFF, sz * 0.8);
        let dir = h.dir.map(|x| f32::from_bits(x.0));
        goo_burst(w, id, dir);
        if health <= 0.0 {
            w.mm(id).mode &= !mode::TARGETABLE;
            w.delete_moby(id);
            return true;
        }
        for side in [-1.0f32, 1.0] {
            let Some(ch) = split_child(w, id) else { continue };
            c::set_pf(w, ch, K + knock::k::GRAVITY, 0.008);
            c::set_pu8(w, ch, K + 0x3d, 3);
            c::set_pi32(w, ch, K + knock::k::FLAGS, 8);
            c::set_pi32(w, ch, K + knock::k::RADIUS, (sz * 0.8 * 1024.0) as i32);
            c::set_pf(w, ch, K + knock::k::ZOFF, sz * 0.8);
            c::set_pf(w, ch, K + knock::k::SPEED, SPLIT_OUT * c::DT);
            c::set_pf(w, ch, K + knock::k::KEY_APEX, 7.0);
            c::set_pf(w, ch, K + knock::k::KEY_LAND, 13.0);
            c::set_pf(w, ch, K + knock::k::UP, SPLIT_UP * c::DT);
            let f = c::pi32(w, ch, K + knock::k::FLAGS) | 4;
            c::set_pi32(w, ch, K + knock::k::FLAGS, f);
            let (mut sp, mut up) = (c::pf(w, ch, K + knock::k::SPEED), c::pf(w, ch, K + knock::k::UP));
            let a = knock::aim(dir, &mut sp, &mut up);
            c::set_pf(w, ch, K + knock::k::SPEED, sp);
            c::set_pf(w, ch, K + knock::k::UP, up);
            let a = if side < 0.0 { c::sub_rot(a, PI / 4.0) } else { c::add_rot(a, PI / 4.0) };
            knock::start(w, ch, K, a, 4, 8, 0);
            w.mm(ch).anim.speed = 1.0;
            set_st(w, ch, 8);
        }
        set_death_bits(w, id, 0, -1);
        w.delete_moby(id);
        return true;
    }
    w.mm(id).hit_slot = 0xff;
    // The fall-out rule (0x2eec68): below the height of ripple patch +0x258 the amoeboid bursts (no bolts).
    let fall = c::pi32(w, id, FALLOUT);
    if fall != 0 && st(w, id) != 0 {
        match usize::try_from(fall).ok().and_then(|i| w.svc.water.patch_level(i)) {
            Some(z) if c::pos(w, id)[2] < z => {
                goo_burst(w, id, [0.0; 4]);
                w.delete_moby(id);
                return true;
            }
            Some(_) => {}
            None => w.svc.unported("amoeboid 572: fall-out rule without the level's ripple patch"),
        }
    }
    if c::pi32(w, id, ALERT) != 0 {
        let t = w.ticks(240);
        c::set_pi32(w, id, ALERT_T, t);
        c::set_pi32(w, id, ALERT, 0);
    }
    let r0 = c::pf(w, id, RANGE0);
    let r = if c::dec_timer_pvar_i32(w, id, ALERT_T) == 0 { r0 + 20.0 } else { r0 };
    c::set_pf(w, id, RANGE, r);
    let gate = c::pi32(w, id, GATE);
    if gate < 1 || w.table.mobys.get(gate as usize).is_some_and(|m| m.cmd != 0) {
        let poly = c::pi32(w, id, POLY);
        let tg = target::acquire_in(w, id, r, (poly >= 0).then_some(poly as usize));
        // The record at +0x1c0: position, Euler, aim, body, +0x40 the moby (= +0x200), +0x44 the kind (= +0x204).
        c::set_pv4(w, id, TGT_REC, tg.pos);
        c::set_pv4(w, id, TGT_REC + 0x10, tg.rot);
        c::set_pv4(w, id, TGT_REC + 0x20, tg.aim);
        c::set_pv4(w, id, TGT_REC + 0x30, tg.body);
        c::set_pi32(w, id, TGT, tg.moby.map_or(0, |m| m as i32 + 1));
        c::set_pi32(w, id, TGT_STATE, tg.kind as i32);
        if tg.kind != 2 {
            let home = c::pv4(w, id, HOME);
            let ok = c::dist3(home, tg.pos) <= r && (c::pos(w, id)[2] - tg.pos[2]).abs() <= 3.0;
            if !ok { c::set_pi32(w, id, TGT_STATE, 2); }
        }
    }
    if c::pi32(w, id, ALT_GROUP) >= 0 {
        if let Some(a) = alt_target(w, id) {
            let d = c::dist3(c::pos(w, id), w.m(a).position);
            if d < r {
                let keep = c::pi32(w, id, TGT_STATE) != 2 && c::dist3(c::pos(w, id), target_pos(w, id)) <= d;
                if !keep {
                    c::set_pi32(w, id, TGT, a as i32 + 1);
                    c::set_pi32(w, id, TGT_STATE, 1);
                }
            }
        }
    }
    if c::pi32(w, id, TGT) == 0 {
        let h = w.hero_moby.map(|h| h as i32 + 1).unwrap_or(0);
        c::set_pi32(w, id, TGT, h);
    }
    let g = c::add_rot(c::pf(w, id, GLOW), c::DT * 9.424_778);
    c::set_pf(w, id, GLOW, g);
    let f = (g.sin() + 1.0) * 0.5;
    w.mm(id).glow = crate::particles::tween_color(f.to_bits(), GLOW_A, GLOW_B);
    false
}

/// `0x2ef478(moby, group)`: the nearest targetable class-623 member of the group within 120.
fn alt_target(w: &World, id: MobyId) -> Option<MobyId> {
    let g = c::pi32(w, id, ALT_GROUP);
    let list = w.svc.groups.lists.get(g as usize)?.as_ref()?;
    let mut best = None;
    let mut bd = 120.0;
    for &e in list {
        let o = (e & 0x7fff) as usize;
        let m = w.m(o);
        if m.mode & mode::TARGETABLE == 0 || m.o_class != 0x26f || m.state >= 0x7f { continue; }
        let d = c::dist3(m.position, c::pos(w, id));
        if d < bd { bd = d; best = Some(o); }
    }
    best
}

/// `0x2ef350(moby)`: the next size's first member of the group still waiting (state 0xc), woken at the parent
/// (update / draw distance, light, mode, collision, position, rotation, health and range copied, matrix rebuilt).
fn split_child(w: &mut World, id: MobyId) -> Option<MobyId> {
    let next = match w.m(id).o_class { 0x23c => 0x361, 0x361 => 0x362, _ => return None };
    let g = w.m(id).group;
    let list = w.svc.groups.lists.get(g as u8 as usize)?.as_ref()?.clone();
    for e in list {
        let ch = (e & 0x7fff) as usize;
        if w.m(ch).state != 0xc || w.m(ch).o_class != next { continue; }
        let (ud, dd, light, ambient, md, pos, rot) = { let p = w.m(id); (p.update_dist, p.draw_dist, p.light, p.ambient, p.mode, p.position, p.rotation) };
        let has = w.classes.info(next).map(|i| i.has_collision && !i.no_header).unwrap_or(false);
        let (hp, rg) = (c::pf(w, id, D), c::pf(w, id, RANGE0));
        let m = w.mm(ch);
        m.update_dist = ud;
        m.draw_dist = dd;
        m.visible = 1;
        m.light = light;
        m.ambient = ambient;
        m.has_collision = has;
        m.mode = md;
        m.position = pos;
        m.rotation = rot;
        c::set_pf(w, ch, D, hp);
        c::set_pf(w, ch, RANGE0, rg);
        w.build_matrix(ch);
        return Some(ch);
    }
    None
}

/// `0x2ef560`: while drawn (+0x31), goo drips (type 52): `creature::fx::goo_drips`.
fn drip(w: &mut World, id: MobyId) {
    let sz = size(w, id);
    fx::goo_drips(w, id, sz);
}

/// `0x2ef770(moby, dir)`: the goo burst of a hit / death (type-2 blobs): `creature::fx::goo_burst`.
fn goo_burst(w: &mut World, id: MobyId, dir: c::V) {
    let sz = size(w, id);
    fx::goo_burst(w, id, dir, sz);
}
