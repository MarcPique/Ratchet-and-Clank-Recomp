//! Ratchet's and Clank's pose producers outside the idle behaviour (docs/plan/hero_gameplay.md §19, G-HERO-009): the
//! writers of the joint records 0x17ab00 (`super::idle::JointRec`) that the hero update and the walk physics run, read
//! from the level01 decompiler output and the disassembly (all four the same code on the 19 overlays, `rc-trace
//! overlay-diff`): the feet `0x22c5c0` (the slope tilt from the ground probe's side probes, the worn feet's scale, the
//! foot motes), `HeroScanTargets` 0x22c080 (the head's look target 0x1415c4), the Magneboots lean `0x2352e0` and Clank's
//! sway on the back `0x235e60`. The grind's records are `super::boots` (level00's physics). Native `f32`.
//!
//! **Coverage** (every call and branch):
//!
//! | address | what it does | port |
//! |---|---|---|
//! | 0x232dc0 tail (0x13f610..0x13f624) | cleared on every probe; outside groups 0xf / 0x15 / 6 / 4 / 5 / 3 and under 0.25 above the ground: per side k (+90°, −90°) a line (flags 0x22, not Ratchet) from the hero-local (0, ±0.2, 0.45) to (0, ±0.2, −0.45) (`0x248c60`); a miss: dz 0, pitch / roll = the ground's (0x13f634 / 0x13f638); a hit: dz = hit z − ground z (0 beyond ±0.4), the raw normal's `0x235fe0` angles, each kept within ±45° | [`Hero::side_probes`] |
//! | `0x22c5c0` 0x22c5e0 | `0x22dea8` (the water groups): nothing | [`Hero::feet_update`] |
//! | 0x22c5ec..0x22c6a0 | \|dz\| > 0.03 or \|pitch\| / \|roll\| > 4° on either side, not on a magnetic floor (0x13f658 ≠ 1): records 4 / 5 y = −pitch; record 6 x = clamp(1.2·atan2(dz₀, 0.2), −10°, 45°), record 7 x = clamp(−1.2·atan2(dz₁, 0.2), −45°, 10°); records 4 / 5 x = −(roll + record 6 / 7 x) (`fast_subtract_rotations`) | [`Hero::feet_update`] |
//! | 0x22c7d0 | a feet item moby (0x140430): records 4 / 5 scale 0.01 | [`Hero::feet_update`] |
//! | 0x22c7f0..0x22c880 | motes enabled (0x1405fe & 0x80 clear): state 3 → `0x248920(0.021, 0.002, 1, 2, +1, −0)`; group 4 landed less than 12 ticks ago (0 < 0x13f768 < ticks(12)) → `0x248920(0.016, 0.002, 1, 2, +0, −1)`; disabled: the count 0x1405f8 = 0 | [`Hero::feet_update`] |
//! | 0x22c88c..0x22c970 | count > 0 and per-tick 0x1405fc ≠ 0: count − 1; at Ratchet's joint lists 22 and 23 (`FUN_002645a8`), `per` times `PartType28Spawn(spread + randf(±jitter), point, rows)` (rows = Ratchet's moby +0xc0 with flag 1, else none) | [`Hero::feet_update`] (`super::fx::PartSpawn::Mote28`) |
//! | `0x248920(spread, jitter, count, per, set, clear)` | 0x1405f0 / 0x1405f4 / 0x1405f8 / 0x1405fc = the arguments, 0x1405fe = (flags & !clear) \| set | [`Hero::foot_motes`]; callers: the fall's, the jumps', the glide's and the hover's landings (`air`, `jump`, `packs`) and the two above |
//! | `HeroScanTargets` 0x22c080 | outside groups 0 / 1 / 2 / 4 / 7: target 0x1415c4 = none; Ratchet on sequence 0x54: nothing | [`Hero::scan_targets`] |
//! | 0x22c0f0..0x22c280 | every 7th tick (0x15f5cc % 7): over the target list 0x1abe80 (mobys with a record): xy distance ≤ record +0x38, yaw off the facing ≤ 110°, elevation within 60°; score `d + off·d` (the current target −1.5, ≥ 0); a record +0x39 priority above the best or a lower score takes it (the best priority kept) | [`Hero::scan_targets`] |
//! | 0x22c290..0x22c2e0 | a line (flags 2, nothing ignored) from the hero-local (0, 0, 1.2) to the target + 0.5 up: blocked → none | [`Hero::scan_targets`] |
//! | 0x22c2e8..0x22c440 | the aim point (+ record +0x3a / 8 up) from the hero-local (0.3, 0, 0.85): yaw off the facing within ±59°, pitch (down) −25°..35°; in the first 27 ticks of the hit invulnerability (ticks(77) − ticks(50) < 0x13f510) yaw within ±17° and pitch 0; record 3 springs (0.021, 0.27), z = 0.7·yaw, y = pitch; record 2 springs (0.014, 0.3), z = 0.55 of record 3's | [`Hero::scan_targets`] |
//! | `0x2352e0` | off a magnetic floor (0x13f658 = 0): nothing; world down in the hero's frame (`fun_001f9d20((0, 0, −1), 0x13f390)`); record 1 springs (0.013, 0.3) | [`Hero::magnet_lean`] |
//! | 0x235354..0x2353f0 | the limit 25° (0x3f: 17°, 0x70: 5°, 0x71 / 4: 11°; 9° at most standing on a moby 0x13f64c) | [`Hero::magnet_lean`] |
//! | 0x2353f0..0x2354dc | record 1 y = clamp(40°·down.x, ±limit), x = clamp(−40°·down.y, ±limit); record 3 y = −½ record 1 y, x = −¼ record 1 x | [`Hero::magnet_lean`] |
//! | 0x2354dc..0x23561c | p = clamp(−70°·down.x, −7°, 57°), at least 80° with down.z > 0.85 (upside down), 70° above 0.5; records 13 / 14 / 15 / 16 y = 0.57 / 0.5 / 0.5 / 0.35·p; q = clamp(70°·down.y, ±70°): record 16 x = q, records 13 / 14 / 15 x = 0.7·q | [`Hero::magnet_lean`] |
//! | `0x235e60` (state 2's physics after `HeroLean`) | without Clank (0x1404d4 = 0): nothing; the cheat 0x15edb3: record 18 scale = gp−0x7de8 (0x15ee18) | [`Hero::clank_sway`] |
//! | 0x235eb4..0x235fc4 | state 2: record 18 springs (0.02, 0.35); z = clamp(−1.2·clamp(residual 0x13f4d8, ±1.4), ±75°); translation z = min(0.1·\|r\| + 0.1, 0.07) (always 0.07: the game's numbers) × 1024 / Clank's scale +0x2c | [`Hero::clank_sway`] |
//!
//! Side effects: the joint records (Ratchet's list +0x64 and Clank's through the springs `0x2273d0`), the foot motes
//! (particle type 28, with their draws), the look target; no sounds, lights, HUD or other mobys.

use super::fx::PartSpawn;
use super::idle::joint;
use super::physics::{ticks, to_f32x3, Env};
use super::Hero;
use crate::collision_query::{coll_line_m, QueryFlags};
use crate::rng::Rng;

const fn f(bits: u32) -> f32 { f32::from_bits(bits) }

/// 4°, 45°, 10°.
const DEG4: f32 = f(0x3d8e_fa35);
const DEG45: f32 = f(0x3f49_0fdb);
const DEG10: f32 = f(0x3e32_b8c2);

/// The ground probe's side probes (0x13f610..0x13f624), left (+90°) then right (−90°): the drop to each side's
/// floor (0x13f610 / 0x13f614), its pitch (0x13f618 / 0x13f61c) and roll (0x13f620 / 0x13f624).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SideProbes {
    pub dz: [f32; 2],
    pub pitch: [f32; 2],
    pub roll: [f32; 2],
}

/// The foot motes (0x1405f0..0x1405fe, written by `0x248920`): spread, jitter, ticks left, motes per foot and tick,
/// flags (1: turned by Ratchet's rows; 0x80: off).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FootMotes {
    pub spread: f32,
    pub jitter: f32,
    pub count: i32,
    pub per: i16,
    pub flags: u8,
}

/// `fast_subtract_rotations(a, b)` on `f32`: `a − b` wrapped into (−π, π].
fn sub_rot(a: f32, b: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut d = (a - b) % TAU;
    if PI < d { d -= TAU } else if d < -PI { d += TAU }
    d
}

impl Hero {
    /// A hero-local point (x forward, y left, z up from the feet) in the world: `rows 0x13f350 · v + 0x13f3d0`
    /// (`0x248cf8` / `0x248c60`).
    fn local_point(&self, v: [f32; 3]) -> [f32; 3] {
        let r = self.rows.map(to_f32x3);
        let p = to_f32x3(self.pos);
        std::array::from_fn(|k| ((r[0][k] * v[0] + r[1][k] * v[1]) + r[2][k] * v[2]) + p[k])
    }

    /// The side probes of the ground probe's tail (module doc; called by the ground probe with the ground's pitch and
    /// roll set, under the same conditions).
    pub(super) fn side_probes(&mut self, env: &Env) {
        let (pitch, roll) = (self.pitch.to_f32(), self.roll.to_f32());
        let gz = self.ground_z.to_f32();
        for k in 0..2 {
            let side = if k == 0 { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 };
            let (c, s) = (side.cos() * 0.2, side.sin() * 0.2);
            let a = self.local_point([c, s, f(0x3ee6_6666)]);
            let b = self.local_point([c, s, f(0xbee6_6666)]);
            let sd = &mut self.idle.side;
            let Some(o) = env.line(super::physics::from_f32x3(a), super::physics::from_f32x3(b), 0x22) else {
                sd.dz[k] = 0.0;
                sd.pitch[k] = pitch;
                sd.roll[k] = roll;
                continue;
            };
            let dz = o.point[2] - gz;
            sd.dz[k] = if 0.4 < dz.abs() { 0.0 } else { dz };
            // The raw normal (the output's +0x40, 0x174300).
            let n = super::physics::from_f32x3(o.normal);
            let (o0, o1) = self.frame_tilt(n);
            let (o0, o1) = (o0.to_f32(), o1.to_f32());
            let sd = &mut self.idle.side;
            if -DEG45 < o1 && o1 < DEG45 { sd.pitch[k] = o1; }
            if -DEG45 < o0 && o0 < DEG45 { sd.roll[k] = o0; }
        }
    }

    /// `0x248920(spread, jitter, count, per, set, clear)`: the foot motes' parameters.
    pub fn foot_motes(&mut self, spread: f32, jitter: f32, count: i16, per: u16, set: u8, clear: u8) {
        let m = &mut self.idle.motes;
        m.jitter = jitter;
        m.count = count as i32;
        m.per = (per & 0xff) as i16;
        m.flags = (m.flags & !clear) | set;
        m.spread = spread;
    }

    /// `0x22c5c0` (the hero update, after the transitions): the feet (module doc).
    pub fn feet_update(&mut self, anim: &dyn super::AnimCtl, rng: &mut Rng) {
        if self.in_water_groups() { return; }
        let sd = self.idle.side;
        let tilted = 0.03 < sd.dz[0].abs() || 0.03 < sd.dz[1].abs() || sd.pitch.iter().chain(&sd.roll).any(|v| DEG4 < v.abs());
        if tilted && self.f658 != 1 {
            let j = &mut self.idle.joints;
            j[joint::FOOT_L].target[1] = -sd.pitch[0];
            j[joint::FOOT_R].target[1] = -sd.pitch[1];
            let l = sd.dz[0].atan2(0.2) * 1.2;
            let r = sd.dz[1].atan2(0.2);
            let mut a = if l <= DEG45 { l } else { DEG45 };
            if a < -DEG10 { a = -DEG10; }
            j[joint::LEG_L].target[0] = a;
            let b = (-r * 1.2).clamp(-DEG45, DEG10);
            j[joint::LEG_R].target[0] = b;
            j[joint::FOOT_L].target[0] = -sub_rot(sd.roll[0], -a);
            j[joint::FOOT_R].target[0] = -sub_rot(sd.roll[1], -b);
        }
        // With a feet item moby (0x140430) Ratchet's feet records 4 / 5 get scale 0.01 this tick: the boots replace
        // his feet.
        if self.feet_slot.state != 0 {
            for k in [joint::FOOT_L, joint::FOOT_R] { self.idle.joints[k].scale = f(0x3c23_d70a); }
        }
        if self.idle.motes.flags & 0x80 != 0 {
            self.idle.motes.count = 0;
            return;
        }
        if self.state == 3 { self.foot_motes(f(0x3cac_0831), f(0x3b03_126f), 1, 2, 1, 0); }
        if self.group == 4 && 0 < self.jump.landed && self.jump.landed < ticks(12) { self.foot_motes(f(0x3c83_126f), f(0x3b03_126f), 1, 2, 0, 1); }
        let m = self.idle.motes;
        if m.count <= 0 || m.per == 0 { return; }
        let rows = (m.flags & 1 != 0).then(|| self.fx.frame.rows.map(|r| [r[0], r[1], r[2]]));
        self.idle.motes.count -= 1;
        for list in [0x16, 0x17] {
            let pos = super::fx::joint_point(self, anim, list);
            for _ in 0..m.per.max(0) {
                let spread = m.spread + rng.randf(-m.jitter, m.jitter);
                // PartType28Spawn's draws: 7 with the rows, 9 without (made here, as the game makes them).
                let r = super::fx::reserve(rng, if rows.is_some() { 7 } else { 9 });
                self.fx.parts.push(PartSpawn::Mote28 { spread, pos, rows, rng: r });
            }
        }
    }

    /// `HeroScanTargets` 0x22c080 (module doc). `seq_b` = Ratchet's moby +0x53, `counter` = 0x15f5cc.
    pub fn scan_targets(&mut self, env: &Env, seq_b: u8, counter: i32) {
        if !matches!(self.group, 0 | 1 | 2 | 4 | 7) {
            self.idle.look_target = 0;
            return;
        }
        if seq_b == 0x54 { return; }
        let targets = env.world.map(|w| w.melee_targets()).unwrap_or(&[]);
        let me = to_f32x3(self.pos);
        let facing = self.rot[2].to_f32();
        if counter % 7 == 0 {
            let (mut best_score, mut best_prio, mut best) = (1e8f32, 0.0f32, 0i32);
            for t in targets {
                let d = ((me[0] - t.pos[0]).powi(2) + (me[1] - t.pos[1]).powi(2)).sqrt();
                if !(d <= t.look[0] as f32) { continue; }
                let off = crate::moby_update::creature::diff_rots((t.pos[1] - me[1]).atan2(t.pos[0] - me[0]), facing);
                if !(off <= f(0x3ff5_bb53)) { continue; }
                let el = (me[2] - t.pos[2]).atan2(d);
                if !(el.abs() <= f(0x3f86_0a92)) { continue; }
                let mut score = off * d + d;
                if t.id as i32 + 1 == self.idle.look_target {
                    score -= 1.5;
                    if score < 0.0 { score = 0.0; }
                }
                let prio = t.look[1] as f32;
                if best_prio < prio || score < best_score {
                    best_score = score;
                    best = t.id as i32 + 1;
                    if best_prio < prio { best_prio = prio; }
                }
            }
            self.idle.look_target = best;
        }
        if self.idle.look_target == 0 { return; }
        let Some(t) = targets.iter().find(|t| t.id as i32 + 1 == self.idle.look_target).copied() else {
            // [L] The target left the list since the scan (the game reads the moby through its pointer).
            return;
        };
        let from = self.local_point([0.0, 0.0, 1.2]);
        let to = [t.pos[0], t.pos[1], t.pos[2] + 0.5];
        if coll_line_m(env.coll, env.mobys, from, to, QueryFlags(2), None).is_some() {
            self.idle.look_target = 0;
            return;
        }
        let c = [t.pos[0], t.pos[1], t.pos[2] + t.look[2] as f32 * 0.125];
        let h = self.local_point([0.3, 0.0, 0.85]);
        let mut yaw = sub_rot((c[1] - h[1]).atan2(c[0] - h[0]), facing);
        let mut pitch = (h[2] - c[2]).atan2(((c[0] - h[0]).powi(2) + (c[1] - h[1]).powi(2)).sqrt());
        let ylim = f(0x3f83_ce7d);
        if ylim < yaw { yaw = ylim; } else if yaw < -ylim { yaw = -ylim; }
        if f(0x3f1c_61aa) < pitch { pitch = f(0x3f1c_61aa); }
        if pitch < f(0xbedf_66f3) { pitch = f(0xbedf_66f3); }
        if ticks(0x4d) - ticks(0x32) < self.f510 {
            let m = f(0x3e97_e9d8);
            if m < yaw { yaw = m; } else if yaw < -m { yaw = -m; }
            pitch = 0.0;
        }
        let j = &mut self.idle.joints;
        (j[joint::HEAD].k, j[joint::HEAD].d) = (0.021, 0.27);
        j[joint::HEAD].target[2] = yaw * 0.7;
        j[joint::HEAD].target[1] = pitch;
        (j[joint::REC2].k, j[joint::REC2].d) = (0.014, 0.3);
        j[joint::REC2].target[2] = j[joint::HEAD].target[2] * 0.55;
    }

    /// `0x2352e0` (the hero update, before the springs): the lean on a magnetic floor (module doc).
    pub fn magnet_lean(&mut self) {
        if self.f658 == 0 { return; }
        let r = self.rows.map(to_f32x3);
        let (x, y, z) = (-r[0][2], -r[1][2], -r[2][2]);
        let mut lim = match self.state {
            0x3f => f(0x3e97_e9d8),
            0x70 => f(0x3db2_b8c2),
            0x71 | 4 => f(0x3e44_9809),
            _ => f(0x3edf_66f3),
        };
        if self.ground_moby.is_some() && f(0x3e20_d97c) < lim { lim = f(0x3e20_d97c); }
        let clamp = |v: f32, l: f32| if l < v { l } else if v < -l { -l } else { v };
        let k40 = f(0x3f32_b8c2);
        let k70 = f(0x3f9c_61aa);
        let j = &mut self.idle.joints;
        (j[joint::NECK].k, j[joint::NECK].d) = (f(0x3c54_fdf4), f(0x3e99_999a));
        j[joint::NECK].target[1] = clamp(x * k40, lim);
        j[joint::HEAD].target[1] = -j[joint::NECK].target[1] * 0.5;
        j[joint::NECK].target[0] = clamp(-y * k40, lim);
        j[joint::HEAD].target[0] = -j[joint::NECK].target[0] * 0.25;
        let mut p = -x * k70;
        if f(0x3f7e_adae) < p { p = f(0x3f7e_adae); }
        if p < f(0xbdfa_35dd) { p = f(0xbdfa_35dd); }
        if f(0x3f59_999a) < z {
            if p < f(0x3fb2_b8c2) { p = f(0x3fb2_b8c2); }
        } else if 0.5 < z && p < k70 {
            p = k70;
        }
        let q = clamp(y * k70, k70);
        let s = joint::SECONDARY;
        j[s].target[1] = p * f(0x3f11_eb85);
        j[s + 2].target[1] = p * 0.5;
        j[s + 3].target[1] = p * f(0x3eb3_3333);
        j[s + 1].target[1] = p * 0.5;
        j[s + 3].target[0] = q;
        let q7 = q * f(0x3f33_3333);
        j[s + 2].target[0] = q7;
        j[s].target[0] = q7;
        j[s + 1].target[0] = q7;
    }

    /// `0x235e60` (the walk physics, after `HeroLean`): Clank's sway on the back, record 18 (module doc).
    pub fn clank_sway(&mut self) {
        if self.back.is_none() { return; }
        // The cheat 0x15edb3: record 18's scale = 0x15ee18.
        if self.cheats.on(crate::cheats::slot::CLANK) { self.idle.joints[joint::CLANK0].scale = self.bodies.scale18; }
        let s = 1024.0 / self.idle.clank_scale;
        if self.state != 2 { return; }
        let r = self.yaw_residual.to_f32().clamp(-1.4, 1.4);
        let rec = &mut self.idle.joints[joint::CLANK0];
        (rec.k, rec.d) = (f(0x3ca3_d70a), f(0x3eb3_3333));
        let lim = f(0x3fa7_8d36);
        let z = -r * 1.2;
        rec.target[2] = if lim < z { lim } else if z < -lim { -lim } else { z };
        let t = (r.abs() * f(0x3dcc_cccd) + f(0x3dcc_cccd)).min(f(0x3d8f_5c29));
        rec.trans_target[2] = t * s;
    }

    /// Clank's moby scale +0x2c (his class's scale): record 18's translation is `1024 / scale` joint units per world unit.
    pub fn set_clank_scale(&mut self, scale: f32) { if 0.0 < scale { self.idle.clank_scale = scale; } }
}

/// The argument sets of the transitions' `0x248920` calls (the landings).
pub mod motes {
    /// `(spread, jitter, count, per, set, clear)` of the landings' calls.
    pub type Args = (f32, f32, i16, u16, u8, u8);
    pub const LAND: Args = (super::f(0x3c83_126f), super::f(0x3b03_126f), 5, 2, 0, 1);
    pub const LAND_ROWS: Args = (super::f(0x3c83_126f), super::f(0x3b03_126f), 5, 2, 1, 0);
    pub const LAND_WALK: Args = (super::f(0x3c83_126f), super::f(0x3b03_126f), 7, 2, 0, 1);
    pub const LAND_ROLL: Args = (super::f(0x3c9b_a5e3), super::f(0x3b03_126f), 10, 2, 0, 1);
    pub const LAND_HARD: Args = (super::f(0x3cb4_3958), super::f(0x3b83_126f), 10, 4, 0, 1);
}

impl Hero {
    /// [`Hero::foot_motes`] with one of [`motes`]' argument sets.
    pub fn land_motes(&mut self, a: motes::Args) { self.foot_motes(a.0, a.1, a.2, a.3, a.4, a.5); }
}
