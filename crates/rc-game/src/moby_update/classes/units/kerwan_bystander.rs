//! Kerwan's talking bystander, class 914 (level03 `0x2dbd90`, 1 placed: #942 at (222.9, 142.9, 48), the only copy;
//! the name is descriptive [L]). A talking NPC while Ratchet has no Swingshot (item 12, 0x13d4cc): its talk sets
//! global flag 25 (0x13d3a1), and the end of the talk's node 2 records a checkpoint at its cuboid (pvar +0x4c). An
//! explosion's hit (attack type 3, with an attacker) blows it up once: **skill point 5** (0x13d40d), a beam explosion,
//! sequence 1, its save bits set; from then on it smokes, and a later visit's load finds it blown up. Read from the
//! level03 decomp and disassembly (the smoke's offset and the explosion's arguments). Native `f32`; the draws at the
//! game's points.
//!
//! **Pvars** (0x50): +0x00 the talk block (`interact::talk`), +0x4c the checkpoint cuboid (−1 none).
//!
//! ## Coverage (`0x2dbd90`)
//! | address | what | port |
//! |---|---|---|
//! | | drawn (+0x31) and within 26 (3-D) of the camera (0x166ec0 = level01's 0x167240): `0x248ba8` (= `0x26f020`), +0x7f = 0x15 | [`update`] (`shadows::probe_down`) |
//! | | the collected byte `0x1bb784[uid]` (level01's 0x1bbb04) or the death bit `0x14c190 + level·0x100` → state 3 and `fun_00212ed8(m, 1, 0)` unless in 3 | [`update`] (`creature::hard_cut`) |
//! | | `MobyGetHitMessage(m, −1, 0)` (`0x248ea8` = `0x26f320`): an attacker (+0x20) and type +0x28 = 3, its bits clear → skill point 5 (`allocate_voice_for_bank_entry(1, 0, 0)`, `ShowBanner(0x53d6)`), `SpawnBeamExplosion(0, 0, 4, 2, 9, 1, 15, m, 0, pos + (0, 0, 1), 10, 3, 16, 0, 1, 1, −1, 0)`, state 3, sequence 1, the death bit and this visit's (0x1ba5d0 = level01's 0x1ba950) | [`update`] (`story::award_skill_point`, `fx::beam_explosion`) |
//! | | +0xa4 = 0xff; 0x13d4cc (the Swingshot) owned and not in state 3 → no states | [`update`] |
//! | (scene exit) | `NpcTalkRefresh(npc, talk, 1)` (`0x2ac608`, outside the class): the dialogue advances | [`update`] (`interact::poll_scene_end`) |
//! | state 0 | +0x38 = Ratchet's moby's light words; → 1; `NpcTalkRegister(m, pvars)` (`0x254a58` = `0x27b480`); flag 25 set → talk +0x04 = 1, +0x36 = 2 | [`update`] (`interact::talk_register_at`) |
//! | state 1 | `NpcTalkUpdate` (`0x254600` = `0x27b028`) → flag 25 = 1, `FUN_00251988(2.7, m)` (= `0x2783a8`), 2 | [`update`] (`interact::talk_update_at`, `interact::place_after_scene`) |
//! | state 2 | game mode ≠ 2 → 1; talk +0x04 = 2 and +0x4c ≠ −1 → the checkpoint record `0x273ac0` (= `0x29ac10`) at the cuboid (+0x30, +0x70) | [`update`] (`checkpoint::record`) |
//! | state 3 | velocity (cos a·randf(0, 0.25)·dt, sin b·randf(0, 0.25)·dt, 0), offset (cos c·randf(0, 0.5)·randf(0, 1), sin d·randf(0, 0.5)·randf(0, 1), 0) + position (`0x1f8c28` = `0x2211a0`); `randi(3)` = 0 → `PartType22Spawn(0x4819cf00, p, v, 0x20202020, 0x7f7f7f, ticks(rand_range(60, 120)))` | [`smoke`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::checkpoint;
use crate::moby_update::creature::{self as c, fx, projectile, DT};
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;
use crate::particles::type22;

pub const UPDATE_FN: u32 = 0x2d_bd90;
pub const REFERENCE_LEVEL: u32 = 3;
pub const CLASSES: [i16; 1] = [914];
/// The pvar of the checkpoint cuboid, and the pvar bytes needed.
const CUBOID: usize = 0x4c;
const SIZE: usize = 0x50;
/// Global flag 25 (0x13d3a1): talked to.
const FLAG_TALKED: usize = 25;
/// Skill point 5 (0x13d40d).
const SKILL: usize = 5;
/// The Swingshot (0x13d4cc): owned, no talk.
const SWINGSHOT: usize = 12;
/// The explosion (module table).
const BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 16, debris: 1, sound: 0, shake: true };

/// Its save bits: the collected byte or the persistent death bit of its uid.
fn done(w: &World, id: MobyId) -> bool {
    let uid = w.m(id).spawn_id;
    w.svc.save.collected.get(&uid).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(w.svc.level, uid))
}

/// State 3 and sequence 1.
fn blown(w: &mut World, id: MobyId) {
    w.mm(id).state = 3;
    c::hard_cut(w, id, 1, 0);
}

/// Level03 `0x2dbd90` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { w.mm(id).pvars.resize(SIZE, 0); }
    // The end of its scene (`NpcTalkRefresh(npc, talk, 1)` in the game's scene exit): the dialogue moves on to the
    // next node. Without it the auto node replayed the same scene as soon as Ratchet was back in range.
    interact::poll_scene_end(w, id);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 26.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x15;
        }
    }
    if done(w, id) && w.m(id).state != 3 { blown(w, id); }
    let hit = w.get_hit(id, u32::MAX, false);
    if hit.is_some_and(|h| h.attacker.is_some() && h.b28 == 3) && !done(w, id) {
        story::award_skill_point(w, SKILL);
        let mut at = c::pos(w, id);
        at[2] += 1.0;
        fx::beam_explosion(w, &BLAST, Some(id), at);
        blown(w, id);
        let (level, uid) = (w.svc.level, w.m(id).spawn_id);
        w.svc.save.death.insert((level, uid));
        w.svc.save.death_level.insert(uid);
    }
    w.mm(id).hit_slot = 0xff;
    let swingshot = w.hero.owned.0.get(SWINGSHOT).is_some_and(|&b| b != 0);
    if swingshot && w.m(id).state != 3 { return; }
    match w.m(id).state {
        0 => {
            let (light, ambient) = w.hero_moby.and_then(|h| w.table.mobys.get(h)).map_or((0, [0; 4]), |h| (h.light, h.ambient));
            let m = w.mm(id);
            (m.light, m.ambient) = (light, ambient);
            m.state = 1;
            interact::talk_register_at(w, id, 0);
            if story::flag(w, FLAG_TALKED) != 0 {
                let pv = &mut w.mm(id).pvars;
                p::set_i16(pv, talk::LAST, 1);
                p::set_i16(pv, talk::NODE, 2);
            }
        }
        1 => {
            if interact::talk_update_at(w, id, 0) {
                interact::set_global_flag(w, FLAG_TALKED, 1);
                interact::place_after_scene(w, id, f32::from_bits(0x402c_cccd));
                w.mm(id).state = 2;
            }
        }
        2 => {
            if w.svc.game_mode == 2 { return; }
            w.mm(id).state = 1;
            let pv = &w.m(id).pvars;
            let (last, cuboid) = (p::i16(pv, talk::LAST), p::i32(pv, CUBOID));
            if last != 2 || cuboid == -1 { return; }
            let Some(s) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, cuboid) else { return };
            let r = checkpoint::Record { pos: s.centre(), rot: s.euler };
            checkpoint::record(w, r);
        }
        3 => smoke(w, id),
        _ => {}
    }
}

/// `fast_cos` / `fast_sin` (0x1f9180 / 0x1f9198).
fn fcos(a: f32) -> f32 { crate::hero::physics::fast_cos(crate::ps2v::Pf::f(a)).to_f32() }
fn fsin(a: f32) -> f32 { crate::hero::physics::fast_sin(crate::ps2v::Pf::f(a)).to_f32() }

/// State 3's smoke (module table), its draws in the game's order.
fn smoke(w: &mut World, id: MobyId) {
    let a = w.rng.rand_angle();
    let vx = fcos(a) * (w.rng.randf(0.0, 0.25) * DT);
    let b = w.rng.rand_angle();
    let vy = fsin(b) * (w.rng.randf(0.0, 0.25) * DT);
    let cc = w.rng.rand_angle();
    let (r1, r2) = (w.rng.randf(0.0, 0.5), w.rng.randf(0.0, 1.0));
    let ox = fcos(cc) * r1 * r2;
    let d = w.rng.rand_angle();
    let (r1, r2) = (w.rng.randf(0.0, 0.5), w.rng.randf(0.0, 1.0));
    let oy = fsin(d) * r1 * r2;
    let p = c::pos(w, id);
    let pos = [ox + p[0], oy + p[1], p[2], p[3]];
    if w.rng.randi(3) != 0 { return; }
    let r = w.rng.rand_range(60, 120);
    let life = w.ticks(r);
    projectile::part22(w, &type22::Spawn { size: f32::from_bits(0x4819_cf00), pos, vel: [vx, vy, 0.0, 0.0], c1: 0x2020_2020, c2: 0x007f_7f7f, life });
}
