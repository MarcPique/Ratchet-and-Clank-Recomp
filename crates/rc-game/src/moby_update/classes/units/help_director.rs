//! The Novalis help-hint director, class 1341 (census unit U82): level01 `HelpHintDirectorUpdate` 0x30acb8, read
//! from the disassembly (the decompile drops six blocks). One instance, no geometry; each tick it tests Ratchet
//! against its cuboids, the play time and the help / move records, and asks the help system for its hints
//! ([`crate::help::Help::request`]). Its own code: no other level has a copy (docs/plan/level_scripting.md §3).
//!
//! **Pvars** (P, bytes): +0x14 i32 (zeroed at init), +0x18 i32 a timer (`FastDecTimer` every tick; nothing sets it),
//! +0x20 / +0x24 the swim cuboids, +0x28 i32 (0; the swim hints need it 0), +0x2c the swim tick counter, +0x30 the
//! crank cuboid, +0x34 the crank-try counter, +0x38 the look-around cuboid, +0x3c the map cuboid.
//!
//! "Idle" = the box idle and no request pending (0x179890 = 0, 0x1798b4 = −1: [`crate::help::Help::idle`]); "group
//! ok" = Ratchet's movement group 0x1413dc < 2 or 9. Help / move records: `H[n]` = 0x141968 + 8n, `M[n]` = 0x141848 +
//! 8n. State 0: +0x14 = 0, update distance 0xff, +0x28 = 0 → 1. State 1, in this order:
//! 1. `0x15f5cc < ticks(400)`, `M[12]` used and `M[11]` not, `ticks(play) − 600·M[12].time < ticks(0x44c)` and
//!    `H[0x51]` never shown (mask ≥ 0) → 20007 (rec 0x51).
//! 2. The same window with `M[29]` (no second record) and `H[0x72]` → 20008 (rec 0x72).
//! 3. Game mode 0, by the planet bits 0x13dd40 (K = Kerwan [3], A = Aridia [2]), idle: K ∧ ¬A ∧ `H[4].count` = 0 →
//!    1000 (rec 4); ¬K ∧ A ∧ `H[5]` = 0 → 1001 (rec 5); K ∧ A ∧ `H[6]` = 0 ∧ `H[4]` = 0 → 1002 (rec 6).
//! 4. In cuboid +0x3c, group ok, idle, one hour (`ticks(0x34bc0)`) of play past `M[9].time` and `H[0x43]` without this
//!    level's bit → 1007 (rec 0x43: the map hint).
//! 5. In cuboid +0x38, group ok, looks 0x15f68c < 3, idle, `H[0x40].count` = 0 → 1004 (rec 0x40).
//! 6. In cuboid +0x38, group ok, state 1, the look timer 0x15f688 = −1, idle, `H[0x40]` used, `H[0x41]` not → 1005
//!    (rec 0x41).
//! 7. `H[0x42].count` < 2, in cuboid +0x30, previous state 0x1413e0 = 0x3b (the crank), state timer 0x13f4e8 = 0:
//!    for every class-280 crank of the run list within 3 whose progress (P+0) is not 1.0, +0x34 += 1, and 1006 (rec
//!    0x42) on the 3rd and 6th.
//! 8. Group 0x12 (swimming), +0x28 = 0, idle, in cuboid +0x20 or +0x24: +0x2c += 1; in +0x20 past `ticks(240)` with
//!    `M[22]` = 0 → 1008 (rec 0x44) and `M[22]` bumped; else in +0x24 past `ticks(360)` with `M[23]` = 0 → 1008 and
//!    `M[23]` bumped. Otherwise group 0x11 sets `H[0x44].count` = 0xffff (never again), any other group zeroes +0x2c.
//! 9. For every class-806 nanotech cluster of the run list within 10: idle, `H[1]`, `H[2]` and `M[13]` unused and the
//!    cluster not in its state 1: below full health (0x1415f8 < 0x15eda0) → `M[13]` bumped, then 1 (rec 1); at full
//!    health, when `FUN_00275690(255, cluster)` ≠ 0 (`FastBSphereCheck`: off screen or fully in view) → 2 (rec 2).
//!
//! Native `f32` for the distances (`VecDistance` 0x221360).

use crate::help::bump;
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::dec_timer_i32;
use crate::moby_update::services::{pvar as p, World};

pub const UPDATE_FN: u32 = 0x30_acb8;
pub const REFERENCE_LEVEL: u32 = 1;
pub const CLASSES: [i16; 1] = [1341];

/// The bolt crank (class 280, 0x118) and the nanotech cluster (806, 0x326).
pub const CRANK: i16 = 0x118;
pub const CLUSTER: i16 = 0x326;

/// Pvar offsets (module doc).
pub mod pv_ {
    pub const P14: usize = 0x14;
    pub const TIMER: usize = 0x18;
    pub const SWIM_A: usize = 0x20;
    pub const SWIM_B: usize = 0x24;
    pub const SWIM_OFF: usize = 0x28;
    pub const SWIM_TICKS: usize = 0x2c;
    pub const CRANK_CUBOID: usize = 0x30;
    pub const CRANK_TRIES: usize = 0x34;
    pub const LOOK: usize = 0x38;
    pub const MAP: usize = 0x3c;
    pub const LEN: usize = 0x40;
}
use pv_ as o;

fn pvi(w: &World, id: MobyId, off: usize) -> i32 { w.m(id).pvars.get(off..off + 4).map_or(0, |_| p::i32(&w.m(id).pvars, off)) }
fn set_pvi(w: &mut World, id: MobyId, off: usize, v: i32) {
    let pv = &mut w.mm(id).pvars;
    if pv.len() < off + 4 { pv.resize(off + 4, 0); }
    p::set_i32(pv, off, v);
}

fn in_cuboid(w: &World, id: MobyId, off: usize) -> bool {
    let q = w.hero.pos.map(|x| f32::from_bits(x.0));
    w.in_cuboid([q[0], q[1], q[2]], pvi(w, id, off))
}

fn dist(a: [f32; 4], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }

pub fn update(w: &mut World, id: MobyId) {
    let mut t = pvi(w, id, o::TIMER);
    dec_timer_i32(&mut t);
    set_pvi(w, id, o::TIMER, t);
    match w.m(id).state {
        0 => {
            set_pvi(w, id, o::P14, 0);
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 1;
            set_pvi(w, id, o::SWIM_OFF, 0);
            return;
        }
        1 => {}
        _ => return,
    }
    let counter = w.counter as i32;
    let (play, level) = (w.svc.help.play_time, w.svc.help.level);
    let tk = |n: i32| crate::hud::scale_ticks(n);
    // 1., 2.: the first 400 ticks of the level.
    {
        let r = &w.svc.help.records;
        if counter < tk(400) && r.moves[12].count != 0 && r.moves[11].count == 0 && tk(play) - 600 * (r.moves[12].time as i32) < tk(0x44c) && (r.help[0x51].mask as i32) >= 0 {
            w.svc.help.request(0x4e27, 0x51);
        }
        let r = &w.svc.help.records;
        if counter < tk(400) && r.moves[29].count != 0 && tk(play) - 600 * (r.moves[29].time as i32) < tk(0x44c) && (r.help[0x72].mask as i32) >= 0 {
            w.svc.help.request(0x4e28, 0x72);
        }
    }
    // 3. The Infobot hints by the planets unlocked.
    if w.svc.game_mode == 0 {
        let pl = |i: usize| w.svc.interact.game.planet_unlocked.get(i).is_some_and(|&b| b != 0);
        let (k, a) = (pl(3), pl(2));
        let (idle, h4, h5, h6) = (w.svc.help.idle(), w.svc.help.records.help[4].count, w.svc.help.records.help[5].count, w.svc.help.records.help[6].count);
        if k && !a && idle && h4 == 0 {
            w.svc.help.request(1000, 4);
        } else if !k && a && idle && h5 == 0 {
            w.svc.help.request(0x3e9, 5);
        } else if k && a && idle && h6 == 0 && h4 == 0 {
            w.svc.help.request(0x3ea, 6);
        }
    }
    let group = w.hero.group;
    let group_ok = (group as u32) < 2 || group == 9;
    // 4. The map hint.
    if in_cuboid(w, id, o::MAP) && group_ok && w.svc.help.idle() {
        let r = &w.svc.help.records;
        if tk(0x34bc0) < tk(play) - 600 * r.moves[9].time as i32 && r.help[0x43].mask & (1u32 << (level as u32 & 31)) == 0 {
            w.svc.help.request(0x3ef, 0x43);
        }
    }
    // 5., 6. The look-around hints.
    let looks = w.hero.help.looks;
    if in_cuboid(w, id, o::LOOK) && group_ok && looks < 3 && w.svc.help.idle() && w.svc.help.records.help[0x40].count == 0 {
        w.svc.help.request(0x3ec, 0x40);
    }
    if in_cuboid(w, id, o::LOOK) && group_ok && w.hero.state == 1 && w.hero.help.look_timer == -1 && w.svc.help.idle() {
        let r = &w.svc.help.records;
        if r.help[0x40].count != 0 && r.help[0x41].count == 0 { w.svc.help.request(0x3ed, 0x41); }
    }
    // 7. The crank hint.
    let list = crate::moby_update::scheduler::build_active_list(w.table, w.camera, &w.svc.groups).0;
    let hero = w.hero.pos.map(|x| f32::from_bits(x.0));
    let hero = [hero[0], hero[1], hero[2]];
    if w.svc.help.records.help[0x42].count < 2 && in_cuboid(w, id, o::CRANK_CUBOID) && w.hero.prev_state == 0x3b && w.hero.timer == 0 {
        for &c in &list {
            if w.m(c).o_class != CRANK || 3.0 <= dist(w.m(c).position, hero) { continue; }
            if w.m(c).pvars.len() >= 4 && p::ff(&w.m(c).pvars, 0) == 1.0 { continue; }
            let n = pvi(w, id, o::CRANK_TRIES) + 1;
            set_pvi(w, id, o::CRANK_TRIES, n);
            if n == 3 || n == 6 { w.svc.help.request(0x3ee, 0x42); }
        }
    }
    // 8. The swimming hints.
    let swim_a = in_cuboid(w, id, o::SWIM_A);
    let swim = group == 0x12 && pvi(w, id, o::SWIM_OFF) == 0 && w.svc.help.idle() && (swim_a || in_cuboid(w, id, o::SWIM_B));
    if swim {
        let n = pvi(w, id, o::SWIM_TICKS) + 1;
        set_pvi(w, id, o::SWIM_TICKS, n);
        let mut done = false;
        if swim_a && tk(0xf0) < n && w.svc.help.records.moves[22].count == 0 {
            w.svc.help.request(0x3f0, 0x44);
            bump(&mut w.svc.help.records.moves[22], level, play);
            done = true;
        }
        if !done && in_cuboid(w, id, o::SWIM_B) && tk(0x168) < n && w.svc.help.records.moves[23].count == 0 {
            w.svc.help.request(0x3f0, 0x44);
            bump(&mut w.svc.help.records.moves[23], level, play);
        }
    } else if group == 0x11 {
        w.svc.help.records.help[0x44].count = 0xffff;
    } else {
        set_pvi(w, id, o::SWIM_TICKS, 0);
    }
    // 9. The nanotech hints.
    let max_hp = w.svc.counters.max_hp;
    for &c in &list {
        if w.m(c).o_class != CLUSTER || 10.0 <= dist(w.m(c).position, hero) || !w.svc.help.idle() { continue; }
        let r = &w.svc.help.records;
        if r.help[1].count != 0 || r.help[2].count != 0 || r.moves[13].count != 0 || w.m(c).state == 1 { continue; }
        if w.hero.health < max_hp {
            bump(&mut w.svc.help.records.moves[13], level, play);
            w.svc.help.request(1, 1);
        } else {
            let sphere = w.m(c).bsphere.map(|x| x * (1.0 / 1024.0));
            if w.view.is_some_and(|v| v.check(255.0, sphere) != 0) { w.svc.help.request(2, 2); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    fn world_parts() -> (MobyTable, crate::hero::Hero, crate::rng::Rng, crate::moby_update::services::Services) {
        let mut m = Moby { o_class: 1341, ..Default::default() };
        m.pvars = vec![0; o::LEN];
        for off in [o::SWIM_A, o::SWIM_B, o::CRANK_CUBOID, o::LOOK, o::MAP] { p::set_i32(&mut m.pvars, off, -1); }
        let t = MobyTable::new(vec![m], 4);
        let mut svc = crate::moby_update::services::Services::new();
        svc.help.bx.enabled = true;
        svc.help.log_ids = std::sync::Arc::new(vec![(3, 21106), (1000, 21107), (0x3e9, 0), (0x3ea, 0), (0x4e27, 0), (1, 0), (2, 0)]);
        svc.interact.game.planet_unlocked = vec![0; 20];
        (t, crate::hero::Hero::new(), crate::rng::Rng::new(), svc)
    }

    #[test]
    fn infobot_hints_by_planet_bits_and_records() {
        let (mut t, hero, mut rng, mut svc) = world_parts();
        let classes = crate::moby_update::ClassTable::default();
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 1000);
            update(&mut w, 0);
            assert_eq!((w.m(0).state, w.m(0).update_dist), (1, 0xff));
            update(&mut w, 0);
            assert_eq!(w.svc.help.request, -1, "no planet bit: nothing");
        }
        // Kerwan unlocked, Aridia not: 1000 (rec 4), logged.
        svc.interact.game.planet_unlocked[3] = 1;
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 1001);
            update(&mut w, 0);
        }
        assert_eq!((svc.help.request, svc.help.rec), (1000, 4));
        assert_eq!(svc.help.log_pos, 2);
        // Both planets: 1002 once record 4 and 6 are unused; with record 4 shown, nothing.
        svc.help.request = -1;
        svc.interact.game.planet_unlocked[2] = 1;
        svc.help.records.help[4].count = 1;
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 1002);
            update(&mut w, 0);
        }
        assert_eq!(svc.help.request, -1);
        svc.help.records.help[4].count = 0;
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 1003);
            update(&mut w, 0);
        }
        assert_eq!((svc.help.request, svc.help.rec), (0x3ea, 6));
        // A box up: the director asks nothing (its idle tests), and outside mode 0 neither.
        svc.help.request = -1;
        svc.help.bx.state = 5;
        svc.help.records.help[6].count = 0;
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 1004);
            update(&mut w, 0);
        }
        assert_eq!(svc.help.request, -1);
    }

    #[test]
    fn early_move_hint_window() {
        let (mut t, hero, mut rng, mut svc) = world_parts();
        let classes = crate::moby_update::ClassTable::default();
        t.mobys[0].state = 1;
        svc.game_mode = 2;
        svc.help.play_time = 1000;
        svc.help.records.moves[12] = crate::game_state::HelpRec { count: 1, time: 1, mask: 0 };
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 399);
            update(&mut w, 0);
        }
        assert_eq!((svc.help.request, svc.help.rec), (0x4e27, 0x51));
        // Past 400 ticks of the level: nothing.
        svc.help.request = -1;
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 400);
            update(&mut w, 0);
        }
        assert_eq!(svc.help.request, -1);
    }

    #[test]
    fn swim_group_0x11_retires_the_swim_hint() {
        let (mut t, mut hero, mut rng, mut svc) = world_parts();
        let classes = crate::moby_update::ClassTable::default();
        t.mobys[0].state = 1;
        svc.game_mode = 2;
        hero.group = 0x11;
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 5000);
            update(&mut w, 0);
        }
        assert_eq!(svc.help.records.help[0x44].count, 0xffff);
    }
}
