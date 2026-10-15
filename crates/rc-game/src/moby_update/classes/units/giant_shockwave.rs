//! **Giant Clank's landing shockwave** (class 0x593 = 1427; made by the hero code's jump 0x5e on its first landed tick,
//! `0x2a99b0` = L15 0x29e9f8 / L18's copy; update level15 0x29ead0, level18 0x2ab7c0, the same code): a ring that grows
//! from his feet and smashes what it touches. Read from the level15 decomp.
//!
//! **Pvar block**: +0x00 (f32) the growth per tick, +0x04 / +0x08 (s32) the life (+0x08 counts down), +0x0c (f32) the
//! damage.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x29e9f8 (spawn) | `CreateMoby(0x593)` at the point; ambient (0x80, 0x80, 0x70) (`0x2650d0`); growth = `dt·15`, scale = 2·2.0 × class scale, damage 40, life = ticks(25) (both words); +0x30 = 0xff, +0x32 = 0xff, +0x31 = 1, state 0 | [`spawn`] (from `hero::bodies::giant::after_update`) |
//! | 0x29ead0 | f = (scale / class scale)·0.5 + growth; scale = 2f × class scale; life < ticks(40): alpha +0x23 = life·0x7f / ticks(40); `0x26e830(f, (int)damage, 1, hero moby, position, 0x30000, 0, 1, 0)`; `FastDecTimer(life)` out → `DeleteMoby` | [`update`] (`creature::attack::sphere_hit`) |
//! | | no sound, particle, light | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, attack};
use crate::moby_update::services::World;

/// The update in the level15 class table (level18's 0x2ab7c0 is the same code).
pub const UPDATE_FN: u32 = 0x29_ead0;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASS: i16 = 0x593;
pub const CLASSES: [i16; 1] = [CLASS];

/// `0x2a99b0(size, growth, damage, hero, point, life)`: the shockwave at `pos` (None: no moby could be made).
pub fn spawn(w: &mut World, size: f32, growth: f32, damage: f32, pos: [f32; 3], life: i32) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    let cs = w.classes.info(CLASS).map_or(1.0, |i| i.scale);
    let m = w.mm(id);
    m.position = [pos[0], pos[1], pos[2], m.position[3]];
    (m.ambient[0], m.ambient[1], m.ambient[2]) = (0x80, 0x80, 0x70);
    m.scale = (size + size) * cs;
    (m.update_dist, m.draw_dist, m.visible, m.state) = (0xff, 0xff, 1, 0);
    c::set_pi32(w, id, 0, growth.to_bits() as i32);
    c::set_pi32(w, id, 0xc, damage.to_bits() as i32);
    c::set_pi32(w, id, 8, life);
    c::set_pi32(w, id, 4, life);
    Some(id)
}

pub fn update(w: &mut World, id: MobyId) {
    let cs = w.classes.info(w.m(id).o_class).map_or(1.0, |i| i.scale);
    let growth = f32::from_bits(c::pi32(w, id, 0) as u32);
    let f = (w.m(id).scale / cs) * 0.5 + growth;
    w.mm(id).scale = (f + f) * cs;
    let life = c::pi32(w, id, 8);
    let t40 = w.ticks(40);
    if life < t40 && t40 != 0 { w.mm(id).alpha = ((life * 0x7f) / t40) as u8; }
    let damage = f32::from_bits(c::pi32(w, id, 0xc) as u32) as i32 as f32;
    // The attacker is the hero moby 0x1413d0 (Giant Clank while he is the hero).
    let ratchet = w.hero_moby.unwrap_or(0);
    let hero = w.hero.hero_moby(ratchet);
    let pos = w.m(id).position;
    attack::sphere_hit(w, f, damage, 1.0, hero, pos, 0x3_0000, 0, 1, 0);
    let mut t = life;
    if crate::hero::idle::dec_timer(&mut t) != 0 {
        c::set_pi32(w, id, 8, t);
        w.delete_moby(id);
        return;
    }
    c::set_pi32(w, id, 8, t);
}
