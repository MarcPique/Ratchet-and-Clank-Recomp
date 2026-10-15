//! Visibomb range limiters, class 832: `RcRangeLimiterUpdate` level01 0x302648, the same code on levels 1–15 and 18
//! (one instance each). A hidden controller that keeps the Visibomb's guided missile (class 172, `0xac`) inside the
//! level's allowed airspace while Ratchet steers it (hero state `0x1d`): outside every allowed area a counter runs;
//! while it runs the screen shows static (draw callback `0x302438` on list 2, the texture-effect quad grid whose
//! alpha grows with the counter over 90 ticks) and past 90 ticks the missile's flight is ended (`0x2cb788`, the
//! Visibomb's own code). Read from the level01 decomp (0x302648, the draw callback 0x302438).
//!
//! **Pvar block**: 20 area records of 4 s32 words {path (−1: none), f32 floor, f32 ceiling, cuboid (−1: none)}
//! (0x140 bytes), then P[0x50] (+0x140) the out-of-range counter.
//!
//! **Update** (update distance 0xff every tick): unless Ratchet is in state 0x1d and a class-172 moby exists, the
//! counter is 0. Otherwise the counter is incremented and the records are walked in order: the missile (its
//! position) is inside a record when (it has a path, floor < z < ceiling and the point is in the path polygon) or
//! it is in the record's cuboid; the first record that holds it zeroes the counter. Then, with the counter ≥ 1:
//! the static overlay is registered (when the missile view's flag 0x15f30c = gp−0x78f4 is set:
//! `RegisterDrawCallback2(0x302438, m)`) and, past `ticks(90)`, the missile's flight ends
//! ([`super::visibomb::end_flight`], `0x2cb788`).
//!
//! **The static** ([`draw_callback`], `0x302438`, list 2; read from the disassembly): over the whole frame (0x13e500 ×
//! 0x13e504) a grid of 32×32 quads from (−16, −16) of effect texture 0x1e, each sampling a random 32×32 window
//! (`randi(32)` for u, then v: the game's stream, at the frame render), colour 0x7f7fff with alpha
//! `trunc(counter·127.5 / ticks(90))`. The quads go to [`super::visibomb::Globals::static_quads`] for the engine's
//! static layer.

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pvar as p, World};

/// The update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x302648;
/// Classes that run [`update`].
pub const CLASSES: [i16; 1] = [832];

/// The Visibomb missile's class (`0xac`).
pub const MISSILE_CLASS: i16 = 172;
/// Ratchet's state while steering it (`0x1413d4 == 0x1d`).
pub const STEERING_STATE: i32 = 0x1d;
/// Area records.
pub const RECORDS: usize = 20;
/// The counter's pvar offset (P[0x50]).
pub const COUNTER: usize = 0x140;
/// Ticks out of range before the flight ends (`ticks(0x5a)`).
pub const LIMIT: i32 = 90;

/// The first live class-172 moby (the game walks its moby list at `0x15ffe4` for it).
fn missile(w: &World) -> Option<MobyId> { w.table.mobys.iter().position(|m| m.o_class == MISSILE_CLASS && m.state < 0x80) }

/// Whether `q` is in one of the records (module doc).
pub fn in_range(w: &World, pv: &[u8], q: [f32; 3]) -> bool {
    (0..RECORDS).any(|k| {
        let r = 0x10 * k;
        let path = p::i32(pv, r);
        if path != -1 && p::ff(pv, r + 4) < q[2] && q[2] < p::ff(pv, r + 8) && w.in_path(q, path) { return true; }
        let c = p::i32(pv, r + 0xc);
        c != -1 && w.in_cuboid(q, c)
    })
}

/// `RcRangeLimiterUpdate` 0x302648 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < COUNTER + 4 { return; }
    w.mm(id).update_dist = 0xff;
    let m = if w.hero.state == STEERING_STATE { missile(w) } else { None };
    let Some(m) = m else {
        p::set_i32(&mut w.mm(id).pvars, COUNTER, 0);
        return;
    };
    let q = { let x = w.m(m).position; [x[0], x[1], x[2]] };
    let mut n = p::i32(&w.m(id).pvars, COUNTER).wrapping_add(1);
    if in_range(w, &w.m(id).pvars, q) { n = 0; }
    p::set_i32(&mut w.mm(id).pvars, COUNTER, n);
    if n < 1 { return; }
    if w.svc.visibomb.view.overlay { w.svc.draw_callbacks.register2(super::draw_callbacks::Callback::RangeStatic, id); }
    if n <= w.ticks(LIMIT) { return; }
    super::visibomb::end_flight(w, m);
}

/// The static's quad size and texture (`GetEffectTex(0x1e)`).
pub const STATIC_FX: usize = 0x1e;
pub const STATIC_QUAD: i32 = 32;

/// `0x302438` (module docs): the state part (the `rand` draws) and the quads for the renderer.
pub fn draw_callback(w: &mut World, id: MobyId) {
    use crate::menus::screen_static::{StaticDraw, StaticTex};
    let n = p::i32(&w.m(id).pvars, COUNTER);
    let alpha = ((n as f32 * 127.5) / w.ticks(LIMIT) as f32) as i32;
    let rgba = ((alpha as u32) << 24).wrapping_add(0x00ff_7f7f);
    let (cols, rows) = ((crate::hud::SCREEN_W >> 5) + 1, (crate::hud::SCREEN_H >> 5) + 1);
    let mut quads = Vec::with_capacity((cols * rows) as usize);
    for i in 0..cols {
        for j in 0..rows {
            let u = w.rng.randi(0x20);
            let v = w.rng.randi(0x20);
            let q = STATIC_QUAD;
            quads.push(StaticDraw { tex: StaticTex::Fx(STATIC_FX), x: i * q - q / 2, y: j * q - q / 2, w: q, h: q, u, v, tw: q, th: q, rgba, additive: false, pass: 2 });
        }
    }
    w.svc.visibomb.static_quads = quads;
    w.svc.visibomb.static_at = Some(w.counter);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    #[test]
    fn idle_without_the_missile() {
        let mut m = Moby { state: 1, pvars: vec![0xff; COUNTER + 4], ..Moby::default() };
        p::set_i32(&mut m.pvars, COUNTER, 7);
        let mut t = MobyTable::new(vec![m], 4);
        let mut hero = crate::hero::Hero::new();
        hero.state = STEERING_STATE;
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        assert_eq!((p::i32(&w.m(0).pvars, COUNTER), w.m(0).update_dist), (0, 0xff));
    }

    /// `0x302438` with the counter at 45: a 17 × 14 grid of 32-pixel quads from (−16, −16), colour 0x7f7fff with alpha
    /// `trunc(45·127.5/90)` = 63, each sampling `(randi(32), randi(32))` (u first) of effect texture 0x1e.
    #[test]
    fn static_grid() {
        let mut m = Moby { state: 1, pvars: vec![0; COUNTER + 4], ..Moby::default() };
        p::set_i32(&mut m.pvars, COUNTER, 45);
        let mut t = MobyTable::new(vec![m], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        rng.srand(7);
        let mut want = rng;
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 5);
        draw_callback(&mut w, 0);
        let q = &w.svc.visibomb.static_quads;
        assert_eq!((q.len(), w.svc.visibomb.static_at), (17 * 14, Some(5)));
        let (u, v) = (want.randi(32), want.randi(32));
        assert_eq!((q[0].x, q[0].y, q[0].w, q[0].u, q[0].v, q[0].rgba), (-16, -16, 32, u, v, 0x3fff_7f7f));
        assert_eq!((q[1].x, q[1].y, q[14].x, q[14].y), (-16, 16, 16, -16), "rows inside columns");
        for _ in 1..q.len() { want.randi(32); want.randi(32); }
        assert_eq!(w.rng.state, want.state, "two draws a quad");
    }
}
