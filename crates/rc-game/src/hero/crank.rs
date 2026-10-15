//! **The bolt crank, state 0x3b**: Ratchet latched onto a big bolt with the wrench, turning it (levels 1, 4 and 8,
//! the same code on each; level01 addresses). docs/plan/hero_states.md "Bolt crank".
//!
//! **Who drives what (the game's split).** The bolt's class 280 drives the state, not the hero code:
//! `BoltCrankUpdate` 0x2e0c68 (ported in [`crate::moby_update::classes::bolt_crank`]) runs in the moby loop, tests
//! Ratchet ([`latch`]), calls `SetState(0x3b, 1)`, then every tick places and turns him on the bolt's ring
//! ([`turn`]), picks his animation and finally lets go with `SetState(0, 1)` ([`release`], or the crank done). This
//! module holds the hero half of that update (the math on the hero block, native `f32`) and the state's own code:
//! * **SetState entry** (0x23cf98 case 0x3b): group 9, frozen `0x1413fd = 1` (the move pipeline and the
//!   straightening do not run: the crank's stores are his position), `0x1413f7 = 1`, `SetAnim(ticks(8), 0x44, 2)`
//!   (the latch), and the wrench (a class-0x47 hand moby) blended to its sequence 0xb, frame 2, over ticks(8) +
//!   ticks(5).
//! * **No physics case and no transition case**: after the transitions' prologue (the hit intake still takes him
//!   out of it; the crank then sees another state and lets go) nothing runs.
//!
//! **The latch** (state 1 of the class, [`latch`]): Ratchet's feet within 1.75 of the bolt in XY, on foot (body
//! 0), facing it within 90°, his feet 0.1..0.5 above the bolt's origin, the wrench (item 8) in hand with its moby,
//! and □ pressed within the last 7 ticks. (The class adds: 30 ticks since the last release, the bolt up, the crank
//! not done, and no targetable moby within 3.25 of it.)
//!
//! **Turning** ([`turn`], each tick in 0x3b): the stick (last tick's 0x141070, forward = −y) turned by the camera
//! yaw into the world; its component along his facing is the speed's target: the speed (crank pvar+0x1c, units per
//! tick) moves toward it by at most 5.5·dt² a tick, is capped at 3.7·dt (3.7 u/s), never negative, and snaps to 0
//! below 5.5·dt²/4. He steps that far along his facing, then a spring (`Spring(0, 0.015, 0.3, 7·dt)` on the gap,
//! velocity in the global gp−0x5360) pulls him back onto the ring of radius 1.05 around the bolt; his height is
//! kept. His target yaw 0x13f4d0 is the ring's tangent (the angle + 90°; − 90° with the mirrored-animation cheat
//! 0x15edb5) and his yaw turns to it at 360°/s. He drops by the crank's fall speed (pvar+0x24 += 9.8·dt²) onto
//! `GroundHeight(0.5)`. Animation: the latch 0x44 → on its wrap the hold 0x3f (blend 7); then, once no blend runs,
//! 0x3f → 0x40 (turn) as soon as he moves, 0x40 → 0x41 (fast turn) above 0.6 of the top speed, 0x41 → 0x40 below
//! 0.4 (blend 16), 0x40 → 0x3f when stopped (blends 14). The angle he travels around the bolt (unsigned) is the
//! crank's progress (the class). His motion block 0x13f430..0x13f4bf is cleared every tick.
//!
//! **Release** ([`release`]): □ again after 60 ticks in the state (or the crank done): `SetState(0, 1)`.
#![allow(clippy::neg_cmp_op_on_partial_ord)] // FPU compare semantics are spelled out on purpose.

use super::physics::ticks;
use super::states::Ctx;
use super::Hero;
use crate::moby_update::creature::{add_rot, atan, diff_rots, dist2, set_len3, sub_rot, DT, DT2, SPEED};
use crate::moby_update::services::HeroPose;
use crate::pad::button;
use crate::ps2v::Pf;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The state id (`0x1413d4`).
pub const STATE: i32 = 0x3b;
/// Ratchet's sequences: the latch (SetState), the hold, the turn, the fast turn.
pub const SEQ_LATCH: u8 = 0x44;
pub const SEQ_HOLD: u8 = 0x3f;
pub const SEQ_TURN: u8 = 0x40;
pub const SEQ_TURN_FAST: u8 = 0x41;
/// The wrench's sequence while it holds the bolt.
pub const WRENCH_SEQ: u8 = 0xb;
/// The latch range (XY) and the ring he walks around the bolt.
pub const RANGE: f32 = 1.75;
pub const RING: f32 = 1.05;
/// The top turning speed (u/s) and the speed's step (u/s²).
pub const MAX_SPEED: f32 = 3.7;
pub const ACCEL: f32 = 5.5;

/// SetState's entry of 0x3b (0x23cf98). `None`: SetState's epilogue follows.
pub(super) fn entry(h: &mut Hero, c: &mut Ctx, _id: i32, _play: bool, _old_sub: i32) -> Option<bool> {
    h.frozen = 1;
    h.items.f13f7 = 1;
    h.group = 9;
    let n = ticks(8);
    h.set_anim(c.anim, c.rng, Pf::from_i32(n), SEQ_LATCH, 2);
    // FUN_00231ac0(0x1403e0): the hand moby is the wrench → MobyAnimBlend(wrench, 0xb, 2, n + ticks(5)).
    if h.items.slot.item.as_ref().is_some_and(|m| m.o_class == super::melee::WRENCH_CLASS) {
        h.items.pending_blend = Some((WRENCH_SEQ, 2, n + ticks(5)));
    }
    None
}

/// 0x2370b8 has no case for 0x3b: nothing (the frozen flag skips the move).
pub(super) fn physics(_h: &mut Hero) -> bool { true }

/// 0x242930 has no case for 0x3b: nothing after the prologue.
pub(super) fn transitions(_h: &mut Hero, _c: &mut Ctx) {}

/// What the crank reads of Ratchet: the hero block as his last update left it and the globals of
/// [`crate::moby_update::services::LoopGlobals`] (this tick's pad, the last camera, his anim fields).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    /// 0x13f3d0: his feet.
    pub pos: [f32; 3],
    /// His moby's +0x48 (0x13f3e8 after the write-back).
    pub yaw: f32,
    /// 0x1413f4: the body (0 = Ratchet on foot).
    pub mode: u8,
    /// 0x140408: the item in hand; 0x1403e0 ≠ 0: its moby exists.
    pub hand_id: i32,
    pub hand: bool,
    /// 0x13f4e8: ticks in the state.
    pub timer: i32,
    /// 0x141070 / 0x141074: last tick's stick (d-pad fallback applied).
    pub stick: [f32; 2],
    /// 0x167258: the camera's yaw.
    pub cam_yaw: f32,
    /// His moby's +0x52 / +0x53 (sequence A / B), 0x13fde8 (bit 1: wrapped), 0x13fdec (a blend is running).
    pub seq_a: u8,
    pub seq_b: u8,
    pub anim_flags: u8,
    pub blending: bool,
    /// `PadPressedWithin(□, ticks(7))` 0x27b940.
    pub square: bool,
}

impl Seen {
    pub fn of(h: &Hero) -> Seen {
        let g = &h.loop_in;
        Seen {
            pos: super::physics::to_f32x3(h.pos),
            yaw: h.rot[2].to_f32(),
            mode: h.mode,
            hand_id: h.items.slot.id,
            hand: h.items.slot.item.is_some(),
            timer: h.timer,
            stick: [h.stick[0].to_f32(), h.stick[1].to_f32()],
            cam_yaw: g.cam_euler[2],
            seq_a: g.anim.seq_a,
            seq_b: g.anim.seq_b,
            anim_flags: g.anim.flags,
            blending: g.anim.blending(),
            square: g.pad.pressed_within(button::SQUARE, ticks(7)).is_some(),
        }
    }
}

fn v4(p: [f32; 3]) -> [f32; 4] { [p[0], p[1], p[2], 0.0] }

/// The hero half of the latch test (0x2e0d50..0x2e0ecc): in range, on foot, facing the bolt, at its height, the
/// wrench in hand, □ just pressed.
pub fn latch(s: &Seen, bolt: [f32; 3]) -> bool {
    if !(dist2(v4(bolt), v4(s.pos)) < RANGE) || s.mode != 0 { return false; }
    let to_bolt = atan(bolt[0] - s.pos[0], bolt[1] - s.pos[1]);
    if !(diff_rots(s.yaw, to_bolt) < FRAC_PI_2) { return false; }
    if !((s.pos[2] - 0.3 - bolt[2]).abs() < 0.2) { return false; }
    s.hand_id == 8 && s.hand && s.square
}

/// Letting go (0x2e11f4): □ pressed again after 60 ticks on the bolt.
pub fn release(s: &Seen) -> bool { s.square && ticks(60) < s.timer }

/// The crank's per-turn state the hero half reads and advances (crank pvar+0x1c, +0x24, the global gp−0x5360).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn {
    /// pvar+0x1c: speed along his facing, units per tick.
    pub speed: f32,
    /// pvar+0x24: the fall this tick (grows by 9.8·dt² until he stands).
    pub fall: f32,
    /// gp−0x5360: the ring spring's velocity (0 at each latch).
    pub spring_v: f32,
}

/// One turning tick's result for the crank to store.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turned {
    /// Ratchet's new position, yaw and target yaw (the crank's stores into the hero block).
    pub pose: HeroPose,
    /// `SetAnim(ticks(n), seq, 0)` on Ratchet: (seq, n).
    pub anim: Option<(u8, i32)>,
    /// His sequence B after that call (the bolt follows him once it is not the latch).
    pub seq_b: u8,
    /// His angle around the bolt before and after the step (`FastArcTan`).
    pub before: f32,
    pub after: f32,
}

/// `0x270ac0(t, step, &x, 0)`: `x` turns toward `t` by at most `step` (the difference wrapped).
fn approach_rot(t: f32, step: f32, x: f32) -> f32 { add_rot(x, sub_rot(t, x).clamp(-step, step)) }

/// One tick of turning (0x2e13b8..0x2e18e0, the hero part): speed, animation, the step and the ring, the yaw, the
/// drop. `ground(p)` is `GroundHeight(0.5, p)` (the hit z below `p` + 0.5, 0 for none). `mirror_anim`: the cheat
/// 0x15edb5 (he then faces the other way round the bolt).
pub fn turn(s: &Seen, bolt: [f32; 3], t: &mut Turn, mirror_anim: bool, ground: impl Fn([f32; 3]) -> f32) -> Turned {
    let before = atan(s.pos[0] - bolt[0], s.pos[1] - bolt[1]);

    // The stick in the camera's frame (forward = −y, left = −x), clamped to 1, turned by the camera yaw.
    let (mut fx, mut fy) = (-s.stick[1], -s.stick[0]);
    let l = (fx * fx + fy * fy).sqrt();
    if 1.0 < l {
        fx /= l;
        fy /= l;
    }
    let (cc, cs) = (s.cam_yaw.cos(), s.cam_yaw.sin());
    let world = [fx * cc - fy * cs, fx * cs + fy * cc];
    let facing = [s.yaw.cos(), s.yaw.sin()];
    let along = facing[0] * world[0] + facing[1] * world[1];

    // The speed: toward the stick's part along the facing, by ≤ 5.5·dt² a tick, capped at 3.7 u/s.
    let step = DT2 * ACCEL;
    let mut sp = t.speed + (along - t.speed).clamp(-step, step);
    // The cap, then never negative (the game's two compares).
    sp = sp.clamp(0.0, DT * MAX_SPEED);
    if sp.abs() < step * 0.25 { sp = 0.0; }
    t.speed = sp;

    // The animation (Ratchet's anim fields as the moby loop sees them, updated by a call made here).
    let (mut a, mut b, mut blending) = (s.seq_a, s.seq_b, s.blending);
    let mut anim = None;
    let mut set = |seq: u8, n: i32, a: &mut u8, b: &mut u8, bl: &mut bool| {
        anim = Some((seq, ticks(n)));
        (*a, *b, *bl) = (0xff, seq, true);
    };
    if s.anim_flags & 2 != 0 && b == SEQ_LATCH { set(SEQ_HOLD, 7, &mut a, &mut b, &mut blending); }
    if b != SEQ_LATCH && !blending {
        let top = DT * MAX_SPEED;
        if a == SEQ_TURN && sp.abs() == 0.0 {
            set(SEQ_HOLD, 14, &mut a, &mut b, &mut blending);
        } else if a == SEQ_HOLD {
            if 0.0 < sp { set(SEQ_TURN, 14, &mut a, &mut b, &mut blending); }
        } else if a == SEQ_TURN {
            if top * 0.6 < sp { set(SEQ_TURN_FAST, 14, &mut a, &mut b, &mut blending); }
        } else if a == SEQ_TURN_FAST && sp < top * 0.4 {
            set(SEQ_TURN, 16, &mut a, &mut b, &mut blending);
        }
    }

    // The step along the facing, then the spring back onto the ring; the height is kept.
    let z0 = s.pos[2];
    let mut p = [s.pos[0] + facing[0] * sp, s.pos[1] + facing[1] * sp, z0];
    let after = atan(p[0] - bolt[0], p[1] - bolt[1]);
    let ring = [bolt[0] + after.cos() * RING, bolt[1] + after.sin() * RING];
    let gap = [ring[0] - p[0], ring[1] - p[1], 0.0, 0.0];
    let mut x = -(gap[0] * gap[0] + gap[1] * gap[1]).sqrt();
    crate::moby_update::classes::flow::spring(SPEED * 0.015, SPEED * 0.3, DT * 7.0, &mut x, &mut t.spring_v);
    let pull = set_len3(gap, t.spring_v);
    p = [p[0] + pull[0], p[1] + pull[1], z0];

    // Facing: the ring's tangent, turned to at 360°/s.
    let target_yaw = add_rot(if mirror_anim { -FRAC_PI_2 } else { FRAC_PI_2 }, after);
    let yaw = approach_rot(target_yaw, DT * TAU, s.yaw);

    // The drop: 2 up, GroundHeight(0.5) from there, 2 down; below the ground → on it, the fall reset.
    t.fall += DT2 * 9.8;
    let raised = p[2] - (t.fall - 2.0);
    let g = ground([p[0], p[1], raised]);
    let z = raised - 2.0;
    if z < g {
        p[2] = g;
        t.fall = 0.0;
    } else {
        p[2] = z;
    }
    Turned { pose: HeroPose { pos: p, yaw, target_yaw }, anim, seq_b: b, before, after }
}

/// The bolt's own spin while he turns it (0x2e18f8..0x2e19a4, when his sequence is not the latch): it approaches
/// his angle + 15° at 180°/s and, being hexagonal, steps back or on by 60° when more than 60° off.
pub fn bolt_follow(bolt_yaw: f32, after: f32) -> f32 {
    let target = add_rot(PI / 12.0, after);
    let mut r = approach_rot(target, DT * PI, bolt_yaw);
    if PI / 3.0 < sub_rot(r, target) {
        r = sub_rot(r, PI / 3.0);
    } else if sub_rot(r, target) < -PI / 3.0 {
        r = add_rot(r, PI / 3.0);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::testkit;
    use crate::hero::anim::RecordingAnim;
    use crate::hero::physics::Env;
    use crate::pad::PadState;
    use crate::rng::Rng;

    fn seen_at(pos: [f32; 3], yaw: f32) -> Seen {
        Seen { pos, yaw, mode: 0, hand_id: 8, hand: true, timer: 0, stick: [0.0; 2], cam_yaw: 0.0, seq_a: SEQ_HOLD, seq_b: SEQ_HOLD, anim_flags: 0, blending: false, square: true }
    }

    /// The latch: every one of the game's conditions matters.
    #[test]
    fn latch_conditions() {
        let bolt = [10.0, 10.0, 5.0];
        // 1.2 west of the bolt, facing east, feet 0.3 above its origin.
        let s = seen_at([8.8, 10.0, 5.3], 0.0);
        assert!(latch(&s, bolt));
        assert!(!latch(&Seen { pos: [8.2, 10.0, 5.3], ..s }, bolt), "out of range (1.8)");
        assert!(!latch(&Seen { yaw: PI, ..s }, bolt), "facing away");
        assert!(latch(&Seen { yaw: 1.5, ..s }, bolt), "86° off still faces it");
        assert!(!latch(&Seen { yaw: 1.6, ..s }, bolt), "92° off");
        assert!(!latch(&Seen { pos: [8.8, 10.0, 5.55], ..s }, bolt), "too high");
        assert!(!latch(&Seen { pos: [8.8, 10.0, 5.05], ..s }, bolt), "too low");
        assert!(!latch(&Seen { hand_id: 12, ..s }, bolt), "Swingshot in hand");
        assert!(!latch(&Seen { hand: false, ..s }, bolt), "no hand moby");
        assert!(!latch(&Seen { square: false, ..s }, bolt), "no □");
        assert!(!latch(&Seen { mode: 1, ..s }, bolt), "Clank");
    }

    /// Release: □ only counts after 60 ticks on the bolt.
    #[test]
    fn release_after_60_ticks() {
        let s = seen_at([0.0; 3], 0.0);
        assert!(!release(&Seen { timer: 60, ..s }));
        assert!(release(&Seen { timer: 61, ..s }));
        assert!(!release(&Seen { timer: 200, square: false, ..s }));
    }

    /// The entry: group 9, frozen, the latch sequence at frame 2 over 8 ticks, the wrench's sequence 0xb queued.
    #[test]
    fn entry_state() {
        let coll = testkit::floor(0.0, 0, 4, 0, 4);
        let mut h = Hero::spawn([6.0, 6.0, 0.0], 0.0);
        let anim = rc_formats::moby_anim::AnimState { seq_a: 1, frame_a: 0, seq_b: 1, frame_b: 0, t: 0.0, speed: 1.0, rate: 1.0, flags: 0, trigger_count: 0, skip_advance: false };
        h.items.slot.item = Some(crate::hero::items::HandItem { o_class: super::super::melee::WRENCH_CLASS, mstate: 0, anim, snapshot: None, scale: 1.0, position: [6.0, 6.0, 1.0], rows: [[0; 4]; 3], hit_timer: 0, flight: Default::default() });
        let pad = PadState::default();
        let env = Env { coll: &coll, pad: &pad, cam_yaw: Pf::ZERO, cam_rows: testkit::cam_x().0, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: None };
        let mut anim = RecordingAnim::default();
        let mut rng = Rng::new();
        let mut c = Ctx { env: &env, anim: &mut anim, rng: &mut rng, voice: None };
        assert!(h.set_state(&mut c, STATE, true));
        assert_eq!((h.state, h.group, h.frozen, h.items.f13f7, h.timer), (STATE, 9, 1, 1, 0));
        assert_eq!(anim.calls.last().copied(), Some((Pf::from_i32(8), SEQ_LATCH, 2)));
        assert_eq!(h.items.pending_blend, Some((WRENCH_SEQ, 2, 13)));
        // A tick in the state: nothing moves him (frozen, no physics), he stays in it.
        let before = h.position();
        let mut r = testkit::Runner::new(before, 0.0);
        r.hero = h;
        r.tick(&coll, crate::pad::PadInput::neutral().stick(1.0, 0.0));
        assert_eq!(r.hero.state, STATE);
        assert_eq!(r.hero.position(), before);
        // SetState(0) lets go: unfrozen, idle.
        let mut c = Ctx { env: &env, anim: &mut r.anim, rng: &mut r.rng, voice: None };
        assert!(r.hero.set_state(&mut c, 0, true));
        assert_eq!((r.hero.state, r.hero.frozen), (0, 0));
    }

    /// Turning: the stick pushed along his facing (the ring's tangent) accelerates him by 5.5·dt² a tick up to
    /// 3.7 u/s; he stays on the ring; the angle he travels matches the speed; the animations go hold → turn → fast.
    #[test]
    fn turning_speed_ring_and_anims() {
        let bolt = [10.0, 10.0, 5.0];
        // On the ring west of the bolt (angle π), facing its tangent (π + 90° = −90°: south).
        let mut s = seen_at([10.0 - RING, 10.0, 5.3], -FRAC_PI_2);
        // Camera looking along +x: the stick "right" (+x) is world −y = his facing.
        s.stick = [1.0, 0.0];
        let mut t = Turn::default();
        let mut travelled = 0.0;
        let mut anims = Vec::new();
        let mut blend_left = 0;
        for k in 0..200 {
            // The stick along his facing (camera yaw 0: world = (−stick.y, −stick.x)).
            s.stick = [-s.yaw.sin(), -s.yaw.cos()];
            let r = turn(&s, bolt, &mut t, false, |_| 5.3);
            let d = ((r.pose.pos[0] - bolt[0]).powi(2) + (r.pose.pos[1] - bolt[1]).powi(2)).sqrt();
            // The step outward and the spring's lag: within 0.05 of the ring at the top speed.
            assert!((d - RING).abs() < 0.05, "tick {k}: {d} off the ring");
            assert_eq!(r.pose.pos[2], 5.3);
            travelled += diff_rots(r.after, r.before);
            if let Some((seq, n)) = r.anim {
                anims.push(seq);
                (s.seq_a, s.seq_b, s.blending) = (0xff, seq, true);
                blend_left = n;
            } else if blend_left > 0 {
                blend_left -= 1;
                if blend_left == 0 { (s.seq_a, s.blending) = (s.seq_b, false); }
            }
            (s.pos, s.yaw) = (r.pose.pos, r.pose.yaw);
            if k == 0 { assert!((t.speed - DT2 * ACCEL).abs() < 1e-7, "first step {}", t.speed); }
        }
        assert!((t.speed - DT * MAX_SPEED).abs() < 1e-6, "top speed {}", t.speed);
        // 200 ticks: 40 accelerating, then at 3.7/60 u per tick on a 1.05 ring.
        assert!(travelled > 9.0 && travelled < 11.5, "travelled {travelled} rad");
        assert_eq!(anims, vec![SEQ_TURN, SEQ_TURN_FAST], "hold → turn → fast turn");
    }

    /// Pushing against his facing never turns the crank backwards: the speed stays 0 and he holds still.
    #[test]
    fn no_backwards_turning() {
        let bolt = [10.0, 10.0, 5.0];
        let mut s = seen_at([10.0 - RING, 10.0, 5.3], -FRAC_PI_2);
        s.stick = [-1.0, 0.0];
        let mut t = Turn::default();
        for _ in 0..30 {
            let r = turn(&s, bolt, &mut t, false, |_| 5.3);
            assert_eq!(t.speed, 0.0);
            assert!(diff_rots(r.after, r.before) < 1e-6);
            (s.pos, s.yaw) = (r.pose.pos, r.pose.yaw);
        }
    }

    /// Off the ground he drops with 9.8 u/s² onto it; the latch wrap goes to the hold sequence.
    #[test]
    fn drop_and_latch_wrap() {
        let bolt = [10.0, 10.0, 5.0];
        let mut s = seen_at([10.0 - RING, 10.0, 6.0], -FRAC_PI_2);
        (s.seq_a, s.seq_b, s.anim_flags) = (SEQ_LATCH, SEQ_LATCH, 2);
        let mut t = Turn::default();
        let r = turn(&s, bolt, &mut t, false, |_| 5.3);
        assert_eq!(r.anim, Some((SEQ_HOLD, 7)));
        assert!((r.pose.pos[2] - (6.0 - DT2 * 9.8)).abs() < 1e-5);
        let mut z = r.pose.pos[2];
        s.anim_flags = 0;
        for _ in 0..60 {
            s.pos[2] = z;
            z = turn(&s, bolt, &mut t, false, |_| 5.3).pose.pos[2];
        }
        assert_eq!((z, t.fall), (5.3, 0.0));
    }

    /// The bolt follows 15° ahead of him, and its hexagon steps by 60° instead of spinning back.
    #[test]
    fn bolt_follows_hexagon() {
        let r = bolt_follow(0.0, 0.0);
        assert!((r - DT * PI).abs() < 1e-6, "approaches +15° at 180°/s: {r}");
        // 70° ahead of the target: one hexagon step back.
        let r = bolt_follow(add_rot(PI / 12.0, 70f32.to_radians()), 0.0);
        assert!((sub_rot(r, PI / 12.0) - (70f32 - 60.0).to_radians() + DT * PI).abs() < 1e-4, "{r}");
    }
}
