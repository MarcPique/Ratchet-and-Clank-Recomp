//! **The camera moby** (class 0x3ef = 1007): an invisible moby with collision that `UpdateAllCameras` keeps while the
//! follow camera is current, so that Ratchet running toward the camera bumps into it instead of walking through
//! the camera. Level01: `Camera_handleCollWithHero` 0x20d068 (every tick, after the switch, before the current
//! camera's update), the create `0x2ba760`, the class update `0x2ba7c8` (level00 0x2a4e48, level03 0x295200,
//! level10 0x295840: the class registry finds the copies, `ClassUpdate::CameraMoby`). Class 1007's blob is on every
//! level with a collision blob (+0x10 = 0x90).
//!
//! **System or not.** Part of the one camera system (0x20d068 is engine code on every level); the moby is one class
//! with one update. The camera asks, the tick does it ([`Call`], `crate::tick`: the moby world creates and deletes
//! through `crate::tick::MobySystem`).
//!
//! | address | call / branch | port |
//! |---|---|---|
//! | 0x20d068 | the current camera is the follow camera (+0x86 = 0): no camera moby (0x167354 = 0) → `0x2ba760`(0x167240) (again each tick while the create fails) | [`Camera::handle_coll_with_hero`] → [`Call::Create`] |
//! | | another camera current: the moby → `DeleteMoby`, 0x167354 = 0 | `handle_coll_with_hero` → [`Call::Delete`] |
//! | 0x2ba760 | `CreateMoby(0x3ef)`; +0x30 = 0xff (always updated), +0x34 \|= 0x41 (hidden, no animation), position = the camera (0x167240: the last published one [L: at a switch the game has the new camera's position there; the update places the moby before the hero moves]), `MobyBuildMatrix` | `crate::tick` (`MobySystem::create_moby`) |
//! | 0x2ba7c8 | no current camera (0x167280 = 0) → position (5, 5, 5) | n/a (the port always has one) |
//! | | position = the current camera's (+0x30, `LoopGlobals::cam_pos`); d = it − the feet; \|d · up\| ≥ 2.5 (Ratchet's moby row +0xe0) → stays there | [`update`] |
//! | | the flat part ≥ 0.1, Ratchet's yaw (0x13f3e8) 135° or more off the camera's (0x167258), his flat speed 0x13f4b4 ≥ 0.1·dt: under water (group 0x11) → stays at the camera; else the camera's spot on Ratchet's plane (the camera's flat part + up·(feet · up)) | `update` |
//! | | else → (15, 15, 15) (parked away) | `update` |
//! | "its entry sound" (gaps.md G-HERO-026) | none: neither function plays a sound | n/a |

use super::level::CLASS_FOLLOW;
use super::Camera;
use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;
use crate::pad::fast_diff_rots;
use crate::ps2v::Pf;

/// The camera moby's class and its level-01 update.
pub const CLASS: i16 = 0x3ef;
pub const UPDATE_FN: u32 = 0x2b_a7c8;
pub const CLASSES: [i16; 1] = [CLASS];

/// What the camera asks the moby world for after its update (`Camera_handleCollWithHero`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// `0x2ba760(pos)`: create the camera moby at `pos`; the tick answers with [`Camera::camera_moby_created`].
    Create { pos: [f32; 3] },
    /// `DeleteMoby(0x167354)`.
    Delete(MobyId),
}

impl Camera {
    /// `Camera_handleCollWithHero` 0x20d068 for the camera now current (module doc); the request goes to the tick
    /// ([`Camera::cam_moby_call`]).
    pub(super) fn handle_coll_with_hero(&mut self) {
        self.cam_moby_call = None;
        if self.current_class() == CLASS_FOLLOW {
            if self.cam_moby.is_none() { self.cam_moby_call = Some(Call::Create { pos: self.out.pos_f32() }); }
        } else if let Some(id) = self.cam_moby.take() {
            self.cam_moby_call = Some(Call::Delete(id));
        }
    }

    /// The tick's answer to [`Call::Create`]: the moby made (None: no slot or no class; the camera asks again next tick).
    pub fn camera_moby_created(&mut self, id: Option<MobyId>) { self.cam_moby = id; }
}

/// `0x15ed6c`: dt (NTSC).
const DT: f32 = 1.0 / 60.0;

/// The class-1007 update `0x2ba7c8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let cam = w.hero.loop_in.cam_pos;
    let h = w.hero;
    let feet = crate::hero::physics::to_f32x3(h.pos);
    let up = w.hero_moby.and_then(|m| w.table.mobys.get(m)).map_or([0.0, 0.0, 1.0], |m| [m.rows[2][0], m.rows[2][1], m.rows[2][2]]);
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let flat = |v: [f32; 3]| {
        let k = dot(v, up);
        [v[0] - up[0] * k, v[1] - up[1] * k, v[2] - up[2] * k]
    };
    let mut pos = cam;
    let d = [cam[0] - feet[0], cam[1] - feet[1], cam[2] - feet[2]];
    let k = dot(d, up);
    if k.abs() < 2.5 {
        let f = flat(d);
        let off = fast_diff_rots(h.rot[2], Pf::f(w.camera_yaw)).to_f32();
        if 0.1 <= dot(f, f).sqrt() && 2.356_194_5 <= off && DT * 0.1 <= h.eff_len_xy.to_f32() {
            if h.group != 0x11 {
                let c = flat(cam);
                let kh = dot(feet, up);
                pos = [c[0] + up[0] * kh, c[1] + up[1] * kh, c[2] + up[2] * kh];
            }
        } else {
            pos = [15.0; 3];
        }
    }
    let m = w.mm(id);
    m.position[0] = pos[0];
    m.position[1] = pos[1];
    m.position[2] = pos[2];
}
