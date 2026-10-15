//! Kerwan's path-mover lines, class 899 (level 03, 4 placed; census U140) and the movers 898 it creates (level03
//! 0x2db280, the spawn `0x2db198`, the mover 0x2db020). A spawner on a path that fills the path with platforms 2.9
//! units apart at its init and then creates one more at the path's start every 2.9 / speed ticks; each platform 898
//! glides along the path at 3 units a second (`dt·3`, a level global the spawner writes), carries what stands on it
//! (`CarryRiders`), and is deleted near the path's end. The spawner keeps class 898's sound 0 looping at itself. Read
//! from the level03 decomp and disassembly. Native `f32`.
//!
//! **Not a spawner system**: the spawn is this class's own `CreateMoby` (`World::create_moby`) and field writes; no other
//! class on the disc calls 0x2db198 (level 03 only). Shared calls used: `CarryRiders` (`triggers::carry_riders`),
//! Ratchet's light (`units::take_hero_light`), `MobyBuildMatrix`.
//!
//! **899 pvars**: +0x00 s32 the path (spline index, −1 none), +0x04 s32 the spawn timer, +0x08 s32 the voice slot.
//! **898 pvars** (a created moby's 0x80 bytes): +0x08 the platform block's offset (0x20), +0x20 the platform block,
//! +0x60 the path (a pointer to the spline in the game; the port keeps the spline index), +0x64 f32 the distance along
//! the path.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2db280 state 0 | path −1 → `printf("…", +0xa8)` (a debug print: n/a) and `DeleteMoby` | [`spawner_update`] |
//! | | the global speed (gp−0x5040, 0x161bc0) = dt (0x15ed6c) · 3.0 | [`spawner_update`] (`units::Globals` word 0x161bc0) |
//! | | each point i < count − 2: point i's w = `VecDistance(p[i], p[i+1])` (0x1f8de8 = L01 0x221360), written into the level's spline (every reader of the path sees it) | [`spawner_update`] (`Services::splines`) |
//! | | timer = 0, state 1, slot = −1 | [`spawner_update`] |
//! | | f = 2.9; while f < count · p[0].w (`c.lt.s`, tested before the first): `0x2db198(f, path)`, f += 2.9 | [`spawner_update`] ([`spawn`]) |
//! | | +0x30 update distance = 0x60 | [`spawner_update`] |
//! | every state | `FastDecTimer(&+0x04)` (0x1f88d0 = L01 0x220e78) not running → timer = `(int)(2.9 / speed)` (0x1f9be8, truncation), `0x2db198(0, path)` | [`spawner_update`] |
//! | | `SoundIsAlive(m, slot)` (0x27a108) false → slot = `PlayClassSoundAs(0, 4, m, 898)` (0x27a4d8 = L01 0x2a16c0) | [`spawner_update`] (`World::play_sound_as`) |
//! | 0x2db198 | `CreateMoby(898)` (0x23d3a8); none → nothing | [`spawn`] |
//! | | state 0, +0x30 = 0x80, +0x32 = 0x40, +0x31 = 1, mode \|= 0x20 (a carrier) | [`spawn`] |
//! | | Ratchet's light word and ambient (0x24bc00 = L01 0x272078) | [`spawn`] (`units::take_hero_light`) |
//! | | position (four words) = point 0; Euler = 0 (0x1f8bf8 = L01 0x221170); z = `FastArcTan(last.x − x, last.y − y)` | [`spawn`] |
//! | | pvar +0x64 = f, +0x60 = the path, +0x08 = the block (+0x20); `MobyBuildMatrix` (0x23f968 = L01 0x265bd8) | [`spawn`] (`World::build_matrix`) |
//! | 0x2db020 | d (+0x64) += the global speed | [`mover_update`] |
//! | | `(cos z · d, sin z · d, 0) + point 0` (`fast_cos` / `fast_sin` 0x1f9180 / 0x1f9198): a dead value (overwritten below) | n/a |
//! | | i = `(int)(d / p[0].w)` (truncation), frac = (d − i · p[0].w) / p[0].w | [`mover_update`] |
//! | | i ≥ count − 2 → `DeleteMoby` (0x23d6d8) | [`mover_update`] |
//! | | else position = `VecAdd(VecScale(frac, VecSub(p[i+1], p[i])), p[i])` (the VU0 ops write xyz and keep the first input's w: position w = p[i+1].w) | [`mover_update`] |
//! | | `CarryRiders(+0x20, position − old (w = the new position's), rot, rot)` (0x24ee28 = L01 0x2755f8: the displacement alone) | [`mover_update`] (`triggers::carry_riders`) |
//! | | no particle, hit, flag, light, bolt; no other moby written | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::gold_bolt::fast_dec_timer;
use crate::moby_update::creature::{self as c, dist3, DT};
use crate::moby_update::services::World;
use crate::moby_update::triggers;

pub const REFERENCE_LEVEL: u32 = 3;
/// The spawner's update and its classes.
pub const UPDATE_FN: u32 = 0x2d_b280;
pub const CLASSES: [i16; 1] = [899];
/// The mover's update and class (0x382).
pub const MOVER_FN: u32 = 0x2d_b020;
pub const MOVER: i16 = 898;
pub const MOVER_CLASSES: [i16; 1] = [MOVER];
/// gp−0x5040 (level03 0x161bc0): the movers' speed per tick.
pub const SPEED_WORD: u32 = 0x16_1bc0;
/// The spacing of the movers along the path.
const GAP: f32 = 2.9;

fn speed(w: &World) -> f32 { f32::from_bits(w.svc.units.word(SPEED_WORD)) }

fn spline(w: &World, path: i32) -> Option<Vec<[f32; 4]>> {
    let s = w.svc.splines.get(usize::try_from(path).ok()?)?;
    Some(s.iter().map(|p| p.map(f32::from_bits)).collect())
}

/// Level03 0x2db280, the spawner (module doc).
pub fn spawner_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    let path = c::pi32(w, id, 0);
    if w.m(id).state == 0 {
        if path == -1 {
            w.delete_moby(id);
            return;
        }
        w.svc.units.set_word(SPEED_WORD, (DT * 3.0).to_bits());
        let Some(pts) = spline(w, path) else { return };
        let n = pts.len() as i32;
        for i in 0..(n - 2).max(0) as usize {
            let d = dist3(pts[i], pts[i + 1]);
            w.svc.splines[path as usize][i][3] = d.to_bits();
        }
        c::set_pi32(w, id, 4, 0);
        w.mm(id).state = 1;
        c::set_pi32(w, id, 8, -1);
        let pts = spline(w, path).unwrap_or_default();
        let len = |pts: &[[f32; 4]]| pts.len() as f32 * pts.first().map_or(0.0, |p| p[3]);
        let mut f = GAP;
        while f < len(&pts) {
            spawn(w, f, path);
            f += GAP;
        }
        w.mm(id).update_dist = 0x60;
    }
    let mut t = c::pi32(w, id, 4);
    let r = fast_dec_timer(&mut t);
    c::set_pi32(w, id, 4, t);
    if r != 0 {
        c::set_pi32(w, id, 4, (GAP / speed(w)) as i32);
        spawn(w, 0.0, path);
    }
    let slot = c::pi32(w, id, 8);
    if !w.sound_alive(slot, id) {
        let s = w.play_sound_as(0, 4, id, MOVER);
        c::set_pi32(w, id, 8, s);
    }
}

/// Level03 `0x2db198(f, path)`: a mover at distance `f` (module table). None when the table is full or the path
/// is missing.
pub fn spawn(w: &mut World, f: f32, path: i32) -> Option<MobyId> {
    let pts = spline(w, path)?;
    let m = w.create_moby(MOVER)?;
    {
        let mm = w.mm(m);
        mm.state = 0;
        mm.update_dist = 0x80;
        mm.draw_dist = 0x40;
        mm.visible = 1;
        mm.mode |= 0x20;
    }
    crate::moby_update::classes::units::take_hero_light(w, m);
    let p0 = pts.first().copied().unwrap_or([0.0; 4]);
    let last = pts.last().copied().unwrap_or(p0);
    {
        let mm = w.mm(m);
        mm.position = p0;
        mm.rotation = [0.0; 4];
        mm.rotation[2] = c::atan(last[0] - p0[0], last[1] - p0[1]);
    }
    if w.m(m).pvars.len() >= 0x68 {
        c::set_pf(w, m, 0x64, f);
        c::set_pi32(w, m, 0x60, path);
        c::set_pi32(w, m, 8, 0x20);
    }
    w.build_matrix(m);
    Some(m)
}

/// Level03 0x2db020, the mover (module doc).
pub fn mover_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x68 { return; }
    let old = w.m(id).position;
    let d = c::pf(w, id, 0x64) + speed(w);
    c::set_pf(w, id, 0x64, d);
    let Some(pts) = spline(w, c::pi32(w, id, 0x60)) else { return };
    let Some(p0) = pts.first() else { return };
    let seg = p0[3];
    let i = (d / seg) as i32;
    let frac = (d - i as f32 * seg) / seg;
    if i >= pts.len() as i32 - 2 {
        w.delete_moby(id);
        return;
    }
    let (a, b) = (pts[i as usize], pts[i as usize + 1]);
    let pos = [(b[0] - a[0]) * frac + a[0], (b[1] - a[1]) * frac + a[1], (b[2] - a[2]) * frac + a[2], b[3]];
    w.mm(id).position = pos;
    let v = [pos[0] - old[0], pos[1] - old[1], pos[2] - old[2], pos[3]];
    let rot = w.m(id).rotation;
    if w.m(id).pvars.len() >= 0x60 { triggers::carry_riders(&mut w.mm(id).pvars, 0x20, v, rot, rot); }
}
