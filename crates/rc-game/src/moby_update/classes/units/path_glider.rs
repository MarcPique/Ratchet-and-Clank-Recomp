//! Path gliders, class 1564: level00 0x2e3a88 (census U36; the same code on level07 0x31ede0, level10 0x2eada8 and
//! level18 0x2fa420: 92 created instances). A flying creature that rides a path at 6 units/s (measured on the
//! path's first segment), at a fixed height above it, halving the path's pitch; when its animation wraps it
//! flaps (sequence 1) with a 1-in-3 chance unless it is climbing, else glides (sequence 0). Read from the level00
//! decomp of 0x2e3a88 and the four levels' data words; native `f32`.
//!
//! **Pvar block**: +0x60 s32 the path (`0x1b04b0[i]`; here the index into `Services::splines`), +0x64 f32 the
//! height above the path, +0x68 f32 the start fraction of the path, +0x6c f32 t (segments along the path),
//! +0x70 s32 the closed flag, +0x74 f32 the step per tick.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | path −1 → `DeleteMoby`; step = 6 (gp−0x4f68, the same word on 07 gp−0x4f30, 10 gp−0x4bac, 18 gp−0x4690) · dt / \|p₀ − p₁\|; closed = \| \|p₀ − p₁\| − \|p₀ − p_last\| \| < 0.1 (the game's test as written); t = start fraction · count; state 1; update distance 0xff; scale ·= 2 (gp−0x4f64 …) | [`update`] |
//! | states 0, 1 | `0x262e40(t, path, closed, &pos, &rot, 0)` ([`crate::path::pose`]); rot.y ·= 0.5; z += height; t += step, wrapped at count | [`update`] |
//! | | anim flags (+0x70) bit 1 (the sequence wrapped): not climbing → `randi(3)` = 0 → `MobyAnimBlend(m, 1, 0, 10)` unless on 1 (and no glide check); else on ≠ 0 → `MobyAnimBlend(m, 0, 0, 10)` | [`update`] |
//! | | no sound, particle, light, hit or save flag | — |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{dist3, pf, pi32, set_pf, set_pi32, DT};
use crate::moby_update::services::World;
use crate::path::pose;

pub const UPDATE_FN: u32 = 0x2e_3a88;
pub const REFERENCE_LEVEL: u32 = 0;
pub const CLASSES: [i16; 1] = [1564];

/// The speed along the first segment, units per second (level00 gp−0x4f68 = 6.0; 07, 10 and 18 hold the same).
pub const SPEED: f32 = 6.0;
/// The scale factor of the init (level00 gp−0x4f64 = 2.0; the same on 07, 10, 18).
pub const SCALE: f32 = 2.0;

pub mod pv {
    pub const PATH: usize = 0x60;
    pub const HEIGHT: usize = 0x64;
    pub const START: usize = 0x68;
    pub const T: usize = 0x6c;
    pub const CLOSED: usize = 0x70;
    pub const STEP: usize = 0x74;
}

/// Level00 0x2e3a88 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x78 { return; }
    let path = pi32(w, id, pv::PATH);
    let pts = match usize::try_from(path).ok().and_then(|i| w.svc.splines.get(i)) {
        Some(p) if !p.is_empty() => p.clone(),
        _ => {
            // The game reads the table entry as it is (−1 is tested only on the init); a missing path deletes [L].
            if w.m(id).state <= 1 { w.delete_moby(id); }
            return;
        }
    };
    let n = pts.len();
    match w.m(id).state {
        0 => {
            let p = |i: usize| pts[i].map(f32::from_bits);
            let l01 = dist3(p(0), p(1.min(n - 1)));
            let l0n = dist3(p(0), p(n - 1));
            set_pf(w, id, pv::STEP, (SPEED * DT) / l01);
            set_pi32(w, id, pv::CLOSED, ((l01 - l0n).abs() < 0.1) as i32);
            let start = pf(w, id, pv::START);
            set_pf(w, id, pv::T, start * n as f32);
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            m.scale *= SCALE;
        }
        1 => {}
        _ => return,
    }
    let z_old = w.m(id).position[2];
    let t = pf(w, id, pv::T);
    let (pos, rot) = pose(&pts, pi32(w, id, pv::CLOSED) != 0, t, true);
    let height = pf(w, id, pv::HEIGHT);
    {
        let m = w.mm(id);
        m.position = pos;
        m.rotation = rot;
        m.rotation[1] *= 0.5;
        m.position[2] += height;
    }
    let mut t = t + pf(w, id, pv::STEP);
    if (n as f32) < t { t -= n as f32; }
    set_pf(w, id, pv::T, t);
    let m = w.m(id);
    if m.anim.flags & 2 == 0 { return; }
    let (z, seq) = (m.position[2], m.anim.seq_b);
    if z <= z_old && w.rng.randi(3) == 0 {
        if seq == 1 { return; }
        w.anim_blend(id, 1, 0, 10);
        return;
    }
    if seq != 0 { w.anim_blend(id, 0, 0, 10); }
}
