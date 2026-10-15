//! U572: class 1906, the boss's hoppers (level18 `0x2fbb98` with its prologue `0x2fc100`, its walk `0x2fc4e0`, the
//! boss's two calls `0x2fc668` / `0x2fc7e0` and its draws `0x2fca98` / `0x2fc860`; 18 placed in group 21). The name
//! is descriptive [L]. A small creature that waits hidden until the boss 1422 launches it (state 1: a lob from beside
//! the boss); it lands, walks at Ratchet inside the arena region (path 60) and bites (joint 1, 1 damage); hit to 0 it
//! is knocked away (1 in 2) and bursts on landing, or bursts at once; Ratchet held (0x72) or a scene parks them all.
//! Only the boss wakes it (G-CLS-001 listed it as "woken by 1422").
//!
//! **Pvars** (0x2f0, mode 0x20): +0x00 → the damage record +0x20, +0x0c → the flash +0x60, +0x10 → the knock record
//! +0x70, +0x14 → +0xd0; +0x138 (s16) the Suck Cannon's word, +0x140 the walker's animation table, +0x180 the target
//! record (`0x274df8`: +0x1c0 the moby, +0x1c4 the kind), +0x1d0 the launch velocity, +0x1e0 the region path, +0x1e8 the
//! yaw spring, +0x1ec the yaw offset, +0x1f0 the walk speed, +0x1f4 the spawn flash, +0x1f8 the glow phase.
//!
//! The level18 words: gp−0x4610 the animation table (04 01 01 01 04 04 04 04), gp−0x4608 4 (?), gp−0x4604 1.0 (the scale
//! factor), gp−0x4600 0 (the bite's start frame), gp−0x45fc (the once-a-frame word), gp−0x45f8 0 (+0x73), gp−0x45f4 /
//! −0x45f0 0x80000080 / 0x80005080 (the glow), gp−0x45ec 400 / −0x45e8 800 (its rate, °/s), gp−0x45e4 10 / −0x45e0 20 /
//! −0x45dc 15 / −0x45d8 3 (the knock's speeds).
//!
//! ## Coverage
//!
//! **The prologue** `0x2fc100` (states other than 0 and 8):
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | | not 7 and a new frame (0x15f5cc ≠ gp−0x45fc): the word = it, `RegisterDrawCallback(0x2fca98)` | the group's glows | [`prologue`] (`Callback::UnitGlow`, [`glow_quads`]) |
//! | | +0x1f8 += (800 in states 4, 5, else 400)°·dt; +0x90 = `FastTweenColor(sin·½ + ½, 0x80000080, 0x80005080)` | | [`prologue`] |
//! | | z < 100, not 7 → 6 | fell off | [`prologue`] |
//! | | scale = class scale · 1 | | [`prologue`] |
//! | | (Ratchet in 0x72 or game mode 2), not 7 → 8, blend 0 (`ticks(10)`), collision off, mode & ~0x1000 \| 0x41 | parked | [`prologue`] |
//! | | else the hit (`0x330000`, the resolver at +0x20, column 4): not k 1, not 6, damage ≠ 0 and the health ≤ 0: `randi(2)` = 0 → the knock (+0xad 0, +0x98 0.5, +0x8c 3·dt, +0x80 10·dt², +0x84 20·dt², +0xbc 2·dt, +0x88 15·dt, +0x94 0x29, +0x90 0x200; mode & ~0x1000; `0x25ce88(hit dir)` the angle, `0x25e898(angle, m, +0x70, 3, 1, 0)`; +0xc0 7.5, +0xc4 15; 5; collision off; +0x67 0x78, the flash); else 6 | the death | [`prologue`] (`knock::aim`, `knock::start`, `flash::start`) |
//! | | +0xa4 = 0xff; `0x25f878`; the target (`0x274df8(512, m, +0x180, 0, 0, path +0x1e0)`); kind 2 → +0x180 = the end of Ratchet's rail (group 0xf) or his position; +0x1c0 = 0 → Ratchet's moby | | [`prologue`] (`target::acquire_in`) |
//!
//! **The update** `0x2fbb98`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | 0 | 8; `randi(2)` → mode 0x8000 set / cleared; mode \| 0x100; +0x73 = 0; +0x140 = the table; +0x58 = 8, +0x20 = 1, +0x5c = 1, +0x30 = 0.5, +0x24 (s16) = 1; +0x1f8 = `rand_angle`; update / draw 0xff; not drawn; mode & ~0x1000 \| 1; collision off | parked | [`update`] |
//! | 1 | vz −= 10·dt²; position += +0x1d0; ground (from 1 up) above z → z = it, 2, blend 0 (`ticks(10)`); else z < 10 → 6 | the lob | [`update`] |
//! | 2 | kind 2 and within 10 (xy) of +0x180 → stay; else 3, blend 1 (`ticks(10)`) | idle | [`update`] |
//! | 3 | `0x2fc4e0(m, +0x180)` = d; (not kind 2 or d ≥ 5): d < 1.5 → 4, blend 2 (frame 0, `ticks(5)`); else (kind 2, d < 5) → 2, blend 0 | the walk | [`update`] ([`walk`]) |
//! | 4 | turn at +0x180; key 14 passed → `0x25bd28(0.5, 1, 1, m, 1, 1, 0, 1, 0)` (the bite on joint list 1); done: not kind 2 → beyond 2.5 → 3, blend 1; kind 2 → 2, blend 0 | the bite | [`update`] (`attack::joint_hit`) |
//! | 5 | `0x25e9d8(m, +0x70)` & 0x140 → 6 | knocked | [`update`] (`knock::update`) |
//! | 6 | `SpawnBeamExplosion(0, 0, 1, 0.5, 9, 0.5, 0, m, +0x40, z + 0.5, 5, 2, 4, sound 4)`; 8, blend 0, collision off, mode & ~0x1000 \| 0x41 | the burst | [`update`] (`fx::beam_explosion`) |
//! | 7 | `0x2e6d80(m, +0x70)` (the Suck Cannon carry) → 2, +0x138 = 0 | carried | [`update`] (`react::carried`) |
//! | tail | rows from the Euler (`0x1fa030`); +0x1f4 ≠ 0: `Approach(0, 4·dt)`, row 2 ·(1 + it), rows 0 / 1 ·(1 − it), `RegisterDrawCallback(0x2fc860)` | the spawn flash | [`update`] (`Callback::UnitQuads`, [`beam_quads`]) |
//! | `0x2fc4e0(m, p)` | yaw toward p + +0x1ec (`0x25e140`, 2π); d = xy distance; `0x25dcb0(d, 6·dt², 6·dt², 5·dt, &0, &+0x1f0)`; move (cos, sin)·+0x1f0, −2·dt (`0x25ab18(0.5, 0.5, 0, m, …, 0)`); pushed out of the region's walls (`0x2640b8(0.5, …)`) | | [`walk`] (`walker::move_collide`, `region::push_out`) |
//! | `0x2fc668(s, group, from, to)` | the first parked (8) member: shown, targetable, collision on, position from, 1, +0x1f4 = 1, +0x1d0 = unit(to − from)·s (xy), vz = `0x25cf30(s, −10·dt², from, to, 0)`, +0x1ec = `randf_sym(15°, 45°)`; 1; none: 0 | the launch | [`launch`] (`knock::lob_up`) |
//! | `0x2fc7e0(group)` | the members of class 1906 not parked | | [`count`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, ground, knock, react, region, target, turn, walker, DT, DT2};
use crate::moby_update::scheduler::group_ids;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2f_bb98;
/// The group's glow draw (`RegisterDrawCallback` from the prologue).
pub const GLOW_FN: u32 = 0x2f_ca98;
/// The spawn beam's draw (`RegisterDrawCallback` from the tail).
pub const BEAM_FN: u32 = 0x2f_c860;
pub const REFERENCE_LEVEL: u32 = 18;
pub const CLASSES: [i16; 1] = [1906];
pub const CLASS: i16 = 0x772;
const DEG: f32 = 0.017_453_292;
const TAU: f32 = 6.283_185_5;

pub mod pv {
    pub const D: usize = 0x20;
    pub const F: usize = 0x60;
    pub const K: usize = 0x70;
    pub const CARRY: usize = 0x138;
    pub const ANIM_TABLE: usize = 0x140;
    pub const TARGET: usize = 0x180;
    pub const TARGET_MOBY: usize = 0x1c0;
    pub const TARGET_KIND: usize = 0x1c4;
    pub const VEL: usize = 0x1d0;
    pub const REGION: usize = 0x1e0;
    pub const YAW_V: usize = 0x1e8;
    pub const YAW_OFS: usize = 0x1ec;
    pub const SPEED: usize = 0x1f0;
    pub const SPAWN: usize = 0x1f4;
    pub const GLOW: usize = 0x1f8;
    pub const SIZE: usize = 0x1fc;
    /// Port-only (past the game's 0x2f0): joints 2..4 at the end of the update, for the draw `0x2fca98` (the draw has
    /// no joint poses in the port).
    pub const JOINTS: usize = 0x2f0;
    /// Port-only: the spawn beam's four corners (`0x2fc860`), built at the registration against the camera then.
    pub const BEAM: usize = 0x320;
    pub const PORT_SIZE: usize = 0x360;
}

/// The once-a-frame word of the glows (gp−0x45fc).
pub const FRAME_WORD: u32 = 0x16_2604;

fn ok(w: &World, id: MobyId) -> bool { w.m(id).o_class == CLASS && w.m(id).pvars.len() >= pv::SIZE }
fn blend(w: &mut World, id: MobyId, seq: u8, frame: i32, n: i32) { let t = w.ticks(n); c::blend_to(w, id, seq, frame, t); }
fn park(w: &mut World, id: MobyId) {
    w.mm(id).state = 8;
    blend(w, id, 0, 0, 10);
    let m = w.mm(id);
    m.has_collision = false;
    m.mode = (m.mode & !mode::TARGETABLE) | 0x41;
}

/// `0x2fc668(s, group, from, to)` (module doc): true when one was launched.
pub fn launch(w: &mut World, s: f32, group: i32, from: c::V, to: c::V) -> bool {
    let Ok(g) = i8::try_from(group) else { return false };
    for m in group_ids(w, g) {
        if w.m(m).o_class != CLASS || w.m(m).state != 8 || !ok(w, m) { continue; }
        let col = super::class_collision(w, CLASS);
        let mo = w.mm(m);
        mo.mode = (mo.mode & !0x41) | mode::TARGETABLE;
        mo.has_collision = col;
        mo.position = from;
        mo.state = 1;
        c::set_pf(w, m, pv::SPAWN, 1.0);
        let mut d = c::sub(to, from);
        d[2] = 0.0;
        let d = c::set_len3(d, s);
        let mut t = 0.0;
        let vz = knock::lob_up(s, -(DT2 * 10.0), from, to, &mut t);
        c::set_pv4(w, m, pv::VEL, [d[0], d[1], vz, d[3]]);
        let o = w.rng.randf_sym(15.0, 45.0);
        c::set_pf(w, m, pv::YAW_OFS, o * DEG);
        return true;
    }
    false
}

/// `0x2fc7e0(group)`: the members not parked.
pub fn count(w: &World, group: i32) -> i32 {
    let Ok(g) = i8::try_from(group) else { return 0 };
    group_ids(w, g).into_iter().filter(|&m| w.m(m).o_class == CLASS && w.m(m).state != 8).count() as i32
}

/// `0x2fc4e0(m, p)` (module doc): the xy distance to `p`.
pub fn walk(w: &mut World, id: MobyId, p: c::V) -> f32 {
    let pos = c::pos(w, id);
    let a = c::add_rot(c::atan(p[0] - pos[0], p[1] - pos[1]), c::pf(w, id, pv::YAW_OFS));
    turn::turn_toward_pvar(w, id, a, TAU * DT2, TAU * DT2, TAU * DT, pv::YAW_V);
    let d = c::dist2(pos, p);
    let (mut x, mut v) = (0.0, c::pf(w, id, pv::SPEED));
    turn::spring(d, 6.0 * DT2, 6.0 * DT2, 5.0 * DT, &mut x, &mut v);
    c::set_pf(w, id, pv::SPEED, v);
    let y = c::yaw(w, id);
    let mut mv = [y.cos() * v, y.sin() * v, -(DT + DT), 0.0];
    walker::move_collide(w, id, 0.5, 0.5, 0.0, &mut mv, 0);
    if let Ok(r) = usize::try_from(c::pi32(w, id, pv::REGION)) {
        if let Some(q) = region::push_out(w, 0.5, r, c::pos(w, id)) { c::set_pos(w, id, q); }
    }
    d
}

/// `0x2fc100` (module doc).
fn prologue(w: &mut World, id: MobyId) {
    let st = w.m(id).state;
    if st == 0 || st == 8 { return; }
    if st != 7 && w.svc.units.word(FRAME_WORD) != w.counter as u32 {
        w.svc.units.set_word(FRAME_WORD, w.counter as u32);
        // `RegisterDrawCallback(0x2fca98)`: the group's joint glows ([`glow_quads`]).
        if let Some(r) = super::row(REFERENCE_LEVEL, GLOW_FN) { w.svc.draw_callbacks.register(Callback::UnitGlow(r), id); }
    }
    let rate = if st.wrapping_sub(4) < 2 { 800.0 } else { 400.0 };
    let a = c::add_rot(c::pf(w, id, pv::GLOW), rate * DEG * DT);
    c::set_pf(w, id, pv::GLOW, a);
    w.mm(id).glow = crate::hud::tween_color(a.sin() * 0.5 + 0.5, 0x8000_0080, 0x8000_5080);
    if w.m(id).position[2] < 100.0 && st != 7 { w.mm(id).state = 6; }
    let s = super::class_scale(w, CLASS);
    w.mm(id).scale = s;
    if (w.hero.state == 0x72 || w.svc.game_mode == 2) && st != 7 {
        park(w, id);
        return;
    }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && w.m(id).state != 6 && res.damage != 0.0 {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        if hp <= 0.0 {
            if w.rng.randi(2) == 0 {
                let k = pv::K;
                c::set_pu8(w, id, k + 0x3d, 0);
                c::set_pf(w, id, k + 0x28, 0.5);
                c::set_pf(w, id, k + 0x1c, 3.0 * DT);
                c::set_pf(w, id, k + 0x10, 10.0 * DT2);
                c::set_pf(w, id, k + 0x14, 20.0 * DT2);
                c::set_pf(w, id, k + 0x4c, DT + DT);
                c::set_pf(w, id, k + 0x18, 15.0 * DT);
                c::set_pi32(w, id, k + 0x24, 0x29);
                c::set_pi32(w, id, k + 0x20, 0x200);
                w.mm(id).mode &= !mode::TARGETABLE;
                let dir = res.hit.map_or([0.0; 4], |h| h.dir.map(|x| f32::from_bits(x.0)));
                let (mut sp, mut up) = (c::pf(w, id, k + 0x18), c::pf(w, id, k + 0x1c));
                let angle = knock::aim(dir, &mut sp, &mut up);
                c::set_pf(w, id, k + 0x18, sp);
                c::set_pf(w, id, k + 0x1c, up);
                knock::start(w, id, k, angle, 3, 1, 0);
                c::set_pf(w, id, k + 0x50, 7.5);
                c::set_pf(w, id, k + 0x54, 15.0);
                w.mm(id).state = 5;
                w.mm(id).has_collision = false;
                c::set_pu8(w, id, pv::F + 7, 0x78);
                flash::start(w, id, pv::F);
            } else {
                w.mm(id).state = 6;
            }
        }
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::F);
    let region = usize::try_from(c::pi32(w, id, pv::REGION)).ok();
    let t = target::acquire_in(w, id, 512.0, region);
    c::set_pv4(w, id, pv::TARGET, t.pos);
    c::set_pv4(w, id, pv::TARGET + 0x10, t.rot);
    c::set_pv4(w, id, pv::TARGET + 0x20, t.aim);
    c::set_pv4(w, id, pv::TARGET + 0x30, t.body);
    c::set_pi32(w, id, pv::TARGET_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, pv::TARGET_KIND, t.kind as i32);
    if t.kind == 2 {
        let rail_end = if w.hero.group == 0xf {
            w.hero.boots.rail.and_then(|r| w.svc.volumes.grind_paths.get(r)).and_then(|g| g.points.last().copied())
        } else {
            None
        };
        let p = rail_end.unwrap_or_else(|| super::hero_pos(w));
        c::set_pv4(w, id, pv::TARGET, p);
    }
    if c::pi32(w, id, pv::TARGET_MOBY) == 0 {
        let h = w.hero_moby.map_or(0, |h| h as i32 + 1);
        c::set_pi32(w, id, pv::TARGET_MOBY, h);
    }
}

/// Level18 0x2fbb98 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if !ok(w, id) { return; }
    prologue(w, id);
    let st = w.m(id).state;
    match st {
        0 => {
            w.mm(id).state = 8;
            let mirror = w.rng.randi(2) != 0;
            let m = w.mm(id);
            if mirror { m.mode |= mode::MIRROR } else { m.mode &= !mode::MIRROR }
            m.mode |= mode::KEEP_ROWS;
            m.b73 = 0;
            // +0x140 = the animation table gp−0x4610 (the walker's; a pointer in the game): kept as its level address.
            c::set_pi32(w, id, pv::ANIM_TABLE, 0x16_25f0);
            c::set_pu8(w, id, 0x58, 8);
            c::set_pf(w, id, pv::D, 1.0);
            c::set_pu8(w, id, 0x5c, 1);
            c::set_pf(w, id, 0x30, 0.5);
            c::set_pi16(w, id, 0x24, 1);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::GLOW, a);
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.draw_dist = 0xff;
            m.visible = 0;
            m.mode = (m.mode & !mode::TARGETABLE) | mode::HIDDEN;
            m.has_collision = false;
        }
        1 => {
            let mut v = c::pv4(w, id, pv::VEL);
            v[2] -= DT2 * 10.0;
            c::set_pv4(w, id, pv::VEL, v);
            let p = c::add(c::pos(w, id), v);
            c::set_pos(w, id, p);
            let mut q = p;
            q[2] += 1.0;
            let g = ground::ground(w, q, 0.5, 0).z;
            if g > w.m(id).position[2] {
                w.mm(id).position[2] = g;
                w.mm(id).state = 2;
                blend(w, id, 0, 0, 10);
            } else if w.m(id).position[2] < 10.0 {
                w.mm(id).state = 6;
            }
        }
        2 => {
            if c::pi32(w, id, pv::TARGET_KIND) == 2 && c::dist2(c::pos(w, id), c::pv4(w, id, pv::TARGET)) <= 10.0 {
                tail(w, id);
                return;
            }
            w.mm(id).state = 3;
            blend(w, id, 1, 0, 10);
        }
        3 => {
            let t = c::pv4(w, id, pv::TARGET);
            let d = walk(w, id, t);
            if c::pi32(w, id, pv::TARGET_KIND) != 2 || 5.0 <= d {
                if d < 1.5 {
                    w.mm(id).state = 4;
                    blend(w, id, 2, 0, 5);
                }
            } else {
                w.mm(id).state = 2;
                blend(w, id, 0, 0, 10);
            }
        }
        4 => {
            let t = c::pv4(w, id, pv::TARGET);
            let p = c::pos(w, id);
            let a = c::atan(t[0] - p[0], t[1] - p[1]);
            turn::turn_toward_pvar(w, id, a, TAU * DT2, TAU * DT2, TAU * DT, pv::YAW_V);
            if ground::passed_frame(w, id, 14.0) { attack::joint_hit(w, 0.5, 1.0, id, 1, 1, 0, 1, 0); }
            if w.m(id).anim.flags & 2 != 0 {
                if c::pi32(w, id, pv::TARGET_KIND) != 2 {
                    if 2.5 < c::dist2(c::pos(w, id), t) {
                        w.mm(id).state = 3;
                        blend(w, id, 1, 0, 10);
                    }
                } else {
                    w.mm(id).state = 2;
                    blend(w, id, 0, 0, 10);
                }
            }
        }
        5 if knock::update(w, id, pv::K) & 0x140 != 0 => burst(w, id),
        6 => burst(w, id),
        7 if react::carried(w, id, pv::K) != 0 => {
            w.mm(id).state = 2;
            c::set_pi16(w, id, pv::CARRY, 0);
        }
        _ => {}
    }
    tail(w, id);
}

/// State 6 (module doc).
fn burst(w: &mut World, id: MobyId) {
    let mut p = c::pos(w, id);
    p[2] += 0.5;
    let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 1.0, flash2: 0.5, flash_dist: 9.0, scale: 0.5, light: 0.0, streaks: 5, sparks: 2, puffs: 4, debris: 0, sound: 4, shake: false };
    fx::beam_explosion(w, &b, Some(id), p);
    park(w, id);
}

/// The update's tail (module doc).
fn tail(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::PORT_SIZE { w.mm(id).pvars.resize(pv::PORT_SIZE, 0); }
    let r = rc_formats::moby_light::rotation_rows({ let e = w.m(id).rotation; [e[0], e[1], e[2]] });
    for (row, q) in w.mm(id).rows.iter_mut().zip(r.iter()) { *row = q.map(f32::from_bits); }
    let s = c::pf(w, id, pv::SPAWN);
    if s != 0.0 {
        let mut s = s;
        turn::approach(0.0, 4.0 * DT, &mut s);
        c::set_pf(w, id, pv::SPAWN, s);
        let m = w.mm(id);
        m.rows[2] = c::scale(m.rows[2], s + 1.0);
        m.rows[0] = c::scale(m.rows[0], 1.0 - s);
        m.rows[1] = c::scale(m.rows[1], 1.0 - s);
        // `RegisterDrawCallback(0x2fc860)`: the spawn beam ([`beam_quads`]).
        beam_corners(w, id, s);
        if let Some(r) = super::row(REFERENCE_LEVEL, BEAM_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(r), id); }
    }
    for (k, j) in (2..5).enumerate() {
        let p = w.joint_point(id, j);
        c::set_pv4(w, id, pv::JOINTS + 0x10 * k, p);
    }
}

/// `0x2fc860`'s quad, built at the registration: `f = camera (0x1677c0) − position`, `up = (0, 0, s + 1 + 1)`
/// (gp−0x45b4 1.0), `e = f × up` set to length `(1 − s) + 0.3` (gp−0x45b0); the rows (`f`, `e`, `up`, position − (0, 0,
/// 0.5) (gp−0x45b8)); corners `(0, ±1, 0 / 1)` (1 for the first two, −1 after; z 1 on odd corners): a camera-facing
/// upright quad from 0.5 below the hopper, `s + 2` tall. (`FastVecCross`'s operand order only mirrors it [L].)
fn beam_corners(w: &mut World, id: MobyId, s: f32) {
    let p = c::pos(w, id);
    let cam = w.camera_point();
    let f = [cam[0] - p[0], cam[1] - p[1], cam[2] - p[2]];
    let h = s + 1.0 + 1.0;
    let e = [f[1] * h, -f[0] * h, 0.0];
    let l = (e[0] * e[0] + e[1] * e[1]).sqrt();
    let k = if l > 0.0 { ((1.0 - s) + 0.3) / l } else { 0.0 };
    let e = [e[0] * k, e[1] * k, 0.0];
    let o = [p[0], p[1], p[2] - 0.5];
    for v in 0..4 {
        let y = if v < 2 { 1.0 } else { -1.0 };
        let z = if v & 1 == 1 { h } else { 0.0 };
        c::set_pv4(w, id, pv::BEAM + 0x10 * v, [o[0] + e[0] * y, o[1] + e[1] * y, o[2] + z, 1.0]);
    }
}

/// `0x2fc860` (draw only): the spawn beam's quad ([`beam_corners`]), FX 6 (gp−0x45bc), ALPHA 0x48 (additive), ST
/// (0,0) (0,1) (1,0) (1,1) (0x1efcd0), colour `FastTweenColor(+0x1f4, 0x000000ff, 0x8000ffff)` (gp−0x45c4 / −0x45c0).
pub fn beam_quads(table: &crate::moby_runtime::MobyTable, _svc: &crate::moby_update::Services, id: MobyId) -> Option<super::FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.o_class == CLASS && m.pvars.len() >= pv::PORT_SIZE)?;
    let s = crate::moby_update::services::pvar::ff(&m.pvars, pv::SPAWN);
    let rgba = crate::hud::tween_color(s, 0x0000_00ff, 0x8000_ffff);
    let at = |v: usize| { let q = crate::moby_update::services::pvar::v4f(&m.pvars, pv::BEAM + 0x10 * v); [q[0], q[1], q[2]] };
    let quad = super::FxQuad { corners: [at(0), at(1), at(2), at(3)], st: [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]], rgba: [rgba; 4] };
    Some(super::FxQuads { fx: 6, additive: true, subtract: false, quads: vec![quad] })
}

/// `0x2fca98` (draw only): each member of the registering hopper's group of class 1906 in states 1..5: glow quads
/// (`0x265e28` = L01 `0x2781d0`) of size 0.175, pull 0.08 at its joints 2, 3, 4, colour `(+0x90 & 0xffffff) +
/// (0x20 << 24)` (gp−0x45ac).
pub fn glow_quads(table: &crate::moby_runtime::MobyTable, svc: &crate::moby_update::Services, id: MobyId) -> Vec<super::GlowQuad> {
    let Some(me) = table.mobys.get(id) else { return Vec::new() };
    let Some(Some(list)) = svc.groups.lists.get(me.group as u8 as usize) else { return Vec::new() };
    let mut out = Vec::new();
    for &e in list {
        let Some(m) = table.mobys.get((e & 0x7fff) as usize) else { continue };
        if m.o_class != CLASS || m.state == 0 || 6 <= m.state || m.pvars.len() < pv::PORT_SIZE { continue; }
        let rgba = (m.glow & 0xff_ffff).wrapping_add(0x20 << 24);
        for k in 0..3 {
            let q = crate::moby_update::services::pvar::v4f(&m.pvars, pv::JOINTS + 0x10 * k);
            out.push(super::GlowQuad { size: f32::from_bits(0x3e33_3333), pull: f32::from_bits(0x3da3_d70a), point: [q[0], q[1], q[2]], rgba });
        }
    }
    out
}
