//! **The hoverboard camera** (camera class 8; level05: activation `0x32a6d8` = 0, init `0x32a6e0` + `0x3296e0` /
//! `0x329540`, update `0x32a758` = `0x329358` + `0x329ce8`, pre hook `0x32a788` (empty); level 16 lists it too, record
//! 05 #0 / 16 #6): while the hero's camera mode 0x1415d4 is 8 (the Hoverboard's ride 0x6b and ramp jump 0x6c:
//! `crate::hero::hoverboard`) the camera follows behind and above the board at the record's distance and height (in the
//! air higher up: the record's second set), pushed up over what it would pass through, and swings sideways with the
//! board's facing. Activation kind 7 (priority 6), blend 3 (the pose copied in and out). Spec: docs/plan/player_controller.md
//! §15 "Class 8".
//!
//! **System or not.** One class (a `camvtbl` row): level 05's code, its copy on level 16 ([`super::level::CameraPorts`]
//! compares the four functions and the helpers). The orbit `0x3298d8` (D+0x1c = 1 while the board's 0x13fbde is set)
//! never runs on level 5: only level 16's board physics writes 0x13fbde (G-HERO-008).
//!
//! **The record** (pvar, 0x60): the header; +0x00 (2.0 → D+0x110, unread); +0x20 / +0x24 the side swing's k / d; +0x28 /
//! +0x2c / +0x30 the distance, height and look height on the ground (5.7, 1.4, 0.5); +0x34 / +0x38 / +0x3c the same
//! in the air (higher than 2 above the ground); +0x40 / +0x44 the position spring's k / d (1, 1: no lag); +0x48 the
//! height spring's velocity (run time, cleared by the init).
//!
//! | address | call / branch | port (`board.rs`) |
//! |---|---|---|
//! | 0x32a6d8 | activation hook: 0 (the kind-7 check: camera mode 8) | `level::activation_check` |
//! | 0x32a6e0 | the previous camera's position and rows; `0x272028(+0x40, cam)` (its Euler as rows: overwritten below) | [`Camera::board_init`] |
//! | 0x3296e0 | record +0x48 = 0; D+0x180..0x194 = the record's six values; D+0x1b0 / +0x1c0 = 0; D+0xc0 = 5, +0xc4 = Ratchet, +0xc8 = 12°, +0xcc = the look height, +0xd8 / +0xdc, +0xe0 = the distance, +0xe4 = 0.5, +0xf0 = the height; D+0x10 / +0x14 = 0.01 / 0.2, +0x18 = 0, +0x1c = 0, D+0x00 = 0; D+0x100 / +0x120 = 0, +0x140 = 0, +0x110 = rec +0x00, +0x134 / +0x138 = the swing's k / d, +0x13c = 0; D+0x50.. (the orbit's, n/a); `0x3294c0`: the six values again and D+0x10 / +0x14 = rec +0x40 / +0x44 (< 0: 0.01 / 0.2); `0x329540`; +0x7e = 0 | `board_init` (D+0xc0..0xe4, +0x110, +0x50..: no class-8 reader, n/a) |
//! | 0x329540 | the pivot (`0x329358`); position = pivot − Ratchet's facing (moby +0xc0) · distance + up · height; forward to pivot + up · look height; left = up × forward, up = forward × left; +0x40 = forward | `board_init` |
//! | 0x329358 | pivot D+0x80 = the feet + rows(0x13f350) · (0, 0, 0.7), copied to D+0x1d0; its z at least 62.4 on level 5 (67.2 more than 48 (xy) from (286.16, 447.82)), 77.5 on level 16; D+0xa0 = −gravity (0x13f5e0) | [`pivot`] |
//! | 0x329ce8 D+0x1c = 0 | the target: the pivot + the camera's offset from it flattened along up and set to the distance + up · height + D+0x1b0; the position springs toward it (`Cam_InterpValues` D+0x10 / +0x14 / +0x18, velocity D+0x00..0x08) | [`Camera::board_update`] |
//! | | from last tick's position (+0x64) in six steps to the new one: a sphere of 0.5 (flags 0x12, nothing ignored) hitting → the height springs toward the push-out's z − the pivot's (0.05, 0.2, velocity rec +0x48); no hit: D+0x1b0 springs to 0 (0.02, 0.2, max 0.05) and the height toward the record's (0.005, 0.2) | `board_update` |
//! | | on the ground or within 2 of it: distance → rec (by 0.03), height → rec (0.1), look height → rec (0.04); else the air set: distance (0.015), height (0.05, only without a hit), look height (0.02) | `board_update` |
//! | D+0x1c ≠ 0 | the orbit (0x3298d8, level 16's 0x13fbde) and the plain steps (0.015 / 0.05 / 0.02 toward the ground set) | n/a on level 5 (module doc) |
//! | tail | D+0x80 = D+0x1d0 (the unclamped pivot); look = it + up · look height; rows from the look; the side swing D+0x140 springs (D+0x134 / +0x138 / +0x13c, velocity D+0x120) toward (Ratchet's facing flattened along the camera's up) · left · 2, or 0 when the camera faces away from the look by more than 60° (\|cos\| < 0.5) or in a trick; the look moved along left by it; the rows again | `board_update` |
//!
//! Native `f32`.

use super::class_cam::{rows_about, set_len};
use super::script::spring;
use super::{fadd, fdot, fscale, fsub, CamInput, Camera};
use crate::hero::physics::to_f32x3;

/// The camera class.
pub const CLASS_BOARD: i32 = 8;
/// The level-05 functions of class 8 ([`super::level::CameraPorts::from_overlays`] looks for their copies).
pub const BOARD_FNS: [u32; 4] = [0x32_a6d8, 0x32_a6e0, 0x32_a758, 0x32_a788];
pub const BOARD_HELPERS: [u32; 6] = [0x32_9358, 0x32_9ce8, 0x32_96e0, 0x32_9540, 0x32_94c0, 0x32_98d8];

/// The record's words (module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoardRec {
    pub swing_k: f32,
    pub swing_d: f32,
    pub ground: [f32; 3],
    pub air: [f32; 3],
    pub k: f32,
    pub d: f32,
    /// +0x48: the height spring's velocity (run time).
    pub vel48: f32,
}

impl BoardRec {
    pub fn parse(p: &[u8]) -> Option<BoardRec> {
        let f = |o: usize| p.get(o..o + 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        Some(BoardRec {
            swing_k: f(0x20)?,
            swing_d: f(0x24)?,
            ground: [f(0x28)?, f(0x2c)?, f(0x30)?],
            air: [f(0x34)?, f(0x38)?, f(0x3c)?],
            k: f(0x40)?,
            d: f(0x44)?,
            vel48: f(0x48)?,
        })
    }
}

/// Class 8's D-block words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardCamState {
    /// D+0x00: the position spring's velocity; D+0x10 / +0x14 / +0x18 its k, d, max.
    pub vel: [f32; 3],
    pub k: f32,
    pub d: f32,
    pub max: f32,
    /// D+0x80 the pivot, D+0x1d0 unclamped; D+0xa0 up.
    pub pivot: [f32; 3],
    pub raw_pivot: [f32; 3],
    pub up: [f32; 3],
    /// D+0xcc / +0xe0 / +0xf0: look height, distance, height.
    pub look_h: f32,
    pub dist: f32,
    pub height: f32,
    /// D+0x140 the side swing, D+0x120 its velocity, D+0x134..0x13c its k, d, max.
    pub swing: f32,
    pub swing_vel: f32,
    pub swing_k: f32,
    pub swing_d: f32,
    pub swing_max: f32,
    /// D+0x184 / +0x188 / +0x180 the ground set (distance, height, look), D+0x190 / +0x194 / +0x18c the air set.
    pub ground: [f32; 3],
    pub air: [f32; 3],
    /// D+0x1b0 the push offset, D+0x1c0 its velocity.
    pub push: [f32; 3],
    pub push_vel: [f32; 3],
}

/// `x` toward `t` by `step`, onto it within `step`.
fn step_to(x: f32, t: f32, step: f32) -> f32 {
    if (x - t).abs() < step { t } else if x < t { x + step } else { x - step }
}

/// `0x329358`: the pivot (module doc).
pub fn pivot(st: &mut BoardCamState, inp: &CamInput, level: u32) {
    let h = inp.hero;
    let rows = h.rows.map(to_f32x3);
    let feet = to_f32x3(h.pos);
    let p = fadd(feet, fscale(rows[2], 0.7));
    st.pivot = p;
    st.raw_pivot = p;
    let zmin = match level {
        5 => {
            let (cx, cy) = (f32::from_bits(0x438f_147b), f32::from_bits(0x43df_e8f6));
            if ((cx - p[0]).powi(2) + (cy - p[1]).powi(2)).sqrt() < 48.0 { 62.4 } else { 67.2 }
        }
        0x10 => 77.5,
        _ => f32::MIN,
    };
    if st.pivot[2] < zmin { st.pivot[2] = zmin; }
    st.up = set_len(fscale(to_f32x3(h.gravity_dir), -1.0), 1.0);
}

impl Camera {
    /// `0x32a6e0` (module doc).
    pub(super) fn board_init(&mut self, inp: &CamInput, prev: (super::R3, [f32; 3])) {
        let slot = self.class_cam.slot;
        let level = self.level_cams.level;
        let c = &mut self.class_cam;
        c.rows = prev.0;
        c.pos = prev.1;
        c.release = 0;
        let rec = self.level_cams.slots.get_mut(slot).and_then(|s| s.board.as_mut());
        let Some(rec) = rec else { return };
        rec.vel48 = 0.0;
        let r = *rec;
        let st = &mut self.class_cam.board;
        *st = BoardCamState {
            k: if r.k < 0.0 { 0.01 } else { r.k },
            d: if r.d < 0.0 { 0.2 } else { r.d },
            max: 0.0,
            look_h: r.ground[2],
            dist: r.ground[0],
            height: r.ground[1],
            swing_k: r.swing_k,
            swing_d: r.swing_d,
            ground: r.ground,
            air: r.air,
            ..BoardCamState::default()
        };
        // 0x329540.
        pivot(st, inp, level);
        let facing = set_len(to_f32x3(inp.hero.moby_rows[0]), st.dist);
        let pos = fadd(fsub(st.pivot, facing), fscale(st.up, st.height));
        let look = fadd(fscale(st.up, st.look_h), st.pivot);
        let fwd = set_len(fsub(look, pos), 1.0);
        let up = st.up;
        let c = &mut self.class_cam;
        c.pos = pos;
        c.rows = rows_about(fwd, up);
        c.saved_fwd = fwd;
    }

    /// `0x32a758` (module doc).
    pub(super) fn board_update(&mut self, inp: &CamInput) {
        let slot = self.class_cam.slot;
        let level = self.level_cams.level;
        let Some(mut rec) = self.level_cams.slots.get(slot).and_then(|s| s.board) else { return };
        let h = inp.hero;
        let c = &mut self.class_cam;
        let st = &mut c.board;
        pivot(st, inp, level);
        let up = st.up;
        let p = st.pivot;
        // The target behind the pivot and the position's spring.
        let d = fsub(c.pos, p);
        let a = fscale(up, fdot(d, up));
        let flat = set_len(fsub(d, a), st.dist);
        let target = fadd(fadd(fadd(p, flat), fscale(up, st.height)), st.push);
        for k in 0..3 { c.pos[k] = spring(c.pos[k], target[k], st.k, st.d, st.max, &mut st.vel[k]); }
        // The push-up over what the camera would pass through.
        let mut probe = c.prev_pos;
        let stepv = fscale(fsub(c.pos, probe), f32::from_bits(0x3e2a_aaab));
        let mut hit = false;
        for _ in 0..6 {
            let pc = crate::hero::physics::from_f32x3(probe);
            if let Some((_, pushed)) = super::sphere(inp, crate::ps2v::Pf::b(0x3f00_0000), pc, 0x12, None) {
                st.height = spring(st.height, pushed[2].to_f32() - p[2], 0.05, 0.2, 0.0, &mut rec.vel48);
                hit = true;
                break;
            }
            probe = fadd(probe, stepv);
        }
        if !hit {
            for k in 0..3 { st.push[k] = spring(st.push[k], 0.0, 0.02, 0.2, 0.05, &mut st.push_vel[k]); }
            st.height = spring(st.height, st.ground[1], f32::from_bits(0x3ba3_d70a), 0.2, 0.0, &mut rec.vel48);
        }
        if h.air_ticks == 0 || h.height.to_f32() <= 2.0 {
            st.dist = step_to(st.dist, st.ground[0], 0.03);
            st.height = step_to(st.height, st.ground[1], 0.1);
            st.look_h = step_to(st.look_h, st.ground[2], 0.04);
        } else {
            st.dist = step_to(st.dist, st.air[0], 0.015);
            if !hit { st.height = step_to(st.height, st.air[1], 0.05); }
            st.look_h = step_to(st.look_h, st.air[2], 0.02);
        }
        // The look and the side swing.
        st.pivot = st.raw_pivot;
        let look = fadd(st.raw_pivot, fscale(up, st.look_h));
        let dir = set_len(fsub(look, c.pos), 1.0);
        let cosine = fdot(c.rows[0], dir);
        let rows = rows_about(dir, up);
        let swing_t = if cosine.abs() < 0.5 || h.board.trick_mode {
            0.0
        } else {
            let f = set_len(to_f32x3(h.moby_rows[0]), 1.0);
            let flat = fsub(f, set_len(rows[2], fdot(f, rows[2])));
            fdot(flat, rows[1]) * 2.0
        };
        st.swing = spring(st.swing, swing_t, st.swing_k, st.swing_d, st.swing_max, &mut st.swing_vel);
        let look = if 1e-5 < st.swing.abs() { fadd(look, set_len(rows[1], st.swing)) } else { look };
        let fwd = set_len(fsub(look, c.pos), 1.0);
        c.rows = rows_about(fwd, up);
        if let Some(b) = self.level_cams.slots.get_mut(slot).and_then(|s| s.board.as_mut()) { b.vel48 = rec.vel48; }
    }
}
