//! **Rilgar's breakable posts, class 846** (level05 `0x30f5c8`, 3 placed; census U201; the name is descriptive [L]).
//! A hit (mask 0xb30000) bursts it: a top piece (0x3d4) knocked back off its facing, and six side pieces (0x3d5) round
//! it, each thrown back with a puff of twenty sparkles (type 53: ten white-violet, ten white); the pieces then tumble,
//! bounce and fade as loose pieces (`units::loose_piece`, the same code on level 05: `0x3186f8`). The burst is kept
//! for the death reload (its command byte, a visit record), and a post found burst is deleted. Read from the level05
//! decomp and disassembly.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x30f5c8` | no hit: +0xbc ≠ 0 → `DeleteMoby` | [`update`] |
//! | | a hit: `CreateMoby(0x3d4)`: update / draw distance 0x40, drawn, the post's light words, position and Euler; thrown (`0x318648`) along −row 0 at `randf(5, 12)·dt`, life `ticks(60)`, its radius +0x0c = 0x200; `PlayClassSound(0, 0)` on it | [`update`] ([`throw`]) |
//! | | six times, a += 60°: o = (0, 1.4·sin a, 1.4·cos a)·rows (the post's), ten pairs of type-53 sparkles at `rand_vec(1.5·dt, 2·dt)`·5 + o + position, `s = randf(0.4, 0.5)`: (0.2s, s, 0, life `ticks(25)`, 0x7f207f7f, rotation 0, spin ±1 by `randi(2)`) and (0.14s, 0.7s, 0, 0x7f7f7f7f, rotation 0x20, the other spin), velocity the vector; `CreateMoby(0x3d5)` set up as the top piece, thrown along −row 0 at `randf(0, 24)·dt`, life `trunc(scale(randf(60, 120)))`, its radius 0x200, its x rotation a, `MobyBuildMatrix` | [`update`] |
//! | | +0xbc = 1; `0x2af5e8(+0xbc, 1, m, 1, 0x1baaa0)` (the visit record); `DeleteMoby` | [`update`] (`visit::record_field`) |
//! | `0x318648(piece, post, vel, offset, life)` | state 1; +0x30 = vel, +0x10 = offset, timer = `ticks(life)`; spin x / y / z = `randf(−240°·dt, 240°·dt)` | [`throw`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{pf, pv, World};

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x30_f5c8;
pub const CLASSES: [i16; 1] = [846];
pub const TOP: i16 = 0x3d4;
pub const SIDE: i16 = 0x3d5;
pub const HIT_MASK: u32 = 0xb3_0000;
const SIXTY: f32 = f32::from_bits(0x3f86_0a92);
const SPIN: f32 = 4.188_790_3;

/// `0x318648(piece, post, vel, offset, life)` (module doc).
pub fn throw(w: &mut World, piece: MobyId, vel: c::V, offset: c::V, life: i32) {
    if w.m(piece).pvars.len() < 0x40 { w.mm(piece).pvars.resize(0x40, 0); }
    w.mm(piece).state = 1;
    c::set_pv4(w, piece, 0x30, vel);
    c::set_pv4(w, piece, 0x10, offset);
    let t = w.ticks(life);
    c::set_pi32(w, piece, 0, t);
    for k in 0..3 {
        let s = w.rng.randf(-(DT * SPIN), DT * SPIN);
        c::set_pf(w, piece, 0x20 + 4 * k, s);
    }
}

/// A piece set up from the post (module doc).
fn piece(w: &mut World, id: MobyId, class: i16) -> Option<MobyId> {
    let p = w.create_moby(class)?;
    let (light, ambient, pos, rot) = { let m = w.m(id); (m.light, m.ambient, m.position, m.rotation) };
    let m = w.mm(p);
    m.update_dist = 0x40;
    m.light = light;
    m.ambient = ambient;
    m.visible = 1;
    m.draw_dist = 0x40;
    m.position = pos;
    m.rotation = rot;
    Some(p)
}

/// Level05 `0x30f5c8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.get_hit(id, HIT_MASK, false).is_none() {
        if w.m(id).cmd != 0 { w.delete_moby(id); }
        return;
    }
    let row0 = w.m(id).rows[0];
    if let Some(top) = piece(w, id, TOP) {
        let s = w.rng.randf(5.0, 12.0);
        let t60 = w.ticks(60);
        throw(w, top, c::scale(row0, -(s * DT)), [0.0; 4], t60);
        c::set_pi32(w, top, 0xc, 0x200);
        w.play_sound(0, 0, top);
    }
    let (pos, rows) = (w.m(id).position, w.m(id).rows);
    let mut a = 0.0f32;
    for _ in 0..6 {
        a = c::add_rot(a, SIXTY);
        let (sa, ca) = (a.sin() * 1.4, a.cos() * 1.4);
        let o: c::V = std::array::from_fn(|k| sa * rows[1][k] + ca * rows[2][k]);
        if let Some(side) = w.create_moby(SIDE) {
            for _ in 0..10 {
                let v = w.rng.rand_vec(DT * 1.5, DT + DT);
                let v4: c::V = [v[0], v[1], v[2], 0.0];
                let p = c::add(c::add(c::scale(v4, 5.0), o), pos);
                let n = w.rng.randi(2);
                let spin: i8 = if n == 0 { -1 } else { n as i8 };
                let s = w.rng.randf(0.4, 0.5);
                let life = w.ticks(25);
                w.part53(pf(s * 0.2), pf(s), pf(0.0), pv(p), life, 0x7f20_7f7f, 0, spin, pv(v4));
                let life = w.ticks(25);
                w.part53(pf(s * 0.14), pf(s * 0.7), pf(0.0), pv(p), life, 0x7f7f_7f7f, 1, -spin, pv(v4));
            }
            let (light, ambient, rot) = { let m = w.m(id); (m.light, m.ambient, m.rotation) };
            {
                let m = w.mm(side);
                m.visible = 1;
                m.light = light;
                m.ambient = ambient;
                m.draw_dist = 0x40;
                m.update_dist = 0x40;
                m.position = pos;
                m.rotation = rot;
            }
            let sp = w.rng.randf(0.0, 24.0);
            let l = w.rng.randf(60.0, 120.0);
            let life = w.svc.timing.scale(crate::ps2v::Pf::f(l)).to_f32() as i32;
            throw(w, side, c::scale(row0, -(sp * DT)), [0.0; 4], life);
            c::set_pi32(w, side, 0xc, 0x200);
            w.mm(side).rotation[0] = a;
            w.build_matrix(side);
        }
    }
    w.mm(id).cmd = 1;
    crate::moby_update::visit::record_field(w, id, id, 0xbc);
    w.delete_moby(id);
}
