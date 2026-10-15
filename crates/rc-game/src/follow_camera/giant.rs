//! **Class 22, Giant Clank's view** (level 15: hook `0x2f8ba8` → `0x2f88e8`, the region test `0x2f85a8`, the view test
//! `0x2f8698`; init / update / pre empty). A region that, while Giant Clank (body 2, 0x1413f4) stands in it and the
//! follow camera is current, turns the follow camera toward the centre of a cuboid (+0x48) and retunes its distance and
//! heights: class 18's modes 1 / 2 ([`super::focus`]) aimed at a cuboid instead of a moby. Its hook answers −1: it
//! never becomes the current camera. Spec: docs/plan/player_controller.md §15. Native `f32`.
//!
//! **The record**: the header (the shapes +0x08..+0x14), +0x00 the turn (degrees a tick at full strength), +0x20 (s16)
//! the counter (−1: off), +0x28 a cylinder where it does nothing, +0x34 / +0x38 / +0x44 the distance, pivot and look
//! heights (0: unchanged), +0x3c the idle count, +0x48 the cuboid it turns toward.
//!
//! | address | call / branch | port |
//! |---|---|---|
//! | 0x2f8ba8 | the follow camera current (0x167400 +0x86 = 0), +0x20 ≥ 0, body 2 → `0x2f88e8`; −1 | [`Camera::giant_hook`] |
//! | 0x2f88e8 | the owner lock `0x2f2600` (= `0x313598`) < 1 → +0x20 = 0; the region test fails → +0x20 = 0, the lock released (`0x2f25c8`) | [`Camera::giant_update`] |
//! | | the follow camera's D+0x230 = 2, +0x20 + 1, the sphere chain (1, 12, 0.11) (`0x2f28b8`); the right stick (0x13ca40 / 0x13ca44) → +0x20 = 300; from 300: +0x3c + 1, no turn below 400, then the view test (30°) restarts it at 560 (else no turn); from 560: +0x3c = 0, +0x20 = 1; between 200 and 300: +0x3c = 300, +0x20 = 200 | `giant_update` |
//! | | the turn `0x2f2c10` (= `0x313b48`) at +0x00° · +0x20 / 200 toward the cuboid's centre; distance (+ the lock `0x2f2860`), pivot, look (`0x2f2690` / `0x2f26f8` / `0x2f2730`, rates 0.003 / 0.003 / 0.005) | `giant_update` |
//! | 0x2f85a8 | +0x28 ≥ 0 and Ratchet in that cylinder → no; then the first shape the header has (cuboid +0x0c, cylinder +0x10, sphere +0x08, path +0x14), none → no | [`Camera::giant_region`] |
//! | 0x2f8698 | the view test of class 18 (`0x2fb788`) toward the cuboid's centre | [`Camera::giant_view`] |

use super::{CamInput, Camera};
use crate::hero::physics::to_f32x3;
use crate::moby_update::triggers::{point_in_cuboid, point_in_cylinder, point_in_path, point_in_sphere};
use rc_formats::cameras::CameraHeader;

/// The camera class.
pub const CLASS_GIANT: i32 = 22;
/// Level 15's hook (0x80 bytes) and helpers ([`super::level::CameraPorts::from_overlays`]).
pub const GIANT_HOOK: u32 = 0x2f_8ba8;
pub const GIANT_HELPERS: [u32; 3] = [0x2f_88e8, 0x2f_85a8, 0x2f_8698];

const DEG: f32 = 0.017_453_292;

/// Class 22's record (module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GiantView {
    pub header: CameraHeader,
    pub turn: f32,
    pub counter: i16,
    pub exclude: i32,
    pub distance: f32,
    pub pivot: f32,
    pub idle: i32,
    pub look: f32,
    pub cuboid: i32,
}

impl GiantView {
    pub fn parse(p: &[u8]) -> Option<GiantView> {
        let w = |o: usize| p.get(o..o + 4).map(|b| [b[0], b[1], b[2], b[3]]);
        let i = |o: usize| w(o).map(i32::from_le_bytes);
        let f = |o: usize| w(o).map(f32::from_le_bytes);
        Some(GiantView {
            header: CameraHeader::parse(p)?,
            turn: f(0x00)?,
            counter: i16::from_le_bytes([*p.get(0x20)?, *p.get(0x21)?]),
            exclude: i(0x28)?,
            distance: f(0x34)?,
            pivot: f(0x38)?,
            idle: i(0x3c)?,
            look: f(0x44)?,
            cuboid: i(0x48)?,
        })
    }
}

fn ticks(n: i32) -> i32 { crate::hero::physics::ticks(n) }

impl Camera {
    fn giant_mut(&mut self, i: usize) -> &mut GiantView { self.level_cams.slots[i].giant.as_mut().unwrap() }

    /// The cuboid +0x48's centre.
    fn giant_target(&self, g: &GiantView) -> Option<[f32; 3]> {
        let c = usize::try_from(g.cuboid).ok()?;
        Some(self.level_cams.shapes()?.cuboids.get(c)?.centre())
    }

    /// `0x2f8ba8` for slot `i` (module doc). Answers −1.
    pub(super) fn giant_hook(&mut self, i: usize, inp: &CamInput) -> i32 {
        let Some(g) = self.level_cams.slots[i].giant else { return -1 };
        if self.follow_is_current() && 0 <= g.counter && inp.hero.mode == 2 { self.giant_update(i, inp); }
        -1
    }

    /// `0x2f88e8` (module doc).
    fn giant_update(&mut self, i: usize, inp: &CamInput) {
        if self.claim_owner(i) < 1 {
            self.giant_mut(i).counter = 0;
            return;
        }
        if !self.giant_region(i, inp) {
            self.giant_mut(i).counter = 0;
            self.release_owner(i);
            return;
        }
        self.level_cams.focus = 2;
        let (l300, l400, l560, l200) = (ticks(300) as i16, ticks(400), ticks(0x230) as i16, ticks(200) as i16);
        let mut turning = true;
        self.giant_mut(i).counter += 1;
        self.set_sphere_chain(1.0, 12.0, f32::from_bits(0x3de1_47ae));
        if inp.pad.rx.to_f32() != 0.0 || inp.pad.ry.to_f32() != 0.0 { self.giant_mut(i).counter = l300; }
        if l300 <= self.giant_mut(i).counter {
            let v = self.giant_mut(i);
            v.idle += 1;
            if v.idle < l400 || !self.giant_view(i, 30.0, 0.0) {
                turning = false;
            } else {
                self.giant_mut(i).counter = l560;
            }
        }
        let v = self.giant_mut(i);
        if l560 <= v.counter {
            turning = true;
            v.idle = 0;
            v.counter = 1;
        }
        if l200 < v.counter && v.counter < l300 {
            v.idle = l300 as i32;
            v.counter = l200;
        }
        let g = *v;
        if turning {
            if let Some(t) = self.giant_target(&g) { self.turn_toward_point(g.turn * DEG * (g.counter as f32 / l200 as f32), 0.0, t); }
        }
        if g.distance != 0.0 {
            self.set_distance(g.distance, f32::from_bits(0x3b44_9ba6), false);
            self.lock_toward();
        }
        if g.pivot != 0.0 { self.set_pivot_height(g.pivot, f32::from_bits(0x3b44_9ba6)); }
        if g.look != 0.0 { self.set_look_height(g.look, f32::from_bits(0x3ba3_d70a), false); }
    }

    /// `0x2f85a8` (module doc).
    fn giant_region(&self, i: usize, inp: &CamInput) -> bool {
        let Some(g) = self.level_cams.slots[i].giant else { return false };
        let Some(v) = self.level_cams.shapes() else { return false };
        let p = to_f32x3(inp.hero.pos);
        if 0 <= g.exclude && point_in_cylinder(v, p, g.exclude) { return false; }
        let h = g.header;
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

    /// `0x2f8698(max_angle, max_pitch)`: class 18's view test toward the cuboid's centre.
    fn giant_view(&self, i: usize, max_angle: f32, max_pitch: f32) -> bool {
        let g = self.level_cams.slots[i].giant.unwrap_or_default();
        let at = self.giant_target(&g).map(|pos| super::focus::CamMoby { state: 0, pos });
        self.focus_view(i, max_angle, max_pitch, at)
    }
}
