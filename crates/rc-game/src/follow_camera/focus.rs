//! **Class 18, the moby focus** (level02: hook `0x2fc298` → update `0x2fb9c8`, region test `0x2fb648`, view test
//! `0x2fb788`; init / update / pre empty; the same code on levels 02, 03, 06, 07, 08, 12 and 18): a region that, while
//! Ratchet is in it and the follow camera is current, turns the follow camera toward a moby (the record's +0x28, or
//! the first live member of the group +0x44) or along the record's facing, and retunes its distance and heights
//! through the follow camera's setters and the region lock shared with classes 17 and 23. Its hook answers 0: it never
//! becomes the current camera. Spec: docs/plan/player_controller.md §15 "Class 18". Native `f32`.
//!
//! **Modes** (+0x3c): 1 / 2 (L07 / L06) the turn eases in over 200 ticks, the stick stops it (counter 300), after 400 /
//! 350 idle ticks with the moby within 30° of the view it starts again (the end-sphere flags 0xb0 and the sphere chain
//! 1 / 12 / 0.11 while inside); 3 (L03) toward the moby, eased in over 90 ticks, the leash off, the stiff springs; 4
//! (L08) along the record's facing (the nearer of it and its reverse to the camera), 5 (L12) along the camera's yaw
//! when the stick last moved (the record's facing at the start), both with the leash off, distance 5.84 (4: further
//! when the moby is more than 110° off the facing), the end-sphere flags 0xb0; 6 (L02) only while Ratchet stands on
//! the moby; 7 (L18) the stick stops the turn for 300 ticks, then a tenth of it.

use super::{CamInput, Camera};
use crate::hero::physics::to_f32x3;
use crate::moby_update::triggers::{point_in_cuboid, point_in_cylinder, point_in_path, point_in_sphere};
use rc_formats::cameras::MobyFocus;
use std::f32::consts::{FRAC_PI_2, PI};

const DEG: f32 = 0.017_453_292;

/// A moby as class 18 reads it: its state byte +0x20 (≥ 0x80: gone) and position +0x10.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CamMoby {
    pub state: u8,
    pub pos: [f32; 3],
}

/// `FastDiffRots(a, b)`: the unsigned wrapped difference.
fn diff_rots(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % (2.0 * PI);
    if d > PI { d -= 2.0 * PI; }
    if d < -PI { d += 2.0 * PI; }
    d.abs()
}

fn wrap(a: f32) -> f32 { if a >= PI { a - 2.0 * PI } else if a < -PI { a + 2.0 * PI } else { a } }

impl Camera {
    fn focus_mut(&mut self, i: usize) -> &mut MobyFocus { self.level_cams.slots[i].focus.as_mut().unwrap() }

    /// The hook `0x2fc298` for slot `i`: with the follow camera current, a region whose moby is gone (+0x20 < 0) does
    /// nothing, else the update; the suppress word +0x50 cleared every tick. Answers 0.
    pub(super) fn focus_hook(&mut self, i: usize, inp: &CamInput) {
        let Some(f) = self.level_cams.slots[i].focus else { return };
        if self.follow_is_current() && 0 <= f.counter { self.focus_update(i, inp); }
        self.focus_mut(i).suppress = 0;
    }

    /// The moby of the region (`0x2fb9c8`'s start): the group's first member whose state is below 0x80, or the moby
    /// +0x28; `Err(())` when gone. An index the table does not have (the record's −1 on level 12) counts as present
    /// with no position [L: the game reads the word before the table].
    fn focus_moby(&self, f: &MobyFocus) -> Result<Option<CamMoby>, ()> {
        let w = &self.world;
        if f.group < 0 {
            let Ok(id) = usize::try_from(f.moby) else { return Ok(None) };
            return match w.mobys.get(&id) {
                Some(m) if (m.state as i8) < 0 => Err(()),
                m => Ok(m.copied()),
            };
        }
        let list = w.groups.get(&f.group).filter(|l| !l.is_empty()).ok_or(())?;
        list.iter().filter_map(|id| w.mobys.get(id)).find(|m| 0 <= m.state as i8).map(|m| Some(*m)).ok_or(())
    }

    /// The id of the region's moby as [`Self::focus_moby`] resolves it (+0x48).
    fn focus_moby_id(&self, f: &MobyFocus) -> Option<usize> {
        let w = &self.world;
        if f.group < 0 { return usize::try_from(f.moby).ok(); }
        w.groups.get(&f.group)?.iter().copied().find(|id| w.mobys.get(id).is_some_and(|m| 0 <= m.state as i8))
    }

    /// `0x2fb9c8` (module doc).
    fn focus_update(&mut self, i: usize, inp: &CamInput) {
        let f = self.level_cams.slots[i].focus.unwrap();
        if f.suppress != 0 {
            self.focus_mut(i).counter = 0;
            self.release_owner(i);
            return;
        }
        let m = match self.focus_moby(&f) {
            Ok(m) => m,
            Err(()) => {
                self.focus_mut(i).counter = -1;
                self.release_owner(i);
                return;
            }
        };
        let h = inp.hero;
        if f.mode == 6 && !self.focus_ground(f, h.ground_moby) {
            self.focus_mut(i).counter = 0;
            self.release_owner(i);
            return;
        }
        if self.claim_owner(i) < 1 {
            self.focus_mut(i).counter = 0;
            return;
        }
        if !self.focus_test(i, inp, m, self.focus_moby_id(&f)) {
            self.focus_mut(i).counter = 0;
            self.release_owner(i);
            return;
        }
        let rot_z = self.level_cams.slots[i].record.rot[2];
        let fwd = to_f32x3(self.cam.rows[0]);
        let cam_yaw = fwd[1].atan2(fwd[0]);
        let (rx, ry) = (inp.pad.rx.to_f32(), inp.pad.ry.to_f32());
        match f.mode {
            5 => {
                if f.counter < 2 {
                    self.focus_mut(i).yaw = rot_z;
                } else if 0.3 <= rx.abs() || 0.3 <= ry.abs() {
                    self.focus_mut(i).yaw = cam_yaw;
                }
                // `fast_add_rotations(yaw, π)` when it is more than 90° off the camera: the result is not stored.
            }
            4 => self.focus_mut(i).yaw = if FRAC_PI_2 < diff_rots(rot_z, cam_yaw) { wrap(rot_z + PI) } else { rot_z },
            _ => {}
        }
        let mode = f.mode;
        let (l300, s_idle, s_end, l200) = if mode == 1 { (300i16, 400i16, 560i16, 200i16) } else { (300, 350, 400, 200) };
        let mut turning = true;
        let mut scale = 1.0;
        {
            let v = self.focus_mut(i);
            v.counter += 1;
        }
        if mode == 7 {
            let v = self.focus_mut(i);
            if rx != 0.0 { v.counter = l300; }
            if l300 <= v.counter {
                turning = false;
                v.counter2 += 1;
                if l300 + 30 < v.counter {
                    scale = 0.1;
                    turning = true;
                }
            }
            if s_end <= v.counter {
                turning = true;
                v.counter2 = 0;
                v.counter = 1;
            }
            if l200 < v.counter && v.counter < l300 {
                v.counter2 = l300;
                v.counter = l200;
            }
        } else if mode != 1 && mode != 2 {
            let v = self.focus_mut(i);
            if l200 < v.counter { v.counter = l200; }
        } else {
            self.set_end_flags(0xb0);
            self.set_sphere_chain(1.0, 12.0, f32::from_bits(0x3de1_47ae));
            if rx != 0.0 || ry != 0.0 { self.focus_mut(i).counter = l300; }
            if l300 <= self.focus_mut(i).counter {
                let v = self.focus_mut(i);
                v.counter2 += 1;
                // Below the idle count, or the moby not within 30° of the view: no turn; else the restart.
                if v.counter2 < s_idle || !self.focus_view(i, 30.0, 0.0, m) {
                    turning = false;
                } else {
                    self.focus_mut(i).counter = s_end;
                }
            }
            let v = self.focus_mut(i);
            if s_end <= v.counter {
                turning = true;
                v.counter2 = 0;
                v.counter = 1;
            }
            if l200 < v.counter && v.counter < l300 {
                v.counter2 = l300;
                v.counter = l200;
            }
        }
        let v = *self.focus_mut(i);
        let mut turn = v.turn * scale * DEG * (v.counter as f32 / l200 as f32);
        let mut seen = false;
        if turning && self.focus_view(i, v.max_angle, v.max_pitch, m) {
            seen = true;
            if mode == 4 || mode == 5 {
                self.turn_toward(turn, 0.0, [v.yaw.cos(), v.yaw.sin(), 0.0]);
            } else {
                if mode == 3 && v.counter < 90 { turn *= v.counter as f32 / 90.0; }
                if let Some(m) = m { self.turn_toward_point(turn, 0.0, m.pos); }
            }
        }
        if v.distance != 0.0 {
            self.set_distance(v.distance, f32::from_bits(0x3b44_9ba6), false);
            self.lock_toward();
        }
        if v.pivot_height != 0.0 { self.set_pivot_height(v.pivot_height, f32::from_bits(0x3b44_9ba6)); }
        if v.look_height != 0.0 { self.set_look_height(v.look_height, f32::from_bits(0x3ba3_d70a), false); }
        if mode == 3 {
            self.set_leash(0);
            self.lock_toward();
            self.look_from_smoothed();
            self.set_v_spring(0.01, 0.2);
            let k = if v.counter < 120 { (v.counter as f32 / 120.0) * 0.02 + 0.01 } else { 0.03 };
            self.set_h_spring(k, 0.2);
        }
        if mode == 4 || mode == 5 {
            self.set_leash(0);
            self.lock_toward();
            self.look_from_smoothed();
            self.set_v_spring(0.02, 0.2);
            self.set_distance(f32::from_bits(0x40ba_e148), f32::from_bits(0x3b44_9ba6), false);
            if mode == 4 {
                if let Some(m) = m {
                    let hero = to_f32x3(h.pos);
                    let y = (m.pos[1] - hero[1]).atan2(m.pos[0] - hero[0]);
                    if seen && 1.919_862_2 < diff_rots(y, v.yaw) {
                        let d = if v.distance == 0.0 { 8.0 } else { v.distance + 3.36 };
                        self.set_distance(d, f32::from_bits(0x3b44_9ba6), false);
                    }
                }
            }
            self.set_end_flags(0xb0);
        }
    }

    /// Mode 6: Ratchet stands on the region's moby (0x13f64c): the moby resolved from the table index or the group's
    /// first live member.
    fn focus_ground(&self, f: MobyFocus, ground: Option<usize>) -> bool {
        let Some(g) = ground else { return false };
        let w = &self.world;
        if f.group < 0 { return usize::try_from(f.moby).is_ok_and(|id| id == g); }
        w.groups.get(&f.group).and_then(|l| l.iter().copied().find(|id| w.mobys.get(id).is_some_and(|m| 0 <= m.state as i8))) == Some(g)
    }

    /// The region test `0x2fb648`: the scripted focus moby (0x16735c, [`Camera::focus_moby`]) being the region's moby
    /// (`id`, +0x48) → inside; mode 6 → standing on it (checked by the caller); +0x22 = 1 → Ratchet within +0x24 of the
    /// moby; else the first shape the header has (cuboid +0x0c, cylinder +0x10, sphere +0x08, path +0x14) with
    /// Ratchet's feet, none → false.
    fn focus_test(&self, i: usize, inp: &CamInput, m: Option<CamMoby>, id: Option<usize>) -> bool {
        let f = self.level_cams.slots[i].focus.unwrap();
        let p = to_f32x3(inp.hero.pos);
        if self.focus_moby.is_some() && self.focus_moby == id { return true; }
        if f.mode == 6 { return true; }
        if f.near_kind == 1 {
            let Some(m) = m else { return false };
            let d = [p[0] - m.pos[0], p[1] - m.pos[1], p[2] - m.pos[2]];
            return (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < f.radius;
        }
        let Some(v) = self.level_cams.shapes() else { return false };
        let h = f.header;
        if 0 <= h.cuboid {
            point_in_cuboid(v, p, h.cuboid)
        } else if 0 <= h.cylinder {
            point_in_cylinder(v, p, h.cylinder)
        } else if 0 <= h.sphere {
            point_in_sphere(v, p, h.sphere)
        } else if 0 <= h.path {
            point_in_path(v, p, h.path)
        } else {
            false
        }
    }

    /// The view test `0x2fb788(max_angle, max_pitch)`: true when both are 0; else the moby's elevation from the camera
    /// within `max_pitch`° of the camera's (about the up 0x1672c0), and the flat angle between the camera's forward
    /// and the moby within `max_angle`° (a flat vector: false).
    pub(super) fn focus_view(&self, _i: usize, max_angle: f32, max_pitch: f32, m: Option<CamMoby>) -> bool {
        if max_angle == 0.0 && max_pitch == 0.0 { return true; }
        let Some(m) = m else { return false };
        let up = to_f32x3(self.g.up2);
        let cam = to_f32x3(self.cam.pos);
        let fwd = to_f32x3(self.cam.rows[0]);
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let d = [m.pos[0] - cam[0], m.pos[1] - cam[1], m.pos[2] - cam[2]];
        let dv = dot(d, up);
        let flat = [d[0] - up[0] * dv, d[1] - up[1] * dv, d[2] - up[2] * dv];
        let lf = dot(flat, flat).sqrt();
        let cv = dot(fwd, up);
        let cf = [fwd[0] - up[0] * cv, fwd[1] - up[1] * cv, fwd[2] - up[2] * cv];
        let lc = dot(cf, cf).sqrt();
        if max_pitch != 0.0 && max_pitch * DEG < diff_rots(dv.atan2(lf), cv.atan2(lc)) { return false; }
        if max_angle == 0.0 { return true; }
        if lc * lf == 0.0 { return false; }
        let a = (FRAC_PI_2 - (dot(flat, cf) / (lc * lf)).clamp(-1.0, 1.0).asin()).abs();
        a < max_angle * DEG
    }
}

#[cfg(test)]
mod tests;
