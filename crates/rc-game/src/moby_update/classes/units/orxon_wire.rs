//! **Orxon's live wires, class 947** (level10 `0x2d8d30`, census U353; 2 placed) and **their sparks, class 1090**
//! (`0x2dd3d8`, made by `0x2dd270`). A wire runs a charge along its path (pvar 0, equal segments: the first segment's
//! length) at 5 a second: a point light (radius 3, colour (1, 1, 0.5)) rides it, and one tick in two (`randi(2)` 0)
//! on odd ticks a spark is thrown along the path from it, the buzz (sound 0 of class 1090, flags 0xc) follows it; at
//! the path's end the light and the buzz go and it waits `ticks(120)` (states 0 / 1).
//!
//! **A spark** (pvars: +0x00 the path, +0x04 the distance, +0x08 the speed 5·dt slowing by `randf(0.97, 0.999)` a tick,
//! +0x0c the segment, +0x10 the wire, +0x14 the life `trunc(10 / (5·dt))`): orange ambient (0x7f, 0x40, 0), alpha
//! 0x40, a type-26 glow (126000, 0x1f4f7f7f); it slides on along the path tumbling at random, grows over its first
//! quarter and shrinks over its last half (half the class scale at most), and while bigger than a quarter of it hits
//! what it touches (radius 0.15: damage 2, push 1 / 1, flags 0x810001, types 2 / 1, from its wire).
//!
//! Read from the level10 decomp. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2d8d30` | the wire | [`update`] |
//! | `0x2dd270` / `0x2dd3d8` | the spark / its update | `spark` / [`spark_update`] |
//! | `0x2554e0` (= L01 `0x277260`) | the point at a distance | `crate::path::at_distance` |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, attack, V, DT};
use crate::moby_update::services::World;
use crate::moby_update::story;
use crate::path::{at_distance, Point};
use crate::point_lights::PointLight;

pub const REFERENCE_LEVEL: u32 = 10;
pub const UPDATE_FN: u32 = 0x2d_8d30;
pub const SPARK_FN: u32 = 0x2d_d3d8;
pub const CLASSES: [i16; 1] = [947];
pub const SPARK_CLASSES: [i16; 1] = [1090];

const SPARK: i16 = 0x442;
const SPEED: f32 = 5.0;

fn path(w: &World, i: i32) -> Vec<Point> { usize::try_from(i).ok().and_then(|i| w.svc.splines.get(i)).cloned().unwrap_or_default() }
fn seg(pts: &[Point]) -> f32 {
    let f = |i: usize| pts.get(i).map_or([0.0; 4], |q| q.map(f32::from_bits));
    c::dist3(f(0), f(1))
}
fn life_ticks() -> i32 { (10.0 / (DT * SPEED)) as i32 }

/// Level10 `0x2d8d30`: the wire (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x14);
    let pi = c::pi32(w, id, 0);
    if pi == -1 {
        w.delete_moby(id);
        return;
    }
    let pts = path(w, pi);
    w.mm(id).update_dist = 0xff;
    match w.m(id).state {
        0 => {
            let l = c::pi32(w, id, 0xc);
            if l != -1 {
                w.svc.point_lights.free(l as usize);
                c::set_pi32(w, id, 0xc, -1);
            }
            let v = c::pi32(w, id, 0x10);
            if v != -1 { w.release_sound(v, id); }
            c::set_pi32(w, id, 0x10, -1);
            if c::dec_timer_pvar_i32(w, id, 8) != 0 {
                w.mm(id).state = 1;
                c::set_pf(w, id, 4, 0.0);
            }
        }
        1 if 2 <= pts.len() => {
            let sg = seg(&pts);
            let d = c::pf(w, id, 4) + DT * SPEED;
            c::set_pf(w, id, 4, d);
            let (node, _, p) = at_distance(&pts, d, sg);
            let l = PointLight { color: [1.0, 1.0, 0.5], intensity: 0.0, pos: [p[0], p[1], p[2]], radius: 3.0 };
            let slot = c::pi32(w, id, 0xc);
            if slot == -1 {
                let load = f32::from_bits(w.svc.frame_load[1].0);
                let got = w.svc.point_lights.alloc(l, load).map_or(-1, |i| i as i32);
                c::set_pi32(w, id, 0xc, got);
            } else {
                w.svc.point_lights.set(slot as usize, l);
            }
            if w.rng.randi(2) != 0 { return; }
            if w.counter & 1 != 0 { spark(w, d, DT * SPEED, sg, id, p, pi); }
            let v = c::pi32(w, id, 0x10);
            if !w.sound_alive(v, id) {
                let s = w.play_sound_as(0, 0xc, id, SPARK);
                c::set_pi32(w, id, 0x10, s);
                if s != -1 { w.hand_over_sound(s, id, [p[0], p[1], p[2]]); }
            } else {
                w.hand_over_sound(v, id, [p[0], p[1], p[2]]);
            }
            if node == pts.len() as i32 - 1 {
                w.mm(id).state = 0;
                let t = w.ticks(0x78);
                c::set_pi32(w, id, 8, t);
            }
        }
        _ => {}
    }
}

/// `0x2dd270(d, speed, seg, wire, p, path)`: a spark.
fn spark(w: &mut World, d: f32, speed: f32, sg: f32, owner: MobyId, p: V, pi: i32) {
    let Some(m) = w.create_moby(SPARK) else { return };
    story::pvars(w, m, 0x1c);
    {
        let mo = w.mm(m);
        mo.draw_dist = 0xff;
        mo.visible = 1;
        mo.alpha = 0x40;
        mo.update_dist = 0xff;
        mo.state = 0;
        mo.position = p;
        mo.ambient = [0x7f, 0x40, 0, mo.ambient[3]];
    }
    c::set_pi32(w, m, 0, pi);
    c::set_pf(w, m, 4, d);
    c::set_pf(w, m, 8, speed);
    c::set_pf(w, m, 0xc, sg);
    c::set_pi32(w, m, 0x10, owner as i32 + 1);
    c::set_pi32(w, m, 0x14, life_ticks());
    let k = w.rng.randf(f32::from_bits(0x3f7f_be77), f32::from_bits(0x3f78_51ec));
    c::set_pf(w, m, 0x18, k);
    crate::moby_update::creature::projectile::part26(w, f32::from_bits(0x47f6_1801), m, 0x1f4f_7f7f, life_ticks());
}

/// Level10 `0x2dd3d8`: a spark (module doc).
pub fn spark_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x1c);
    let d = c::pf(w, id, 4) + c::pf(w, id, 8);
    c::set_pf(w, id, 4, d);
    let v = c::pf(w, id, 8) * ((c::pf(w, id, 0x18) - 1.0) * 1.0 + 1.0);
    c::set_pf(w, id, 8, v);
    let pts = path(w, c::pi32(w, id, 0));
    if !pts.is_empty() {
        let (_, _, p) = at_distance(&pts, d, c::pf(w, id, 0xc));
        c::set_pos(w, id, p);
    }
    let (a, b, cc) = (w.rng.rand_angle(), w.rng.rand_angle(), w.rng.rand_angle());
    w.mm(id).rotation = [a, b, cc, 0.0];
    if c::dec_timer_pvar_i32(w, id, 0x14) != 0 {
        w.delete_moby(id);
        return;
    }
    let full = life_ticks();
    let life = c::pi32(w, id, 0x14);
    let cs = super::class_scale(w, SPARK);
    if life < full / 2 {
        w.mm(id).scale = ((cs * life as f32) / (full / 2) as f32) * 0.5;
    } else if (full / 4) * 3 < life {
        w.mm(id).scale = ((cs * (full - life) as f32) / (full / 4) as f32) * 0.5;
    }
    if cs * 0.25 < w.m(id).scale {
        let owner = usize::try_from(c::pi32(w, id, 0x10) - 1).ok().filter(|&m| m < w.table.mobys.len());
        if let Some(o) = owner {
            let p = c::pos(w, id);
            attack::area_hit(w, f32::from_bits(0x3e19_999a), p, o, 2.0, 1.0, 1.0, None, 0x81_0001, 2, 1);
        }
    }
}
