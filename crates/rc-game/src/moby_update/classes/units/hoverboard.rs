//! **The Hoverboard, class 439** (level05 `0x2f87a8`, census U174; level16 `0x2c7788` is the same code) and the moby side
//! of the board's hero code (`crate::hero::hoverboard`): the snapshot the hero reads ([`world`]) and its stores into
//! other mobys and the game state ([`apply`], at the start of the next moby loop, before any class reads them). Read
//! from the level05 decomp and disassembly. Native `f32`.
//!
//! **Pvars** (0x60): +0x00 the last position, +0x10 the home (where it waits), +0x20 the race line (path), +0x24 the
//! respawn path, +0x28 the race host, +0x2c.. the racers, +0x40 their count, +0x44 / +0x48 / +0x4c the groups of the
//! hoops 1139, the pads 1140 and the weapons 470, +0x50 no home (the board stays where it is left).
//!
//! ## Coverage (`0x2f87a8`)
//! | address | what | port |
//! |---|---|---|
//! | state 0 | Ratchet within 5.5 (3-D), the tick counter ≥ 4, outside groups 0x15 / 0x16: 0x13fbbc = the board, `SetState(0x6b, 1)`; home = its position (unless +0x50); → 1, update distance 0xff | [`update`] (`HeroFields::board`, `HeroCall::SetState`) |
//! | state 1 | no +0x50, global flag 0, move record 30 never shown (0x141938), a time trial finished (gp−0x7558), Ratchet more than ticks(1200) in his group (0x13f4ec), help record 0x5f never shown → `Help_Request(0x138f, 0x5f)`; Ratchet off the board (group ≠ 0x16): update distance 0x10, back home (unless +0x50), → 0 | [`update`] |
//! | tail | d = position − last; last = position; +0xbc ≠ 2 and \|d\| < 2: at joint list (tick & 1) a `rand_angle` (its spread is computed and dropped: the velocity is d with w 8), +0xbc = 0: `PartType04Spawn(joint, d, 0x4f0090f0, 0x4f0000f0, rand_range(ticks(10), ticks(13)), 15, 15, additive)`; else (boosting) `PartType04Spawn(joint, 0.92·d, 0x5032f0d2, 0x4f0000f0, rand_range(ticks(15), ticks(18)), 30, 20, additive)` and `PartType21Spawn(15000, joint, d, 0x4f0090f0, 0x4f0000f0, rand_range(ticks(8), ticks(17)), 1)`; +0xbc = 0 | [`update`] |
//!
//! ## The hero code's stores ([`apply`])
//! | store | where | port |
//! |---|---|---|
//! | the pickups' cooldowns +0xbc, the board's +0xbc (1 boosting, 2 crashed), the host's +0xbc (1 the finish) and +0x30 | `0x253b48`, `0x244a70`, `0x254608` | [`apply`] |
//! | the pads' glow (+0x34 \|= 0x10, +0x90) | `0x253b48` | [`apply`] |
//! | the board's sequence for Ratchet's (`MobyAnimBlend`) | `0x2478f8` | [`apply`] |
//! | `PlayClassSound(1, 0, board)` (a pickup) | `0x253b48` | [`apply`] |
//! | skill points 7 / 9 (jingle, banner) | `0x255208`, `0x254608` | [`apply`] (`story::award_skill_point`) |
//! | the best time / score records 0x15ee58 / 0x15ee68 (+4 on level 16) and +1000 bolts for each new one; 0x15ee38 / 0x15ee3c + 1 | `0x254608` | [`apply`] ([`Records`]) |
//! | move record 30 bumped | `0x254058` | [`apply`] |
//! | the race HUD's slots 5 / 7 (lap / place, time) | `0x24cee8` | [`apply`] (`Services::hud`) |
//! | the boost meter slot 0x10, `update_resource_counter` | `0x24cee8`, `0x254608` | logged (G-LVL-007) |

use crate::hero::hoverboard::{BoardCmd, BoardWorld, Member};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::creature::{self as c, fx};
use crate::moby_update::services::{HeroCall, Services, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x2f_87a8;
pub const CLASSES: [i16; 1] = [439];
/// Joint lists 0 / 1 (the thrusters' points).
pub const JOINTS: [i16; 1] = [439];

/// The race records (0x15ee58 / 0x15ee5c best times in ticks, 0x15ee68 / 0x15ee6c best scores, 0x15ee38 / 0x15ee3c the
/// finished races; index 0 level 5, 1 level 16).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Records {
    pub best_time: [i32; 2],
    pub best_score: [i32; 2],
    pub finished: [i32; 2],
}

/// The board's moby-side state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Globals {
    /// The hero code's stores of the last hero update, applied by the next moby loop.
    pub cmds: Vec<BoardCmd>,
    pub records: Records,
}

fn pv_i32(pv: &[u8], o: usize) -> i32 { pv.get(o..o + 4).map_or(-1, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])) }
fn pv_f32(pv: &[u8], o: usize) -> f32 { pv.get(o..o + 4).map_or(0.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])) }

/// The board's view of the world for the hero update ([`BoardWorld`]): the board's pvars, its two paths, the racers'
/// positions, the pickup groups and the game-state words the race reads.
pub fn world(svc: &Services, table: &MobyTable, board: MobyId) -> Option<BoardWorld> {
    let m = table.mobys.get(board)?;
    let pv: [i32; 0x18] = std::array::from_fn(|k| pv_i32(&m.pvars, k * 4));
    let path = |i: i32| -> Vec<[f32; 4]> {
        usize::try_from(i).ok().and_then(|i| svc.splines.get(i)).map(|p| p.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
    };
    let count = pv[0x40 / 4].clamp(0, 5) as usize;
    let racers = (0..count).map(|k| usize::try_from(pv[0x2c / 4 + k]).ok().and_then(|i| table.mobys.get(i)).map(|r| r.position)).collect();
    let group = |g: i32| -> Option<Vec<Member>> {
        let g = usize::try_from(g).ok()?;
        let ids = svc.groups.lists.get(g).and_then(|l| l.clone()).unwrap_or_default();
        Some(
            ids.into_iter()
                .filter_map(|i| {
                    let id = i as MobyId;
                    let x = table.mobys.get(id)?;
                    let o = if x.pvars.len() >= 4 { pv_i32(&x.pvars, 0) } else { 0 };
                    Some(Member { id, o_class: x.o_class, pos: x.position, rows: [x.rows[0], x.rows[1], x.rows[2]], bc: x.cmd, value: o })
                })
                .collect(),
        )
    };
    let game = &svc.interact.game;
    let mut skill = [false; 32];
    for (k, s) in skill.iter_mut().enumerate() { *s = game.skill_points.get(k).is_some_and(|&b| b != 0); }
    Some(BoardWorld {
        board,
        o_class: m.o_class,
        pv,
        race: path(pv[0x20 / 4]),
        spawn: path(pv[0x24 / 4]),
        racers,
        aim: std::array::from_fn(|k| usize::try_from(pv[0x2c / 4 + k]).ok().and_then(|i| table.mobys.get(i).map(|r| (i, r.position)))),
        hoops: group(pv[0x44 / 4]),
        pads: group(pv[0x48 / 4]),
        weapons: group(pv[0x4c / 4]),
        flag0: game.flags.first().is_some_and(|&f| f != 0),
        skill,
        pal: svc.timing.timer_scale != crate::ps2v::Pf::ONE,
    })
}

/// The hero code's stores of the last hero update ([`BoardCmd`]), in order.
pub fn apply(w: &mut World) {
    for cmd in std::mem::take(&mut w.svc.board.cmds) {
        match cmd {
            BoardCmd::Bc { id, v } => {
                if id < w.table.mobys.len() { w.mm(id).cmd = v; }
            }
            BoardCmd::Byte30 { id, v } => {
                if id < w.table.mobys.len() { w.mm(id).update_dist = v; }
            }
            BoardCmd::Glow { id, rgba } => {
                if id < w.table.mobys.len() {
                    let m = w.mm(id);
                    m.mode |= 0x10;
                    m.glow = rgba;
                }
            }
            BoardCmd::Anim { id, seq, frame, ticks } => {
                if id >= w.table.mobys.len() { continue; }
                match seq {
                    Some(s) => { w.anim_blend(id, s, frame, ticks); }
                    None => {
                        if w.m(id).anim.seq_b != 0 { w.anim_blend(id, 0, 0, ticks); }
                    }
                }
            }
            BoardCmd::Sound { id, index } => {
                if id < w.table.mobys.len() { w.play_sound(index, 0, id); }
            }
            BoardCmd::Skill(k) => { story::award_skill_point(w, k); }
            BoardCmd::Records { k, ticks, score } => {
                let r = &mut w.svc.board.records;
                let mut bolts = 0;
                if r.best_time[k] == 0 || ticks < r.best_time[k] {
                    r.best_time[k] = ticks;
                    bolts += 1000;
                }
                if r.best_score[k] < score {
                    r.best_score[k] = score;
                    bolts += 1000;
                }
                if bolts != 0 { story::add_bolts(w, bolts); }
            }
            BoardCmd::Finished(k) => w.svc.board.records.finished[k] += 1,
            BoardCmd::MoveBump(rec) => {
                if rec < w.svc.help.records.moves.len() { super::hints::bump_move(w, rec); }
            }
            BoardCmd::RaceHud => {
                use crate::hud::{Element, Request};
                w.svc.hud.queue(Request::new(5, 0xffff, Element::RaceLap, 0));
                w.svc.hud.queue(Request::new(7, 0xffff, Element::RaceTime, 0));
            }
            BoardCmd::Meter(on) => {
                let r = crate::hud::Request::race_meter(w.ticks(0x11));
                if on { w.svc.hud.queue(r) } else { w.svc.hud.set_flags(r, 0) }
            }
            BoardCmd::Hud(what) => w.svc.unported(what),
            BoardCmd::Help { msg, rec } => { w.svc.help.request(msg, rec); }
        }
    }
}

/// Level05 `0x2f87a8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let pos = w.m(id).position;
    match w.m(id).state {
        1 => {
            let home_fixed = c::pi32(w, id, 0x50) != 0;
            let game_flag0 = story::flag(w, 0) != 0;
            let h = &w.svc.help.records;
            let shown = h.moves.get(30).map_or(0, |r| r.count) != 0 || h.help_count(0x5f) != 0;
            if !home_fixed && game_flag0 && !shown && w.hero.board.trial_done && w.ticks(0x4b0) < w.hero.f4ec {
                w.svc.help.request(0x138f, 0x5f);
            }
            if w.hero.group != crate::hero::hoverboard::GROUP {
                w.mm(id).update_dist = 0x10;
                if !home_fixed {
                    let home = [c::pf(w, id, 0x10), c::pf(w, id, 0x14), c::pf(w, id, 0x18), c::pf(w, id, 0x1c)];
                    w.mm(id).position = home;
                }
                w.mm(id).state = 0;
            }
        }
        0 => {
            let h = super::hero_pos(w);
            let d = ((pos[0] - h[0]).powi(2) + (pos[1] - h[1]).powi(2) + (pos[2] - h[2]).powi(2)).sqrt();
            let g = w.hero.group;
            if d < 5.5 && 4 <= w.counter && g != 0x15 && g != 0x16 {
                {
                    let f = w.hero_fields_mut();
                    f.board = Some(id);
                    f.call(HeroCall::SetState { id: crate::hero::hoverboard::id::RIDE, play: true });
                }
                if c::pi32(w, id, 0x50) == 0 {
                    for (k, &v) in pos.iter().enumerate() { c::set_pf(w, id, 0x10 + 4 * k, v); }
                }
                let m = w.mm(id);
                m.state = 1;
                m.update_dist = 0xff;
            }
        }
        _ => {}
    }
    // The thrusters.
    let pos = w.m(id).position;
    let last = [0, 1, 2].map(|k| pv_f32(&w.m(id).pvars, 4 * k));
    let d = [pos[0] - last[0], pos[1] - last[1], pos[2] - last[2]];
    for (k, &v) in pos.iter().enumerate() { c::set_pf(w, id, 4 * k, v); }
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if w.m(id).cmd != 2 && l < 2.0 {
        let at = w.joint_point(id, (w.counter & 1) as usize);
        // The spread (rows 2 / 1 at 0.25·dt·cos / sin of a random angle, plus d) is overwritten by d before the spawn:
        // only its draw remains.
        w.rng.rand_angle();
        let v = [d[0], d[1], d[2], 8.0];
        let (c1, c2) = (0x4f00_90f0u32, 0x4f00_00f0u32);
        if w.m(id).cmd == 0 {
            let (a, b) = (w.ticks(10), w.ticks(13));
            let life = w.rng.rand_range(a, b);
            fx::part04(w, &crate::particles::type04::Spawn { pos: at, vel: v, c1, c2, life, base: 15, growth: 15, additive: true });
        } else {
            let s = 0.92f32;
            let v2 = [d[0] * s, d[1] * s, d[2] * s, v[3]];
            let (a, b) = (w.ticks(15), w.ticks(18));
            let life = w.rng.rand_range(a, b);
            fx::part04(w, &crate::particles::type04::Spawn { pos: at, vel: v2, c1: 0x5032_f0d2, c2, life, base: 0x1e, growth: 0x14, additive: true });
            let (a, b) = (w.ticks(8), w.ticks(0x11));
            let life = w.rng.rand_range(a, b);
            fx::part21(w, 15000.0, at, v, c1, c2, life, 1);
        }
    }
    w.mm(id).cmd = 0;
}
