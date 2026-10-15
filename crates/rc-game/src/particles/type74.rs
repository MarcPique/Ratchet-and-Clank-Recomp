//! Particle type 74, a puff carried in a moby's frame: the Sonic Summoner's jets (level01 spawner `PartType74Spawn`
//! 0x28a7a8, update 0x28a908, read from the decomp; the update is the same code on all 19 levels, `overlay-diff`; the
//! only caller the census finds is Hoven's unit U371, class 1242: G-ENM-001).
//!
//! **Spawn** `(moby a0, pos a1, vel a2, desc a3, additive t0)`: byte9 `trunc(4) − 0x60` = 0xa4, ALPHA additive ? 0x48 :
//! 0x44, byte1 0, texture `def[74][0]`, rotation = the low byte of one raw `rand()`, size 0, position = pos, velocity
//! +0x20 = vel.xyz, +0x30 = the offset in the moby's frame `rows · (pos − moby)` (`fun_001fa2d8` transposes the rows,
//! `fun_001f9d20` applies them), +0x2c the moby, +0x3c the descriptor, timer = its life's low half, RGBA = its first
//! colour. The descriptor (the caller's static data): `{rgba0, rgba1, s16 size_range, s16 size_base, i32 life}`.
//!
//! **Update**: size = `((range·(life − t)) / life + base)·1000` (integer division); vel ·= 1 − 0.02·speed; the moby
//! pointer 0 or its state 0xfe / 0xfd → killed. Else offset += vel; position = moby + Σ rows[k]·offset[k]
//! (`MatrixMulVec3`); rotation byte + 1; RGBA = `FastTweenColor(t / life, rgba1, rgba0)` (from the first colour to the
//! second as the timer runs out); then the timer fires → killed. No RNG.
//!
//! **The moby and the descriptor.** The record keeps the moby index + 1 at +0x2c ([`Particles::moby_frames`] gives its
//! position, rows and state; a moby missing there is gone) and the descriptor's index + 1 in
//! [`Particles::descs74`] at +0x3c (the game keeps a pointer to the caller's data). Native `f32`.

use super::{fast_dec_timer, rec, tween_color, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 74;

/// The caller's descriptor (`param_4`): read at spawn (the first colour, the life) and every update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Desc {
    /// +0: the colour at spawn.
    pub rgba0: u32,
    /// +4: the colour as the timer runs out.
    pub rgba1: u32,
    /// +8: the size's range (× 1000).
    pub size_range: i16,
    /// +0xa: the size's base (× 1000).
    pub size_base: i16,
    /// +0xc: the life in ticks (the timer takes its low half).
    pub life: i32,
}

/// The descriptor's index in [`Particles::descs74`] (added once, kept for the level).
pub fn desc_index(sys: &mut Particles, d: Desc) -> usize {
    match sys.descs74.iter().position(|x| *x == d) {
        Some(k) => k,
        None => {
            sys.descs74.push(d);
            sys.descs74.len() - 1
        }
    }
}

/// `PartType74Spawn(moby, pos, vel, desc, additive)` 0x28a7a8 with the moby's position and rows: one `rand()` with a
/// record; none when the pool is full.
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, rng: &mut Rng, moby: usize, moby_pos: [f32; 3], rows: [[f32; 3]; 3], pos: [f32; 4], vel: [f32; 3], desc: Desc, additive: bool) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let k = desc_index(sys, desc);
    let rot = rng.rand() as u8;
    let d = [pos[0] - moby_pos[0], pos[1] - moby_pos[1], pos[2] - moby_pos[2]];
    let local = rows.map(|row| row[0] * d[0] + row[1] * d[1] + row[2] * d[2]);
    let r = &mut sys.pool.recs[i];
    r[9] = 4u8.wrapping_sub(0x60);
    r[3] = if additive { 0x48 } else { 0x44 };
    r[1] = 0;
    r[2] = def;
    r[8] = rot;
    rec::set_ff(r, 0xc, 0.0);
    rec::set_v4(r, 0x10, pos);
    rec::set_v3(r, 0x20, vel);
    rec::set_v3(r, 0x30, local);
    rec::set_u32(r, 0x2c, moby as u32 + 1);
    rec::set_u32(r, 0x3c, k as u32 + 1);
    rec::set_i16(r, 10, desc.life as i16);
    rec::set_u32(r, 4, desc.rgba0);
    Some(i)
}

/// The moby a live type-74 record rides.
pub fn moby_of(r: &Record) -> Option<usize> {
    (r[0] == TYPE && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x2c) as usize).checked_sub(1)).flatten()
}

/// Update 0x28a908 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let Some(d) = (rec::u32(&sys.pool.recs[i], 0x3c) as usize).checked_sub(1).and_then(|k| sys.descs74.get(k).copied()) else {
        sys.kill_part(i);
        return;
    };
    let frame = moby_of(&sys.pool.recs[i]).and_then(|m| sys.moby_frames.get(&m).copied());
    let r = &mut sys.pool.recs[i];
    let t = rec::i16(r, 10) as i32;
    let life = d.life.max(1); // the game traps on 0
    let size = ((d.size_range as i32 * (d.life - t)) / life + d.size_base as i32) as f32;
    rec::set_ff(r, 0xc, size * 1000.0);
    let k = -0.019_999_98f32 + 1.0;
    let v = rec::v3(r, 0x20).map(|x| x * k);
    rec::set_v3(r, 0x20, v);
    let Some(f) = frame.filter(|f| f.state != 0xfe && f.state != 0xfd) else {
        sys.kill_part(i);
        return;
    };
    let o = rec::v3(r, 0x30);
    let o = [o[0] + v[0], o[1] + v[1], o[2] + v[2]];
    rec::set_v3(r, 0x30, o);
    let w: [f32; 3] = std::array::from_fn(|l| f.rows[0][l] * o[0] + f.rows[1][l] * o[1] + f.rows[2][l] * o[2]);
    rec::set_v3(r, 0x10, [f.pos[0] + w[0], f.pos[1] + w[1], f.pos[2] + w[2]]);
    r[8] = r[8].wrapping_add(1);
    let c = tween_color((t as f32 / d.life as f32).to_bits(), d.rgba1, d.rgba0);
    rec::set_u32(r, 4, c);
    if fast_dec_timer(r, 10) != 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::MobyFrame;

    const DESC: Desc = Desc { rgba0: 0x80ff_8040, rgba1: 0x0000_2080, size_range: 30, size_base: 10, life: 20 };

    /// A jet in a moby turned 90° about z: the offset is taken in the moby's frame, the puff rides the moby, grows
    /// from base to base + range, slows by 0.98 a tick, tweens its colour, lives `life` updates; the moby deleted
    /// kills it.
    #[test]
    fn rides_the_moby_grows_and_tweens() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let mut twin = rng;
        let rows = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let i = spawn(&mut s, &mut rng, 3, [10.0, 0.0, 0.0], rows, [10.0, 2.0, 0.0, 1.0], [0.1, 0.0, 0.0], DESC, true).unwrap();
        assert_eq!(s.pool.recs[i][8], twin.rand() as u8);
        assert_eq!(rng, twin);
        let r = &s.pool.recs[i];
        assert_eq!((r[3], r[9], rec::i16(r, 10), rec::u32(r, 4)), (0x48, 0xa4, 20, DESC.rgba0));
        assert_eq!(rec::v3(r, 0x30), [2.0, 0.0, 0.0], "local x = row 0 · (0, 2, 0)");
        s.moby_frames.insert(3, MobyFrame { pos: [5.0, 5.0, 5.0], rows, state: 1, o_class: 1242 });
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        assert_eq!(rec::ff(r, 0xc), 10000.0, "t = life: base");
        let v = 0.1 * (-0.019_999_98f32 + 1.0);
        assert_eq!(rec::ff(r, 0x20), v);
        // world = 5 + row0·(2 + v) = (5, 5 + 2 + v, 5)
        assert_eq!(rec::v3(r, 0x10), [5.0, 5.0 + (2.0 + v), 5.0]);
        assert_eq!(rec::u32(r, 4), tween_color(1.0f32.to_bits(), DESC.rgba1, DESC.rgba0));
        assert_eq!(rec::i16(r, 10), 19);
        let mut n = 1;
        while s.pool.count > 0 && n < 40 {
            s.update_parts(&mut rng);
            n += 1;
        }
        assert_eq!(n, 20, "life updates");
        assert_eq!(rng, twin, "no draws in the update");
        // Deleted moby: killed on the next update.
        spawn(&mut s, &mut rng, 3, [10.0, 0.0, 0.0], rows, [10.0, 2.0, 0.0, 1.0], [0.1, 0.0, 0.0], DESC, false).unwrap();
        s.moby_frames.get_mut(&3).unwrap().state = 0xfe;
        s.update_parts(&mut rng);
        assert_eq!(s.pool.count, 0);
        assert_eq!(s.descs74.len(), 1, "one descriptor kept");
    }
}
