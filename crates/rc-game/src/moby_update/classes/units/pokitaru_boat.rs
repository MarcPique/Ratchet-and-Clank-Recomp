//! U363 (census 2026-09-30; U360 / U364 in other runs): class 1075, Pokitaru's two boats (level11 `0x309ac0`, the only
//! copy; instances #440 and #441). A boat waits at the first point of its path (+0xa0: splines 12 / 30, 600 / 500
//! points) with its four propellers turning slowly; the commando 114 (U353) rides it and starts it when Ratchet stands
//! on it (`0x30a480`, [`try_start`]); it then sails its path at up to 6 u/s (the engine's running loop while Ratchet is
//! aboard, the idle loop otherwise, slowing to a stop without him), drags the area path of its boarders (+0xa8, the
//! area path of the 1246 biters waiting for it at sea: [`super::pokitaru_biter`]) along in its own frame, and carries
//! everyone on it (`CarryRiders`). At the end of the path it waits until its group's biters (+0xb4) are all gone,
//! dying, held or still waiting, then rises 6 (boat #440, +0xb0 = 1: then it works as a lift, 1 or 8 above its end
//! point depending on whether Ratchet is below or above it) or just stops (state 5). While its state is past 1 the
//! hidden boarders show up and jump aboard (five at most at a time); its state 5 deletes the ones that never showed.
//! Every tick it bobs (0.13, a quarter turn a second) and wobbles (2.4°, at 25° / 35° a second), and within 38 of the
//! camera its propellers churn the sea (type-34 bubbles, below 224.5 only).
//!
//! **Pvars** (0x200): +0x00 the damage record (+0x20), +0x08 the platform block (+0x60; flags +0x9c), +0xa0 the path,
//! +0xa8 the moving area path and +0xac its local copy, +0xb0 the lift flag, +0xb4 the group of its boarders (57 / 58),
//! +0xb8 the skip request ([`skip_to_end`]), +0xc0 the camera flag, +0xc4 the path (the game: its pointer; the port: the
//! index), +0xc8 / +0xcc the idle / running loop's voice, +0xd0 the speed (and the rise's spring velocity), +0xd4 the
//! node, +0xd8 the turn velocity, +0xe0 the bob phase, +0xe4 the un-bobbed z, +0xe8 / +0xec the wobble phases,
//! +0xf0..+0xfc the four propeller angles, +0x100 + 0x40·i the propeller manipulators (joint lists 0..3).
//!
//! The level11 `$gp` words (gp = 0x166c00): gp−0x4e2c 6 (the top speed ×dt, the speed's step ×dt², the node
//! radius ×dt), −0x4e28 30 (the turn's acceleration, degrees ×dt²), −0x4e24 180 (its top rate, degrees ×dt), −0x4e14 7 /
//! −0x4e10 5 (the camera's distance / pivot height past the camera node).
//!
//! ## Coverage
//!
//! **The update** `0x309ac0`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | entry: the position and Euler kept (CarryRiders' old pose); state ≠ 0 → z = +0xe4 (the bob undone) | | [`update`] |
//! | `0x30a500` every tick, before the states | the propellers and the wake | [`propellers`] |
//! | 0: draw distance 0xff; seq 1 (`fun_00212f90(m, 1, 0, 1)` unless on it); the local copy of the area path (+0xac[k] = (+0xa8[k] − pos)·rowsᵀ, `fun_001fa2d8` / `fun_001f9d20`); +0xc4 = the path, at its point 0, node 1; state 1; four `random_angle_radians` into +0xf0..; voices −1; block flags \|= 1 | init | [`init`] |
//! | 1: +0xb8 set → at the path's second-to-last point, facing its last (`FastArcTan`), state 5 | the skip (the commando's mission already done) | [`update`] |
//! | 2: Ratchet's ground moby (0x13f64c) is the boat: running loop `PlayClassSound(1, 4)` kept alive (`SoundIsAlive` 0x2b0798), the idle one released (owner-guarded `release_voice_slot`), voice −1; speed `Approach(6·dt, 6·dt²)` (0x281708); else the idle loop (sound 0) kept, the running one released, `Approach(0, 6·dt²)` | the engine | [`sail`] |
//! | 2: d = node − pos, `ClampLen(speed, d)` (0x284f70), pos += d | | [`sail`] |
//! | 2: node before the last: within 6·dt (3-D) → next node; its w = 37 → +0xc0 = 1; the last: \|d\|xy < 0.001 → both voices released, state 3 | | [`sail`] |
//! | 2: turn to the node (`0x281ca0`: 30°·dt² / 30°·dt², 180°·dt, +0xd8) | | [`sail`] (`turn::turn_toward_pvar`) |
//! | 2: +0xc0 → `0x323800(7, 0.003, 0)`, `0x323868(5, 0.003)` | the follow camera pulled out past the camera node | [`sail`] (`cinematic::follow_distance` / `follow_pivot_height`) |
//! | 2: +0xa8[k] = rows·+0xac[k] + pos (`matrix_mul_vec3`, `vec_add`) | the boarders' area moves with the boat | [`sail`] |
//! | 3: `0x30a850` (group +0xb4's class-1246 members alive all in 0x14 / 0xb / 0x12 / 0xc, `0x316128`) → +0xb0: speed 0, 4; else 5 | | [`boarders_done`] |
//! | 4: `0x281810(node z + 6, dt², dt², dt, &z, &+0xd0)` (spring); z ≥ node z + 5.9 → 5 | the rise | [`update`] (`turn::spring`) |
//! | 5: +0xb0: Ratchet's z (0x13f3d8) below the boat → spring to node z + 1, else node z + 8 | the lift | [`update`] |
//! | tail: +0xe4 = z; +0xe0 += dt·π/2; z += sin·0.13 | the bob | [`update`] |
//! | tail: `0x2885c8(0.0419, 25°·dt, 35°·dt, m, +0xe8, +0xec)` | the wobble (rot x / y) | [`update`] (`units::wobble`) |
//! | tail: `CarryRiders(+0x60, pos − entry pos, entry rot, rot)` (0x285cf8) | Ratchet, the boarders, bolts on it ride it | [`update`] (`triggers::carry_riders`) |
//!
//! **The propellers and the wake** `0x30a500`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | i = 0..3: not attached → `AttachManipulator(m, i, +0x100 + 0x40·i)`; attached: +0xf0[i] += dt·700° (150° in state 1), `0x231890` (= `FUN_00221e38`) on axis 0 | the four propellers | [`propellers`] (`manip::attach`, `manip::set_axis`) |
//! | xy distance to the camera (0x1677c0) ≤ 38; state 1 on odd ticks (0x15f5cc & 1) → nothing | | [`propellers`] |
//! | z < 224.5: joints 0..3 (`0x2755f8` = `0x2645a8`): 2 bubbles each (1 in state 1): ±0.3 about the joint, heading = yaw + π ± 7° (`randf_sym(0, 0.1222)`), speed `randf(2, 5)·dt` (×0.4 in state 1), vz `randf(−2, 2)·dt`, size `randf(6300, 14700)`, level 222.5 (`0x294450` = `PartType34Spawn`) | the wake | [`propellers`] (`bomb_water::part34`) |
//!
//! **Called by other classes:** `0x30a480` ([`try_start`]: in state 1 with Ratchet on it (0x13f64c, air ticks 0) → state
//! 2, seq 0 over `ticks(20)`; true), `0x30a840` ([`skip_to_end`]: +0xb8 = 1), `0x30a830` ([`arrived`]: state 5). Their
//! only callers are the commando 114 (`0x2d1030` state 9, `0x2d14b0`: U353, [`super::pokitaru_commando`], 2026-10-01)
//! and the boarders 1246 (`0x315968`: the state-5 test, ported). Without 114 the boats would stay in state 1 at their
//! path's start (the boarders hidden).
//!
//! **Not the game's, noted [L]:** the local copy's w lane is the point's w − the boat's (the transpose's row 3 is
//! (0, 0, 0, 1)); the moved points' w is that + the boat's (the boat's rows' w lanes are 0). Native `f32`; the rand
//! draws at the game's points.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn};
use crate::moby_update::services::World;
use crate::moby_update::{manip, triggers};

/// The update in the level11 class table.
pub const UPDATE_FN: u32 = 0x30_9ac0;
pub const REFERENCE_LEVEL: u32 = 11;
pub const CLASSES: [i16; 1] = [1075];
/// The joint lists the port reads (the propellers' manipulator targets and the wake's joint points).
pub const JOINTS: [i16; 1] = [1075];
/// The boarders' class (`0x4de`).
pub const BOARDER_CLASS: i16 = 1246;

/// Pvar offsets (module doc).
pub mod pv {
    pub const BLOCK: usize = 0x60;
    pub const BLOCK_FLAGS: usize = 0x9c;
    pub const PATH: usize = 0xa0;
    pub const AREA: usize = 0xa8;
    pub const AREA_LOCAL: usize = 0xac;
    pub const LIFT: usize = 0xb0;
    pub const GROUP: usize = 0xb4;
    pub const SKIP: usize = 0xb8;
    pub const CAMERA: usize = 0xc0;
    pub const PATH_NOW: usize = 0xc4;
    pub const IDLE_VOICE: usize = 0xc8;
    pub const RUN_VOICE: usize = 0xcc;
    pub const SPEED: usize = 0xd0;
    pub const NODE: usize = 0xd4;
    pub const TURN_V: usize = 0xd8;
    pub const BOB: usize = 0xe0;
    pub const Z: usize = 0xe4;
    pub const WOBBLE_A: usize = 0xe8;
    pub const WOBBLE_B: usize = 0xec;
    pub const PROP_ANGLE: usize = 0xf0;
    pub const PROP: usize = 0x100;
    pub const PROP_SIZE: usize = 0x40;
    pub const SIZE: usize = 0x200;
}

/// The level11 `$gp` words and the update's literals (module doc).
pub mod k {
    /// gp−0x4e2c: the top speed (u/s), the speed's step (u/s²) and the node radius (×dt).
    pub const SPEED: f32 = 6.0;
    /// gp−0x4e28 / −0x4e24: the turn (degrees /s² and /s).
    pub const TURN_ACC_DEG: f32 = 30.0;
    pub const TURN_MAX_DEG: f32 = 180.0;
    /// gp−0x4e14 / −0x4e10 and the rate 0x3b449ba6: the follow camera's distance and pivot height.
    pub const CAM_DIST: f32 = 7.0;
    pub const CAM_HEIGHT: f32 = 5.0;
    pub const CAM_RATE_BITS: u32 = 0x3b44_9ba6;
    /// A node's w that turns the camera flag on.
    pub const CAMERA_NODE_W: f32 = 37.0;
    /// The rise (state 4) and the lift's heights above the end node (state 5).
    pub const RISE: f32 = 6.0;
    pub const RISE_DONE: f32 = 5.9;
    pub const LIFT_LOW: f32 = 1.0;
    pub const LIFT_HIGH: f32 = 8.0;
    /// The bob: a quarter turn a second, 0.13 high.
    pub const BOB_RATE: f32 = 1.570_796_4;
    pub const BOB_AMP: f32 = 0.13;
    /// The wobble (0x3d2b92a6 rad; 25° / 35° a second).
    pub const WOBBLE_AMP_BITS: u32 = 0x3d2b_92a6;
    pub const WOBBLE_A: f32 = 0.436_332_32;
    pub const WOBBLE_B: f32 = 0.610_865_24;
    /// The propellers' spin (rad/s): 700° sailing, 150° waiting (state 1).
    pub const PROP_FAST: f32 = 12.217_304;
    pub const PROP_SLOW: f32 = 2.617_993_8;
    /// The wake: camera range (xy), the sea's top for it, the bubbles' level and spreads.
    pub const WAKE_RANGE: f32 = 38.0;
    pub const WAKE_TOP: f32 = 224.5;
    pub const WAKE_LEVEL: f32 = 222.5;
    pub const WAKE_JITTER: f32 = 0.3;
    pub const WAKE_HEADING_BITS: u32 = 0x3dfa_35dd;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }

/// Spline `i` (None: out of range).
fn spline(w: &World, i: i32) -> Option<usize> { usize::try_from(i).ok().filter(|&p| p < w.svc.splines.len()) }
fn point(w: &World, p: usize, k: usize) -> c::V { w.svc.splines[p].get(k).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]) }
fn count(w: &World, p: Option<usize>) -> i32 { p.map_or(0, |p| w.svc.splines[p].len() as i32) }
/// The path (+0xc4) and the node point (+0xd4).
fn node_point(w: &World, id: MobyId) -> c::V {
    let n = c::pi32(w, id, pv::NODE);
    spline(w, c::pi32(w, id, pv::PATH_NOW)).zip(usize::try_from(n).ok()).map(|(p, n)| point(w, p, n)).unwrap_or([0.0; 4])
}

/// Releases voice `o` when it still plays a sound of the boat (`SoundIsAlive`, then the owner-guarded
/// `release_voice_slot`), voice −1.
fn release(w: &mut World, id: MobyId, o: usize) {
    let s = c::pi32(w, id, o);
    if !w.sound_alive(s, id) { return; }
    if s != -1 && w.sound_owner(s) == Some(id) { w.release_sound(s, id); }
    c::set_pi32(w, id, o, -1);
}

/// `if (!SoundIsAlive(m, voice)) voice = PlayClassSound(index, 4, m)`.
fn keep(w: &mut World, id: MobyId, o: usize, index: i32) {
    if !w.sound_alive(c::pi32(w, id, o), id) {
        let s = w.play_sound(index, 4, id);
        c::set_pi32(w, id, o, s);
    }
}

/// `0x30a480(boat)`: the commando's start. In state 1 with Ratchet standing on the boat (ground moby 0x13f64c, air
/// ticks 0x13f65e = 0) the boat sets off: state 2, sequence 0 over `ticks(20)`. True when it started.
pub fn try_start(w: &mut World, boat: MobyId) -> bool {
    if w.m(boat).state != 1 || w.hero.ground_moby != Some(boat) || w.hero.air_ticks != 0 { return false; }
    set_state(w, boat, 2);
    let t = w.ticks(20);
    c::blend_to(w, boat, 0, 0, t);
    true
}

/// `0x30a840(boat)`: +0xb8 = 1 (in state 1 the boat then jumps to its path's end, state 5).
pub fn skip_to_end(w: &mut World, boat: MobyId) {
    if w.m(boat).pvars.len() >= pv::SIZE { c::set_pi32(w, boat, pv::SKIP, 1); }
}

/// `0x30a830(boat)`: the boat is at its end (state 5).
pub fn arrived(w: &World, boat: MobyId) -> bool { w.m(boat).state == 5 }

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    w.mm(id).draw_dist = 0xff;
    c::blend_to(w, id, 1, 0, 1);
    let (a8, ac) = (spline(w, c::pi32(w, id, pv::AREA)), spline(w, c::pi32(w, id, pv::AREA_LOCAL)));
    let (p, rows) = (c::pos(w, id), w.m(id).rows);
    if let (Some(a8), Some(ac)) = (a8, ac) {
        for k in 0..w.svc.splines[a8].len() {
            let v = c::sub(point(w, a8, k), p);
            let l = [0, 1, 2].map(|j| v[0] * rows[j][0] + v[1] * rows[j][1] + v[2] * rows[j][2]);
            if let Some(q) = w.svc.splines[ac].get_mut(k) { *q = [l[0], l[1], l[2], v[3]].map(f32::to_bits); }
        }
    }
    let path = c::pi32(w, id, pv::PATH);
    c::set_pi32(w, id, pv::PATH_NOW, path);
    let p0 = spline(w, path).map(|s| point(w, s, 0)).unwrap_or([0.0; 4]);
    c::set_pos(w, id, p0);
    c::set_pi32(w, id, pv::NODE, 1);
    set_state(w, id, 1);
    for i in 0..4 {
        let a = w.rng.rand_angle();
        c::set_pf(w, id, pv::PROP_ANGLE + 4 * i, a);
    }
    c::set_pi32(w, id, pv::RUN_VOICE, -1);
    c::set_pi32(w, id, pv::IDLE_VOICE, -1);
    let f = c::pi32(w, id, pv::BLOCK_FLAGS) | 1;
    c::set_pi32(w, id, pv::BLOCK_FLAGS, f);
}

/// State 2: sailing the path (module doc).
fn sail(w: &mut World, id: MobyId) {
    let t = node_point(w, id);
    let mut speed = c::pf(w, id, pv::SPEED);
    if w.hero.ground_moby == Some(id) {
        keep(w, id, pv::RUN_VOICE, 1);
        release(w, id, pv::IDLE_VOICE);
        turn::approach(k::SPEED * c::DT, c::DT2 * k::SPEED, &mut speed);
    } else {
        keep(w, id, pv::IDLE_VOICE, 0);
        release(w, id, pv::RUN_VOICE);
        turn::approach(0.0, c::DT2 * k::SPEED, &mut speed);
    }
    c::set_pf(w, id, pv::SPEED, speed);
    let d = c::clamp_len3(c::sub(t, c::pos(w, id)), speed);
    let p = c::add(c::pos(w, id), d);
    c::set_pos(w, id, p);
    let path = spline(w, c::pi32(w, id, pv::PATH_NOW));
    let node = c::pi32(w, id, pv::NODE);
    if node < count(w, path) - 1 {
        if c::dist3(p, t) < k::SPEED * c::DT {
            let n = node + 1;
            c::set_pi32(w, id, pv::NODE, n);
            if path.map_or(0.0, |s| point(w, s, n as usize)[3]) == k::CAMERA_NODE_W { c::set_pi32(w, id, pv::CAMERA, 1); }
        }
    } else if c::len2(d) < 0.001 {
        release(w, id, pv::RUN_VOICE);
        release(w, id, pv::IDLE_VOICE);
        set_state(w, id, 3);
    }
    let r = c::DT2 * (k::TURN_ACC_DEG * 0.017_453_292);
    turn::turn_toward_pvar(w, id, c::atan(t[0] - p[0], t[1] - p[1]), r, r, c::DT * (k::TURN_MAX_DEG * 0.017_453_292), pv::TURN_V);
    if c::pi32(w, id, pv::CAMERA) != 0 {
        let rate = f32::from_bits(k::CAM_RATE_BITS);
        crate::cinematic::follow_distance(w, k::CAM_DIST, rate, false);
        crate::cinematic::follow_pivot_height(w, k::CAM_HEIGHT, rate);
    }
    let (a8, ac) = (spline(w, c::pi32(w, id, pv::AREA)), spline(w, c::pi32(w, id, pv::AREA_LOCAL)));
    let (p, rows) = (c::pos(w, id), w.m(id).rows);
    if let (Some(a8), Some(ac)) = (a8, ac) {
        for i in 0..w.svc.splines[a8].len() {
            let Some(l) = w.svc.splines[ac].get(i).map(|q| q.map(f32::from_bits)) else { break };
            let q: [f32; 4] = std::array::from_fn(|j| rows[0][j] * l[0] + rows[1][j] * l[1] + rows[2][j] * l[2] + if j == 3 { l[3] } else { 0.0 });
            w.svc.splines[a8][i] = c::add(q, p).map(f32::to_bits);
        }
    }
}

/// `0x30a850(boat)`: true when every member of the boat's group (+0xb4) that is alive (state < 0x80) and of class 1246
/// is held (0x14), dying (0xb / 0x12) or still waiting (0xc) (`0x316128`); a bad or empty group counts as done.
fn boarders_done(w: &World, id: MobyId) -> bool {
    let Ok(g) = i8::try_from(c::pi16(w, id, pv::GROUP)) else { return true };
    !crate::moby_update::scheduler::group_ids(w, g).into_iter().filter_map(|m| w.table.mobys.get(m)).any(|m| (m.state as i8) >= 0 && m.o_class == BOARDER_CLASS && ![0x14, 0xb, 0x12, 0xc].contains(&m.state))
}

/// `0x30a500`: the propellers and the wake (module doc).
fn propellers(w: &mut World, id: MobyId) {
    let s1 = state(w, id) == 1;
    for i in 0..4 {
        let o = pv::PROP + pv::PROP_SIZE * i;
        if !manip::attached(w, id, o) { manip::attach(w, id, i as u8, id, o); }
        if manip::attached(w, id, o) {
            let rate = if s1 { k::PROP_SLOW } else { k::PROP_FAST };
            let a = c::add_rot(c::pf(w, id, pv::PROP_ANGLE + 4 * i), c::DT * rate);
            c::set_pf(w, id, pv::PROP_ANGLE + 4 * i, a);
            manip::set_axis(w, id, id, o, a, 0);
        }
    }
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if k::WAKE_RANGE < c::dist2(c::pos(w, id), cam) { return; }
    if s1 && w.counter & 1 != 0 { return; }
    if k::WAKE_TOP <= c::pos(w, id)[2] { return; }
    let n = if s1 { 1 } else { 2 };
    for j in 0..4 {
        let jp = w.joint_point(id, j);
        for _ in 0..n {
            let x = jp[0] + w.rng.randf_sym(0.0, k::WAKE_JITTER);
            let y = jp[1] + w.rng.randf_sym(0.0, k::WAKE_JITTER);
            let z = jp[2] + w.rng.randf_sym(0.0, k::WAKE_JITTER);
            let h = w.rng.randf_sym(0.0, f32::from_bits(k::WAKE_HEADING_BITS));
            let a = c::add_rot(c::yaw(w, id), h + std::f32::consts::PI);
            let mut sp = w.rng.randf(2.0, 5.0) * c::DT;
            if s1 { sp *= 0.4; }
            let vz = w.rng.randf(-2.0, 2.0) * c::DT;
            let size = w.rng.randf(6300.0, 14700.0);
            crate::moby_update::classes::bomb_water::part34(w, size, k::WAKE_LEVEL, [x, y, z, jp[3]], [a.cos() * sp, a.sin() * sp, vz]);
        }
    }
}

/// `0x309ac0`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    let (entry_pos, entry_rot) = (c::pos(w, id), w.m(id).rotation);
    if state(w, id) != 0 {
        let z = c::pf(w, id, pv::Z);
        w.mm(id).position[2] = z;
    }
    propellers(w, id);
    match state(w, id) {
        0 => init(w, id),
        1 if c::pi32(w, id, pv::SKIP) != 0 => {
            if let Some(s) = spline(w, c::pi32(w, id, pv::PATH)) {
                c::set_pi32(w, id, pv::PATH_NOW, s as i32);
                let n = w.svc.splines[s].len();
                // `count − 2` / `count − 1`: a path of fewer than two points reads before its first point in the game.
                let (a, b) = (point(w, s, n.wrapping_sub(2)), point(w, s, n.wrapping_sub(1)));
                c::set_pos(w, id, a);
                c::set_yaw(w, id, c::atan(b[0] - a[0], b[1] - a[1]));
            }
            set_state(w, id, 5);
        }
        2 => sail(w, id),
        3 if boarders_done(w, id) => {
            if c::pi32(w, id, pv::LIFT) != 0 {
                c::set_pf(w, id, pv::SPEED, 0.0);
                set_state(w, id, 4);
            } else {
                set_state(w, id, 5);
            }
        }
        4 => {
            let t = node_point(w, id)[2];
            let (mut z, mut v) = (c::pos(w, id)[2], c::pf(w, id, pv::SPEED));
            turn::spring(t + k::RISE, c::DT2, c::DT2, c::DT, &mut z, &mut v);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, pv::SPEED, v);
            if t + k::RISE_DONE <= z { set_state(w, id, 5); }
        }
        5 if c::pi32(w, id, pv::LIFT) != 0 => {
            let t = node_point(w, id)[2];
            let hz = crate::moby_update::classes::units::hero_pos(w)[2];
            let (mut z, mut v) = (c::pos(w, id)[2], c::pf(w, id, pv::SPEED));
            let to = if hz < z { t + k::LIFT_LOW } else { t + k::LIFT_HIGH };
            turn::spring(to, c::DT2, c::DT2, c::DT, &mut z, &mut v);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, pv::SPEED, v);
        }
        _ => {}
    }
    let z = c::pos(w, id)[2];
    c::set_pf(w, id, pv::Z, z);
    let a = c::add_rot(c::pf(w, id, pv::BOB), c::DT * k::BOB_RATE);
    c::set_pf(w, id, pv::BOB, a);
    w.mm(id).position[2] = z + a.sin() * k::BOB_AMP;
    crate::moby_update::classes::units::wobble(w, id, f32::from_bits(k::WOBBLE_AMP_BITS), c::DT * k::WOBBLE_A, c::DT * k::WOBBLE_B, pv::WOBBLE_A, pv::WOBBLE_B);
    let d = c::sub(c::pos(w, id), entry_pos);
    let r = w.m(id).rotation;
    triggers::carry_riders(&mut w.mm(id).pvars, pv::BLOCK, d, entry_rot, r);
}
