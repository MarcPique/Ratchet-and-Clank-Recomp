//! The scripted hold 0x72 (docs/plan/cutscenes.md §3.4): the state the in-level cinematics put Ratchet in while
//! the script camera is up (`CameraTriggerUpdate` 0x2fb5b0 through `HeroTeleport(…, 0x72, 1)`, `GunshipUpdate`
//! 0x2f7728 through `SetState(0x72, 0)`); they end it with `SetState(0, 1)` (`FUN_002405a0` for the trigger).
//!
//! * **Entry** (`SetState` 0x23cf98 case 0x72): group 9, 0x1413fc = 1 (no control: the weapon check and the hand
//!   items refuse), camera mode 0x1415d4 = 0, the weapon put away (`0x22efd8`), then — whatever `play` says, the
//!   case `break`s into the common `SetAnim` — the idle sequence `0x226f10(0)` blended over 10 ticks.
//! * **Physics** (`0x2370b8`): the idle group's case (0, 1, 3, 4, 0x1d, 0x1e, 0x1f, 0x72, …): the drag, the edge
//!   brake, `StickTarget` — he stands and settles, gravity applies.
//! * **Transitions** (`0x242930`): no case, only the prologue (hits, death height). Nothing leaves 0x72 but the
//!   script's own `SetState`.
//!
//! **Steering the Visibomb 0x1d** (level01 `SetState` case 0x1d, `0x2370b8`'s idle case, no case in `0x242930`): the
//! missile's launch `0x2cb540` sets it (`SetState(0x1d, 1)`) and the flight's end `0x2cb788` gives him back
//! (`SetState(0, 1)`); `crate::moby_update::classes::visibomb`. The entry is 0x72's without the put-away: group 9,
//! 0x1413fc = 1, 0x1415d4 = 0, the idle sequence `0x226f10(0)` over 10 ticks; the Visibomb stays in his hand. The
//! physics and the (missing) transitions are 0x72's. Hurt (0x16), dead (0x3d) or put in 0x72 / 0x65, the missile
//! ends its own flight.
//!
//! **The scene body 99 / 100** (level01 `SetState` case 99 / 100, `0x2370b8` case 99 / 100; no case in `0x242930`):
//! the state a mode-2 scene (`SetState(100, 2)`, docs/plan/cutscenes.md §7) and the vendor (`OpenVendorMenu`:
//! `SetState(100, 1)`) put Ratchet in; they end it with `SetState(0, 1)`.
//! * **Entry**: group 0x18, 0x1413fc = 1 (no control), the weapon put away (`0x22efd8`); with `play`, the idle sequence
//!   `0x226f10(0)` on the eased curve 1 (`SetAnim(−1, …)`, 11 ticks).
//! * **Physics**: the velocity 0x13f430 zeroed (no gravity, no drag): he stays where the scene found him.
//! * **Transitions**: none. The gameplay Ratchet is never seen in this state: the scene hides him and his items
//!   (`FUN_002486c0`) and draws its own Ratchet actor; the vendor hides him too.
//!
//! **The class holds 0x1f / 0x32 / 0x78** (G-HERO-002, docs/plan/hero_states.md "Scripted control"): three more
//! SetState cases of the same shape (group 9, 0x1415d4 = 0), each set and ended by the classes that use it; level 01
//! compiles all three (`0x23cf98` cases 0x1f / 0x32 / 0x78), level 00's copies are the same code.
//! * **0x1f, held with control** (the Sonic Summoner's mouse 1818 while it runs out to him, `SetState(0x1f, 0)` +
//!   its own `SetAnim(6, 0, 0)`; the path trooper 459's arrival camera): group 9, 0x1415d4 = 0, the idle sequence
//!   `0x226f10(0)` over 10 ticks whatever `play` says; unlike 0x72 / 0x1d **0x1413fc is left clear** (SetState's
//!   prologue cleared it) and the hand item stays out. Physics: `0x2370b8`'s idle case (as 0x72). Transitions: none.
//! * **0x32, mounted** (a turret or vehicle class holds and places him: Batalia's turret 440, Gaspar's 1201, Hoven's
//!   1267, the ships 1242 / 69 / 1379): 0x1413fc = 1 (no control), group 9, 0x1415d4 = 0, 0x1413f7 = 1, frozen
//!   0x1413fd = 1 (the move pipeline skips him); with `play` the idle sequence on the eased curve 1 (`SetAnim(−1, …)`).
//!   Physics: no case (nothing changes the velocity; the frozen flag skips the move). Transitions: none. The rest of
//!   the game reads the state: the hit intake refuses hits (`super::damage`), the HUD hides health and bolts, the fog
//!   map, the pause gate and the prompts skip it (their own modules).
//! * **0x78, held by the Umbris boss** (class 1106, level07 0x314150, which then writes his position every tick):
//!   0x1413fc = 1, group 9, 0x1415d4 = 0, 0x1413f7 = 1, the velocity 0x13f430 zeroed (`0x221170`); with `play` the
//!   fall sequence 0xb on the eased curve 1. Not frozen. Physics: no case (the velocity stays 0). Transitions: none.

use super::common::blend;
use super::physics::Env;
use super::states::Ctx;
use super::{AnimCtl, Hero};
use crate::rng::Rng;

/// The scripted hold.
pub const HOLD: i32 = 0x72;

/// The scene body (99 is the same case).
pub const SCENE: i32 = 100;

/// Steering the Visibomb's missile.
pub const MISSILE: i32 = 0x1d;

/// Held by a class with control kept (the mouse's summon, the trooper's arrival camera).
pub const HELD: i32 = 0x1f;

/// Mounted: a turret or vehicle class holds and places him (frozen).
pub const MOUNTED: i32 = 0x32;

/// Held by the Umbris boss 1106.
pub const GRABBED: i32 = 0x78;

/// The fall sequence 0x78 plays.
pub const SEQ_GRABBED: u8 = 0xb;

/// `SetAnim`'s blend −1: the eased curve 1 (0x32, 0x78, the scene body).
const CURVE_1: crate::ps2v::Pf = crate::ps2v::Pf::b(0xbf80_0000);

/// SetState's entry of 0x1d, 0x1f, 0x32, 0x72, 0x78, 99 and 100. `None`: SetState's epilogue follows.
pub(super) fn entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool, _old_sub: i32) -> Option<bool> {
    match id {
        SCENE | 99 => {
            h.group = 0x18;
            h.items.f13fc = 1;
            super::weapons::put_away(h);
            if play {
                let seq = h.idle_seq();
                h.set_anim(c.anim, c.rng, CURVE_1, seq, 0);
            }
        }
        HOLD | MISSILE | HELD => {
            h.group = 9;
            // 0x1f leaves 0x1413fc as the prologue left it (clear): he keeps control of his items.
            if id != HELD { h.items.f13fc = 1; }
            h.f15d4 = 0;
            if id == HOLD { super::weapons::put_away(h); }
            let seq = h.idle_seq();
            h.set_anim(c.anim, c.rng, blend(10), seq, 0);
        }
        MOUNTED => {
            h.items.f13fc = 1;
            h.group = 9;
            h.f15d4 = 0;
            h.items.f13f7 = 1;
            h.frozen = 1;
            if play {
                let seq = h.idle_seq();
                h.set_anim(c.anim, c.rng, CURVE_1, seq, 0);
            }
        }
        GRABBED => {
            h.items.f13fc = 1;
            h.group = 9;
            h.f15d4 = 0;
            h.items.f13f7 = 1;
            h.vel = super::physics::V0;
            if play { h.set_anim(c.anim, c.rng, CURVE_1, SEQ_GRABBED, 0); }
        }
        _ => {}
    }
    None
}

/// `0x2370b8`: the idle case for 0x1d / 0x1f / 0x72; the scene body's zeroed velocity for 99 / 100; no case (nothing
/// changes) for 0x32 / 0x78.
pub(super) fn physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) -> bool {
    match h.state {
        SCENE | 99 => h.vel = super::physics::V0,
        HOLD | MISSILE | HELD => h.phys_ground(env, anim, rng),
        MOUNTED | GRABBED => {}
        _ => return false,
    }
    true
}

/// 0x242930 has no case for any of them: only the script (the class) that set the state ends it.
pub(super) fn transitions(_h: &mut Hero, _c: &mut Ctx) {}

#[cfg(test)]
mod tests {
    use super::super::testkit::*;
    use super::super::HeroTick;
    use super::*;
    use crate::pad::PadInput;

    /// `SetState(100, 2)` (a scene): group 0x18, no control, the idle sequence on the eased curve; the hero update
    /// runs (nothing unported) and keeps him still whatever the pad does; `SetState(0, 1)` gives him back.
    /// `SetState(0x1d, 1)` (the Visibomb's launch): group 9, no control, the idle sequence over 10 ticks, the hand
    /// item kept (no put-away); the pad moves nothing; `SetState(0, 1)` (the flight's end) gives him back.
    #[test]
    fn visibomb_flight_holds_ratchet_with_the_gun_out() {
        let coll = floor(100.0, 100, 106, 100, 106);
        let mut r = Runner::new([410.0, 410.0, 100.0], 0.0);
        r.run(&coll, PadInput::neutral(), 5);
        r.hero.f13f8 = 1;
        assert!(r.with_ctx(&coll, |h, c| h.set_state(c, MISSILE, true)));
        assert_eq!((r.hero.state, r.hero.group, r.hero.items.f13fc, r.hero.f15d4), (MISSILE, 9, 1, 0));
        assert_eq!(r.hero.f13f8, 1, "not put away (0x72 puts it away)");
        let at = r.hero.position();
        for _ in 0..60 {
            assert_eq!(r.tick(&coll, PadInput::neutral().stick(1.0, -1.0).press(crate::pad::button::CROSS)), HeroTick::Ran);
        }
        assert_eq!(r.hero.state, MISSILE, "no transition leaves 0x1d");
        let p = r.hero.position();
        assert!((0..3).all(|k| (p[k] - at[k]).abs() < 1e-3), "moved {at:?} → {p:?}");
        assert!(r.with_ctx(&coll, |h, c| h.set_state(c, 0, true)));
        assert_eq!(r.hero.items.f13fc, 0, "control back");
        r.run(&coll, PadInput::neutral().stick(0.0, -1.0), 30);
        assert_eq!(r.hero.state, 2, "he walks again");
    }

    /// One `SetState(id, play)` from standing: the fields each case writes (`0x23cf98` cases 0x1f / 0x32 / 0x78,
    /// level 01 = level 00) and the animation call it makes.
    fn enter(id: i32, play: bool) -> (Runner, rc_formats::collision::Collision) {
        let coll = floor(100.0, 100, 106, 100, 106);
        let mut r = Runner::new([410.0, 410.0, 100.0], 0.0);
        r.run(&coll, PadInput::neutral(), 5);
        r.hero.f13f8 = 1;
        r.hero.vel = [crate::ps2v::Pf::ONE; 4];
        r.anim.calls.clear();
        assert!(r.with_ctx(&coll, |h, c| h.set_state(c, id, play)));
        (r, coll)
    }

    /// The three class holds' entries: group 9 and 0x1415d4 = 0 for all; 0x1f keeps control (0x1413fc clear), the
    /// hand item and the velocity and plays the idle sequence over 10 ticks even without `play` (the mouse's
    /// `SetState(0x1f, 0)`); 0x32 takes control, keeps the hand item selected (0x1413f7) and freezes him; 0x78
    /// takes control, keeps the item selected and zeroes the velocity; both play only with `play`, on the eased
    /// curve 1 (idle for 0x32, the fall sequence 0xb for 0x78). None puts the weapon away.
    #[test]
    fn class_hold_entries_match_setstate() {
        let v1 = [crate::ps2v::Pf::ONE; 4];
        type Row = (i32, bool, u8, u8, u8, bool, Option<(crate::ps2v::Pf, u8)>);
        let rows: [Row; 6] = [
            // (id, play, 0x1413fc, 0x1413f7, 0x1413fd, velocity kept, SetAnim (blend, seq))
            (HELD, false, 0, 0, 0, true, Some((blend(10), 0))),
            (HELD, true, 0, 0, 0, true, Some((blend(10), 0))),
            (MOUNTED, true, 1, 1, 1, true, Some((CURVE_1, 0))),
            (MOUNTED, false, 1, 1, 1, true, None),
            (GRABBED, true, 1, 1, 0, false, Some((CURVE_1, SEQ_GRABBED))),
            (GRABBED, false, 1, 1, 0, false, None),
        ];
        for (id, play, fc, f7, frozen, vel_kept, anim) in rows {
            let (r, _) = enter(id, play);
            let h = &r.hero;
            assert_eq!((h.state, h.group, h.f15d4, h.items.f13fc, h.items.f13f7, h.frozen), (id, 9, 0, fc, f7, frozen), "{id:#x} play {play}");
            assert_eq!(h.f13f8, 1, "{id:#x}: the hand item stays out (no put-away)");
            assert_eq!(h.vel == v1, vel_kept, "{id:#x}: velocity");
            assert_eq!(r.anim.calls.last().map(|c| (c.0, c.1)), anim, "{id:#x} play {play}: SetAnim");
            assert_eq!(h.timer, 0, "the epilogue");
        }
    }

    /// 0x1f: the idle physics, no transition: the pad moves nothing (he settles where he stands) and he stays until
    /// the class's `SetState(0, 1)` gives him back; a hit is taken (the hit intake refuses only 0x32).
    #[test]
    fn held_ignores_the_pad_until_let_go() {
        let (mut r, coll) = enter(HELD, false);
        r.hero.vel = super::super::physics::V0;
        let at = r.hero.position();
        for _ in 0..60 {
            assert_eq!(r.tick(&coll, PadInput::neutral().stick(1.0, -1.0).press(crate::pad::button::CROSS)), HeroTick::Ran);
        }
        assert_eq!(r.hero.state, HELD, "no transition leaves 0x1f");
        let p = r.hero.position();
        assert!((0..3).all(|k| (p[k] - at[k]).abs() < 1e-3), "moved {at:?} → {p:?}");
        assert!(r.with_ctx(&coll, |h, c| h.set_state(c, 0, true)));
        r.run(&coll, PadInput::neutral().stick(0.0, -1.0), 30);
        assert_eq!(r.hero.state, 2, "he walks again");
    }

    /// 0x32: no physics and the frozen flag: placed 2 above the floor he hangs there whatever the pad does; the hit
    /// intake refuses the hit; `SetState(0, 1)` unfreezes him, clears 0x1413f7 (the hand item's restore request
    /// 0x14145c) and he falls to the floor.
    #[test]
    fn mounted_hangs_frozen_and_takes_no_hits() {
        let (mut r, coll) = enter(MOUNTED, true);
        r.hero.pos[2] = r.hero.pos[2] + crate::ps2v::Pf::from_i32(2);
        let at = r.hero.position();
        for t in 0..60 {
            if t == 30 {
                r.hero.damage.hit = Some(super::super::damage::HeroHit { attacker: None, flags: 1, b28: 0, damage: 1.0, w30: 0, dir: [0.0; 4] });
            }
            assert_eq!(r.tick(&coll, PadInput::neutral().stick(1.0, -1.0).press(crate::pad::button::CROSS)), HeroTick::Ran);
            r.hero.damage.hit = None;
        }
        assert_eq!(r.hero.state, MOUNTED, "no transition, no hurt state");
        assert!(r.hero.damage.events.is_empty(), "the hit intake refuses 0x32: {:?}", r.hero.damage.events);
        assert_eq!(r.hero.position(), at, "frozen");
        assert!(r.with_ctx(&coll, |h, c| h.set_state(c, 0, true)));
        assert_eq!((r.hero.frozen, r.hero.items.f13f7, r.hero.items.f13fc, r.hero.items.restore_pending), (0, 0, 0, 1));
        r.run(&coll, PadInput::neutral(), 60);
        assert!((r.hero.position()[2] - 100.0).abs() < 1e-3, "landed: {:?}", r.hero.position());
    }

    /// 0x78: the velocity zeroed and no physics: no gravity, he stays where the boss puts him (the boss writes his
    /// position every tick; here once); a hit is taken (0x16); `SetState(0, 1)` and he falls.
    #[test]
    fn grabbed_hangs_where_the_boss_puts_him() {
        let (mut r, coll) = enter(GRABBED, true);
        r.hero.pos[2] = r.hero.pos[2] + crate::ps2v::Pf::from_i32(3);
        let at = r.hero.position();
        for _ in 0..60 {
            assert_eq!(r.tick(&coll, PadInput::neutral().stick(0.0, -1.0).press(crate::pad::button::CROSS)), HeroTick::Ran);
        }
        assert_eq!(r.hero.state, GRABBED);
        let p = r.hero.position();
        assert!((0..3).all(|k| (p[k] - at[k]).abs() < 1e-4), "moved {at:?} → {p:?}");
        assert_eq!(r.hero.vel, super::super::physics::V0);
        assert!(r.with_ctx(&coll, |h, c| h.set_state(c, 0, true)));
        r.run(&coll, PadInput::neutral(), 90);
        assert!((r.hero.position()[2] - 100.0).abs() < 1e-3, "landed: {:?}", r.hero.position());
    }

    /// The hit intake takes a hit in 0x1f and 0x78 (only 0x32 is refused, with the groups 0x14 / 7).
    #[test]
    fn hits_reach_the_held_and_the_grabbed() {
        for id in [HELD, GRABBED] {
            let (mut r, coll) = enter(id, true);
            r.hero.damage.hit = Some(super::super::damage::HeroHit { attacker: None, flags: 1, b28: 0, damage: 1.0, w30: 0, dir: [0.0; 4] });
            r.tick(&coll, PadInput::neutral());
            assert!(r.hero.damage.events.contains(&super::super::damage::DamageEvent::Hit), "{id:#x}");
            assert_ne!(r.hero.state, id, "{id:#x}: the hit's state");
        }
    }

    #[test]
    fn scene_body_holds_ratchet_still() {
        let coll = floor(100.0, 100, 106, 100, 106);
        let mut r = Runner::new([410.0, 410.0, 100.0], 0.0);
        r.run(&coll, PadInput::neutral(), 5);
        assert!(r.with_ctx(&coll, |h, c| h.set_state(c, SCENE, true)));
        assert_eq!((r.hero.state, r.hero.group, r.hero.items.f13fc), (SCENE, 0x18, 1));
        assert_eq!(r.anim.calls.last().map(|c| (c.0, c.2)), Some((crate::ps2v::Pf::b(0xbf80_0000), 0)));
        let at = r.hero.position();
        for _ in 0..60 {
            assert_eq!(r.tick(&coll, PadInput::neutral().stick(0.0, -1.0).press(crate::pad::button::CROSS)), HeroTick::Ran);
        }
        assert_eq!(r.hero.state, SCENE, "no transition leaves the scene body");
        let p = r.hero.position();
        assert!((0..3).all(|k| (p[k] - at[k]).abs() < 1e-4), "moved {at:?} → {p:?}");
        assert!(r.with_ctx(&coll, |h, c| h.set_state(c, 0, true)));
        r.run(&coll, PadInput::neutral().stick(0.0, -1.0), 30);
        assert_eq!(r.hero.state, 2, "control is back");
    }
}
