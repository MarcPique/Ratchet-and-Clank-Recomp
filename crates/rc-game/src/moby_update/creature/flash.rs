//! The hit flash on a moby's ambient colour (level01 `0x272318` start, `0x2723f8` update; the ambient is moby
//! +0x3c..+0x3e, `FUN_00265100` reads it and `FUN_002650d0` writes it).
//!
//! The flash record `F` (critter 577 pvar+0x110): +0x00 u16 timer, +0x02 u16 phase (0 in, 1 out), +0x04..+0x06 the
//! saved ambient r, g, b, +0x07..+0x09 the flash colour r, g, b (0 = channel untouched), +0x0c / +0x0e s16 the fade-in
//! / fade-out lengths (`ticks(n)`). On Novalis the critters flash red (0x80, 0, 0) over 4 ticks in, 15 out.

use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

fn u16_(w: &World, id: MobyId, o: usize) -> u16 { super::pi16(w, id, o) as u16 }
fn set_u16(w: &mut World, id: MobyId, o: usize, x: u16) { super::set_pi16(w, id, o, x as i16) }

/// `0x272318(moby, F)`: (re)start the flash. Idle (timer 0) or fading out: fade in over `ticks(F.c)`; from idle the
/// ambient is saved first; from a fade-out the fade-in starts where the fade-out was
/// (`trunc(ticks(F.c) · timer / ticks(F.e))`, at least 1). Already fading in: nothing.
pub fn start(w: &mut World, id: MobyId, f: usize) {
    let old = u16_(w, id, f);
    if old != 0 && u16_(w, id, f + 2) == 0 { return; }
    let tin = super::ticks(w, super::pi16(w, id, f + 0xc) as i32);
    set_u16(w, id, f, tin as u16);
    set_u16(w, id, f + 2, 0);
    if old == 0 {
        let a = w.m(id).ambient;
        for (k, &v) in a.iter().take(3).enumerate() { super::set_pu8(w, id, f + 4 + k, v); }
    } else {
        let tout = super::ticks(w, super::pi16(w, id, f + 0xe) as i32) as f32;
        let t = (tin as f32 * (old as i16 as f32 / tout)) as i32 as i16;
        set_u16(w, id, f, if t < 1 { 1 } else { t as u16 });
    }
}

/// `0x2723f8(moby, F)`: one tick of the flash: the ambient eased from the saved colour to the flash colour over the
/// fade-in, back over the fade-out, restored when it ends.
pub fn update(w: &mut World, id: MobyId, f: usize) {
    if u16_(w, id, f) == 0 { return; }
    let r = super::dec_timer_pvar_s16(w, id, f);
    if r != 0 {
        if u16_(w, id, f + 2) != 0 {
            let a = [super::pu8(w, id, f + 4), super::pu8(w, id, f + 5), super::pu8(w, id, f + 6)];
            set_ambient(w, id, a);
            return;
        }
        set_u16(w, id, f + 2, 1);
        let t = super::ticks(w, super::pi16(w, id, f + 0xe) as i32);
        set_u16(w, id, f, t as u16);
    }
    let out = u16_(w, id, f + 2) != 0;
    let len = super::ticks(w, super::pi16(w, id, f + if out { 0xe } else { 0xc }) as i32) as f32;
    let k = (len - u16_(w, id, f) as i16 as f32) / len;
    let mut a = [0u8; 3];
    for (c, slot) in a.iter_mut().enumerate() {
        let base = super::pu8(w, id, f + 4 + c);
        let hi = super::pu8(w, id, f + 7 + c);
        *slot = if hi == 0 {
            base
        } else if out {
            (hi as f32 + (base as i32 - hi as i32) as f32 * k) as i32 as u8
        } else {
            (base as f32 + (hi as i32 - base as i32) as f32 * k) as i32 as u8
        };
    }
    set_ambient(w, id, a);
}

/// `FUN_002650d0(moby, r, g, b)`.
fn set_ambient(w: &mut World, id: MobyId, a: [u8; 3]) {
    let m = w.mm(id);
    m.ambient[0] = a[0];
    m.ambient[1] = a[1];
    m.ambient[2] = a[2];
}
