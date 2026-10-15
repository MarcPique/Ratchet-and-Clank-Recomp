//! The water splash, class 775 (level01 spawner `FUN_002ff768`, update `0x2ff810`; read from the disassembly): a
//! translucent spray shell that grows and fades. Spawned by the Plumber's jump in the Novalis scene 1
//! (`CutsceneFxUpdate`, size 3, alpha 0x70), the bomb's water entry (size 2, alpha 0x70) and the hero's big splashes
//! (`0x22b3a8(…, 1)`: size 2.25, the class's alpha; `crate::hero::swim::effects`).
//!
//! **Spawn** `(size, pos)`: `CreateMoby(775)`; Ratchet's light word (+0x38..: light sets, `FUN_00272078`) with the
//! ambient (40, 40, 70) (`0x2650d0`); update distance 0xff, draw distance 0x40; scale × size; position; rotation z =
//! `rand_angle` (one draw, with a moby); `MobyBuildMatrix`. The caller may set its alpha (+0x23).
//! **Update**: alpha − 3 (a byte), scale × 1.025; deleted when the alpha is below 4. Drawn by the moby renderer as a
//! translucent moby (alpha < 0x80: `MobyBlend::Translucent`, blended on display bytes). Standard `f32`.

use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

/// The update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2ff810;
/// The splash class.
pub const CLASS: i16 = 775;
pub const CLASSES: [i16; 1] = [CLASS];

/// `FUN_002ff768(size, pos)`.
pub fn spawn(w: &mut World, size: f32, pos: [f32; 4]) -> Option<MobyId> {
    let m = w.create_moby(CLASS)?;
    let hero = w.hero_moby.map(|h| w.m(h).light);
    let a = w.rng.rand_angle();
    fill(w.mm(m), hero, size, pos, a);
    w.build_matrix(m);
    Some(m)
}

/// The fields `FUN_002ff768` writes into the new moby after `CreateMoby(775)`: Ratchet's light word `hero_light`, the
/// ambient (40, 40, 70), update / draw distances, scale × `size`, `pos`, rotation z = `angle` (its `rand_angle`).
/// Shared by [`spawn`] (the moby loop's callers) and the hero's splashes (`crate::hero::fx::create_mobys`, whose draw
/// is made at the hero's call).
pub fn fill(mo: &mut crate::moby_runtime::Moby, hero_light: Option<u32>, size: f32, pos: [f32; 4], angle: f32) {
    if let Some(l) = hero_light { mo.light = l; }
    mo.ambient = [40, 40, 70, mo.ambient[3]];
    mo.update_dist = 0xff;
    mo.draw_dist = 0x40;
    mo.scale *= size;
    mo.position = pos;
    mo.rotation[2] = angle;
}

/// `0x2ff810`.
pub fn update(w: &mut World, id: MobyId) {
    let m = w.mm(id);
    m.alpha = m.alpha.wrapping_sub(3);
    m.scale *= 1.025;
    if m.alpha < 4 { w.delete_moby(id); }
}
