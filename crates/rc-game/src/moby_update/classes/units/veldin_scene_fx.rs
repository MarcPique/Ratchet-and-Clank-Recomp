//! **Veldin's cutscene FX driver, class 1545** (level00 `0x2e3800`, census U38; one placed): level 00's counterpart of
//! level 01's `CutsceneFxUpdate` 1546 ([`super::super::cutscene_fx`]), with Veldin's scenes. While a scene plays (game
//! mode 2) it fires the infobot's thrusters at its actor (scene 2 from tick 1560: actor 3; scene 3: actor 2) and, in
//! scene 4 between ticks 150 and 220, the dust of actor 3's joint list 0 (a jet glow and ten grey puffs a tick).
//! Read from the level00 decomp. Native `f32`; the draws at the game's points.
//!
//! ## Coverage (`0x2e3800`)
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, update distance 0xff | [`update`] |
//! | state 1 | not game mode 2 (0x15f5c4) → nothing | [`update`] |
//! | | scene 0x16c890 = 2 from `ticks(0x618)` (scene tick 0x16c894), or scene 3: `FUN_002637f8(actor)` (L01 `0x278450`, the infobot's thrusters) on actor 3 (scene 2) / 2 (scene 3) | [`update`] (`cutscene_fx::infobot_thrusters`) |
//! | | scene 4 with actor 3 (0x16c9e4), ticks(150) ≤ tick ≤ ticks(220): its joint list 0 point (`0x24f7c8`); `FUN_00263b70(120000, 50000, p, p, 0)` (L01 `0x278810`, the jet glow); ten `PartType23Spawn(0.75, 1, 1.03, 120000, p, ±rand_range(0, 4) (sign `randi(2)`), 0, alpha \| 0x787878)` with alpha `rand_range(0x40, 0x80)`, patched: ALPHA 0x44, rotation `rand_range(0, 255)`, timer `ticks(60)`, phase 2 from alpha over the timer | [`update`], [`dust`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::cutscene_fx::infobot_thrusters;
use crate::moby_update::creature::fx;
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 0;
pub const UPDATE_FN: u32 = 0x2e_3800;
pub const CLASSES: [i16; 1] = [1545];

/// Level00 `0x2e3800` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            return;
        }
        1 => {}
        _ => return,
    }
    if w.svc.game_mode != 2 { return; }
    let Some(scene) = w.svc.cinematic.scene.clone() else { return };
    let (sid, tick) = (scene.id as i32, scene.tick);
    if (sid == 2 && w.ticks(0x618) <= tick) || sid == 3 {
        let k = if sid == 2 { 3 } else { 2 };
        if let Some(a) = scene.actors.get(k) { infobot_thrusters(w, a); }
    }
    if sid == 4 && w.ticks(0x96) <= tick && tick <= w.ticks(0xdc) {
        let Some(a) = scene.actors.get(3) else { return };
        let p = a.joint_point(0);
        fx::jet_puffs(w, 120000.0, 50000.0, p, p, [0.0; 4]);
        for _ in 0..10 { dust(w, p); }
    }
}

/// One dust puff of scene 4 (module table): its draws in the game's order, the record's patch after the spawn.
fn dust(w: &mut World, p: [f32; 4]) {
    let alpha = w.rng.rand_range(0x40, 0x80) as u8;
    let k = w.rng.rand_range(0, 4);
    let spin = if w.rng.randi(2) == 0 { k } else { -k };
    let rgba = (alpha as u32) << 24 | 0x78_7878;
    *w.svc.fx.part_spawns.entry(23).or_default() += 1;
    let Some(sys) = w.particles.as_deref_mut() else { return };
    let Some(i) = crate::particles::type23::spawn(sys, w.rng, 0.75, 1.0, f32::from_bits(0x3f83_d70a), 120000.0, p, spin, [0.0; 4], rgba) else {
        w.svc.fx.part_failed += 1;
        return;
    };
    let rot = w.rng.rand_range(0, 0xff) as u8;
    let life = w.ticks(0x3c);
    let r = &mut w.particles.as_deref_mut().unwrap().pool.recs[i];
    use crate::particles::rec;
    r[3] = 0x44;
    r[8] = rot;
    rec::set_i16(r, 0xa, life as i16);
    rec::set_u32(r, 0x24, 2);
    r[0x2a] = alpha;
    r[0x2b] = r[0xa];
}
