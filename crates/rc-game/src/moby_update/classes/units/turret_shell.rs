//! **The ridden turrets' shell, class 458** (census: not listed, created by code only): its spawner level12 `0x2ef2e8`
//! and its update `0x2ef648` (census U405's shell). The same code on level08 (`0x2e7eb0` / `0x2e8210`, cluster pairs of
//! two; Batalia's turret 440 fires it): **one shared unit**, data-driven through [`LevelPorts`]' code identity (the
//! class table of each level names the update), with the level-08 constants checked equal (level08 gp−0x4f28..−0x4ee0 =
//! level12 gp−0x51a8..−0x5160, the same 19 words). A new consumer (440) calls [`spawn`] with its own owner, muzzle
//! point and velocity. Read from the level12 decomp and disassembly (the explosion's stack arguments, the
//! type-02 spawns' `def` = −1). Native `f32`.
//!
//! **Pvar block** (0x20 bytes): +0x00 the velocity (xyzw), +0x10 the homing target (a moby, 0 none), +0x14 the owner,
//! +0x18 the life timer (s32), +0x1c the spin's sign (±1).
//!
//! **Constants** (level12 gp−0x51a8.., the same values at level08 gp−0x4f28..): 240 the life off level 12 (·scale),
//! 2 the trail blobs a tick, 10 the muzzle burst's count, 320 °/s the spin, 0x30e0ffff / 0x1080c0c0 the trail's
//! colours, 0x10b0c0c0 / 0x08708080 the burst's type-02 colours, 0x4f0090f0 / 0x4f0000f0 its type-21 colours, 1.0 the
//! trail's spread speed, 2.0 the burst's speed, 7 / 8 / 15 the type-02 phases, 2.0 / 3.0 the burst's sizes, 0.5 / 0.5
//! the trail's sizes.
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x2ef2e8 | `CreateMoby(458)` (none: nothing); +0x14 = owner, +0x10 = target | [`spawn`] |
//! | 0x2ef2e8 | ambient (0xa0, 0xa0, 0xa0) (`0x2650d0`), rot.x = 0, draw distance 0xff, update distance 0xff, +0x31 = 1 | [`spawn`] |
//! | 0x2ef2e8 | rot.y = −atan(\|vel.xy\|, vel.z), rot.z = atan(vel.x, vel.y); position = the muzzle; +0x00 = vel | [`spawn`] |
//! | 0x2ef2e8 | +0x1c = 1 on odd ticks (0x15f5cc & 1), else −1 | [`spawn`] |
//! | 0x2ef2e8 | burst ×10: a = `rand_angle`, s = `randf(2, 4)`·dt, v = row2·s·cos a + row1·s·sin a (`EulerToMatrix` of the new rotation); `PartType21Spawn(20000, pos, (v, 2), 0x4f0090f0, 0x4f0000f0, rand_range(ticks(10), ticks(20)), 1)`; `PartType02Spawn(pos, (v, 2), (v, 3), 0x10b0c0c0, 0x08708080, trunc(scale(randf(7, 8.4))), trunc(scale(randf(8, 9.6))), trunc(scale(randf(15, 22.5))), −1)` | [`spawn`] (`fx::part21`, `fx::part02`) |
//! | 0x2ef2e8 | life +0x18 = `ticks(120)` on level 12, else `trunc(scale(240))` | [`spawn`] |
//! | 0x2ef2e8 | `MobyBuildMatrix` | [`spawn`] |
//! | 0x2ef648 | old = position; position += velocity | [`update`] |
//! | 0x2ef648 | any of x, y, z outside 10..500 → `DeleteMoby` | [`update`] |
//! | 0x2ef648 | the trail ×2: step = −vel/2; p += step; a = `rand_angle`, s = `randf(0, 1)`·dt; v2 = row1·s·sin a; v1 = row2·s·cos a + v2; `PartType02Spawn(p, (v1, 0.5), (v2, 0.5), 0x30e0ffff, 0x1080c0c0, phases as the burst, −1)` | [`update`] |
//! | 0x2ef648 | rot.x += 320°·dt·(+0x1c) (`fast_add_rotations`) | [`update`] |
//! | 0x2ef648 | homing: target deleted (state 0xfe / 0xfd) or not class 0x1b3 → +0x10 = 0; class 0x1b3: lead = target + its pvar +0x20 · (dist / speed); d = normalise(lead − pos)·speed − vel, clamped to 3.7·dt (`0x2745f0`); vel += d, renormalised to the speed | [`update`] (no class 0x1b3 on levels 08 / 12's tables yet: the branch runs for none) |
//! | 0x2ef648 | the hit template: vel, attacker self, flags 0x50001, +0x18 = the stale stack word (the z of the trail step or of the homing aim), damage 2, +0x20 = 1 | [`update`] |
//! | 0x2ef648 | `FastDecTimer(+0x18)` out → `DeleteMoby` (no blast) | [`update`] |
//! | 0x2ef648 | else `CollLine_Fix(old, pos, 0, self, template)`: no hit → done; a hit → position = the hit point; x, y, z all > 15: `SpawnBeamExplosion(3, 3, 5, 3, 9, 1, min(24, x − 2, y − 2, z − 2), self, vel, pos, 0, 10, 20, −1, 1, 1, −1, 0)`; `DeleteMoby` | [`update`] (`fx::beam_explosion`) |
//! | | no class sound, light (besides the blast's), save flag, bolt | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{beam_explosion, part02, part21, Beam};
use crate::moby_update::creature::{self as c, add, add_rot, atan, dist3, len2, len3, scale, set_len3, sub, DT};
use crate::moby_update::services::{pf as to_pf, pv, HitTemplate, World};

pub const REFERENCE_LEVEL: u32 = 12;
/// The update in the level12 class table (level08 `0x2e8210` is the same code).
pub const UPDATE_FN: u32 = 0x2e_f648;
/// The spawner (level08 `0x2e7eb0`).
pub const SPAWN_FN: u32 = 0x2e_f2e8;
pub const CLASS: i16 = 458;
pub const CLASSES: [i16; 1] = [CLASS];

/// The homing target class the shell steers for.
const HOMING_CLASS: i16 = 0x1b3;

pub mod pvo {
    pub const VEL: usize = 0x00;
    pub const TARGET: usize = 0x10;
    pub const OWNER: usize = 0x14;
    pub const LIFE: usize = 0x18;
    pub const SPIN: usize = 0x1c;
    pub const LEN: usize = 0x20;
}

/// The level data (module doc).
mod k {
    pub const LIFE: f32 = 240.0;
    pub const TRAIL_N: f32 = 2.0;
    pub const BURST_N: f32 = 10.0;
    pub const SPIN_DEG: f32 = 320.0;
    pub const TRAIL_C: (u32, u32) = (0x30e0_ffff, 0x1080_c0c0);
    pub const BURST_C02: (u32, u32) = (0x10b0_c0c0, 0x0870_8080);
    pub const BURST_C21: (u32, u32) = (0x4f00_90f0, 0x4f00_00f0);
    pub const TRAIL_SPEED: f32 = 1.0;
    pub const BURST_SPEED: f32 = 2.0;
    pub const PHASES: [i32; 3] = [7, 8, 15];
    pub const BURST_W: (f32, f32) = (2.0, 3.0);
    pub const TRAIL_W: (f32, f32) = (0.5, 0.5);
    /// The homing turn (·dt) and the blast's bounds.
    pub const HOMING: f32 = 3.7;
}

/// The blast row (its light radius is the distance to the low edge, set per hit).
const BLAST: Beam = Beam { damage_r: 3.0, damage: 3.0, flash: 5.0, flash2: 3.0, flash_dist: 9.0, scale: 1.0, light: 0.0, streaks: 0, sparks: 10, puffs: 20, debris: 1, sound: -1, shake: true };

/// `trunc(multiply_global_scale(x))`.
fn gscale(w: &World, x: f32) -> i32 { w.svc.timing.scale(to_pf(x)).to_f32() as i32 }

/// The three type-02 phases (`trunc(scale(randf(t, 1.2t)))`, `trunc(scale(randf(t, 1.2t)))`, `trunc(scale(randf(t,
/// 1.5t)))`), drawn in that order.
fn phases(w: &mut World) -> [i32; 3] {
    let [a, b, cc] = k::PHASES.map(|x| x as f32);
    let r0 = w.rng.randf(a, a * 1.2);
    let t0 = gscale(w, r0);
    let r1 = w.rng.randf(b, b * 1.2);
    let t1 = gscale(w, r1);
    let r2 = w.rng.randf(cc, cc * 1.5);
    let t2 = gscale(w, r2);
    [t0, t1, t2]
}

/// `FastVecNormalize(len, out, v)`: `v` scaled to length `len` (xyz; w 0).
fn norm(v: [f32; 4], len: f32) -> [f32; 4] { let r = set_len3([v[0], v[1], v[2], 0.0], len); [r[0], r[1], r[2], 0.0] }

/// Level12 `0x2ef2e8(owner, pos, vel, target)` (level08 `0x2e7eb0`): a shell fired from `pos` at `vel` (module doc).
pub fn spawn(w: &mut World, owner: Option<MobyId>, pos: [f32; 4], vel: [f32; 4], target: Option<MobyId>) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    crate::moby_update::story::pvars(w, id, pvo::LEN);
    c::set_pi32(w, id, pvo::OWNER, owner.map_or(0, |o| o as i32 + 1));
    c::set_pi32(w, id, pvo::TARGET, target.map_or(0, |t| t as i32 + 1));
    {
        let m = w.mm(id);
        m.ambient = [0xa0, 0xa0, 0xa0, m.ambient[3]];
        m.rotation[0] = 0.0;
        m.draw_dist = 0xff;
        m.update_dist = 0xff;
        m.visible = 1;
        m.rotation[1] = -atan(len2(vel), vel[2]);
        m.rotation[2] = atan(vel[0], vel[1]);
        m.position = pos;
    }
    c::set_pv4(w, id, pvo::VEL, vel);
    c::set_pi32(w, id, pvo::SPIN, if w.counter & 1 == 0 { -1 } else { 1 });
    let r = w.m(id).rotation;
    let rows = crate::follow_camera::script::euler_rows([r[0], r[1], r[2]]);
    let row = |i: usize| [rows[i][0], rows[i][1], rows[i][2], 0.0];
    let mut i = 0.0;
    while i < k::BURST_N {
        let a = w.rng.rand_angle();
        let s = w.rng.randf(k::BURST_SPEED, k::BURST_SPEED + k::BURST_SPEED) * DT;
        let v = add(norm(row(2), s * a.cos()), norm(row(1), s * a.sin()));
        let v1 = [v[0], v[1], v[2], k::BURST_W.0];
        let v2 = [v[0], v[1], v[2], k::BURST_W.1];
        let (t10, t20) = (w.ticks(10), w.ticks(20));
        let life = w.rng.rand_range(t10, t20);
        part21(w, 20000.0, pos, v1, k::BURST_C21.0, k::BURST_C21.1, life, 1);
        let t = phases(w);
        part02(w, &crate::particles::type02::Spawn { pos, v1, v2, c1: k::BURST_C02.0, c2: k::BURST_C02.1, t, def: -1 });
        i += 1.0;
    }
    let life = if w.svc.level == 0xc { w.ticks(120) } else { gscale(w, k::LIFE) };
    c::set_pi32(w, id, pvo::LIFE, life);
    w.build_matrix(id);
    Some(id)
}

/// Level12 `0x2ef648` (level08 `0x2e8210`), module doc.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pvo::LEN { w.delete_moby(id); return; }
    let old = w.m(id).position;
    let mut vel = c::pv4(w, id, pvo::VEL);
    let p = add(old, [vel[0], vel[1], vel[2], vel[3]]);
    w.mm(id).position = p;
    let inside = (0..3).all(|i| 10.0 <= p[i] && p[i] <= 500.0);
    if !inside {
        w.delete_moby(id);
        return;
    }
    // The trail: back along this tick's move.
    let mut step = scale(vel, -1.0 / k::TRAIL_N);
    let mut q = p;
    let mut i = 0.0;
    while i < k::TRAIL_N {
        q = add(q, step);
        let a = w.rng.rand_angle();
        let s = w.rng.randf(0.0, k::TRAIL_SPEED) * DT;
        let rows = w.m(id).rows;
        let r = |i: usize| [rows[i][0], rows[i][1], rows[i][2], 0.0];
        let v2 = norm(r(1), s * a.sin());
        let v1 = add(norm(r(2), s * a.cos()), v2);
        let t = phases(w);
        let a = crate::particles::type02::Spawn { pos: q, v1: [v1[0], v1[1], v1[2], k::TRAIL_W.0], v2: [v2[0], v2[1], v2[2], k::TRAIL_W.1], c1: k::TRAIL_C.0, c2: k::TRAIL_C.1, t, def: -1 };
        part02(w, &a);
        i += 1.0;
    }
    let spin = c::pi32(w, id, pvo::SPIN) as f32;
    let rx = w.m(id).rotation[0];
    w.mm(id).rotation[0] = add_rot(rx, k::SPIN_DEG * 0.017_453_292 * DT * spin);
    // The homing.
    let t = c::pi32(w, id, pvo::TARGET);
    if t != 0 {
        match usize::try_from(t - 1).ok().filter(|&m| m < w.table.mobys.len()) {
            Some(m) if w.m(m).state == 0xfe || w.m(m).state == 0xfd => c::set_pi32(w, id, pvo::TARGET, 0),
            Some(m) if w.m(m).o_class == HOMING_CLASS => {
                let tp = w.m(m).position;
                let tv = if w.m(m).pvars.len() >= 0x30 { crate::moby_update::services::pvar::v4f(&w.m(m).pvars, 0x20) } else { [0.0; 4] };
                let speed = len3(vel);
                let d = dist3(p, tp);
                let lead = add(scale(tv, d / speed), tp);
                step = norm(sub(lead, p), speed);
                let mut turn = sub(step, vel);
                let l = len3(turn);
                let max = DT * k::HOMING;
                if max < l { turn = scale(turn, max / l); }
                vel = add(vel, turn);
                vel = norm(vel, speed);
                c::set_pv4(w, id, pvo::VEL, vel);
            }
            _ => c::set_pi32(w, id, pvo::TARGET, 0),
        }
    }
    // The hit template (+0x18: the stack word left by the trail step or the homing aim).
    let tmpl = HitTemplate { dir: pv(vel), attacker: Some(id), flags: 0x5_0001, b18: 0, b19: 0, h1a: 0, damage: to_pf(2.0), w20: 1 };
    let stale = step[2].to_bits();
    let tmpl = HitTemplate { b18: stale as u8, b19: (stale >> 8) as u8, h1a: (stale >> 16) as u16, ..tmpl };
    if c::dec_timer_pvar_i32(w, id, pvo::LIFE) != 0 {
        w.delete_moby(id);
        return;
    }
    let hit = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(old), pv(p), 0, Some(id), &tmpl);
    let Some(h) = hit else { return };
    let hp = [h.point[0], h.point[1], h.point[2], p[3]];
    w.mm(id).position = hp;
    if 15.0 < hp[0] && 15.0 < hp[1] && 15.0 < hp[2] {
        let r = 24.0f32.min(hp[0] - 2.0).min(hp[1] - 2.0).min(hp[2] - 2.0);
        beam_explosion(w, &Beam { light: r, ..BLAST }, Some(id), hp);
    }
    w.delete_moby(id);
}
