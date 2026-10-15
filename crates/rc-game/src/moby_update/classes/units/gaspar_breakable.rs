//! **Gaspar's staged breakable rocks, classes 276, 1298, 1299** (level09 `0x2edc30`, census U302; 5 placed: #166–#170,
//! class 276), **their chunks 320 / 321** (`0x2ef9b0`) and **the last stage 1300** (its update `0x309828` is an empty
//! function).
//!
//! **The rock**: deleted when its mission (+0xb0, 0xff none) is done. A hit (`MobyGetHitMessage(m, 0x40000, 0)`) once
//! its cooldown (+0xbc, `ticks(10)`) has run out: the next stage takes its place (`0x2edb60`: 276 → 1298 → 1299 →
//! 1300, the same position, rotation and mode, the cooldown, Ratchet's light; 1300 mirrored on `rand() & 1`; from
//! 1299 the bolts first, 4 up: `SetDeathBits(m, 0, −1)`), sound 0, and from a point 1.25 up (jittered by 0.4) toward
//! the camera: 20 chunks' worth of spray (an offset in the rock's frame within (±0.5, ±2, ±4 on 276 / ±2), pushed
//! along the hit by `(|3 − d|/3)²·8` and out at `(|3 − d|/3)·randf(6, 10)·dt`; a chunk 320 / 321 (alternating) of
//! size `randf(1, 2)` one time in four, a death explosion of `randf(1, 1.6)` every third) and a beam explosion (3, 2,
//! 1, 1, light 15; 10 streaks, 3 sparks, 16 puffs, 1 debris, shake). The old stage is then deleted.
//!
//! **A chunk**: tumbling (z by `randf(±9.08)·dt`, y by `randf(±2π)·dt`), falling at 12.74·dt², deleted out of [2, 1021]
//! or beyond 64 (xy) of the camera. Its timer (`ticks(rand_range(90, 110))`) running: a hit along its motion
//! (`CollLine` flags 2) on surface 1 slows it (0.4, spins 0.25); on anything else it bounces (reflected, `·randf(0.1,
//! 0.3)`, scale 0.8, spins reversed at random, 0.05 off the face; slower than 5·dt: the timer ends). Run out: eight
//! dust puffs (type 27) and deleted.
//!
//! **The ring rocks 1285–1288** (`0x3082d8`; 3 placed: #809–#811, class 1285): the same staging (1285 → 1286 → 1287 →
//! 1288, `0x308220`: also the scale and pvar word 0 copied, no mirror), the spray from the rock's centre in a ring
//! (40, or 30 under frame load 0.85: around its y axis at `randf(0.3, 2.6)` with a jitter of (±0.4, ±0.6, ±0.4),
//! pushed by `(|6 − d|/6)²·8` along the hit and out at `(|6 − d|/6)·randf(5, 8)·dt`; chunks of `randf(1, 1.3)` only from
//! the last stage; death explosions `randf(0.3, 0.6)` every third), the same beam explosion. The last stage broken, or
//! its mission done: the story flag 0x13d3cb + pvar word 0 (−1 none) set and deleted; placed: that flag cleared.
//!
//! Read from the level09 decomp and disassembly (the explosion's arguments). Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2edc30` | the rock (module doc) | [`update`] |
//! | `0x2edb60(m, class)` | the next stage | `next_stage` |
//! | `0x2ef868(size, m, p, v, kind)` | a chunk | `chunk` |
//! | `0x2ef9b0` | a chunk's update | [`chunk_update`] |
//! | `0x309828` | 1300: nothing | `empty::update` |
//! | `0x3082d8` / `0x308220(m, class)` | the ring rocks / their next stage | [`ring_update`] / `next_ring` |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{self, Beam};
use crate::moby_update::creature::{self as c, add, add_rot, len3, set_len3, sub, V, DT, DT2};
use crate::moby_update::services::{fast_dec_timer_u8, pv, reflect, World};
use crate::moby_update::story;
use std::f32::consts::TAU;

pub const REFERENCE_LEVEL: u32 = 9;
pub const UPDATE_FN: u32 = 0x2e_dc30;
pub const CHUNK_FN: u32 = 0x2e_f9b0;
pub const LAST_FN: u32 = 0x30_9828;
pub const CLASSES: [i16; 3] = [276, 1298, 1299];
pub const CHUNK_CLASSES: [i16; 2] = [320, 321];
pub const LAST_CLASSES: [i16; 1] = [1300];
pub const RING_FN: u32 = 0x30_82d8;
pub const RING_CLASSES: [i16; 4] = [1285, 1286, 1287, 1288];

const RING_LAST: i16 = 0x508;
const RING_FLAG: u32 = 0x13_d3cb;

const FIRST: i16 = 0x114;
const LAST: i16 = 0x514;
const CHUNK: i16 = 0x140;
const BLAST: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: 3.0, flash2: 2.0, flash_dist: 1.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 0x10, debris: 1, sound: -1, shake: true };

mod cpo {
    pub const VEL: usize = 0x00;
    pub const SPIN_Z: usize = 0x10;
    pub const SPIN_Y: usize = 0x14;
    pub const OWNER: usize = 0x18;
    pub const TIMER: usize = 0x1c;
    pub const LEN: usize = 0x20;
}

fn hero_light(w: &World) -> Option<(u32, [u8; 4])> { w.hero_moby.map(|h| (w.m(h).light, w.m(h).ambient)) }

/// `0x2edb60(m, class)`: the next stage in `m`'s place.
fn next_stage(w: &mut World, id: MobyId, class: i16) -> Option<MobyId> {
    let n = w.create_moby(class)?;
    let (p, r, mode) = (w.m(id).position, w.m(id).rotation, w.m(id).mode);
    let t = w.ticks(10) as u8;
    {
        let m = w.mm(n);
        m.draw_dist = 0xff;
        m.update_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.position = p;
        m.rotation = r;
        m.cmd = t;
    }
    w.build_matrix(n);
    if let Some((l, a)) = hero_light(w) {
        w.mm(n).light = l;
        w.mm(n).ambient = a;
    }
    w.mm(n).mode = mode;
    if class == LAST && w.rng.rand() & 1 != 0 { w.mm(n).mode |= 0x8000; }
    Some(n)
}

/// `0x2ef868(size, m, p, v, kind)`: a chunk.
fn chunk(w: &mut World, size: f32, owner: MobyId, p: V, v: V, kind: i16) {
    let Some(m) = w.create_moby(CHUNK + kind) else { return };
    story::pvars(w, m, cpo::LEN);
    c::set_pi32(w, m, cpo::OWNER, owner as i32 + 1);
    let (l, a) = (w.m(owner).light, w.m(owner).ambient);
    let (x, y, z) = (w.rng.rand_angle(), w.rng.rand_angle(), w.rng.rand_angle());
    {
        let mo = w.mm(m);
        mo.draw_dist = 0x7f;
        mo.visible = 1;
        mo.update_dist = 0x7f;
        mo.light = l;
        mo.ambient = a;
        mo.rotation = [x, y, z, mo.rotation[3]];
        mo.position = p;
    }
    c::set_pv4(w, m, cpo::VEL, v);
    let s = super::class_scale(w, CHUNK + kind) * size;
    w.mm(m).scale = s;
    let k = DT * f32::from_bits(0x4111_361e);
    let sz = w.rng.randf(-k, k);
    c::set_pf(w, m, cpo::SPIN_Z, sz);
    let k = DT * TAU;
    let sy = w.rng.randf(-k, k);
    c::set_pf(w, m, cpo::SPIN_Y, sy);
    let r = w.rng.rand_range(0x5a, 0x6e);
    let t = w.ticks(r);
    c::set_pi32(w, m, cpo::TIMER, t);
    w.build_matrix(m);
}

/// Level09 `0x2edc30` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let mission = w.m(id).mission;
    if mission != 0xff && story::mission_done(w, mission as i32) {
        w.delete_moby(id);
        return;
    }
    let hit = w.get_hit(id, 0x4_0000, false);
    if w.m(id).state == 0 { w.mm(id).state = 1; }
    let mut t = w.m(id).cmd;
    let ready = fast_dec_timer_u8(&mut t) != 0;
    w.mm(id).cmd = t;
    let (true, Some(hit)) = (ready, hit) else { return };
    let class = w.m(id).o_class;
    let mut done = false;
    if class < LAST {
        let next = match class {
            0x512 => 0x513,
            0x513 => {
                let p = w.m(id).position;
                w.mm(id).position[2] += 4.0;
                crate::moby_update::classes::crate_::set_death_bits(w, id, 0, -1);
                w.mm(id).position = p;
                LAST
            }
            _ => 0x512,
        };
        done = next_stage(w, id, next).is_some();
    }
    let t = w.ticks(10) as u8;
    w.mm(id).cmd = t;
    let p = c::pos(w, id);
    let a = [p[0], p[1], p[2] + 1.25, p[3]];
    let mut b = a;
    fx::jitter(w, 0.4, &mut b);
    w.play_sound(0, 0, id);
    w.mm(id).hit_slot = 0xff;
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let e = set_len3(sub([cam[0], cam[1], cam[2], 0.0], b), 2.0);
    let f = if class == FIRST { 4.0 } else { 2.0 };
    let dir = hit.dir.map(|x| f32::from_bits(x.0));
    let rows = w.m(id).rows;
    for i in 0..20u32 {
        let x = w.rng.randf(-0.5, 0.5);
        let y = w.rng.randf(-2.0, 2.0);
        let z = w.rng.randf(-f, f);
        let o: V = std::array::from_fn(|l| rows[0][l] * x + rows[1][l] * y + rows[2][l] * z);
        let mut q = add(o, a);
        let mut d = sub(q, b);
        let k = (3.0 - len3(d)).abs() / 3.0;
        d = add(d, set_len3(dir, k * k * 8.0));
        let s = w.rng.randf(6.0, 10.0);
        d = set_len3(d, k * s * DT);
        q = add(q, e);
        if class == LAST || w.rng.randi(4) == 0 {
            let size = w.rng.randf(1.0, 2.0);
            chunk(w, size, id, q, d, (i & 1) as i16);
        }
        if i % 3 == 0 {
            let size = w.rng.randf(1.0, f32::from_bits(0x3fcc_cccd));
            fx::death_explosion(w, size, 0.0, Some(id), q, -1);
        }
    }
    b = add(b, e);
    fx::beam_explosion(w, &BLAST, Some(id), b);
    if class == LAST || done { w.delete_moby(id); }
}

/// Level09 `0x2ef9b0`: a chunk (module doc).
pub fn chunk_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, cpo::LEN);
    let old = c::pos(w, id);
    let v = c::pv4(w, id, cpo::VEL);
    let p = add(old, v);
    c::set_pos(w, id, p);
    let inside = |x: f32| (2.0..=1021.0).contains(&x);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let near = ((p[0] - cam[0]).powi(2) + (p[1] - cam[1]).powi(2)).sqrt() <= 64.0;
    if !(inside(p[0]) && inside(p[1]) && inside(p[2]) && near) {
        w.delete_moby(id);
        return;
    }
    let z = add_rot(w.m(id).rotation[2], c::pf(w, id, cpo::SPIN_Z));
    w.mm(id).rotation[2] = z;
    let y = add_rot(w.m(id).rotation[1], c::pf(w, id, cpo::SPIN_Y));
    w.mm(id).rotation[1] = y;
    let mut v = v;
    v[2] -= DT2 * f32::from_bits(0x414b_d70a);
    c::set_pv4(w, id, cpo::VEL, v);
    if c::dec_timer_pvar_i32(w, id, cpo::TIMER) == 0 {
        let Some(h) = w.coll_line(pv(old), pv(p), 2, Some(id)) else { return };
        if h.surface_id() == 1 {
            c::set_pv4(w, id, cpo::VEL, v.map(|x| x * f32::from_bits(0x3ecc_cccd)));
            let (sz, sy) = (c::pf(w, id, cpo::SPIN_Z), c::pf(w, id, cpo::SPIN_Y));
            c::set_pf(w, id, cpo::SPIN_Z, sz * 0.25);
            c::set_pf(w, id, cpo::SPIN_Y, sy * 0.25);
            return;
        }
        w.mm(id).scale *= f32::from_bits(0x3f4c_cccd);
        let n = [h.normal[0], h.normal[1], h.normal[2], 0.0];
        let r = reflect(pv(v), pv(n)).map(|x| f32::from_bits(x.0));
        let k = w.rng.randf(f32::from_bits(0x3e99_999a), f32::from_bits(0x3dcc_cccd));
        let v = r.map(|x| x * k);
        c::set_pv4(w, id, cpo::VEL, v);
        if len3(v) < DT * 5.0 { c::set_pi32(w, id, cpo::TIMER, 0); }
        let sz = w.rng.randf(0.0, -c::pf(w, id, cpo::SPIN_Z));
        c::set_pf(w, id, cpo::SPIN_Z, sz);
        let sy = w.rng.randf(0.0, -c::pf(w, id, cpo::SPIN_Y));
        c::set_pf(w, id, cpo::SPIN_Y, sy);
        let off = set_len3(n, f32::from_bits(0x3d4c_cccd));
        let at = [h.point[0] + off[0], h.point[1] + off[1], h.point[2] + off[2], p[3]];
        c::set_pos(w, id, at);
        return;
    }
    for _ in 0..8 {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let o = [x, y, z, 0.0];
        let q = add(set_len3(o, len3(o) * 0.5), o);
        let s = w.rng.randf(DT * 3.0, DT * 6.0);
        let q = set_len3(q, s);
        let (a, b) = (w.ticks(10), w.ticks(0xf));
        let life = w.rng.rand_range(a, b);
        fx::part27(w, 100_000.0, p, q, 0x7f2f_4f6f, life);
    }
    w.delete_moby(id);
}

fn ring_flag(w: &mut World, id: MobyId, v: u8) {
    let k = c::pi32(w, id, 0);
    if k != -1 {
        let i = story::flag_index(RING_FLAG) as i32 + k;
        if let Ok(i) = usize::try_from(i) { story::set_flag(w, i, v); }
    }
}

/// `0x308220(m, class)`: the ring rock's next stage.
fn next_ring(w: &mut World, id: MobyId, class: i16) -> Option<MobyId> {
    let n = w.create_moby(class)?;
    let (p, r, mode, sc) = (w.m(id).position, w.m(id).rotation, w.m(id).mode, w.m(id).scale);
    let t = w.ticks(10) as u8;
    {
        let m = w.mm(n);
        m.draw_dist = 0xff;
        m.update_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.position = p;
        m.rotation = r;
        m.cmd = t;
    }
    w.build_matrix(n);
    if let Some((l, a)) = hero_light(w) {
        w.mm(n).light = l;
        w.mm(n).ambient = a;
    }
    w.mm(n).scale = sc;
    w.mm(n).mode = mode;
    story::pvars(w, n, 4);
    let k = c::pi32(w, id, 0);
    c::set_pi32(w, n, 0, k);
    Some(n)
}

/// Level09 `0x3082d8`: the ring rocks (module doc).
pub fn ring_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 4);
    if w.m(id).state == 0 {
        ring_flag(w, id, 0);
        w.mm(id).state = 1;
    }
    let mission = w.m(id).mission;
    if mission != 0xff && story::mission_done(w, mission as i32) {
        ring_flag(w, id, 1);
        w.delete_moby(id);
        return;
    }
    let hit = w.get_hit(id, 0x4_0000, false);
    let mut t = w.m(id).cmd;
    let ready = fast_dec_timer_u8(&mut t) != 0;
    w.mm(id).cmd = t;
    let (true, Some(hit)) = (ready, hit) else { return };
    let class = w.m(id).o_class;
    let mut done = false;
    if class < RING_LAST { done = next_ring(w, id, class + 1).is_some(); }
    let t = w.ticks(10) as u8;
    w.mm(id).cmd = t;
    let a = c::pos(w, id);
    w.play_sound(0, 0, id);
    w.mm(id).hit_slot = 0xff;
    let load = [w.svc.frame_load[0], w.svc.frame_load[1]].map(|x| f32::from_bits(x.0));
    let n = if 0.85 < load[0] || 0.85 < load[1] { 30 } else { 40 };
    let dir = hit.dir.map(|x| f32::from_bits(x.0));
    let rows = w.m(id).rows;
    let mut ang = -std::f32::consts::PI;
    for i in 0..n as u32 {
        let jx = w.rng.randf(-0.4, 0.4);
        let jy = w.rng.randf(-0.6, 0.6);
        let jz = w.rng.randf(-0.4, 0.4);
        let r = w.rng.randf(f32::from_bits(0x3e99_999a), f32::from_bits(0x4026_6666));
        let (x, y, z) = (jx + r * ang.cos(), jy, jz + r * ang.sin());
        let o: V = std::array::from_fn(|l| rows[0][l] * x + rows[1][l] * y + rows[2][l] * z);
        let q = add(o, a);
        let mut d = sub(q, a);
        let k = (6.0 - len3(d)).abs() / 6.0;
        d = add(d, set_len3(dir, k * k * 8.0));
        let s = w.rng.randf(5.0, 8.0);
        d = set_len3(d, k * s * DT);
        if class == RING_LAST {
            let size = w.rng.randf(1.0, f32::from_bits(0x3fa6_6666));
            chunk(w, size, id, q, d, (i & 1) as i16);
        }
        if i % 3 == 0 {
            let size = w.rng.randf(f32::from_bits(0x3e99_999a), f32::from_bits(0x3f19_999a));
            fx::death_explosion(w, size, 0.0, Some(id), q, -1);
        }
        ang += std::f32::consts::TAU / n as f32;
    }
    fx::beam_explosion(w, &BLAST, Some(id), a);
    if class == RING_LAST {
        ring_flag(w, id, 1);
        w.delete_moby(id);
        return;
    }
    if done { w.delete_moby(id); }
}
