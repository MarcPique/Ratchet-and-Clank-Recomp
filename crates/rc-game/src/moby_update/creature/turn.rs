//! Turning towards a heading and the eased approach (level01 `SpringTurn` 0x26cef0, `SpringTurn2` 0x26d058,
//! `0x270cc0` with its helpers `0x270ac0` / `0x2709f8`, `Approach` 0x270728). Angles are radians in [−π, π), wrapped the game's way
//! ([`super::add_rot`] / [`super::sub_rot`]).

use super::{add_rot, sub_rot};
use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

/// `Approach(t, step, &x)` 0x270728: `x` moves toward `t` by at most `step`; returns `|t − x|` afterwards.
pub fn approach(t: f32, step: f32, x: &mut f32) -> f32 {
    let d = t - *x;
    if d.abs() <= step { *x = t; } else if d > 0.0 { *x += step; } else { *x -= step; }
    (t - *x).abs()
}

/// `SpringTurn(cur, target, acc, damp, max, &vel)` 0x26cef0 on any angle: `d = wrap(target − cur)`,
/// `x = clamp(d / (π/20), −1, 1)`, `vel += acc·x − damp·vel`, `|vel| ≤ max` when `max ≠ 0`, `|vel| ≤ |d|`; returns
/// `wrap(cur + vel)`. (459 tilts its pitch with it, the dropship turns its camera; `classes::flyer` keeps a private
/// copy of the same function.)
pub fn spring_turn(cur: f32, target: f32, acc: f32, damp: f32, max: f32, vel: &mut f32) -> f32 {
    let d = sub_rot(target, cur);
    let x = (d / 0.157_079_64).clamp(-1.0, 1.0);
    *vel += acc * x - damp * *vel;
    if max != 0.0 {
        if max < *vel { *vel = max; } else if *vel < -max { *vel = -max; }
    }
    let a = d.abs();
    if a < *vel { *vel = a; } else if *vel < -a { *vel = -a; }
    add_rot(cur, *vel)
}

/// `SpringTurn2(target, acc, damp, max, moby, &vel)` 0x26d058: [`spring_turn`] on the moby's yaw (+0x48).
pub fn spring_turn2(w: &mut World, id: MobyId, target: f32, acc: f32, damp: f32, max: f32, vel: &mut f32) {
    let yaw = super::yaw(w, id);
    let y = spring_turn(yaw, target, acc, damp, max, vel);
    super::set_yaw(w, id, y);
}

/// [`spring_turn2`] with the velocity kept in the pvar f32 at `vel_off`.
pub fn spring_turn2_pvar(w: &mut World, id: MobyId, target: f32, acc: f32, damp: f32, max: f32, vel_off: usize) {
    let mut v = super::pf(w, id, vel_off);
    spring_turn2(w, id, target, acc, damp, max, &mut v);
    super::set_pf(w, id, vel_off, v);
}

/// `0x270ac0(t, step, &x, 0)`: an angle-valued approach (the difference wrapped, clamped to ±`step`, added wrapped).
pub fn approach_rot(t: f32, step: f32, x: &mut f32) {
    let d = sub_rot(t, *x).clamp(-step, step);
    *x = add_rot(*x, d);
}

/// `x += clamp(target − x, −step, step)`: the step `FUN_00270830` makes (the water managers' own `Approach` use too).
fn approach_add(x: &mut f32, target: f32, step: f32) {
    let d = target - *x;
    *x += d.clamp(-step, step);
}

/// `FUN_00270830(target, accel, decel, vmax, &x, &v)`: `x` springs toward `target` with velocity `v` (accelerating
/// by `accel`, braking by `decel` so that it stops there, at most `vmax` a tick): the linear twin of [`turn_toward`].
/// Used by the water managers' levels and the census units (the Rilgar floodgates 852 / 853, …).
///
/// Returns the game's value: the remaining difference when it snapped onto the target, else the velocity (0 once
/// settled: the Orxon flyers' rise ends on it).
pub fn spring(target: f32, accel: f32, decel: f32, vmax: f32, x: &mut f32, v: &mut f32) -> f32 {
    let d = target - *x;
    if *v * d < 0.0 || d == 0.0 {
        approach_add(v, 0.0, decel);
        *x += *v;
        return *v;
    }
    let stop = (*v * *v) / decel * 0.5;
    if d.abs() < stop {
        let k = if stop < d.abs() + v.abs() { 1.0 } else { 1.1 };
        approach_add(v, 0.0, decel * k);
    } else {
        // `vec_length` (VU0 `vsqrt`) of 2·decel·d: the square root of the magnitude.
        let s = (2.0 * decel * d).abs().sqrt().min(vmax);
        approach_add(v, if d < 0.0 { -s } else { s }, accel);
    }
    if d.abs() <= v.abs() {
        *x = target;
        d
    } else {
        *x += *v;
        *v
    }
}

/// `0x270cc0(target, accel, decel, vmax, &angle, &vel)`: turn `angle` toward `target` with an angular velocity that
/// accelerates by `accel`, brakes by `decel` in time to stop on the target (`v²/decel/2` ahead; 1.1× the braking when
/// already past that point), and is capped at `vmax`. Reversing or on target: brake to 0. Snaps onto the target when
/// the remaining turn is within one step. Returns the step taken (or the remaining difference when it snapped).
pub fn turn_toward(target: f32, accel: f32, decel: f32, vmax: f32, angle: &mut f32, vel: &mut f32) -> f32 {
    let d = sub_rot(target, *angle);
    if *vel * d < 0.0 || d == 0.0 {
        approach_rot(0.0, decel, vel);
    } else {
        let stop = ((*vel * *vel) / decel) * 0.5;
        if d.abs() < stop {
            if stop < d.abs() + vel.abs() { approach(0.0, decel, vel); } else { approach(0.0, decel * 1.1, vel); }
        } else {
            // `fun_001f9988` is the VU0 square root (of |x|).
            let s = ((decel + decel) * d).abs().sqrt().min(vmax);
            approach_rot(if d < 0.0 { -s } else { s }, accel, vel);
        }
        if d.abs() <= vel.abs() {
            *angle = target;
            return d;
        }
    }
    *angle = add_rot(*vel, *angle);
    *vel
}

/// [`turn_toward`] on the moby's yaw with the velocity in the pvar f32 at `vel_off`.
pub fn turn_toward_pvar(w: &mut World, id: MobyId, target: f32, accel: f32, decel: f32, vmax: f32, vel_off: usize) -> f32 {
    let mut a = super::yaw(w, id);
    let mut v = super::pf(w, id, vel_off);
    let r = turn_toward(target, accel, decel, vmax, &mut a, &mut v);
    super::set_yaw(w, id, a);
    super::set_pf(w, id, vel_off, v);
    r
}
