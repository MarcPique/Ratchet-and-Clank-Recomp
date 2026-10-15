//! Hydrodisplacer pads, class 341: level05 0x2f8080, the same code on 07, 11, 12 and 18 (census U170; 17 created
//! instances). The pad's animation follows the command bits (+0xbc) of the moby it is linked to (the water machinery
//! it drives): bit 0 raises it (sequence 0 → 1 → 2), bit 1 lowers it (2 → 3 → 0), but not while Ratchet stands on it
//! in movement group 0x13. Its glow fades in (over half a second) while Ratchet stands on it and out when he leaves,
//! pulsing once a second green with the Hydrodisplacer (item 0x16) in his hand, blue without; the pad's own +0xbc
//! tells the machinery that he is on it holding the Hydrodisplacer. Read from the level05 decomp (0x2f8080, 0x2f81b8);
//! the glow words (0x80808080, 0x8060c060, 0x806060c0) checked on all five copies. Native `f32`.
//!
//! **Pvar block**: +0x00 the linked moby (runtime index, −1 none), +0x18 the pulse phase, +0x1c the glow fade.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2f8080 | on = Ratchet's ground moby `0x13f64c` = m and movement group `0x1413dc` = 0x13 | [`update`] |
//! | | sequence (+0x53) 0 → 1 (`MobyAnimBlend(m, 1, 0, 0)`, 0x26c660) when the link's +0xbc bit 0 and not on; 1 → 2 on the wrap (+0x70 & 2); 2 → 3 on bit 1 and not on; 3 → 0 on the wrap | [`update`] (`World::anim_blend`) |
//! | 0x2f81b8 | Ratchet on it: fade `Approach(1, 2·dt)` (0x270728), +0xbc = the hand slot `0x1403e0` holds item 0x16 (`0x140408`); else `Approach(0, dt)` | [`glow`] |
//! | | fade 0: glow +0x90 = 0x80808080; else phase += 2π·dt, `c = tween((sin + 1)/2, 0x80808080, +0xbc ? 0x8060c060 : 0x806060c0)`, glow = `tween(fade, 0x80808080, c)` (0x2221a8) | [`glow`] |
//! | | no sound, particle, light, save flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::turn::approach;
use crate::moby_update::creature::{add_rot, pf, pi32, set_pf, DT};
use crate::moby_update::services::World;

/// The update in the level05 class table.
pub const UPDATE_FN: u32 = 0x2f_8080;
pub const REFERENCE_LEVEL: u32 = 5;
pub const CLASSES: [i16; 1] = [341];

/// The Hydrodisplacer's item id.
pub const HYDRODISPLACER: i32 = 0x16;
const GREY: u32 = 0x8080_8080;

/// Level05 0x2f8080 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { return; }
    let on = w.hero.ground_moby == Some(id) && w.hero.group == 0x13;
    let link = usize::try_from(pi32(w, id, 0)).ok().and_then(|l| w.table.mobys.get(l)).map(|m| m.cmd);
    let wrapped = w.m(id).anim.flags & 2 != 0;
    let next = match w.m(id).anim.seq_b {
        0 => link.filter(|c| c & 1 != 0 && !on).map(|_| 1),
        1 => wrapped.then_some(2),
        2 => link.filter(|c| c & 2 != 0 && !on).map(|_| 3),
        3 => wrapped.then_some(0),
        _ => None,
    };
    if let Some(s) = next { w.anim_blend(id, s, 0, 0); }
    glow(w, id);
}

/// Level05 0x2f81b8 (module doc).
fn glow(w: &mut World, id: MobyId) {
    let mut fade = pf(w, id, 0x1c);
    if w.hero.ground_moby == Some(id) {
        approach(1.0, DT + DT, &mut fade);
        let slot = &w.hero.items.slot;
        w.mm(id).cmd = (slot.item.is_some() && slot.id == HYDRODISPLACER) as u8;
    } else {
        approach(0.0, DT, &mut fade);
    }
    set_pf(w, id, 0x1c, fade);
    if fade == 0.0 {
        w.mm(id).glow = GREY;
        return;
    }
    let ph = add_rot(pf(w, id, 0x18), DT * std::f32::consts::TAU);
    set_pf(w, id, 0x18, ph);
    let to = if w.m(id).cmd == 1 { 0x8060_c060 } else { 0x8060_60c0 };
    let c = crate::hud::tween_color((ph.sin() + 1.0) * 0.5, GREY, to);
    w.mm(id).glow = crate::hud::tween_color(fade, GREY, c);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;

    #[test]
    fn glow_fades_in_under_ratchet() {
        let mut m = Moby { o_class: 341, pvars: vec![0; 0x20], ..Moby::default() };
        p::set_i32(&mut m.pvars, 0, -1);
        let mut t = MobyTable::new(vec![m], 4);
        let mut hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
            update(&mut w, 0);
            assert_eq!(w.m(0).glow, GREY);
        }
        hero.ground_moby = Some(0);
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 1);
        for _ in 0..30 { update(&mut w, 0); }
        assert_eq!((pf(&w, 0, 0x1c), w.m(0).cmd), (1.0, 0), "faded in; no Hydrodisplacer in hand");
        assert_ne!(w.m(0).glow, GREY);
    }
}
