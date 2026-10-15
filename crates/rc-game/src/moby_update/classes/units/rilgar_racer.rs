//! **Rilgar's hoverboard racers, class 717** (level05 `0x307910`, census U177; four placed at the race start). Each rides
//! its own board (a created class 439, [`super::hoverboard`]) along the race's paths while Ratchet is on his board
//! (movement group 0x16): its start lane, then the racing line with its branches and ramp jumps, boosted by the course's
//! hoops and pads, its speed pulled toward Ratchet's place. Read from the level05 decomp. Native `f32`.
//!
//! **Shared with Kalebo III's racers 556** ([`super::kalebo_racer`]): the level's copies of the waypoint, pickup, jump and
//! jump-arc code differ only in where the fields sit and in the branch rule, so [`advance`], [`pickups`], [`jumps`] and
//! [`jump_arc`] take a [`Layout`].
//!
//! **Pvars** (0x280): +0x00 / +0x80 / +0x100 / +0x180 four look-at records (`manip::look`, joint lists 0 / 2 / 1 / 3),
//! +0x200 the point on the track (its z the track height), +0x210 the jump's landing point, +0x220 the racing line,
//! +0x224.. the branches (by a branch node's w − 1), +0x234 the start lane, +0x238 / +0x23c the speed and boosted
//! speed, +0x240 the speed, +0x244 the current path, +0x248 the yaw's spring velocity, +0x24c the vertical speed,
//! +0x250 the racer's number, +0x258 (s16) the waypoint, +0x25a (s16) the boost timer, +0x264 the pickups' cooldown,
//! +0x268 the board, +0x26c the jump's ticks, +0x272 (s16) the trick (−1 none), +0x274 the bob's phase, +0x27c the
//! jump's landing waypoint.
//!
//! ## Coverage (`0x307910`)
//! | address | what | port |
//! |---|---|---|
//! | top | drawn and within 29 of the camera: the shadow probe `0x26f020`, +0x7f = 0x17 | [`update`] |
//! | state 0 | update distance 0x18, hidden (+0x34 \|= 0x41); the start lane's point 0 (`0x306d30`); the facing along the lane; speeds 17·dt / 24·dt + number·1.55·dt, the first race slowed by 1% per finished race past the fourth (0x15ee38, ≥ 77%); the bob's phase random; → 1 | [`update`] |
//! | state 1 | Ratchet on his board: update distance 0xff, shown, waypoint 0, boost ticks(180), the motion cleared; its board created (`0x2f8718`: class 439 in state 2 at its pose, +0x50 = 1); odd numbers mirrored (+0x34 \|= 0x8000, the board too); → 2 | [`update`] |
//! | state 2 | off the board → 0; the waypoint (`0x306e10`), the pickups (`0x307010`), the jumps (`0x307358`); a jump: the vertical speed that lands at the landing waypoint (21·dt² gravity, by 110·dt²); the anims (`0x307570`); the yaw springs toward the waypoint (0.035, 0.3, 400°/s); the track point moves by the speed along the facing; crates within 1 of 4 steps ahead get a hit (flags 0x30000); gravity and the landing on `GroundHeight(0.5)`; the speed: boosted while the boost timer runs, else toward the base (19·dt in the air; + 3·dt in the first ticks(700) of the race; − 1.5·dt when ahead of Ratchet, + dt when behind; by 7 or 15·dt²); the lean records; the position = the track point + 0.25 + the bob; the board follows (pose, light, +0xbc = 1 boosting, its anim speed) | [`update`] |
//! | `0x306e10` | the waypoint advances while it is behind (> 90° from the facing) or within 2 (4 when behind); on the racing line a branch node (w > 0) switches to branch w (`randi(100) > 0`); a branch's last point returns to the racing line, 5 past its point nearest the track point | [`advance`] |
//! | `0x307010` | from ticks(180) into the race with Ratchet's board, the cooldown out: a hoop of the board's group within 8 of the track point + 0.8 (box 0.5 × 2.5 × 2.5) or a pad (2.8 × 1.4 × 0.5): boost + ticks(120), cooldown ticks(60) | [`pickups`] |
//! | `0x307358` | on a branch, falling or level, at a node with w = 1: the landing is the next node with w = 2; the jump starts | [`jumps`] |
//! | `0x307570` / `0x307480` | the anims: idle 0 (back to 0 at a wrap); the jump 5; a trick 1..3 (`rand_range(0, 3) + 1`) when the landing is more than ticks(27) away, timed to it, then the landing (5 frame 3 or 2); the board plays the same (5 → 6), its frame scaled by the rate ratio | [`anims`] |

#![allow(clippy::neg_cmp_op_on_partial_ord)] // The game's compares, NaN included.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, ground, turn};
use crate::moby_update::services::HitTemplate;
use crate::moby_update::manip;
use crate::moby_update::services::World;
use crate::moby_update::story;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x30_7910;
pub const CLASSES: [i16; 1] = [717];
pub const JOINTS: [i16; 1] = [717];

mod pv {
    pub const TRACK: usize = 0x200;
    pub const LAND: usize = 0x210;
    pub const LINE: usize = 0x220;
    pub const BRANCHES: usize = 0x224;
    pub const LANE: usize = 0x234;
    pub const BASE: usize = 0x238;
    pub const BOOSTED: usize = 0x23c;
    pub const SPEED: usize = 0x240;
    pub const PATH: usize = 0x244;
    pub const YAW_VEL: usize = 0x248;
    pub const VZ: usize = 0x24c;
    pub const NUMBER: usize = 0x250;
    pub const WP: usize = 0x258;
    pub const BOOST: usize = 0x25a;
    pub const COOLDOWN: usize = 0x264;
    pub const BOARD: usize = 0x268;
    pub const JUMP: usize = 0x26c;
    pub const TRICK: usize = 0x272;
    pub const PHASE: usize = 0x274;
    pub const LAND_WP: usize = 0x27c;
    pub const LEN: usize = 0x280;
}

pub(super) const DT: f32 = 1.0 / 60.0;
pub(super) const DT2: f32 = 1.0 / 3600.0;
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;

pub(super) fn atan(dx: f32, dy: f32) -> f32 { c::atan(dx, dy) }
pub(super) fn pt(path: &[[f32; 4]], i: i32) -> [f32; 4] { usize::try_from(i).ok().and_then(|i| path.get(i)).copied().unwrap_or([0.0; 4]) }
pub(super) fn path(w: &World, i: i32) -> Vec<[f32; 4]> {
    usize::try_from(i).ok().and_then(|i| w.svc.splines.get(i)).map(|p| p.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}
pub(super) fn track(w: &World, id: MobyId, l: &Layout) -> [f32; 4] { [0, 1, 2, 3].map(|k| c::pf(w, id, l.track + 4 * k)) }
pub(super) fn set_track(w: &mut World, id: MobyId, l: &Layout, p: [f32; 4]) { for (k, &v) in p.iter().enumerate() { c::set_pf(w, id, l.track + 4 * k, v); } }
fn board(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, pv::BOARD) - 1).ok().filter(|&b| b < w.table.mobys.len()) }
pub(super) fn cur_path(w: &World, id: MobyId, l: &Layout) -> Vec<[f32; 4]> { path(w, c::pi32(w, id, l.path)) }
pub(super) fn dist2(a: [f32; 4], b: [f32; 4]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }
pub(super) fn dist3(a: [f32; 4], b: [f32; 4]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }

/// `0x306d30(m, i)` (level16 `0x2cf9d0`): the current path's point `i` (the sideways offset is 0).
pub(super) fn point(w: &World, id: MobyId, l: &Layout, i: i32) -> [f32; 4] { pt(&cur_path(w, id, l), i) }

/// `0x287a18(path, i, step, closed)` on a closed path.
pub(super) fn step(n: i32, i: i32, s: i32) -> i32 { if n == 0 { 0 } else { (i + s + n) % n } }

/// Where a racer family keeps its race fields, and the few constants its copies of the shared race code differ in
/// (Rilgar's 717 [`RILGAR`], Kalebo III's 556 `kalebo_racer::LAYOUT`).
pub(super) struct Layout {
    pub track: usize,
    pub land: usize,
    pub line: usize,
    pub branches: usize,
    pub base: usize,
    pub boosted: usize,
    pub speed: usize,
    /// The current path (the game keeps its pointer; the port the spline index).
    pub path: usize,
    pub yaw_vel: usize,
    pub vz: usize,
    pub wp: usize,
    pub boost: usize,
    pub cooldown: usize,
    pub jump: usize,
    pub land_wp: usize,
    /// The landing waypoint is an s16 (Kalebo's) rather than a word.
    pub land_wp_s16: bool,
    /// A racing-line node branches when its w is above `branch_min`, on `branch_chance < randi(100)`, to the branch
    /// `w − branch_first`.
    pub branch_min: f32,
    pub branch_chance: i32,
    pub branch_first: i32,
    /// The jumps start only off the racing line.
    pub jump_off_line: bool,
}

const RILGAR: Layout = Layout {
    track: pv::TRACK, land: pv::LAND, line: pv::LINE, branches: pv::BRANCHES, base: pv::BASE, boosted: pv::BOOSTED,
    speed: pv::SPEED, path: pv::PATH, yaw_vel: pv::YAW_VEL, vz: pv::VZ, wp: pv::WP, boost: pv::BOOST,
    cooldown: pv::COOLDOWN, jump: pv::JUMP, land_wp: pv::LAND_WP, land_wp_s16: false, branch_min: 0.0, branch_chance: 0, branch_first: 1,
    jump_off_line: true,
};

/// Level05 `0x307910` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pv::LEN);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if dist3(c::pos(w, id), cam) < 29.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x17;
        }
    }
    match w.m(id).state {
        0 => {
            {
                let m = w.mm(id);
                m.update_dist = 0x18;
                m.mode |= 0x41;
            }
            let lane = c::pi32(w, id, pv::LANE);
            c::set_pi32(w, id, pv::PATH, lane);
            let p0 = point(w, id, &RILGAR, 0);
            w.mm(id).position = p0;
            set_track(w, id, &RILGAR, p0);
            let p1 = point(w, id, &RILGAR, 1);
            let m = w.mm(id);
            m.rotation[0] = 0.0;
            m.rotation[1] = 0.0;
            m.rotation[2] = atan(p1[0] - p0[0], p1[1] - p0[1]);
            let k = c::pi32(w, id, pv::NUMBER) as f32 * DT * 1.55;
            let (mut fast, mut base) = (DT * 24.0 + k, DT * 17.0 + k);
            if story::flag(w, 0) == 0 {
                let n = ((w.svc.board.records.finished[0] - 4) as f32).max(0.0);
                let f = (1.0 - n * 0.01).max(0.77);
                fast *= f;
                base *= f;
            }
            c::set_pf(w, id, pv::BOOSTED, fast);
            c::set_pf(w, id, pv::BASE, base);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::PHASE, a);
            w.mm(id).state = 1;
        }
        1 => {
            if w.hero.group != crate::hero::hoverboard::GROUP { return; }
            {
                let m = w.mm(id);
                m.update_dist = 0xff;
                m.mode &= 0xffbe;
            }
            c::set_pi16(w, id, pv::WP, 0);
            let t = w.ticks(0xb4) as i16;
            c::set_pi16(w, id, pv::BOOST, t);
            for o in [pv::VZ, pv::YAW_VEL, pv::JUMP, pv::COOLDOWN, pv::SPEED] { c::set_pi32(w, id, o, 0); }
            if board(w, id).is_none() {
                if let Some(b) = create_board(w, id) {
                    c::set_pi32(w, id, pv::BOARD, b as i32 + 1);
                    crate::moby_update::story::pvars(w, b, 0x60);
                    c::set_pi32(w, b, 0x50, 1);
                }
            }
            let n = c::pi32(w, id, pv::NUMBER);
            if n / 2 * 2 == n - 1 {
                w.mm(id).mode |= 0x8000;
                if let Some(b) = board(w, id) { w.mm(b).mode |= 0x8000; }
            }
            w.mm(id).state = 2;
        }
        2 => ride(w, id),
        _ => {}
    }
}

/// `0x2f8718`: the racer's board (class 439 in state 2 at its pose).
fn create_board(w: &mut World, id: MobyId) -> Option<MobyId> {
    let b = w.create_moby(super::hoverboard::CLASSES[0])?;
    let (pos, rot, light) = { let m = w.m(id); (m.position, m.rotation, m.light) };
    {
        let m = w.mm(b);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.visible = 1;
        m.state = 2;
        m.position = pos;
        m.rotation = rot;
        m.light = light;
    }
    w.build_matrix(b);
    Some(b)
}

fn ride(w: &mut World, id: MobyId) {
    if w.hero.group != crate::hero::hoverboard::GROUP {
        w.mm(id).state = 0;
        return;
    }
    advance(w, id, &RILGAR);
    pickups(w, id, &RILGAR);
    jumps(w, id, &RILGAR);
    jump_arc(w, id, &RILGAR);
    anims(w, id);
    let q = point(w, id, &RILGAR, c::pi16(w, id, pv::WP) as i32);
    let pos = c::pos(w, id);
    let to = atan(q[0] - pos[0], q[1] - pos[1]);
    {
        let (mut a, mut v) = (Pf::f(c::yaw(w, id)), Pf::f(c::pf(w, id, pv::YAW_VEL)));
        crate::hero::physics::turn_spring(Pf::f(to), Pf::f(0.035), Pf::f(0.3), Pf::f(DT * 6.981_317), &mut a, &mut v, 0);
        c::set_yaw(w, id, a.to_f32());
        c::set_pf(w, id, pv::YAW_VEL, v.to_f32());
    }
    let mut p = track(w, id, &RILGAR);
    if p[2] < q[2] - 5.0 { p[2] = q[2]; }
    let yaw = c::yaw(w, id);
    let speed = c::pf(w, id, pv::SPEED);
    let v = [yaw.cos() * speed, yaw.sin() * speed, 0.0, 0.0];
    p = [p[0] + v[0], p[1] + v[1], p[2] + v[2], p[3]];
    // Crates ahead.
    let centre = [v[0] * 4.0 + p[0], v[1] * 4.0 + p[1], v[2] * 4.0 + p[2], p[3]];
    let pv4 = crate::moby_update::services::pv;
    let listed = crate::moby_update::services::sphere_mobys_in(w.table, w.svc, w.classes, Pf::ONE, pv4(centre), 0, Some(id), None);
    if !listed.is_empty() {
        let dir = pv4([v[0] * 7.0, v[1] * 7.0, v[2] * 7.0, 0.0]);
        for m in listed {
            if (500..=540).contains(&w.m(m).o_class) {
                let t = HitTemplate { dir, attacker: Some(id), flags: 0x3_0000, damage: Pf::ONE, w20: 1, ..Default::default() };
                w.deliver_hit(m, &t);
            }
        }
    }
    // The ground.
    let gz = w.ground_height(Pf::b(0x3f00_0000), pv4(p), 0).to_f32();
    let dz = p[2] - gz;
    let mut vz = c::pf(w, id, pv::VZ);
    if 0.0 < dz || 0.0 < vz {
        vz -= DT2 * 21.0;
    } else if dz.abs() < 0.5 {
        vz = 0.0;
        p[2] = gz;
        c::set_pi32(w, id, pv::JUMP, 0);
    }
    p[2] += vz;
    c::set_pf(w, id, pv::VZ, vz);
    // The speed.
    let mut speed = c::pf(w, id, pv::SPEED);
    if c::dec_timer_pvar_s16(w, id, pv::BOOST) == 0 {
        turn::approach(c::pf(w, id, pv::BOOSTED), DT2 * 20.0, &mut speed);
    } else {
        let mut t = c::pf(w, id, pv::BASE);
        if gz < p[2] { t = DT * 19.0; }
        if w.hero.board.race_ticks < w.ticks(700) { t += DT * 3.0; }
        let n = (c::pi32(w, id, pv::NUMBER) as usize).min(7);
        if (w.hero.board.racer_place[n] as i32) < w.hero.board.place { t -= DT * 1.5; } else { t += DT; }
        let rate = if t < speed { 7.0 } else { 15.0 };
        turn::approach(t, DT2 * rate, &mut speed);
    }
    c::set_pf(w, id, pv::SPEED, speed);
    if dz < 0.2 && vz < 0.0 {
        p[2] = gz;
        c::set_pi32(w, id, pv::JUMP, 0);
    }
    set_track(w, id, &RILGAR, p);
    // The lean records (the big-head cheat's scale on the head's).
    manip::big_head_scale(w, 3.0, id, 0x180);
    let lim = DT * 2.617_993_8;
    let mut lean = c::pf(w, id, pv::YAW_VEL).clamp(-lim, lim);
    if w.m(id).mode & 0x8000 != 0 { lean = -lean; }
    c::set_pf(w, id, 0x1e8, lean * 30.0);
    c::set_pf(w, id, 0x68, lean * 20.0);
    c::set_pf(w, id, 0x168, lean * 50.0);
    c::set_pf(w, id, 0xe4, lean * 40.0);
    manip::look(w, id, id, 0x180, 3, 0.02, 0.3);
    manip::look(w, id, id, 0x00, 0, 0.02, 0.3);
    manip::look(w, id, id, 0x100, 1, 0.02, 0.3);
    manip::look(w, id, id, 0x80, 2, 0.015, 0.3);
    // The position: the track point, 0.25 up, the bob.
    let phase = c::add_rot(c::pf(w, id, pv::PHASE), DT * 6.108_652);
    c::set_pf(w, id, pv::PHASE, phase);
    let z = p[2] + 0.25 + phase.sin() * 0.08;
    w.mm(id).position = [p[0], p[1], z, p[3]];
    // The board follows.
    if let Some(b) = board(w, id) {
        let (pos, rot, light, boost) = { let m = w.m(id); (m.position, m.rotation, m.light, c::pi16(w, id, pv::BOOST)) };
        let m = w.mm(b);
        m.light = light;
        m.position = pos;
        m.rotation = rot;
        if boost != 0 { m.cmd = 1; }
    }
}

fn land_wp(w: &World, id: MobyId, l: &Layout) -> i32 {
    if l.land_wp_s16 { c::pi16(w, id, l.land_wp) as i32 } else { c::pi32(w, id, l.land_wp) }
}

/// The jump's vertical speed (`0x307910` state 2; level16 `0x2d04a0`): for ticks(17) from the take-off, toward the
/// speed that lands at the landing waypoint (21·dt² gravity, by 110·dt²).
pub(super) fn jump_arc(w: &mut World, id: MobyId, l: &Layout) {
    if c::pi32(w, id, l.jump) == 0 { return; }
    let j = c::pi32(w, id, l.jump) + 1;
    c::set_pi32(w, id, l.jump, j);
    if j >= w.ticks(0x11) { return; }
    let p = cur_path(w, id, l);
    let n = p.len() as i32;
    let (from, to) = (c::pi16(w, id, l.wp) as i32, land_wp(w, id, l));
    let mut d = 0.0;
    let mut k = from;
    while k < to {
        let k2 = step(n, k, 1);
        d += dist2(pt(&p, k), pt(&p, k2));
        k = k2;
        if k == from { break; }
    }
    let t = d / c::pf(w, id, l.speed);
    let (z0, z1) = (pt(&p, from)[2], pt(&p, to)[2]);
    let g = DT2 * 21.0;
    if c::pf(w, id, l.vz) < 0.0 { c::set_pf(w, id, l.vz, 0.0); }
    let mut v = c::pf(w, id, l.vz);
    turn::approach((z1 - z0) / t + g * t * 0.5, DT2 * 110.0, &mut v);
    c::set_pf(w, id, l.vz, v);
}

/// `0x306e10` (level16 `0x2cf9f8`): the waypoint (module doc).
pub(super) fn advance(w: &mut World, id: MobyId, l: &Layout) {
    let line = c::pi32(w, id, l.line);
    let mut wp = c::pi16(w, id, l.wp) as i32;
    for _ in 0..1024 {
        let pos = c::pos(w, id);
        let q = point(w, id, l, wp);
        let a = atan(q[0] - pos[0], q[1] - pos[1]);
        let q2 = point(w, id, l, c::pi16(w, id, l.wp) as i32);
        let d = dist2(q2, track(w, id, l));
        let ahead = c::diff_rots(a, c::yaw(w, id)) < HALF_PI;
        if (ahead || 4.0 < d) && 2.0 < d { return; }
        let p = cur_path(w, id, l);
        let n = p.len() as i32;
        if n == 0 { return; }
        let next = (c::pi16(w, id, l.wp) as i32 + 1) % n;
        c::set_pi16(w, id, l.wp, next as i16);
        wp = next;
        let on_line = c::pi32(w, id, l.path) == line;
        if !on_line {
            if next == n - 1 {
                c::set_pi32(w, id, l.path, line);
                let lp = path(w, line);
                let pts: Vec<crate::path::Point> = lp.iter().map(|q| q.map(f32::to_bits)).collect();
                let k = crate::path::nearest_at_distance(&pts, 0.0, track(w, id, l));
                let k = step(lp.len() as i32, k, 5);
                c::set_pi16(w, id, l.wp, k as i16);
                wp = k;
            }
            continue;
        }
        if pt(&p, next)[3] <= l.branch_min { continue; }
        if l.branch_chance < w.rng.randi(100) {
            let b = pt(&p, next)[3] as i32;
            let to = c::pi32(w, id, l.branches + 4 * (b - l.branch_first).max(0) as usize);
            c::set_pi16(w, id, l.wp, 0);
            c::set_pi32(w, id, l.path, to);
            wp = 0;
        }
    }
}

/// `0x307010` (level16 `0x2cfc00`): the boost hoops and pads of Ratchet's board's groups.
pub(super) fn pickups(w: &mut World, id: MobyId, l: &Layout) {
    if w.hero.board.race_ticks < w.ticks(0xb4) { return; }
    if c::dec_timer_pvar_i32(w, id, l.cooldown) == 0 { return; }
    let Some(hb) = w.hero.board.moby.filter(|&b| b < w.table.mobys.len()) else { return };
    let (hoops, pads) = (c::pi32(w, hb, 0x44), c::pi32(w, hb, 0x48));
    let tp = track(w, id, l);
    let members = |w: &World, g: i32| -> Vec<MobyId> {
        usize::try_from(g).ok().and_then(|g| w.svc.groups.lists.get(g)).and_then(|l| l.clone()).unwrap_or_default().into_iter().map(|m| m as MobyId).collect()
    };
    let local = |w: &World, m: MobyId, from: [f32; 4]| -> [f32; 3] {
        let x = w.m(m);
        let d = [x.position[0] - from[0], x.position[1] - from[1], x.position[2] - from[2]];
        std::array::from_fn(|k| d[0] * x.rows[k][0] + d[1] * x.rows[k][1] + d[2] * x.rows[k][2])
    };
    let hit = |w: &mut World| {
        let n = c::pi16(w, id, l.boost).wrapping_add(w.ticks(0x78) as i16);
        c::set_pi16(w, id, l.boost, n);
        let t = w.ticks(0x3c);
        c::set_pi32(w, id, l.cooldown, t);
    };
    if hoops != -1 {
        let at = [tp[0], tp[1], tp[2] + 0.8, tp[3]];
        for m in members(w, hoops) {
            if w.m(m).o_class != crate::hero::hoverboard::HOOP_CLASS || !(dist3(w.m(m).position, at) <= 8.0) { continue; }
            let q = local(w, m, at);
            if q[0].abs() < 0.5 && q[1].abs() < 2.5 && q[2].abs() < 2.5 { hit(w); }
        }
    }
    if pads != -1 {
        for m in members(w, pads) {
            if w.m(m).o_class != crate::hero::hoverboard::PAD_CLASS || !(dist3(w.m(m).position, tp) <= 8.0) { continue; }
            let q = local(w, m, tp);
            if q[0].abs() < 2.8 && q[1].abs() < 1.4 && q[2].abs() < 0.5 { hit(w); }
        }
    }
}

/// `0x307358` (level16 `0x2cff48`): a jump starts at a node with w = 1 (falling or level; Rilgar's only off the racing
/// line); its landing is the next w = 2 node.
pub(super) fn jumps(w: &mut World, id: MobyId, l: &Layout) {
    if (l.jump_off_line && c::pi32(w, id, l.path) == c::pi32(w, id, l.line)) || 0.0 < c::pf(w, id, l.vz) { return; }
    let p = cur_path(w, id, l);
    let n = p.len() as i32;
    let mut k = c::pi16(w, id, l.wp) as i32;
    if pt(&p, k)[3] != 1.0 { return; }
    for _ in 0..n {
        if pt(&p, k)[3] == 2.0 { break; }
        k = step(n, k, 1);
    }
    if l.land_wp_s16 { c::set_pi16(w, id, l.land_wp, k as i16); } else { c::set_pi32(w, id, l.land_wp, k); }
    for (i, &v) in pt(&p, k).iter().enumerate() { c::set_pf(w, id, l.land + 4 * i, v); }
    c::set_pi32(w, id, l.jump, 1);
}

/// `0x307480(m, seq, frame, ticks)`: the racer's blend and its board's (5 → 6, the frame scaled by the rate ratio).
fn blend(w: &mut World, id: MobyId, seq: u8, frame: i32, ticks: i32) {
    w.anim_blend(id, seq, frame, ticks);
    let Some(b) = board(w, id) else { return };
    let bs = if seq == 5 { 6 } else { seq };
    let ratio = w.m(b).anim.rate / w.m(id).anim.rate;
    let f = frame as f32 * ratio;
    let count = w.anim_class(w.m(b).o_class).and_then(|cl| cl.sequence(bs)).map_or(0, |s| s.frames.len()) as f32;
    let f = if f <= count { f.max(0.0) } else { count };
    w.anim_blend(b, bs, f as i32, ticks);
}

/// `Quad(a, b, c, &r0, &r1)`'s count and first root `r0 = (−b + √D) / 2a` (−b / 2a for D = 0).
pub(super) fn quad_first(a: f32, b: f32, cc: f32) -> (i32, f32) {
    let d = b * b - (a * 4.0) * cc;
    if d == 0.0 { return (1, -b / (a + a)); }
    let s = d.abs().sqrt();
    (if 0.0 < d { 2 } else { 0 }, (-b + s) / (a + a))
}

/// `0x307570`: the anims (module doc).
fn anims(w: &mut World, id: MobyId) {
    let jump = c::pi32(w, id, pv::JUMP);
    if jump == 0 {
        w.mm(id).anim.speed = 1.0;
        if w.m(id).anim.flags & 2 != 0 && w.m(id).anim.seq_b != 0 {
            let t = w.ticks(8);
            blend(w, id, 0, 0, t);
        }
    } else {
        if w.m(id).anim.seq_b != 5 && jump <= w.ticks(3) {
            let t = w.ticks(5);
            blend(w, id, 5, 1, t);
            c::set_pi16(w, id, pv::TRICK, -1);
        }
        let g = DT2 * 21.0 * -0.5;
        let (n, r) = quad_first(g, c::pf(w, id, pv::VZ) - g, c::pf(w, id, pv::TRACK + 8) - c::pf(w, id, pv::LAND + 8));
        let t = if 0 < n && 0.0 < r { (r as i32) as f32 } else { 30.0 };
        if w.ticks(7) < jump && jump < w.ticks(0xb) && w.m(id).anim.seq_b == 5 && (w.ticks(0x1b) as f32) < t {
            let k = w.rng.rand_range(0, 3) + 1;
            c::set_pi16(w, id, pv::TRICK, k as i16);
            let b = w.ticks(7);
            blend(w, id, k as u8, 1, b);
        }
        if c::pi16(w, id, pv::TRICK) != -1 {
            let a = w.m(id).anim;
            if a.seq_a == a.seq_b {
                let f = ground::key_time(w, id);
                if 7.0 < f && f < 20.0 && t != 0.0 { w.mm(id).anim.speed = (30.0 / t).clamp(0.3, 1.7); }
            }
            if t < w.ticks(9) as f32 {
                let b = w.ticks(7);
                blend(w, id, 5, 3, b);
                c::set_pi16(w, id, pv::TRICK, -1);
            } else if w.m(id).anim.flags & 2 != 0 {
                let b = w.ticks(7);
                blend(w, id, 5, 2, b);
                c::set_pi16(w, id, pv::TRICK, -1);
            }
        }
        let a = w.m(id).anim;
        if a.seq_a == a.seq_b && a.seq_a == 5 && t != 0.0 {
            let f = ground::key_time(w, id);
            w.mm(id).anim.speed = (25.0 - f) / (t * 0.5);
        }
    }
    if let Some(b) = board(w, id) {
        let s = w.m(id).anim.speed;
        w.mm(b).anim.speed = s;
    }
}
