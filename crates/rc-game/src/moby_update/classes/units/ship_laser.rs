//! **The ships' laser shot, class 1009** (created by code only): its spawner level13 `0x3025f8`, its update `0x302780`
//! and its target search `0x302420`, the same code on levels 11 and 17 (`0x3089d0` / `0x2e22b0` the updates): one
//! shared unit through [`LevelPorts`] code identity. Gemlik's ship 69 fires it ([`super::gemlik_ship`]); the other
//! levels' flown ships call [`spawn`] the same way. No level data. Read from the level13 decomp. Native `f32`.
//!
//! **Pvar block** (0x30): +0x00 the velocity, +0x10 the brightness (1.0, ×0.74 a tick once its life is out), +0x14 its
//! life (ticks), +0x18 the life timer, +0x1c the owner (index + 1), +0x20..+0x28 the target's position when last
//! seen, +0x2c the target (index + 1).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x3025f8 | `CreateMoby(0x3f1)` (none: nothing); +0x1c = owner; update distance 0xff, draw distance `trunc(range)`, rot.x = 0, drawn (+0x31 = 1) | [`spawn`] |
//! | 0x3025f8 | rot.y = −atan(\|dir\|xy, dir.z), rot.z = atan(dir.x, dir.y); +0x10 = 1 | [`spawn`] |
//! | 0x3025f8 | speed > 0: velocity = dir at that speed; else velocity = dir, speed = \|dir\|; position; +0x18 = +0x14 = `trunc(range / speed)` | [`spawn`] |
//! | 0x3025f8 | mode = 0x200; scale = class scale · (owner scale / owner class scale); alpha = `trunc(+0x10 · 24)`; ambient 0xff, 0xff, 0xff (`0x25bf78` = L01 `0x2650d0`); `MobyBuildMatrix` | [`spawn`] |
//! | 0x302780 | scale += (class scale − scale) · 0.1; old = position | [`update`] |
//! | 0x302780 | the tick (0x15f5cc) and the moby's index agree mod 6 → the target dropped (a fresh search) | [`update`] |
//! | 0x302780 | alpha = `trunc(+0x10 · 24)` | [`update`] |
//! | 0x302780 | no target, the target deleted (0xfe / 0xfd) or moved more than 3 from where it was seen → `0x302420(15°, 15°, 60, position, rotation)` | [`update`], [`search`] |
//! | 0x302780 | a target: its position kept; the yaw / pitch toward it (pitch the game's −atan(xy, dz)) differ by 15° or more → dropped; else each turn clamped to 50°·dt, velocity = polar(\|velocity\|, yaw, −pitch); `0x218828` | [`update`] (`0x218828`: n/a, an empty debug hook) |
//! | 0x302780 | position += velocity | [`update`] |
//! | 0x302780 | outside [2, 1021]³ → `DeleteMoby` | [`update`] |
//! | 0x302780 | the template (`0x26e7d8`: damage 1, flags 0x10000, self, dir 0; +0x1a = the class); `CollLine_Fix(old, position, 0, owner, tmpl)` | [`update`] (`services::line_hit_in`) |
//! | 0x302780 | no hit: `FastDecTimer(+0x18)` running → done; out → +0x10 ·= 0.74, below 0.02 → `DeleteMoby` | [`update`] |
//! | 0x302780 | a hit: five sparks (`PartType27Spawn(90000, position, normalise(r + normalise(randf³)·0.5·\|r\|)·randf(5·dt, 9·dt), 0x7f2f4f6f, rand_range(ticks(10), ticks(15)))`, r = the velocity reflected off the hit normal (`0x221570`)), `DeleteMoby` | [`update`] (`hero::guns::reflect`, `fx::part27`) |
//! | 0x302420 | the run list (0x15ffe4): targetable (mode 0x1000), with a class of type 5 (a creature), within `range` (xy); its point = position + 0.4 up; `FastDiffRots(rot.z, atan)`² < yaw cone², `FastDiffRots(rot.y, atan(xy, dz))`² < pitch cone²; score `dy²·dp²·30 + sqrt(distance)` below the best (10 to start) → the best | [`search`] (`hints::run_list`; the pitch is compared with the game's sign [L: rot.y is minus the elevation; the test uses the elevation]) |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::fx::part27;
use crate::moby_update::creature::{self as c, add, add_rot, atan, diff_rots, dist2, dist3, len3, set_len3, sub_rot, DT};
use crate::moby_update::services::{pv, HitTemplate, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 13;
pub const UPDATE_FN: u32 = 0x30_2780;
pub const SPAWN_FN: u32 = 0x30_25f8;
pub const CLASS: i16 = 0x3f1;
pub const CLASSES: [i16; 1] = [CLASS];

pub mod pvo {
    pub const VEL: usize = 0x00;
    pub const GLOW: usize = 0x10;
    pub const LIFE: usize = 0x14;
    pub const TIMER: usize = 0x18;
    pub const OWNER: usize = 0x1c;
    pub const SEEN: usize = 0x20;
    pub const TARGET: usize = 0x2c;
    pub const LEN: usize = 0x30;
}

/// 15° (0x3e860a92), the turn limit 50°·dt (0.87266463), the search range.
const CONE: f32 = f32::from_bits(0x3e86_0a92);
const TURN: f32 = 0.872_664_63;
const RANGE: f32 = 60.0;

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    usize::try_from(v - 1).ok().filter(|&m| m < w.table.mobys.len())
}

fn set_link(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)); }

/// The class scale × (the owner's scale / its class scale): the shots keep the owner's relative size.
pub fn owner_scale(w: &World, class: i16, owner: MobyId) -> f32 {
    let own = w.m(owner);
    use crate::moby_update::services::fl;
    fl(w.class_scale(class)) * (own.scale / fl(w.class_scale(own.o_class)))
}

/// Level13 `0x3025f8(range, speed, owner, dir, pos)` (module doc).
pub fn spawn(w: &mut World, range: f32, speed: f32, owner: MobyId, dir: [f32; 4], pos: [f32; 4]) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    crate::moby_update::story::pvars(w, id, pvo::LEN);
    set_link(w, id, pvo::OWNER, Some(owner));
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = range as i16;
        m.rotation[0] = 0.0;
        m.visible = 1;
        let xy = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        m.rotation[1] = -atan(xy, dir[2]);
        m.rotation[2] = atan(dir[0], dir[1]);
    }
    c::set_pf(w, id, pvo::GLOW, 1.0);
    let speed = if 0.0 < speed {
        c::set_pv4(w, id, pvo::VEL, set_len3(dir, speed));
        speed
    } else {
        c::set_pv4(w, id, pvo::VEL, dir);
        len3(dir)
    };
    w.mm(id).position = pos;
    let life = (range / speed) as i32;
    c::set_pi32(w, id, pvo::TIMER, life);
    c::set_pi32(w, id, pvo::LIFE, life);
    let scale = owner_scale(w, CLASS, owner);
    {
        let m = w.mm(id);
        m.mode = 0x200;
        m.scale = scale;
    }
    let glow = c::pf(w, id, pvo::GLOW);
    {
        let m = w.mm(id);
        m.alpha = (glow * 24.0) as i32 as u8;
        m.ambient = [0xff, 0xff, 0xff, 0];
    }
    w.build_matrix(id);
    Some(id)
}

/// Level13 `0x302420(yaw cone, pitch cone, range, from, rotation)`: the best creature in the cone (module doc).
pub fn search(w: &World, cone_yaw: f32, cone_pitch: f32, range: f32, from: [f32; 4], rot: [f32; 4]) -> Option<MobyId> {
    let mut best = None;
    let mut score = 10.0f32;
    for m in super::hints::run_list(w) {
        let mo = w.m(m);
        if mo.mode & mode::TARGETABLE == 0 || !mo.has_class { continue; }
        if w.classes.info(mo.o_class).map(|i| i.ty) != Some(5) { continue; }
        let d = dist2(from, mo.position);
        if range <= d { continue; }
        let t = [mo.position[0], mo.position[1], mo.position[2] + 0.4, mo.position[3]];
        let dy = diff_rots(rot[2], atan(t[0] - from[0], t[1] - from[1]));
        if cone_yaw * cone_yaw <= dy * dy { continue; }
        let dp = diff_rots(rot[1], atan(d, t[2] - from[2]));
        if cone_pitch * cone_pitch <= dp * dp { continue; }
        let s = dy * dy * dp * dp * 30.0 + dist3(from, t).abs().sqrt();
        if s < score {
            score = s;
            best = Some(m);
        }
    }
    best
}

fn alive(w: &World, m: MobyId) -> bool { let s = w.m(m).state; s != 0xfe && s != 0xfd }

/// Level13 `0x302780` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pvo::LEN { w.delete_moby(id); return; }
    let cs = crate::moby_update::services::fl(w.class_scale(CLASS));
    {
        let m = w.mm(id);
        m.scale += (cs - m.scale) * 0.1;
    }
    let old = w.m(id).position;
    if w.counter % 6 == (id as u64) % 6 { set_link(w, id, pvo::TARGET, None); }
    let glow = c::pf(w, id, pvo::GLOW);
    w.mm(id).alpha = (glow * 24.0) as i32 as u8;
    let seen = c::pv4(w, id, pvo::SEEN);
    let seen = [seen[0], seen[1], seen[2], 0.0];
    let keep = link(w, id, pvo::TARGET).filter(|&t| alive(w, t) && dist3(w.m(t).position, seen) <= 3.0);
    if keep.is_none() {
        let (p, r) = (w.m(id).position, w.m(id).rotation);
        let t = search(w, CONE, CONE, RANGE, p, r);
        set_link(w, id, pvo::TARGET, t);
    }
    if let Some(t) = link(w, id, pvo::TARGET) {
        let tp = w.m(t).position;
        let s = c::pv4(w, id, pvo::SEEN);
        c::set_pv4(w, id, pvo::SEEN, [tp[0], tp[1], tp[2], s[3]]);
        let speed = len3(c::pv4(w, id, pvo::VEL));
        let p = w.m(id).position;
        let mut dy = sub_rot(atan(tp[0] - p[0], tp[1] - p[1]), w.m(id).rotation[2]);
        let ay = dy.abs();
        let mut dp = sub_rot(-atan(dist2(p, tp), tp[2] - p[2]), w.m(id).rotation[1]);
        let ap = dp.abs();
        if CONE <= ay || CONE <= ap {
            set_link(w, id, pvo::TARGET, None);
        } else {
            if DT * TURN < ay { dy = (dy / ay) * DT * TURN; }
            let rz = add_rot(w.m(id).rotation[2], dy);
            w.mm(id).rotation[2] = rz;
            if DT * TURN < ap { dp = (dp / ap) * DT * TURN; }
            let ry = add_rot(w.m(id).rotation[1], dp);
            w.mm(id).rotation[1] = ry;
            c::set_pv4(w, id, pvo::VEL, crate::moby_update::creature::fx::polar(speed, rz, -ry));
        }
    }
    let vel = c::pv4(w, id, pvo::VEL);
    let p = add(w.m(id).position, vel);
    w.mm(id).position = p;
    if !crate::moby_update::creature::projectile::in_world(p) {
        w.delete_moby(id);
        return;
    }
    let tmpl = HitTemplate { dir: [Pf::ZERO; 4], attacker: Some(id), flags: 0x1_0000, b18: 0, b19: 0, h1a: w.m(id).o_class as u16, damage: Pf::ONE, w20: 0 };
    let ign = link(w, id, pvo::OWNER);
    let Some(h) = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(old), pv(p), 0, ign, &tmpl) else {
        if c::dec_timer_pvar_i32(w, id, pvo::TIMER) == 0 { return; }
        let g = c::pf(w, id, pvo::GLOW) * 0.74;
        c::set_pf(w, id, pvo::GLOW, g);
        if 0.02 <= g { return; }
        w.delete_moby(id);
        return;
    };
    hit_sparks(w, p, vel, h.normal);
    w.delete_moby(id);
}

/// The five sparks of a laser hit (`PartType27Spawn(90000, …)`, module doc); the fighters' shot 1017
/// ([`super::fighter_shot`]) has the same lines.
pub fn hit_sparks(w: &mut World, p: [f32; 4], vel: [f32; 4], normal: [f32; 3]) {
    let v3 = [vel[0], vel[1], vel[2]];
    for _ in 0..5 {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let r = crate::hero::guns::reflect(v3, normal);
        let l = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        let q = set_len3([x, y, z, 0.0], l * 0.5);
        let s = w.rng.randf(DT * 5.0, DT * 9.0);
        let q = set_len3([r[0] + q[0], r[1] + q[1], r[2] + q[2], 0.0], s);
        let (t10, t15) = (w.ticks(10), w.ticks(15));
        let life = w.rng.rand_range(t10, t15);
        part27(w, 90000.0, p, q, 0x7f2f_4f6f, life);
    }
}
