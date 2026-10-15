//! **The Visibomb** (item 13, gun class 163; level01 item update `0x2c8cc0`, read from the decompiler output; the
//! item table 0x179f40 gives item 13 the class 163). No weapon-check case: its update fires. Its missile is class 172
//! (`crate::moby_update::classes::visibomb`), the missile view the type-6 camera (`crate::follow_camera::type6`),
//! Ratchet's state while it flies 0x1d (`super::scripted`). docs/plan/hero_gameplay.md §17.
//!
//! **Pvars** ([`Visibomb`]): +0x00 the fire timer, +0x04 the missile in flight (cleared by the flight's end
//! `0x2cb788`: here, the missile gone from the table).
//!
//! **The update**: the fire timer steps (`FastDecTimer`). **Firing**: ○ pressed (the slot's fire mask), the timer out,
//! no missile in flight, not 0x1413fc, movement group 0, 1, 8 or 0xc (not 2..7, 9..0xb, ≥ 0xd), not in states 0x72 /
//! 0x65, the camera not the script camera (type 5: every script camera on the ported levels holds Ratchet in 0x72 or
//! 100, so the state tests cover it [L]), on the ground (0x13f650 ≠ 0), not on the classes 0x13e / 0x46f (the ground
//! moby 0x13f64c: never set in the port). One ammo (`0x249450(−1, 1)`); none → the gun's class sound 0 (the click).
//! With it: the aim — **first person** (state 1 with Ratchet hidden by the first-person camera, 0x1413f5): along the
//! view, yaw `atan(fwd.x, fwd.y)`, pitch `−atan(|fwd.xy|, fwd.z)`, from the eye + 0.3·fwd − 0.35·up; else Ratchet's
//! yaw (0x13f9d8 = 0x13f3e8 outside the grind group), pitch −0, from `polar(0.6, yaw, −0)` + the gun (0x141618 = 1,
//! the arm raised at a wall: the pitch of the gun's row 1, the point 0.6 along it); the gun's sequence 3; the help box
//! suspended (`0x225a28`); the launch `0x2cb540(yaw, pitch, gun, point)` (+ `SetState(0x1d, 1)`, made right after the
//! slot loop: `super::weapons::after_items`); the shot statistics 0x1416e8 (not ported: G-SAV-009); the fire timer
//! `ticks(60)`. **Tail**: the gun's sequence 1 once its sequence wrapped (+0x70 & 2); +0x34 |= 4 (keep the matrix:
//! the hand item's matrix is the attach's in the port, n/a).
//!
//! **Native.** Standard `f32`; the item is not a table moby. Gold (0x13e52d) is not mirrored.

use super::guns::{self, add3, with_len};
use super::items::{HitSink, ItemEnv};
use super::physics::{ticks, to_f32x3};
use super::Hero;
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::visibomb as missile;
use crate::moby_update::creature::atan;
use crate::rng::Rng;
use rc_formats::moby_anim;

pub const VISIBOMB: i32 = 13;
pub const CLASS: i16 = 163;
/// The gun's class sounds: the empty click.
pub const EMPTY_SOUND: i32 = 0;
/// The gun's sequences: the launch, and the one it returns to.
pub const FIRE_SEQ: u8 = 3;
pub const REST_SEQ: u8 = 1;

/// The gun's pvars (item moby +0x78).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Visibomb {
    pub fire_timer: i32,
    pub missile: Option<MobyId>,
}

/// Movement groups the Visibomb fires in (not 2..7, 9..0xb, ≥ 0xd).
pub fn group_fires(g: i32) -> bool { matches!(g, 0 | 1 | 8 | 0xc) }

/// `0x2c8cc0`, the Visibomb's update (from the slot loop with the slot ready).
pub fn update(hero: &mut Hero, table: &mut MobyTable, _anim: &dyn super::anim::AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    let Some(class) = hero.items.slot.item.as_ref().and_then(|m| env.data.class(m.o_class)).cloned() else { return };
    let v = &mut hero.weapons.visibomb;
    guns::dec(&mut v.fire_timer);
    if v.missile.is_some_and(|m| table.mobys.get(m).is_none_or(|mo| mo.o_class != missile::CLASS || mo.state >= 0x80)) { v.missile = None; }
    let pressed = env.pad.pressed & hero.items.slot.fire_mask != 0;
    let fires = pressed
        && hero.weapons.visibomb.fire_timer == 0
        && hero.weapons.visibomb.missile.is_none()
        && hero.items.f13fc == 0
        && group_fires(hero.group)
        && !matches!(hero.state, 0x72 | 0x65)
        && hero.grounded_ticks != 0;
    if fires {
        let id = hero.items.slot.id;
        if !hero.weapons.use_ammo(id, 1) {
            hero.fx.item_sounds.push(EMPTY_SOUND);
        } else {
            fire(hero, table, env, hits, rng, &class);
        }
    }
    // The tail.
    if let Some(it) = hero.items.slot.item.as_mut() {
        if it.anim.flags & 2 != 0 && it.anim.seq_b != REST_SEQ { moby_anim::set_sequence(&mut it.anim, &class.anim, REST_SEQ, 0, 0, &mut it.snapshot); }
    }
}

/// The aim `(yaw, pitch, point)` of a launch (module docs).
pub fn aim(hero: &Hero, env: &ItemEnv) -> (f32, f32, [f32; 3]) {
    let look = hero.state == 1 && hero.f13f5 != 0;
    if let (true, Some((eye, f)), Some(u)) = (look, env.camera, env.camera_up) {
        let yaw = atan(f[0], f[1]);
        let pitch = -atan((f[0] * f[0] + f[1] * f[1]).sqrt(), f[2]);
        return (yaw, pitch, add3(add3(with_len(f, 0.3), with_len(u, -0.35)), eye));
    }
    let yaw = hero.rot[2].to_f32();
    let gun = hero.items.slot.item.as_ref().map_or(to_f32x3(hero.pos), |it| it.position);
    // The arm raised at a wall (0x141618 = 1): the pitch of the gun's row 1 (its +0xd0: the barrel is −row 1),
    // `atan(|row1.xy|, row1.z)`, the point 0.6 along it.
    if let (true, Some(r)) = (super::guns::arm_raised(hero), super::guns::item_rows(hero)) {
        let p = atan((r[1][0] * r[1][0] + r[1][1] * r[1][1]).sqrt(), r[1][2]);
        return (yaw, p, add3(crate::targeting::polar(0.6, yaw, -p), gun));
    }
    (yaw, -0.0, add3(crate::targeting::polar(0.6, yaw, -0.0), gun))
}

/// The firing part of `0x2c8cc0` (with the ammo used).
fn fire(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng, class: &super::items::ItemClass) {
    let (yaw, pitch, point) = aim(hero, env);
    if let Some(it) = hero.items.slot.item.as_mut() {
        if it.anim.seq_b != FIRE_SEQ { moby_anim::set_sequence(&mut it.anim, &class.anim, FIRE_SEQ, 0, 0, &mut it.snapshot); }
    }
    let gun = hero.items.slot.item.as_ref().map_or(to_f32x3(hero.pos), |it| it.position);
    let mut launched = None;
    hits.world(table, hero, rng, env.frame as u64, &mut |w| {
        w.svc.help.suspend();
        launched = missile::launch(w, yaw, pitch, gun, point);
    });
    hero.weapons.visibomb.missile = launched;
    if launched.is_some() { hero.weapons.deferred = Some(super::scripted::MISSILE); }
    hero.weapons.visibomb.fire_timer = ticks(0x3c);
}
