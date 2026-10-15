//! Particle type 32, the pulsing glow that rides the Glove of Doom's canister 230 (level01 spawner `0x283d88`, update
//! `0x283ea8`, read from the decomp; docs/plan/hero_gameplay.md §18). The canister makes one every 4 ticks while held
//! (not in first person), flying, and while it lets its first two bots out.
//!
//! **Spawn** `(moby)`: +0x20 the moby, +0x24 alpha 0, +0x28 the alpha step `0.016·[0x15ed60]`, +0x2c the rotation
//! `randf(0, 1)` (one draw), +0x30 / +0x34 / +0x38 the colour (1, 1, 1), position = the moby's, RGBA
//! `0x270ea0(1, 1, 1, 0)` = 0x00ffffff, byte9 `trunc(4) + 0x40`, byte1 0, ALPHA 0x48 (additive), size
//! `randf(50000, 50000)` (a second draw), rotation byte `trunc(rot·255)`, frame `def[32][0]`.
//!
//! **Update**: the moby gone (its state ≥ 0xf0) → killed. Else it follows the moby; alpha += step: rising past 0.6
//! the step turns negative (the pulse falls back), below 0 → killed (about 76 ticks in all). The colour fades from
//! white by (0.0174, 0.01446, 0.0015) a tick (clamped at 0: it turns blue), RGBA = `0x270ea0(r, g, b, alpha)`; the
//! rotation turns 0.0011 a tick (wrapping at 1); the size = the moby's scale × 3 000 000 + 50 000 (it grows with the
//! canister).
//!
//! **The moby.** The record keeps the moby index + 1 at +0x20; its position and scale come from
//! [`Particles::anchors`] / [`Particles::anchor_scales`], which the canister writes every tick of its update and
//! removes when it is deleted (a moby not listed there is gone). Standard `f32`; `[0x15ed60]` = 1 (NTSC).

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 32;

/// `0x270ea0(r, g, b, a)`: an RGBA word from four 0..1 channels (`trunc(x·255)`; the alpha's full integer shifted).
pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let t = |x: f32| (x * 255.0) as i32 as u32;
    (t(r) & 0xff) | (t(g) & 0xff) << 8 | (t(b) & 0xff) << 16 | t(a) << 24
}

/// `0x283d88(moby)` with the moby's position `at`: two draws (the rotation and the size) with a record; none when the
/// pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, moby: usize, at: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let rot = rng.randf(0.0, 1.0);
    let size = rng.randf(50000.0, 50000.0);
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 0x20, moby as u32 + 1);
    rec::set_ff(r, 0x24, 0.0);
    rec::set_ff(r, 0x28, 0.016);
    rec::set_ff(r, 0x2c, rot);
    rec::set_ff(r, 0x30, 1.0);
    rec::set_ff(r, 0x34, 1.0);
    rec::set_ff(r, 0x38, 1.0);
    rec::set_v4(r, 0x10, at);
    rec::set_u32(r, 4, rgba(1.0, 1.0, 1.0, 0.0));
    r[1] = 0;
    r[3] = 0x48;
    r[9] = 4 + 0x40;
    rec::set_ff(r, 0xc, size);
    r[8] = (rot * 255.0) as i32 as u8;
    r[2] = def;
    Some(i)
}

/// Update 0x283ea8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let m = (rec::u32(&sys.pool.recs[i], 0x20) as usize).checked_sub(1);
    let anchor = m.and_then(|m| Some((*sys.anchors.get(&m)?, *sys.anchor_scales.get(&m)?)));
    let Some((p, scale)) = anchor else {
        sys.kill_part(i);
        return;
    };
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, p);
    let a = rec::ff(r, 0x24) + rec::ff(r, 0x28);
    rec::set_ff(r, 0x24, a);
    if a < 0.0 {
        sys.kill_part(i);
        return;
    }
    let mut k = 1.0;
    if 0.6 < a {
        let s = -rec::ff(r, 0x28);
        rec::set_ff(r, 0x28, s);
        rec::set_ff(r, 0x24, a + s);
        k = 1.0;
    }
    let fade = |r: &mut super::Record, o: usize, d: f32| {
        let x = rec::ff(r, o) + d;
        rec::set_ff(r, o, if x < 0.0 { 0.0 } else { x });
    };
    fade(r, 0x30, k * -0.0174);
    fade(r, 0x34, -0.01446);
    fade(r, 0x38, -0.0015);
    let c = rgba(rec::ff(r, 0x30), rec::ff(r, 0x34), rec::ff(r, 0x38), rec::ff(r, 0x24));
    rec::set_u32(r, 4, c);
    let mut rot = rec::ff(r, 0x2c) + 0.0011;
    if 1.0 < rot { rot -= 1.0; }
    rec::set_ff(r, 0x2c, rot);
    r[8] = (rot * 255.0) as i32 as u8;
    rec::set_ff(r, 0xc, scale * 100000.0 * 30.0 + 50000.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pulse: alpha climbs 0.016 a tick to 0.6, falls back and the record dies below 0; it follows its moby
    /// and grows with its scale; the colour turns blue; a moby no longer listed kills it.
    #[test]
    fn pulses_follows_and_dies() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, &mut rng, 7, [1.0, 2.0, 3.0, 1.0]).unwrap();
        assert_eq!(rec::u32(&s.pool.recs[i], 4), 0x00ff_ffff);
        s.anchors.insert(7, [4.0, 5.0, 6.0]);
        s.anchor_scales.insert(7, 0.01);
        let mut ticks = 0;
        let mut peak = 0.0f32;
        while s.pool.count > 0 && ticks < 200 {
            s.update_parts(&mut rng);
            ticks += 1;
            if s.pool.count > 0 { peak = peak.max(rec::ff(&s.pool.recs[i], 0x24)); }
            if ticks == 1 {
                assert_eq!(rec::pos(&s.pool.recs[i]), [4.0, 5.0, 6.0]);
                assert_eq!(rec::ff(&s.pool.recs[i], 0xc), 0.01 * 100000.0 * 30.0 + 50000.0);
            }
            if ticks == 60 {
                let c = rec::u32(&s.pool.recs[i], 4);
                assert_eq!(c & 0xff, 0, "red gone");
                assert!((c >> 16) & 0xff > 0xe0, "blue kept: {c:#x}");
            }
        }
        // The stored alpha never passes 0.6: the step that crosses it is taken back the same tick.
        assert!((0.58..=0.6).contains(&peak), "peak {peak}");
        assert!((74..=78).contains(&ticks), "lived {ticks}");
        let j = spawn(&mut s, &mut rng, 7, [1.0, 2.0, 3.0, 1.0]).unwrap();
        s.anchors.remove(&7);
        s.update_parts(&mut rng);
        assert_eq!(s.pool.count, 0, "moby gone: {j} killed");
    }
}
