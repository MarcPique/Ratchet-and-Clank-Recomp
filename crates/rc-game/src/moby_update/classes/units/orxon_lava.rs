//! **Orxon's lava spouts, class 939** (level10 `0x2d8c00`, census U352; 11 placed) and **their glowing rocks, class
//! 938** (`0x2d85c8`, made by `0x2d8058`). Every `ticks(rand_range(240, 600))` a spout throws a rock up its heading
//! (± 10°) at 50–60° and `randf(7, 9)` a second, sound 0. The rock flies under 9.8 gravity, bounces off the floor and
//! mobys (after its first `ticks(60)`) tilting with the face, comes to rest glowing and cools over `ticks(240)`, then
//! bursts; a hit from another class (0x830000) bats it away from Ratchet (an unhit push: at 10·dt, the hit's push
//! aiming it) and it flies hot until it hits something or the floor, and bursts: the area hit (radius 1.5, damage 1,
//! push 1 / 1, flags 0x810001, types 2 / 1) and the death explosion (0.5, 13, sound 0).
//!
//! **Rock pvars**: +0x00 the velocity, +0x10 the spout, +0x14 (s16) the timer, +0x18 / +0x1c spins (unused). The rock
//! is kept 0.1 higher than its point (gp−0x5014) between ticks.
//!
//! **The bounce** (`0x2d8170(m, v, old)`, not while the timer runs): `CollLine(old, pos, 0x10)`; a moby: pushed out
//! from it by 9.7·dt², v reflected off the push and ·0.35, the tilt (the face's normal turned into the rock's yaw:
//! pitch and roll eased by 3°); the world: onto the hit 9.7·dt² off the face, v reflected ·0.5; a floor (normal
//! z > 0.707): kept above the ground height, a falling v.z ·−0.4, slower than dt → at rest (3); the tilt; sound 1.
//!
//! Read from the level10 decomp and disassembly; the constants from the overlay (gp−0x5014 .. −0x4ff8: 0.1, 240, 600,
//! 10, 50, 60, 7, 9). The batted rock builds a hit template it never uses (its collision line takes none). Native
//! `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2d8c00` | the spout | [`update`] |
//! | `0x2d8058(m, p, v)` / `0x2d85c8` | the rock / its update | `rock` / [`rock_update`] |
//! | `0x2d8170` | the bounce | `bounce` |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, add, atan, attack, fx, knock, len3, set_len3, V, DT, DT2};
use crate::moby_update::services::{pf, pv, reflect, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 10;
pub const UPDATE_FN: u32 = 0x2d_8c00;
pub const ROCK_FN: u32 = 0x2d_85c8;
pub const CLASSES: [i16; 1] = [939];
pub const ROCK_CLASSES: [i16; 1] = [938];

const ROCK: i16 = 0x3aa;
const LIFT: f32 = f32::from_bits(0x3dcc_cccd);
const DEG: f32 = 0.017_453_292;
const TILT: f32 = f32::from_bits(0x3d56_7750);

mod rpo {
    pub const VEL: usize = 0x00;
    pub const OWNER: usize = 0x10;
    pub const TIMER: usize = 0x14;
    pub const T16: usize = 0x16;
    pub const SPIN_A: usize = 0x18;
    pub const SPIN_B: usize = 0x1c;
    pub const LEN: usize = 0x20;
}

fn reload(w: &mut World, id: MobyId) {
    let r = w.rng.rand_range(240, 600);
    let t = w.ticks(r);
    c::set_pi32(w, id, 0, t);
}

/// Level10 `0x2d8c00`: the spout.
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 4);
    if w.m(id).state == 0 {
        reload(w, id);
        w.mm(id).state = 1;
    }
    if c::dec_timer_pvar_i32(w, id, 0) == 0 { return; }
    let off = w.rng.randf(-10.0, 10.0);
    let yaw = c::add_rot(w.m(id).rotation[2], off * DEG);
    let pitch = w.rng.randf(50.0, 60.0);
    let speed = w.rng.randf(7.0, 9.0);
    let v = fx::polar(speed * DT, yaw, pitch * DEG);
    let v = [v[0], v[1], v[2], 0.0];
    let p = add(c::pos(w, id), v);
    rock(w, id, p, v);
    w.play_sound(0, 0, id);
    reload(w, id);
}

/// `0x2d8058(m, p, v)`: a rock.
fn rock(w: &mut World, owner: MobyId, p: V, v: V) {
    let Some(m) = w.create_moby(ROCK) else { return };
    story::pvars(w, m, rpo::LEN);
    {
        let mo = w.mm(m);
        mo.update_dist = 0xff;
        mo.draw_dist = 0x7f;
        mo.visible = 1;
        mo.state = 0;
        mo.position = [p[0], p[1], p[2] + LIFT, p[3]];
    }
    c::set_pi32(w, m, rpo::OWNER, owner as i32 + 1);
    c::set_pi16(w, m, rpo::T16, 0);
    c::set_pv4(w, m, rpo::VEL, v);
    let (a, b) = (DT * std::f32::consts::FRAC_PI_3, DT * std::f32::consts::PI);
    let s = w.rng.randf(a, b);
    c::set_pf(w, m, rpo::SPIN_A, s);
    let s = w.rng.randf(a, b);
    c::set_pf(w, m, rpo::SPIN_B, s);
    let t = w.ticks(0x3c) as i16;
    c::set_pi16(w, m, rpo::TIMER, t);
    w.build_matrix(m);
}

fn outside(p: V) -> bool { !(1.0..=511.0).contains(&p[0]) || !(1.0..=511.0).contains(&p[1]) || !(1.0..=511.0).contains(&p[2]) }

/// A hit on a flying or resting rock: batted (→ 2) or burst (→ 3).
fn hits(w: &mut World, id: MobyId) {
    if let Some(h) = w.get_hit(id, 0x83_0000, false) {
        let other = h.attacker.is_some_and(|a| w.m(a).o_class != w.m(id).o_class);
        if other {
            if h.b28 == 0 {
                let (mut speed, mut up) = (DT * 10.0, DT * 10.0);
                let a = knock::aim(h.dir.map(|x| f32::from_bits(x.0)), &mut speed, &mut up);
                c::set_pv4(w, id, rpo::VEL, [a.cos() * speed, a.sin() * speed, up, 0.0]);
                let t = w.ticks(0xf) as i16;
                c::set_pi16(w, id, rpo::TIMER, t);
                w.mm(id).state = 2;
            } else {
                w.mm(id).state = 3;
            }
        }
    }
    w.mm(id).hit_slot = 0xff;
}

fn red(w: &mut World, id: MobyId, r: u8) { w.mm(id).ambient[0] = r; }

/// Level10 `0x2d85c8`: the rock (module doc).
pub fn rock_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, rpo::LEN);
    if outside(c::pos(w, id)) {
        w.delete_moby(id);
        return;
    }
    w.mm(id).position[2] -= LIFT;
    let old = c::pos(w, id);
    match w.m(id).state {
        0 => {
            fly(w, id);
            hits(w, id);
            if c::dec_timer_pvar_s16(w, id, rpo::TIMER) != 0 && bounce(w, id, old) == 3 {
                let t = w.ticks(0xf0) as i16;
                c::set_pi16(w, id, rpo::TIMER, t);
                w.mm(id).state = 1;
            } else if w.m(id).position[2] < 2.0 {
                w.mm(id).state = 3;
            }
        }
        1 => {
            let full = w.ticks(0xf0);
            let k = ((full - c::pi16(w, id, rpo::TIMER) as i32) * 0x7f) / full.max(1);
            red(w, id, k as u8);
            if c::dec_timer_pvar_s16(w, id, rpo::TIMER) == 0 { hits(w, id) } else { w.mm(id).state = 3 }
        }
        2 => {
            red(w, id, 0xff);
            fly(w, id);
            let out = c::dec_timer_pvar_s16(w, id, rpo::TIMER) != 0;
            if (out && bounce(w, id, old) != 0) || w.m(id).position[2] < 2.0 { w.mm(id).state = 3; }
        }
        3 => {
            let p = c::pos(w, id);
            attack::area_hit(w, 1.5, p, id, 1.0, 1.0, 1.0, None, 0x81_0001, 2, 1);
            fx::death_explosion(w, 0.5, 13.0, Some(id), p, 0);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    w.mm(id).position[2] += LIFT;
    if outside(c::pos(w, id)) { w.delete_moby(id); }
}

fn fly(w: &mut World, id: MobyId) {
    let mut v = c::pv4(w, id, rpo::VEL);
    let p = add(c::pos(w, id), v);
    c::set_pos(w, id, p);
    v[2] -= DT2 * 9.8;
    c::set_pv4(w, id, rpo::VEL, v);
}

/// The tilt toward the face's normal `n` (module doc).
fn tilt(w: &mut World, id: MobyId, n: V) {
    let rz = w.m(id).rotation[2];
    let (cz, sz) = (rz.cos(), rz.sin());
    let f80 = n[0] * cz + n[1] * sz;
    let f7c = n[1] * cz - n[0] * sz;
    let f78 = n[2];
    let a = atan((f80 * f80 + f78 * f78).abs().sqrt(), f7c);
    let rx = crate::moby_update::classes::mine::approach_angle(w.m(id).rotation[0], -a, TILT);
    w.mm(id).rotation[0] = rx;
    let ry = crate::moby_update::classes::mine::approach_angle(w.m(id).rotation[1], atan(f78, f80), TILT);
    w.mm(id).rotation[1] = ry;
}

/// `0x2d8170(m, v, old)` (module doc): 0 none, 1 a moby, 2 the world, 3 at rest.
fn bounce(w: &mut World, id: MobyId, old: V) -> i32 {
    if c::pi16(w, id, rpo::TIMER) != 0 { return 0; }
    let p = c::pos(w, id);
    let Some(h) = w.coll_line(pv(old), pv(p), 0x10, Some(id)) else { return 0 };
    let at = [h.point[0], h.point[1], h.point[2], p[3]];
    let v = c::pv4(w, id, rpo::VEL);
    if let Some(m) = h.moby {
        let d = c::sub(p, w.m(m).position);
        let q = add(at, set_len3(d, DT2 * 9.7));
        c::set_pos(w, id, q);
        let r = reflect(pv(v), pv(d)).map(|x| f32::from_bits(x.0));
        c::set_pv4(w, id, rpo::VEL, r.map(|x| x * f32::from_bits(0x3eb3_3333)));
        tilt(w, id, set_len3(d, 1.0));
        w.play_sound(1, 0, id);
        return 1;
    }
    if h.kind <= 0 { return 0; }
    let n = [h.normal[0], h.normal[1], h.normal[2], 0.0];
    let mut q = add(at, set_len3(n, DT2 * 9.7));
    c::set_pos(w, id, q);
    let mut r = reflect(pv(v), pv(n)).map(|x| f32::from_bits(x.0)).map(|x| x * 0.5);
    let u = set_len3(n, 1.0);
    if 0.707 < u[2] {
        let g = f32::from_bits(w.ground_height(pf(0.5), pv(q), 0).0);
        if q[2] < g {
            q[2] = g;
            c::set_pos(w, id, q);
        }
        if r[2] < 0.0 { r[2] *= -0.4; }
        if len3(r) < DT {
            c::set_pv4(w, id, rpo::VEL, [0.0; 4]);
            return 3;
        }
    }
    c::set_pv4(w, id, rpo::VEL, r);
    tilt(w, id, u);
    w.play_sound(1, 0, id);
    2
}
