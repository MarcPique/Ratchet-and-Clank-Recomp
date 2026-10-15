//! U268 (census 2026-09-29): class 252, the hover zappers of Batalia and Oltanis (level08 `0x2d2af0`, level14
//! `0x2d98f0`, the same code; 77 created). A floating orb that bobs above its home (a sine on its height), drifts to a
//! random spot near home while no target is in its area path, flies at a target (Ratchet or a decoy inside the area,
//! within 14 of home and 4 in height), charges (class sound 0, waiting for it to end), then zaps: key 9..14 of its
//! sequence 3 an electric arc grows from its joint 2 to the target (a draw callback), and at key 14 a target within 3
//! (xy) and 1 in height takes a hit of 1. Any weapon hit the resolver passes kills it: `SetDeathBits` (the bolts, the
//! save bit), class sound 1 and a beam explosion. Its glow (joint 3, a camera-facing quad) fades to green, or red while
//! charging and zapping. The Suck Cannon takes it (the table [`react::HOVER_252`]: held 5; let go, it goes home).
//!
//! **Pvars** (0x234): +0x20 the damage record, +0x40 the velocity, +0x60 the suck record, +0x110 the knockback record
//! the carried update uses, +0x170 the target record (+0x1b0 the moby, +0x1b4 the kind), +0x1c0 home, +0x1d0 / +0x1e0
//! the arc's two kinks (y, z offsets and the fraction of the length in w), +0x1f0 the drift point, +0x200 the hover
//! lift, +0x204 the bob phase, +0x208 the bob amplitude, +0x20c the phase step, +0x210 the turn velocity, +0x214 (s16)
//! the charge sound's slot, +0x216 (s16) home on the ground (the lift applies), +0x218 the target moby, +0x21c the area
//! path (−1: deleted at once), +0x220 the glow point (joint 3), +0x230 the big-head cheat's.
//!
//! ## Coverage (level08 `0x2d2af0`, its tick `0x2d36e0`, its move `0x2d3d78`, the draw callbacks `0x2d3878` / `0x2d4108`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x26dae0(3.6, m, 2, +0x230)` | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | drawn and within 28 of the camera: `0x264650` (= `0x26f020`), +0x7f = 0x16 | the shadow probe | [`update`] (`shadows::probe_down`) |
//! | `0x2d36e0`: `MobyGetHitMessage(m, 0x330000, 0)`, `0x2649a8` (= `0x26f378`, col 4); out5 ≠ 1 and not dying: health −= damage (≤ 0 → reaction 1); reaction 0..11 → `SetDeathBits(m, 0, −1)`, state 99; +0xa4 = 0xff | every weapon's hit record through the resolver kills it; the bolts and the save bit | [`tick`] (`World::get_hit`, `damage::resolve`, `crate_::set_death_bits`) |
//! | states ∉ {0, 4}: `0x26a650(14, m, +0x170, …, area path +0x21c)` (= `0x274df8`); a target farther than 14 from home or 3 in height → kind 2; no moby → Ratchet's | the target search (decoys win) | [`tick`] (`target::acquire_in`) |
//! | +0x218 = +0x1b0 | | [`update`] |
//! | case 0: path −1 → deleted; targetable, yaw and phase `rand_angle`, lift 4, amplitude 0.5, velocity 0, step 2π/3·dt, +0x58 15 / +0x5a 9, +0xd0 gp−0x5378, blend 0, state 1; home = pos, on the ground (`GroundHeight(0.5)` above z − 10) → home z, +0x216 1; drift point home; +0x29 0 | init | [`init`] |
//! | case 1: no target: once the sequence wraps, a drift point `randf(0, 3)` from home at `rand_angle`, blend 2 (6), 7; a target: lift 0, amplitude 0.1, blend 2 (6), 2 | idle | [`update`] |
//! | case 2: `SpringTurn2(atan(target − pos), 0.01, 0.3, 0.05)` (0x262778); facing within 45° → velocity += (cos, sin)·12·dt²; the target within 3 (xy) and 2 in height → class sound 0 (slot +0x214), 6; target beyond 15 of home or 4 in height → lift 4, amplitude 0.5, blend 2 (6), 3 | the chase | [`update`] (`turn::spring_turn2_pvar`) |
//! | case 3: the same toward home; within 1 (xy) and 3.9 above → blend 0 (3), 1 | back home | [`update`] |
//! | case 4: turn; not blending, keys 9..14: f = (k − 9)/5, kinks (sin a, cos a)·0.25·f at f/3 and 2f/3 (`rand_angle`), `RegisterDrawCallback(0x2d3878)`; `0x26bdc8(14)` key 14 passed: the target within 3 (xy) and 1 in height → `0x2640f8(1, target, m, 1, target + 0.75 z, 0.2·dir)` (= `0x26eaa8`); lift 4, amplitude 0.5, blend 2 (3), 3 | the zap on Ratchet (his moby's hit record: the hero's intake) | [`zap`] (`attack::hit_moby`, `ground::passed_frame`) |
//! | case 5: `0x2fe770(m, +0x110)` (= `0x305260`) done → lift 4, amplitude 0.5, blend 2 (3), record state 0 | the Suck Cannon's carried update | [`update`] (`react::carried`) |
//! | case 6: turn; `SoundIsAlive(m, +0x214)` → wait; else blend 3 (3), 4 | the charge | [`update`] (`World::sound_alive`) |
//! | case 7: turn toward the drift point, accelerate facing it; within 0.5 → blend 0 (3), 1 | drift | [`update`] |
//! | case 99: class sound 1, `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 0, m, vel, pos + 1 z, 5, 2, 4, −1, 0, 1, −1, 0)` (0x268940 = 0x273310), deleted | the death | [`update`] ([`DEATH`], `fx::beam_explosion`) |
//! | `0x2d3d78`: speed `|vel| − 6·dt²` in [0, 3·dt], pos += vel; phase += step; vel.z = clamp(home z + amplitude·sin phase (+ lift on the ground) − z, ±3·dt); `0x2025d8(…)` | the hover | [`hover`]; `0x2025d8` is `jr ra; li v0, 0` in the overlay (a compiled-out sphere push): nothing |
//! | state ≠ 5, alive: glow `FastTweenColor(0.1, +0x90, 0x7f20ff20 / 0x7f2828a0 in 4 and 6)`, `0x259cc8(m, 3, +0x220)` (= `0x2645a8`), `RegisterDrawCallback2(0x2d4108)` | the glow | [`update`] (`Callback::UnitGlow`) |
//! | `0x2d4108` (list 2): the glow quad of `0x2781d0` inline: FX 0xb, size 0.175 (gp−0x5364), pull 0.2 (gp−0x5368), at +0x220, RGBA `+0x90 & 0xffffff \| 0x40000000`, additive | | [`glow_quads`] + `rc-engine` fx_draw |
//! | `0x2d3878` (list 1): v = `0x2745f0(3, target − joint 2)` (length ≤ 3), v.z = target z + 0.25 − joint z, L = \|v\|; the frame `Euler(0, −atan(vz, \|v.xy\|), atan(v))` at joint 2; three segments (0 → kink 1 → kink 2 → 3·kink 1's w) of two crossed quads each (z ±0.25, then y −/+ 0.25 with z +/− 0.25), FX 0xe, RGBA 0x3ff08080 ×2 / 0x3ff08030 ×2 (gp−0x5388..), ST (0,0) (1,0) (0,1) (1,1), additive | the arc | [`fx_quads`] + `rc-engine` fx_draw; joint 2 is taken when the update registers the callback (the matrix of `register_with_matrix`) [L] |
//! | table 0x2d3f60 / 0x2d3fb0 / 0x2d4000 (held 5, refused → 1), 0x2d4050 (let go: state 3, lift 4, amplitude 0.5, blend 2 over 3, record state 0), 0x2d40d8 (+0x60), `DeleteMoby` | the Suck Cannon | `react::slot_*` with [`react::HOVER_252`], [`let_go`] |
//!
//! No knockback (it has no flight), no pieces, no light. Native `f32`; the rand draws at the game's points.

use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::fx::{beam_explosion, Beam};
use crate::moby_update::creature::{self as c, attack, damage, ground, react, target, turn};
use crate::moby_update::services::{pvar as p, Services, World};
use super::{FxQuad, FxQuads, GlowQuad};

pub const UPDATE_FN: u32 = 0x2d_2af0;
pub const REFERENCE_LEVEL: u32 = 8;
pub const CLASSES: [i16; 1] = [252];
/// The draw callbacks (their own `PORTS` rows: `Callback::UnitGlow` / `Callback::UnitQuads` payloads).
pub const GLOW_FN: u32 = 0x2d_4108;
pub const ARC_FN: u32 = 0x2d_3878;

pub mod pv {
    pub const D: usize = 0x20;
    pub const VEL: usize = 0x40;
    pub const SUCK: usize = 0x60;
    pub const KNOCK: usize = 0x110;
    pub const TGT: usize = 0x170;
    pub const TGT_MOBY: usize = 0x1b0;
    pub const TGT_KIND: usize = 0x1b4;
    pub const HOME: usize = 0x1c0;
    pub const KINK1: usize = 0x1d0;
    pub const KINK2: usize = 0x1e0;
    pub const DRIFT: usize = 0x1f0;
    pub const LIFT: usize = 0x200;
    pub const PHASE: usize = 0x204;
    pub const AMP: usize = 0x208;
    pub const STEP: usize = 0x20c;
    pub const TURN_V: usize = 0x210;
    pub const SLOT: usize = 0x214;
    pub const GROUNDED: usize = 0x216;
    pub const TARGET: usize = 0x218;
    pub const AREA: usize = 0x21c;
    pub const GLOW_AT: usize = 0x220;
    pub const SIZE: usize = 0x234;
}

pub mod st {
    pub const INIT: u8 = 0;
    pub const IDLE: u8 = 1;
    pub const CHASE: u8 = 2;
    pub const HOME: u8 = 3;
    pub const ZAP: u8 = 4;
    pub const HELD: u8 = 5;
    pub const CHARGE: u8 = 6;
    pub const DRIFT: u8 = 7;
    pub const DYING: u8 = 99;
}

/// The death's beam explosion (`0x268940`).
pub const DEATH: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 0.0, streaks: 5, sparks: 2, puffs: 4, debris: 1, sound: -1, shake: false };
/// The glow's colours (`FastTweenColor` targets): idle and charging / zapping.
pub const GLOW_IDLE: u32 = 0x7f20_ff20;
pub const GLOW_ANGRY: u32 = 0x7f28_28a0;
/// The glow quad (level08 gp−0x5364 / −0x5368).
pub const GLOW_SIZE: f32 = 0.175;
pub const GLOW_PULL: f32 = 0.2;
/// The arc: FX 0xe, the corner colours gp−0x5388..−0x537c.
pub const ARC_FX: usize = 0xe;
pub const ARC_RGBA: [u32; 4] = [0x3ff0_8080, 0x3ff0_8080, 0x3ff0_8030, 0x3ff0_8030];
const ARC_ST: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
/// 45° (`0x3f490fdb`).
const FACING: f32 = std::f32::consts::FRAC_PI_4;

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn seq(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
/// `fun_00212f90(m, s, 0, n)` behind `if (m+0x53 != s)` (the literal tick counts of this class).
fn blend(w: &mut World, id: MobyId, s: u8, n: i32) { if seq(w, id) != s { w.anim_blend(id, s, 0, n); } }
fn moby_ref(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    (v > 0).then(|| (v - 1) as MobyId).filter(|&m| m < w.table.mobys.len())
}
fn set_moby_ref(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)); }
fn area(w: &World, id: MobyId) -> Option<usize> { usize::try_from(c::pi32(w, id, pv::AREA)).ok().filter(|&p| p < w.svc.splines.len()) }
fn lift(w: &mut World, id: MobyId) {
    c::set_pf(w, id, pv::LIFT, 4.0);
    c::set_pf(w, id, pv::AMP, 0.5);
}

/// `SpringTurn2(atan(to − pos), 0.01, 0.3, 0.05)`, then the thrust along the heading when facing within 45°.
fn steer(w: &mut World, id: MobyId, to: c::V, thrust: bool) {
    let p = c::pos(w, id);
    let h = c::atan(to[0] - p[0], to[1] - p[1]);
    turn::spring_turn2_pvar(w, id, h, 0.01, 0.3, 0.05, pv::TURN_V);
    if thrust && c::diff_rots(c::yaw(w, id), h) < FACING {
        let (co, si) = c::cs(c::yaw(w, id));
        let v = c::pv4(w, id, pv::VEL);
        c::set_pf(w, id, pv::VEL, v[0] + co * c::DT2 * 12.0);
        c::set_pf(w, id, pv::VEL + 4, v[1] + si * c::DT2 * 12.0);
    }
}

/// `0x2d36e0` (module doc).
fn tick(w: &mut World, id: MobyId) {
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && state(w, id) != st::DYING {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        let r = if hp <= 0.0 { 1 } else { res.reaction as i32 };
        if (0..0xc).contains(&r) {
            set_death_bits(w, id, 0, -1);
            set_state(w, id, st::DYING);
        }
    }
    w.mm(id).hit_slot = 0xff;
    if !matches!(state(w, id), st::INIT | st::ZAP) {
        let t = target::acquire_in(w, id, 14.0, area(w, id));
        c::set_pv4(w, id, pv::TGT, t.pos);
        c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
        c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
        c::set_pv4(w, id, pv::TGT + 0x30, t.body);
        set_moby_ref(w, id, pv::TGT_MOBY, t.moby);
        c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
        if t.kind != 2 {
            let h = c::pv4(w, id, pv::HOME);
            if 14.0 < c::dist2(h, t.pos) || 3.0 < (h[2] - t.pos[2]).abs() { c::set_pi32(w, id, pv::TGT_KIND, 2); }
        }
    }
    if moby_ref(w, id, pv::TGT_MOBY).is_none() {
        let h = w.hero_moby;
        set_moby_ref(w, id, pv::TGT_MOBY, h);
    }
}

/// Case 0.
fn init(w: &mut World, id: MobyId) {
    if c::pi32(w, id, pv::AREA) == -1 {
        w.delete_moby(id);
        return;
    }
    w.mm(id).mode |= mode::TARGETABLE;
    let y = w.rng.rand_angle();
    c::set_yaw(w, id, y);
    c::set_pf(w, id, pv::LIFT, 4.0);
    let ph = w.rng.rand_angle();
    c::set_pf(w, id, pv::AMP, 0.5);
    c::set_pf(w, id, pv::PHASE, ph);
    c::set_pv4(w, id, pv::VEL, [0.0; 4]);
    c::set_pf(w, id, pv::STEP, c::DT * 2.094_395_2);
    c::set_pu8(w, id, 0x58, 15);
    c::set_pu8(w, id, 0x5a, 9);
    if seq(w, id) != 0 { w.anim_blend(id, 0, 0, 0); }
    set_state(w, id, st::IDLE);
    let p = c::pos(w, id);
    let mut home = p;
    let g = ground::ground(w, p, 0.5, 0).z;
    if p[2] - 10.0 < g {
        home[2] = ground::ground(w, p, 0.5, 0).z;
        c::set_pi16(w, id, pv::GROUNDED, 1);
    } else {
        home[2] = p[2];
        c::set_pi16(w, id, pv::GROUNDED, 0);
    }
    c::set_pv4(w, id, pv::HOME, home);
    c::set_pv4(w, id, pv::DRIFT, home);
    c::set_pu8(w, id, pv::D + 9, 0);
}

/// Case 4.
fn zap(w: &mut World, id: MobyId, t: c::V) {
    let p = c::pos(w, id);
    turn::spring_turn2_pvar(w, id, c::atan(t[0] - p[0], t[1] - p[1]), 0.01, 0.3, 0.05, pv::TURN_V);
    let k = ground::key_time(w, id);
    let a = w.m(id).anim;
    if a.seq_a == a.seq_b && (9.0..=14.0).contains(&k) {
        let f = (k - 9.0) / 5.0;
        c::set_pf(w, id, pv::KINK1 + 0xc, f / 3.0);
        c::set_pf(w, id, pv::KINK2 + 0xc, (f + f) / 3.0);
        let ang = w.rng.rand_angle();
        c::set_pf(w, id, pv::KINK1 + 4, ang.sin() * 0.25 * f);
        c::set_pf(w, id, pv::KINK1 + 8, ang.cos() * 0.25 * f);
        c::set_pf(w, id, pv::KINK2 + 4, ang.sin() * 0.25 * f);
        c::set_pf(w, id, pv::KINK2 + 8, ang.cos() * 0.25 * f);
        if let Some(i) = super::row(REFERENCE_LEVEL, ARC_FN) {
            let j = w.joint_point(id, 2);
            let m = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [j[0], j[1], j[2], 1.0]];
            w.svc.draw_callbacks.register_with_matrix(Callback::UnitQuads(i), id, m);
        }
    }
    if !ground::passed_frame(w, id, 14.0) { return; }
    if let Some(tm) = moby_ref(w, id, pv::TARGET) {
        let me = c::pos(w, id);
        if c::dist2(c::pos(w, tm), me) < 3.0 && (c::pos(w, tm)[2] - me[2]).abs() < 1.0 {
            let (co, si) = c::cs(c::yaw(w, id));
            let dir = [co * 0.2, si * 0.2, 0.0, 0.0];
            let at = [t[0], t[1], t[2] + 0.75, t[3]];
            attack::hit_moby(w, tm, id, 1.0, 1, at, dir);
        }
    }
    lift(w, id);
    blend(w, id, 2, 3);
    set_state(w, id, st::HOME);
}

/// `0x2d3d78` (module doc).
fn hover(w: &mut World, id: MobyId) {
    let v = c::pv4(w, id, pv::VEL);
    let mut s = c::len3(v) - c::DT2 * 6.0;
    let max = c::DT * 3.0;
    if max < s { s = max; } else if s < 0.0 { s = 0.0; }
    let v = c::set_len3(v, s);
    c::set_pv4(w, id, pv::VEL, v);
    let m = w.mm(id);
    for (p, d) in m.position.iter_mut().zip(v) { *p += d; }
    let ph = c::add_rot(c::pf(w, id, pv::PHASE), c::pf(w, id, pv::STEP));
    c::set_pf(w, id, pv::PHASE, ph);
    let mut z = c::pf(w, id, pv::HOME + 8) + c::pf(w, id, pv::AMP) * ph.sin();
    if c::pi16(w, id, pv::GROUNDED) != 0 { z += c::pf(w, id, pv::LIFT); }
    let d = z - w.m(id).position[2];
    let vz = if max < d { max } else if d < -max { -max } else { d };
    c::set_pf(w, id, pv::VEL + 8, vz);
    // 0x2025d8(sphere, 400, 0, m, 0): a stub in the overlay (`jr ra; li v0, 0`): no push.
}

/// Slot +0x0c of 252's table (level08 0x2d4050), after the shared let-go handler.
pub fn let_go(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    set_state(w, id, st::HOME);
    lift(w, id);
    blend(w, id, 2, 3);
    c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
}

/// Level08 `0x2d2af0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    // 0x26dae0(3.6, m, 2, +0x230): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 3.6, id, 2, id, 0x230);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam) < 28.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x16;
    }
    tick(w, id);
    let tm = c::pi32(w, id, pv::TGT_MOBY);
    c::set_pi32(w, id, pv::TARGET, tm);
    let t = c::pv4(w, id, pv::TGT);
    match state(w, id) {
        st::INIT => {
            init(w, id);
            if state(w, id) >= 0x80 { return; }
        }
        st::IDLE => {
            if c::pi32(w, id, pv::TGT_KIND) == 2 {
                if w.m(id).anim.flags & 2 != 0 {
                    let a = w.rng.rand_angle();
                    let r = w.rng.randf(0.0, 3.0);
                    let h = c::pv4(w, id, pv::HOME);
                    c::set_pv4(w, id, pv::DRIFT, [h[0] + a.cos() * r, h[1] + a.sin() * r, h[2], h[3]]);
                    blend(w, id, 2, 6);
                    set_state(w, id, st::DRIFT);
                }
            } else {
                c::set_pf(w, id, pv::LIFT, 0.0);
                c::set_pf(w, id, pv::AMP, 0.1);
                blend(w, id, 2, 6);
                set_state(w, id, st::CHASE);
            }
        }
        st::CHASE => {
            steer(w, id, t, true);
            let me = c::pos(w, id);
            let near = moby_ref(w, id, pv::TARGET).is_some_and(|m| c::dist2(me, c::pos(w, m)) < 3.0 && (c::pos(w, m)[2] - me[2]).abs() < 2.0);
            if near {
                let s = w.play_sound(0, 0, id);
                c::set_pi16(w, id, pv::SLOT, s as i16);
                set_state(w, id, st::CHARGE);
            } else {
                let h = c::pv4(w, id, pv::HOME);
                if 15.0 < c::dist2(t, h) || 4.0 < (h[2] - t[2]).abs() {
                    lift(w, id);
                    blend(w, id, 2, 6);
                    set_state(w, id, st::HOME);
                }
            }
        }
        st::HOME => {
            let h = c::pv4(w, id, pv::HOME);
            steer(w, id, h, true);
            if c::dist2(c::pos(w, id), h) < 1.0 && 3.9 <= (h[2] - w.m(id).position[2]).abs() {
                blend(w, id, 0, 3);
                set_state(w, id, st::IDLE);
            }
        }
        st::ZAP => zap(w, id, t),
        st::HELD => {
            if react::carried(w, id, pv::KNOCK) != 0 {
                lift(w, id);
                blend(w, id, 2, 3);
                c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
            }
        }
        st::CHARGE => {
            steer(w, id, t, false);
            let slot = c::pi16(w, id, pv::SLOT) as i32;
            if !w.sound_alive(slot, id) {
                blend(w, id, 3, 3);
                set_state(w, id, st::ZAP);
            }
        }
        st::DRIFT => {
            let d = c::pv4(w, id, pv::DRIFT);
            steer(w, id, d, true);
            if c::dist2(c::pos(w, id), d) < 0.5 {
                blend(w, id, 0, 3);
                set_state(w, id, st::IDLE);
            }
        }
        st::DYING => {
            let mut at = c::pos(w, id);
            at[2] += 1.0;
            w.play_sound(1, 0, id);
            beam_explosion(w, &DEATH, Some(id), at);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    hover(w, id);
    let s = state(w, id);
    if s != st::HELD && s < 0x80 {
        let to = if s == st::ZAP || s == st::CHARGE { GLOW_ANGRY } else { GLOW_IDLE };
        let g = crate::hud::tween_color(0.1, w.m(id).glow, to);
        w.mm(id).glow = g;
        let j = w.joint_point(id, 3);
        c::set_pv4(w, id, pv::GLOW_AT, j);
        if let Some(i) = super::row(REFERENCE_LEVEL, GLOW_FN) { w.svc.draw_callbacks.register2(Callback::UnitGlow(i), id); }
    }
}

/// `0x2d4108`: the glow quad at the joint-3 point (draw only).
pub fn glow_quads(table: &MobyTable, id: MobyId) -> Vec<GlowQuad> {
    let Some(m) = table.mobys.get(id).filter(|m| m.pvars.len() >= pv::SIZE) else { return Vec::new() };
    let q = p::v4f(&m.pvars, pv::GLOW_AT);
    vec![GlowQuad { size: GLOW_SIZE, pull: GLOW_PULL, point: [q[0], q[1], q[2]], rgba: (m.glow & 0x00ff_ffff) | 0x4000_0000 }]
}

/// `0x2d3878`: the arc from joint 2 (the registration's matrix) to the target (draw only).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= pv::SIZE)?;
    let j = svc.draw_callbacks.matrices.get(&id)?[3];
    let tr = p::i32(&m.pvars, pv::TARGET);
    let tm = table.mobys.get(usize::try_from(tr - 1).ok()?)?;
    let tp = tm.position;
    let mut v = [tp[0] - j[0], tp[1] - j[1], tp[2] - j[2], 0.0];
    let l = c::len3(v);
    if 3.0 < l { v = c::set_len3(v, 3.0); }
    v[2] = (tp[2] + 0.25) - j[2];
    let len = c::len3(v);
    let rows = rc_formats::moby_light::rotation_rows([0.0, -c::atan(c::len2(v), v[2]), c::atan(v[0], v[1])]).map(|r| r.map(f32::from_bits));
    let world = |q: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| q[0] * rows[0][k] + q[1] * rows[1][k] + q[2] * rows[2][k] + j[k]) };
    let k1 = p::v4f(&m.pvars, pv::KINK1);
    let k2 = p::v4f(&m.pvars, pv::KINK2);
    let neg0 = f32::from_bits(0x8000_0000);
    let (x1, x2, xe) = (len * k1[3], len * k2[3], len * k1[3] * 3.0);
    // The three segments as the game fills its four vertices, then moves some of them for the crossed quad (y −/+ 0.25,
    // z +/− 0.25 for the vertices below / from index 2): the arc's two ends are single points and stay.
    let segs: [([[f32; 3]; 4], &[usize]); 3] = [
        ([[0.0, 0.0, neg0], [x1, k1[1], k1[2] - 0.25], [0.0, 0.0, 0.0], [x1, k1[1], k1[2] + 0.25]], &[1, 3]),
        ([[x1, k1[1], k1[2] - 0.25], [x2, k2[1], k2[2] - 0.25], [x1, k1[1], k1[2] + 0.25], [x2, k2[1], k2[2] + 0.25]], &[0, 1, 2, 3]),
        ([[x2, k2[1], k2[2] - 0.25], [xe, 0.0, neg0], [x2, k2[1], k2[2] + 0.25], [xe, 0.0, 0.0]], &[0, 2]),
    ];
    let mut quads = Vec::new();
    for (mut vs, moved) in segs {
        quads.push(FxQuad { corners: vs.map(world), st: ARC_ST, rgba: ARC_RGBA });
        for &i in moved {
            if i < 2 { vs[i][1] -= 0.25; vs[i][2] += 0.25; } else { vs[i][1] += 0.25; vs[i][2] -= 0.25; }
        }
        quads.push(FxQuad { corners: vs.map(world), st: ARC_ST, rgba: ARC_RGBA });
    }
    Some(FxQuads { fx: ARC_FX, additive: true, subtract: false, quads })
}
