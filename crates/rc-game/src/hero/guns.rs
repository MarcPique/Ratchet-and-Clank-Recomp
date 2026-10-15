//! **What the gun-family hand items share** (docs/plan/hero_gameplay.md §9): the Blaster ([`super::blaster`]), the
//! Devastator ([`super::devastator`]), the R.Y.N.O. ([`super::ryno`]) and the Tesla Claw ([`super::tesla`]), with the
//! Pyrocitor ([`super::pyrocitor`]) and the Bomb Glove ([`super::weapons`]) before them. Only what the game's
//! updates share as code or as the same reads lives here; every update keeps its own constants and its own search:
//!
//! * [`item_point`] — `FUN_002645a8(item, list, out)` on the hand item (the muzzle: the Blaster's, the
//!   Devastator's and the Tesla Claw's list 0, the R.Y.N.O.'s lists 0..8 (one per barrel), the Pyrocitor's nozzle);
//! * [`first_person`] — the first-person test of the gun updates: Ratchet in the look stance 0x1e with the camera
//!   hiding him (`Ratchet moby +0x34 & 1`, the port's `0x1413f5`);
//! * [`camera_point`] / [`camera_axes`] — a point in the camera's frame (camera x = right, y = down, z = forward:
//!   `fun_001fa2d8` of the view rows 0x167100), where the first-person muzzles sit;
//! * [`aim_angles`] — the `(yaw, pitch)` pair every gun turns its aim vector into (`FastArcTan(v.x, v.y)` and
//!   `−FastArcTan(|v.xy|, v.z)`, the pitch positive down);
//! * [`crate::targeting::polar`] (`FUN_00277b50`) and [`crate::targeting::cone_miss`] (the Blaster's and the
//!   Devastator's cone), [`crate::targeting::Markers`] (the screen markers of the Blaster, the Devastator and the
//!   R.Y.N.O.), the arm / stance layer of [`super::weapons`] (`0x22ee08` / `0x22efd8`, the persistent-arm rules).
//!
//! Native `f32`.

use super::items::ItemEnv;
use super::Hero;
use rc_formats::moby_anim;

/// `FUN_002645a8(item, list, out)` for the hand item: the world point of the last joint of the item class's joint list
/// `list` in the item's pose now (its rows and position as `HeroItemsAttach` left them this tick). Without the list the
/// item's origin.
pub fn item_point(hero: &Hero, env: &ItemEnv, list: usize) -> [f32; 3] {
    let Some(it) = hero.items.slot.item.as_ref() else { return super::physics::to_f32x3(hero.pos) };
    let chain = env.data.class(it.o_class).and_then(|c| c.chains.get(list).filter(|c| !c.is_empty()).map(|ch| (c, ch)));
    match chain {
        Some((class, chain)) => {
            let p = moby_anim::evaluate_chains(&class.anim, &it.anim, it.snapshot.as_ref(), &[chain.as_slice()]);
            super::physics::to_f32x3(super::melee::list_point(&p[0], &it.rows, it.position, it.scale))
        }
        None => it.position,
    }
}

/// The hand item's rows (moby +0xc0 / +0xd0 / +0xe0) as `HeroItemsAttach` left them this tick.
pub fn item_rows(hero: &Hero) -> Option<[[f32; 3]; 3]> {
    hero.items.slot.item.as_ref().map(|it| it.rows.map(|r| [f32::from_bits(r[0]), f32::from_bits(r[1]), f32::from_bits(r[2])]))
}

/// The hand item's Euler angles (moby +0x40 / +0x44 / +0x48): `HeroItemsAttach` 0x22fec0 rewrites them every tick from
/// its matrix (`FUN_002721f0`).
pub fn item_euler(hero: &Hero) -> Option<[f32; 3]> {
    use crate::ps2v::Pf;
    let it = hero.items.slot.item.as_ref()?;
    let r = |i: usize| [Pf::b(it.rows[i][0]), Pf::b(it.rows[i][1]), Pf::b(it.rows[i][2]), Pf::ZERO];
    let e = crate::moby_update::services::rows_euler(&[r(0), r(1), r(2), [Pf::ZERO; 4]]);
    Some([e[0].to_f32(), e[1].to_f32(), e[2].to_f32()])
}

/// The weapon arm raised at a wall (0x141618 = 1, `super::idle`'s `0x22b700`): the guns aim along it instead.
pub fn arm_raised(hero: &Hero) -> bool { hero.arm_at_wall }

/// Joint record 9's pitch now (0x17b174, its +0x44: the raised arm's angle).
pub fn arm_pitch(hero: &Hero) -> f32 { hero.idle.joints[super::idle::joint::ARM].cur[1] }

/// The guns' first-person test: the look stance 0x1e with Ratchet hidden by the first-person camera.
pub fn first_person(hero: &Hero) -> bool { hero.state == 0x1e && hero.f13f5 != 0 }

/// The camera's world axes (right, down, forward) = the rows of `fun_001fa2d8(view 0x167100)`, from the camera rows
/// the items see (forward 0x167450, up 0x167470; left = up × forward).
pub fn camera_axes(env: &ItemEnv) -> Option<[[f32; 3]; 3]> {
    let (_, f) = env.camera?;
    let u = env.camera_up?;
    let l = [u[1] * f[2] - u[2] * f[1], u[2] * f[0] - u[0] * f[2], u[0] * f[1] - u[1] * f[0]];
    Some([[-l[0], -l[1], -l[2]], [-u[0], -u[1], -u[2]], f])
}

/// A point `local` (x right, y down, z forward) in the camera's frame, in the world: `camera 0x167240 + local · axes`.
pub fn camera_point(env: &ItemEnv, local: [f32; 3]) -> Option<[f32; 3]> {
    let (eye, _) = env.camera?;
    let a = camera_axes(env)?;
    Some(std::array::from_fn(|k| eye[k] + local[0] * a[0][k] + local[1] * a[1][k] + local[2] * a[2][k]))
}

/// A vector's aim `(FastArcTan(v.x, v.y), −FastArcTan(|v.xy|, v.z))`.
pub fn aim_angles(v: [f32; 3]) -> (f32, f32) {
    use crate::moby_update::creature::atan;
    (atan(v[0], v[1]), -atan((v[0] * v[0] + v[1] * v[1]).sqrt(), v[2]))
}

/// `FUN_00248cf8(x, y, z, out)`: the point `(x, y, z)` in Ratchet's frame (his rows 0x13f350, his position 0x13f3d0).
pub fn hero_point(hero: &Hero, local: [f32; 3]) -> [f32; 3] {
    let r = hero.rows.map(super::physics::to_f32x3);
    let p = super::physics::to_f32x3(hero.pos);
    std::array::from_fn(|k| p[k] + local[0] * r[0][k] + local[1] * r[1][k] + local[2] * r[2][k])
}

/// `FastDecTimer` on an `i32` timer: true when it was 0 or reaches 0 now.
pub fn dec(t: &mut i32) -> bool {
    if *t == 0 { return true; }
    *t = (*t).max(1) - 1;
    *t <= 0
}

/// `FastDecTimer` on an `s16` timer.
pub fn dec16(t: &mut i16) -> bool { super::idle::dec_timer_s16(t) != 0 }

pub fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
pub fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
pub fn scale3(a: [f32; 3], k: f32) -> [f32; 3] { a.map(|x| x * k) }
pub fn len3(a: [f32; 3]) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
pub fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
pub fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
/// `FastVecNormalize(l, out, v)`: `v` scaled to length `l` (0 stays 0).
pub fn with_len(a: [f32; 3], l: f32) -> [f32; 3] {
    let n = len3(a);
    if n == 0.0 { [0.0; 3] } else { scale3(a, l / n) }
}
/// `FUN_00221570(out, v, n)`: `v` reflected off the plane of normal `n` when it moves into it (`v·n ≤ 0`), else `v`.
pub fn reflect(v: [f32; 3], n: [f32; 3]) -> [f32; 3] {
    let u = with_len(n, 1.0);
    let d = dot3(v, u);
    if d <= 0.0 { sub3(v, scale3(u, 2.0 * d)) } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aims_and_reflections() {
        let (y, p) = aim_angles([1.0, 1.0, 0.0]);
        assert!((y - std::f32::consts::FRAC_PI_4).abs() < 1e-6 && p.abs() < 1e-6);
        let (_, p) = aim_angles([1.0, 0.0, -1.0]);
        assert!((p - std::f32::consts::FRAC_PI_4).abs() < 1e-6, "pitch positive down: {p}");
        assert_eq!(reflect([1.0, 0.0, -1.0], [0.0, 0.0, 2.0]), [1.0, 0.0, 1.0]);
        assert_eq!(reflect([1.0, 0.0, 1.0], [0.0, 0.0, 1.0]), [1.0, 0.0, 1.0], "moving out: kept");
        let mut t = 2;
        assert!(!dec(&mut t) && dec(&mut t) && dec(&mut t));
    }
}
