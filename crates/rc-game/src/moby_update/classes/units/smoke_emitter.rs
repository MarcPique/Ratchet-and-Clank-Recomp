//! Smoke emitters, class 648: level08 0x2f5830, the same code on 10 (census U281; 21 created instances). Every period
//! the emitter puts out one trail blob (particle type 2) a little off its position, moving along its forward row
//! with some sideways spread and rising, from its own pvar settings — or, with +0x38 ≠ 0, from the level's default
//! plume (`$gp` 0x161f74..0x161fa8: grey 0x20706060 → 0x20f0d0d0, sizes 45 / 12, every 60 ticks). Read from the
//! level08 disassembly (0x2f5830). Native `f32`; the `rand` draws in the game's order.
//!
//! **Pvar block** (s32 / f32): +0x00 / +0x04 colours, +0x08 / +0x0c / +0x10 phase lengths, +0x14 the period, +0x18
//! forward speed, +0x1c rise, +0x20 second forward speed, +0x24 second rise, +0x28 / +0x2c sizes, +0x30 / +0x34 the
//! sideways spreads, +0x38 use the defaults, +0x3c the timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2f5830 | `FastDecTimer(+0x3c)` (0x220e78) ≠ 0 → one blob, timer = the period | [`update`] |
//! | | rows = `EulerToMatrix(rot)` (0x221980); p = position + `randf(±0.05)` on x, y, z (the defaults: x, y, **x** again: the game's slip, kept) | [`emit`] |
//! | | one unused `randf(±10°)` | [`emit`] |
//! | | v1 = rows[0] at `randf(f, 1.2f)·dt` + rows[1] at `randf(±s1)·dt` (0x221410, 0x221188), v1.z += `randf(r, 1.2r)·dt`; v2 = rows[0] at `randf(f2, 1.5f2)·dt` + rows[1] at `randf(±s2)·dt`, v2.z += `randf(r2, 1.5r2)·dt`; sizes into the w lanes | [`emit`] |
//! | | phases `trunc(randf(t, 1.2t))` ×3; `PartType02Spawn(p, v1, v2, c1, c2, t0, t1, t2, −1)` (0x27dc98) | [`emit`] (`fx::part02`) |
//! | | no sound, light, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::part02;
use crate::moby_update::creature::{add, dec_timer_pvar_i32, pf, pi32, set_len3, set_pi32, DT};
use crate::moby_update::services::World;
use crate::particles::type02::Spawn;

/// The update in the level08 class table.
pub const UPDATE_FN: u32 = 0x2f_5830;
pub const REFERENCE_LEVEL: u32 = 8;
pub const CLASSES: [i16; 1] = [648];

/// One plume's settings (the pvar block or the level's defaults).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plume {
    pub c1: u32,
    pub c2: u32,
    pub t: [i32; 3],
    pub period: i32,
    pub fwd: f32,
    pub rise: f32,
    pub fwd2: f32,
    pub rise2: f32,
    pub size: [f32; 2],
    pub side: [f32; 2],
    /// The defaults jitter x twice and z never.
    pub z_slip: bool,
}

/// The level's default plume (level08 `$gp` 0x161f74..0x161fa8; the same words on 10).
pub const DEFAULT: Plume = Plume { c1: 0x2070_6060, c2: 0x20f0_d0d0, t: [60, 120, 120], period: 60, fwd: 0.0, rise: 0.4, fwd2: 0.3, rise2: 0.4, size: [45.0, 12.0], side: [0.0, 0.3], z_slip: true };

fn plume(w: &World, id: MobyId) -> Plume {
    if pi32(w, id, 0x38) != 0 { return DEFAULT; }
    Plume {
        c1: pi32(w, id, 0) as u32,
        c2: pi32(w, id, 4) as u32,
        t: [pi32(w, id, 8), pi32(w, id, 0xc), pi32(w, id, 0x10)],
        period: pi32(w, id, 0x14),
        fwd: pf(w, id, 0x18),
        rise: pf(w, id, 0x1c),
        fwd2: pf(w, id, 0x20),
        rise2: pf(w, id, 0x24),
        size: [pf(w, id, 0x28), pf(w, id, 0x2c)],
        side: [pf(w, id, 0x30), pf(w, id, 0x34)],
        z_slip: false,
    }
}

/// Level08 0x2f5830 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { return; }
    if dec_timer_pvar_i32(w, id, 0x3c) == 0 { return; }
    let pl = plume(w, id);
    set_pi32(w, id, 0x3c, pl.period);
    emit(w, id, &pl);
}

/// One blob (module doc).
pub fn emit(w: &mut World, id: MobyId, pl: &Plume) {
    let m = w.m(id);
    let r = crate::follow_camera::script::euler_rows([m.rotation[0], m.rotation[1], m.rotation[2]]);
    let rows = [0, 1].map(|k| [r[k][0], r[k][1], r[k][2], 0.0]);
    let mut p = m.position;
    p[0] += w.rng.randf(-0.05, 0.05);
    p[1] += w.rng.randf(-0.05, 0.05);
    let j = w.rng.randf(-0.05, 0.05);
    if pl.z_slip { p[0] += j } else { p[2] += j }
    w.rng.randf(f32::from_bits(0xbe32_b8c2), f32::from_bits(0x3e32_b8c2));
    let k12 = f32::from_bits(0x3f99_999a);
    let s = w.rng.randf(pl.fwd, pl.fwd * k12);
    let mut v1 = set_len3(rows[0], s * DT);
    let s = w.rng.randf(-pl.side[0], pl.side[0]);
    v1 = add(v1, set_len3(rows[1], s * DT));
    v1[2] += w.rng.randf(pl.rise, pl.rise * k12) * DT;
    let s = w.rng.randf(pl.fwd2, pl.fwd2 * 1.5);
    let mut v2 = set_len3(rows[0], s * DT);
    let s = w.rng.randf(-pl.side[1], pl.side[1]);
    v2 = add(v2, set_len3(rows[1], s * DT));
    v2[2] += w.rng.randf(pl.rise2, pl.rise2 * 1.5) * DT;
    v1[3] = pl.size[0];
    v2[3] = pl.size[1];
    let scale = w.svc.timing.timer_scale.to_f32();
    let t = pl.t.map(|t| (w.rng.randf(t as f32, t as f32 * k12) * scale) as i32);
    part02(w, &Spawn { pos: p, v1, v2, c1: pl.c1, c2: pl.c2, t, def: -1 });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;

    #[test]
    fn one_blob_per_period() {
        let mut m = Moby { o_class: 648, pvars: vec![0; 0x40], ..Moby::default() };
        p::set_i32(&mut m.pvars, 0x38, 1);
        let mut t = MobyTable::new(vec![m], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        for _ in 0..121 { update(&mut w, 0); }
        // Tick 0 (timer untouched) and every 60 after.
        assert_eq!(w.svc.fx.part_spawns[&2], 3);
    }
}
