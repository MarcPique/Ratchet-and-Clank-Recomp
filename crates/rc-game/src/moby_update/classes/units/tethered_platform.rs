//! Gaspar's tethered platforms, classes 1182–1189 (level 09, 80 instances): level09 0x2c26c8 with its private piece
//! spawn 0x2c2500 and search 0x2c2610 (census unit U294). The core 1184 (0x4a0) floats over the lava (a bob and a
//! tilt wobble); its pieces 1182, 1183, 1185–1189 hang on it (placed, or made by the core at init), following its
//! joint-0 point and rotation. When the core is told to break (+0xbc = 1: the chain links 1181 that tether it tell
//! it, `super::chain_link`) the whole platform shakes loose, explodes, and every piece tumbles away from Ratchet into
//! the lava, splashing. Read from the level09 decomp and disassembly; the tuning constants are the level's `$gp` words
//! (gp 0x166c00, cited by address). The name is descriptive [L]. Native `f32`.
//!
//! **Pvar block** (P, bytes): +0x00 velocity, +0x10 the spawn point (made pieces), +0x20 / +0x24 spin x / y per tick,
//! +0x28 s16 "make the pieces" (the core), +0x2a s16 timer, +0x2c the core (a piece; the port stores `id + 1`, 0 none),
//! +0x30 the joint-0 offset in the moby's frame, +0x40 / +0x44 the bob's phase and last offset, +0x48 / +0x4c the bob's
//! amplitude and rate (°/s), +0x50 / +0x54 the wobble's phases, +0x58 / +0x5c / +0x60 its amplitude (°) and rates
//! (°/s), +0x64 the fall's gravity, +0x68 i32 a global flag (−1 none), +0x6c.. the core's pieces (9 slots).
//!
//! * **0** init: when the flag `0x13d3c1[+0x68]` is set the moby is deleted. Draw distance 0x3ff. The core with +0x28:
//!   draw distance 0xff, core none, and one piece of every class 1182..1189 but itself made at its joint-0 point
//!   (0x2c2500: update distance / draw distance 0xff, state 1, drawn, the spawn point, the core, placed so its own
//!   joint-0 point lands there under the core's rows, the core's rotation and rows). A placed piece takes the nearest
//!   1184 within 20 as its core and enters itself in the core's slot `class − 1182` (none: the game's debug print).
//!   Then +0x30 = the joint-0 translation · scale/1024 (`fun_00210850` list 0, `VecScale`); → **1**.
//! * **1** / **2**: the core bobs (`0x277a00`: +0x48, +0x4c·°·dt) and wobbles (`0x277a80`: +0x58°, +0x5c / +0x60 °·dt).
//!   A piece whose core is not in state 3 takes its rotation and sits at `joint0(core) − R(rotation)·(+0x30)`. A piece
//!   with a core copies its hidden bit (mode 1) and its +0xbc. In **2** the core also turns (x) 1°·dt; when the timer
//!   ends → **3**. In **1**, told to break (+0xbc = 1): update distance 0xff, timer `rand_range(ticks(10), ticks(50))`,
//!   the core's pieces' update distances 0xff, → **2**.
//! * **3**: global flag 0x13d3c0 = 1 and the moby's own flag byte = 1 (`0x13d388` writes); `SetDeathBits(0x200)`; the
//!   death explosion `0x273f50(4, 13, moby, pos, −1)`; the core plays class sound 2; velocity = (pos − joint0)·3·dt/20
//!   plus (joint0 − Ratchet) at 3·dt (3.3·dt for 1185 / 1186), z 0; spins `randf(∓18°·dt)`; timer
//!   `randf(ticks(90), ticks(150))`; gravity `randf(4.2, 6.2)·dt²` (1185 / 1186: spin x = 90° over the timer, gravity
//!   `randf(1, 2.5)·dt²`); collision off; → **4**.
//! * **4** falling: velocity z −= gravity, pos += velocity, spins. Out of the box [12, 1011]³: deleted. One tick in
//!   three (`randi(3) = 0`) a smoke spark (`PartType11Spawn(4e5, randf(8, 16)·dt, pos, 0, 0x2f3f3f7f, 0x2f000000 /
//!   0x3f000000)`). Below z 25 (the lava): 50 splash blobs (`PartType02Spawn`: within ±3 in x / y, velocities spread
//!   0.016 / 0.001 a blob around a random heading, sizes 2 / 6, colours 0x600080e9 / 0x400040e9, phases
//!   `randf(n, 2n)` for 30, 30, 16), velocity ·0.45, spins ·0.9, class sound 0, → **5**.
//! * **5** sinking: velocity z −= 10.8·dt²/3, pos += velocity, spins; out of the box: deleted.
//!
//! **Quirks.** The splash blobs' two velocity vectors are stack words the loop only adds to: the port starts them at
//! what this tick's spark left there (0 and the colour pair `0x2f000000, 0x3f000000` read as floats) or 0 when no
//! spark was made (the game's leftover stack [L]). The piece slots are read only within the pvar block.

#![allow(clippy::needless_range_loop, clippy::eq_op)] // lane loops; `x / x` is the game's (1, or NaN for 0).

use super::{bob, hero_pos, wobble};
use crate::follow_camera::script::euler_rows;
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, DT, DT2};
use crate::moby_update::interact::set_global_flag;
use crate::moby_update::services::{pf, pv, World};

pub const UPDATE_FN: u32 = 0x2c_26c8;
pub const REFERENCE_LEVEL: u32 = 9;
pub const CORE: i16 = 1184;
pub const FIRST: i16 = 1182;
pub const CLASSES: [i16; 8] = [1182, 1183, 1184, 1185, 1186, 1187, 1188, 1189];
/// The joint-0 points the port reads.
pub const JOINTS: [i16; 8] = CLASSES;

pub mod pv_ {
    pub const VEL: usize = 0x00;
    pub const SPAWN: usize = 0x10;
    pub const SPIN_X: usize = 0x20;
    pub const SPIN_Y: usize = 0x24;
    pub const MAKE: usize = 0x28;
    pub const TIMER: usize = 0x2a;
    pub const CORE: usize = 0x2c;
    pub const OFFSET: usize = 0x30;
    pub const BOB_PHASE: usize = 0x40;
    pub const BOB_PREV: usize = 0x44;
    pub const BOB_AMP: usize = 0x48;
    pub const BOB_RATE: usize = 0x4c;
    pub const WOB_A: usize = 0x50;
    pub const WOB_B: usize = 0x54;
    pub const WOB_AMP: usize = 0x58;
    pub const WOB_RATE_A: usize = 0x5c;
    pub const WOB_RATE_B: usize = 0x60;
    pub const GRAVITY: usize = 0x64;
    pub const FLAG: usize = 0x68;
    pub const SLOTS: usize = 0x6c;
    pub const LEN: usize = 0x6c;
}
use pv_ as o;

/// π/180 as loaded (0x3c8efa35).
const DEG: f32 = 0.017_453_292;
/// The level's tuning words (`$gp`, level09 0x161368..0x1613d4).
mod k {
    pub const SPLASH_N: i32 = 50; // 0x161368
    pub const SPLASH_C1: u32 = 0x6000_80e9; // 0x16136c
    pub const SPLASH_C2: u32 = 0x4000_40e9; // 0x161370
    pub const SPLASH_V1Z: f32 = 1.0; // 0x161374 (·dt)
    pub const SPLASH_V2Z: f32 = -1.0; // 0x161378 (·dt)
    pub const SPLASH_V1XY: f32 = 0.016; // 0x16137c
    pub const SPLASH_V2XY: f32 = 0.001; // 0x161380
    pub const SPLASH_T: [i32; 3] = [30, 30, 16]; // 0x161384..0x16138c
    pub const SPLASH_S1: f32 = 2.0; // 0x161390
    pub const SPLASH_S2: f32 = 6.0; // 0x161394
    pub const GRAV: (f32, f32) = (4.2, 6.2); // 0x161398, 0x16139c (·dt²)
    pub const GRAV_SLOW: (f32, f32) = (1.0, 2.5); // 0x1613a0, 0x1613a4 (·dt²)
    pub const SHOVE: f32 = 3.0; // 0x1613a8 (·dt / 20)
    pub const AWAY: f32 = 3.0; // 0x1613ac (·dt)
    pub const SPIN_X: f32 = 18.0; // 0x1613b0 (°·dt)
    pub const SPIN_Y: f32 = 18.0; // 0x1613b4
    pub const AWAY_SLOW: f32 = 3.3; // 0x1613b8
    pub const TURN: f32 = 1.0; // 0x1613bc (°·dt)
    pub const DELAY: (i32, i32) = (10, 50); // 0x1613c0, 0x1613c4 (ticks)
    pub const SMOKE_C2: [u32; 2] = [0x2f00_0000, 0x3f00_0000]; // 0x1613d0
}
/// The level box (0x41400000, 0x447cc000).
const LO: f32 = 12.0;
const HI: f32 = 1011.0;

fn slow(oc: i16) -> bool { oc == 1185 || oc == 1186 }

fn core_of(w: &World, id: MobyId) -> Option<MobyId> {
    let k = usize::try_from(c::pi32(w, id, o::CORE) - 1).ok()?;
    (k < w.table.mobys.len()).then_some(k)
}

fn in_box(p: c::V) -> bool { (0..3).all(|k| (LO..=HI).contains(&p[k])) }

/// `0x221608` with `EulerToMatrix` rows (row 3 = (0, 0, 0, 1)): `r0·v.x + r1·v.y + r2·v.z`.
fn rotate(e: c::V, v: c::V) -> c::V {
    let r = euler_rows([e[0], e[1], e[2]]);
    let x: [f32; 3] = std::array::from_fn(|l| r[0][l] * v[0] + r[1][l] * v[1] + r[2][l] * v[2]);
    [x[0], x[1], x[2], v[3]]
}

/// Level09 0x2c2610: the nearest moby of `class` to `p` (first of equals), and its distance (100000 when none).
fn nearest(w: &World, p: c::V, class: i16) -> (Option<MobyId>, f32) {
    let mut best = (None, 100000.0f32);
    for (i, m) in w.table.mobys.iter().enumerate() {
        if m.o_class != class || m.state >= 0x80 { continue; }
        let d = c::dist3(p, m.position);
        if d < best.1 { best = (Some(i), d); }
    }
    best
}

/// Level09 0x2c2500: a piece of `class` made by the core at `q`.
fn make_piece(w: &mut World, core: MobyId, q: c::V, class: i16) -> Option<MobyId> {
    let n = w.create_moby(class)?;
    if w.m(n).pvars.len() < o::LEN { w.mm(n).pvars.resize(0x80, 0); }
    let m = w.mm(n);
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.state = 1;
    m.visible = 1;
    m.position = q;
    c::set_pv4(w, n, o::SPAWN, q);
    c::set_pi32(w, n, o::CORE, core as i32 + 1);
    w.build_matrix(n);
    let u = w.joint_point(n, 0);
    let d = c::sub(q, u);
    let r = w.m(core).rows;
    let d: [f32; 3] = std::array::from_fn(|l| r[0][l] * d[0] + r[1][l] * d[1] + r[2][l] * d[2]);
    let (rot, rows) = (w.m(core).rotation, w.m(core).rows);
    let m = w.mm(n);
    m.position = [q[0] + d[0], q[1] + d[1], q[2] + d[2], q[3]];
    m.rotation = rot;
    m.rows[..3].copy_from_slice(&rows[..3]);
    w.build_matrix(n);
    Some(n)
}

/// Level09 0x2c26c8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    // `gp−0x5838 += 1` (0x1613c8): the platforms updated this tick, read and reset by the help director 1000's skill
    // point (`help_gaspar`).
    *w.svc.level_words.entry(super::help_gaspar::PLATFORM_WORD).or_default() += 1;
    if w.m(id).pvars.len() < o::LEN { return; }
    let oc = w.m(id).o_class;
    match w.m(id).state {
        0 => init(w, id, oc),
        1 | 2 => hold(w, id, oc),
        3 => shake_loose(w, id, oc),
        4 => fall(w, id),
        5 => {
            let mut v = c::pv4(w, id, o::VEL);
            v[2] -= (DT2 * 10.8) / 3.0;
            c::set_pv4(w, id, o::VEL, v);
            spin_and_move(w, id, v);
            if !in_box(c::pos(w, id)) { w.delete_moby(id); }
        }
        _ => {}
    }
}

fn flag_set(w: &World, id: MobyId) -> bool {
    let f = c::pi32(w, id, o::FLAG);
    f != -1 && w.svc.interact.game.flags.get((f + 0x39) as usize).is_some_and(|&b| b != 0)
}

/// State 0.
fn init(w: &mut World, id: MobyId, oc: i16) {
    let q = w.joint_point(id, 0);
    if flag_set(w, id) {
        w.delete_moby(id);
        return;
    }
    w.mm(id).draw_dist = 0x3ff;
    if oc == CORE {
        if c::pi16(w, id, o::MAKE) != 0 {
            w.mm(id).draw_dist = 0xff;
            c::set_pi32(w, id, o::CORE, 0);
            for class in FIRST..FIRST + 8 {
                if class != CORE { make_piece(w, id, q, class); }
            }
        }
    } else {
        let (core, d) = nearest(w, c::pos(w, id), CORE);
        match core {
            Some(k) if d < 20.0 => {
                c::set_pi32(w, id, o::CORE, k as i32 + 1);
                let slot = o::SLOTS + 4 * (oc - FIRST) as usize;
                if w.m(k).pvars.len() >= slot + 4 { c::set_pi32(w, k, slot, id as i32 + 1); }
            }
            _ => w.svc.unported("1182..1189 platform piece: no 1184 within 20 (the game's debug print)"),
        }
    }
    let off = w.joint_local(id, 0);
    c::set_pv4(w, id, o::OFFSET, off);
    w.mm(id).state = 1;
}

/// States 1 and 2.
fn hold(w: &mut World, id: MobyId, oc: i16) {
    if oc == CORE {
        let (amp, rate) = (c::pf(w, id, o::BOB_AMP), c::pf(w, id, o::BOB_RATE) * DEG * DT);
        bob(w, id, amp, rate, o::BOB_PHASE, o::BOB_PREV);
        let (amp, ra, rb) = (c::pf(w, id, o::WOB_AMP) * DEG, c::pf(w, id, o::WOB_RATE_A) * DEG * DT, c::pf(w, id, o::WOB_RATE_B) * DEG * DT);
        wobble(w, id, amp, ra, rb, o::WOB_A, o::WOB_B);
    } else if let Some(k) = core_of(w, id) {
        if w.m(k).state != 3 {
            let rot = w.m(k).rotation;
            w.mm(id).rotation = rot;
            let d = rotate(rot, c::pv4(w, id, o::OFFSET));
            let u = w.joint_point(k, 0);
            let m = w.mm(id);
            m.position = [u[0] - d[0], u[1] - d[1], u[2] - d[2], u[3] - d[3]];
        }
    }
    if let Some(k) = core_of(w, id) {
        let (hidden, cmd) = (w.m(k).mode & 1, w.m(k).cmd);
        let m = w.mm(id);
        m.mode = (m.mode & !1) | hidden;
        m.cmd = cmd;
    }
    if w.m(id).state == 2 {
        if oc == CORE {
            let m = w.mm(id);
            m.rotation[0] = c::add_rot(m.rotation[0], k::TURN * DEG * DT);
        }
        if c::dec_timer_pvar_s16(w, id, o::TIMER) != 0 { w.mm(id).state = 3; }
    } else if w.m(id).cmd == 1 {
        w.mm(id).update_dist = 0xff;
        let (a, b) = (w.ticks(k::DELAY.0), w.ticks(k::DELAY.1));
        let t = w.rng.rand_range(a, b);
        c::set_pi16(w, id, o::TIMER, t as i16);
        let n = ((w.m(id).pvars.len() - o::SLOTS) / 4).min(9);
        for s in 0..n {
            let r = c::pi32(w, id, o::SLOTS + 4 * s);
            if let Some(m) = usize::try_from(r - 1).ok().and_then(|i| w.table.mobys.get_mut(i)) { m.update_dist = 0xff; }
        }
        w.mm(id).state = 2;
    }
}

/// State 3.
fn shake_loose(w: &mut World, id: MobyId, oc: i16) {
    c::dec_timer_pvar_s16(w, id, o::TIMER);
    set_global_flag(w, 0x38, 1);
    let f = c::pi32(w, id, o::FLAG);
    if f != -1 { set_global_flag(w, (f + 0x39) as usize, 1); }
    crate::moby_update::classes::crate_::set_death_bits(w, id, 0x200, -1);
    let p = c::pos(w, id);
    fx::death_explosion(w, 4.0, 13.0, Some(id), p, -1);
    if oc == CORE { w.play_sound(2, 0, id); }
    let q = w.joint_point(id, 0);
    let v = c::scale(c::sub(p, q), (k::SHOVE * DT) / 20.0);
    let d = c::sub(q, hero_pos(w));
    let d = c::set_len3(d, if slow(oc) { k::AWAY_SLOW } else { k::AWAY } * DT);
    let mut v = c::add(v, d);
    v[2] = 0.0;
    c::set_pv4(w, id, o::VEL, v);
    let s = k::SPIN_X * DEG * DT;
    let sx = w.rng.randf(-s, s);
    c::set_pf(w, id, o::SPIN_X, sx);
    let s = k::SPIN_Y * DEG * DT;
    let sy = w.rng.randf(-s, s);
    c::set_pf(w, id, o::SPIN_Y, sy);
    let (a, b) = (w.ticks(90) as f32, w.ticks(150) as f32);
    let t = w.rng.randf(a, b) as i32 as i16;
    c::set_pi16(w, id, o::TIMER, t);
    let g = w.rng.randf(k::GRAV.0 * DT2, k::GRAV.1 * DT2);
    c::set_pf(w, id, o::GRAVITY, g);
    if slow(oc) {
        c::set_pf(w, id, o::SPIN_X, (sx / sx) * (std::f32::consts::FRAC_PI_2 / t as f32));
        let g = w.rng.randf(k::GRAV_SLOW.0 * DT2, k::GRAV_SLOW.1 * DT2);
        c::set_pf(w, id, o::GRAVITY, g);
    }
    let m = w.mm(id);
    m.has_collision = false;
    m.state = 4;
}

fn spin_and_move(w: &mut World, id: MobyId, v: c::V) {
    let (sx, sy) = (c::pf(w, id, o::SPIN_X), c::pf(w, id, o::SPIN_Y));
    let m = w.mm(id);
    for k in 0..3 { m.position[k] += v[k]; }
    m.rotation[0] = c::add_rot(m.rotation[0], sx);
    m.rotation[1] = c::add_rot(m.rotation[1], sy);
}

/// State 4.
fn fall(w: &mut World, id: MobyId) {
    let mut v = c::pv4(w, id, o::VEL);
    v[2] -= c::pf(w, id, o::GRAVITY);
    c::set_pv4(w, id, o::VEL, v);
    spin_and_move(w, id, v);
    let p = c::pos(w, id);
    if !in_box(p) {
        w.delete_moby(id);
        return;
    }
    // The stack words the splash reuses (module doc).
    let (mut v1, mut v2) = ([0.0f32; 4], [0.0f32; 4]);
    if w.rng.randi(3) == 0 {
        v2[0] = f32::from_bits(k::SMOKE_C2[0]);
        v2[1] = f32::from_bits(k::SMOKE_C2[1]);
        let j = w.rng.randi(2) as usize;
        let (a, b) = (w.ticks(15), w.ticks(20));
        let life = w.rng.rand_range(a, b);
        let (a, b) = (w.ticks(25), w.ticks(30));
        let t1 = w.rng.rand_range(a, b);
        let sp = w.rng.randf(8.0, 16.0) * DT;
        w.part11(pf(400000.0), pf(sp), pv(p), pv([0.0; 4]), 0x2f3f_3f7f, k::SMOKE_C2[j], life, t1, 0, 0);
        v1 = [0.0; 4];
    }
    if 25.0 <= p[2] { return; }
    for _ in 0..k::SPLASH_N {
        let a = w.rng.rand_angle();
        let mut q = p;
        q[0] += w.rng.randf(-3.0, 3.0);
        q[1] += w.rng.randf(-3.0, 3.0);
        let (ca, sa) = (a.cos(), a.sin());
        v1[2] = k::SPLASH_V1Z * DT;
        v1[0] += ca * k::SPLASH_V1XY;
        v1[1] += sa * k::SPLASH_V1XY;
        v2[2] = k::SPLASH_V2Z * DT;
        v2[0] += ca * k::SPLASH_V2XY;
        v2[1] += sa * k::SPLASH_V2XY;
        v1[3] = k::SPLASH_S1;
        v2[3] = k::SPLASH_S2;
        let t: [i32; 3] = std::array::from_fn(|i| {
            let n = k::SPLASH_T[i];
            (w.rng.randf(n as f32, (n << 1) as f32) * w.svc.timing.timer_scale.to_f32()) as i32
        });
        fx::part02(w, &crate::particles::type02::Spawn { pos: q, v1, v2, c1: k::SPLASH_C1, c2: k::SPLASH_C2, t, def: -1 });
    }
    let v = c::scale(c::pv4(w, id, o::VEL), 0.45);
    c::set_pv4(w, id, o::VEL, v);
    let (sx, sy) = (c::pf(w, id, o::SPIN_X), c::pf(w, id, o::SPIN_Y));
    c::set_pf(w, id, o::SPIN_Y, sy * 0.9);
    c::set_pf(w, id, o::SPIN_X, sx * 0.9);
    w.play_sound(0, 0, id);
    w.mm(id).state = 5;
}
