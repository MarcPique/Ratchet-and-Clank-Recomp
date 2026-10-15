//! The alarms, class 408: level15 0x2cb4c8, the same code on 17 (census U480; 48 created instances). An alarm is a
//! small animated state machine: set off (its pvar +0x08 raised by the family call [`set_off`], 0x2cbac0, which the
//! guards 44 make: 0x2979d8, [`super::quartu_guard`]), it opens (sequences 3 → 4 / 5 → 6), and while its timer runs it
//! releases one alarm drone 77 ([`super::quartu_drone::release`]) every +0x10 ticks when Ratchet is within 16 units
//! (xy) and in its line of sight, then closes (7 → 8 → 1). Kind 1 alarms pulse their light red while armed and
//! darker red while running. Every active alarm keeps the level's alarm word set, which the drones read; the first
//! alarm of each tick (0x2cbc30) eases the level's alarm fade toward 1 while the word was set (toward 0 on Quartu
//! with Ratchet at y ≤ 240, or in hero state 0x65, which also stops the alarms), keeps the alarm loop (class sound
//! 5) playing while the fade is above 0, and registers the full-screen red tint (0x2cbd88). Read from the level15
//! decomp (0x2cb4c8, 0x2cbac0, 0x2cbc30, 0x2cbd88); the level words are gp−0x514c.. on 15 and gp−0x5278.. on 17 with
//! the same bytes. Native `f32`.
//!
//! **Pvar block** (s32): +0x00 the kind (1: lit, starts closed; 2: starts open), +0x04 the drones' moby group,
//! +0x08 active, +0x0c the release timer, +0x10 the release period, +0x14 f32 the light phase, +0x18 the alarm timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2cb4c8 state 0 | level words: last tick = 0, alarm word = 0, fade = 0; kind 2 → state 6, seq 4; else state 3, seq 2 (`MobyAnimBlend`, 0 ticks) | [`update`] |
//! | state 1 | +0x90 = `FastTweenColor((sin φ + 1)/2, 0x801414bc, 0x801414dc)`, φ += τ·dt; active and not in a cutscene (0x15f5c4 ≠ 2): kind 1 → state 6, seq 4 (ticks(30)); else state 5 | [`update`] |
//! | state 2 | sequence wrapped (+0x70 & 2) → state 3, seq 2 | [`update`] |
//! | state 3 | active, not in a cutscene → state 4, seq 3 | [`update`] |
//! | state 4 | wrapped: inactive or cutscene → state 10, seq 8; kind ≠ 1 → seq 8, state 5; else state 7, seq 5 | [`update`] |
//! | state 5 | wrapped → seq 0; +0x90 tween to 0x8004048c, φ += 2τ·dt; `FastDecTimer(+0x18)` or cutscene → [`set_off`]`(m, 0)`, +0x08 = 0, seq 8, state 10 | [`update`] |
//! | state 6 / 7 | active → state 7, seq 5; wrapped → state 8, seq 6 | [`update`] |
//! | state 8 | `VecDistance2(m, Ratchet) < 16`, no cutscene, `FastDecTimer(+0x0c)`: `CollLine_Fix(m + 1 z, body point 0x13f420, 2)` clear and a drone released (0x2a2868) → +0x0c = +0x10; `FastDecTimer(+0x18)` or cutscene → [`set_off`]`(m, 0)`, +0x08 = 0, state 9, seq 7 | [`update`] (`World::line`, `quartu_drone::release`) |
//! | state 9 / 10 | wrapped: kind 2 → state 6, seq 4, else state 10, seq 8; 10 → state 2, seq 1 | [`update`] |
//! | tail | +0x08 ≠ 0 → alarm word = 1 (not after the early exits of states 1 / 3 / 6); first alarm of the tick (last tick ≠ 0x15f5cc) → last tick = counter, 0x2cbc30, alarm word = 0 | [`update`] |
//! | 0x2cbac0 | (m, t): t = −1 → fade = 0, t = 0; no group: +0x08 = (t ≠ 0), +0x18 = ticks(t); else every 408 of the moby's group | [`set_off`] |
//! | 0x2cbc30 | hero state 0x65 → `Approach(0, 2·dt, fade)` (0x270728), [`set_off`]`(m, 0)`; else alarm word 0 or (level 15 (0x15ed84) and Ratchet y ≤ 240) → toward 0, else toward 1 | [`fade_tick`] |
//! | | fade 0: the loop's voice alive (`SoundIsAlive` 0x2a12f0) → `release_voice_slot` (0x2a1348), owner 0, slot −1; playing flag 0 | [`fade_tick`] |
//! | | fade > 0: playing flag 1; the voice not alive → `PlayClassSound(5, 0x11, m)` (0x2a1618), owner m; `RegisterDrawCallback2(0x2cbd88, m)` (list 2) | [`fade_tick`]; the tint's draw is **not ported** (G-REN-030: no full-screen tint from a class callback) |
//! | 0x2cbd88 | a = (fade + 0.1·sin((counter mod ticks(40)) / ticks(40) · 6.28318)) max 0; `emit_rgba_draw_packet(r, g, b, trunc(0x20·a))` of 0x200000ff (red) | [`tint`] (the colour; drawing it: G-REN-030) |
//! | | no particle, save flag, bolt; the drones are the only other mobys touched | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{add_rot, dec_timer_pvar_i32, pf, pi32, set_pf, set_pi32, turn, DT};
use crate::moby_update::services::{pv as pvec, Services, World};

/// The update in the level15 class table.
pub const UPDATE_FN: u32 = 0x2c_b4c8;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 1] = [408];

/// Level words (level15 addresses; `Services::units`).
pub mod lw {
    /// The tick the fade last ran.
    pub const LAST: u32 = 0x16_1ab4;
    /// The alarm word ([`super::super::quartu_drone::ALARM_WORD`]).
    pub const ALARM: u32 = 0x16_1ab8;
    /// f32: the alarm fade 0..1.
    pub const FADE: u32 = 0x16_1abc;
    /// The loop is playing.
    pub const PLAYING: u32 = 0x16_1ac0;
    /// The loop's voice slot + 1 (the data word is −1: stored + 1 so the port's 0 default reads −1).
    pub const SLOT: u32 = 0x16_1ac4;
    /// The loop's owner moby + 1 (0: none).
    pub const OWNER: u32 = 0x16_1ac8;
}

/// 0x161acc..0x161ad4: the tint colour (GS RGBA, alpha 0x20), its pulse amplitude and period (ticks).
pub const TINT_RGBA: u32 = 0x2000_00ff;
pub const TINT_PULSE: f32 = 0.1;
pub const TINT_PERIOD: i32 = 40;
/// Hero state 0x65 stops the alarms.
pub const HERO_STATE_OFF: i32 = 0x65;
/// The light colours (GS RGBA).
pub const LIGHT_ARMED: (u32, u32) = (0x8014_14bc, 0x8014_14dc);
pub const LIGHT_RUNNING: (u32, u32) = (0x8014_14dc, 0x8004_048c);

/// Pvar offsets.
pub mod pv {
    pub const KIND: usize = 0x00;
    pub const GROUP: usize = 0x04;
    pub const ACTIVE: usize = 0x08;
    pub const RELEASE: usize = 0x0c;
    pub const PERIOD: usize = 0x10;
    pub const PHASE: usize = 0x14;
    pub const TIMER: usize = 0x18;
}

fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }

/// `if (m+0x53 != seq) MobyAnimBlend(m, seq, 0, ticks(n))`.
fn seq(w: &mut World, id: MobyId, s: u8, n: i32) {
    if w.m(id).anim.seq_b != s {
        let t = w.ticks(n);
        w.anim_blend(id, s, 0, t);
    }
}

fn light(w: &mut World, id: MobyId, c: (u32, u32), rate: f32) {
    let ph = pf(w, id, pv::PHASE);
    w.mm(id).glow = crate::hud::tween_color((ph.sin() + 1.0) * 0.5, c.0, c.1);
    set_pf(w, id, pv::PHASE, add_rot(ph, DT * rate));
}

/// Level15 0x2cb4c8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x1c { return; }
    let cut = w.svc.game_mode == 2;
    let kind = pi32(w, id, pv::KIND);
    let active = |w: &World| pi32(w, id, pv::ACTIVE) != 0;
    let mut tail = true;
    match w.m(id).state {
        0 => {
            w.svc.units.set_word(lw::LAST, 0);
            w.svc.units.set_word(lw::ALARM, 0);
            w.svc.units.set_word(lw::FADE, 0);
            if kind == 2 { w.mm(id).state = 6; seq(w, id, 4, 0); } else { w.mm(id).state = 3; seq(w, id, 2, 0); }
        }
        1 => {
            light(w, id, LIGHT_ARMED, f32::from_bits(0x40c9_0fdb));
            if !active(w) { tail = false; } else if !cut {
                if kind == 1 { w.mm(id).state = 6; seq(w, id, 4, 0x1e); } else { w.mm(id).state = 5; }
            }
        }
        2 if wrapped(w, id) => { w.mm(id).state = 3; seq(w, id, 2, 10); }
        3 => {
            if !active(w) { tail = false; } else if !cut { w.mm(id).state = 4; seq(w, id, 3, 10); }
        }
        4 if wrapped(w, id) => {
            if !active(w) || cut { seq(w, id, 8, 10); w.mm(id).state = 10; } else if kind != 1 { seq(w, id, 8, 10); w.mm(id).state = 5; } else { w.mm(id).state = 7; seq(w, id, 5, 10); }
        }
        5 => {
            if wrapped(w, id) && w.m(id).anim.seq_b != 0 { let t = w.ticks(10); w.anim_blend(id, 0, 0, t); }
            light(w, id, LIGHT_RUNNING, f32::from_bits(0x4149_0fdb));
            if dec_timer_pvar_i32(w, id, pv::TIMER) != 0 || cut {
                set_off(w, id, 0);
                set_pi32(w, id, pv::ACTIVE, 0);
                seq(w, id, 8, 10);
                w.mm(id).state = 10;
            }
        }
        6 => if !active(w) { tail = false; } else { w.mm(id).state = 7; seq(w, id, 5, 10); },
        7 if wrapped(w, id) => { w.mm(id).state = 8; seq(w, id, 6, 10); }
        8 => {
            let h = super::hero_pos(w);
            let p = w.m(id).position;
            let d = ((p[0] - h[0]).powi(2) + (p[1] - h[1]).powi(2)).sqrt();
            if d < 16.0 && !cut && dec_timer_pvar_i32(w, id, pv::RELEASE) != 0 {
                let from = [p[0], p[1], p[2] + 1.0, p[3]];
                let body = w.hero.body_point;
                if w.line(pvec(from), body, 2, None).is_none() && super::quartu_drone::release(w, pi32(w, id, pv::GROUP), p) {
                    let n = pi32(w, id, pv::PERIOD);
                    set_pi32(w, id, pv::RELEASE, n);
                }
            }
            if dec_timer_pvar_i32(w, id, pv::TIMER) != 0 || cut {
                set_off(w, id, 0);
                set_pi32(w, id, pv::ACTIVE, 0);
                w.mm(id).state = 9;
                seq(w, id, 7, 10);
            }
        }
        9 if wrapped(w, id) => {
            if kind == 2 { w.mm(id).state = 6; seq(w, id, 4, 10); } else { w.mm(id).state = 10; seq(w, id, 8, 10); }
        }
        10 if wrapped(w, id) => { w.mm(id).state = 2; seq(w, id, 1, 10); }
        _ => {}
    }
    if tail && active(w) { w.svc.units.set_word(lw::ALARM, 1); }
    let tick = w.counter as u32;
    if w.svc.units.word(lw::LAST) != tick {
        w.svc.units.set_word(lw::LAST, tick);
        fade_tick(w, id);
        w.svc.units.set_word(lw::ALARM, 0);
    }
}

/// Level15 0x2cbac0(m, t): sets the alarms of `id`'s moby group (or `id` alone) active for `t` ticks (0: stops
/// them; −1: stops them and clears the fade). The guards 44 call it with 180 / 30 / −1 (unported).
pub fn set_off(w: &mut World, id: MobyId, mut t: i32) {
    if t == -1 {
        w.svc.units.set_word(lw::FADE, 0);
        t = 0;
    }
    let arm = |w: &mut World, a: MobyId| {
        set_pi32(w, a, pv::ACTIVE, (t != 0) as i32);
        let n = w.ticks(t);
        set_pi32(w, a, pv::TIMER, n);
    };
    let g = w.m(id).group;
    if g == -1 { arm(w, id); return; }
    let Some(list) = w.svc.groups.lists.get(g as u8 as usize).and_then(|l| l.clone()) else { return };
    for e in list {
        let a = (e & 0x7fff) as usize;
        if w.table.mobys.get(a).is_some_and(|m| m.o_class == CLASSES[0] && m.pvars.len() >= 0x1c) { arm(w, a); }
    }
}

/// Level15 `0x2cbba0(m)`: a guard 44 at the alarm (its poke): closed (1) → opening (2, sequence 1 over `ticks(10)`);
/// open (3) and not set off → its timer `ticks(120)`.
pub fn poke(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x1c { return; }
    if w.m(id).state == 1 {
        w.mm(id).state = 2;
        if w.m(id).anim.seq_b != 1 {
            let t = w.ticks(10);
            w.anim_blend(id, 1, 0, t);
        }
    }
    if w.m(id).state == 3 && pi32(w, id, pv::ACTIVE) == 0 {
        let t = w.ticks(0x78);
        set_pi32(w, id, pv::TIMER, t);
    }
}

/// Level15 0x2cbc30: the level's alarm fade, loop and tint, once per tick (module doc).
pub fn fade_tick(w: &mut World, id: MobyId) {
    let mut fade = f32::from_bits(w.svc.units.word(lw::FADE));
    if w.hero.state == HERO_STATE_OFF {
        turn::approach(0.0, DT + DT, &mut fade);
        w.svc.units.set_word(lw::FADE, fade.to_bits());
        set_off(w, id, 0);
    } else {
        let quiet = w.svc.units.word(lw::ALARM) == 0 || (w.svc.level == 15 && super::hero_pos(w)[1] <= 240.0);
        turn::approach(if quiet { 0.0 } else { 1.0 }, DT + DT, &mut fade);
        w.svc.units.set_word(lw::FADE, fade.to_bits());
    }
    let slot = w.svc.units.word(lw::SLOT) as i32 - 1;
    let owner = (w.svc.units.word(lw::OWNER) as usize).checked_sub(1);
    let alive = owner.is_some_and(|o| w.sound_alive(slot, o));
    if fade == 0.0 {
        if let (true, Some(o)) = (alive, owner) {
            w.release_sound(slot, o);
            w.svc.units.set_word(lw::OWNER, 0);
            w.svc.units.set_word(lw::SLOT, 0);
        }
        w.svc.units.set_word(lw::PLAYING, 0);
    } else {
        w.svc.units.set_word(lw::PLAYING, 1);
        if !alive {
            let s = w.play_sound(5, 0x11, id);
            w.svc.units.set_word(lw::SLOT, (s + 1) as u32);
            w.svc.units.set_word(lw::OWNER, id as u32 + 1);
        }
    }
}

/// Level15 0x2cbd88: the full-screen tint's GS RGBA for tick `counter` (None while the fade is 0: no callback).
/// Its draw is G-REN-030.
pub fn tint(svc: &Services, counter: u64) -> Option<u32> {
    let fade = f32::from_bits(svc.units.word(lw::FADE));
    if fade == 0.0 { return None; }
    let n = svc.ticks(TINT_PERIOD).max(1);
    let s = (((counter as i64).rem_euclid(n as i64)) as f32 / n as f32 * f32::from_bits(0x40c9_0fd0)).sin();
    let a = (fade + TINT_PULSE * s).max(0.0);
    let alpha = ((TINT_RGBA >> 24) as f32 * a) as u32;
    Some(TINT_RGBA & 0xff_ffff | alpha << 24)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    fn alarm(kind: i32) -> Moby {
        let mut m = Moby { o_class: 408, group: -1, pvars: vec![0; 0x1c], ..Moby::default() };
        crate::moby_update::services::pvar::set_i32(&mut m.pvars, pv::KIND, kind);
        m
    }

    #[test]
    fn set_off_arms_the_group_and_the_fade_follows_the_alarm_word() {
        let mut t = MobyTable::new(vec![alarm(1)], 4);
        let mut far = crate::hero::Hero::new();
        far.pos = crate::hero::physics::v4(0.0, 300.0, 0.0);
        let mut near = crate::hero::Hero::new();
        near.pos = crate::hero::physics::v4(0.0, 100.0, 0.0);
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = Services::new();
        svc.level = 15;
        let mut w = World::new(&mut t, &far, &mut rng, &classes, &mut svc, 1);
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 3, "kind 1 starts closed");
        set_off(&mut w, 0, 180);
        assert_eq!((pi32(&w, 0, pv::ACTIVE), pi32(&w, 0, pv::TIMER)), (1, 180));
        // Active: state 4; the alarm word is set before this tick's fade runs, so the fade rises at once.
        w.counter = 2;
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 4);
        let fade = f32::from_bits(w.svc.units.word(lw::FADE));
        assert!((fade - 2.0 * DT).abs() < 1e-6, "{fade}");
        assert_eq!(w.svc.units.word(lw::PLAYING), 1);
        assert_eq!(w.svc.units.word(lw::OWNER), 1, "the loop's owner");
        assert!(w.svc.sounds.iter().any(|s| (s.index, s.flags, s.moby) == (5, 0x11, 0)), "the alarm loop");
        assert_eq!(w.svc.units.word(lw::ALARM), 0, "cleared after the fade ran");
        let c = tint(w.svc, 2).unwrap();
        assert_eq!(c & 0xff_ffff, 0xff);
        // Ratchet at y ≤ 240 on Quartu: the fade falls back, the loop stops, no tint.
        w.hero = &near;
        for k in 0..10 { w.counter = 3 + k; update(&mut w, 0); }
        assert_eq!(f32::from_bits(w.svc.units.word(lw::FADE)), 0.0);
        assert_eq!(w.svc.units.word(lw::PLAYING), 0);
        assert!(tint(w.svc, 20).is_none());
        set_off(&mut w, 0, -1);
        assert_eq!(pi32(&w, 0, pv::ACTIVE), 0);
    }
}
