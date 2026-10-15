//! **Kalebo's rail switches, class 1442** (level16 `0x2e54a0`, 3 placed in group 37; census U561; the name is
//! descriptive [L]). A switch beside a grind rail that pulses red until Ratchet, grinding past within 4 (xy), leans
//! towards it (lean 3 with it on his right, 2 on his left, about level with him), or hits it (hit mask 0x330000); then
//! it turns on for good (command byte 1, green pulse). Read from the level16 decomp and its words gp−0x4e10.
//!
//! **Pvars**: +0x00 the pulse phase, +0x04 a timer nothing sets (always run out).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, phase = `random_angle_radians` | [`update`] |
//! | state 1 | `FastDecTimer(+0x04)` ≠ 0, `vec_distance2(position, Ratchet)` < 4, Ratchet grinding (0x1413d4 = 0x28) with a lean (0x13f8f8 ≠ −1): p = (0, −1, 1)·rows + position, l = (p − Ratchet)·his transposed rows (0x13f390); \|l.y\| < 0.1 and (l.x > 0 with lean 3, or l.x < 0 with lean 2) → on (`0x2e56e0`: phase −π, → 2, +0xbc = 1), `PlayClassSound(1, 0)` | [`update`] |
//! | | `MobyGetHitMessage(m, 0x330000, 0)` → on, `PlayClassSound(0, 0)`, +0xa4 = 0xff | [`update`] |
//! | every tick | phase = `fast_add_rotations(phase, 4π·dt)`; glow = `FastTweenColor((sin + 1)/2, +0xbc ? 0x80208020 : 0x80202080, 0x80202020)` | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_54a0;
pub const CLASSES: [i16; 1] = [1442];
/// The hit mask.
pub const HIT_MASK: u32 = 0x33_0000;
/// Ratchet's grind state.
pub const GRIND: i32 = 0x28;
/// The point beside the switch (gp−0x4e10: (0, −1, 1)).
const POINT: [f32; 3] = [0.0, -1.0, 1.0];
const OFF: u32 = 0x8020_2080;
const ON: u32 = 0x8020_8020;
const DIM: u32 = 0x8020_2020;

fn turn_on(w: &mut World, id: MobyId) {
    c::set_pf(w, id, 0, f32::from_bits(0xc049_0fda));
    let m = w.mm(id);
    m.state = 2;
    m.cmd = 1;
}

fn leaned_into(w: &World, id: MobyId) -> bool {
    let (h, m) = (super::hero_pos(w), w.m(id));
    let lean = w.hero.boots.lean;
    if 4.0 <= c::dist2(m.position, h) || w.hero.state != GRIND || lean == -1 { return false; }
    let p: [f32; 3] = std::array::from_fn(|k| POINT[0] * m.rows[0][k] + POINT[1] * m.rows[1][k] + POINT[2] * m.rows[2][k] + m.position[k]);
    let d = [p[0] - h[0], p[1] - h[1], p[2] - h[2]];
    let rows = w.hero.rows.map(|r| r.map(|x| x.to_f32()));
    let l: [f32; 2] = std::array::from_fn(|k| d[0] * rows[k][0] + d[1] * rows[k][1] + d[2] * rows[k][2]);
    l[1].abs() < 0.1 && ((0.0 < l[0] && lean == 3) || (l[0] < 0.0 && lean == 2))
}

/// Level16 `0x2e54a0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 8 { return; }
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            let a = w.rng.rand_angle();
            c::set_pf(w, id, 0, a);
        }
        1 => {
            if c::dec_timer_pvar_i32(w, id, 4) != 0 && leaned_into(w, id) {
                turn_on(w, id);
                w.play_sound(1, 0, id);
            }
            if w.get_hit(id, HIT_MASK, false).is_some() {
                turn_on(w, id);
                w.play_sound(0, 0, id);
                w.mm(id).hit_slot = 0xff;
            }
        }
        _ => {}
    }
    let phase = c::add_rot(c::pf(w, id, 0), DT * 12.566_371);
    c::set_pf(w, id, 0, phase);
    let from = if w.m(id).cmd == 0 { OFF } else { ON };
    w.mm(id).glow = crate::particles::tween_color(((phase.sin() + 1.0) * 0.5).to_bits(), from, DIM);
}
