//! The Fleet's small classes (level 17): **the floating mines 835** (`0x2dc3f8`, census U606; read from the
//! disassembly, no decomp) and **the scene thrusters 1562** (`0x2f2e78`, U616). Native `f32`.
//!
//! * **835**: bob on the water (0.25 up and down, half a turn a second, from a random phase; `0x277a00`). A hit
//!   (0x230000) hard enough (`0x26f378(…, 4)` ≥ 2) by anything but another mine: the blast (`SpawnBeamExplosion`: flashes
//!   2 / 1 beyond 4, scale 1, light 7, 3 streaks, 3 spark pairs, 5 puffs, class sound 1, no shake, a debris piece, no
//!   damage sphere; at its position, the position argument 0), gone. Touched (a sphere of 1 at its position + 0.75
//!   up, mask 0x10, the hit template: damage 1, flags 0x10001) by anything but another mine (the hit moby 0x174758),
//!   or Ratchet's capsule on it (0x13f58c / 0x13f590): the same blast, then a splash of 150 drops (type 34, size
//!   `randf(0.05, 0.1)`·209920, speed `randf(4·dt, 8·dt)` along (`randf(−1, 1)`, `randf(−1, 1)`, `randf(−0.5, 2)`),
//!   the water's height a unit above it), gone.
//! * **1562**: updated always; in a scene (game mode 2) 0 or 2, the thrusters (`0x25d658` = L01 `0x278450`) on actor 2
//!   (0x16d2e0; Quartu's 1560 is the level-15 copy, `quartu_small`).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2dc3f8` | 835 | [`mine_update`] |
//! | `0x2f2e78` | 1562 | [`thrusters_update`] |
//!
//! [L] Ratchet's capsule touch skips the sphere query, so the game tests a stale hit moby 0x174758 (the last query's);
//! the port takes the touch as a blast. The words are `Hero::cap_moby` (0x13f58c) and `Hero::wall_moby` (0x13f590).

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::bomb_water::{part34, water_level};
use crate::moby_update::creature::{self as c, damage, fx, DT};
use crate::moby_update::services::{pv, HitTemplate, World};
use crate::moby_update::story;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 17;
pub const MINE_FN: u32 = 0x2d_c3f8;
pub const MINE_CLASSES: [i16; 1] = [835];
pub const THRUSTERS_FN: u32 = 0x2f_2e78;
pub const THRUSTERS_CLASSES: [i16; 1] = [1562];

const MINE: i16 = 0x343;
const PHASE: usize = 0x60;
const BOB: usize = 0x64;
/// gp−0x4ed4 / −0x4ed0: the bob's height and rate (degrees a second).
const BOB_AMP: f32 = 0.25;
const BOB_RATE: f32 = 180.0;
const BOOM: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 4.0, scale: 1.0, light: 7.0, streaks: 3, sparks: 3, puffs: 5, debris: 1, sound: 1, shake: false };

fn not_mine(w: &World, m: Option<MobyId>) -> bool { !m.is_some_and(|a| w.m(a).o_class == MINE) }

/// Level17 `0x2dc3f8` (module doc).
pub fn mine_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x68);
    let hit = w.get_hit(id, 0x23_0000, false);
    let res = damage::resolve(w, id, hit, 0x20, 0, 4);
    if res.out5 >= 2 && hit.is_some_and(|h| not_mine(w, h.attacker)) {
        let p = c::pos(w, id);
        fx::beam_explosion(w, &BOOM, Some(id), p);
        w.delete_moby(id);
        return;
    }
    match w.m(id).state {
        0 => {
            let a = w.rng.rand_angle();
            c::set_pf(w, id, PHASE, a);
            c::set_pf(w, id, BOB, 0.0);
            w.mm(id).state = 1;
        }
        1 => {
            super::bob(w, id, BOB_AMP, BOB_RATE * f32::from_bits(0x3c8e_fa35) * DT, PHASE, BOB);
            let tmpl = HitTemplate { attacker: Some(id), flags: 0x1_0001, damage: Pf::ONE, ..Default::default() };
            let mut centre = c::pos(w, id);
            centre[2] += 0.75;
            if w.hero.cap_moby != Some(id) && w.hero.wall_moby != Some(id) {
                let list = w.sphere_mobys_list(Pf::ONE, pv(centre), 0x10, Some(id), Some(&tmpl));
                let Some(&first) = list.first() else { return };
                if !not_mine(w, Some(first)) { return; }
            }
            let p = c::pos(w, id);
            fx::beam_explosion(w, &BOOM, Some(id), p);
            let level = water_level(w, [p[0], p[1], p[2] + 1.0, p[3]]);
            for _ in 0..150 {
                let x = w.rng.randf(-1.0, 1.0);
                let y = w.rng.randf(-1.0, 1.0);
                let z = w.rng.randf(-0.5, 2.0);
                let l = w.rng.randf(DT * 4.0, DT * 8.0);
                let v = c::set_len3([x, y, z, 0.0], l);
                let size = w.rng.randf(0.05, 0.1) * 209_920.0;
                part34(w, size, level, p, [v[0], v[1], v[2]]);
            }
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// Level17 `0x2f2e78` (module doc).
pub fn thrusters_update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            if w.svc.game_mode != 2 { return; }
            let Some(scene) = w.svc.cinematic.scene.clone() else { return };
            if scene.id != 0 && scene.id != 2 { return; }
            if let Some(a) = scene.actors.get(2) { crate::moby_update::classes::cutscene_fx::infobot_thrusters(w, a); }
        }
        _ => {}
    }
}
