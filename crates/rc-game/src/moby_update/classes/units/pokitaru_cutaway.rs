//! U363: class 1157, Pokitaru's cutaway machine (level11 `0x30d800` with its placement `0x30dd18`, the only copy: 1
//! placed). The name is descriptive [L]. A machine of 24 slats (class 1207, created by it: four rows of six) that open
//! like shutters once the floor switch 830 it watches is pressed, shown in a cutaway: the screen fades to black,
//! Ratchet is held at the switch (state 0x72) and the script camera cuts to a cuboid's view, the camera glides to a
//! second cuboid while the slats open on a spring, then a second fade hands Ratchet back (state 0) behind the follow
//! camera. The commando 114 (`pokitaru_commando`, phase 2: his state 0xb) waits for its state 5.
//!
//! **Pvars** (P): +0x00 + 0x18·k + 4·j the slat moby of row k (0..3), column j (0..5) (an index here), +0x60 the switch
//! 830 (a moby index), +0x64 / +0x68 the cuboids of the camera's start and end view, +0x6c the opening t (0..1), +0x70
//! its spring velocity, +0x74 the last t, +0x78 the fade phase x (−1..1: the fade is `1 − |x|`).
//!
//! The level11 words (gp = 0x166c00): gp−0x4cc0 7.0 (0x161f40: the rows' offset along the machine's row 0), gp−0x4cbc
//! 7.0 (its row 1), gp−0x4cb8 10.0 (the slat's pivot along its own row 1), gp−0x4ca8 / −0x4ca4 / −0x4ca0 0.2, 0.2, 0.2
//! (the spring's acceleration / braking per dt² and top speed per dt); gp−0x4cac (0x161f54) the word state 1 clears.
//!
//! ## Coverage
//!
//! **The update** `0x30d800` (`switch(state)`, then always `0x30dd18`):
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | 0: four rows × six `CreateMoby(0x4b7)` (1207): draw distance (+0x32) 0x40, drawn (+0x31) 1, light word / ambient (+0x38) = the hero moby's, mode (+0x34) = the machine's, position / rotation = the machine's; +0x6c = 0, state 1 | the slats | [`update`] |
//! | 1: the switch's command byte (+0xbc) 1 (just pressed) → state 2, +0x78 = −1, 0x161f54 = 0, `PlayClassSound(0, 0, m)` (`0x2b0ac0`) | the start | [`update`] (0x161f54: n/a, no reader on level 11) |
//! | 1: the byte 2 (pressed before, a revisit) → +0x6c = 1, state 5 | already open | [`update`] |
//! | 2: `Approach(1, 1 / ticks(20), &x)` (`0x281708`); 0x15f3fc = 1 − \|x\| (`0x230b80` = `fabs`); x crossing 0 (old < 0 ≤ x) → `HeroTeleport(switch +0x10, switch +0x40, 0x72, 1)` (`0x246398`) and `CameraScript(cuboid +0x64 centre, its Euler, 1, 0, 0)` (`0x325e50`, a cut); else x ≥ 1 → 3 | the fade in | [`update`] (`cinematic::set_fade`, `hero_teleport`, `camera_script`) |
//! | 3: +0x74 = t; `0x281810(1, 0.2·dt², 0.2·dt², 0.2·dt, &t, &v)`; camera position = `lerp(cuboid +0x64 centre, cuboid +0x68 centre, t)` (`0x230c40`, four lanes), Euler = per axis `add_rot(e64, sub_rot(e68, e64)·t)` (w 0, `0x230bc8`), `0x325d88` / `0x325de0`; t ≥ 1 → 4, x = −1 | the glide | [`update`] (`turn::spring`, `cinematic::camera_targets`) |
//! | 4: the fade again; x crossing 0 → `HeroTeleport(0x13f3d0, 0x13f3e0, 0, 1)` (Ratchet's own position and Euler, state 0, the camera behind him), `CameraScript2(3)` (`0x325fc8`); else x ≥ 1 → 5 | the fade out | [`update`] |
//! | 5 and above: nothing | done | [`update`] |
//!
//! **The placement** `0x30dd18` (every tick, every state): A = the machine's row 0 at length 7, B = its row 1 at length 7
//! (`0x230e68` = `FastVecNormalize`); a = max(90 − 180t, −90) (rows 0, 1), b = clamp(90 − 180(t − 0.5), 0, 90) (rows 2,
//! 3), in degrees; for row k, column j: position = machine ± A (+ for k < 2); v = (j − 2.5)·15°; even k: yaw = machine
//! yaw + π, v = `sub_rot(v, angle·deg)`, v > 45° → v − 90°; odd k: yaw = machine yaw, v = `add_rot(v, angle·deg)`, v <
//! −45° → v + 90°; v clamped to ±45°, yaw = `add_rot(yaw, v)`; rows from the Euler (`0x2313b8`); position += (the slat's
//! row 1 at length −10) − B (even k) / + B (odd k); `0x276c28` (`MobyBuildMatrix`). [`place`].
//!
//! **Not the game's, noted [L]:** the slat links are kept as moby indices (pointers in the game); the machine never
//! sets the letterbox 0x15f404 (only the fade and the script camera).

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;

/// The update in the level11 class table.
pub const UPDATE_FN: u32 = 0x30_d800;
pub const REFERENCE_LEVEL: u32 = 11;
pub const CLASSES: [i16; 1] = [1157];
/// The slats (no update of their own).
pub const SLAT: i16 = 0x4b7;
const ROWS: usize = 4;
const COLS: usize = 6;
const DEG: f32 = 0.017_453_292;
/// gp−0x4cc0 / −0x4cbc / −0x4cb8.
pub const ROW_OFF_A: f32 = 7.0;
pub const ROW_OFF_B: f32 = 7.0;
pub const PIVOT: f32 = 10.0;
/// gp−0x4ca8 / −0x4ca4 / −0x4ca0.
pub const SPRING: [f32; 3] = [0.2, 0.2, 0.2];
/// The fade's length (`ticks(20)` each way).
pub const FADE_TICKS: i32 = 20;
/// Ratchet's state while the cutaway runs.
pub const HELD: i32 = 0x72;

/// Pvar offsets (module doc).
pub mod pv {
    pub const SLATS: usize = 0x00;
    pub const SWITCH: usize = 0x60;
    pub const CUBOID_A: usize = 0x64;
    pub const CUBOID_B: usize = 0x68;
    pub const T: usize = 0x6c;
    pub const VEL: usize = 0x70;
    pub const LAST_T: usize = 0x74;
    pub const FADE: usize = 0x78;
}

fn slat(w: &World, id: MobyId, k: usize, j: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, pv::SLATS + 0x18 * k + 4 * j)).ok().filter(|&m| m < w.table.mobys.len())
}

/// A level cuboid's centre (+0x30, four lanes) and Euler (+0x70).
fn cuboid(w: &World, i: i32) -> Option<([f32; 4], [f32; 3])> {
    let s = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i)?;
    Some((s.matrix[3], s.euler))
}

/// The fade of states 2 and 4: x approaches 1 by `1 / ticks(20)`, 0x15f3fc = 1 − |x|; true when x crossed 0 this tick.
fn fade(w: &mut World, id: MobyId) -> bool {
    let old = c::pf(w, id, pv::FADE);
    let mut x = old;
    let n = w.ticks(FADE_TICKS) as f32;
    turn::approach(1.0, 1.0 / n, &mut x);
    c::set_pf(w, id, pv::FADE, x);
    crate::cinematic::set_fade(w, 1.0 - x.abs());
    old < 0.0 && 0.0 <= x
}

/// Level11 0x30d800 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x7c { return; }
    match w.m(id).state {
        0 => {
            let (pos, rot, md) = (w.m(id).position, w.m(id).rotation, w.m(id).mode);
            let light = w.hero_moby.map(|h| (w.m(h).light, w.m(h).ambient));
            for k in 0..ROWS {
                for j in 0..COLS {
                    let s = w.create_moby(SLAT);
                    c::set_pi32(w, id, pv::SLATS + 0x18 * k + 4 * j, s.map_or(-1, |s| s as i32));
                    let Some(s) = s else { continue };
                    let m = w.mm(s);
                    m.draw_dist = 0x40;
                    m.visible = 1;
                    if let Some((l, a)) = light { m.light = l; m.ambient = a; }
                    m.mode = md;
                    m.position = pos;
                    m.rotation = rot;
                }
            }
            c::set_pf(w, id, pv::T, 0.0);
            w.mm(id).state = 1;
        }
        1 => {
            let sw = usize::try_from(c::pi32(w, id, pv::SWITCH)).ok().and_then(|s| w.table.mobys.get(s)).map(|m| m.cmd);
            match sw {
                Some(1) => {
                    w.mm(id).state = 2;
                    c::set_pf(w, id, pv::FADE, -1.0);
                    w.play_sound(0, 0, id);
                }
                Some(2) => {
                    c::set_pf(w, id, pv::T, 1.0);
                    w.mm(id).state = 5;
                }
                _ => {}
            }
        }
        2 => {
            if fade(w, id) {
                let sw = usize::try_from(c::pi32(w, id, pv::SWITCH)).ok().and_then(|s| w.table.mobys.get(s)).map(|m| (m.position, m.rotation));
                if let Some((p, r)) = sw {
                    crate::cinematic::hero_teleport(w, [p[0], p[1], p[2]], [r[0], r[1], r[2]], HELD, true);
                }
                if let Some((p, e)) = cuboid(w, c::pi32(w, id, pv::CUBOID_A)) {
                    crate::cinematic::camera_script(w, [p[0], p[1], p[2]], e, 1, 0, false);
                }
            } else if 1.0 <= c::pf(w, id, pv::FADE) {
                w.mm(id).state = 3;
            }
        }
        3 => {
            let (mut t, mut v) = (c::pf(w, id, pv::T), c::pf(w, id, pv::VEL));
            c::set_pf(w, id, pv::LAST_T, t);
            turn::spring(1.0, SPRING[0] * DT2, SPRING[1] * DT2, SPRING[2] * DT, &mut t, &mut v);
            c::set_pf(w, id, pv::T, t);
            c::set_pf(w, id, pv::VEL, v);
            let a = cuboid(w, c::pi32(w, id, pv::CUBOID_A));
            let b = cuboid(w, c::pi32(w, id, pv::CUBOID_B));
            if let (Some((pa, ea)), Some((pb, eb))) = (a, b) {
                let p: [f32; 4] = std::array::from_fn(|k| pa[k] + (pb[k] - pa[k]) * t);
                let e: [f32; 3] = std::array::from_fn(|k| c::lerp_rot(ea[k], eb[k], t));
                crate::cinematic::camera_targets(w, Some([p[0], p[1], p[2]]), Some(e));
            }
            if 1.0 <= t {
                w.mm(id).state = 4;
                c::set_pf(w, id, pv::FADE, -1.0);
            }
        }
        4 => {
            if fade(w, id) {
                let h = super::hero_pos(w);
                let r = w.hero.rot.map(|x| f32::from_bits(x.0));
                crate::cinematic::hero_teleport(w, [h[0], h[1], h[2]], [r[0], r[1], r[2]], 0, true);
                crate::cinematic::camera_script2(w, 3);
            } else if 1.0 <= c::pf(w, id, pv::FADE) {
                w.mm(id).state = 5;
            }
        }
        _ => {}
    }
    place(w, id);
}

/// Level11 0x30dd18: the slats at the opening t (module doc).
pub fn place(w: &mut World, id: MobyId) {
    let me = w.m(id);
    let (pos, yaw) = (me.position, me.rotation[2]);
    let a_off = c::set_len3(me.rows[0], ROW_OFF_A);
    let b_off = c::set_len3(me.rows[1], ROW_OFF_B);
    let t = c::pf(w, id, pv::T);
    let lo = (90.0 - t * 180.0).max(-90.0);
    let hi = {
        let x = 90.0 - (t - 0.5) * 180.0;
        x.clamp(0.0, 90.0)
    };
    let quarter = std::f32::consts::FRAC_PI_4;
    let right = std::f32::consts::FRAC_PI_2;
    for k in 0..ROWS {
        let angle = if k < 2 { lo } else { hi };
        let odd = k & 1 != 0;
        for j in 0..COLS {
            let Some(s) = slat(w, id, k, j) else { continue };
            let base = if k < 2 { c::add(pos, a_off) } else { c::sub(pos, a_off) };
            let mut v = (j as f32 - 2.5) * 0.261_799_4;
            let mut y;
            if odd {
                y = c::add_rot(yaw, 0.0);
                v = c::add_rot(v, angle * DEG);
                if v < -quarter { v += right; }
            } else {
                y = c::add_rot(yaw, std::f32::consts::PI);
                v = c::sub_rot(v, angle * DEG);
                if quarter < v { v -= right; }
            }
            v = v.clamp(-quarter, quarter);
            y = c::add_rot(y, v);
            let m = w.mm(s);
            m.position = base;
            m.rotation[2] = y;
            let r = rc_formats::moby_light::rotation_rows([m.rotation[0], m.rotation[1], m.rotation[2]]);
            for (row, q) in m.rows.iter_mut().zip(r.iter()) { *row = q.map(f32::from_bits); }
            let piv = c::set_len3(m.rows[1], -PIVOT);
            let off = if odd { c::add(piv, b_off) } else { c::sub(piv, b_off) };
            m.position = c::add(m.position, off);
            w.build_matrix(s);
        }
    }
}
