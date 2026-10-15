//! **Kalebo's pass-through gates, classes 1439 and 1441** (level16 `0x2e5010` / `0x2e5258`, 3 placed each, groups 31 /
//! 30; census U559 / U560; the name is descriptive [L]). Each glows on a 30-tick pulse between two colours and, while
//! Ratchet is within 16 and inside its box (his position in the gate's frame), sets its command byte, clicks (sound 1)
//! and rests for `ticks(600)` before it can fire again. The two classes differ only in their box, the order of the
//! click and the store, and their colours. Read from the level16 decomp and its words gp−0x4e28..−0x4e14,
//! 0x1d9750..0x1d977c.
//!
//! **Pvars**: +0x00 the rest timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | glow (+0x90) = `FastTweenColor(0.5·sin(2π·(tick mod ticks(30))/ticks(30) − π) + 0.5, a, b)` (1439: 0x80808080 → 0x80202020; 1441 the reverse) | [`update`] |
//! | state 0 | → 1, update distance 0xff | [`update`] |
//! | state 1 | `vec_distance(position, Ratchet)` < 16 and (Ratchet − position)·rows (the transposed rows 0x1fa2d8, `fun_001f9d20`) in the box (1439: x ±4, y ±1, z −1..9; 1441: x ±3, y ±1, z −1..8) → +0xbc = 1, → 2, +0x00 = `ticks(600)`, `PlayClassSound(1, 0)` | [`update`] |
//! | state 2 | `FastDecTimer(+0x00)` ≠ 0 → 1, +0xbc = 0 | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_5010;
pub const UPDATE_FN_1441: u32 = 0x2e_5258;
pub const CLASSES: [i16; 1] = [1439];
pub const CLASSES_1441: [i16; 1] = [1441];
/// The rest after a pass (gp−0x4e28 / −0x4e1c: 600).
pub const REST: i32 = 600;
/// The glow's period.
pub const PULSE: i32 = 30;
/// The reach (`vec_distance` < 16).
pub const REACH: f32 = 16.0;

struct Gate {
    lo: [f32; 3],
    hi: [f32; 3],
    colours: (u32, u32),
}

const G1439: Gate = Gate { lo: [-4.0, -1.0, -1.0], hi: [4.0, 1.0, 9.0], colours: (0x8080_8080, 0x8020_2020) };
const G1441: Gate = Gate { lo: [-3.0, -1.0, -1.0], hi: [3.0, 1.0, 8.0], colours: (0x8020_2020, 0x8080_8080) };

/// Level16 `0x2e5010` (module doc).
pub fn update(w: &mut World, id: MobyId) { run(w, id, &G1439) }

/// Level16 `0x2e5258` (module doc).
pub fn update_1441(w: &mut World, id: MobyId) { run(w, id, &G1441) }

#[allow(clippy::approx_constant)] // the code's own 6.28318 and 3.14159 (not 2π and π).
fn run(w: &mut World, id: MobyId, g: &Gate) {
    if w.m(id).pvars.len() < 4 { return; }
    let n = w.ticks(PULSE).max(1);
    let f = ((w.counter as i64 % n as i64) as f32 / n as f32 * 6.28318 - 3.14159).sin();
    w.mm(id).glow = crate::particles::tween_color((f * 0.5 + 0.5).to_bits(), g.colours.0, g.colours.1);
    let state = w.m(id).state;
    match state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            let (h, m) = (super::hero_pos(w), w.m(id));
            let d = [h[0] - m.position[0], h[1] - m.position[1], h[2] - m.position[2]];
            if REACH <= (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() { return; }
            let l: [f32; 3] = std::array::from_fn(|k| d[0] * m.rows[k][0] + d[1] * m.rows[k][1] + d[2] * m.rows[k][2]);
            if !(0..3).all(|k| g.lo[k] < l[k] && l[k] < g.hi[k]) { return; }
            let t = w.ticks(REST);
            let m = w.mm(id);
            m.cmd = 1;
            m.state = 2;
            c::set_pi32(w, id, 0, t);
            w.play_sound(1, 0, id);
        }
        2 if c::dec_timer_pvar_i32(w, id, 0) != 0 => {
            let m = w.mm(id);
            m.state = 1;
            m.cmd = 0;
        }
        _ => {}
    }
}
