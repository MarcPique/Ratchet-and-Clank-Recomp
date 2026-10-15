//! U174 (census 2026-10-02): class 642, Eudora's drifting speck (level04 0x2d7e90, the only copy: 1 placed). Read from
//! the level04 disassembly (the decompiler mixes up its stack vectors). Native `f32`; the `rand` draws in the game's
//! order (three every tick).
//!
//! A speck that waits unseen at its place until Ratchet's camera is within 32 (xy) and a random 2–40 s has passed,
//! then drifts on the level wind for 12–20 s (bouncing off the world, falling at most 0.5·dt a tick), grows in over
//! 10 ticks, fades over its last 100 and starts again at its place. The wind is the engine's ambience block
//! (`LevelAmbienceUpdate`, L01 0x2b9a68; here 0x1cb940: +0x04 its direction, +0x14 its speed). Nothing on level 04
//! sets the block's mode, so both stay 0 ([`WIND_DIR`], [`WIND_SPEED`]): the speck only sinks.
//!
//! **Pvars**: +0x00 velocity, +0x10 the drift, +0x20 the spin (per tick), +0x30 the place, +0x40 s32 the period, +0x44
//! s32 the timer, +0x4c the full scale.
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | R = (`randf(−1, 1)` ×3, 0); the old position; \|velocity\| (xy, `0x221318`) | [`update`] |
//! | 0 | +0x30 = position; period = timer = `rand_range(120, 2400)`; +0x4c = scale · `randf(0.8, 1.2)`; 1; scale 0 | [`update`] |
//! | 1 | within 32 (xy) of the camera and the timer (`FastDecTimer__FRi`) out: R2 = three draws; draw distance 0x40, update distance 0xff, scale 0; T = R2 at wind/5; velocity = (wind·cos dir, wind·sin dir, −0.5·dt); Euler = T at π, spin = T at π/32, drift = T at wind/2; period = timer = `rand_range(720, 1200)`; 2 | [`update`] |
//! | 2 | the timer; drift += R at wind/10; `CollLine_Fix(old, position + drift at 5, 6)` hit and `randi(8)` = 0 → drift reflected (`0x221570`), z + wind/10; drift at wind/2; velocity = 0.6·(velocity + drift) + 0.4·(wind·cos, wind·sin, −0.5·dt); position += it; `CollLine_Fix(old, position, 6)` hit → position = the hit, velocity reflected, ·0.25, position += it, z + wind/5; wind < the start's \|velocity\| → velocity at wind; z: above −0.5·dt → + (−0.5·dt)/10, else −0.5·dt; Euler += spin (`fast_add_rotations`) | [`drift`] |
//! | 2 | scale: within `ticks(10)` of the period's start → +0x4c·(timer − period)/`scale(10)`; under `ticks(100)` left → ·0.99; else +0x4c; the timer out → back at the place, period = timer = `rand_range(120, 2400)`, 1, scale 0 | [`update`] |
//!
//! **The game's, kept:** the grow-in scale is `timer − period`, so negative (the model drawn mirrored) for its first 10
//! ticks.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{self as sv, pv, World};
use crate::ps2v::Pf;
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x2d_7e90;
pub const CLASSES: [i16; 1] = [642];

/// The ambience block's wind direction (0x1cb944) and speed (0x1cb954) on level 04: never set (module doc).
pub const WIND_DIR: f32 = 0.0;
pub const WIND_SPEED: f32 = 0.0;

const VEL: usize = 0x00;
const DRIFT: usize = 0x10;
const SPIN: usize = 0x20;
const HOME: usize = 0x30;
const PERIOD: usize = 0x40;
const TIMER: usize = 0x44;
const FULL: usize = 0x4c;
const SIZE: usize = 0x50;

fn rvec(w: &mut World) -> c::V {
    let x = w.rng.randf(-1.0, 1.0);
    let y = w.rng.randf(-1.0, 1.0);
    let z = w.rng.randf(-1.0, 1.0);
    [x, y, z, 0.0]
}

fn restart(w: &mut World, id: MobyId, lo: i32, hi: i32) {
    let t = w.rng.rand_range(lo, hi);
    c::set_pi32(w, id, PERIOD, t);
    c::set_pi32(w, id, TIMER, t);
}

/// The wind vector (wind·cos dir, wind·sin dir, −0.5·dt).
fn wind() -> c::V { [WIND_SPEED * WIND_DIR.cos(), WIND_SPEED * WIND_DIR.sin(), DT * -0.5, 0.0] }

/// Level04 0x2d7e90 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    let r = rvec(w);
    let old = w.m(id).position;
    let speed0 = c::len2(c::pv4(w, id, VEL));
    match w.m(id).state {
        0 => {
            c::set_pv4(w, id, HOME, old);
            restart(w, id, 120, 2400);
            let k = w.rng.randf(0.8, 1.2);
            let s = w.m(id).scale * k;
            c::set_pf(w, id, FULL, s);
            w.mm(id).state = 1;
        }
        1 => {
            let cam = w.camera.map(|x| x.to_f32());
            if 32.0 <= c::dist2(old, cam) { return; }
            if c::dec_timer_pvar_i32(w, id, TIMER) == 0 { return; }
            let r2 = rvec(w);
            {
                let m = w.mm(id);
                m.draw_dist = 0x40;
                m.update_dist = 0xff;
                m.scale = 0.0;
            }
            let t = c::set_len3(r2, WIND_SPEED / 5.0);
            c::set_pv4(w, id, VEL, wind());
            let e = c::set_len3(t, PI);
            {
                let m = w.mm(id);
                m.rotation[0] = e[0];
                m.rotation[1] = e[1];
                m.rotation[2] = e[2];
            }
            c::set_pv4(w, id, SPIN, c::set_len3(t, PI / 32.0));
            c::set_pv4(w, id, DRIFT, c::set_len3(t, WIND_SPEED * 0.5));
            restart(w, id, 720, 1200);
            w.mm(id).state = 2;
            return;
        }
        2 => {
            drift(w, id, r, old, speed0);
            if 0 < c::pi32(w, id, TIMER) { return; }
            let home = c::pv4(w, id, HOME);
            c::set_pos(w, id, home);
            restart(w, id, 120, 2400);
            w.mm(id).state = 1;
        }
        _ => return,
    }
    w.mm(id).scale = 0.0;
}

/// `CollLine_Fix(a, b, 6, 0, 0)`: the hit point and normal.
fn line(w: &World, a: c::V, b: c::V) -> Option<([f32; 3], [f32; 3])> { w.coll_line(pv(a), pv(b), 6, None).map(|h| (h.point, h.normal)) }

fn reflect(v: c::V, n: [f32; 3]) -> c::V { sv::reflect(pv(v), pv([n[0], n[1], n[2], 0.0])).map(Pf::to_f32) }

/// State 2's flight and scale (module table).
fn drift(w: &mut World, id: MobyId, r: c::V, old: c::V, speed0: f32) {
    c::dec_timer_pvar_i32(w, id, TIMER);
    let r = c::set_len3(r, WIND_SPEED / 10.0);
    let mut a = c::add(c::pv4(w, id, DRIFT), r);
    let probe = c::add(c::set_len3(a, 5.0), w.m(id).position);
    a = c::set_len3(a, WIND_SPEED * 0.5);
    if let Some((_, n)) = line(w, old, probe) {
        if w.rng.randi(8) == 0 {
            a = reflect(a, n);
            a[2] += WIND_SPEED / 10.0;
        }
    }
    c::set_pv4(w, id, DRIFT, a);
    let mut v = c::add(c::pv4(w, id, VEL), a);
    v = c::add(c::scale(wind(), 0.4), c::scale(v, 0.6));
    let mut p = c::add(w.m(id).position, v);
    if let Some((hit, n)) = line(w, old, p) {
        p = [hit[0], hit[1], hit[2], p[3]];
        v = c::scale(reflect(v, n), 0.25);
        p = c::add(p, v);
        v[2] += WIND_SPEED / 5.0;
    }
    c::set_pos(w, id, p);
    if WIND_SPEED < speed0 { v = c::set_len3(v, WIND_SPEED); }
    let g = DT * -0.5;
    v[2] = if g < v[2] { v[2] + g / 10.0 } else { g };
    c::set_pv4(w, id, VEL, v);
    let s = c::pv4(w, id, SPIN);
    {
        let m = w.mm(id);
        for (r, d) in m.rotation.iter_mut().zip(&s[..3]) { *r = c::add_rot(*r, *d); }
    }
    let (period, timer, full) = (c::pi32(w, id, PERIOD), c::pi32(w, id, TIMER), c::pf(w, id, FULL));
    let scale = if period - w.ticks(10) < timer {
        let k = sv::fl(w.svc.timing.scale(Pf::f(10.0)));
        full * (timer - period) as f32 / k
    } else if timer < w.ticks(100) {
        w.m(id).scale * 0.99
    } else {
        full
    };
    w.mm(id).scale = scale;
}
