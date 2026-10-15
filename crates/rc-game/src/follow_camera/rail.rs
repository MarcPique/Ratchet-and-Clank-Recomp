//! **The rail / slide camera** (camera class 3; level01: activation `0x315dd8` = 0, init `0x315de0` + `0x314e98`,
//! update `0x315358`, pre hook `0x316030`): while the hero's camera mode 0x1415d4 is 3 (the grind states, the cable
//! slide 0x74, Novalis's slide 0x31) and, for a record with a rail (pvar +0x24), Ratchet rides that grind path, the
//! camera rides a camera path (pvar +0x20) beside him and looks ahead. 13 records on 01 (the slide: mode 3), 05 (mode
//! 1), 08 (the rails: modes 0 and 2), 16 (modes 2, 0, 1), 17 (no path: inert). Activation kind 7 (priority 6); blend
//! 5 on Novalis (the orbit blend in, 40 ticks; the rates blend out at 0.01), 3 elsewhere (a cut both ways). Kerwan's
//! cables (03) have no class-3 record: class 17's mode-2 regions retune the follow camera there ([`super::level`]).
//! Spec: docs/plan/player_controller.md §15 "Class 3".
//!
//! **System or not.** One class (a `camvtbl` row), level 01's code on the 5 levels that list it (masked overlay diff
//! of the four functions and `0x314e98`: [`super::level::CameraPorts`]); its level branch (level 14, mode 0: the look
//! ahead not wrapped) is data in the one copy. The spline walks are the shared spline library ([`crate::spline`]:
//! `0x2726c8` = [`spline::advance`], `0x272e28` = [`spline::nearest`]).
//!
//! **Data.** The camera path's w are recomputed by every init as the chord to the next point (the last to the first);
//! mode 2's first init maps its two mapping paths once for the level (pvar +0x34): each point of +0x28 is projected
//! onto the camera path and each point of +0x2c onto the record's rail (`nearest`, range 20, step 5); the game
//! rewrites the points in place (x := the old w, w := the segment, y := the length since the previous point's
//! segment); the port keeps them per slot ([`MapPoint`]; no other reader of those paths [L]). The length is the game's:
//! `k` times the w of segment `(previous segment + point index) mod count` (the loop's index never moves; the update's
//! own sum walks the segments properly).
//!
//! | address | call / branch | port (`rail.rs`) |
//! |---|---|---|
//! | 0x315dd8 | activation hook: 0 (the kind-7 check decides: camera mode 3, the rail and not off its end) | `Candidate::hook` 0 / `level::activation_check` |
//! | 0x315de0 | the previous camera's position and rows (0x167284), +0x40 = its forward | [`Camera::rail_init`] |
//! | | path +0x20 ≥ 0: every point's w = the distance to the next ((i + 1) mod n) | `rail_init` (`RailSlot::path`) |
//! | | +0x36 = 0; mode 3: the path point nearest the feet (`0x272e28`(20, 1, band 0, open)); its segment's direction (to point seg + 1, the last clamped); the camera ahead of Ratchet along it → +0x36 = 1 | `rail_init` |
//! | | `0x314e98` | below |
//! | 0x314e98 | D+0xcc = 1.5 (0.5 in mode 3), D+0xc0 = 0, D+0xc8 = 12°, D+0xe0 = 6, D+0xe4 = 0.5, D+0xf0 = 2 | look height: `RailState::look_h`; the others: n/a (no class-3 reader) |
//! | | D+0x20 = 0.0005, D+0x14 = 0.175, D+0x10 = 0.01, D+0x18 / D+0x1c / D+0x24 = 0, D+0x00..0x0c = 0 | the rate D+0x10 and the velocities D+0x00: `RailState`; others n/a |
//! | | D+0x54 = 0.3, D+0x50 = 0.01, D+0x58 = 0.2, D+0x5c = 0, D+0x30..0x3c = 0, D+0x100.., D+0x120.. = 0, D+0x140 = 3, D+0x130 = 0.003, D+0x134 = 0.1, D+0x138 = 0.2, D+0x13c = 0 | the angle rate D+0x50: `RailState::ang_rate`; others n/a |
//! | | D+0x150 / +0x154 = 0 (the cursor), D+0x160 = 1, D+0x158 / +0x15c = 0 | `RailState::cursor`, `mapped_at` |
//! | | mode 2, +0x34 = 0: +0x34 = 1, the two mappings (module doc) | `RailSlot::map` |
//! | | +0x7e = 0 | `ClassCam::release` |
//! | 0x315358 mode 0 | the hero's rail cursor (0x13f8b4 / 0x13f8b8) → D+0x150; `0x2726c8`(1, path, open) → D+0x80 | [`Camera::rail_update`] (D+0x80: n/a, no reader) |
//! | | target = path[seg] + setlen(path[seg + 1] − path[seg], path[seg].w · t / rail[seg].w) | `rail_update` (no point seg + 1: the direction 0 [L]) |
//! | | the position springs toward it (`Cam_InterpValues` k 0.01, d 0.175, D+0x00..0x08) | `rail_update` |
//! | mode 2 | the mapped interval [seg(prev), seg(i)) of +0x2c that holds the hero's rail segment (wrapping) → D+0x160 | `rail_update` (none holds it: no position step [L]) |
//! | | the look-ahead = +0x2c[prev]'s old w (0 → pvar +0x00); the arc from +0x2c[prev]'s segment to the hero (the rail's w, + t) over +0x2c[i]'s length × +0x28[i]'s length → `0x2726c8` from +0x28[prev]'s segment on the camera path (closed) | `rail_update` |
//! | | the rate D+0x10 → +0x28[prev]'s old w · 0.01 (≤ 0: 0.01) by 0.00015 a tick; the position springs at that rate (d 0.175); the angle rate's target is it too | `rail_update` |
//! | else (1, 3) | rail < 0: the path point nearest the feet (20, 1, open), then `0x2726c8`(+0x38, ×−4 when +0x36 = 1, closed); rail ≥ 0: +0x38 along the hero's rail (closed), the path point nearest that | `rail_update` (no nearest point: no position step [L]) |
//! | | the position springs (0.01, 0.175) | `rail_update` |
//! | look | rail < 0: the path point nearest the feet (20, 1, open), the look-ahead along the path (closed; open on level 14 in mode 0) → feet + (that − nearest); rail ≥ 0: the look-ahead along the hero's rail (closed) | `rail_update` (no nearest point: the feet [L]) |
//! | | + the smoothed up 0x1672b0 · D+0xcc; dir = look − position; angle = 90° − asin(dir · forward) (0 for a zero dir) | `rail_update` |
//! | | D+0x50 → the angle rate's target (0.01; mode 2: the rate) by 0.00015; turn = `0x20cf28`(0, angle, D+0x50, 0.175, 0, D+0x64) | `rail_update` (D+0x64's first value: the block's word as the camera before left it: `Camera::d_word_64`) |
//! | | forward = rot(saved, turn, saved × dir); saved = forward normalised; left = up_s × forward; up = forward × left | `rail_update` |
//! | 0x316030 | hero camera mode = 3: rail < 0 → stay; on the record's rail (the spline pointers 0x13f8b0 / 0x15f70c[rail]+0x10) and 0x13f8c0 = 0 → stay; else +0x7e = 3 (mode 3: 5) | [`Camera::rail_pre`] |
//! | 0x20d110 back | the follow record's blend 0: +0x7e = 3 → the pose copied, no blend; 5 → copied with the rates blend (0.01 on level 1) | `Camera::follow_switch_in` |
//!
//! Native `f32`.

use super::class_cam::{rows_about, set_len};
use super::script::{angle_spring, spring};
use super::{fadd, fdot, frot, fsub, CamInput, Camera, R3};
use crate::hero::physics::to_f32x3;
use crate::spline::{self, Cursor, Point};
use rc_formats::cameras::RailCamera;
use std::f32::consts::FRAC_PI_2;

/// The camera class.
pub const CLASS_RAIL: i32 = 3;
/// The level-01 functions of class 3 ([`super::level::CameraPorts::from_overlays`] looks for their copies).
pub const RAIL_ACTIVATE: u32 = 0x31_5dd8;
pub const RAIL_INIT: u32 = 0x31_5de0;
pub const RAIL_UPDATE: u32 = 0x31_5358;
pub const RAIL_PRE: u32 = 0x31_6030;
/// The init's D-block part (with mode 2's mapping).
pub const RAIL_DATA_INIT: u32 = 0x31_4e98;

/// `Cam_InterpValues` k / d of the position (0x3c23d70a, 0x3e333333) and the angle spring's d.
const K: f32 = 0.01;
const D: f32 = 0.175;
/// The rates' step a tick (0x39 1d4951 = 0.00015).
const STEP: f32 = 0.00015;

/// One point of a mode-2 mapping path after the init's rewrite.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MapPoint {
    /// x: the point's w before the rewrite (on +0x28: the rate · 100; on +0x2c: the look-ahead).
    pub old_w: f32,
    /// y: the length since the previous point's segment (module doc).
    pub len: f32,
    /// w: the segment of the spline the point projects on (stored as a float, read truncated).
    pub seg: f32,
}

/// A class-3 slot's level data: its block, its camera path (with the init's lengths) and mode 2's mappings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RailSlot {
    pub rec: RailCamera,
    pub path: Vec<Point>,
    /// +0x28 over the camera path, +0x2c over the rail (empty until the first init in mode 2).
    pub map_path: Vec<MapPoint>,
    pub map_rail: Vec<MapPoint>,
}

impl RailSlot {
    pub fn new(rec: RailCamera) -> RailSlot { RailSlot { rec, ..Default::default() } }
}

/// The D-block words class 3 reads (module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailState {
    /// D+0x00 / +0x04 / +0x08: the position's spring velocities.
    pub vel: [f32; 3],
    /// D+0x10: mode 2's position rate.
    pub rate: f32,
    /// D+0x50: the angle spring's rate; D+0x64 its velocity.
    pub ang_rate: f32,
    pub ang_vel: f32,
    /// D+0xcc: the look height along the smoothed up.
    pub look_h: f32,
    /// D+0x150 / +0x154: mode 0's cursor (the hero's, then a step of 1 along the camera path); D+0x80 that point.
    pub cursor: Cursor,
    pub ahead_pt: [f32; 3],
    /// D+0x160: mode 2's mapped point holding the hero (1 after the init).
    pub mapped_at: i32,
}

/// The mapping of `pts` onto `over` (module doc): the nearest segment of each point (20, step 5, open), then the
/// game's length since the previous point's segment.
fn map_onto(pts: &[Point], over: &[Point]) -> Vec<MapPoint> {
    let mut seg = 0i32;
    let mut out: Vec<MapPoint> = pts
        .iter()
        .map(|p| {
            if let Some((_, c)) = spline::nearest(over, false, 20.0, 5.0, 0.0, [p[0], p[1], p[2]]) { seg = c.seg; }
            MapPoint { old_w: p[3], len: 0.0, seg: seg as f32 }
        })
        .collect();
    let n = out.len();
    let m = over.len() as i32;
    for i in 0..n {
        let prev = if i == 0 { n - 1 } else { i - 1 };
        let a = out[prev].seg as i32;
        let mut k = out[i].seg as i32 - a;
        if k < 0 { k += m; }
        let mut sum = 0.0f32;
        if m > 0 {
            let w = over[(a + i as i32).rem_euclid(m) as usize][3];
            for _ in 0..k { sum += w; }
        }
        out[i].len = sum;
    }
    out
}

impl Camera {
    /// The init `0x315de0` (+ `0x314e98`) of the current class-3 camera from the previous camera (`prev`).
    pub(super) fn rail_init(&mut self, inp: &CamInput, prev: (R3, [f32; 3])) {
        let slot = self.class_cam.slot;
        let feet = to_f32x3(inp.hero.pos);
        let vols = self.level_cams.shapes_arc();
        let c = &mut self.class_cam;
        c.rows = prev.0;
        c.pos = prev.1;
        c.saved_fwd = prev.0[0];
        c.release = 0;
        let Some(rs) = self.level_cams.slots[slot].rail_cam.as_mut() else { return };
        // The camera path's lengths (the chord to the next point, the last to the first).
        rs.path = usize::try_from(rs.rec.path).ok().and_then(|i| vols.as_ref()?.paths.get(i).cloned()).unwrap_or_default();
        let n = rs.path.len();
        for i in 0..n {
            let (a, b) = (rs.path[i], rs.path[(i + 1) % n]);
            rs.path[i][3] = spline::dist3([a[0], a[1], a[2]], [b[0], b[1], b[2]]);
        }
        // +0x36: the camera ahead of Ratchet along the path (mode 3).
        rs.rec.flipped = 0;
        if rs.rec.mode == 3 {
            if let Some((_, cur)) = spline::nearest(&rs.path, false, 20.0, 1.0, 0.0, feet) {
                let s = cur.seg.max(0) as usize;
                let j = (s + 1).min(n.saturating_sub(1));
                let d = fsub(xyz(rs.path[j]), xyz(rs.path[s]));
                if fdot(d, d).sqrt() != 0.0 && j != s && 0.0 < fdot(fsub(self.class_cam.pos, feet), set_len(d, 1.0)) {
                    rs.rec.flipped = 1;
                }
            }
        }
        // 0x314e98: the D words class 3 reads; mode 2's mapping once for the level.
        let st = &mut self.class_cam.rail;
        st.look_h = if rs.rec.mode == 3 { 0.5 } else { 1.5 };
        st.rate = K;
        st.vel = [0.0; 3];
        st.ang_rate = K;
        st.cursor = Cursor::default();
        st.mapped_at = 1;
        if rs.rec.mode == 2 && rs.rec.mapped == 0 {
            rs.rec.mapped = 1;
            let path_of = |i: i32| usize::try_from(i).ok().and_then(|i| vols.as_ref()?.paths.get(i).cloned()).unwrap_or_default();
            let rail = usize::try_from(rs.rec.rail).ok().and_then(|i| vols.as_ref()?.grind_paths.get(i)).map(|g| g.points.clone()).unwrap_or_default();
            rs.map_path = map_onto(&path_of(rs.rec.map_path), &rs.path);
            rs.map_rail = map_onto(&path_of(rs.rec.map_rail), &rail);
        }
    }

    /// The pre hook `0x316030` of the current class-3 camera (module doc).
    pub(super) fn rail_pre(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let Some(rec) = self.level_cams.slots[self.class_cam.slot].rail_cam.as_ref().map(|r| r.rec) else { return };
        if h.f15d4 == CLASS_RAIL {
            if rec.rail < 0 { return; }
            if h.boots.rail == Some(rec.rail as usize) && h.boots.off_rail == 0 { return; }
        }
        self.class_cam.release = if rec.mode != 3 { 3 } else { 5 };
    }

    /// The update `0x315358` of the current class-3 camera (module doc).
    pub(super) fn rail_update(&mut self, inp: &CamInput) {
        let h = inp.hero;
        let feet = to_f32x3(h.pos);
        let slot = self.class_cam.slot;
        let Some(rs) = self.level_cams.slots[slot].rail_cam.clone() else { return };
        let rec = rs.rec;
        let vols = self.level_cams.shapes();
        let hero_rail: &[Point] = h.boots.rail.and_then(|i| vols?.grind_paths.get(i)).map_or(&[], |g| &g.points[..]);
        let rec_rail: &[Point] = usize::try_from(rec.rail).ok().and_then(|i| vols?.grind_paths.get(i)).map_or(&[], |g| &g.points[..]);
        let hcur = h.boots.cur;
        let path = &rs.path;
        let level = self.level_cams.level;
        let up_s = to_f32x3(self.g.up_s);
        let st = &mut self.class_cam.rail;
        let mut pos = self.class_cam.pos;
        let mut ahead = rec.ahead;
        let mut ang_tgt = K;
        // The position's target and its spring rate (None: no step this tick).
        let (target, k) = match rec.mode {
            0 => {
                st.cursor = hcur;
                st.ahead_pt = spline::advance(path, false, 1.0, &mut st.cursor).0;
                let s = hcur.seg.max(0) as usize;
                match (path.get(s), hero_rail.get(s)) {
                    (Some(&p0), Some(r)) => {
                        let frac = hcur.t / r[3];
                        let dir = path.get(s + 1).map_or([0.0; 3], |&p1| fsub(xyz(p1), xyz(p0)));
                        (Some(fadd(xyz(p0), set_len(dir, p0[3] * frac))), K)
                    }
                    _ => (None, K),
                }
            }
            2 => {
                let (mp, mpath) = (&rs.map_rail, &rs.map_path);
                let n = mp.len();
                let hs = hcur.seg;
                let mut prev = n.wrapping_sub(1);
                let mut found = n;
                for i in 0..n {
                    let (a, b) = (mp[prev].seg as i32, mp[i].seg as i32);
                    let inside = if b < a { a <= hs || hs < b } else { a <= hs && hs < b };
                    if inside {
                        found = i;
                        break;
                    }
                    prev = i;
                }
                st.mapped_at = found as i32;
                if found < n && found < mpath.len() {
                    let prev = (found + n - 1) % n;
                    let f14 = mp[found].len;
                    let f16 = mp[prev].old_w;
                    ahead = if f16 == 0.0 { rec.ahead } else { f16 };
                    let f15 = mpath[found].len;
                    let start = mp[prev].seg as i32;
                    let m = rec_rail.len() as i32;
                    let mut kk = hs - start;
                    if kk < 0 { kk += m; }
                    let mut sum = 0.0f32;
                    for j in 0..kk.max(0) {
                        if m > 0 { sum += rec_rail[(start + j).rem_euclid(m) as usize][3]; }
                    }
                    let f17 = sum + hcur.t;
                    let mut c = Cursor { seg: mpath.get(prev).map_or(0, |p| p.seg as i32), t: 0.0 };
                    let tgt = spline::advance(path, true, f15 * (f17 / f14), &mut c).0;
                    let x = mpath.get(prev).map_or(0.0, |p| p.old_w);
                    let rate = if x <= 0.0 { K } else { x * 0.01 };
                    ang_tgt = rate;
                    st.rate = approach(st.rate, rate);
                    (Some(tgt), st.rate)
                } else {
                    (None, st.rate)
                }
            }
            _ => {
                if rec.rail < 0 {
                    match spline::nearest(path, false, 20.0, 1.0, 0.0, feet) {
                        Some((_, mut c)) => {
                            let d = if rec.flipped == 1 { -(rec.along * 4.0) } else { rec.along };
                            (Some(spline::advance(path, true, d, &mut c).0), K)
                        }
                        None => (None, K),
                    }
                } else {
                    let mut c = hcur;
                    let a = spline::advance(hero_rail, true, rec.along, &mut c).0;
                    (spline::nearest(path, false, 20.0, 1.0, 0.0, a).map(|(q, _)| q), K)
                }
            }
        };
        if let Some(t) = target {
            for (k3, p) in pos.iter_mut().enumerate() { *p = spring(*p, t[k3], k, D, 0.0, &mut st.vel[k3]); }
        }
        // The look point.
        let mut look = if rec.rail < 0 {
            match spline::nearest(path, false, 20.0, 1.0, 0.0, feet) {
                Some((p, mut c)) => {
                    let closed = !(level == 0xe && rec.mode == 0);
                    let a = spline::advance(path, closed, ahead, &mut c).0;
                    fadd(feet, fsub(a, p))
                }
                None => feet,
            }
        } else {
            let mut c = hcur;
            spline::advance(hero_rail, true, ahead, &mut c).0
        };
        look = fadd(look, set_len(up_s, st.look_h));
        let mut dir = fsub(look, pos);
        let l = fdot(dir, dir).sqrt();
        let mut angle = 0.0;
        let c = &mut self.class_cam;
        if l != 0.0 {
            dir = [dir[0] / l, dir[1] / l, dir[2] / l];
            angle = FRAC_PI_2 - fdot(dir, c.rows[0]).clamp(-1.0, 1.0).asin();
        }
        let st = &mut c.rail;
        st.ang_rate = approach(st.ang_rate, ang_tgt);
        let turn = angle_spring(0.0, angle, st.ang_rate, D, 0.0, &mut st.ang_vel);
        let axis = super::fcross(c.saved_fwd, dir);
        let f = frot(c.saved_fwd, turn, axis);
        c.saved_fwd = set_len(f, 1.0);
        c.rows = rows_about(c.saved_fwd, up_s);
        c.pos = pos;
    }
}

fn xyz(p: Point) -> [f32; 3] { [p[0], p[1], p[2]] }

/// `x` toward `t` by 0.00015 (set when nearer: `0x221128`, the game's compare).
fn approach(x: f32, t: f32) -> f32 {
    if (x - t).abs() < STEP {
        t
    } else if x < t {
        x + STEP
    } else {
        x - STEP
    }
}
