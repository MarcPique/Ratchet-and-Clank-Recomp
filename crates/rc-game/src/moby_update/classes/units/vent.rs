//! Vents, class 1512: level06 0x308c68 (census U229; 26 created instances on Kalebo III). While in view (64 units,
//! a 3-unit sphere) a vent blows trail blobs (particle type 2) along its forward row: steam puffs (+0x00 ≠ 0: state
//! 4, one puff most ticks and a burst of five every 1–2 s) or sparks (state 3: a burst of 1, or 5–9, every
//! 20–40 ticks, falling). A vent with +0x04 ≠ 0 is off (state 1); state 2 is a delayed small explosion then the
//! vent's emission (entered from outside the class). Read from the level06 disassembly (0x308c68); the constants
//! are its `$gp` words (0x1622f0..0x16237c). Native `f32`; the `rand` draws in the game's order on the shared stream.
//!
//! **Pvar block** (s32): +0x00 steam (≠ 0) or sparks, +0x04 off, +0x20 the spark timer, +0x24 the burst timer,
//! +0x28 the explosion timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x308c68 | `FastBSphereCheck(64, pos, 3)` (0x2221f0) = −1 → nothing | [`update`] (`fx::in_view`) |
//! | state 0 | rows from the Euler angles (0x221960); +0x04 → 1 (off); else → 4 (steam) / 3 (sparks) | [`update`] |
//! | state 2 | `FastDecTimer(+0x28)` run out → the death explosion `0x273f50(0.333, 13, m, pos, −1)`, → 4 / 3 | [`update`] (`fx::death_explosion`) |
//! | state 3 | `FastDecTimer(+0x20)` run out → +0x20 = `trunc(randf(20, 40))`; n = `randi(7) ≠ 0` ? 1 : `randi(5)` + 5; per spark: `r = rand_vec(0, 3·dt)`; v1 = rows[0] at `6·(randf(−.6, .6) + 1)·dt` + r; v2 = v1 at `2·(randf(−.6, .6) + 1)·dt`, v2.z −= 5·dt; sizes `randf(.3, .2)`, `randf(.2, .1)`; colours `tween(randf(0, 1), 0x804080ff, 0x8040ffff)`, `tween(randf(0, 1), 0x80204040, 0x80204080)`; phases `trunc(0·randf(0, 1) + 1)`, `trunc(20·(randf(−.2, .2) + 1))` ×2; def 0x10019 (25, additive): `PartType02Spawn` (0x27dc98) | [`sparks`] (`fx::part02`) |
//! | state 4 | n = 1; `FastDecTimer(+0x24)` run out → n = 5, +0x24 = `trunc(randf(60, 120))`; `randi(4) ≠ 0` and n = 1 → none; per puff: v1 = rows[0] at `3·(randf(−.1, .1) + 1)·dt`; v2 = (0, 0, `(randf(−.1, .1) + 1)·dt`) + rows[1] at `randf(−.25, .25)·dt`; sizes `randf(.05, .1)`, `randf(.333, .5)`; colours `tween(randf(0, 1), 0x80ffc0c0, 0x80ffffc0)`, `tween(randf(0, 1), 0x20c08080, 0x20c0c080)`; phases `trunc(20·randf(0, 1) + 1)`, `trunc(30·(randf(−.1, .1) + 1))`, `trunc(40·(randf(−.1, .1) + 1))`; def −1: `PartType02Spawn` | [`puffs`] (`fx::part02`) |
//! | | no sound (the explosion's is −1), light beyond the explosion's, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{death_explosion, in_view, part02};
use crate::moby_update::creature::{add, dec_timer_pvar_i32, pi32, set_len3, set_pi32, DT};
use crate::moby_update::services::World;
use crate::particles::type02::Spawn;

/// The update in the level06 class table.
pub const UPDATE_FN: u32 = 0x30_8c68;
pub const REFERENCE_LEVEL: u32 = 6;
pub const CLASSES: [i16; 1] = [1512];

type V = [f32; 4];

/// `truncate_float_to_s32(multiply_global_scale(x))`.
fn scaled(w: &World, x: f32) -> i32 { (x * w.svc.timing.timer_scale.to_f32()) as i32 }

fn tween(w: &mut World, a: u32, b: u32) -> u32 { let t = w.rng.randf(0.0, 1.0); crate::hud::tween_color(t, a, b) }

fn row(w: &World, id: MobyId, k: usize) -> V { let r = w.m(id).rows[k]; [r[0], r[1], r[2], 0.0] }

/// Level06 0x308c68 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x2c { return; }
    if !in_view(w, 64.0, w.m(id).position, 3.0) { return; }
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            let r = crate::follow_camera::script::euler_rows([m.rotation[0], m.rotation[1], m.rotation[2]]);
            for (row, e) in m.rows.iter_mut().zip(r) { *row = [e[0], e[1], e[2], 0.0]; }
            if pi32(w, id, 4) != 0 { w.mm(id).state = 1; return; }
            w.mm(id).state = if pi32(w, id, 0) == 0 { 3 } else { 4 };
        }
        2 => {
            if dec_timer_pvar_i32(w, id, 0x28) == 0 { return; }
            let p = w.m(id).position;
            death_explosion(w, f32::from_bits(0x3eaa_7efa), 13.0, Some(id), p, -1);
            w.mm(id).state = if pi32(w, id, 0) == 0 { 3 } else { 4 };
        }
        3 => sparks(w, id),
        4 => puffs(w, id),
        _ => {}
    }
}

/// State 3 (module doc).
fn sparks(w: &mut World, id: MobyId) {
    if dec_timer_pvar_i32(w, id, 0x20) == 0 { return; }
    let t = w.rng.randf(20.0, 40.0);
    let t = scaled(w, t);
    set_pi32(w, id, 0x20, t);
    let n = if w.rng.randi(7) != 0 { 1 } else { w.rng.randi(5) + 5 };
    for _ in 0..n.max(0) {
        let r = w.rng.rand_vec(0.0, 3.0 * DT);
        let k = w.rng.randf(-0.6, 0.6) + 1.0;
        let mut v1 = add(set_len3(row(w, id, 0), 6.0 * k * DT), [r[0], r[1], r[2], 0.0]);
        let k2 = w.rng.randf(-0.6, 0.6) + 1.0;
        let mut v2 = set_len3(v1, 2.0 * k2 * DT);
        v2[2] -= 5.0 * DT;
        v1[3] = w.rng.randf(0.3, 0.2);
        v2[3] = w.rng.randf(0.2, 0.1);
        let c1 = tween(w, 0x8040_80ff, 0x8040_ffff);
        let c2 = tween(w, 0x8020_4040, 0x8020_4080);
        let f = w.rng.randf(0.0, 1.0);
        let t0 = scaled(w, 0.0 * f + 1.0);
        let f = w.rng.randf(-0.2, 0.2);
        let t1 = scaled(w, 20.0 * (f + 1.0));
        let f = w.rng.randf(-0.2, 0.2);
        let t2 = scaled(w, 20.0 * (f + 1.0));
        let pos = w.m(id).position;
        part02(w, &Spawn { pos, v1, v2, c1, c2, t: [t0, t1, t2], def: 0x1_0019 });
    }
}

/// State 4 (module doc).
fn puffs(w: &mut World, id: MobyId) {
    let mut n = 1;
    if dec_timer_pvar_i32(w, id, 0x24) != 0 {
        n = 5;
        let t = w.rng.randf(60.0, 120.0);
        let t = scaled(w, t);
        set_pi32(w, id, 0x24, t);
    }
    if w.rng.randi(4) != 0 && n < 2 { return; }
    for _ in 0..n {
        let k = w.rng.randf(-0.1, 0.1) + 1.0;
        let mut v1 = set_len3(row(w, id, 0), 3.0 * k * DT);
        let k2 = w.rng.randf(-0.1, 0.1) + 1.0;
        let up = 1.0 * k2 * DT;
        let s = w.rng.randf(-0.25, 0.25);
        let mut v2 = add([0.0, 0.0, up, 0.0], set_len3(row(w, id, 1), s * DT));
        v1[3] = w.rng.randf(0.05, 0.1);
        v2[3] = w.rng.randf(f32::from_bits(0x3eaa_7efa), 0.5);
        let c1 = tween(w, 0x80ff_c0c0, 0x80ff_ffc0);
        let c2 = tween(w, 0x20c0_8080, 0x20c0_c080);
        let f = w.rng.randf(0.0, 1.0);
        let t0 = scaled(w, 20.0 * f + 1.0);
        let f = w.rng.randf(-0.1, 0.1);
        let t1 = scaled(w, 30.0 * (f + 1.0));
        let f = w.rng.randf(-0.1, 0.1);
        let t2 = scaled(w, 40.0 * (f + 1.0));
        let pos = w.m(id).position;
        part02(w, &Spawn { pos, v1, v2, c1, c2, t: [t0, t1, t2], def: -1 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;

    #[test]
    fn steam_and_sparks_spawn_trail_blobs() {
        let vent = |steam: i32, off: i32| {
            let mut m = Moby { o_class: 1512, pvars: vec![0; 0x30], ..Moby::default() };
            p::set_i32(&mut m.pvars, 0, steam);
            p::set_i32(&mut m.pvars, 4, off);
            m
        };
        let mut t = MobyTable::new(vec![vent(1, 0), vent(0, 0), vent(0, 1)], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        for id in 0..3 { update(&mut w, id); }
        assert_eq!([w.m(0).state, w.m(1).state, w.m(2).state], [4, 3, 1]);
        assert_eq!(w.m(0).rows[0][..3], [1.0, 0.0, 0.0]);
        // The first tick of each: the timers are 0 (untouched): steam bursts five puffs, sparks spawn a burst.
        update(&mut w, 0);
        assert_eq!(w.svc.fx.part_spawns.get(&2).copied().unwrap_or(0), 5);
        assert!((60..=120).contains(&pi32(&w, 0, 0x24)));
        update(&mut w, 1);
        let n = w.svc.fx.part_spawns[&2] - 5;
        assert!(n == 1 || (5..10).contains(&n), "{n}");
        assert!((20..=40).contains(&pi32(&w, 1, 0x20)));
        // A spark burst waits for its timer.
        let before = w.svc.fx.part_spawns[&2];
        update(&mut w, 1);
        assert_eq!(w.svc.fx.part_spawns[&2], before);
    }
}
