//! U164 (census 2026-10-02): class 427, Eudora's gunners (level04 0x2c4850, the only copy: 13 placed). The name is
//! descriptive [L]. Read from the level04 decomp and disassembly. Native `f32`; the `rand` draws in the game's order.
//!
//! A creature holding a gun (class 499, kept on its joint 1) inside an area (one of two path polygons, +0xe0 / +0xe4,
//! swapped after each knock). It watches Ratchet (head and body turning toward him), keeps its distance: farther than
//! 14 it runs to the area's point farthest from him, between 2.5 and 14 with a clear line it aims and fires bursts of
//! three gun shots (`gun_shot`, class 184) when the camera faces it, closer it strikes (damage 1 at key 12). Hits push
//! it back or knock it over; a kill flings it (a knock flight) and dissolves it into a cloud of puffs along its body.
//!
//! **Pvars** (0x210): header (+0x00 D, +0x0c F, +0x10 K); +0x20 D (health 2, meter 2, column 1, +0x2e 1 while alive,
//! +0x38 the lure); +0x58 / +0x5a bytes 8 / 10; +0x60 F; +0x70 K; +0xd0 the fall speed; +0xd4 the home height; +0xd8
//! last tick's key time; +0xdc the range; +0xe0 / +0xe4 the area paths; +0xe8 s16 area A in use; +0xec the turn
//! velocity; +0xf0 s16 a guard (only cleared); +0xf2 s16 the hold; +0xf4 the push-back; +0xf8 the gun (port: index + 1);
//! +0xfc byte starts aiming; +0xfd byte shots left; +0xfe byte the alert timer; +0xff byte the run sequence timer;
//! +0x108 s32 the dash cooldown; +0x110 / +0x190 the look-at records (joint lists 5 / 6; +0x68 their yaw targets).
//!
//! | address | what | port |
//! |---|---|---|
//! | top | alerted = lure or alert timer; `0x256428(3, +0x110)` the big-head scale; shadow probe within 27; the target (range ×2 alerted; distances to its +0x00, aims at its +0x20); key time | [`update`] |
//! | top | not in 0 and D+0x0e ≠ 1 → the gun and itself deleted; a path −1 → `printf`, [`dissolve`] | [`update`] |
//! | top | the area (+0xe8 ? A : B); its point farthest (xy) from the aim point: the retreat, its distance | [`update`] |
//! | hits | `MobyGetHitMessage(m, 0x210000)`, the resolver (column 4); guard and hold timers; a hit in a state below 0xb with the guard out: the heading from the attacker; health −= damage (≤ 0 → 1); alert `ticks(240)`; 1 / 2: not targetable, F colour 0xfa, K (gravity 37·dt², drag 20·dt², speed 17·dt, up 13·dt, `0x26fa48` along the push), the flight (seq 0xd), keys 12 / 25, 0xb, `SetDeathBits`; 3 / 6: colour 0x96, seq 8 (`ticks(5)`), 8, push 2/`scale(15)` (half for 0x131 / 0xb0 attackers); 4 / 5: colour 0xfa, seq 0xc, 10, push 2/`scale(15)`; then the heading and push from the hit (`0x26fa48`), yaw = heading + π; any hit: the flash | [`hits`] |
//! | 0 | targetable; D fields; home height; mirror on `rand() & 1`; +0xfc 0 → seq `(rand() ^ 1) & 1`, 1; else seq 3, 5 | [`init`] |
//! | 1 | wrapped → seq `(rand() ^ 1) & 1`; turn to the aim point (0.04, 0.3, 7.33·dt); alerted → seq 0xb (`ticks(8)`), 3; else beyond 2.5 with a retreat: beyond 14 → seq 10, 2; under 8 with the dash cooldown out → seq 0xb, 3; within 2.5: hold out → seq 7, 7, else hold ≤ `ticks(20)`; 2.5–14 and 8 in height, hold out, a clear line (`CollLine_Fix`, +0.5 up) to the target → seq 3, 5 | [`update`] |
//! | 2 | under 13 or at the retreat → seq 0, 1; else turn to the retreat (0.03, 0.3, 4.71·dt), lean (look yaws 0.7 / 0.4 of 30 × the turn velocity, ±45°), 2.5·dt ahead | [`update`] |
//! | 3 | dash cooldown `ticks(100)`; not alerted: within 1 and facing (15°) → seq 7, 7; beyond 9 → seq 3, 5; at the retreat → seq 0, 1; else turn to the retreat (0.035), 6·dt ahead; alerted: turn to the aim point (0.04, 4.89·dt), 6·dt ahead kept inside the area (seq 0xb, else 0, every `ticks(15)`); hold out: within 1 and facing → 7, under 14 → seq 3, 5 | [`dash`] |
//! | 4 | seq 2 to its end → seq 0, 1; else seq 2, fall speed −7.668·dt | [`update`] |
//! | 5 | turn to the aim point (0.03, 5.59·dt); wrapped → seq 4; not alerted, beyond 2.5 with a retreat: beyond 14 → seq 10 (`ticks(9)`), 2; under 8 and the cooldown out → seq 0xb (`ticks(5)`), 3; within 2.5 → 7; else hold out, under 14, 4 in height, in front of the camera (35°) and facing (45°) → seq 5, 6, hold `ticks(30)`, 3 shots; beyond 14 → seq 6, 1 (3 alerted), hold `ticks(30)` | [`update`] |
//! | 6 | turn (0.04, 4.71·dt); within 2.5 → 7; seq 5 crossing key 2 with the gun: a shot from its joint 0 toward the aim point (heading clamped to the yaw beyond 15°), 10·dt, class sound 7, `gun_shot::spawn` (life 14/(10·dt)), a shot less; wrapped, hold out, no shots → hold `ticks(180)`, 5, seq 4 | [`shoot`] |
//! | 7 | turn (0.04, 5.59·dt); key 12–13 of its sequence, 2 in height, within 1 and facing 15°: a hit on the target (dir (cos, sin, 1, exact), flags 1, type 0 / 1, the class, damage 1); wrapped → 5, hold `ticks(20)`, seq 4 | [`update`] |
//! | 8–10 | the push-back (`0x26d270(0.5, 1, 0.5, 30°, …, 1)`, −2/`scale(15)`² a tick); wrapped → the other area, seq 9 (in 9) else 0, 1, cooldown `ticks(120)` | [`update`] |
//! | 0xb | the knock flight; landed / wrapped → [`dissolve`] | [`update`] (`knock::update`) |
//! | tail | the flash; in 1, 3–6 the look yaws at the target (Ratchet without one) within 24: half the turn to it (±55°); mirrored: negated; `FUN_002777d8` on both records; the gun made (`0x2ce080`) or held at joint 1; the fall (9.8·dt²) onto `GroundHeight(0.5)`; 5 below home → `SetDeathBits`, [`dissolve`] | [`update`] |
//!
//! **[`dissolve`]** (`0x2c4808`): the gun deleted; 14 body segments between the points of `0x1d2c90` (turned by the
//! rows, at radius ×1.75): n type-23 puffs along each, two type-21 sparks at the ends; `DeleteMoby`.
//!
//! **The game's, kept:** a dissolve puff's size is `r_a·210000 + (r_b − r_a)/n` for every puff of a segment; mirrored, the
//! look yaws are negated every tick (outside the states that set them they flip each tick).

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::classes::units::gun_shot;
use crate::moby_update::creature::{self as c, damage, flash, fx, knock, react, region, target, turn, walker, DT, DT2};
use crate::moby_update::manip;
use crate::moby_update::services::{self as sv, pf, pv, HitTemplate, World};
use crate::ps2v::Pf;
use std::f32::consts::{FRAC_PI_4, FRAC_PI_6, PI};

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x2c_4850;
pub const CLASSES: [i16; 1] = [427];
/// The gun (`0x2ce080`).
pub const GUN: i16 = 499;

mod p {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const F: usize = 0x60;
    pub const K: usize = 0x70;
    pub const FALL: usize = 0xd0;
    pub const HOME_Z: usize = 0xd4;
    pub const LAST_KT: usize = 0xd8;
    pub const RANGE: usize = 0xdc;
    pub const AREA_A: usize = 0xe0;
    pub const AREA_B: usize = 0xe4;
    pub const USE_A: usize = 0xe8;
    pub const TURN_V: usize = 0xec;
    pub const GUARD: usize = 0xf0;
    pub const HOLD: usize = 0xf2;
    pub const PUSH: usize = 0xf4;
    pub const GUN: usize = 0xf8;
    pub const AIMS: usize = 0xfc;
    pub const SHOTS: usize = 0xfd;
    pub const ALERT: usize = 0xfe;
    pub const RUN_T: usize = 0xff;
    pub const DASH: usize = 0x108;
    pub const LOOK_A: usize = 0x110;
    pub const LOOK_B: usize = 0x190;
    pub const SIZE: usize = 0x210;
}

/// The body points (x, y, z, radius) of `0x1d2c90` (level04 data).
const POINTS: [[f32; 4]; 24] = [
    [0.0, 0.0, 0.187, 0.172], [-0.545, 0.0, 0.3, 0.2], [-0.583, 0.0, 0.599, 0.115], [-0.865, 0.0, 0.295, 0.115],
    [0.0, -0.095, 0.176, 0.099], [0.207, -0.144, 0.175, 0.117], [0.524, -0.211, 0.193, 0.19], [0.628, -0.236, 0.412, 0.117],
    [0.0, 0.095, 0.176, 0.099], [0.207, 0.122, 0.175, 0.117], [0.524, 0.174, 0.193, 0.19], [0.588, 0.195, 0.452, 0.117],
    [-0.525, -0.233, 0.308, 0.128], [0.0, -0.664, 0.062, 0.21], [-0.525, 0.256, 0.308, 0.128], [0.078, 0.528, 0.062, 0.21],
    [-0.499, -0.215, 0.088, 0.142], [-0.844, -0.183, 0.088, 0.142], [-0.869, -0.198, 0.128, 0.066], [-1.39, -0.2, 0.087, 0.066],
    [-0.499, 0.155, 0.088, 0.142], [-0.786, 0.183, 0.088, 0.142], [-0.886, 0.215, 0.128, 0.066], [-1.757, 0.266, 0.048, 0.066],
];
/// `0x2c46d0`'s segments: (point a, point b, puffs; gp−0x5340..−0x5324).
const SEGMENTS: [(usize, usize, i32); 14] =
    [(0, 1, 10), (2, 3, 10), (4, 5, 10), (5, 6, 10), (6, 7, 8), (8, 9, 10), (9, 10, 10), (10, 11, 8), (12, 13, 20), (14, 15, 20), (16, 17, 8), (20, 21, 8), (18, 19, 20), (22, 23, 20)];

fn st(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_st(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) {
    let t = w.ticks(t);
    w.anim_blend(id, seq, 0, t);
}
fn hold(w: &World, id: MobyId) -> i16 { c::pi16(w, id, p::HOLD) }
fn set_hold(w: &mut World, id: MobyId, t: i32) {
    let t = w.ticks(t);
    c::set_pi16(w, id, p::HOLD, t as i16);
}
fn gscale(w: &World, x: f32) -> f32 { sv::fl(w.svc.timing.scale(Pf::f(x))) }
fn heading_to(a: c::V, b: c::V) -> f32 { c::atan(b[0] - a[0], b[1] - a[1]) }
fn yaw(w: &World, id: MobyId) -> f32 { w.m(id).rotation[2] }
fn turn_to(w: &mut World, id: MobyId, at: c::V, k: f32, max: f32) {
    let a = heading_to(c::pos(w, id), at);
    turn::spring_turn2_pvar(w, id, a, k, 0.3, DT * max, p::TURN_V);
}
fn facing(w: &World, id: MobyId, at: c::V, lim: f32) -> bool { c::diff_rots(heading_to(c::pos(w, id), at), yaw(w, id)) < lim }
fn gun(w: &World, id: MobyId) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, p::GUN) - 1).ok().filter(|&g| g < w.table.mobys.len())
}
fn ahead(w: &World, id: MobyId, s: f32) -> c::V {
    let me = c::pos(w, id);
    let a = yaw(w, id);
    [me[0] + a.cos() * s, me[1] + a.sin() * s, me[2], me[3]]
}
/// `0x26d270(0.5, 1, 0.5, 30°, m, pos, to, flags)` on the moby's own position.
fn step(w: &mut World, id: MobyId, to: c::V, flags: u32) -> i32 {
    let mut from = c::pos(w, id);
    let mut to = to;
    let r = walker::move_ground(w, id, 0.5, 1.0, 0.5, FRAC_PI_6, &mut from, &mut to, flags);
    let m = w.mm(id);
    m.position[0] = from[0];
    m.position[1] = from[1];
    r
}

/// The facts of this tick (the top of the update).
struct Tick {
    alert: bool,
    t: target::Target,
    /// The xy distance to the target; the aim point (target +0x20); the key time.
    d: f32,
    aim: c::V,
    kt: f32,
    /// The area path; the retreat point and its xy distance.
    area: usize,
    retreat: c::V,
    rd: f32,
}

/// `0x2c4808`: the gun deleted, the body dissolved (module doc), `DeleteMoby`.
fn dissolve(w: &mut World, id: MobyId) {
    if let Some(g) = gun(w, id) { w.delete_moby(g); }
    let (pos, rows) = (c::pos(w, id), w.m(id).rows);
    let at = |q: [f32; 4]| -> c::V { std::array::from_fn(|k| if k == 3 { pos[3] + 1.0 } else { rows[0][k] * q[0] + rows[1][k] * q[1] + rows[2][k] * q[2] + pos[k] }) };
    for (a, b, n) in SEGMENTS {
        segment(w, at(POINTS[a]), at(POINTS[b]), POINTS[a][3] * 1.75, POINTS[b][3] * 1.75, n);
    }
    w.delete_moby(id);
}

/// `0x2c41e0(r_a, r_b, m, a, b, n)` (module doc).
fn segment(w: &mut World, a: c::V, b: c::V, ra: f32, rb: f32, n: i32) {
    let d = c::scale(c::sub(b, a), 1.0 / n as f32);
    let size = ra * 210_000.0 + (rb - ra) / n as f32;
    for i in 0..n {
        let at = c::add(a, c::scale(d, i as f32));
        let vx = w.rng.randf_sym(0.0, 0.005);
        let vy = w.rng.randf_sym(0.0, 0.005);
        let vz = w.rng.randf(0.005 * 0.1, 0.005);
        let alpha = w.rng.rand_range(0x40, 0x70) as u32;
        let g = w.rng.rand_range(0x40, 0x7f) as u32;
        let hi = w.rng.randf(1.0, f32::from_bits(0x3f82_8f5c));
        let spin = w.rng.rand_range(-2, 2);
        let grey = |g: u32| alpha << 24 | g | g << 8 | g << 16;
        let Some(sys) = w.particles.as_deref_mut() else {
            fx::part_unported(w, 23);
            continue;
        };
        *w.svc.fx.part_spawns.entry(23).or_default() += 1;
        let Some(k) = crate::particles::type23::spawn(sys, w.rng, 0.1, 1.0, hi, size, at, spin, [vx, vy, vz, 1.0], grey(g)) else {
            w.svc.fx.part_failed += 1;
            continue;
        };
        let recolour = if w.rng.randi(2) != 0 { Some(w.rng.rand_range(0x60, 0xe0) as u32) } else { None };
        let t90 = w.ticks(90);
        let r = &mut w.particles.as_deref_mut().unwrap().pool.recs[k];
        use crate::particles::rec;
        if let Some(g) = recolour {
            r[3] = 0x7e;
            rec::set_u32(r, 4, grey(g));
        }
        rec::set_i16(r, 0xa, t90 as i16);
        r[0x2a] = alpha as u8;
        rec::set_u32(r, 0x24, 2);
        r[0x2b] = t90 as u8;
    }
    for (at, size) in [(a, 10000.0), (b, 20000.0)] {
        let x = w.rng.rand_angle().cos() * 0.05;
        let y = w.rng.rand_angle().sin() * 0.05;
        let z = w.rng.randf(0.01, f32::from_bits(0x3cf5_c28f));
        let (lo, hi) = (w.ticks(10), w.ticks(20));
        let life = w.rng.rand_range(lo, hi);
        fx::part21(w, size, at, [x, y, z, 1.0], 0x4f00_7fff, 0x1fff_ffff, life, 1);
    }
}

/// The hits (module doc).
fn hits(w: &mut World, id: MobyId) {
    let hit = w.get_hit(id, 0x21_0000, false);
    let res = damage::resolve(w, id, hit, p::D, 0, 4);
    c::dec_timer_pvar_s16(w, id, p::GUARD);
    c::dec_timer_pvar_s16(w, id, p::HOLD);
    let Some(h) = hit else { return };
    if 0xb <= st(w, id) || c::pi16(w, id, p::GUARD) != 0 { return; }
    // The heading from the attacker (`FastArcTan(pos − attacker)`) is a dead store: `0x26fa48` below replaces it.
    let mut up = 1.0;
    let hp = c::pf(w, id, p::D) - res.damage;
    c::set_pf(w, id, p::D, hp);
    let reaction = if hp <= 0.0 { 1 } else { res.reaction };
    let t = w.ticks(240);
    c::set_pu8(w, id, p::ALERT, t as u8);
    let dir = h.dir.map(|x| f32::from_bits(x.0));
    let class = h.attacker.filter(|&a| a < w.table.mobys.len()).map(|a| w.m(a).o_class);
    let push_back = |w: &mut World, half: bool| {
        let k = 2.0 / gscale(w, 15.0);
        c::set_pf(w, id, p::PUSH, if half { k * 0.5 } else { k });
    };
    match reaction {
        1 | 2 => {
            w.mm(id).mode &= !mode::TARGETABLE;
            c::set_pu8(w, id, p::F + 7, 0xfa);
            let k = p::K;
            c::set_pf(w, id, k + knock::k::DRAG, DT2 * 20.0);
            c::set_pf(w, id, k + knock::k::UP, DT * 13.0);
            c::set_pf(w, id, k + knock::k::GRAVITY, DT2 * 37.0);
            c::set_pf(w, id, k + knock::k::SPEED, DT * 17.0);
            let (mut sp, mut u) = (c::pf(w, id, k + knock::k::SPEED), c::pf(w, id, k + knock::k::UP));
            let a = knock::aim(dir, &mut sp, &mut u);
            c::set_pf(w, id, k + knock::k::SPEED, sp);
            c::set_pf(w, id, k + knock::k::UP, u);
            knock::start(w, id, k, a, 0xd, 1, 0);
            c::set_pf(w, id, k + knock::k::KEY_APEX, 12.0);
            c::set_pf(w, id, k + knock::k::KEY_LAND, 25.0);
            set_st(w, id, 0xb);
            set_death_bits(w, id, 0, -1);
        }
        r @ (3..=6) => {
            if r == 3 || r == 6 {
                c::set_pu8(w, id, p::F + 7, 0x96);
                blend(w, id, 8, 5);
                set_st(w, id, 8);
                push_back(w, matches!(class, Some(0x131) | Some(0xb0)));
            } else {
                c::set_pu8(w, id, p::F + 7, 0xfa);
                blend(w, id, 0xc, 5);
                set_st(w, id, 10);
                push_back(w, false);
            }
            let mut sp = c::pf(w, id, p::PUSH);
            let heading = knock::aim(dir, &mut sp, &mut up);
            c::set_pf(w, id, p::PUSH, sp);
            w.mm(id).rotation[2] = c::add_rot(PI, heading);
        }
        _ => {}
    }
    flash::start(w, id, p::F);
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    w.mm(id).mode |= mode::TARGETABLE;
    let d = p::D;
    c::set_pf(w, id, d, 2.0);
    c::set_pi16(w, id, d + 4, 2);
    c::set_pu8(w, id, d + 8, 1);
    c::set_pu8(w, id, d + 0xe, 1);
    c::set_pu8(w, id, d + 9, 1);
    c::set_pi16(w, id, p::GUARD, 0);
    c::set_pu8(w, id, p::ALERT, 0);
    c::set_pu8(w, id, p::RUN_T, 0);
    c::set_pf(w, id, p::FALL, 0.0);
    let z = w.m(id).position[2];
    c::set_pu8(w, id, 0x58, 8);
    c::set_pf(w, id, p::HOME_Z, z);
    c::set_pu8(w, id, 0x5a, 10);
    if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
    if c::pu8(w, id, p::AIMS) == 0 {
        let s = ((w.rng.rand() ^ 1) & 1) as u8;
        blend(w, id, s, 10);
        set_st(w, id, 1);
    } else {
        blend(w, id, 3, 10);
        set_st(w, id, 5);
    }
}

/// State 3 (module doc).
fn dash(w: &mut World, id: MobyId, k: &Tick) {
    let t = w.ticks(100);
    c::set_pi32(w, id, p::DASH, t);
    if !k.alert {
        if k.d < 1.0 && facing(w, id, k.aim, 0.261_799_4) {
            set_st(w, id, 7);
            blend(w, id, 7, 7);
        } else if 9.0 < k.d {
            set_st(w, id, 5);
            blend(w, id, 3, 7);
        } else if k.rd < 1.0 {
            blend(w, id, 0, 10);
            set_st(w, id, 1);
        } else {
            turn::spring_turn2_pvar(w, id, heading_to(c::pos(w, id), k.retreat), 0.035, 0.3, DT * 4.712_389, p::TURN_V);
            let to = ahead(w, id, DT * 6.0);
            step(w, id, to, 0);
        }
        return;
    }
    turn_to(w, id, k.aim, 0.04, 4.886_922);
    let mut from = c::pos(w, id);
    let mut to = ahead(w, id, DT * 6.0);
    let ok = walker::move_ground(w, id, 0.5, 1.0, 0.5, FRAC_PI_6, &mut from, &mut to, 0);
    let mut seq = 0;
    if ok != 0 && region::point_in_polygon(w, k.area, from) {
        let m = w.mm(id);
        m.position[0] = from[0];
        m.position[1] = from[1];
        m.position[2] = from[2];
        seq = 0xb;
    }
    let mut rt = c::pu8(w, id, p::RUN_T);
    let out = sv::fast_dec_timer_u8(&mut rt) != 0;
    c::set_pu8(w, id, p::RUN_T, rt);
    if out && seq != w.m(id).anim.seq_b {
        blend(w, id, seq, 10);
        let t = w.ticks(15);
        c::set_pu8(w, id, p::RUN_T, t as u8);
    }
    if hold(w, id) != 0 { return; }
    if k.d < 1.0 && facing(w, id, k.aim, 0.261_799_4) {
        set_st(w, id, 7);
        blend(w, id, 7, 7);
    } else if k.d < 14.0 {
        set_st(w, id, 5);
        blend(w, id, 3, 7);
    } else {
        return;
    }
    c::set_pu8(w, id, p::RUN_T, 0);
}

/// State 6 (module doc).
fn shoot(w: &mut World, id: MobyId, k: &Tick) {
    turn_to(w, id, k.aim, 0.04, 4.712_389);
    if k.d < 2.5 {
        set_st(w, id, 7);
        blend(w, id, 7, 7);
    }
    let crossed = w.m(id).anim.seq_b == 5 && c::pf(w, id, p::LAST_KT) < 2.0 && 2.0 <= k.kt;
    if let Some(g) = gun(w, id).filter(|_| crossed) {
        let p0 = w.joint_point(g, 0);
        let v = c::sub(k.aim, p0);
        let mut a = c::atan(v[0], v[1]);
        if 0.261_799_4 < c::diff_rots(a, yaw(w, id)) { a = yaw(w, id); }
        let e = c::atan(c::len2(v), v[2]);
        let vel = fx::polar(DT * 10.0, a, e);
        w.play_sound(7, 0, id);
        let life = (14.0 / (DT * 10.0)) as i32;
        gun_shot::spawn(w, id, p0, vel, life);
        let mut s = c::pu8(w, id, p::SHOTS);
        sv::fast_dec_timer_u8(&mut s);
        c::set_pu8(w, id, p::SHOTS, s);
        return;
    }
    if wrapped(w, id) && hold(w, id) == 0 && c::pu8(w, id, p::SHOTS) == 0 {
        set_hold(w, id, 180);
        set_st(w, id, 5);
        blend(w, id, 4, 10);
    }
}

/// The retreat in states 1 and 5: beyond 14 → flee (seq 10, 2); under 8 with the cooldown out → dash (seq 0xb, 3).
fn retreat_or_dash(w: &mut World, id: MobyId, k: &Tick, t_flee: i32, t_dash: i32) -> bool {
    if k.alert || k.d <= 2.5 || k.rd <= 1.0 { return false; }
    if 14.0 < k.d {
        blend(w, id, 10, t_flee);
        set_st(w, id, 2);
        return true;
    }
    if k.d < 8.0 && c::pi32(w, id, p::DASH) == 0 {
        blend(w, id, 0xb, t_dash);
        set_st(w, id, 3);
        return true;
    }
    false
}

/// The state machine (module doc).
fn states(w: &mut World, id: MobyId, k: &Tick) -> bool {
    let dz = (w.m(id).position[2] - k.t.pos[2]).abs();
    match st(w, id) {
        0 => init(w, id),
        1 => {
            if wrapped(w, id) {
                let s = ((w.rng.rand() ^ 1) & 1) as u8;
                blend(w, id, s, 10);
            }
            turn_to(w, id, k.aim, 0.04, 7.330_383);
            if k.alert {
                blend(w, id, 0xb, 8);
                set_st(w, id, 3);
            } else if retreat_or_dash(w, id, k, 8, 8) {
            } else if 14.0 <= k.d || k.d < 2.5 || 8.0 <= dz {
                if k.d <= 2.5 {
                    if hold(w, id) < 1 {
                        set_st(w, id, 7);
                        blend(w, id, 7, 7);
                    } else if w.ticks(20) < hold(w, id) as i32 {
                        set_hold(w, id, 20);
                    }
                }
            } else if hold(w, id) == 0 {
                let me = c::pos(w, id);
                let a = [me[0], me[1], me[2] + 0.5, me[3]];
                let b = [k.aim[0], k.aim[1], k.aim[2] + 0.5, k.aim[3]];
                if w.coll_line(pv(a), pv(b), 0, Some(id)).is_some_and(|o| o.moby == k.t.moby) {
                    set_st(w, id, 5);
                    blend(w, id, 3, 7);
                }
            }
        }
        2 => {
            if k.d < 13.0 || k.rd < 1.0 {
                blend(w, id, 0, 10);
                set_st(w, id, 1);
            } else {
                turn::spring_turn2_pvar(w, id, heading_to(c::pos(w, id), k.retreat), 0.03, 0.3, DT * 4.712_389, p::TURN_V);
                let lean = (c::pf(w, id, p::TURN_V) * 30.0).clamp(-FRAC_PI_4, FRAC_PI_4);
                c::set_pf(w, id, p::LOOK_A + 0x68, lean * 0.7);
                c::set_pf(w, id, p::LOOK_B + 0x68, lean * 0.4);
                let to = ahead(w, id, DT * 2.5);
                step(w, id, to, 0);
            }
        }
        3 => dash(w, id, k),
        4 => {
            if w.m(id).anim.seq_b == 2 {
                if wrapped(w, id) {
                    blend(w, id, 0, 10);
                    set_st(w, id, 1);
                }
            } else {
                blend(w, id, 2, 7);
                c::set_pf(w, id, p::FALL, -(DT * 7.668));
            }
        }
        5 => {
            turn_to(w, id, k.aim, 0.03, 5.585_053_4);
            if wrapped(w, id) && w.m(id).anim.seq_a != 4 { blend(w, id, 4, 7); }
            if retreat_or_dash(w, id, k, 9, 5) {
            } else if k.d < 2.5 {
                set_st(w, id, 7);
                blend(w, id, 7, 7);
            } else {
                let cam = w.camera.map(|x| x.to_f32());
                let me = c::pos(w, id);
                let seen = c::diff_rots(w.camera_yaw, c::atan(me[0] - cam[0], me[1] - cam[1])) < 0.610_865_24;
                if hold(w, id) == 0 && k.d < 14.0 && dz < 4.0 && seen && c::diff_rots(yaw(w, id), heading_to(me, k.aim)) < FRAC_PI_4 {
                    set_st(w, id, 6);
                    blend(w, id, 5, 7);
                    set_hold(w, id, 30);
                    c::set_pu8(w, id, p::SHOTS, 3);
                } else if 14.0 < k.d {
                    set_st(w, id, if k.alert { 3 } else { 1 });
                    blend(w, id, 6, 10);
                    set_hold(w, id, 30);
                }
            }
        }
        6 => shoot(w, id, k),
        7 => {
            turn_to(w, id, k.aim, 0.04, 5.585_053_4);
            let a = &w.m(id).anim;
            if a.seq_a == a.seq_b && 12.0 <= k.kt && k.kt < 13.0 && dz < 2.0 && c::dist2(c::pos(w, id), k.aim) < 1.0 && facing(w, id, k.aim, 0.261_799_4) {
                if let Some(t) = k.t.moby {
                    let y = yaw(w, id);
                    let tmpl = HitTemplate {
                        dir: [pf(y.cos()), pf(y.sin()), Pf::ONE, Pf::b(0x45af_df66)],
                        attacker: Some(id),
                        flags: 1,
                        b18: 0,
                        b19: 1,
                        h1a: w.m(id).o_class as u16,
                        damage: Pf::ONE,
                        w20: 0,
                    };
                    w.deliver_hit(t, &tmpl);
                }
            }
            if wrapped(w, id) {
                set_st(w, id, 5);
                set_hold(w, id, 20);
                blend(w, id, 4, 7);
            }
        }
        8..=10 => {
            let push = c::pf(w, id, p::PUSH);
            if 0.0 < push {
                let a = c::add_rot(yaw(w, id), PI);
                let me = c::pos(w, id);
                let to = [me[0] + a.cos() * push, me[1] + a.sin() * push, me[2], me[3]];
                let s = gscale(w, 15.0);
                c::set_pf(w, id, p::PUSH, push + -2.0 / (s * s));
                step(w, id, to, 1);
            }
            if wrapped(w, id) {
                let u = c::pi16(w, id, p::USE_A) ^ 1;
                c::set_pi16(w, id, p::USE_A, u);
                let s = if st(w, id) == 9 { 9 } else { 0 };
                blend(w, id, s, 9);
                set_st(w, id, 1);
                let t = w.ticks(120);
                c::set_pi32(w, id, p::DASH, t);
            }
        }
        0xb if knock::update(w, id, p::K) & 0x60 != 0 => {
            dissolve(w, id);
            return true;
        }
        _ => {}
    }
    false
}

/// The gun on joint 1 (module doc).
fn hold_gun(w: &mut World, id: MobyId) { hold_gun_at(w, id, p::GUN); }

/// The gun (class 499, the moby + 1 at pvar `ofs`) on joint 1: created there the first time (level12's copy
/// `0x2f1118`), then carried by the joint and hidden with its holder (level12 `0x2e8720` with `0x27b9c0`);
/// Hoven's soldiers 294 hold the same gun.
pub fn hold_gun_at(w: &mut World, id: MobyId, ofs: usize) {
    let linked = usize::try_from(c::pi32(w, id, ofs) - 1).ok().filter(|&g| g < w.table.mobys.len());
    let Some(g) = linked else {
        let at = w.joint_point(id, 1);
        let Some(g) = w.create_moby(GUN) else { return };
        let (dd, yaw, light, ambient) = { let m = w.m(id); (m.draw_dist, m.rotation[2], m.light, m.ambient) };
        let m = w.mm(g);
        m.update_dist = 0xff;
        m.visible = 1;
        m.draw_dist = dd;
        m.rotation = [0.0, 0.0, yaw, 0.0];
        m.light = light;
        m.ambient = ambient;
        m.position = at;
        w.build_matrix(g);
        c::set_pi32(w, id, ofs, g as i32 + 1);
        return;
    };
    let mtx = w.joint_matrix(id, 1);
    w.mm(g).position = mtx[3];
    crate::moby_update::anim_sound::advance(w, g);
    let mut rows = [mtx[0].map(f32::to_bits), mtx[1].map(f32::to_bits), mtx[2].map(f32::to_bits)];
    rc_formats::moby_anim::normalise_columns(&mut rows);
    let hidden = w.m(id).mode & mode::HIDDEN != 0;
    {
        let m = w.mm(g);
        for (row, r) in m.rows.iter_mut().zip(rows) { *row = r.map(f32::from_bits); }
        if hidden { m.mode |= 0x41; } else { m.mode &= !0x41; }
    }
    react::sphere_lerp(w, g);
    w.mm(g).mode |= 6;
}

/// Level04 0x2c4850 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < p::SIZE { return; }
    let alert = c::pi32(w, id, p::LURE) != 0 || c::pu8(w, id, p::ALERT) != 0;
    manip::big_head_scale(w, 3.0, id, p::LOOK_A);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| x.to_f32());
        if c::dist3(c::pos(w, id), cam) < 27.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x15;
        }
    }
    let r = c::pf(w, id, p::RANGE);
    let t = target::acquire(w, id, if alert { r + r } else { r });
    let d = c::dist2(c::pos(w, id), t.pos);
    let kt = c::ground::key_time(w, id);
    if st(w, id) != 0 && c::pu8(w, id, p::D + 0xe) != 1 {
        if let Some(g) = gun(w, id) { w.delete_moby(g); }
        w.delete_moby(id);
        return;
    }
    let (a, b) = (c::pi32(w, id, p::AREA_A), c::pi32(w, id, p::AREA_B));
    let path = if c::pi16(w, id, p::USE_A) != 0 { a } else { b };
    let area = usize::try_from(path).ok().filter(|&s| s < w.svc.splines.len());
    let Some(area) = area.filter(|_| a != -1 && b != -1) else {
        // The game prints the missing path and dissolves it.
        return dissolve(w, id);
    };
    let aim = t.aim;
    let mut best: Option<usize> = None;
    let mut far = 0.0;
    for (i, q) in w.svc.splines[area].iter().enumerate() {
        let q = q.map(f32::from_bits);
        let e = c::dist2(aim, q);
        if far < e {
            far = e;
            best = Some(i);
        }
    }
    let me = c::pos(w, id);
    let (retreat, rd) = match best {
        Some(i) => {
            let q = w.svc.splines[area][i].map(f32::from_bits);
            (q, c::dist2(q, me))
        }
        None => (me, 0.0),
    };
    let tick = Tick { alert, t, d, aim, kt, area, retreat, rd };
    hits(w, id);
    if w.m(id).state >= 0xfd { return; }
    w.mm(id).hit_slot = 0xff;
    if c::pi32(w, id, p::LURE) != 0 {
        let t = w.ticks(240);
        c::set_pu8(w, id, p::ALERT, t as u8);
    }
    c::set_pi32(w, id, p::LURE, 0);
    let mut al = c::pu8(w, id, p::ALERT);
    sv::fast_dec_timer_u8(&mut al);
    c::set_pu8(w, id, p::ALERT, al);
    c::dec_timer_pvar_i32(w, id, p::DASH);
    if states(w, id, &tick) { return; }
    flash::update(w, id, p::F);
    if matches!(st(w, id), 1 | 3..=6) {
        let tgt = t.moby.or(w.hero_moby);
        if let Some(q) = tgt.map(|m| w.m(m).position) {
            let me = c::pos(w, id);
            if c::dist2(me, q) < 24.0 {
                let rel = c::sub_rot(heading_to(me, q), yaw(w, id)).clamp(-0.959_931_1, 0.959_931_1);
                c::set_pf(w, id, p::LOOK_B + 0x68, rel * 0.5);
                c::set_pf(w, id, p::LOOK_A + 0x68, rel * 0.5);
            }
        }
    }
    if w.m(id).mode & mode::MIRROR != 0 {
        for o in [p::LOOK_A, p::LOOK_B] {
            let v = c::pf(w, id, o + 0x68);
            c::set_pf(w, id, o + 0x68, -v);
        }
    }
    manip::look(w, id, id, p::LOOK_A, 5, 0.018, 0.3);
    manip::look(w, id, id, p::LOOK_B, 6, 0.024, 0.3);
    hold_gun(w, id);
    let v = c::pf(w, id, p::FALL) + DT2 * 9.8;
    c::set_pf(w, id, p::FALL, v);
    w.mm(id).position[2] -= v - 2.0;
    let gz = c::ground::ground(w, c::pos(w, id), 0.5, 0).z;
    let z = w.m(id).position[2] - 2.0;
    w.mm(id).position[2] = z;
    if z < gz {
        w.mm(id).position[2] = gz;
        c::set_pf(w, id, p::FALL, 0.0);
    }
    if c::pf(w, id, p::HOME_Z) - 5.0 <= w.m(id).position[2] {
        c::set_pf(w, id, p::LAST_KT, kt);
        return;
    }
    set_death_bits(w, id, 0, -1);
    dissolve(w, id);
}
