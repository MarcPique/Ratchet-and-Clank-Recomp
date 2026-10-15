//! **The floats that carry Ratchet by writing his platform delta** (G-HERO-034): Gaspar's sinking floats 664 (level09
//! `0x2f86a0`, census U307, 6 instances) and 1293 / 1320 (level09 `0x3091b0`, U316, 6 instances: the same float that
//! rises out of the lava once its linked moby is gone), and Umbris' rocking floats 1069 (level07 `0x3112c8`, U255, 16
//! instances: they drop off their perch when told and settle on the water). While Ratchet stands on one (ground moby
//! 0x13f64c = it, on the ground) the float sinks / rocks toward him and carries him itself: Ratchet's position turned
//! about the float by its rotation change (`0x27fe88` / level07 `0x288968`: `services::turn_about`) becomes his
//! platform delta 0x13f440 and the float's drop is added to 0x13f448 (`services::HeroFields::ride`: the general hero
//! seam a class uses to carry Ratchet itself). Everything else on them rides the platform block (`CarryRiders`
//! 0x2755f8: `triggers::carry_riders`). Read from the level09 / level07 decomp and disassembly; the floats' constants
//! are the overlays' small data (level09 gp−0x520c..−0x51e8 for 664, gp−0x4e3c..−0x4e18 for 1293 / 1320; no writer).
//!
//! **System or not.** The ride write is the shared seam (`HeroFields::ride` + `turn_about`, the one code the three
//! classes and the levels' copies share); the floats themselves are per-class code (two units on 09 with their own
//! constants and states, one on 07 with a different state machine), kept per class here.
//!
//! **Pvars** (664 / 1293): +0x20 the platform block, +0x60 / +0x64 the per-tick tilt (x / y; +0x68 / +0x6c copied
//! with them), +0x70 / +0x74 their spring rates, +0x80 initialised, +0x84 the rest height, +0x88 the ride timer, +0x8c
//! (1293) the linked moby, +0x90 / +0x94 the wobble phases, +0x98 / +0x9c the bob phase / last offset.
//! (1069): +0x20.. a header (+0x20 = 0, +0x24 = 0, +0x28 = 4, +0x3e = 0xd), +0x60 the platform block, +0xa0 the linked
//! moby, +0xa4 / +0xa8 the rest tilt, +0xac the rock timer / ride timer, +0xb0 the vertical speed, +0xb4 the fall
//! acceleration, +0xb8 the water height, +0xbc keep after the fall, +0xbd the drop delay, +0xbe / +0xbf tell the
//! linked moby (and the value), +0xc0 / +0xc4 the bob phases, +0xc8 the splash timer, +0xcc last tick's height.
//!
//! **Coverage, 664** (level09 0x2f86a0):
//!
//! | address | what | status |
//! |---|---|---|
//! | +0 | the old position / rotation kept; low = +0x84 − 0.25 (before the init) | ported ([`float_664`]) |
//! | +1 | +0x80 = 0: +0x60.. = the rotation, +0x70.. = 0, z −= 0.3, +0x80 = 1, +0x84 = z | ported |
//! | +2 | Ratchet grounded (0x13f65e = 0) on it (0x13f64c): +0x88 = 0 → `PlayClassSound(0, 0, m)`; +0x88 raised to `ticks(30)` | ported |
//! | +3 | +0x88 = 0: the tilt springs back to 0 (`0x270b58(0, 10°·dt², 0.01, 5°·dt)`), z += 0.3·dt·(+0x84 − z) (≤ +0x84); crossing low upward → `PlayClassSound(2, 0, m)` | ported (`hero::physics::turn_spring`) |
//! | +4 | +0x88 ≠ 0: `FastDecTimer`; the tilt springs toward Ratchet (x: −0.1·(d·row1), y: 0.1·(d·row0); 40°·dt², 0.01, 10°·dt); z −= 0.35·dt; crossing low downward → `PlayClassSound(1, 0, m)`; Ratchet carried (`0x27fe88` with the old / new tilt) → 0x13f440, 0x13f448 += the drop | ported (`HeroFields::ride`) |
//! | +5 | the bob (`0x277a00(0.14, 30°·dt)`) and the wobble (`0x277a80(0.087, 18°·dt, 28°·dt)`); rotation += the tilt; `CarryRiders(+0x20, position change, old rotation, rotation)` | ported (`units::bob`, `units::wobble`, `triggers::carry_riders`) |
//!
//! **Coverage, 1293 / 1320** (level09 0x3091b0):
//!
//! | address | what | status |
//! |---|---|---|
//! | state 0 | +0x80 = 1, +0x60.. = the rotation, +0x70.. = 0, +0x84 = z; a link (+0x8c ≠ −1): z −= 5, rotation x / y = `randf(−90°, 90°)` each, → 1; none: `STUB_printf`, → 2 | ported ([`float_1293`]) |
//! | state 1 | the linked moby alive (not 0xfe / 0xfd) → wait; gone → 2 | ported |
//! | state 2 | as 664 with low = +0x84 − 0.3, the rise 1.0·dt toward +0x84 (more than 2 below it: one more step and the springs ×3 with max 90°·dt), the ride sink 0.35·dt, the wobble 25°·dt / 31°·dt | ported |
//! | other states | return | ported |
//!
//! **Coverage, 1069** (level07 0x3112c8):
//!
//! | address | what | status |
//! |---|---|---|
//! | +0 | +0xbc = 2 (told to drop): state 2, +0xbc = 3, +0xac = +0xbd | ported ([`float_1069`]) |
//! | state 0 | the header; `rand() & 1` → yaw += π; rest tilt = rotation x / y; +0xc8 = −1; → 1, +0xbc = 0; a mission (+0xb0 ≠ 0xff) already done: z −= 5, +0xbc (pvar) = 0 → `DeleteMoby`, else → 5 | ported |
//! | state 1 | +0xbc = 1 (told to rock): +0xac = `rand() % 10` when 0; +0xbe → `0x26e090(group, 2)`; the rock: tilt = rest + 1°·sin(wrap(+0xac·0.75)), 1°·cos(wrap(+0xac·0.89)) | ported (`creature::attack::group_command`) |
//! | state 2 | the rock; `FastDecTimer(+0xac)` out: +0xbe → the linked moby's pvar +0x19d = +0xbf and its state 0xc; → 3 | ported |
//! | state 3 | the fall: rotation x += (80° − x)·0.05, z −= 3.1·sin(that step), +0xb0 −= +0xb4·dt², z += +0xb0; below +0xb8 − 15: keep (+0xbc) → +0xb0 = 0, 4; else `DeleteMoby` | ported |
//! | state 4 | the linked moby's pvar +0xef < 2 and its state > 0xb → wait; else +0xb0 = `randf(dt, 4·dt)`, the bob phases `rand_angle` ×2, → 5, tilt 0, +0xac = 0 | ported |
//! | state 5 | the float: the ride timer (`ticks(90)`, sound 0 on the first touch); free: buoyancy 0.2·(+0xb8 − z) (±0.9) below the water, gravity 9.8·dt², at most 5·dt up; ridden: −0.25·dt within 2.6 of the water, else 0; z += it; the bob phases +0.01 / +0.017; ridden: the tilt toward Ratchet (±0.03 of d·row0 / row1), Ratchet carried (`0x288968` with the rotation before and after this point: the same, so 0x13f440 = 0 and no drop) ; the tilt approaches (`0x270ac0`, 10°·dt) 0.02·sin of the phases, clamped to ±30° | ported |
//! | tail (state 5) | `FastDecTimer(+0xc8)` out and z crossed +0xb8 since last tick → `PlayClassSound(0, 0, m)`, +0xc8 = `rand() % ticks(20) + ticks(20)`; +0xcc = z | ported |
//! | tail | `CarryRiders(+0x60, position change, old rotation, rotation)` | ported |
//!
//! No particle, light, hit or flag besides the listed sounds and the linked moby's command.

use crate::hero::physics::turn_spring;
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT, DT2};
use crate::moby_update::services::{turn_about, World};
use crate::moby_update::triggers;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL_664: u32 = 9;
pub const UPDATE_FN_664: u32 = 0x2f_86a0;
pub const CLASSES_664: [i16; 1] = [664];
pub const REFERENCE_LEVEL_1293: u32 = 9;
pub const UPDATE_FN_1293: u32 = 0x30_91b0;
pub const CLASSES_1293: [i16; 2] = [1293, 1320];
pub const REFERENCE_LEVEL_1069: u32 = 7;
pub const UPDATE_FN_1069: u32 = 0x31_12c8;
pub const CLASSES_1069: [i16; 1] = [1069];

const DEG: f32 = 0.017_453_292;
/// 0x15ed64: the speed multiplier (1.0).
const SPEED: f32 = c::SPEED;

/// The two floats' constants (level09 small data).
struct Consts {
    /// gp−0x520c / −0x4e3c: the tilt toward Ratchet per unit.
    lean: f32,
    /// −0x5208 / −0x4e38: the ride sink (u/s).
    sink: f32,
    /// −0x5204 / −0x4e34: the rise rate (1/s).
    rise: f32,
    /// −0x51fc..−0x51f4 / −0x4e2c..: the ridden spring (k °/s², d, max °/s).
    ride: [f32; 3],
    /// −0x51f0..−0x51e8 / −0x4e20..: the free spring.
    free: [f32; 3],
    /// The wobble rates (°/s).
    wobble: [f32; 2],
    /// low = +0x84 − this.
    low: f32,
}

const C664: Consts = Consts { lean: 0.1, sink: 0.35, rise: 0.3, ride: [40.0, 0.01, 10.0], free: [10.0, 0.01, 5.0], wobble: [0.314_159_27, 0.488_692_2], low: 0.25 };
const C1293: Consts = Consts { lean: 0.1, sink: 0.35, rise: 1.0, ride: [40.0, 0.01, 10.0], free: [10.0, 0.01, 5.0], wobble: [0.436_332_32, 0.541_052_04], low: 0.3 };

fn pf(w: &World, id: MobyId, o: usize) -> f32 { c::pf(w, id, o) }
fn set(w: &mut World, id: MobyId, o: usize, v: f32) { c::set_pf(w, id, o, v) }
fn v3(a: [f32; 4]) -> [f32; 3] { [a[0], a[1], a[2]] }

/// `0x270b58(target, k, d, max, &pvar a, &pvar v, 0)` on the pvars.
#[allow(clippy::too_many_arguments)]
fn spring(w: &mut World, id: MobyId, target: f32, k: f32, d: f32, max: f32, a: usize, v: usize) {
    let (mut ang, mut vel) = (Pf::f(pf(w, id, a)), Pf::f(pf(w, id, v)));
    turn_spring(Pf::f(target), Pf::f(k), Pf::f(d), Pf::f(max), &mut ang, &mut vel, 0);
    set(w, id, a, ang.to_f32());
    set(w, id, v, vel.to_f32());
}

/// Ratchet stands on `id` (0x13f64c), on the ground (0x13f65e = 0).
fn ridden(w: &World, id: MobyId) -> bool { w.hero.air_ticks == 0 && w.hero.ground_moby == Some(id) }

/// The ride timer at `t` (664 / 1293: `ticks(30)`, 1069: `ticks(90)`): the first touch plays class sound 0. Returns it.
fn ride_timer(w: &mut World, id: MobyId, t: usize, n: i32) -> i32 {
    if ridden(w, id) {
        if c::pi32(w, id, t) == 0 { w.play_sound(0, 0, id); }
        let m = w.ticks(n);
        if c::pi32(w, id, t) < m { c::set_pi32(w, id, t, m); }
    }
    c::pi32(w, id, t)
}

/// Ratchet carried: `turn_about(hero, position, old, new)` → `HeroFields::ride`.
fn carry_hero(w: &mut World, id: MobyId, old: [f32; 3], new: [f32; 3], dz: f32) {
    let h = w.hero.pos.map(|x| x.to_f32());
    let out = turn_about([h[0], h[1], h[2]], v3(w.m(id).position), old, new);
    w.hero_fields_mut().ride(h, out, dz);
}

/// The tilt from Ratchet's offset `d` along the float's rows 0 / 1 (`FastVecDot` with +0xc0 / +0xd0).
fn dots(w: &World, id: MobyId) -> (f32, f32) {
    let h = w.hero.pos.map(|x| x.to_f32());
    let m = w.m(id);
    let d = [h[0] - m.position[0], h[1] - m.position[1], h[2] - m.position[2]];
    let dot = |r: [f32; 4]| d[0] * r[0] + d[1] * r[1] + d[2] * r[2];
    (dot(m.rows[0]), dot(m.rows[1]))
}

/// The sinking float of 664 / 1293 (their shared body; `k` the class's constants; `init` = 664's first-tick setup).
fn sinking(w: &mut World, id: MobyId, k: &Consts, init: bool) {
    let (pos0, rot0) = (w.m(id).position, w.m(id).rotation);
    let z0 = pos0[2];
    let low = pf(w, id, 0x84) - k.low;
    if init && c::pi32(w, id, 0x80) == 0 {
        let r = w.m(id).rotation;
        for (i, x) in r.iter().enumerate() { set(w, id, 0x60 + 4 * i, *x); }
        for i in 0..4 { set(w, id, 0x70 + 4 * i, 0.0); }
        w.mm(id).position[2] -= 0.3;
        c::set_pi32(w, id, 0x80, 1);
        let z = w.m(id).position[2];
        set(w, id, 0x84, z);
    }
    let t = ride_timer(w, id, 0x88, 30);
    if t == 0 {
        let rest = pf(w, id, 0x84);
        let mut z = w.m(id).position[2];
        let rate = k.rise * DT;
        z += rate * (rest - z);
        let mut f = k.free;
        let mut max = f[2] * DEG * DT;
        if !init && z < rest - 2.0 {
            // 1293 / 1320 far below: one more step, the springs ×3 with max 90°·dt.
            z += rate;
            f[0] *= 3.0;
            max = DT * std::f32::consts::FRAC_PI_2;
        }
        spring(w, id, 0.0, f[0] * DEG * DT2, f[1] * SPEED, max, 0x60, 0x70);
        spring(w, id, 0.0, f[0] * DEG * DT2, f[1] * SPEED, max, 0x64, 0x74);
        if rest < z { z = rest; }
        w.mm(id).position[2] = z;
        if z0 < low && low <= z { w.play_sound(2, 0, id); }
    } else {
        c::dec_timer_pvar_i32(w, id, 0x88);
        let (a, b) = dots(w, id);
        let old = [pf(w, id, 0x60), pf(w, id, 0x64), pf(w, id, 0x68)];
        let zb = w.m(id).position[2];
        let r = k.ride;
        spring(w, id, -k.lean * b, r[0] * DEG * DT2, r[1] * SPEED, r[2] * DEG * DT, 0x60, 0x70);
        spring(w, id, k.lean * a, r[0] * DEG * DT2, r[1] * SPEED, r[2] * DEG * DT, 0x64, 0x74);
        let z = zb - k.sink * DT;
        w.mm(id).position[2] = z;
        if low < z0 && z <= low { w.play_sound(1, 0, id); }
        let new = [pf(w, id, 0x60), pf(w, id, 0x64), pf(w, id, 0x68)];
        carry_hero(w, id, old, new, z - zb);
    }
    crate::moby_update::classes::units::bob(w, id, f32::from_bits(0x3e0f_5c29), DT * std::f32::consts::FRAC_PI_6, 0x98, 0x9c);
    crate::moby_update::classes::units::wobble(w, id, f32::from_bits(0x3db2_b8c2), DT * k.wobble[0], DT * k.wobble[1], 0x90, 0x94);
    let (tx, ty) = (pf(w, id, 0x60), pf(w, id, 0x64));
    let m = w.mm(id);
    m.rotation[0] = c::add_rot(tx, m.rotation[0]);
    m.rotation[1] = c::add_rot(ty, m.rotation[1]);
    let (d, r) = (c::sub(m.position, pos0), m.rotation);
    triggers::carry_riders(&mut m.pvars, 0x20, d, rot0, r);
}

/// Level09 `0x2f86a0` (class 664; module doc).
pub fn float_664(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xa0 { w.mm(id).pvars.resize(0xa0, 0); }
    sinking(w, id, &C664, true);
}

/// Level09 `0x3091b0` (classes 1293 / 1320; module doc).
pub fn float_1293(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xa0 { w.mm(id).pvars.resize(0xa0, 0); }
    match w.m(id).state {
        0 => {
            c::set_pi32(w, id, 0x80, 1);
            let r = w.m(id).rotation;
            for (i, x) in r.iter().enumerate() { set(w, id, 0x60 + 4 * i, *x); }
            for i in 0..4 { set(w, id, 0x70 + 4 * i, 0.0); }
            let z = w.m(id).position[2];
            set(w, id, 0x84, z);
            if c::pi32(w, id, 0x8c) != -1 {
                w.mm(id).position[2] = z - 5.0;
                let h = std::f32::consts::FRAC_PI_2;
                let x = w.rng.randf(-h, h);
                w.mm(id).rotation[0] = x;
                let y = w.rng.randf(-h, h);
                w.mm(id).rotation[1] = y;
                w.mm(id).state = 1;
                return;
            }
            // STUB_printf(0x209228, spawn id, class): a float without its link.
            w.mm(id).state = 2;
        }
        1 => {
            let l = c::pi32(w, id, 0x8c);
            let alive = usize::try_from(l).ok().and_then(|m| w.table.mobys.get(m)).is_some_and(|m| m.state != 0xfe && m.state != 0xfd);
            if alive { return; }
            w.mm(id).state = 2;
        }
        2 => sinking(w, id, &C1293, false),
        _ => {}
    }
}

/// Level07 `0x3112c8` (class 1069; module doc).
pub fn float_1069(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xd0 { w.mm(id).pvars.resize(0xd0, 0); }
    let (pos0, rot0) = (w.m(id).position, w.m(id).rotation);
    let link = usize::try_from(c::pi32(w, id, 0xa0)).ok().filter(|&m| m < w.table.mobys.len());
    let pb = |w: &World, o: usize| w.m(id).pvars[o];
    if w.m(id).cmd == 2 {
        let m = w.mm(id);
        m.state = 2;
        m.cmd = 3;
        let d = m.pvars[0xbd] as i32;
        c::set_pi32(w, id, 0xac, d);
    }
    let rock = |w: &mut World| {
        let t = c::pi32(w, id, 0xac) as f32;
        let a = crate::moby_update::classes::flyer::wrap_frac(t * 0.75).sin();
        let x = c::add_rot(pf(w, id, 0xa4), a * DEG);
        w.mm(id).rotation[0] = x;
        let b = crate::moby_update::classes::flyer::wrap_frac(t * 0.89).cos();
        let y = c::add_rot(pf(w, id, 0xa8), b * DEG);
        w.mm(id).rotation[1] = y;
    };
    match w.m(id).state {
        0 => {
            c::set_pi32(w, id, 0x20, 0);
            let pv = &mut w.mm(id).pvars;
            pv[0x24] = 0;
            pv[0x25] = 0;
            pv[0x28] = 4;
            pv[0x3e..0x40].copy_from_slice(&0xdi16.to_le_bytes());
            if w.rng.rand() & 1 != 0 {
                let z = c::add_rot(w.m(id).rotation[2], std::f32::consts::PI);
                w.mm(id).rotation[2] = z;
            }
            let (x, y) = (w.m(id).rotation[0], w.m(id).rotation[1]);
            set(w, id, 0xa4, x);
            c::set_pi32(w, id, 0xc8, -1);
            set(w, id, 0xa8, y);
            w.mm(id).state = 1;
            w.mm(id).cmd = 0;
            let mission = w.m(id).mission;
            if mission != 0xff && crate::moby_update::classes::units::hints::mission_done(w, mission as i32) {
                w.mm(id).position[2] -= 5.0;
                if pb(w, 0xbc) == 0 {
                    w.delete_moby(id);
                    return;
                }
                w.mm(id).state = 5;
            }
        }
        1 => {
            if w.m(id).cmd == 1 {
                if c::pi32(w, id, 0xac) == 0 {
                    let r = w.rng.rand() % 10;
                    c::set_pi32(w, id, 0xac, r);
                }
                if pb(w, 0xbe) != 0 {
                    let g = w.m(id).group;
                    crate::moby_update::creature::attack::group_command(w, g, 2);
                }
                rock(w);
            }
        }
        2 => {
            rock(w);
            if c::dec_timer_pvar_i32(w, id, 0xac) != 0 {
                if pb(w, 0xbe) != 0 {
                    if let Some(l) = link {
                        let v = pb(w, 0xbf);
                        let lm = w.mm(l);
                        if lm.pvars.len() <= 0x19d { lm.pvars.resize(0x19e, 0); }
                        lm.pvars[0x19d] = v;
                        lm.state = 0xc;
                    }
                }
                w.mm(id).state = 3;
            }
        }
        3 => {
            let x = w.m(id).rotation[0];
            let step = (1.396_263_4 - x) * 0.05;
            w.mm(id).rotation[0] = x + step;
            w.mm(id).position[2] -= step.sin() * 3.1;
            let v = pf(w, id, 0xb0) - pf(w, id, 0xb4) * DT2;
            set(w, id, 0xb0, v);
            let z = w.m(id).position[2] + v;
            w.mm(id).position[2] = z;
            if z < pf(w, id, 0xb8) - 15.0 {
                if pb(w, 0xbc) == 0 {
                    w.delete_moby(id);
                    return;
                }
                set(w, id, 0xb0, 0.0);
                w.mm(id).state = 4;
            }
        }
        4 => {
            let wait = link.is_some_and(|l| w.m(l).pvars.get(0xef).copied().unwrap_or(0) < 2 && 0xb < w.m(l).state);
            if !wait {
                let v = w.rng.randf(DT, DT * 4.0);
                set(w, id, 0xb0, v);
                let a = w.rng.rand_angle();
                set(w, id, 0xc0, a);
                let b = w.rng.rand_angle();
                set(w, id, 0xc4, b);
                let m = w.mm(id);
                m.state = 5;
                m.rotation[0] = 0.0;
                m.rotation[1] = 0.0;
                c::set_pi32(w, id, 0xac, 0);
            }
        }
        5 => {
            let water = pf(w, id, 0xb8);
            let z1 = w.m(id).position[2] + pf(w, id, 0xb0);
            let t = ride_timer(w, id, 0xac, 90);
            if t == 0 {
                let mut v = pf(w, id, 0xb0);
                if z1 < water { v += ((water - z1) * 0.2).clamp(-0.9, 0.9); }
                v -= DT2 * 9.8;
                if DT * 5.0 < v { v = DT * 5.0; }
                set(w, id, 0xb0, v);
            } else if water - 2.6 < w.m(id).position[2] {
                set(w, id, 0xb0, -(DT * 0.25));
            } else {
                set(w, id, 0xb0, 0.0);
            }
            let v = pf(w, id, 0xb0);
            w.mm(id).position[2] += v;
            let a = c::add_rot(pf(w, id, 0xc0), 0.01);
            set(w, id, 0xc0, a);
            let b = c::add_rot(a, f32::from_bits(0x3c8b_4396));
            set(w, id, 0xc4, b);
            let mut tx = a.sin() * 0.02;
            let ty = b.sin();
            if c::pi32(w, id, 0xac) != 0 {
                c::dec_timer_pvar_i32(w, id, 0xac);
                let (da, db) = dots(w, id);
                tx = c::add_rot(c::add_rot(tx, db * -0.03), da * 0.03);
                // `0x288968` with the rotation before and after this point (both the current one): no turn, no drop.
                let r = v3(w.m(id).rotation);
                carry_hero(w, id, r, r, 0.0);
            }
            let m = w.mm(id);
            crate::moby_update::creature::turn::approach_rot(tx, DT * 0.174_532_92, &mut m.rotation[0]);
            crate::moby_update::creature::turn::approach_rot(ty * 0.02, DT * 0.174_532_92, &mut m.rotation[1]);
            let lim = f32::from_bits(0x3f06_0a92);
            m.rotation[0] = m.rotation[0].clamp(-lim, lim);
            m.rotation[1] = m.rotation[1].clamp(-lim, lim);
        }
        _ => {}
    }
    let z = w.m(id).position[2];
    if w.m(id).state == 5 && c::dec_timer_pvar_i32(w, id, 0xc8) != 0 {
        let water = pf(w, id, 0xb8);
        let last = pf(w, id, 0xcc);
        if (water <= last && z < water) || (last <= water && water < z) {
            w.play_sound(0, 0, id);
            let r = w.rng.rand();
            let n = w.ticks(20);
            c::set_pi32(w, id, 0xc8, r % n + n);
        }
    }
    set(w, id, 0xcc, z);
    let m = w.mm(id);
    let (d, r) = (c::sub(m.position, pos0), m.rotation);
    triggers::carry_riders(&mut m.pvars, 0x60, d, rot0, r);
}
