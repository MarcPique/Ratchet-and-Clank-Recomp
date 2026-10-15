//! Veldin's moving carriers, class 1584 (level 18, 38 instances): level18 0x2fa728 (census unit U563). The class moves
//! by other code (it is a group member or driven by a path owner) and carries whoever rides it: each tick it reports
//! its displacement and rotation since the last tick to its platform block (pvar +0x20, `CarryRiders`). Told to
//! (+0xbc ≠ 0) with +0x80 set, it makes once an attachment of class 1892 (0x764) that follows it. Read from the level18
//! decomp. The name is descriptive [L]. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1 | [`update`] |
//! | state 1 | `CarryRiders(+0x20, pos − +0x60, +0x70, rot)` (L01 0x2755f8) | `triggers::carry_riders` (with the rotation change, G-CLS-024) |
//! | every state | +0xbc ≠ 0, +0x80 ≠ 0, no attachment (+0x84): `CreateMoby(1892)`: the draw distance, drawn, the light word and ambient (+0x38), scale · (own scale / own class scale) | [`update`] |
//! | | +0x60 = pos, +0x70 = rot; the attachment: pos, rot, `MobyBuildMatrix` (effect on the other moby) | [`update`] |
//! | | no sound, particle, save flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;
use crate::moby_update::triggers;

pub const UPDATE_FN: u32 = 0x2f_a728;
pub const REFERENCE_LEVEL: u32 = 18;
pub const CLASSES: [i16; 1] = [1584];
/// The attachment (0x764).
pub const ATTACHMENT: i16 = 1892;
pub const BLOCK: usize = 0x20;
pub const OLD_POS: usize = 0x60;
pub const OLD_ROT: usize = 0x70;
pub const WANTS: usize = 0x80;
/// The attachment (the port stores `id + 1`, 0 none).
pub const CHILD: usize = 0x84;

/// Level18 0x2fa728 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x88 { return; }
    match w.m(id).state {
        0 => w.mm(id).state = 1,
        1 => {
            let d = c::sub(c::pos(w, id), c::pv4(w, id, OLD_POS));
            let (old, new) = (c::pv4(w, id, OLD_ROT), w.m(id).rotation);
            triggers::carry_riders(&mut w.mm(id).pvars, BLOCK, d, old, new);
        }
        _ => {}
    }
    if w.m(id).cmd != 0 && c::pi32(w, id, WANTS) != 0 && c::pi32(w, id, CHILD) == 0 {
        let made = w.create_moby(ATTACHMENT);
        c::set_pi32(w, id, CHILD, made.map_or(0, |n| n as i32 + 1));
        if let Some(n) = made {
            let (dd, light, amb, s) = { let m = w.m(id); (m.draw_dist, m.light, m.ambient, m.scale) };
            let k = s / super::class_scale(w, w.m(id).o_class);
            let a = w.mm(n);
            a.draw_dist = dd;
            a.visible = 1;
            a.light = light;
            a.ambient = amb;
            a.scale *= k;
        }
    }
    let (p, r) = (c::pos(w, id), w.m(id).rotation);
    c::set_pv4(w, id, OLD_POS, p);
    c::set_pv4(w, id, OLD_ROT, r);
    if let Ok(n) = usize::try_from(c::pi32(w, id, CHILD) - 1) {
        if n < w.table.mobys.len() {
            let a = w.mm(n);
            a.position = p;
            a.rotation = r;
            w.build_matrix(n);
        }
    }
}
