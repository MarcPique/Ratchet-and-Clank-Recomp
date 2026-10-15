//! U556: class 587, the floating platforms of Veldin's last arena (level18 `0x2d82c0` with its sparks `0x2d88b8`, its
//! bob and tilt `0x2d8710` and the boss's two calls `0x2d8858` / `0x2d8888`; 28 placed, groups 10, 11, 12: the boss
//! 1422 hides and shows them by group). The name is descriptive [L]. A platform that bobs in the air and carries
//! Ratchet (the platform block at +0x20, `CarryRiders`); when the boss marks one it starts to tilt and spark after a
//! delay, and once Ratchet steps on it (or, with +0x80, comes within 10) it plays its sound and falls, ever faster,
//! taking Ratchet's life when he rides it below the level's death height, and is deleted below z 15. The more often
//! Ratchet died on one (the difficulty word 0x1623a8 the boss counts), the slower they tilt and fall.
//!
//! **Pvars** (0xa0, mode 0x20): +0x08 the platform block's offset (0x20), +0x20 the block (+0x5c its flags: bit 0 set at
//! the init), +0x60 the placed height, +0x64 the base height, +0x6c the tilt rate, +0x70 the fall speed, +0x74 the bob
//! phase, +0x78 / +0x7c the boss's two times, +0x80 / +0x84 / +0x88 the kind words (0 on every placed one), +0x8c the
//! spark timer, +0x92 (s16) the sound's voice.
//!
//! The level18 words: gp−0x51c0 0.5 / gp−0x51bc 2.0 (the fall's acceleration per dt², ×(1 − f/3)), gp−0x51b8 0.3 (the
//! bob's height), gp−0x51b4 120 (its rate, °/s), gp−0x51b0 2.0 / gp−0x51a8 20.0 (the tilt rate, °/s, with / without
//! +0x84), gp−0x51a4 10 (the +0x80 trigger distance), gp−0x51a0 3 / gp−0x519c 12 (f = clamp((deaths − 3) / 9, 0, 1)),
//! the sparks' words gp−0x5198..−0x514c (below).
//!
//! ## Coverage
//!
//! **The update** `0x2d82c0` (old position = −position; old rotation kept; `switch(state)`; then `0x2d8710`, the delta,
//! `CarryRiders(+0x20, delta, old rotation, rotation)` `0x262a78` = L01 `0x2755f8`):
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | 0 | +0x60 = +0x64 = z; +0x70 = +0x68 = +0x6c = 0; +0x74 = `rand_angle`; +0x5c \|= 1; +0x88 ≠ 0 → 2, else 1 | [`update`] |
//! | 1, 2 | nothing (the bob and the carry) | [`update`] |
//! | 3 | `FastDecTimer(+0x7c)` out → +0x6c = (+0x84 ? 2 : 20)·(1 − f/3)·deg·dt, 4 | [`update`] |
//! | 4 | drawn and `FastDecTimer(+0x8c)` out → `0x2d88b8`, +0x8c = `ticks(rand_range(90, 120))`; Ratchet stands on it (0x13f64c, 0x13f65e = 0) → +0x8c = 0, 5, +0x92 = `PlayClassSound(1, 0, m)`; +0x80 and Ratchet within 10 (xy) → 5, the sound | [`update`] |
//! | 5 | drawn and the spark timer out → sparks, `ticks(rand_range(15, 30))`; fall speed −= (+0x80, or +0x88 without +0x84: 15·dt²; else (+0x84 ? 0.5 : 2)·(1 − f/3)·dt²); base += speed | [`update`] |
//! | | z < placed − 10: Ratchet stands on it below the death height 0x15f638, not in 0x77 → `SetState(0x77, 1)` (his death fall); speed −= 13.7·dt²; z < 15 → `DeleteMoby` (no carry that tick) | [`update`] (`cinematic::hero_state`, `svc.death_z`) |
//! | `0x2d8710` | pitch += +0x6c, clamped to ±30° (5), ±5° (4), else ±2°; +0x74 += 120°·dt; z = base + 0.3·sin | [`bob`] |
//! | `0x2d88b8` | n = `randi(5)` + 5; base = position + rows·(cos a, sin a, 0)·`randf(1, 2.5)` (a = `rand_angle`); up = rows·(the `0x1f9a00` vector); per spark: r = `rand_vec(0, 3·dt)`; v1 = up at 6·(`randf(±0.6)` + 1)·dt + r; v2 = v1 at 2·(`randf(±0.6)` + 1)·dt, v2.z −= 5·dt; sizes `randf(0.2, 0.3)`, `randf(0.2, 0.1)`; colours `tween(randf(0, 1), 0x804080ff, 0x8040ffff)` / `(0x80204040, 0x80204080)`; phases `trunc(scale(5·randf(0, 1) + 1))`, `trunc(scale(40·(randf(±0.2) + 1)))`, `trunc(scale(10·(randf(±0.2) + 1)))`; `PartType02Spawn(…, def 0x10019)` | [`sparks`] (`fx::part02`) |
//! | `0x2d8858(m)` (the boss) | state 1 → 1; state 2 → state 5, 0; else 0 | [`poll`] |
//! | `0x2d8888(m, a, b)` (the boss) | state 1 → state 3, +0x78 = b, +0x7c = a, 1; else 0 | [`arm`] |
//! | | +0x78: no reader (stored only) | n/a |
//! | | no light, hit, save write, bolt | n/a |
//!
//! `0x1f9a00` is the boot's "unit z" constant vector (0, 0, 1, 0) [L: the same helper the vents use for their up row].

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, DT, DT2};
use crate::moby_update::services::World;
use crate::moby_update::triggers;
use crate::particles::type02::Spawn;

/// The update in the level18 class table.
pub const UPDATE_FN: u32 = 0x2d_82c0;
pub const REFERENCE_LEVEL: u32 = 18;
pub const CLASSES: [i16; 1] = [587];
/// The boss's difficulty word (gp−0x4858; counted by the boss 1422 when Ratchet dies off one of these).
pub const DEATHS_WORD: u32 = 0x16_23a8;
const DEG: f32 = 0.017_453_292;

pub mod pv {
    pub const BLOCK: usize = 0x20;
    pub const FLAGS: usize = 0x5c;
    pub const HOME_Z: usize = 0x60;
    pub const BASE_Z: usize = 0x64;
    pub const W68: usize = 0x68;
    pub const TILT: usize = 0x6c;
    pub const FALL: usize = 0x70;
    pub const PHASE: usize = 0x74;
    pub const ARM_B: usize = 0x78;
    pub const ARM_A: usize = 0x7c;
    pub const NEAR: usize = 0x80;
    pub const SLOW: usize = 0x84;
    pub const KIND: usize = 0x88;
    pub const SPARK: usize = 0x8c;
    pub const VOICE: usize = 0x92;
    pub const SIZE: usize = 0x94;
}

/// The level18 words (module doc).
pub const FALL_SLOW: f32 = 0.5;
pub const FALL_FAST: f32 = 2.0;
pub const BOB_H: f32 = 0.3;
pub const BOB_RATE: f32 = 120.0;
pub const TILT_SLOW: f32 = 2.0;
pub const TILT_FAST: f32 = 20.0;
pub const NEAR_R: f32 = 10.0;
pub const DEATHS_LO: f32 = 3.0;
pub const DEATHS_HI: f32 = 12.0;

fn ok(w: &World, id: MobyId) -> bool { w.m(id).pvars.len() >= pv::SIZE }

/// f = clamp((deaths − 3) / (12 − 3), 0, 1) (the word read as an int, `cvt.s.w`).
fn hardness(w: &World) -> f32 {
    let d = w.svc.units.word(DEATHS_WORD) as i32 as f32;
    let f = (d - DEATHS_LO) / (DEATHS_HI - DEATHS_LO);
    f.clamp(0.0, 1.0)
}

fn on_it(w: &World, id: MobyId) -> bool { w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 }

/// `0x2d8858(m)`: 1 while it waits (state 1); a marked one in state 2 falls (5).
pub fn poll(w: &mut World, id: MobyId) -> i32 {
    match w.m(id).state {
        1 => 1,
        2 => {
            w.mm(id).state = 5;
            0
        }
        _ => 0,
    }
}

/// `0x2d8888(m, a, b)`: a waiting one (state 1) is marked: state 3, +0x78 = b, +0x7c = a; 1 when it was.
pub fn arm(w: &mut World, id: MobyId, a: i32, b: i32) -> i32 {
    if w.m(id).state != 1 || !ok(w, id) { return 0; }
    w.mm(id).state = 3;
    c::set_pi32(w, id, pv::ARM_B, b);
    c::set_pi32(w, id, pv::ARM_A, a);
    1
}

fn scaled(w: &World, x: f32) -> i32 { (x * f32::from_bits(w.svc.timing.timer_scale.0)) as i32 }

/// `0x2d88b8` (module doc).
pub fn sparks(w: &mut World, id: MobyId) {
    let n = w.rng.randi(5) + 5;
    let r = w.rng.randf(1.0, 2.5);
    let a = w.rng.rand_angle();
    let rows = w.m(id).rows;
    let local = [a.cos() * r, a.sin() * r, 0.0, 0.0];
    let mul = |v: c::V| -> c::V { std::array::from_fn(|k| rows[0][k] * v[0] + rows[1][k] * v[1] + rows[2][k] * v[2]) };
    let base = c::add(mul(local), w.m(id).position);
    let up = mul([0.0, 0.0, 1.0, 0.0]);
    for _ in 0..n {
        let rv = w.rng.rand_vec(0.0, 3.0 * DT);
        let k = w.rng.randf(-0.6, 0.6);
        let mut v1 = c::set_len3(up, 6.0 * (k + 1.0) * DT);
        v1 = c::add(v1, [rv[0], rv[1], rv[2], 0.0]);
        let k = w.rng.randf(-0.6, 0.6);
        let mut v2 = c::set_len3(v1, 2.0 * (k + 1.0) * DT);
        v2[2] -= 5.0 * DT;
        v1[3] = w.rng.randf(0.2, 0.3);
        v2[3] = w.rng.randf(0.2, 0.1);
        let f = w.rng.randf(0.0, 1.0);
        let c1 = crate::hud::tween_color(f, 0x8040_80ff, 0x8040_ffff);
        let f = w.rng.randf(0.0, 1.0);
        let c2 = crate::hud::tween_color(f, 0x8020_4040, 0x8020_4080);
        let f = w.rng.randf(0.0, 1.0);
        let t0 = scaled(w, 5.0 * f + 1.0);
        let f = w.rng.randf(-0.2, 0.2);
        let t1 = scaled(w, 40.0 * (f + 1.0));
        let f = w.rng.randf(-0.2, 0.2);
        let t2 = scaled(w, 10.0 * (f + 1.0));
        fx::part02(w, &Spawn { pos: base, v1, v2, c1, c2, t: [t0, t1, t2], def: 0x1_0019 });
    }
}

/// `0x2d8710` (module doc).
pub fn bob(w: &mut World, id: MobyId) {
    let p = c::add_rot(w.m(id).rotation[1], c::pf(w, id, pv::TILT));
    let lim = match w.m(id).state {
        5 => std::f32::consts::FRAC_PI_6,
        4 => 0.087_266_46,
        _ => 0.034_906_585,
    };
    w.mm(id).rotation[1] = if lim < p { lim } else if p < -lim { -lim } else { p };
    let a = c::add_rot(c::pf(w, id, pv::PHASE), BOB_RATE * DEG * DT);
    c::set_pf(w, id, pv::PHASE, a);
    let z = c::pf(w, id, pv::BASE_Z) + BOB_H * a.sin();
    w.mm(id).position[2] = z;
}

fn spark_timer(w: &mut World, id: MobyId, lo: i32, hi: i32) {
    if w.m(id).visible == 0 || c::dec_timer_pvar_i32(w, id, pv::SPARK) == 0 { return; }
    sparks(w, id);
    let r = w.rng.rand_range(lo, hi);
    let t = w.ticks(r);
    c::set_pi32(w, id, pv::SPARK, t);
}

fn start_fall(w: &mut World, id: MobyId) {
    w.mm(id).state = 5;
    let v = w.play_sound(1, 0, id);
    c::set_pi16(w, id, pv::VOICE, v as i16);
}

/// Level18 0x2d82c0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if !ok(w, id) { return; }
    let old_pos = w.m(id).position;
    let old_rot = w.m(id).rotation;
    match w.m(id).state {
        0 => {
            let z = w.m(id).position[2];
            c::set_pf(w, id, pv::HOME_Z, z);
            c::set_pf(w, id, pv::FALL, 0.0);
            c::set_pf(w, id, pv::W68, 0.0);
            c::set_pf(w, id, pv::TILT, 0.0);
            c::set_pf(w, id, pv::BASE_Z, z);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::PHASE, a);
            let f = c::pi32(w, id, pv::FLAGS) | 1;
            c::set_pi32(w, id, pv::FLAGS, f);
            w.mm(id).state = if c::pi32(w, id, pv::KIND) != 0 { 2 } else { 1 };
        }
        3 => {
            if c::dec_timer_pvar_i32(w, id, pv::ARM_A) != 0 {
                let f = hardness(w);
                let a = if c::pi32(w, id, pv::SLOW) != 0 { TILT_SLOW } else { TILT_FAST };
                c::set_pf(w, id, pv::TILT, a * (1.0 - f * 0.333) * DEG * DT);
                w.mm(id).state = 4;
            }
        }
        4 => {
            spark_timer(w, id, 0x5a, 0x78);
            if on_it(w, id) {
                c::set_pi32(w, id, pv::SPARK, 0);
                start_fall(w, id);
            }
            if c::pi32(w, id, pv::NEAR) != 0 && c::dist2(w.m(id).position, super::hero_pos(w)) < NEAR_R {
                start_fall(w, id);
            }
        }
        5 => {
            spark_timer(w, id, 0xf, 0x1e);
            let f = hardness(w);
            let (near, kind, slow) = (c::pi32(w, id, pv::NEAR), c::pi32(w, id, pv::KIND), c::pi32(w, id, pv::SLOW));
            let a = if near != 0 || (kind != 0 && slow == 0) {
                DT2 * 15.0
            } else {
                (if slow != 0 { FALL_SLOW } else { FALL_FAST }) * (1.0 - f * 0.333) * DT2
            };
            let v = c::pf(w, id, pv::FALL) - a;
            c::set_pf(w, id, pv::FALL, v);
            let base = c::pf(w, id, pv::BASE_Z) + v;
            c::set_pf(w, id, pv::BASE_Z, base);
            if w.m(id).position[2] < c::pf(w, id, pv::HOME_Z) - 10.0 {
                if on_it(w, id) && f32::from_bits(w.hero.pos[2].0) < w.svc.death_z && w.hero.state != 0x77 {
                    crate::cinematic::hero_state(w, 0x77, true);
                }
                let v = c::pf(w, id, pv::FALL) - DT2 * 13.7;
                c::set_pf(w, id, pv::FALL, v);
                if w.m(id).position[2] < 15.0 {
                    w.delete_moby(id);
                    return;
                }
            }
        }
        _ => {}
    }
    bob(w, id);
    let new_pos = w.m(id).position;
    let delta = c::sub(new_pos, old_pos);
    let rot = w.m(id).rotation;
    triggers::carry_riders(&mut w.mm(id).pvars, pv::BLOCK, delta, old_rot, rot);
}
