//! Rail mines, class 621: level08 0x2f44c0 (census U280; 25 created instances on Batalia's grind rails). An armed mine
//! (state 1) blows up when Ratchet grinds past it (within 2 units in xy while in the grind state 0x42) — a damaging
//! beam explosion — or when it takes a hit (flags 0x330000) — the same explosion without damage; either way it plays
//! sound 0 and is deleted. State 2 is a mine running along a path (a mine set loose by another moby: none of the
//! placed ones, whose +0x14 is −1): it turns toward the next point and moves 3 units/s, ends at point 0 (exploding
//! near Ratchet, within 24, else fading out: state 3), and explodes the same way on a grinding Ratchet. Read from the
//! level08 decomp and disassembly (0x2f44c0). Native `f32`.
//!
//! **Pvar block** (s32): +0x00 the current path point, +0x04 (s8) the step (1), +0x10 the path (the game's spline
//! pointer; here the index into `Services::splines` [L]: no placed mine has one), +0x14 −1: no path, +0x34 the turn
//! velocity.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | z += 0.4, → 1 | [`update`] |
//! | state 1 | `MobyGetHitMessage(m, 0x330000, 0)` (0x26f320); Ratchet within 2 (xy, 0x221398) in state 0x42, or his capsule-hit moby `0x13f58c` = m (never set in the port) → sound 0, `SpawnBeamExplosion(3, 1, 3, 5, 9, 1, 15, m, 0x15f580, pos, 10, 20, 60, −1, 1, 1, −1, 0)` (0x273310), delete; else a hit → sound 0, the same with damage 0 and flashes 1 / 3; else nothing | [`update`] (`fx::beam_explosion`, `World::get_hit`) |
//! | state 2 | z −= 0.4; no path (+0x14 = −1 or a count of 0) → delete; yaw += 180°/s (`fast_add_rotations`); Ratchet grinding within 2 → beam (0, 0, 3, 5, 9, …), delete; the next point `(i + n + step) mod n`: `SpringTurn2(atan2, 0.01, 0.3, 0.1)` (0x26d058) toward it; point ≠ 0: move 3·dt toward it (0x2211b8 / 0x221410 / 0x221188); point 0: Ratchet within 24 → beam, sound 0, delete, else → 3; within 0.1 (0x221360) of the point: it becomes current; z += 0.4 | [`path_mode`] (`turn::spring_turn2_pvar`) |
//! | state 3 | alpha `Approach(0, 128 / ticks(30))` (0x270728); 0 → delete | [`update`] |
//! | | no particle, light, save flag beyond the explosion's; the explosion's damage sphere hits other mobys | `fx::beam_explosion` |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{beam_explosion, Beam};
use crate::moby_update::creature::turn::{approach, spring_turn2_pvar};
use crate::moby_update::creature::{add, add_rot, atan, dist2, dist3, pi32, set_len3, set_pi32, sub, DT};
use crate::moby_update::services::World;

/// The update in the level08 class table.
pub const UPDATE_FN: u32 = 0x2f_44c0;
pub const REFERENCE_LEVEL: u32 = 8;
pub const CLASSES: [i16; 1] = [621];

/// Ratchet grinding (`0x1413d4`).
const GRIND: i32 = 0x42;
/// The hover height above the path.
const LIFT: f32 = 0.4;

/// The damaging blast (Ratchet grinding into it).
pub const BLAST: Beam = Beam { damage_r: 3.0, damage: 1.0, flash: 3.0, flash2: 5.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 20, puffs: 60, debris: 1, sound: -1, shake: true };
/// The blast of a hit, and of the path mode (no damage).
pub const HIT_BLAST: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: 1.0, flash2: 3.0, ..BLAST };
const PATH_BLAST: Beam = Beam { damage_r: 0.0, damage: 0.0, ..BLAST };

fn hero_near_grinding(w: &World, id: MobyId, r: f32) -> bool {
    let hp = w.hero.pos.map(|x| f32::from_bits(x.0));
    dist2(w.m(id).position, hp) < r && w.hero.state == GRIND
}

fn explode(w: &mut World, id: MobyId, b: &Beam) {
    let p = w.m(id).position;
    beam_explosion(w, b, Some(id), p);
    w.delete_moby(id);
}

/// Level08 0x2f44c0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x38 { return; }
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.position[2] += LIFT;
            m.state = 1;
        }
        1 => {
            let hit = w.get_hit(id, 0x33_0000, false).is_some();
            if hero_near_grinding(w, id, 2.0) {
                w.play_sound(0, 0, id);
                explode(w, id, &BLAST);
            } else if hit {
                w.play_sound(0, 0, id);
                explode(w, id, &HIT_BLAST);
            }
        }
        2 => path_mode(w, id),
        3 => {
            let mut a = w.m(id).alpha as f32;
            let n = w.ticks(30);
            approach(0.0, 128.0 / n as f32, &mut a);
            w.mm(id).alpha = a as i32 as u8;
            if w.m(id).alpha == 0 { w.delete_moby(id); }
        }
        _ => {}
    }
}

/// State 2 (module doc).
fn path_mode(w: &mut World, id: MobyId) {
    w.mm(id).position[2] -= LIFT;
    let path = w.svc.splines.get(pi32(w, id, 0x10).max(0) as usize).cloned().unwrap_or_default();
    if pi32(w, id, 0x14) == -1 || path.is_empty() { w.delete_moby(id); return; }
    let yaw = add_rot(w.m(id).rotation[1], 180.0 * f32::from_bits(0x3c8e_fa35) * DT);
    w.mm(id).rotation[1] = yaw;
    if hero_near_grinding(w, id, 2.0) { explode(w, id, &PATH_BLAST); return; }
    let n = path.len() as i32;
    let step = w.m(id).pvars[4] as i8 as i32;
    let i = (pi32(w, id, 0) + n + step).rem_euclid(n);
    let q = path[i as usize].map(f32::from_bits);
    let pos = w.m(id).position;
    spring_turn2_pvar(w, id, atan(q[0] - pos[0], q[1] - pos[1]), 0.01, 0.3, 0.1, 0x34);
    let point = [q[0], q[1], q[2], q[3]];
    if i != 0 {
        let v = set_len3(sub(point, pos), 3.0 * DT);
        w.mm(id).position = add(pos, v);
    } else {
        let hp = w.hero.pos.map(|x| f32::from_bits(x.0));
        if dist2(pos, hp) < 24.0 {
            beam_explosion(w, &PATH_BLAST, Some(id), pos);
            w.play_sound(0, 0, id);
            w.delete_moby(id);
            return;
        }
        w.mm(id).state = 3;
    }
    if dist3(w.m(id).position, point) < 0.1 { set_pi32(w, id, 0, i); }
    w.mm(id).position[2] += LIFT;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::{pvar as p, HitTemplate};
    use crate::ps2v::Pf;

    #[test]
    fn a_grinding_ratchet_or_a_hit_sets_it_off() {
        let mine = |x: f32| {
            let mut m = Moby { o_class: 621, position: [x, 0.0, 0.0, 1.0], pvars: vec![0; 0x80], ..Moby::default() };
            p::set_i32(&mut m.pvars, 0x14, -1);
            m
        };
        let mut t = MobyTable::new(vec![mine(0.0), mine(50.0), mine(100.0)], 4);
        let mut hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
            for id in 0..3 { update(&mut w, id); }
            assert_eq!((w.m(0).state, w.m(0).position[2]), (1, LIFT));
            update(&mut w, 0);
            assert_eq!(w.m(0).state, 1, "Ratchet away: armed");
            let tmpl = HitTemplate { dir: [Pf::ZERO; 4], attacker: None, flags: 0x10_0000, b18: 0, b19: 0, h1a: 0, damage: Pf::ONE, w20: 0 };
            w.deliver_hit(1, &tmpl);
            update(&mut w, 1);
            assert!(w.m(1).state >= 0x80, "a hit sets it off");
        }
        hero.state = GRIND;
        hero.pos = [Pf::f(100.5), Pf::ZERO, Pf::ZERO, Pf::ONE];
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 1);
        update(&mut w, 2);
        assert!(w.m(2).state >= 0x80, "grinding past it sets it off");
        assert_eq!(w.svc.sounds.len(), 2);
        assert!(w.m(0).state == 1);
    }
}
