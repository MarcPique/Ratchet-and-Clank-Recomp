//! Class 577, the Novalis critters (`GroundCritterUpdate` 0x2efc60, level01; the only class with this update).
//! A thin state machine over the shared creature layer ([`crate::moby_update::creature`]); spec
//! `docs/plan/creatures.md` "Critter 577".
//!
//! Two kinds by pvar+0x20a: **hoverers** (0: lifted 6 above their spawn at init, state 0xe) drift around their home
//! point 5–7 units up, picking a new spot every `ticks(120)` (3 draws); when Ratchet comes within the range + 4 (and
//! 8 in height) a `randi(19)` roll drops them (0x11) to the ground, where they **walk** (state 3) towards him with a
//! random side offset of ±30° that flips every `randf(30, 90)` ticks, **bite** (state 4: at key frame 13 of
//! sequence 3, within 2 xy and 0.5 height and facing within 15°: a hit of 1 on Ratchet with a 0.2 push), go **home**
//! (5) when he leaves the range, and **rise** back (0x10) where there is no ground or water under them. **Walkers**
//! (≠ 0) start in state 1 (sequence 4 at once) and never hover. The range (+0x1fc) is 12, or 24 for `ticks(240)` after
//! an alert (+0x38).
//!
//! Hits (mask 0x330000) through the resolver ([`creature::damage::resolve`]) while the class cooldown +0x26 is below
//! `ticks(45)`: the group is alerted (+0xbc = 1), the red hit flash starts, health −= damage; alive → knocked back
//! along the push (state 6: `4.4·dt` out, `7·dt` up, sequence 6, 60-tick cooldown), else the death flight (99:
//! `11·dt` out and up, sequence 7, not targetable) and `SetDeathBits` (the bolt drop, [`crate_::set_death_bits`]).
//! The flight ends (anim wrapped, out of the world, or landed dead on a moby, or fallen below z 0) in the death
//! explosion (at most one per 60 ticks from a 3-slot table: `0x273f50(0.5, 13, …, sound 4)`) and three body pieces
//! (1747–1749, [`creature::fx::break_piece`]), then `DeleteMoby`.
//!
//! Pvars (624 bytes; header at 0: damage +0x20, flash +0x110, knockback +0x120, walker +0x180): +0x1d0 home, +0x1e0
//! hover target, +0x1f0 turn velocity, +0x1f4 / +0x1f8 side offset (rad / deg), +0x1fc range, +0x204 speed, +0x208 /
//! +0x214 s16 timers, +0x20a kind, +0x20c hover pick timer, +0x210 offset flip timer, +0x216 s16 leash, +0x218 leash
//! moby, +0x21c path (−1 on Novalis), +0x228 hover wait, +0x60 the suck record (`creature::react`; state 7 is the Suck
//! Cannon's: `0x305260`). Not ported (counted in `Services::unported`): the path clamp `0x28b5e0` (no Novalis critter has a path), the
//! shadow probe `0x26f020` (moby +0x84/+0x88). The big-head cheat `0x278720(1.5, m, 0, +0x230)` (0x15edb7): `manip::big_head`.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, self as creature, attack, damage, flash, fx, ground, knock, target, turn, walker};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2efc60;
pub const CLASSES: [i16; 1] = [577];

const D: usize = 0x20;
const HIT_CD: usize = 0x26;
const ALERT: usize = 0x38;
const HOVER_V: usize = 0x40;
const VZ: usize = 0x48;
const FLASH: usize = 0x110;
const K: usize = 0x120;
const J: usize = 0x180;
const J_VZ: usize = 0x198;
const HOME: usize = 0x1d0;
const HOVER_T: usize = 0x1e0;
const TURN_V: usize = 0x1f0;
const SIDE: usize = 0x1f4;
const SIDE_DEG: usize = 0x1f8;
const RANGE: usize = 0x1fc;
const SPEED: usize = 0x204;
const T208: usize = 0x208;
const KIND: usize = 0x20a;
const HOVER_PICK: usize = 0x20c;
const FLIP: usize = 0x210;
const ALERT_T: usize = 0x214;
const LEASH: usize = 0x216;
const LEASH_M: usize = 0x218;
const PATH: usize = 0x21c;
const HOVER_WAIT: usize = 0x228;

/// `gp−0x51a0` 7.0, `gp−0x519c` 4.4, `gp−0x5198` 7.0, `gp−0x5194` 0.008 (level01 .lit 0x161a60..).
const UP_HIT: f32 = 7.0;
const OUT_HIT: f32 = 4.4;
const THROW_SPEED: f32 = 7.0;
const THROW_G: f32 = 0.008;

/// The body pieces of the death (`BreakFxB` classes).
pub const PIECES: [i16; 3] = [0x6d3, 0x6d4, 0x6d5];

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn seq_b(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
fn blend(w: &mut World, id: MobyId, seq: u8, frame: i32, t: i32) {
    let t = if t == 0 { 0 } else { w.ticks(t) };
    w.anim_blend(id, seq, frame, t);
}
fn blend_if(w: &mut World, id: MobyId, seq: u8, t: i32) { if seq_b(w, id) != seq { blend(w, id, seq, 0, t); } }

/// The side-offset pick (the repeated block of 0x2efc60): flip the ±30° sign on `randi(256)` odd, the offset in radians,
/// the flip timer `ticks(trunc(randf(30, 90)))`.
fn pick_side(w: &mut World, id: MobyId) {
    if w.rng.randi(0x100) & 1 != 0 {
        let d = -c::pf(w, id, SIDE_DEG);
        c::set_pf(w, id, SIDE_DEG, d);
    }
    let d = c::pf(w, id, SIDE_DEG);
    c::set_pf(w, id, SIDE, d * 0.017_453_292);
    let f = w.rng.randf(30.0, 90.0);
    let t = w.ticks(f as i32);
    c::set_pi32(w, id, FLIP, t);
}

fn zero_hover_v(w: &mut World, id: MobyId) { c::set_pv4(w, id, HOVER_V, [0.0; 4]); }

/// The walker step towards the facing (the classes pass `2·(cos yaw, sin yaw, 0)` as the target).
fn walk(w: &mut World, id: MobyId) -> u32 {
    let (cy, sy) = c::cs(c::yaw(w, id));
    let mut out = [0.0; 4];
    walker::step(w, id, J, 1.0, [cy + cy, sy + sy, 0.0, 0.0], &mut out)
}

/// The aim point of states 3 / 5 / 0xc: `p`, or its clamp to the path +0x21c.
fn aim(w: &mut World, id: MobyId, p: c::V) -> c::V {
    if c::pi32(w, id, PATH) != -1 { w.svc.unported("critter 577: path clamp 0x28b5e0"); }
    p
}

/// The ground check that ends states 3 / 5 / 0xc: no ground or water (surface 0) or surface 8 under → rise (0x10,
/// sequence 0); `+0xbc = 0` either way.
fn ground_or_rise(w: &mut World, id: MobyId) {
    let g = ground::ground(w, c::pos(w, id), 0.5, 0);
    if g.surface == 0 || g.surface == 8 {
        set_state(w, id, 0x10);
        blend_if(w, id, 0, 3);
    }
    w.mm(id).cmd = 0;
}

/// The death flight's end: the explosion (rate-limited) when `explode`, the three body pieces, `DeleteMoby`.
fn die(w: &mut World, id: MobyId, explode: bool) {
    let p = c::pos(w, id);
    if explode {
        let now = w.counter as i32;
        let dur = (w.ticks(1) as f32 * 60.0) as i32;
        if fx::rate_slot(&mut w.svc.creatures.death_fx_slots, now, dur) >= 0 {
            fx::death_explosion(w, 0.5, 13.0, Some(id), p, 4);
        }
    }
    let rot = w.m(id).rotation;
    for cl in PIECES { fx::break_piece(w, id, cl, p, rot, 0, 0); }
    w.delete_moby(id);
}

/// `GroundCritterUpdate` 0x2efc60.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x238 { return; }
    // 0x278720(1.5, m, 0, +0x230): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 1.5, id, 0, id, 0x230);
    if w.svc.game_mode == 2 {
        let m = w.mm(id);
        m.has_collision = false;
        m.mode |= 0x41;
        return;
    }
    {
        let has = w.classes.info(w.m(id).o_class).map(|i| i.has_collision && !i.no_header).unwrap_or(false);
        let m = w.mm(id);
        m.has_collision = has;
        m.mode &= !0x41;
    }
    let range0 = c::pf(w, id, RANGE);
    let tg = target::acquire(w, id, range0 + 4.0);
    let old = c::pos(w, id);
    if c::pi32(w, id, ALERT) != 0 {
        let t = w.ticks(240);
        c::set_pi16(w, id, ALERT_T, t as i16);
    }
    c::set_pi32(w, id, ALERT, 0);
    c::dec_timer_pvar_s16(w, id, T208);
    let running = c::dec_timer_pvar_s16(w, id, ALERT_T) == 0;
    c::set_pf(w, id, RANGE, if running { 24.0 } else { 12.0 });
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, D, 0, 4);
    c::dec_timer_pvar_s16(w, id, HIT_CD);
    let cd = c::pi16(w, id, HIT_CD);
    if (cd as i32) < w.ticks(45) {
        if let (Some(_), Some(h)) = (hit, res.hit) {
            if state(w, id) != 99 {
                on_hit(w, id, &h, tg);
            }
        }
    }
    w.mm(id).hit_slot = 0xff;
    let r = c::pf(w, id, RANGE);
    let pos = c::pos(w, id);
    let to_t = c::sub(tg.pos, pos);
    let dz_t = (pos[2] - tg.pos[2]).abs();
    match state(w, id) {
        0 => init(w, id),
        1 => {
            let near = c::len3(to_t) < r + 4.0 && dz_t < 8.0;
            if near && w.rng.randi(0x13) == 0 {
                set_state(w, id, 2);
                blend_if(w, id, 5, 3);
            } else if w.m(id).cmd != 1 || w.rng.randi(0x13) != 0 {
                if !(c::len3(to_t) <= r + 6.0 || c::pi16(w, id, KIND) != 0) {
                    set_state(w, id, 0x10);
                    blend_if(w, id, 0, 3);
                }
            } else {
                pick_side(w, id);
                set_state(w, id, 3);
                blend_if(w, id, 2, 3);
                w.mm(id).cmd = 0;
            }
        }
        2 => state2(w, id, tg, r),
        3 => {
            let dz = (pos[2] - tg.pos[2]).abs();
            let d2 = c::dist2(pos, tg.pos);
            let a = aim(w, id, tg.pos);
            let side = c::pf(w, id, SIDE);
            let h = c::atan(a[0] - pos[0], a[1] - pos[1]) + side;
            turn::spring_turn2_pvar(w, id, h, 0.05, 0.3, 0.2, TURN_V);
            let res = walk(w, id);
            if c::dec_timer_pvar_i32(w, id, FLIP) != 0 {
                let f = w.rng.randf(30.0, 90.0);
                let t = w.ticks(f as i32);
                c::set_pi32(w, id, FLIP, t);
                let s = -c::pf(w, id, SIDE);
                c::set_pf(w, id, SIDE, s);
            }
            let aux = c::sub(tg.pos, c::pv4(w, id, HOME));
            let p = c::pos(w, id);
            if 1.5 <= c::dist2(p, tg.pos) || res & 2 != 0 {
                if 1.0 < dz / d2 || res & 2 != 0 {
                    let q = c::pos(w, id);
                    c::set_pos(w, id, [old[0], old[1], q[2], q[3]]);
                    set_state(w, id, 0xc);
                    blend_if(w, id, 5, 3);
                } else if r + 4.0 <= c::len3(aux) || 8.0 <= (p[2] - tg.pos[2]).abs() {
                    if w.m(id).cmd != 1 { set_state(w, id, 5); }
                } else {
                    let g = w.m(id).group;
                    attack::group_command(w, g, 1);
                }
            } else {
                set_state(w, id, 4);
                blend_if(w, id, 3, 3);
            }
            ground_or_rise(w, id);
        }
        4 => {
            let a = c::atan(tg.pos[0] - pos[0], tg.pos[1] - pos[1]);
            turn::spring_turn2_pvar(w, id, a, 0.05, 0.3, 0.2, TURN_V);
            let an = &w.m(id).anim;
            if an.seq_a == an.seq_b && ground::key_time(w, id) == 13.0 && (pos[2] - tg.pos[2]).abs() < 0.5 && c::dist2(pos, tg.pos) < 2.0 {
                let a = c::atan(tg.pos[0] - pos[0], tg.pos[1] - pos[1]);
                let y = c::yaw(w, id);
                if c::diff_rots(a, y) < 0.261_799_4 {
                    let (cy, sy) = c::cs(y);
                    let dir = [cy * 0.2, sy * 0.2, 0.0, 0.0];
                    let p = [tg.pos[0], tg.pos[1], tg.pos[2] + 0.75, tg.pos[3]];
                    if let Some(t) = tg.moby { attack::hit_moby(w, t, id, 1.0, 1, p, dir); }
                }
            }
            if w.m(id).anim.flags & 2 != 0 && 1.5 < c::dist2(c::pos(w, id), tg.pos) {
                set_state(w, id, 3);
                blend_if(w, id, 2, 3);
            }
            fall(w, id, J_VZ);
        }
        5 => {
            let home = c::pv4(w, id, HOME);
            let a = aim(w, id, home);
            let h = c::atan(a[0] - pos[0], a[1] - pos[1]);
            turn::spring_turn2_pvar(w, id, h, 0.02, 0.3, 0.1, TURN_V);
            let res = walk(w, id);
            if c::dist2(c::pos(w, id), home) < 2.0 {
                set_state(w, id, 1);
                blend_if(w, id, 4, 3);
            }
            let aux = c::sub(tg.pos, home);
            if c::len3(aux) < r {
                let g = w.m(id).group;
                attack::group_command(w, g, 1);
                set_state(w, id, 3);
                blend_if(w, id, 2, 3);
            } else if w.m(id).cmd == 1 {
                set_state(w, id, 3);
                blend_if(w, id, 2, 3);
            } else if res & 2 != 0 {
                let q = c::pos(w, id);
                c::set_pos(w, id, [old[0], old[1], q[2], q[3]]);
                set_state(w, id, 0xc);
                blend_if(w, id, 5, 3);
            }
            ground_or_rise(w, id);
        }
        6 => {
            let r = knock::update(w, id, K);
            if r & knock::res::LANDED != 0 {
                set_state(w, id, 3);
                if seq_b(w, id) != 2 { blend(w, id, 2, 0, 3); }
                return;
            }
            if c::pos(w, id)[2] < 5.0 {
                set_death_bits(w, id, 0, -1);
                w.delete_moby(id);
                return;
            }
        }
        7 => {
            // FUN_00305260: carried by the Suck Cannon (the shared reaction layer); a let-go critter that landed
            // waits in 0xe (hover wait `ticks(180)`, sequence 0 at once), its suck record's state (+0xc8) cleared.
            if creature::react::carried(w, id, K) != 0 {
                set_state(w, id, 0xe);
                let t = w.ticks(0xb4);
                c::set_pi32(w, id, HOVER_WAIT, t);
                if seq_b(w, id) != 0 { w.anim_blend(id, 0, 0, 0); }
                c::set_pi16(w, id, 0x60 + creature::react::rec::STATE, 0);
            }
        }
        8 => {
            if c::len3(to_t) < r - 4.0 && dz_t < 8.0 && w.rng.randi(0x13) == 0 {
                set_state(w, id, 2);
                let mut q = c::pos(w, id);
                q[2] += 1.0;
                c::set_pos(w, id, q);
                blend_if(w, id, 5, 3);
            }
        }
        10 => state10(w, id, tg, r),
        0xb => {
            let r = knock::update(w, id, K);
            if r & knock::res::LANDED != 0 {
                set_state(w, id, 3);
                pick_side(w, id);
                if seq_b(w, id) != 2 { blend(w, id, 2, 8, 3); }
                return;
            }
            if c::pos(w, id)[2] < 5.0 {
                set_death_bits(w, id, 0, -1);
                w.delete_moby(id);
                return;
            }
        }
        0xc => {
            let dz = (pos[2] - tg.pos[2]).abs();
            let d2 = c::dist2(pos, tg.pos);
            let home = c::pv4(w, id, HOME);
            let a = aim(w, id, home);
            let h = c::atan(a[0] - pos[0], a[1] - pos[1]);
            turn::spring_turn2_pvar(w, id, h, 0.05, 0.3, 0.2, TURN_V);
            let res = walk(w, id);
            let p = c::pos(w, id);
            if 1.5 <= c::dist2(p, tg.pos) || 1.0 < dz / d2 {
                if 1.0 < dz / d2 || res & 2 != 0 {
                    if r + 6.0 < c::dist2(p, tg.pos) {
                        set_state(w, id, 0x10);
                        blend_if(w, id, 0, 3);
                    } else if w.m(id).anim.flags & 2 != 0 {
                        w.rng.randi(4);
                        let s = if w.rng.randi(4) != 0 { 4 } else { 5 };
                        blend(w, id, s, 0, 3);
                    }
                } else {
                    set_state(w, id, 5);
                    blend_if(w, id, 2, 3);
                }
            } else {
                set_state(w, id, 4);
                blend_if(w, id, 3, 3);
            }
            ground_or_rise(w, id);
        }
        0xd => state13(w, id),
        0xe => {
            let d = c::len3(to_t);
            let iv = if c::dec_timer_pvar_i32(w, id, HOVER_WAIT) == 0 {
                c::pi32(w, id, HOVER_WAIT)
            } else if d < r + 4.0 && dz_t < 8.0 && w.rng.randi(0x13) == 0 {
                set_state(w, id, 0x11);
                end(w, id);
                return;
            } else {
                c::pi32(w, id, HOVER_WAIT)
            };
            if iv == 0 && w.m(id).cmd == 1 && w.rng.randi(0x13) == 0 {
                pick_side(w, id);
                set_state(w, id, 0x11);
                zero_hover_v(w, id);
                w.mm(id).cmd = 0;
            } else if c::dec_timer_pvar_i32(w, id, HOVER_PICK) != 0 {
                let t = w.ticks(120);
                c::set_pi32(w, id, HOVER_PICK, t);
                let rr = w.rng.randf(0.0, 3.0);
                let a = w.rng.rand_angle();
                let (ca, sa) = c::cs(a);
                let z = w.rng.randf(5.0, 7.0);
                let home = c::pv4(w, id, HOME);
                c::set_pv4(w, id, HOVER_T, c::add([ca * rr, sa * rr, z, 0.0], home));
                set_state(w, id, 0xf);
            }
        }
        0xf => {
            let t = c::pv4(w, id, HOVER_T);
            let h = c::atan(t[0] - pos[0], t[1] - pos[1]);
            turn::spring_turn2_pvar(w, id, h, 0.05, 0.3, 0.2, TURN_V);
            let v = c::clamp_len3(c::sub(t, c::pos(w, id)), c::DT * 4.0);
            c::set_pv4(w, id, HOVER_V, v);
            let np = c::add(v, c::pos(w, id));
            let blocked = w.coll_sphere(crate::moby_update::services::pv(np), crate::moby_update::services::pf(0.4), 0, Some(id)).is_some();
            if !blocked {
                c::set_pos(w, id, np);
            } else {
                zero_hover_v(w, id);
                set_state(w, id, 0xe);
            }
            if c::len3(c::pv4(w, id, HOVER_V)) == 0.0 { set_state(w, id, 0xe); }
        }
        0x10 => {
            let gz = ground::ground(w, pos, 0.5, 0).z;
            let mut vz = c::pf(w, id, VZ);
            turn::approach(c::DT * 8.0, c::DT2 * 10.8, &mut vz);
            c::set_pf(w, id, VZ, vz);
            let mut p = c::pos(w, id);
            p[2] += vz;
            c::set_pos(w, id, p);
            if gz + 6.0 <= p[2] {
                p[2] = gz + 6.0;
                c::set_pos(w, id, p);
                set_state(w, id, 0xe);
                let t = w.ticks(180);
                c::set_pi32(w, id, HOVER_WAIT, t);
                zero_hover_v(w, id);
            }
        }
        0x11 => {
            let gz = ground::ground(w, pos, 0.5, 0).z;
            let mut vz = c::pf(w, id, VZ);
            turn::approach(-(c::DT * 8.0), c::DT2 * 10.8, &mut vz);
            c::set_pf(w, id, VZ, vz);
            let mut p = c::pos(w, id);
            p[2] += vz;
            c::set_pos(w, id, p);
            if seq_b(w, id) != 1 {
                let k = w.svc.timing.scale(crate::moby_update::services::pf(45.0)).to_f32();
                if p[2] - gz <= c::DT * 8.0 * k { blend(w, id, 1, 0, 5); }
            }
            if c::pos(w, id)[2] <= gz {
                let mut p = c::pos(w, id);
                p[2] = gz;
                c::set_pos(w, id, p);
                pick_side(w, id);
                set_state(w, id, 3);
                zero_hover_v(w, id);
                if seq_b(w, id) != 2 { blend(w, id, 2, 0, 3); }
            }
        }
        99 => {
            w.mm(id).mode &= !mode::TARGETABLE;
            let r = knock::update(w, id, K);
            if r & 0x140 == 0 {
                if 0.0 <= c::pos(w, id)[2] {
                    end(w, id);
                    return;
                }
                die(w, id, false);
            } else {
                die(w, id, true);
            }
            return;
        }
        _ => {}
    }
    end(w, id);
}

/// `LAB_002f1c30`: the hit flash (`0x2723f8`) and the shadow probe (`0x26f020`, moby +0x84 / +0x88).
fn end(w: &mut World, id: MobyId) {
    if w.m(id).is_deleted() { return; }
    flash::update(w, id, FLASH);
    crate::shadows::probe_down(w, id);
}

/// Gravity on the walker's vertical speed (+0x198) onto the ground (states 4 and 0xd).
fn fall(w: &mut World, id: MobyId, vz_off: usize) {
    let p = c::pos(w, id);
    let gz = ground::ground(w, p, 0.5, 0).z;
    let vz = c::pf(w, id, vz_off) - c::DT2 * 9.8;
    c::set_pf(w, id, vz_off, vz);
    let mut q = c::pos(w, id);
    q[2] += vz;
    if q[2] < gz {
        c::set_pf(w, id, vz_off, 0.0);
        q[2] = gz;
    }
    c::set_pos(w, id, q);
}

/// The hit branch of 0x2efc60 (module doc).
fn on_hit(w: &mut World, id: MobyId, h: &crate::moby_update::services::HitRecord, tg: target::Target) {
    // An attacker of class 0xb0 / 0xb1 does nothing while the cooldown runs. (The wrench's records carry no attacker:
    // the game reads a low-memory halfword there, never 0xb0 / 0xb1.)
    let mut dmg = f32::from_bits(h.damage.0);
    if let Some(a) = h.attacker {
        if (w.m(a).o_class as u16).wrapping_sub(0xb0) < 2 && c::pi16(w, id, HIT_CD) != 0 {
            dmg = 0.0;
            set_record_damage(w, id, 0.0);
        }
    }
    let g = w.m(id).group;
    if g != -1 { attack::group_command(w, g, 1); }
    if dmg == 0.0 { return; }
    let pos = c::pos(w, id);
    let _away = c::atan(pos[0] - tg.pos[0], pos[1] - tg.pos[1]);
    let health = c::pf(w, id, D) - dmg;
    c::set_pf(w, id, K + knock::k::GRAVITY, 0.008);
    c::set_pf(w, id, K + knock::k::DRAG, f32::from_bits(0x3a03_126f));
    c::set_pf(w, id, K + knock::k::SPEED, OUT_HIT * c::DT);
    c::set_pf(w, id, K + knock::k::UP, UP_HIT * c::DT);
    c::set_pi32(w, id, K + knock::k::FLAGS, 9);
    c::set_pf(w, id, D, health);
    c::set_pu8(w, id, K + 0x3d, 0);
    let dir = h.dir.map(|x| f32::from_bits(x.0));
    if 0.0 < health {
        let (mut s, mut u) = (c::pf(w, id, K + knock::k::SPEED), c::pf(w, id, K + knock::k::UP));
        let a = knock::aim(dir, &mut s, &mut u);
        c::set_pf(w, id, K + knock::k::SPEED, s);
        c::set_pf(w, id, K + knock::k::UP, u);
        knock::start(w, id, K, a, 6, 1, 0);
        c::set_pf(w, id, K + knock::k::KEY_APEX, 5.0);
        c::set_pf(w, id, K + knock::k::KEY_LAND, 10.0);
        set_state(w, id, 6);
        c::set_pu8(w, id, FLASH + 7, 0xfa);
        let t = w.ticks(60);
        c::set_pi16(w, id, HIT_CD, t as i16);
        flash::start(w, id, FLASH);
    } else {
        w.mm(id).mode &= !mode::TARGETABLE;
        c::set_pf(w, id, K + knock::k::UP, c::DT * 11.0);
        c::set_pf(w, id, K + knock::k::SPEED, c::DT * 11.0);
        let (mut s, mut u) = (c::DT * 11.0, c::DT * 11.0);
        let a = knock::aim(dir, &mut s, &mut u);
        c::set_pf(w, id, K + knock::k::SPEED, s);
        c::set_pf(w, id, K + knock::k::UP, u);
        knock::start(w, id, K, a, 7, 1, 0);
        c::set_pf(w, id, K + knock::k::KEY_APEX, 11.0);
        c::set_pf(w, id, K + knock::k::KEY_LAND, 18.0);
        set_state(w, id, 99);
        c::set_pu8(w, id, FLASH + 7, 0x78);
        flash::start(w, id, FLASH);
        set_death_bits(w, id, 0, -1);
    }
}

fn set_record_damage(w: &mut World, id: MobyId, d: f32) {
    let s = w.m(id).hit_slot;
    if let Some(r) = w.svc.hits.records.get_mut(s as usize) { r.damage = crate::moby_update::services::pf(d); }
}

/// State 0: the init (module doc).
fn init(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    c::set_pv4(w, id, HOME, p);
    c::set_pu8(w, id, D + 9, 0);
    if c::pi16(w, id, KIND) == 0 {
        set_state(w, id, 0xe);
        let mut q = p;
        q[2] += 6.0;
        c::set_pos(w, id, q);
        if seq_b(w, id) != 0 { w.anim_blend(id, 0, 0, 0); }
    } else {
        set_state(w, id, 1);
        if seq_b(w, id) != 4 { w.anim_blend(id, 4, 0, 0); }
    }
    c::set_pi16(w, id, ALERT_T, 0);
    c::set_pi16(w, id, T208, 0);
    c::set_pf(w, id, D, 1.0);
    c::set_pi16(w, id, D + 4, 1);
    c::set_pu8(w, id, 0x58, 8);
    c::set_pu8(w, id, 0x5a, 4);
    walker::seed(&mut w.mm(id).pvars, J);
    let v = c::pf(w, id, SPEED) * c::DT;
    c::set_pi32(w, id, J, 0x38d);
    c::set_pf(w, id, J + 8, 2.0);
    c::set_pf(w, id, J + 0xc, 2.0);
    c::set_pf(w, id, J + 0x24, v);
    if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
}

fn state2(w: &mut World, id: MobyId, tg: target::Target, r: f32) {
    let pos = c::pos(w, id);
    let a = c::atan(pos[0] - tg.pos[0], pos[1] - tg.pos[1]);
    turn::spring_turn2_pvar(w, id, a, 0.02, 0.3, 0.1, TURN_V);
    let d = c::len3(c::sub(tg.pos, pos));
    let dz = (pos[2] - tg.pos[2]).abs();
    let mut picked = false;
    if d < r && dz < 8.0 && w.rng.randi(0x13) == 0 {
        let g = w.m(id).group;
        attack::group_command(w, g, 1);
        picked = true;
    } else if w.m(id).cmd == 1 && w.rng.randi(0x13) == 0 {
        picked = true;
    }
    if picked {
        pick_side(w, id);
        set_state(w, id, 3);
        if seq_b(w, id) != 2 { blend(w, id, 2, 0, 3); }
        w.mm(id).cmd = 0;
    } else if r + 6.0 < d && c::pi16(w, id, KIND) == 0 {
        set_state(w, id, 0x10);
        blend_if(w, id, 0, 3);
    }
    if seq_b(w, id) == 5 && w.m(id).anim.flags & 2 != 0 { blend(w, id, 4, 0, 3); }
}

fn state10(w: &mut World, id: MobyId, tg: target::Target, r: f32) {
    let pos = c::pos(w, id);
    let home = c::pv4(w, id, HOME);
    let a = c::atan(home[0] - pos[0], home[1] - pos[1]);
    turn::spring_turn2_pvar(w, id, a, 0.05, 0.3, 0.2, TURN_V);
    let d = c::len3(c::sub(tg.pos, home));
    let go = if d < r && (home[2] - tg.pos[2]).abs() < 8.0 { true } else { w.m(id).cmd == 2 };
    if !go { return; }
    let g = w.m(id).group;
    attack::group_command(w, g, 2);
    if seq_b(w, id) != 6 { blend(w, id, 6, 0, 3); }
    c::set_pf(w, id, K + knock::k::DRAG, 0.0);
    c::set_pf(w, id, K + knock::k::GRAVITY, THROW_G);
    c::set_pf(w, id, K + knock::k::SPEED, THROW_SPEED * c::DT);
    let mut t = 0.0;
    let up = knock::lob_up(THROW_SPEED * c::DT, -THROW_G, c::pos(w, id), home, &mut t);
    c::set_pf(w, id, K + knock::k::UP, up);
    c::set_pi32(w, id, K + knock::k::FLAGS, 0xd);
    c::set_pu8(w, id, K + 0x3d, 0);
    let y = c::yaw(w, id);
    knock::start(w, id, K, y, 6, 1, 0);
    c::set_pf(w, id, K + knock::k::KEY_APEX, 5.0);
    c::set_pf(w, id, K + knock::k::KEY_LAND, 10.0);
    set_state(w, id, 0xb);
}

/// State 0xd: pushed along the facing at the walking speed, sliding round Ratchet and other critters within 2.
fn state13(w: &mut World, id: MobyId) {
    let an = w.m(id).anim;
    if an.seq_a != 2 && an.seq_b != 2 { blend(w, id, 2, 0, 3); }
    let pos = c::pos(w, id);
    let (cy, sy) = c::cs(c::yaw(w, id));
    let s = c::pf(w, id, SPEED) * c::DT;
    let mut e = c::add(pos, [cy * s, sy * s, 0.0, 0.0]);
    let n = w.table.mobys.len();
    for o in 0..n {
        if o == id { continue; }
        let m = w.m(o);
        if m.state >= 0x80 { continue; }
        if m.o_class != 0 && m.o_class != 0x241 { continue; }
        let op = m.position;
        if c::dist2(e, op) < 2.0 {
            let z = pos[2];
            let a = c::atan(op[0] - pos[0], op[1] - pos[1]);
            let d = c::sub_rot(c::yaw(w, id), a);
            let step = if d <= 0.0 { -(c::DT * std::f32::consts::FRAC_PI_2) } else { c::DT * std::f32::consts::FRAC_PI_2 };
            let y = c::add_rot(c::yaw(w, id), step);
            c::set_yaw(w, id, y);
            e = c::add(c::set_len3(c::sub(pos, op), 2.0), op);
            e[2] = z;
        }
    }
    c::set_pos(w, id, e);
    fall(w, id, J_VZ);
    let anchor = c::pi32(w, id, LEASH_M);
    if anchor > 0 {
        let a = (anchor - 1) as usize;
        if a < w.table.mobys.len() && c::dist2(c::pos(w, id), w.m(a).position) <= c::pi16(w, id, LEASH) as f32 {
            w.mm(id).cmd = 0;
            return;
        }
    }
    set_state(w, id, 5);
    blend_if(w, id, 2, 3);
    w.mm(id).cmd = 0;
}
