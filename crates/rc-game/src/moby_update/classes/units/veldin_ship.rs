//! **Ratchet's ship on Veldin, class 530** (level00 `0x2d1e80`, census U24; one placed beside his workshop; the same
//! code as level01 `0x2ecde0`, which no level-01 class runs). It stands where the opening's ship actor stands and gives
//! way to it: hidden while a scene plays its own ship (game mode 2 with scene 0, 1, 2 or 4), shown otherwise, with the
//! canopy glass's shine drawn over its cockpit. Read from the level00 decomp and disassembly. Native `f32`.
//!
//! **Pvars** (0x44): +0x00 the matrix of its joint list 0 (the cockpit joint), taken every tick for the glass;
//! +0x40 the glass's cross-fade timer (the draw's: `rc-engine`'s fx_draw keeps it).
//!
//! ## Coverage (`0x2d1e80`)
//! | address | what | port |
//! |---|---|---|
//! | state 0 | +0xbc = 1, → 1, the glass timer 0 | [`update`] |
//! | head | game mode 2 (0x15f5c4) with scene 0x16c890 < 3 or = 4: not drawn (+0x31 = 0), mode \|= 1; else drawn, mode &= ~1 | [`update`] |
//! | `0x24f728(m, 0, pvars)` | the joint list 0 matrix into +0x00 (L01 `0x264508`) | [`update`] (`World::joint_matrix`) |
//! | drawn | `RegisterDrawCallback2(0x2d19b0, m)` (L01 `0x2ec910`): the canopy glass (the ships' glass code with this class's tables, its near / far cross-fade) | [`update`] (`Callback::ShipGlass`, drawn by `rc-engine`'s fx_draw) |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 0;
pub const UPDATE_FN: u32 = 0x2d_1e80;
pub const CLASSES: [i16; 1] = [530];

const LEN: usize = 0x44;

/// Level00 `0x2d1e80` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, LEN);
    if w.m(id).state == 0 {
        let m = w.mm(id);
        m.cmd = 1;
        m.state = 1;
        crate::moby_update::services::pvar::set_i32(&mut m.pvars, 0x40, 0);
    }
    let scene = w.svc.cinematic.scene.as_ref().map_or(-1, |s| s.id as i32);
    let hidden = w.svc.game_mode == 2 && (scene < 3 || scene == 4);
    {
        let m = w.mm(id);
        m.visible = !hidden as u8;
        if hidden { m.mode |= 1; } else { m.mode &= 0xfffe; }
    }
    let mat = w.joint_matrix(id, 0);
    {
        let p = &mut w.mm(id).pvars;
        for (r, row) in mat.iter().enumerate() {
            for (c, v) in row.iter().enumerate() { crate::moby_update::services::pvar::set_ff(p, (r * 4 + c) * 4, *v); }
        }
    }
    if !hidden {
        w.svc.draw_callbacks.register2(Callback::ShipGlass, id);
        w.svc.draw_callbacks.matrices.insert(id, mat);
    }
}
