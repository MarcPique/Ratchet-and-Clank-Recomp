//! **Batalia's tanks** (class 253, level08 `0x2d42d8`, its prologue `0x2d4ca0`, drive `0x2d55c8`, smoke `0x2d5950`,
//! wall break `0x2d5d08` / `0x2d6450`; census U292, 4 placed), **their treads 331** (`0x2da3c0`: the code of level 18's
//! `veldin_tank::tread_update`) and **their shells 425** (`0x2dc9a8`, made by `0x2dc8a0`). A tank waits hidden
//! behind a wall (until Ratchet enters its region: it breaks through, the wall's pieces flying), behind a bolt crank's
//! gate (until the crank is wound fully: it appears at its gate) or starts on its path; it patrols its path, and with
//! Ratchet within 20 in its region turns its turret on him and fires from either barrel at animation keys 4 and 10,
//! lobbing bouncing shells. Hits flash it; its first death blows its turret off (the turret's manipulator scaled to
//! 0, five pieces) and leaves a smoking wreck still driving its path; the wreck's death blows it up (five pieces, the
//! treads gone). Tanks 1 and 2 destroyed by class 1633 award skill point 13. Read from the level08
//! decomp and data. Native `f32`.
//!
//! **Pvars** (0x190): +0x20 the damage record (health 9, +0x28 3, +0x2a 0xc, +0x2e 1 (2 → deleted), +0x30 3, +0x3e
//! \| 2; +0x26 the hit's flash ticks), +0x40 (zeroed; read as the tank's motion by the hum and the smoke but never
//! written: the hum never starts [the game's]), +0x60 the riders' block, +0xa0 the flash, +0xb0 the target record
//! (+0xf0 the moby, +0xf4 the kind), +0x100 the turret's manipulator (joint list 0), +0x140 the bolt crank, +0x144 a
//! member of the wall's group, +0x148 the wall's debris cuboid, +0x14c the gate path, +0x150 the drive path, +0x154
//! the region path, +0x158 the body's turn speed, +0x15c the hum's slot, +0x160 / +0x164 the treads (index + 1),
//! +0x168 the path point, +0x16c the drive speed (2·dt), +0x170 / +0x174 the turret's yaw (from the body) and its
//! speed, +0x178 / +0x17c its pitch, +0x180 the speed, +0x184 its number (1..4), +0x188 the hum's volume (the game
//! reads it back from the slot: kept here [L]).
//!
//! | address | what | port |
//! |---|---|---|
//! | top | +0x2e = 2 → the treads and the tank deleted | [`update`] |
//! | `0x2d4ca0` | the hits of the tank and its treads (`MobyGetHitMessage` 0x330000 / 0x210000; the heaviest, `0x269c70`; its own shells 425 and the wrench 0x47 ignored); `0x26f378(…, +0x20, 0, 4)`: a hit (state ≠ 0): health −= damage, sound 4; alive: flash 0x78 for `ticks(60)` (classes 0xb0 / 0xb1) or `ticks(30)` under 4 damage, else 200 / 0xe6; state 0xc → sequence 1; dead in 99: the skill point (level 8, tank +0x184, class 0x661: 0x161590.. flags, tank 2 → skill point 13), `SetDeathBits`, sound 2, `SpawnBeamExplosion(0, 0, 4, 2, 9, 1, 15; 20 streaks, 8 sparks, 20 puffs, shake)` 1 up, pieces 0x6fe ×2, 0x6ff ×3, all deleted; dead first: flash 0xfa, the turret's scale 0, the explosion 3 up, pieces 0x6c0..0x6c4, sequence 1, → 99, health 6, +0x3e &= ~2; the flash | [`prologue`] |
//! | | +0xa4 = 0xff (all three); the target in the region path (`0x274df8(100)`; none → Ratchet's moby, the kind kept); the turret: kind 2 → yaw / pitch to 0, else d = target − position, z − 3, yaw = atan d − body yaw, pitch = −atan(\|d.xy\|, d.z) in [−45°, 20°] (`0x270cc0`: 4π·dt², 4π·dt², 3π·dt); the record = `Q(pitch, y)·Q(yaw, z)`; the flash on the tank and its treads | [`prologue`] (`target::acquire_in`, `manip::set_quat`) |
//! | 0 | the treads (`CreateMoby(0x14b)`, 0x40, drawn, Ratchet's light / ambient, mode 0x4000 / 0xc000, collision off, sequence 0); the manipulator; crank → 0xb (hidden, frozen, not targetable, collision off); no wall → on the drive path's first point, → 6; the wall standing (0x13d3b6 clear) → 0xe (hidden); fallen → its pieces deleted (`0x2d6450`), on the path, → 6; the record (health 9 …) | [`update`] |
//! | 0xb | the crank at 1 → on the gate path's first point, shown; collision on, targetable; a target → 6, sequence 1 | [`update`] |
//! | 0xe | a target → the wall breaks (`0x2d5d08`), sound 6 unless the first, 0x13d3b6 = 1; shown, → 6 | [`wall_break`] |
//! | 6 | the drive; a target within 20 → the help (record 0x5a count 0, item 11 not owned: `Help_Request(0x1f44, 0x5a)`), sequence 2, → 3 | [`update`] |
//! | 3 | the drive; no target or beyond 22 → 6, sequence 1; keys 4 / 10 passed → a shell from joint list 1 (at key 4) or 2, `20·dt` along the turret (`0x2dc8a0`) | [`fire`] |
//! | 99 | the drive (the wreck); keys 30 / 90 → smoke at joint list 5, 60 / 120 → at 4 (10 puffs each) | [`smoke`] |
//! | tail | `CarryRiders(+0x60, position − old, Euler, old Euler)` (the game's order) | [`update`] |
//! | `0x2d55c8` | the point (+0x168); yaw turned to it (`0x270cc0`: 1.5π·dt², 1.5π·dt², π·dt); speed `Approach(+0x16c, 2·dt², +0x180)`; moved along the yaw; z = `GroundHeight(0.5)`; within 0.5 → the next point; the treads: anim speed `clamp(speed / (4·dt), 0, 2)`, the tank's Euler and position; the hum by +0x40 (sound 1, faded over `ticks(60)`) | [`drive`] |
//! | `0x2d5950(m, list, n)` | n puffs (type 2) at the joint: velocity the joint's z axis at `randf(0, 1.5)·dt` (the last two thirds `randf(1.5, 1.8)·dt`) + +0x40, the second half of +0x40 plus a drift `randf(0, 0.25)` / `randf(0, 1)`·dt round, 0.5·dt up; colours 0x20405048 / 0x20607068, sizes 0.1 / 0.2, lives `randf(15, 18)`, `randf(15, 18)`, `randf(30, 45)` ticks | [`smoke`] |
//! | `0x2d5d08` | with 50 free moby slots: up to 40 pieces (`CreateMoby(0x310 / 0x311)`, a random Euler, in the cuboid +0x148 at random, flung 15·dt within ±15° of the x axis, up `randf(8, 12)·dt`, `ticks(randf(60, 90))`); the group of +0x144: the loose pieces 588–598 flung from their own points (the table), up `randf(8, 12)·dt`, life 10..20 (facing the camera within 15°) or 30..45; 472 explodes (`SpawnBeamExplosion(0, 0, 12, 8, 9, 1, 15; 50, 60, 60, shake)`) and is deleted | [`wall_break`] (`loose_piece::fling`) |
//! | `0x2dc9a8` (425) | the 0.5 sphere (`coll_sphere_mobys`): a moby other than the owner, or any after a bounce → off; flying: position += v, v.z −= 10.8·dt², the line (hit template damage 1, flags 0x810000): a hit not on the owner → off at the point, bounced at 2·dt; the timer (300) out → off; off: a 1.0 area hit (flags 9, damage 1, 0x810001), sound 0, 50 type-16 puffs (kinds by turn), deleted | [`shell_update`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, ground, target, turn, DT, DT2, V};
use crate::moby_update::manip;
use crate::moby_update::services::{self as sv, pvar as p, HitTemplate, World};
use crate::moby_update::story;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 8;
pub const UPDATE_FN: u32 = 0x2d_42d8;
pub const CLASSES: [i16; 1] = [253];
pub const TREAD_FN: u32 = 0x2d_a3c0;
pub const TREAD_CLASSES: [i16; 1] = [TREAD];
pub const SHELL_FN: u32 = 0x2d_c9a8;
pub const SHELL_CLASSES: [i16; 1] = [SHELL];

const TREAD: i16 = 0x14b;
const SHELL: i16 = 0x1a9;
const WRENCH: i16 = 0x47;
const TURRET_PIECES: [i16; 5] = [0x6c0, 0x6c1, 0x6c2, 0x6c3, 0x6c4];
const WRECK_PIECES: [i16; 5] = [0x6fe, 0x6fe, 0x6ff, 0x6ff, 0x6ff];
/// gp−0x5360 / −0x5358 / −0x5334 / −0x5330: the tread rate, the shell speed, two bytes of the record.
const TREAD_RATE: f32 = 4.0;
const SHELL_SPEED: f32 = 20.0;
const B58: f32 = 14.0;
const B5A: f32 = 1.7;
/// 0x13d3b6: the wall has fallen.
const WALL_FLAG: u32 = 0x13_d3b6;

mod pv {
    pub const LEN: usize = 0x190;
    pub const D: usize = 0x20;
    pub const MOTION: usize = 0x40;
    pub const RIDERS: usize = 0x60;
    pub const F: usize = 0xa0;
    pub const T_POS: usize = 0xb0;
    pub const T_MOBY: usize = 0xf0;
    pub const T_KIND: usize = 0xf4;
    pub const MANIP: usize = 0x100;
    pub const CRANK: usize = 0x140;
    pub const WALL: usize = 0x144;
    pub const DEBRIS: usize = 0x148;
    pub const GATE: usize = 0x14c;
    pub const PATH: usize = 0x150;
    pub const REGION: usize = 0x154;
    pub const BODY_V: usize = 0x158;
    pub const HUM: usize = 0x15c;
    pub const TREADS: usize = 0x160;
    pub const NODE: usize = 0x168;
    pub const DRIVE: usize = 0x16c;
    pub const YAW: usize = 0x170;
    pub const YAW_V: usize = 0x174;
    pub const PITCH: usize = 0x178;
    pub const PITCH_V: usize = 0x17c;
    pub const SPEED: usize = 0x180;
    pub const NUMBER: usize = 0x184;
    pub const HUM_VOL: usize = 0x188;
}

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }
fn treads(w: &World, id: MobyId) -> [Option<MobyId>; 2] { [link(w, id, pv::TREADS), link(w, id, pv::TREADS + 4)] }
fn blend(w: &mut World, id: MobyId, seq: u8, n: i32) {
    if w.m(id).anim.seq_b == seq { return; }
    let t = w.ticks(n);
    w.anim_blend(id, seq, 0, t);
}
fn path_points(w: &World, id: MobyId, o: usize) -> Option<Vec<[f32; 4]>> { usize::try_from(c::pi32(w, id, o)).ok().and_then(|i| w.svc.volumes.paths.get(i)).cloned() }
fn path_start(w: &World, id: MobyId, o: usize) -> Option<V> { path_points(w, id, o).and_then(|p| p.first().copied()) }
fn set_all(w: &mut World, id: MobyId, f: impl Fn(&mut crate::moby_runtime::Moby)) {
    f(w.mm(id));
    for t in treads(w, id).into_iter().flatten() { f(w.mm(t)); }
}
fn place_all(w: &mut World, id: MobyId, at: V) {
    c::set_pos(w, id, at);
    for t in treads(w, id).into_iter().flatten() { w.mm(t).position = at; }
}

/// Level08 `0x2d42d8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pv::LEN);
    if c::pu8(w, id, pv::D + 0xe) == 2 {
        for t in treads(w, id).into_iter().flatten() { w.delete_moby(t); }
        w.delete_moby(id);
        return;
    }
    let (start, rot0) = (c::pos(w, id), w.m(id).rotation);
    if !prologue(w, id) { return; }
    let kind = c::pi32(w, id, pv::T_KIND);
    let tpos = c::pv4(w, id, pv::T_POS);
    let tm = link(w, id, pv::T_MOBY);
    let tm_pos = tm.map_or(tpos, |m| w.m(m).position);
    match w.m(id).state {
        0 => init(w, id),
        0xb => {
            let crank = usize::try_from(c::pi32(w, id, pv::CRANK)).ok().and_then(|k| w.table.mobys.get(k));
            if crank.is_some_and(|m| m.pvars.len() >= 4 && p::ff(&m.pvars, 0) == 1.0) {
                if let Some(at) = path_start(w, id, pv::GATE) { place_all(w, id, at); }
                set_all(w, id, |m| m.mode &= !0x41);
                appear(w, id, kind);
            }
        }
        0xe => {
            if kind != 2 {
                wall_break(w, id);
                if story::flag(w, story::flag_index(WALL_FLAG)) != 0 { w.play_sound(6, 0, id); }
                story::set_flag(w, story::flag_index(WALL_FLAG), 1);
                let at = c::pos(w, id);
                place_all(w, id, at);
                set_all(w, id, |m| m.mode &= !0x41);
                appear(w, id, kind);
            }
        }
        3 => {
            drive(w, id);
            if kind == 2 || 22.0 < c::dist2(c::pos(w, id), tm_pos) {
                w.mm(id).state = 6;
                blend(w, id, 1, 10);
            }
            if ground::passed_frame(w, id, 4.0) || ground::passed_frame(w, id, 10.0) { fire(w, id); }
        }
        6 => {
            drive(w, id);
            if kind != 2 && c::dist2(c::pos(w, id), tm_pos) < 20.0 {
                if w.svc.help.records.help[0x5a].count == 0 && !super::hints::owned(w, 11) { w.svc.help.request(0x1f44, 0x5a); }
                w.mm(id).state = 3;
                blend(w, id, 2, 10);
            }
        }
        99 => {
            drive(w, id);
            let k = ground::key_time(w, id);
            if (59.0..=60.0).contains(&k) || (119.0..=120.0).contains(&k) {
                smoke(w, id, 4, 10);
            } else if (29.0..=30.0).contains(&k) || (89.0..=90.0).contains(&k) {
                smoke(w, id, 5, 10);
            }
        }
        _ => {}
    }
    let now = c::pos(w, id);
    let delta = c::sub(now, start);
    let rot = w.m(id).rotation;
    crate::moby_update::triggers::carry_riders(&mut w.mm(id).pvars, pv::RIDERS, delta, rot, rot0);
}

/// `LAB_002d4aac`: collision on (all three), targetable; a target → 6 and sequence 1.
fn appear(w: &mut World, id: MobyId, kind: i32) {
    for m in std::iter::once(Some(id)).chain(treads(w, id)).flatten() {
        let has = super::class_collision(w, w.m(m).o_class);
        w.mm(m).has_collision = has;
    }
    w.mm(id).mode |= mode::TARGETABLE;
    if kind == 2 { return; }
    w.mm(id).state = 6;
    blend(w, id, 1, 10);
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    for k in 0..2 {
        if link(w, id, pv::TREADS + 4 * k).is_none() { make_tread(w, id, k); }
    }
    manip::attach(w, id, 0, id, pv::MANIP);
    w.mm(id).mode |= mode::TARGETABLE;
    c::set_pf(w, id, pv::DRIVE, DT + DT);
    let hide = |w: &mut World, id: MobyId, untarget: bool| {
        set_all(w, id, |m| {
            m.mode |= 0x41;
            m.has_collision = false;
        });
        if untarget { w.mm(id).mode &= !mode::TARGETABLE; }
        if w.m(id).anim.seq_b != 0 { w.anim_blend(id, 0, 0, 0); }
    };
    if c::pi32(w, id, pv::CRANK) < 1 {
        if c::pi32(w, id, pv::WALL) < 1 {
            on_path(w, id);
        } else if story::flag(w, story::flag_index(WALL_FLAG)) == 0 {
            w.mm(id).state = 0xe;
            hide(w, id, false);
        } else {
            clear_wall(w, id);
            on_path(w, id);
        }
    } else {
        w.mm(id).state = 0xb;
        hide(w, id, true);
    }
    {
        let pvs = &mut w.mm(id).pvars;
        p::set_v4f(pvs, pv::MOTION, [0.0; 4]);
        p::set_u8(pvs, pv::D + 9, 0);
        p::set_u8(pvs, pv::D + 8, 3);
        p::set_ff(pvs, pv::D + 0x10, 3.0);
        p::set_u8(pvs, pv::D + 0xa, 0xc);
        p::set_u8(pvs, pv::D + 0xe, 1);
        p::set_ff(pvs, pv::D, 9.0);
    }
    let f = c::pi16(w, id, pv::D + 0x1e) as u16 | 2;
    c::set_pi16(w, id, pv::D + 0x1e, f as i16);
    c::set_pf(w, id, pv::PITCH, 0.0);
    let yaw = w.m(id).rotation[2];
    c::set_pf(w, id, pv::YAW, yaw);
    c::set_pu8(w, id, 0x58, B58 as i32 as u8);
    c::set_pu8(w, id, 0x5a, (B5A * 8.0) as i32 as u8);
}

fn on_path(w: &mut World, id: MobyId) {
    if let Some(at) = path_start(w, id, pv::PATH) { c::set_pos(w, id, at); }
    w.mm(id).state = 6;
    if w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 0); }
}

/// A tread (`CreateMoby(0x14b)`): `k` 0 → +0x160 (mode 0x4000), 1 → +0x164 (mode 0xc000).
fn make_tread(w: &mut World, id: MobyId, k: usize) {
    let Some(t) = w.create_moby(TREAD) else { return };
    let (light, ambient) = w.hero_moby.map_or((0, [0x40; 4]), |h| (w.m(h).light, w.m(h).ambient));
    let pos = w.m(id).position;
    {
        let m = w.mm(t);
        m.update_dist = 0x40;
        m.draw_dist = 0x40;
        m.visible = 1;
        m.light = light;
        m.ambient = ambient;
        m.mode = if k == 0 { 0x4000 } else { 0xc000 };
        m.position = pos;
        m.has_collision = false;
    }
    w.anim_blend(t, 0, 0, 0);
    w.build_matrix(t);
    c::set_pi32(w, id, pv::TREADS + 4 * k, t as i32 + 1);
}

/// The heaviest of the hits (`0x269c70`: the later of equal damage wins).
fn heaviest(hits: [Option<sv::HitRecord>; 3]) -> Option<sv::HitRecord> {
    let mut best: Option<sv::HitRecord> = None;
    for h in hits.into_iter().flatten() {
        match &best {
            Some(b) if f32::from_bits(h.damage.0) < f32::from_bits(b.damage.0) => {}
            _ => best = Some(h),
        }
    }
    best
}

/// `0x2d4ca0` (module doc); false when the tank is gone.
fn prologue(w: &mut World, id: MobyId) -> bool {
    let tr = treads(w, id);
    let own = w.get_hit(id, 0x33_0000, false);
    let h1 = tr[0].and_then(|t| w.get_hit(t, 0x21_0000, false));
    let h2 = tr[1].and_then(|t| w.get_hit(t, 0x21_0000, false));
    let mut hit = heaviest([own, h1, h2]);
    let by = hit.as_ref().and_then(|h| h.attacker).map(|a| w.m(a).o_class);
    if matches!(by, Some(SHELL) | Some(WRENCH)) { hit = None; }
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && w.m(id).state != 0 {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        if res.damage != 0.0 { w.play_sound(4, 0, id); }
        if 0.0 < hp {
            let n = if res.damage < 4.0 {
                c::set_pu8(w, id, pv::F + 7, 0x78);
                if by.is_some_and(|k| (0xb0..=0xb1).contains(&k)) { 0x3c } else { 0x1e }
            } else {
                c::set_pu8(w, id, pv::F + 7, if res.damage < 6.0 { 200 } else { 0xe6 });
                0x1e
            };
            let t = w.ticks(n);
            c::set_pi16(w, id, pv::D + 6, t as i16);
            if w.m(id).state == 0xc && w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 0); }
        } else if w.m(id).state == 99 {
            skill_point(w, id, by);
            blow_up(w, id);
            return false;
        } else {
            turret_off(w, id);
        }
        flash::start(w, id, pv::F);
    }
    w.mm(id).hit_slot = 0xff;
    for t in tr.into_iter().flatten() { w.mm(t).hit_slot = 0xff; }
    let region = usize::try_from(c::pi32(w, id, pv::REGION)).ok();
    let t = target::acquire_in(w, id, 100.0, region);
    {
        let pvs = &mut w.mm(id).pvars;
        p::set_v4f(pvs, pv::T_POS, t.pos);
        p::set_u32(pvs, pv::T_KIND, t.kind);
    }
    let tm = t.moby.or(w.hero_moby);
    c::set_pi32(w, id, pv::T_MOBY, tm.map_or(0, |m| m as i32 + 1));
    let (acc, vmax) = (DT2 * 12.566_371, DT * 9.424_778);
    let (yaw_t, pitch_t) = if t.kind == 2 {
        (0.0, 0.0)
    } else {
        let me = c::pos(w, id);
        let mut d = c::sub(t.pos, me);
        d[2] -= 3.0;
        let yaw = c::atan(d[0], d[1]);
        let pitch = (-c::atan((d[0] * d[0] + d[1] * d[1]).sqrt(), d[2])).clamp(-std::f32::consts::FRAC_PI_4, 0.349_065_84);
        (c::sub_rot(yaw, w.m(id).rotation[2]), pitch)
    };
    turn_pvar(w, id, yaw_t, acc, vmax, pv::YAW, pv::YAW_V);
    turn_pvar(w, id, pitch_t, acc, vmax, pv::PITCH, pv::PITCH_V);
    let (yaw, pitch) = (c::pf(w, id, pv::YAW), c::pf(w, id, pv::PITCH));
    let q = sv::quat_mul(sv::axis_quat(Pf::f(pitch), 1), sv::axis_quat(Pf::f(yaw), 2));
    manip::set_quat(w, id, id, pv::MANIP, sv::fv(q));
    let active = c::pi16(w, id, pv::F) != 0;
    flash::update(w, id, pv::F);
    if active {
        let a = w.m(id).ambient;
        for t in treads(w, id).into_iter().flatten() { w.mm(t).ambient = a; }
    }
    true
}

/// `0x270cc0(target, acc, acc, vmax, &+a, &+v)` on two pvar floats.
fn turn_pvar(w: &mut World, id: MobyId, target: f32, acc: f32, vmax: f32, a: usize, v: usize) {
    let (mut x, mut vel) = (c::pf(w, id, a), c::pf(w, id, v));
    turn::turn_toward(target, acc, acc, vmax, &mut x, &mut vel);
    c::set_pf(w, id, a, x);
    c::set_pf(w, id, v, vel);
}

/// The skill point of the death in state 99 (module doc).
fn skill_point(w: &mut World, id: MobyId, by: Option<i16>) {
    let n = c::pi32(w, id, pv::NUMBER);
    if w.svc.level != 8 || n == 0 || by != Some(0x661) { return; }
    if !(1..=4).contains(&n) { return; }
    // gp−0x7650.. (0x15f5b0..0x15f5bc): the tanks destroyed this way; tanks 1 and 2 → the skill point (0x15f5b0 is
    // also the front end's exit flag; the port keeps the tanks' words apart) [L].
    w.svc.units.set_word(0x15_f5b0 + 4 * (n as u32 - 1), 1);
    if w.svc.units.word(0x15_f5b0) != 0 && w.svc.units.word(0x15_f5b4) != 0 { story::award_skill_point(w, 13); }
}

/// The wreck's death (module doc).
fn blow_up(w: &mut World, id: MobyId) {
    let me = c::pos(w, id);
    let up = [me[0], me[1], me[2] + 1.0, me[3]];
    set_death_bits(w, id, 0, -1);
    w.play_sound(2, 0, id);
    let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 20, sparks: 8, puffs: 20, debris: 0, sound: -1, shake: true };
    fx::beam_explosion(w, &b, Some(id), up);
    let rot = w.m(id).rotation;
    for class in WRECK_PIECES { fx::break_piece(w, id, class, me, rot, 0, 0); }
    let tr = treads(w, id);
    for t in tr.into_iter().flatten() { w.delete_moby(t); }
    w.delete_moby(id);
}

/// The first death: the turret blown off (module doc).
fn turret_off(w: &mut World, id: MobyId) {
    let me = c::pos(w, id);
    let up = [me[0], me[1], me[2] + 3.0, me[3]];
    c::set_pu8(w, id, pv::F + 7, 0xfa);
    p::set_v4f(&mut w.mm(id).pvars, pv::MANIP + manip::rec::SCALE, [0.0; 4]);
    manip::sync(w, id, id, pv::MANIP);
    let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 20, sparks: 8, puffs: 20, debris: 0, sound: -1, shake: true };
    fx::beam_explosion(w, &b, Some(id), up);
    let rot = w.m(id).rotation;
    for class in TURRET_PIECES { fx::break_piece(w, id, class, me, rot, 0, 0); }
    if w.m(id).anim.seq_b != 1 {
        let t = w.ticks(3);
        w.anim_blend(id, 1, 0, t);
    }
    w.mm(id).state = 99;
    c::set_pf(w, id, pv::D, 6.0);
    let f = c::pi16(w, id, pv::D + 0x1e) as u16 & !2;
    c::set_pi16(w, id, pv::D + 0x1e, f as i16);
}

/// `0x2d55c8(m, path)` (module doc).
fn drive(w: &mut World, id: MobyId) {
    let Some(pts) = path_points(w, id, pv::PATH).filter(|p| !p.is_empty()) else { return };
    let n = pts.len() as i32;
    let k = c::pi32(w, id, pv::NODE).rem_euclid(n);
    let node = pts[k as usize];
    let me = c::pos(w, id);
    let yaw_t = c::atan(node[0] - me[0], node[1] - me[1]);
    {
        let mut yaw = w.m(id).rotation[2];
        let mut v = c::pf(w, id, pv::BODY_V);
        let a = DT2 * 4.712_389;
        turn::turn_toward(yaw_t, a, a, DT * std::f32::consts::PI, &mut yaw, &mut v);
        w.mm(id).rotation[2] = yaw;
        c::set_pf(w, id, pv::BODY_V, v);
    }
    let mut speed = c::pf(w, id, pv::SPEED);
    turn::approach(c::pf(w, id, pv::DRIVE), DT2 + DT2, &mut speed);
    c::set_pf(w, id, pv::SPEED, speed);
    let yaw = w.m(id).rotation[2];
    let mut p = c::add(me, [yaw.cos() * speed, yaw.sin() * speed, 0.0, 0.0]);
    p[2] = ground::ground(w, p, 0.5, 0).z;
    c::set_pos(w, id, p);
    if c::dist2(p, node) < 0.5 { c::set_pi32(w, id, pv::NODE, (k + 1) % n); }
    let f = (speed / (TREAD_RATE * DT)).clamp(0.0, 2.0);
    let (rot, pos) = (w.m(id).rotation, w.m(id).position);
    for t in treads(w, id).into_iter().flatten() {
        let m = w.mm(t);
        m.anim.speed = f;
        m.rotation = rot;
        m.position = pos;
    }
    // The hum by the never-written +0x40 (module doc).
    let slot = c::pi32(w, id, pv::HUM);
    let step = 0x400 / w.ticks(0x3c).max(1);
    let vol = c::pi32(w, id, pv::HUM_VOL);
    if DT * 0.1 < c::len3(c::pv4(w, id, pv::MOTION)) {
        if !w.sound_alive(slot, id) {
            let s = w.play_sound(1, 4, id);
            c::set_pi32(w, id, pv::HUM, s);
            set_hum(w, id, s, 1);
        } else if vol < 0x400 {
            set_hum(w, id, slot, vol + step);
        }
    } else if w.sound_alive(slot, id) {
        if vol < 0x100 {
            w.release_sound(slot, id);
            c::set_pi32(w, id, pv::HUM, -1);
        } else {
            set_hum(w, id, slot, vol - step);
        }
    }
}

fn set_hum(w: &mut World, id: MobyId, slot: i32, v: i32) {
    w.set_volume(slot, v);
    c::set_pi32(w, id, pv::HUM_VOL, v);
}

/// State 3's shot (module doc).
fn fire(w: &mut World, id: MobyId) {
    let list = if ground::key_time(w, id) == 4.0 { 1 } else { 2 };
    let at = w.joint_point(id, list);
    let k = SHELL_SPEED * DT;
    let yaw = c::add_rot(c::pf(w, id, pv::YAW), w.m(id).rotation[2]);
    let pitch = c::pf(w, id, pv::PITCH);
    let v = [yaw.cos() * pitch.cos() * k, yaw.sin() * pitch.cos() * k, -pitch.sin() * k, 0.0];
    if let Some(s) = spawn_shell(w, id, at, v) { w.mm(s).state = 1; }
}

/// `0x2d5950(m, list, n)` (module doc).
fn smoke(w: &mut World, id: MobyId, list: usize, n: i32) {
    let at = w.joint_point(id, list);
    let up = w.joint_matrix(id, list)[2];
    let motion = c::pv4(w, id, pv::MOTION);
    for i in 0..n {
        let a = w.rng.rand_angle();
        // A jittered copy of the point (±0.05 a lane) the spawn never reads (the game's).
        for _ in 0..3 { w.rng.randf(-0.05, 0.05); }
        let late = n / 3 < i;
        let s = if late { w.rng.randf(1.5, 1.5 * 1.2) } else { w.rng.randf(0.0, 1.5) };
        let d = c::set_len3(up, s * DT);
        let v1 = c::add(motion, d);
        let mut v2 = c::scale(motion, 0.5);
        let r = if late { 1.0 } else { 0.25 };
        let rx = w.rng.randf(0.0, r);
        v2[0] += a.cos() * rx * DT;
        let ry = w.rng.randf(0.0, r);
        v2[1] += a.sin() * ry * DT;
        v2[2] = 0.5 * DT;
        let l1 = w.rng.randf(15.0, 18.0);
        let l2 = w.rng.randf(15.0, 18.0);
        let l3 = w.rng.randf(30.0, 45.0);
        let t = [l1, l2, l3].map(|x| w.svc.timing.scale(Pf::f(x)).to_f32() as i32);
        let s = crate::particles::type02::Spawn { pos: at, v1: [v1[0], v1[1], v1[2], 0.1], v2: [v2[0], v2[1], v2[2], 0.2], c1: 0x2040_5048, c2: 0x2060_7068, t, def: -1 };
        fx::part02(w, &s);
    }
}

/// The table of `0x2d5d08`: the loose pieces' fling points (classes 588..598).
const FLING: [[f32; 3]; 11] = [
    [1.629, -1.341, 11.48],
    [1.299, -5.558, 11.48],
    [0.629, -2.527, 3.409],
    [0.019, -6.19, 3.409],
    [3.196, -2.105, 7.784],
    [2.11, -3.013, 2.625],
    [4.33, -6.449, 7.397],
    [3.286, -6.925, 2.96],
    [1.539, -1.071, 13.144],
    [1.596, -2.512, 8.359],
    [0.885, -5.824, 8.224],
];

fn group_of(w: &World, id: MobyId) -> Option<Vec<MobyId>> {
    let m = usize::try_from(c::pi32(w, id, pv::WALL)).ok()?;
    let g = usize::try_from(w.table.mobys.get(m)?.group).ok()?;
    Some(w.svc.groups.lists.get(g)?.as_ref()?.iter().map(|&x| x as usize).collect())
}

/// `0x2d5d08` (module doc).
fn wall_break(w: &mut World, id: MobyId) {
    let Some(list) = group_of(w, id) else { return };
    let cub = c::pi32(w, id, pv::DEBRIS);
    if 0x31 < w.table.free_slots {
        for _ in 0..0x28 {
            let class = if w.rng.randi(0xff) & 1 != 0 { 0x310 } else { 0x311 };
            if let Some(m) = w.create_moby(class) {
                let (a, b, cc) = (w.rng.rand_angle(), w.rng.rand_angle(), w.rng.rand_angle());
                let (x, y, z) = (w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0));
                let at = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, cub).map_or([x, y, z], |s| s.world([x, y, z]));
                {
                    let mo = w.mm(m);
                    mo.update_dist = 0xff;
                    mo.draw_dist = 0x40;
                    mo.visible = 1;
                    mo.rotation = [a, b, cc, 0.0];
                    mo.position = [at[0], at[1], at[2], 1.0];
                }
                let d = w.rng.randf(f32::from_bits(0xbe86_0a92), f32::from_bits(0x3e86_0a92));
                let up = w.rng.randf(DT * 8.0, DT * 12.0);
                let v = [d.cos() * DT * 15.0, d.sin() * DT * 15.0, up, 0.0];
                let life = w.rng.randf(60.0, 90.0) as i32;
                super::loose_piece::fling(w, m, v, [0.0; 4], life);
            }
            if w.table.free_slots <= 0x31 { break; }
        }
    }
    for m in list {
        if m == id || w.table.mobys.get(m).is_none_or(|x| 0x7f <= x.state) { continue; }
        let class = w.m(m).o_class;
        let k = class - 0x24c;
        if !(0..=10).contains(&k) {
            if class == 0x1d8 {
                let at = c::pos(w, m);
                let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 12.0, flash2: 8.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 0x32, sparks: 0x3c, puffs: 0x3c, debris: 0, sound: -1, shake: true };
                fx::beam_explosion(w, &b, Some(m), at);
                w.delete_moby(m);
            }
            continue;
        }
        let d = w.rng.randf(f32::from_bits(0xbe86_0a92), f32::from_bits(0x3e86_0a92));
        let up = w.rng.randf(DT * 8.0, DT * 12.0);
        let v = [d.cos() * DT * 15.0, d.sin() * DT * 15.0, up, 0.0];
        let f = FLING[k as usize];
        let off = [f[0], f[1], f[2], 0.0];
        let me = c::pos(w, id);
        let k_cam = w.camera.map(|x| x.to_f32());
        let facing = c::diff_rots(c::atan(v[0], v[1]), c::atan(k_cam[0] - me[0], k_cam[1] - me[1])) < 0.261_799_4;
        let life = if facing { w.rng.randf(10.0, 20.0) } else { w.rng.randf(30.0, 45.0) } as i32;
        super::loose_piece::fling(w, m, v, off, life);
    }
}

/// `0x2d6450`: the fallen wall's pieces (588–598, 472) deleted.
fn clear_wall(w: &mut World, id: MobyId) {
    let Some(list) = group_of(w, id) else { return };
    for m in list {
        if m == id || w.table.mobys.get(m).is_none_or(|x| 0x7f <= x.state) { continue; }
        let class = w.m(m).o_class;
        if (0x24c..=0x256).contains(&class) || class == 0x1d8 { w.delete_moby(m); }
    }
}

// -------------------------------------------------------------------------------------------------
// The shell 425

/// `0x2dc8a0(m, p, v)`: `CreateMoby(0x1a9)`.
fn spawn_shell(w: &mut World, owner: MobyId, at: V, v: V) -> Option<MobyId> {
    let s = w.create_moby(SHELL)?;
    if w.m(s).pvars.len() < 0x24 { w.mm(s).pvars.resize(0x24, 0); }
    {
        let m = w.mm(s);
        m.draw_dist = 0xff;
        m.update_dist = 0xff;
        m.state = 1;
        m.visible = 1;
        m.position = at;
    }
    c::set_pi32(w, s, 0x20, owner as i32 + 1);
    let t = w.ticks(300);
    c::set_pi32(w, s, 0x10, t);
    c::set_pf(w, s, 0x1c, 1.0);
    c::set_pi32(w, s, 0x14, 0);
    c::set_pv4(w, s, 0, v);
    let rx = w.rng.randf(-180.0, 180.0) * 0.017_453_292;
    w.mm(s).rotation[0] = rx;
    w.mm(s).rotation[2] = c::atan(v[0], v[1]) + std::f32::consts::FRAC_PI_2;
    w.build_matrix(s);
    Some(s)
}

/// Level08 `0x2dc9a8` (module doc).
pub fn shell_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x24 { w.mm(id).pvars.resize(0x24, 0); }
    let owner = usize::try_from(c::pi32(w, id, 0x20) - 1).ok();
    let pos = c::pos(w, id);
    if let Some(&m) = w.sphere_mobys_list(Pf::f(0.5), sv::pv(pos), 0, Some(id), None).first() {
        if Some(m) != owner || c::pi32(w, id, 0x14) != 0 { w.mm(id).state = 99; }
    }
    match w.m(id).state {
        1 => {
            let mut v = c::pv4(w, id, 0);
            let to = c::add(pos, v);
            c::set_pos(w, id, to);
            v[2] -= DT2 * 10.8;
            c::set_pv4(w, id, 0, v);
            let tmpl = HitTemplate { dir: sv::pv(v), attacker: Some(id), flags: 0x81_0000, damage: Pf::ONE, w20: 1, ..Default::default() };
            let hit = sv::line_hit_in(w.table, w.svc, w.classes, w.coll, sv::pv(pos), sv::pv(to), 0, Some(id), &tmpl);
            match hit {
                None => {
                    if c::dec_timer_pvar_i32(w, id, 0x10) != 0 { w.mm(id).state = 99; }
                }
                Some(o) => {
                    if owner.is_some() && o.moby != owner {
                        w.mm(id).state = 99;
                        c::set_pos(w, id, [o.point[0], o.point[1], o.point[2], to[3]]);
                        let r = sv::reflect(sv::pv(v), sv::pv([o.normal[0], o.normal[1], o.normal[2], 0.0])).map(|x| x.to_f32());
                        c::set_pv4(w, id, 0, c::set_len3(r, DT + DT));
                    }
                }
            }
        }
        99 => {
            let tmpl = HitTemplate { attacker: Some(id), flags: 0x81_0001, damage: Pf::ONE, ..Default::default() };
            w.sphere_mobys(Pf::ONE, sv::pv(pos), 9, Some(id), Some(&tmpl));
            w.play_sound(0, 0, id);
            burst(w, pos);
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// The 50 puffs of the shell's state 99 (module doc; kind = the turn mod 4, 1 none).
fn burst(w: &mut World, pos: V) {
    use crate::particles::type16::Spawn;
    let tops = [14.0 * DT, 22.0 * DT, 12.0 * DT, 0.0];
    for i in 0..0x32 {
        let k = i % 4;
        let vx = w.rng.randf(DT * -0.5, DT * 0.5);
        let vy = w.rng.randf(DT * -0.5, DT * 0.5);
        let vz = w.rng.randf(DT * 4.0, tops[k]);
        let r = w.rng.randf(0.0, 0.25);
        let a = w.rng.rand_angle();
        let mut q = c::add([a.cos() * r, a.sin() * r, 0.0, 0.0], pos);
        q[2] -= r * 0.717;
        let mut vel = [vx, vy, vz - r * DT * 8.0, 0.0];
        let spawn = match k {
            0 => {
                let size = w.rng.randf(200_000.0, 300_000.0);
                let (a0, b0) = (w.ticks(0x3c), w.ticks(0x78));
                let life = w.rng.rand_range(a0, b0);
                Some(Spawn { size, pos: q, vel, c1: 0x1f10_1820, c2: 0x0010_1010, life, kind: 0 })
            }
            2 => {
                let c1 = if w.rng.randi(100) < 0x28 { 0x5ff8_f8f8 } else { 0x2f48_6078 };
                let (a0, b0) = (w.ticks(0x1e), w.ticks(0x2d));
                let life = w.rng.rand_range(a0, b0);
                Some(Spawn { size: 150_000.0, pos: q, vel, c1, c2: 0x0f00_0020, life, kind: 2 })
            }
            3 => {
                let t1 = w.rng.randf(0.25, 1.0);
                let c1 = crate::hud::tween_color(t1, 0x7f00_0000, 0x7f18_2030);
                let t2 = w.rng.randf(0.5, 1.0);
                let c2 = crate::hud::tween_color(t2, 0, 0x005f_5f5f);
                let sp = w.rng.randf(0.0, 1.0) * DT;
                let b = w.rng.rand_angle();
                vel = fx::polar(sp, a, b);
                let size = w.rng.randf(150_000.0, 200_000.0);
                let (a0, b0) = (w.ticks(0x3c), w.ticks(0x78));
                let life = w.rng.rand_range(a0, b0);
                Some(Spawn { size, pos: q, vel, c1, c2, life, kind: 3 })
            }
            _ => None,
        };
        if let Some(s) = spawn { crate::moby_update::creature::projectile::part16(w, &s); }
    }
}
