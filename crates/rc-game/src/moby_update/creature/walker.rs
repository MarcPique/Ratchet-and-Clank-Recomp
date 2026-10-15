//! Ground walking for creatures (level01 `SeedJumpPattern` 0x26d930, the step `0x26d9a8`, the move `0x26d8b0` /
//! `0x26d610` and the ledge probe `0x26d1d0`; the walk toward a point `0x26de80`, the free ground move `0x26d270`; the
//! random wander of the other levels, level00 `0x261630`).
//!
//! **Level copies** (G-ENM-009, 2026-09-29; the census's cluster hashes differ only by a callee's address): level03
//! `0x247c10` / level02 `0x25b710` (clusters ad33de15cdab, a8d6490c966c: levels 00, 02–05, 07, 08, 10, 17, 18) are
//! [`walk_to`] word for word — the only difference is the address of the `VecZero` they call first (level01
//! `0x221170`, level03 `0x1f8bf8`, level02 `0x2102c8`: each overlay's own copy of the same four stores) and of the
//! `SpringTurn2` / [`step`] copies. The wander `0x261630` has no level-01 copy: [`wander`].
//!
//! The walker record `J` (f32 words; the class's pvar, e.g. critter 577 at +0x180, amoeboid at +0x170):
//!
//! | word | seed | meaning |
//! |---|---|---|
//! | 0 | 0x280 (int) | collision radius ×1024 |
//! | 1 | 1.0 | step-up height (the sphere / line start above the feet) |
//! | 2 / 3 | 0.5 / 0.5 | ledge probe up / down |
//! | 4 | 0 | current speed (per tick) |
//! | 6 | 0 | vertical speed (gravity 9·dt² per tick while bit 0 of the flags is clear) |
//! | 7 | 0.3 | acceleration (× distance to the target when within 1) |
//! | 8 | 0.3 | braking fraction (within 10 ticks of the target) |
//! | 9 | 2.7 | top speed |
//! | 10 | 1.57 | — |
//! | 11 | 0.1 | "arrived" distance (result bit 4) |
//! | 12 | (pvar) | height of the origin above the ground (probe depth) |
//! | 14 | (pvar) | flags: 1 no gravity, 4 move along word 19 instead of the yaw, 8 walk off ledges, 0x10 / 0x20 the moby passes |
//! | 19 | — | heading used with flag 4 |
//!
//! **Step** (`0x26d9a8(k, moby, J, target, &out)`): speed += `min(acc·k·d, acc·k)` (d = xy distance to `target`),
//! braked by `J8·speed` when `d < 10·speed`, clamped to `±J9·k`; the move = (cos yaw, sin yaw)·speed plus the vertical
//! speed. The ledge probe (a line from `up` above to `down` below the stepped point, flags 0x22) decides: no ground →
//! result 0x22 (or 8 with flag 8), a slope steeper than 0.5 rad → one more probe 0.57 ahead (the far side may be
//! walkable). Blocked ledges (bit 1) do not move and reset the vertical speed. Otherwise the move (`0x26d610`): a line
//! guard for moves longer than the radius, up to six sphere push-outs (flags 4) at the step-up height, then a ground
//! line from the step-up height to `J12 + 0.05` below (flags 2): ground (a world face) → bit 2 and the vertical speed
//! reset to −0.01·[0x15ed60]; steep → bit 4; the origin eased up 30 % of the way when below `J12`. Walls → bit 0 (and
//! bit 1 too when less than 20 % of the move survived); a move that changed the height by more than the ledge limits is
//! undone (0x22). `out` receives the move actually made. Bit 4 is also set when within `J11` of the target.
//!
//! Note: the classes pass a *direction* (2·(cos yaw, sin yaw, 0)) as the target point (577, 572), so `d` is the
//! distance to a point near the world origin: in the game those creatures always accelerate to full speed and never
//! "arrive". The port passes the same values and gets the same result.

use super::{cs, dot3, len2, len3, scale, set_len3, sub, V};
use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pv, pvar, World};

/// `SeedJumpPattern(J)` 0x26d930.
pub fn seed(p: &mut [u8], j: usize) {
    pvar::set_i32(p, j, 0x280);
    pvar::set_u32(p, j + 0x2c, 0x3dcc_cccd);
    pvar::set_u32(p, j + 0x4, 0x3f80_0000);
    pvar::set_u32(p, j + 0xc, 0x3f00_0000);
    pvar::set_u32(p, j + 0x24, 0x402c_cccd);
    pvar::set_u32(p, j + 0x1c, 0x3e99_999a);
    pvar::set_u32(p, j + 0x28, 0x3fc8_f5c3);
    pvar::set_u32(p, j + 0x8, 0x3f00_0000);
    pvar::set_u32(p, j + 0x14, 0);
    pvar::set_u32(p, j + 0x10, 0);
    pvar::set_u32(p, j + 0x18, 0);
    pvar::set_u32(p, j + 0x20, 0x3e99_999a);
}

/// Result bits of [`step`].
pub mod res {
    pub const BLOCKED: u32 = 1;
    pub const STOPPED: u32 = 2;
    pub const ARRIVED: u32 = 4;
    pub const LEDGE_ALLOWED: u32 = 8;
    pub const NO_GROUND: u32 = 0x22;
}

fn w32(w: &World, id: MobyId, j: usize, k: usize) -> f32 { super::pf(w, id, j + 4 * k) }
fn set_w32(w: &mut World, id: MobyId, j: usize, k: usize, x: f32) { super::set_pf(w, id, j + 4 * k, x) }
fn flags(w: &World, id: MobyId, j: usize) -> u32 { pvar::u32(&w.m(id).pvars, j + 0x38) }

/// `0x26d1d0(up, down, p, moby, skip_mobys)`: `CollLine_Fix((p.xy, p.z + up), (p.xy, max(p.z − down, 0)), 0x22, moby)`;
/// a moby-primitive hit (negative kind) counts as none, and so does any moby hit with `skip_mobys`.
fn ledge_probe(w: &World, id: MobyId, up: f32, down: f32, p: V, skip_mobys: bool) -> Option<crate::collision_query::CollOutput> {
    let a = [p[0], p[1], p[2] + up, p[3]];
    let b = [p[0], p[1], (p[2] - down).max(0.0), p[3]];
    let o = w.coll_line(pv(a), pv(b), 0x22, Some(id))?;
    if o.kind < 0 { return None; }
    if skip_mobys && o.moby.is_some() { return None; }
    Some(o)
}

/// The slope of a hit normal from vertical: `FastArcTan(n.z, |n.xy|)`.
fn slope(o: &crate::collision_query::CollOutput) -> f32 {
    let n = [o.normal[0], o.normal[1], o.normal[2], 0.0];
    super::atan(n[2], len2(n))
}

/// `0x26d610(up, radius, down, moby, &mv, flags)`: the move with collision (module doc). Returns bit 0 walls, bit 1
/// ground under the feet, bit 2 steep ground; `mv` becomes the displacement made.
pub fn move_collide(w: &mut World, id: MobyId, up: f32, radius: f32, down: f32, mv: &mut V, flags: u32) -> u32 {
    let sflags = (flags & 0x10) << 1;
    let old = super::pos(w, id);
    let mut p = super::add(old, *mv);
    super::set_pos(w, id, p);
    let mut r = 0u32;
    if radius < len3(*mv) {
        let mut a = old;
        a[2] += up;
        let s = set_len3(*mv, radius);
        let mut b = super::add(s, p);
        b[2] += up;
        if let Some(o) = w.coll_line(pv(a), pv(b), sflags, Some(id)) {
            let h = [o.point[0], o.point[1], o.point[2], 0.0];
            p = sub(h, s);
            r = 1;
            p[2] -= up;
            super::set_pos(w, id, p);
        }
    }
    for _ in 0..6 {
        let mut c = p;
        c[2] += up;
        let Some(o) = w.coll_sphere(pv(c), crate::moby_update::services::pf(radius), sflags | 4, Some(id)) else { break };
        let Some(pc) = o.pushed_centre else { break };
        p = [pc[0], pc[1], pc[2], p[3]];
        r |= 1;
        p[2] -= up;
        super::set_pos(w, id, p);
    }
    let zlo = p[2] - down;
    let mut a = p;
    a[2] += up;
    let mut b = p;
    b[2] = zlo - 0.05;
    let mut out = r;
    if let Some(o) = w.coll_line(pv(a), pv(b), sflags | 2, Some(id)) {
        if o.kind > 0 {
            out = r | 2;
            if 0.5 < slope(&o) { out |= 4; }
            if zlo < o.point[2] {
                p[2] += (o.point[2] - zlo) * 0.3;
                super::set_pos(w, id, p);
            }
        }
    }
    *mv = sub(p, old);
    out
}

/// `0x26d9a8(k, moby, J, target, &out)`: one walking step (module doc). Returns the result bits ([`res`]); `out` gets
/// the move made (unchanged when the step was refused).
pub fn step(w: &mut World, id: MobyId, j: usize, k: f32, target: V, out: &mut V) -> u32 {
    let mut r = 0u32;
    let acc = w32(w, id, j, 7) * k;
    let vmax = w32(w, id, j, 9) * k;
    let fl = flags(w, id, j);
    let p0 = super::pos(w, id);
    let d = len2(sub(target, p0));
    let mut a = acc * d;
    if acc < a { a = acc; }
    let mut speed = w32(w, id, j, 4) + a;
    if d < speed * 10.0 { speed -= w32(w, id, j, 8) * speed; }
    if vmax < speed { speed = vmax; } else if speed < -vmax { speed = -vmax; }
    set_w32(w, id, j, 4, speed);
    let heading = if fl & 4 != 0 { w32(w, id, j, 19) } else { super::yaw(w, id) };
    let (c, s) = cs(heading);
    let mut mv: V = [c * speed, s * speed, 0.0, 0.0];
    if fl & 1 == 0 {
        mv[2] = w32(w, id, j, 6);
        set_w32(w, id, j, 6, mv[2] - super::DT2 * 9.0);
    }
    let zc = p0[2] + 0.04;
    let depth = w32(w, id, j, 12);
    let (up, down) = (w32(w, id, j, 2), w32(w, id, j, 3));
    let mut q = super::add(p0, mv);
    q[2] = p0[2] - depth;
    let skip = fl & 0x20 != 0;
    let mut stop;
    match ledge_probe(w, id, up, down, q, skip) {
        None => {
            r = if fl & 8 != 0 { 8 } else { res::NO_GROUND };
            stop = r & 2 != 0;
        }
        Some(o) => {
            let gz = o.point[2];
            stop = false;
            if 0.5 < slope(&o) && 0.1 < len2(mv) {
                let mut q2 = super::add(set_len3(mv, 0.57), p0);
                q2[2] = p0[2] - depth;
                let far = ledge_probe(w, id, up, down, q2, skip);
                // LAB_0026dca0: 0x22, stopping only when the stepped ground is at or above the feet.
                let ledge_rule = |r: &mut u32| -> bool { *r = 0x22; if zc <= gz { true } else { *r = 8; false } };
                match far {
                    None => {
                        r = 0x22;
                        stop = if fl & 8 != 0 { ledge_rule(&mut r) } else { true };
                    }
                    Some(o2) => {
                        if 0.5 <= slope(&o2) {
                            r = 0x22;
                            stop = if fl & 8 != 0 { if zc <= o2.point[2] { true } else { ledge_rule(&mut r) } } else { true };
                        }
                    }
                }
            }
        }
    }
    if !stop {
        let l = len2(mv);
        let dirn = if l == 0.0 { [0.0; 4] } else { scale(mv, 1.0 / l) };
        let before = p0;
        let up1 = w32(w, id, j, 1);
        let radius = super::pi32(w, id, j) as f32 * (1.0 / 1024.0);
        let m = move_collide(w, id, up1, radius, depth, &mut mv, fl);
        if m & 2 != 0 { set_w32(w, id, j, 6, super::SPEED * -0.01); }
        if m & 1 != 0 {
            r |= 1;
            if mv[0] * dirn[0] + mv[1] * dirn[1] < l * 0.2 { r |= 3; }
            let g = super::ground::ground(w, super::pos(w, id), 0.5, 0).z;
            if up < before[2] - g || down < g - before[2] {
                r |= 0x22;
                super::set_pos(w, id, before);
            }
        }
        *out = mv;
    } else if fl & 1 == 0 {
        set_w32(w, id, j, 6, super::SPEED * -0.01);
    }
    let _ = dot3;
    if d < w32(w, id, j, 11) { r |= 4; }
    r
}

/// `0x26de80(m, J, target, &out)`: the walk toward `target`: `out` cleared; bit 4 when within J11 (3-D); the turn
/// (`SpringTurn2(heading, J15, J16, J17, m, &J5)`); bit 0x10 when facing away by J10 or more; the step timer J18
/// (`FastDecTimer`, and kept at least 4 ticks while facing within J10); while it runs, [`step`] scaled by how well it
/// faces the target (`(J10 − diff) / J10`), its bit 1 dropped when more than 0.1 off. Returns the bits. The mouse 1818
/// (its record in its pvars) and the Glove of Doom's bots 186 (the globals 0x141210, kept per bot) use it.
pub fn walk_to(w: &mut World, id: MobyId, j: usize, target: V, out: &mut V) -> u32 {
    *out = [0.0; 4];
    let mut r = 0;
    if super::dist3(super::pos(w, id), target) < w32(w, id, j, 11) { r = 4; }
    let p = super::pos(w, id);
    let a = super::atan(target[0] - p[0], target[1] - p[1]);
    let (acc, damp, max) = (w32(w, id, j, 15), w32(w, id, j, 16), w32(w, id, j, 17));
    super::turn::spring_turn2_pvar(w, id, a, acc, damp, max, j + 0x14);
    let d = super::diff_rots(a, super::yaw(w, id));
    let lim = w32(w, id, j, 10);
    if lim <= d { r |= 0x10; }
    let t4 = w.ticks(4);
    let mut tm = super::pi32(w, id, j + 0x48);
    super::dec_timer_i32(&mut tm);
    if d < lim && tm < t4 { tm = t4; }
    super::set_pi32(w, id, j + 0x48, tm);
    if tm != 0 {
        let s = step(w, id, j, (lim - d) / lim, target, out);
        r = if 0.1 < d { r | (s & !2) } else { r | s };
    }
    r
}

/// `0x26d270(up, radius, max_dz, max_slope, moby, &from, &to, flags)`: a free move from `from` to `to` over the ground
/// (the chicken 270's walk; census: 749, 238, 1023, 294 and the floating pushables 695 call it too). Returns 1 when the
/// move went through untouched, 0 when it was refused or pushed; `to` is the point reached (z as the pushes left it)
/// and `from.xy` becomes `to.xy` (the caller's copy of the position; its z is not written).
///
/// 1. The ground under `to` (`GroundHeight(0.5, to + up, 0x20)`, `0x26e690`): more than `max_dz` from `from.z` → back
///    to `from` (0) — going up always, going down only without flag 1.
/// 2. Unless refused, a line from `from + up` along the move, `1.2·radius` long (flags `(flags & 2) | 0x24`): a
///    world face (`CollOutput +0x1c > 0`) whose slope from vertical is at least `max_slope` slides the move along the
///    wall (the normal flattened and normalised; the part of the move into it removed; 0); any such hit sets
///    `to = from + move`.
/// 3. Without flag 2, up to six sphere push-outs (radius `radius`, centre `to + up + radius`, flags 0x24): a moby hit,
///    or a hit point whose elevation from `to` exceeds `max_slope`, moves `to` to the pushed centre (0).
/// 4. The ground test of 1 again on the result.
#[allow(clippy::too_many_arguments)]
pub fn move_ground(w: &mut World, id: MobyId, up: f32, radius: f32, max_dz: f32, max_slope: f32, from: &mut V, to: &mut V, flags: u32) -> i32 {
    move_ground_in(w, id, [up, radius, max_dz, max_slope], from, Some(to), flags)
}

/// [`move_ground`] with `from` and `to` the same point (the chicken's "stay" calls pass one buffer for both): the
/// line of 2 has no length, the push-outs of 3 move the point (and so the `from` of the ground tests).
#[allow(clippy::too_many_arguments)]
pub fn settle(w: &mut World, id: MobyId, up: f32, radius: f32, max_dz: f32, max_slope: f32, p: &mut V, flags: u32) -> i32 {
    move_ground_in(w, id, [up, radius, max_dz, max_slope], p, None, flags)
}

fn move_ground_in(w: &mut World, id: MobyId, k: [f32; 4], from: &mut V, to: Option<&mut V>, flags: u32) -> i32 {
    let [up, radius, max_dz, max_slope] = k;
    let alias = to.is_none();
    let mut own = *from;
    let to: &mut V = match to { Some(t) => t, None => &mut own };
    let mut ok = 1;
    let ground_z = |w: &World, p: V| -> f32 {
        let mut q = p;
        q[2] += up;
        // FUN_0026e690 = GroundHeight(0.5, q, 0x20).
        super::ground::ground(w, q, 0.5, 0x20).z
    };
    let refuse = |g: f32, from: &V| max_dz < (g - from[2]).abs() && ((flags ^ 1) & 1 != 0 && g < from[2] || from[2] < g);
    let g = ground_z(w, *to);
    if refuse(g, from) {
        *to = *from;
        ok = 0;
    }
    let mut d = if alias { [0.0; 4] } else { sub(*to, *from) };
    let dir = set_len3(d, radius * 1.2);
    let mut a = *from;
    a[2] += up;
    let b = super::add(a, dir);
    if ok != 0 && !alias {
        if let Some(o) = w.coll_line(pv(a), pv(b), (flags & 2) | 0x24, Some(id)) {
            if 0 < o.kind {
                let n = [o.normal[0], o.normal[1], o.normal[2], 0.0];
                let slope = super::atan(n[2], len2(n));
                if max_slope <= slope {
                    let flat = set_len3([n[0], n[1], 0.0, 0.0], 1.0);
                    let into = -d[0] * flat[0] - d[1] * flat[1];
                    if 0.0 < into {
                        ok = 0;
                        d = super::add(d, scale(flat, into));
                    }
                }
                *to = super::add(*from, d);
            }
        }
    }
    if flags & 2 == 0 {
        let lift = up + radius;
        for _ in 0..6 {
            let mut c = *to;
            c[2] += lift;
            let Some(o) = w.coll_sphere(pv(c), crate::moby_update::services::pf(radius), 0x24, Some(id)) else { break };
            let hp = [o.point[0], o.point[1], o.point[2], 0.0];
            let elev = super::atan(len2(sub(*to, hp)), hp[2] - to[2]);
            if o.moby.is_some() || max_slope < elev {
                if let Some(pc) = o.pushed_centre { *to = [pc[0], pc[1], pc[2], to[3]]; }
                ok = 0;
                to[2] -= lift;
                if alias { *from = *to; }
            }
        }
    }
    let g = ground_z(w, *to);
    if refuse(g, from) {
        *to = *from;
        ok = 0;
    }
    let (x, y) = (to[0], to[1]);
    from[0] = x;
    from[1] = y;
    ok
}

/// The random wander's record (`0x261630`'s last argument; 749 keeps it at pvar +0x240, 340 at its own offset).
pub mod wr {
    /// +0x00 vec: the home point the leash measures from.
    pub const HOME: usize = 0x00;
    /// +0x10: the move's collision radius; +0x14 the step a tick; +0x18 the turn step a tick.
    pub const RADIUS: usize = 0x10;
    pub const STEP: usize = 0x14;
    pub const TURN: usize = 0x18;
    /// +0x1c: the leash (xy distance from home); +0x20 the distance within which Ratchet is shied from.
    pub const LEASH: usize = 0x1c;
    pub const SHY: usize = 0x20;
    /// +0x24: the heading the wander turns toward.
    pub const HEADING: usize = 0x24;
    /// +0x28 s16: 1 while turning toward a picked heading, 0 = pick a new one; +0x2a s16 its timer; +0x2c / +0x2e s16
    /// the timer's range (`rand_range`).
    pub const TURNING: usize = 0x28;
    pub const TIMER: usize = 0x2a;
    pub const T_MIN: usize = 0x2c;
    pub const T_MAX: usize = 0x2e;
}

/// `0x261630(up, max_dz, moby, R)` (level00; the same code on levels 04 `0x254348`, 06 `0x2701c0`, 10 `0x254140`, 12
/// `0x27a0d8`, 18 `0x263970`, cluster 9a9b8f2fffcb; not linked on level 01): a random wander on the ground around a home
/// point ([`wr`]).
///
/// 1. Not turning: heading += `randf_sym(π/4, 5π/6)`, timer = `rand_range(R.2c, R.2e)`, turning. Turning: the yaw
///    approaches the heading by at most R.18 (`0x270ac0`); the timer out → not turning.
/// 2. The step: `to = pos + R.14·(cos yaw, sin yaw)`, [`move_ground`]`(up, R.10, max_dz, 30°, pos → to, flags 0)` (the
///    moby's xy follows), then z = `GroundHeight(0.5, pos, 0)` (0 over no ground, as in the game).
/// 3. The move refused or pushed, or farther than R.1c (xy) from home: head home (the heading = the direction to it), a
///    turn of `rand_range(ticks 30, ticks 90)`. Otherwise Ratchet (his feet) within R.20 (xy): the heading turns
///    toward "away from him" by the fraction `distance / R.20` ([`super::lerp_rot`]).
pub fn wander(w: &mut World, id: MobyId, up: f32, max_dz: f32, r: usize) {
    let g = |w: &World, o: usize| super::pf(w, id, r + o);
    if super::pi16(w, id, r + wr::TURNING) == 0 {
        let d = w.rng.randf_sym(f32::from_bits(0x3f49_0fdb), f32::from_bits(0x4027_8d36));
        let h = g(w, wr::HEADING) + d;
        super::set_pf(w, id, r + wr::HEADING, h);
        let (a, b) = (super::pi16(w, id, r + wr::T_MIN), super::pi16(w, id, r + wr::T_MAX));
        let t = w.rng.rand_range(a as i32, b as i32);
        super::set_pi16(w, id, r + wr::TIMER, t as i16);
        super::set_pi16(w, id, r + wr::TURNING, 1);
    } else {
        let mut yaw = super::yaw(w, id);
        super::turn::approach_rot(g(w, wr::HEADING), g(w, wr::TURN), &mut yaw);
        super::set_yaw(w, id, yaw);
        if super::dec_timer_pvar_s16(w, id, r + wr::TIMER) != 0 { super::set_pi16(w, id, r + wr::TURNING, 0); }
    }
    let mut pos = super::pos(w, id);
    let (c, s) = cs(super::yaw(w, id));
    let step = g(w, wr::STEP);
    let mut to = [pos[0] + c * step, pos[1] + s * step, pos[2], pos[3]];
    let ok = move_ground(w, id, up, g(w, wr::RADIUS), max_dz, f32::from_bits(0x3f06_0a92), &mut pos, &mut to, 0);
    pos[2] = super::ground::ground(w, pos, 0.5, 0).z;
    super::set_pos(w, id, pos);
    let home = super::pv4(w, id, r + wr::HOME);
    if ok == 0 || g(w, wr::LEASH) < super::dist2(pos, home) {
        super::set_pf(w, id, r + wr::HEADING, super::atan(home[0] - pos[0], home[1] - pos[1]));
        let (a, b) = (w.ticks(30), w.ticks(90));
        let t = w.rng.rand_range(a, b);
        super::set_pi16(w, id, r + wr::TIMER, t as i16);
        super::set_pi16(w, id, r + wr::TURNING, 1);
    } else {
        let hero = w.hero.pos.map(|x| f32::from_bits(x.0));
        let dh = super::dist2(pos, hero);
        let shy = g(w, wr::SHY);
        if dh < shy {
            let away = super::atan(pos[0] - hero[0], pos[1] - hero[1]);
            let h = super::lerp_rot(g(w, wr::HEADING), away, dh / shy);
            super::set_pf(w, id, r + wr::HEADING, h);
        }
    }
}
