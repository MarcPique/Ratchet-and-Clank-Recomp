//! The hero's water effects: the splash, the treading wake, the bow rings, the spray drops and the breath bubbles, and
//! the state code's calls to them (level01, read from the disassembly; every call site, its condition and its order in
//! `docs/plan/hero_states.md` "Swim effects"). One set of helpers for every hero state: a state calls them where the game
//! does, and they spawn through the general particle types (34 bubbles, 35 drops, 45 / 46 rings) and the splash moby
//! 775 (`crate::moby_update::classes::splash`), queued in the hero effects channel ([`super::super::fx`]) with the
//! spawners' random draws made here, at the call.
//!
//! | helper | game | what |
//! |---|---|---|
//! | [`splash`] | `0x22b3a8(rings, drops, big)` | big: the splash moby (size 2.25) on the water level; `rings` type-45 rings (±0.3, size 0.3..0.6); `drops` type-35 drops (up 3..8 u/s, out 0..3 u/s, kind `randi(2)`) |
//! | [`wake`] | `0x22ac40(max, after)` | every `after`+ ticks (counter 0x13fc54, restarted at `randi(max)`) one type-45 ring (±0.05, size 0.4..0.6) |
//! | [`bow_rings`] | `0x22ad38(max, after)` | every `after`+ ticks (0x13fc58) two type-46 rings 45° either side of the facing, 0.3 ahead, drifting back |
//! | [`spray`] | `0x22af48(max, after)` | every `after`+ ticks (0x13fc56) one type-35 drop 8 ticks of displacement ahead (kind 3 in water < 0.25 deep, else 1) |
//! | [`super::super::fx::bubbles_at`] | `0x22b140(n, mode)` | `n` type-34 bubbles at the feet (0) or Ratchet's joint lists 0 / 0xe (1), 0x17 / 0x16 (2) |
//! | [`breath`] | inline in 0x2370b8 | one big type-34 bubble (25200) at Ratchet's joint list 4 (the mouth) + 0.4 along his x row |
//! | [`jets`] | inline in 0x2370b8 (0x35) | four type-34 bubbles (6300..9450) at the Hydro-Pack's joint lists 0 / 1 |
//!
//! Standard `f32`; the one game quirk kept: [`splash`]'s drops start at `x + 8·vx`, `y + 8·vx` (the game adds the drop's
//! x velocity to both coordinates), which puts them on a diagonal through the hero.

use super::super::fx::{self, reserve, MobySpawn, PartSpawn};
use super::super::physics::{ticks, to_f32x3};
use super::super::{AnimCtl, Hero};
use crate::particles::type34;
use crate::rng::Rng;

/// `0x15ed6c`: dt.
const DT: f32 = 1.0 / 60.0;

fn hero_xyzw(h: &Hero) -> [f32; 4] { h.pos.map(|v| v.to_f32()) }

/// `0x22b3a8(rings, drops, big)`.
pub fn splash(h: &mut Hero, rng: &mut Rng, rings: i32, drops: i32, big: bool) {
    let p = hero_xyzw(h);
    let w = h.water_level.to_f32();
    if big {
        // FUN_002ff768(2.25, {x, y, 0x13f640, w}): CreateMoby(775) and its rand_angle.
        let angle = rng.rand_angle();
        h.fx.mobys.push(MobySpawn::Splash { size: 2.25, pos: [p[0], p[1], w, p[3]], angle });
    }
    for _ in 0..rings.max(0) {
        let x = p[0] + rng.randf(-0.3, 0.3);
        let y = p[1] + rng.randf(-0.3, 0.3);
        let size = rng.randf(0.3, 0.6);
        let r = reserve(rng, 4);
        h.fx.parts.push(PartSpawn::Ring45 { size, growth: 5250.0, pos: [x, y, w, p[3]], rng: r });
    }
    for _ in 0..drops.max(0) {
        let a = rng.rand_angle();
        let s = rng.randf(DT * 0.0, DT * 3.0);
        let (vx, vy) = (a.cos() * s, a.sin() * s);
        let vz = rng.randf(DT * 3.0, DT * 8.0);
        // The game's quirk: both coordinates move by 8 × the x velocity.
        let (x, y) = (p[0] + vx * 8.0, p[1] + vx * 8.0);
        let z = w - rng.randf(0.0, 0.2);
        let kind = rng.randi(2);
        let life = rng.rand_range(0x5a, 0x78);
        let r = reserve(rng, 2);
        h.fx.parts.push(PartSpawn::Drop35 { pos: [x, y, z, 0.0], vel: [vx, vy, vz, 0.0], kind, life, rng: r });
    }
}

/// `0x22ac40(max, after)`: the counter 0x13fc54 counts up; past `after` a type-45 ring on the water level at the hero
/// (±0.05), size `randf(0.4, 0.6)`, growth 5250, and the counter restarts at `randi(max)`.
pub fn wake(h: &mut Hero, rng: &mut Rng, max: i32, after: i16) {
    let c = &mut h.swim.fx_counters[0];
    *c = c.wrapping_add(1);
    if *c <= after { return; }
    let p = hero_xyzw(h);
    let w = h.water_level.to_f32();
    let x = p[0] + rng.randf(-0.05, 0.05);
    let y = p[1] + rng.randf(-0.05, 0.05);
    let size = rng.randf(0.4, 0.6);
    let r = reserve(rng, 4);
    h.fx.parts.push(PartSpawn::Ring45 { size, growth: 5250.0, pos: [x, y, w, 0.0], rng: r });
    h.swim.fx_counters[0] = rng.randi(max) as i16;
}

/// `0x22ad38(max, after)`: the counter 0x13fc58 counts up; past `after` two type-46 rings, at the facing + 45° (spin
/// +2) and − 45° (spin −2): velocity `(cos, sin)(a)·0.6·dt` less the facing's `1.5·dt`, placed 18 ticks of the first
/// part ahead on the water level, size `randf(0.4, 0.5)`; after each the counter restarts (0 for `max` 0, else
/// `randi(max)`).
pub fn bow_rings(h: &mut Hero, rng: &mut Rng, max: i32, after: i16) {
    let c = &mut h.swim.fx_counters[2];
    *c = c.wrapping_add(1);
    if *c <= after { return; }
    let p = hero_xyzw(h);
    let w = h.water_level.to_f32();
    let yaw = h.rot[2].to_f32();
    for side in 0..2 {
        let a = if side == 0 { yaw + std::f32::consts::FRAC_PI_4 } else { yaw - std::f32::consts::FRAC_PI_4 };
        let (mut vx, mut vy) = (a.cos() * (DT * 0.6), a.sin() * (DT * 0.6));
        let pos = [p[0] + vx * 18.0, p[1] + vy * 18.0, w, 0.0];
        vx -= yaw.cos() * (DT * 1.5);
        vy -= yaw.sin() * (DT * 1.5);
        let size = rng.randf(0.4, 0.5);
        let spin = if side == 0 { 2.0 } else { -2.0 };
        let r = reserve(rng, 2);
        h.fx.parts.push(PartSpawn::Ring46 { size, spin, pos, vel: [vx, vy, 0.0, 0.0], rng: r });
        h.swim.fx_counters[2] = if max == 0 { 0 } else { rng.randi(max) as i16 };
    }
}

/// `0x22af48(max, after)`: the counter 0x13fc56 counts up; past `after` one type-35 drop at the hero ± 0.15 plus 8 ticks
/// of his displacement 0x13f450, 0.1 above the water level, thrown up `randf(3·dt, 6.5·dt)` and out `randf(0, 3·dt)`
/// along `rand_angle` plus 0.75 of the displacement, life `rand_range(90, 120)`, kind 3 (lands on the water) in water
/// under 0.25 deep, else 1 (a line to the ground); the counter restarts at `randi(max)`.
pub fn spray(h: &mut Hero, rng: &mut Rng, max: i32, after: i16) {
    let c = &mut h.swim.fx_counters[1];
    *c = c.wrapping_add(1);
    if *c <= after { return; }
    let p = hero_xyzw(h);
    let d = to_f32x3(h.disp);
    let w = h.water_level.to_f32();
    let x = (p[0] + rng.randf(-0.15, 0.15)) + d[0] * 8.0;
    let y = (p[1] + rng.randf(-0.15, 0.15)) + d[1] * 8.0;
    let a = rng.rand_angle();
    let s = rng.randf(DT * 0.0, DT * 3.0);
    let vx = a.cos() * s + d[0] * 0.75;
    let vy = a.sin() * s + d[1] * 0.75;
    let vz = rng.randf(DT * 3.0, DT * 6.5);
    let life = rng.rand_range(0x5a, 0x78);
    let kind = if h.f15f4.to_f32() < 0.25 { 3 } else { 1 };
    let r = reserve(rng, 2);
    h.fx.parts.push(PartSpawn::Drop35 { pos: [x, y, w + 0.1, 0.0], vel: [vx, vy, vz, 0.0], kind, life, rng: r });
    h.swim.fx_counters[1] = rng.randi(max) as i16;
}

/// The breath bubble (inline in `0x2370b8`, cases 0x33..0x35 and 0x6a / 0x82): `PartType34Spawn(25200, −1, pos, vel)`
/// at Ratchet's joint list 4 plus 0.4 along his moby's x row, drifting with his displacement plus 1.5·dt along that row
/// and 2·dt up; the spawner's six draws.
pub fn breath(h: &mut Hero, rng: &mut Rng, anim: &dyn AnimCtl) {
    let j = fx::joint_point(h, anim, 4);
    let off = fx::moby_dir(h, [0.4, 0.0, 0.0]);
    let pos = [j[0] + off[0], j[1] + off[1], j[2] + off[2], j[3]];
    let l = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt();
    let k = if 0.0 < l { (DT * 1.5) / l } else { 0.0 };
    let d = to_f32x3(h.disp);
    let vel = [d[0] + off[0] * k, d[1] + off[1] * k, d[2] + off[2] * k + (DT + DT)];
    let level = if h.state == 0x31 { h.pos[2].to_f32() + 0.4 } else { h.water_level.to_f32() };
    let draws = type34::Draws::draw(rng);
    h.fx.parts.push(PartSpawn::Bubble { size: 25200.0, level, pos, vel, draws });
}

/// Under water (0x33..0x35, after the target speed): `FastDecTimer(0x13fc40)`; when it fires (1 at 0, 2 when it runs
/// out) the breath bubble, then the next one in `rand_range(ticks(4), ticks(11))` ticks when `randi(100) < 40`, else in
/// `rand_range(ticks(40), ticks(90))`.
pub fn breath_underwater(h: &mut Hero, rng: &mut Rng, anim: &dyn AnimCtl) {
    if dec_timer(&mut h.swim.breath) == 0 { return; }
    breath(h, rng, anim);
    let (a, b) = if (rng.randi(100) as f32) < 40.0 { (ticks(4), ticks(11)) } else { (ticks(40), ticks(90)) };
    h.swim.breath = rng.rand_range(a, b);
}

/// Drowned / eaten (0x6a / 0x82, after the momentum): the breath bubble only when 0x13fc40 runs out (2), then the next
/// one in `rand_range(ticks(2), ticks(5)) + timer/5` ticks during the first 40, or `rand_range(ticks(10), ticks(20))`
/// after 70 (none in between: the timer then stays out).
pub fn breath_drowning(h: &mut Hero, rng: &mut Rng, anim: &dyn AnimCtl) {
    if dec_timer(&mut h.swim.breath) != 2 { return; }
    breath(h, rng, anim);
    if h.timer < ticks(40) {
        h.swim.breath = rng.rand_range(ticks(2), ticks(5)) + h.timer / 5;
    } else if ticks(70) < h.timer {
        h.swim.breath = rng.rand_range(ticks(10), ticks(20));
    }
}

/// The Hydro-Pack's jets (0x35, after the breath, before the SpeedStep; 0x238244): `FastDecTimer(0x13fc48)` (s16); when
/// it fires and the back slot's pack moby 0x1404d0 is the Hydro-Pack (class 0x261), four bubbles at the pack's joint
/// lists 0, 1, 0, 1 ([`fx::pack_point`]), each moved 0.15 back along Ratchet's x row and by `randf(0, 0.03)` per axis,
/// drifting with his displacement plus `(cos yaw·cos pitch, sin yaw·cos pitch, sin pitch)·(−0.75·dt)` (`0x277bf0` with
/// the Euler pitch 0x13f3e4 as it is), size `randf(6300, 9450)`, `PartType34Spawn(size, −1, pos, vel)`; the timer
/// restarts at `trunc((7·dt − speed) / (7·dt) · ticks(5))` (every tick at full thrust).
pub fn jets(h: &mut Hero, rng: &mut Rng) {
    if dec_timer_s16(&mut h.swim.jets) == 0 { return; }
    let (pos0, is_hydro) = match h.back.as_ref() {
        Some(b) => (h.fx.back.position, b.pack_o_class == 0x261),
        None => return,
    };
    if !is_hydro { return; }
    let off = fx::moby_dir(h, [-0.15, 0.0, 0.0]);
    let d = to_f32x3(h.disp);
    let (yaw, pitch) = (h.rot[2].to_f32(), h.rot[1].to_f32());
    let s = DT * -0.75;
    let push = [yaw.cos() * s * pitch.cos(), yaw.sin() * s * pitch.cos(), pitch.sin() * s];
    let level = h.water_level.to_f32();
    for i in 0..4 {
        let j = fx::pack_point(h, i & 1).unwrap_or(pos0);
        let mut pos = [j[0] + off[0], j[1] + off[1], j[2] + off[2], j[3]];
        for p in pos.iter_mut().take(3) { *p += rng.randf(0.0, 0.03); }
        let vel = [d[0] + push[0], d[1] + push[1], d[2] + push[2]];
        let k = (DT * 7.0 - h.speed.to_f32()) / (DT * 7.0);
        h.swim.jets = (k * ticks(5) as f32) as i16;
        let size = rng.randf(6300.0, 9450.0);
        let draws = type34::Draws::draw(rng);
        h.fx.parts.push(PartSpawn::Bubble { size, level, pos, vel, draws });
    }
}

fn dec_timer_s16(t: &mut i16) -> i32 {
    let mut v = *t as i32;
    let r = dec_timer(&mut v);
    *t = v as i16;
    r
}

/// `FastDecTimer__FRi` 0x220e78: 1 when `t` is 0 (unchanged), else `t = max(t, 1) − 1` and 2 when that reaches 0, else 0.
fn dec_timer(t: &mut i32) -> i32 {
    if *t == 0 { return 1; }
    *t = (*t).max(1) - 1;
    if *t <= 0 { 2 } else { 0 }
}

/// The calls at the top of `0x2370b8`'s cases, before the state's own physics ([`Hero::state_physics`]):
/// * the ground cases 0, 1, 3, 4, 0x17, 0x1d, 0x1e, 0x1f, 0x3d, 0x70, 0x71, 0x72, 0x80 (0x23714c): standing in wading
///   water (0x1413f9) the wake `0x22ac40(15, 30)`;
/// * the walk / wade case 2, 0x73 (0x23a2e4): wading, or in water (0x140634) under 0.25 deep: moving faster than 1 u/s
///   (|eff.xy| 0x13f4b4 > dt) the bow rings `0x22ad38(0, 2)`, faster than 0.5 u/s the spray `0x22af48(2, 4)`, and every
///   tick five bubbles at the feet `0x22b140(5, 2)`.
pub fn physics_prologue(h: &mut Hero, anim: &dyn AnimCtl, rng: &mut Rng) {
    match h.state {
        0 | 1 | 3 | 4 | 0x17 | 0x1d | 0x1e | 0x1f | 0x3d | 0x70 | 0x71 | 0x72 | 0x80 => {
            if h.f13f9 != 0 { wake(h, rng, 15, 30); }
        }
        2 | 0x73 => {
            let shallow = h.f0634 != 0 && h.f15f4.to_f32() < 0.25;
            if h.f13f9 != 0 || shallow {
                let v = h.eff_len_xy.to_f32();
                if DT < v { bow_rings(h, rng, 0, 2); }
                if DT * 0.5 < v { spray(h, rng, 2, 4); }
                fx::bubbles_at(h, rng, Some(anim), 5, 2);
            }
        }
        _ => {}
    }
}

/// The deep-water jump 0x12 (its case of the jump physics, after the stick targets, which draw nothing and move nothing
/// these read): at tick 20 the big splash `0x22b3a8(3, 16, 1)` and `RippleDisturb(x, y, 0.4, 0.35)`, and while Ratchet's
/// frame is in [4, 7] seven bubbles at the feet `0x22b140(7, 0)`.
pub fn water_jump(h: &mut Hero, anim: &dyn AnimCtl, rng: &mut Rng) {
    if h.timer == ticks(0x14) {
        splash(h, rng, 3, 0x10, true);
        let (x, y) = (h.pos[0].to_f32(), h.pos[1].to_f32());
        h.swim.events.push(super::SwimEvent::Splash { rings: 3, drops: 0x10, big: true });
        h.swim.events.push(super::SwimEvent::Ripple { x, y, r: 0.4, amp: 0.35 });
    }
    let f = anim.view().frame;
    if (4.0..=7.0).contains(&f) { fx::bubbles(h, rng, 7); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ps2v::Pf;

    fn counted(from: Rng, to: Rng) -> usize {
        let mut r = from;
        (0..100_000).find(|_| { let hit = r == to; r.rand(); hit }).expect("stream never reached")
    }

    /// `0x22b3a8(3, n, 1)`: one moby draw, 3 + 4 per ring, 6 + 2 per drop; the spawns queued in the game's order.
    #[test]
    fn splash_draws_and_queue() {
        let mut h = Hero::new();
        h.pos = crate::hero::physics::v4(100.0, 100.0, 39.0);
        h.water_level = Pf::f(39.1);
        let mut rng = Rng::new();
        let r0 = rng;
        splash(&mut h, &mut rng, 3, 10, true);
        assert_eq!(counted(r0, rng), 1 + 3 * 7 + 10 * 8);
        assert_eq!(h.fx.mobys.len(), 1);
        let rings = h.fx.parts.iter().filter(|p| matches!(p, PartSpawn::Ring45 { .. })).count();
        let drops = h.fx.parts.iter().filter(|p| matches!(p, PartSpawn::Drop35 { .. })).count();
        assert_eq!((rings, drops), (3, 10));
        for p in &h.fx.parts {
            if let PartSpawn::Drop35 { pos, vel, .. } = p {
                // The game's quirk: x and y both move by 8·vx.
                assert!(((pos[0] - 100.0) - (pos[1] - 100.0)).abs() < 1e-4 && ((pos[0] - 100.0) - vel[0] * 8.0).abs() < 1e-4);
                assert!(pos[2] <= 39.1 && pos[2] >= 38.9);
            }
        }
    }

    /// The counters: `0x22ac40(15, 30)` spawns on the 31st call, then after 31 − randi(15) more.
    #[test]
    fn wake_counter() {
        let mut h = Hero::new();
        let mut rng = Rng::new();
        let mut at = Vec::new();
        for t in 0..200 {
            let n = h.fx.parts.len();
            wake(&mut h, &mut rng, 15, 30);
            if h.fx.parts.len() > n { at.push(t); }
        }
        assert_eq!(at[0], 30);
        assert!(at.windows(2).all(|w| (16..=31).contains(&(w[1] - w[0]))), "{at:?}");
    }
}
