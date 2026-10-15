//! The particle pool, allocator and per-tick scheduler (docs/plan/particles.md §2–§4), level01 addresses.
//!
//! * **Pool**: 2048 records × 0x40 bytes kept as raw bytes (the per-type fields at +0x20.. are addressed by
//!   offset, as the game does), allocation bitmap 0x1b1d00, second bitmap 0x1b1e00 (never set in retail: flag
//!   0x40 is never written), lowest-free hint gp−0x6a90, highest-ever-live index gp−0x6a8c ("hw", −1 when
//!   empty), live count gp−0x6a88. [`PartPool::level_init`] (level init 0x255958) resets the counters and
//!   bitmaps but **not** the records.
//! * **[`PartPool::create_part`]** (`CreatePart` 0x27c498 → `CreatePart2` 0x27c4a0): takes the hint slot, sets
//!   its bit, `hw = max(hw, i)` (`pmaxw`), new hint = `i + 1` when `i > old hw`, else the first clear bit
//!   scanning from bit 0 of the byte holding `i + 1` (0x800 when none). Record: `sq zero, 0x0` then **three**
//!   `sq zero, 0x10` (0x27c5c8..d4), so bytes 0x20..0x3f keep the previous occupant's data; byte 0 = type.
//! * **[`PartPool::kill_part`]** (0x27c5e8): count−−; clear the bitmap-2 bit if byte1 & 0x40; clear the bit;
//!   if `i == hw`, hw = the next lower set bit (−1 if none); byte1 = 0x80; hint = `min(hint, i)` (`pminw`).
//! * **[`Particles::update_parts`]** (`UpdateParts` 0x27c7e8): snapshots `end = hw + 1` at entry and calls
//!   `table[byte0](rec)` for every record whose byte1 bit 7 is clear, in index order. Records created during
//!   the pass above the old hw wait a tick; ones created in a hole ahead of the cursor run this tick.
//! * **Type table** (`RegisterPartTypes` 0x27d4e0 → 0x1b2300, 81 entries): [`Particles::table`]. Ported: type 2
//!   ([`type02`], bomb trail / amoeboid goo), 4 ([`type04`], fireball smoke), 6
//!   ([`type06`], class-27 emitters), 8 ([`type08`], explosion puffs), 11 ([`type11`], TNT sparks, smoke rings), 12
//!   ([`type12`], the Pyrocitor's flames), 13
//!   ([`type13`], crate-break dust), 15 ([`type15`], explosion streaks), 52 ([`type52`], goo drips, flat), 25
//!   ([`type25`], grind / cable sparks), 34 ([`type34`], bubbles), 47 ([`type47`], dust / sand puffs), 53 ([`type53`], bolt-pickup and cable
//!   sparkles), 59 ([`type59`], hero sparkles), 60 ([`type60`], glints), 62 ([`type62`], the nanotech orbs and their
//!   trails), and (2026-09-27) 16 ([`type16`], smoke), 19 / 55 ([`type19`], ribbons), 22 ([`type22`], rising puffs),
//!   23 ([`type23`], glow puffs), 26 ([`type26`], moby glow), 35 ([`type35`], drops), 45 / 66 ([`type45`], flat
//!   rings), 46 ([`type46`], water rings), 64 ([`type64`], bursting scorch), and (2026-09-28) 32 ([`type32`], the Glove of Doom canister's glow),
//!   and (2026-09-29, G-PRT-001 / G-PRT-007, docs/plan/particles.md "Update types, 2026-09-29") 0 / 73 / 1 ([`type00`],
//!   [`type73`], [`type01`]: the weather's streaks, flakes and splashes, `SpawnImpactSparks`), 5 / 7 ([`type05`]), 10
//!   ([`type10`]), 14 ([`type14`]), 18 ([`type18`]), 28 ([`type28`]), 31 ([`type31`]), 39 ([`type39`]), 41
//!   ([`type41`]), 43 ([`type43`]), 48 / 50 ([`type48`]), 49 ([`type49`]), 51 ([`type51`]), 61 ([`type61`]), 65 (a row
//!   on [`type25`]), 67 ([`type67`]), 68 ([`type68`]), 69 ([`type69`]), 70 ([`type70`]), 71 ([`type71`]), 74 ([`type74`]), 77
//!   ([`type77`]), 78 ([`type78`]), 79 / 80 ([`type79`]); a record of any other type kills itself on its first update
//!   and is counted in [`PartStats::unported_kills`], so a missing type is visible in the stats line.
//! * **Mobys held by pointer** (types 14, 31, 39, 61, 67, 68, 74, 78, 79): the record keeps the moby index + 1; the moby
//!   loop publishes those mobys ([`Particles::moby_frames`]: position, rows, state, class) and type 61 / 68's joints
//!   ([`Particles::joint_frames`]) after its pass ([`Particles::moby_refs`] lists them), and each update applies the
//!   game's own gone test.
//! * **RNG.** Updates draw from the `&mut Rng` given to [`Particles::update_parts`] (the game's one stream), in
//!   pool order; only types 11 (its split spawns five children and its phase changes draw one value), 16 (the landing
//!   smoke), 35 (its rings' spawns), 55 (two a tick), 64 (the burst), 15 (a
//!   splitting streak's child: 6 + 1) and 34 (a bubble near the surface draws one value a tick) draw.
//!
//! **Tick placement.** Game-state update 0x2a4080: moby updates (0x2793d8, where the class-27 emitters spawn)
//! → level callbacks 0x2a1a18 → hero 0x228870 → `UpdateParts` → camera → render. The engine has no gameplay
//! tick yet: it runs [`type06::emitter_update`] for the emitters and then [`Particles::update_parts`] once per
//! 60 Hz `FixedUpdate` tick (the fixed clock of `rc-engine`'s `determinism.rs`), i.e. after the (not yet ported)
//! moby updates and before the camera.
//!
//! **Floats** follow the EE FPU / VU0 model of [`crate::ps2v`] (round toward zero, no denormals); every field is
//! read and written as PS2 bit patterns.
//!
//! **Owner pvars.** Type 6 keeps a pointer to its owner's pvars at +0x2c. The port stores the index of the owner
//! in [`Particles::owners`] there, and the owner's pvar bytes live in [`Owner::pvars`] (mutable: the emitter
//! keeps its countdown at +0xc8 and its moby at +0x7c), copied from `rc_formats::gameplay::parse_pvars`
//! (the loader's table walk and fixups) when the level is set up.

pub mod type00;
pub mod type01;
pub mod type02;
pub mod type04;
pub mod type05;
pub mod type06;
pub mod type08;
pub mod type10;
pub mod type11;
pub mod type12;
pub mod type13;
pub mod type14;
pub mod type15;
pub mod type16;
pub mod type18;
pub mod type19;
pub mod type21;
pub mod type22;
pub mod type23;
pub mod type25;
pub mod type26;
pub mod type27;
pub mod type28;
pub mod type31;
pub mod type32;
pub mod type34;
pub mod type35;
pub mod type39;
pub mod type41;
pub mod type43;
pub mod type44;
pub mod type45;
pub mod type46;
pub mod type47;
pub mod type48;
pub mod type49;
pub mod type51;
pub mod type52;
pub mod type53;
pub mod type56;
pub mod type57;
pub mod type40;
pub mod type58;
pub mod type59;
pub mod type60;
pub mod type61;
pub mod type62;
pub mod type64;
pub mod type67;
pub mod type68;
pub mod type69;
pub mod type70;
pub mod type71;
pub mod type72;
pub mod type73;
pub mod type74;
pub mod type78;
pub mod type77;
pub mod type79;

use crate::ps2v::{self, F};
use crate::rng::Rng;
use rc_formats::particle_tex::PartDefs;

pub const POOL_RECORDS: usize = 2048;
pub const RECORD_SIZE: usize = 0x40;
/// Entries of the update table 0x1b2300.
pub const PART_TYPES: usize = 81;

/// One raw particle record.
pub type Record = [u8; RECORD_SIZE];

/// Byte-level accessors on a record (little-endian, PS2 float bits for `f`).
pub mod rec {
    use super::{Record, F};
    #[inline]
    pub fn u32(r: &Record, o: usize) -> u32 { u32::from_le_bytes(r[o..o + 4].try_into().unwrap()) }
    #[inline]
    pub fn set_u32(r: &mut Record, o: usize, v: u32) { r[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
    #[inline]
    pub fn f(r: &Record, o: usize) -> F { u32(r, o) }
    #[inline]
    pub fn set_f(r: &mut Record, o: usize, v: F) { set_u32(r, o, v); }
    #[inline]
    pub fn i16(r: &Record, o: usize) -> i16 { i16::from_le_bytes([r[o], r[o + 1]]) }
    #[inline]
    pub fn set_i16(r: &mut Record, o: usize, v: i16) { r[o..o + 2].copy_from_slice(&v.to_le_bytes()); }
    /// Position (+0x10, xyz) as `f32`.
    pub fn pos(r: &Record) -> [f32; 3] { [0x10, 0x14, 0x18].map(|o| f32::from_bits(u32(r, o))) }
    /// An `f32` field (native arithmetic; the value is stored as its bits).
    #[inline]
    pub fn ff(r: &Record, o: usize) -> f32 { f32::from_bits(u32(r, o)) }
    #[inline]
    pub fn set_ff(r: &mut Record, o: usize, v: f32) { set_u32(r, o, v.to_bits()); }
    /// Three `f32` at `o` (a vector's xyz).
    pub fn v3(r: &Record, o: usize) -> [f32; 3] { [ff(r, o), ff(r, o + 4), ff(r, o + 8)] }
    pub fn set_v3(r: &mut Record, o: usize, v: [f32; 3]) { for (k, x) in v.iter().enumerate() { set_ff(r, o + 4 * k, *x); } }
    /// Four `f32` at `o` (a vector's xyzw).
    pub fn v4(r: &Record, o: usize) -> [f32; 4] { [ff(r, o), ff(r, o + 4), ff(r, o + 8), ff(r, o + 12)] }
    /// Four `f32` at `o`.
    pub fn set_v4(r: &mut Record, o: usize, v: [f32; 4]) { for (k, x) in v.iter().enumerate() { set_ff(r, o + 4 * k, *x); } }
}

/// Record byte 1 flags.
pub const FLAG_DEAD: u8 = 0x80;
pub const FLAG_BITMAP2: u8 = 0x40;
/// Set by `PartProc` on records it drew this frame, cleared when a previously drawn record is culled.
pub const FLAG_DRAWN: u8 = 0x20;

/// The 2048-record pool with the game's allocator state.
#[derive(Clone)]
pub struct PartPool {
    pub recs: Box<[Record]>,
    /// 0x1b1d00: bit i = record i allocated.
    pub bitmap: [u8; 256],
    /// 0x1b1e00: only touched for records with byte1 & 0x40 (never in retail).
    pub bitmap2: [u8; 256],
    /// gp−0x6a90: lowest free index (0x800 = full).
    pub hint: i32,
    /// gp−0x6a8c: highest index ever live since the last kill of the top record, −1 when empty.
    pub hw: i32,
    /// gp−0x6a88: live records.
    pub count: i32,
}

impl Default for PartPool {
    fn default() -> Self { Self::new() }
}

impl PartPool {
    /// A pool in the level-init state. The records start as zero bytes: on the PS2 the pool is placed after
    /// other level data and its previous contents are unknown (level init does not clear it).
    pub fn new() -> Self {
        PartPool { recs: vec![[0u8; RECORD_SIZE]; POOL_RECORDS].into_boxed_slice(), bitmap: [0; 256], bitmap2: [0; 256], hint: 0, hw: -1, count: 0 }
    }

    /// Level init 0x255958: hint 0, hw −1, count 0, both bitmaps zero; records untouched.
    pub fn level_init(&mut self) {
        self.hint = 0;
        self.hw = -1;
        self.count = 0;
        self.bitmap = [0; 256];
        self.bitmap2 = [0; 256];
    }

    fn bit(&self, i: usize) -> bool { self.bitmap[i >> 3] & (1 << (i & 7)) != 0 }

    /// `CreatePart(type)` 0x27c498/0x27c4a0: the record index, or `None` when the pool is full.
    pub fn create_part(&mut self, ty: u8) -> Option<usize> {
        let i = self.hint;
        if i - 0x800 >= 0 { return None; }
        let iu = i as usize;
        self.bitmap[iu >> 3] |= 1 << (iu & 7);
        let mut next = i + 1;
        if i - self.hw < 1 {
            // Scan from bit 0 of the byte holding i + 1 for the first clear bit.
            let mut byte = (next as u32 >> 3) as usize;
            next = (byte << 3) as i32;
            'scan: while next - 0x800 < 0 {
                let mut m: u32 = 1;
                loop {
                    let set = self.bitmap[byte] as u32 & m;
                    m <<= 1;
                    if set == 0 { break 'scan; }
                    next += 1;
                    if m & 0xff == 0 { break; }
                }
                byte += 1;
            }
        }
        self.hint = next;
        self.hw = self.hw.max(i); // pmaxw
        self.count += 1;
        let r = &mut self.recs[iu];
        r[..0x10].fill(0); // sq zero, 0x0(v0)
        for _ in 0..3 { r[0x10..0x20].fill(0); } // sq zero, 0x10(v0) three times
        r[0] = ty;
        Some(iu)
    }

    /// `KillPart(rec)` 0x27c5e8.
    pub fn kill_part(&mut self, i: usize) {
        self.count -= 1;
        let m = 1u8 << (i & 7);
        if self.recs[i][1] & FLAG_BITMAP2 != 0 { self.bitmap2[i >> 3] &= !m; }
        self.bitmap[i >> 3] &= !m;
        if i as i32 == self.hw {
            let mut h = self.hw - 1;
            while h >= 0 && !self.bit(h as usize) { h -= 1; }
            self.hw = h.max(-1);
        }
        self.recs[i][1] = FLAG_DEAD;
        self.hint = self.hint.min(i as i32); // pminw
    }

    /// Records the renderer walks: `0..=hw` with byte1 bit 7 clear.
    pub fn live(&self) -> impl Iterator<Item = (usize, &Record)> {
        self.recs[..(self.hw + 1).max(0) as usize].iter().enumerate().filter(|(_, r)| r[1] & FLAG_DEAD == 0)
    }
}

/// Time base (`fun_00214970`): 0x15ed60 speed, 0x15ed68 timer scale and 0x15ed70 dt².
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeBase {
    /// 0x15ed60 = gp−0x7ea0: 1.0 NTSC, 1.2 PAL.
    pub speed: F,
    /// 0x15ed68 = gp−0x7e98: 1.0 NTSC, 0.8333 PAL.
    pub timer_scale: F,
    /// 0x15ed70 = gp−0x7e90: dt² (1/3600 NTSC = 0x3991a2b4, 1/2500 PAL).
    pub dt2: F,
}

impl TimeBase {
    pub const NTSC: TimeBase = TimeBase { speed: 0x3f80_0000, timer_scale: 0x3f80_0000, dt2: 0x3991_a2b4 };

    /// 0x220e30: `cvt.w.s(0.25 + 0.25 + (f32)n · timer_scale)` (`adda.s` then `madd.s`).
    pub fn ticks(&self, n: i32) -> i32 {
        let acc = ps2v::add(0x3e80_0000, 0x3e80_0000);
        ps2v::ftoi0(ps2v::add(acc, ps2v::mul(ps2v::itof0(n), self.timer_scale)))
    }
}

/// `FastDecTimer__FRs` 0x220ea8 on the s16 at `r[o]`: 1 if it is 0 (not written), else t = max(t, 1) − 1 and
/// 2 when t ≤ 0 (the kill signal), 0 otherwise.
pub fn fast_dec_timer(r: &mut Record, o: usize) -> i32 {
    let t = rec::i16(r, o) as i32;
    if t == 0 { return 1; }
    let t = t.max(1) - 1;
    rec::set_i16(r, o, t as i16);
    if t > 0 { 0 } else { 2 }
}

/// `FUN_00220ed8` on the u8 at `r[o]`: [`fast_dec_timer`] on a byte (1 if 0, else t = max(t, 1) − 1 and 2 when
/// it reaches 0).
pub fn fast_dec_timer_u8(r: &mut Record, o: usize) -> i32 {
    let t = r[o];
    if t == 0 { return 1; }
    let t = t - 1;
    r[o] = t;
    if t > 0 { 0 } else { 2 }
}

/// `FastTweenColor(f, a, b)` 0x2221a8 on VU0: per byte (R low) `ftoi0(itof0(a)·(1 − f) + itof0(b)·f)` (`vsubx.w`
/// 1 − f, `vmulaw` then `vmaddx`), packed back with `ppach`/`ppacb` (the low byte of each lane).
pub fn tween_color(f: F, a: u32, b: u32) -> u32 {
    let g = ps2v::sub(ps2v::ONE, f);
    (0..4).fold(0u32, |out, k| {
        let (ca, cb) = (ps2v::itof0((a >> (8 * k) & 0xff) as i32), ps2v::itof0((b >> (8 * k) & 0xff) as i32));
        let v = ps2v::ftoi0(ps2v::add(ps2v::mul(ca, g), ps2v::mul(cb, f)));
        out | (v as u32 & 0xff) << (8 * k)
    })
}

/// Signed-magnitude order of PS2 floats, as `c.lt.s`/`c.le.s` see them (−0 = +0, exponent 255 finite).
fn key(x: F) -> i64 { if x & ps2v::SIGN != 0 { -((x & ps2v::MAX) as i64) } else { x as i64 } }
/// `c.lt.s a, b`.
pub fn lt(a: F, b: F) -> bool { key(a) < key(b) }
/// `c.le.s a, b`.
pub fn le(a: F, b: F) -> bool { key(a) <= key(b) }

/// An owner of type-6 particles: a class-27 emitter moby (the fields its update reads).
#[derive(Clone, Debug)]
pub struct Owner {
    /// Gameplay instance index (for reports; the runtime moby index on the PS2).
    pub instance: usize,
    /// moby+0x10: position (xyz). moby+0x1c is not modelled (0): the game copies it into particle +0x1c,
    /// which kind-0 sprites never read.
    pub pos: [f32; 4],
    /// moby+0x40: Euler rotation (radians).
    pub rot: [f32; 3],
    /// The instance's pvar block (0xe0 bytes for class 27), mutable run-time state.
    pub pvars: Vec<u8>,
}

impl Owner {
    pub fn pf(&self, o: usize) -> F { u32::from_le_bytes(self.pvars[o..o + 4].try_into().unwrap()) }
    pub fn pu32(&self, o: usize) -> u32 { self.pf(o) }
    pub fn set_pu32(&mut self, o: usize, v: u32) { self.pvars[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
}

/// Counters for the `RC_PART_STATS` line.
#[derive(Clone, Debug)]
pub struct PartStats {
    pub created: u64,
    pub create_failed: u64,
    pub killed: u64,
    /// Kills of records whose type has no ported update, per type.
    pub unported_kills: [u64; PART_TYPES],
    /// Type-6 updates that would have run the collision branch (pvar +0x70 bit 0), which is not ported.
    pub unported_collision: u64,
    /// Emitter spawns that needed `fast_add_rotations` paths or other unported branches.
    pub unported_emitter: u64,
    /// Updates that took a branch of a ported type no known spawner reaches (type 69's modes 1–4).
    pub unported_branch: u64,
}

impl Default for PartStats {
    fn default() -> Self {
        PartStats { created: 0, create_failed: 0, killed: 0, unported_kills: [0; PART_TYPES], unported_collision: 0, unported_emitter: 0, unported_branch: 0 }
    }
}

/// A per-record update: `table[byte0](rec)`, drawing from the shared stream.
pub type UpdateFn = fn(&mut Particles, usize, &mut Rng);

/// The particle system state the game keeps in globals.
pub struct Particles {
    pub pool: PartPool,
    pub time: TimeBase,
    /// 0x1b2300.
    pub table: [Option<UpdateFn>; PART_TYPES],
    /// `part_defs` (0x1b2500 / 0x1b2700): spawners take `*def[n]` as the first frame.
    pub defs: Option<PartDefs>,
    /// Owner mobys referenced from records (+0x2c of type 6) and by index from pvar +0x7c.
    pub owners: Vec<Owner>,
    pub stats: PartStats,
    /// 0x15f5d0 / 0x15f5d4: the frame-load measures the type-11 spawner throttles on (0.85 / 0.9 / 1.0) and its
    /// smoke phase reads (an extra timer step above 1.0). Not modelled yet: 0 (no throttle).
    pub frame_load: [F; 2],
    /// 0x167240: the camera position as the previous tick's camera update left it (type 11 shrinks sparks
    /// within 10 units of it). The engine sets it before [`Particles::update_parts`].
    pub camera: [F; 3],
    /// The level's collision mesh for the updates that test lines against the world (type 25's `CollLine_Fix`);
    /// None: they never hit.
    pub coll: Option<std::sync::Arc<rc_formats::collision::Collision>>,
    /// 0x13f3d0: the hero position (the attached glints of type 60 follow it), set by the particle hook.
    pub hero: [f32; 3],
    /// 0x13f420: the hero's body point (type 71's motes drift toward a barrier's touch point near it), set by the hook.
    pub hero_body: [f32; 3],
    /// 0x167258: the camera yaw as the previous tick's camera update left it (type 34's wobble), set by the hook.
    pub cam_yaw: f32,
    /// 0x15f5cc: the tick counter as the tick's updates see it (types 2 and 15 act on odd ticks), set by the hook.
    pub counter: u64,
    /// Moby positions for the records attached to a moby (type 62 kind 2 keeps the moby pointer at +0x24 and
    /// follows it), by moby index, written by the owning class during the moby loop.
    pub anchors: std::collections::HashMap<usize, [f32; 3]>,
    /// The scales of the mobys in [`Particles::anchors`] that type-32 records follow (the Glove of Doom's canister
    /// writes its position and scale every tick and removes both when it is deleted: [`type32`]).
    pub anchor_scales: std::collections::HashMap<usize, f32>,
    /// The points of the mobys the live type-26 / 55 records follow, by (moby, joint list; −1 = the moby's position,
    /// type 26 moved 0.4 toward the camera), written by the moby loop at the end of its pass
    /// (`moby_update::services::World::refresh_particle_anchors`); a moby that is gone is not listed.
    pub joint_anchors: std::collections::HashMap<(usize, i16), [f32; 3]>,
    /// 0x13f640: the water level (the hero's; type 35's kind 3 lands on it).
    pub water_z: f32,
    /// 0x15ed84: the level number (type 64 adds rings and smoke on level 10).
    pub level: u32,
    /// 0x13e530: the Pyrocitor's gold flag (type 12's weapon flames grow and last longer when gold).
    pub gold: u8,
    /// Records a moby owns by pointer in its pvars (the Blaster shot's trail, type 72: `(moby, slot) → record`),
    /// written where the hero code's queued spawns are created (`crate::hero::fx::create_particles`); the owner
    /// reads and forgets them.
    pub links: std::collections::HashMap<(usize, u8), usize>,
    /// 0x13f490: Ratchet's platform motion this tick (type 44's puffs ride it), set by the particle hook.
    pub hero_plat: [f32; 3],
    /// The mobys the live records hold by pointer ([`Particles::moby_refs`]: types 67, 68, 74, 78), as their updates
    /// read them, written by the moby loop after its pass (`moby_update::services::World::refresh_particle_anchors`).
    /// A moby missing here is gone (its slot left the table); the per-type updates apply the game's own state tests.
    pub moby_frames: std::collections::HashMap<usize, MobyFrame>,
    /// The world matrices of the joints the live type-68 records hold (`MobyAttachToJoint` 0x264508 of (moby, joint
    /// list): rows 0–2 the joint's axes, row 3 its point), written with [`Particles::moby_frames`].
    pub joint_frames: std::collections::HashMap<(usize, u8), [[f32; 4]; 4]>,
    /// The points type 69's modes 1 / 2 / 3 copy out of a moby's pvar block (+0xd0, +0x1f0, +0xe0: [`type69::moby_of`]),
    /// written with [`Particles::moby_frames`] (`moby_update::services::pvar_block_point`).
    pub pvar_points: std::collections::HashMap<usize, [[f32; 3]; 3]>,
    /// The point at +0x00 of the pvar blocks type-71 records hold (the energy barrier's touch point), written with
    /// [`Particles::moby_frames`].
    pub pvar_heads: std::collections::HashMap<usize, [f32; 3]>,
    /// The descriptors type-74 records point at (+0x3c: the caller's static data), added by [`type74::spawn`].
    pub descs74: Vec<type74::Desc>,
    /// 0x13f5e0: the gravity direction (type 78 homes around it); (0, 0, −1) unless the hero hook writes it.
    pub gravity: [f32; 3],
    /// The level's height grid (core +0xa4, `rc_formats::level::HeightGrid`): the weather particles (types 0, 73 and
    /// `SpawnImpactSparks`) read it; None on levels without one.
    pub grid: Option<std::sync::Arc<rc_formats::level::HeightGrid>>,
    /// The weather globals the weather emitter 1400 writes and types 0 / 73 read.
    pub weather: Weather,
    /// The hits the updates give (type 58's emitter: `0x26eaa8` from inside `UpdateParts`), delivered by the moby
    /// loop's next tick (`moby_update::scheduler`; G-PRT-008).
    pub hits: Vec<type58::Hit>,
    /// The fresh type-40 flames' lines this tick, run against the mobys by the moby loop's next tick (G-PRT-008).
    pub lines: Vec<type40::Line>,
}

/// The weather globals (level 08's 1400 update `0x307cf0` writes them each tick; levels without weather leave them 0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Weather {
    /// 0x160250: the camera's move this tick (camera 0x1675c0 − the emitter's last camera), added to every drop.
    pub wind: [f32; 4],
    /// 0x160260: the lowest a drop falls (camera z − the emitter's depth).
    pub floor: f32,
}

/// A moby as the particle updates that keep a moby pointer read it: +0x10 its position, +0x20 its state, +0xa6 its
/// class, +0xc0 its rows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MobyFrame {
    pub pos: [f32; 3],
    pub rows: [[f32; 3]; 3],
    pub state: u8,
    pub o_class: i16,
}

impl Particles {
    /// Level-init state with the ported update table.
    pub fn new(defs: Option<PartDefs>, owners: Vec<Owner>) -> Self {
        let mut table: [Option<UpdateFn>; PART_TYPES] = [None; PART_TYPES];
        table[0] = Some(type00::update as UpdateFn);
        table[1] = Some(type01::update as UpdateFn);
        table[2] = Some(type02::update as UpdateFn);
        table[4] = Some(type04::update as UpdateFn);
        table[5] = Some(type05::update as UpdateFn);
        table[6] = Some(type06::update as UpdateFn);
        // Type 7's update 0x27f200 is type 5's code (one cluster hash): type05's module doc.
        table[7] = Some(type05::update as UpdateFn);
        table[8] = Some(type08::update as UpdateFn);
        table[10] = Some(type10::update as UpdateFn);
        table[11] = Some(type11::update as UpdateFn);
        table[12] = Some(type12::update as UpdateFn);
        table[13] = Some(type13::update as UpdateFn);
        table[14] = Some(type14::update as UpdateFn);
        table[15] = Some(type15::update as UpdateFn);
        table[16] = Some(type16::update as UpdateFn);
        table[18] = Some(type18::update as UpdateFn);
        table[19] = Some(type19::update19 as UpdateFn);
        table[21] = Some(type21::update as UpdateFn);
        table[22] = Some(type22::update as UpdateFn);
        table[23] = Some(type23::update as UpdateFn);
        table[25] = Some(type25::update as UpdateFn);
        table[26] = Some(type26::update as UpdateFn);
        table[27] = Some(type27::update as UpdateFn);
        table[28] = Some(type28::update as UpdateFn);
        table[31] = Some(type31::update as UpdateFn);
        table[32] = Some(type32::update as UpdateFn);
        table[34] = Some(type34::update as UpdateFn);
        table[35] = Some(type35::update as UpdateFn);
        table[39] = Some(type39::update as UpdateFn);
        table[41] = Some(type41::update as UpdateFn);
        table[43] = Some(type43::update as UpdateFn);
        table[44] = Some(type44::update as UpdateFn);
        table[45] = Some(type45::update45 as UpdateFn);
        table[46] = Some(type46::update as UpdateFn);
        table[47] = Some(type47::update as UpdateFn);
        table[48] = Some(type48::update48 as UpdateFn);
        table[49] = Some(type49::update as UpdateFn);
        table[50] = Some(type48::update50 as UpdateFn);
        table[51] = Some(type51::update as UpdateFn);
        table[52] = Some(type52::update as UpdateFn);
        table[53] = Some(type53::update as UpdateFn);
        table[56] = Some(type56::update as UpdateFn);
        table[57] = Some(type57::update as UpdateFn);
        table[40] = Some(type40::update as UpdateFn);
        table[58] = Some(type58::update as UpdateFn);
        table[59] = Some(type59::update as UpdateFn);
        table[60] = Some(type60::update as UpdateFn);
        table[55] = Some(type19::update55 as UpdateFn);
        table[61] = Some(type61::update as UpdateFn);
        table[62] = Some(type62::update as UpdateFn);
        table[64] = Some(type64::update as UpdateFn);
        // Type 65's update 0x289388 is 0x282760 less the variant branch its spawner never reaches: type25's doc.
        table[65] = Some(type25::update as UpdateFn);
        table[66] = Some(type45::update66 as UpdateFn);
        table[67] = Some(type67::update as UpdateFn);
        table[68] = Some(type68::update as UpdateFn);
        table[69] = Some(type69::update as UpdateFn);
        table[70] = Some(type70::update as UpdateFn);
        table[71] = Some(type71::update as UpdateFn);
        table[72] = Some(type72::update as UpdateFn);
        table[73] = Some(type73::update as UpdateFn);
        table[74] = Some(type74::update as UpdateFn);
        table[77] = Some(type77::update as UpdateFn);
        table[78] = Some(type78::update as UpdateFn);
        table[79] = Some(type79::update79 as UpdateFn);
        table[80] = Some(type79::update80 as UpdateFn);
        Particles { pool: PartPool::new(), time: TimeBase::NTSC, table, defs, owners, stats: PartStats::default(), frame_load: [0; 2], camera: [0; 3], coll: None, hero: [0.0; 3], hero_body: [0.0; 3], cam_yaw: 0.0, counter: 0, anchors: Default::default(), anchor_scales: Default::default(), joint_anchors: Default::default(), water_z: 0.0, level: 0, gold: 0, links: Default::default(), hero_plat: [0.0; 3], moby_frames: Default::default(), joint_frames: Default::default(), pvar_points: Default::default(), pvar_heads: Default::default(), descs74: Vec::new(), gravity: [0.0, 0.0, -1.0], grid: None, weather: Weather::default(), hits: Vec::new(), lines: Vec::new() }
    }

    pub fn create_part(&mut self, ty: u8) -> Option<usize> {
        let r = self.pool.create_part(ty);
        if r.is_some() { self.stats.created += 1 } else { self.stats.create_failed += 1 }
        r
    }

    pub fn kill_part(&mut self, i: usize) {
        self.pool.kill_part(i);
        self.stats.killed += 1;
    }

    /// `*def[n]` (0 when the level has no defs).
    pub fn def_first(&self, n: u8) -> u8 { self.defs.as_ref().and_then(|d| d.first_frame(n as usize)).unwrap_or(0) }

    /// `*(def[n] + k)` (0 when the level has no defs).
    pub fn def_frame(&self, n: u8, k: usize) -> u8 { self.defs.as_ref().and_then(|d| d.start(n as usize).and_then(|s| d.blob.get(s + k).copied())).unwrap_or(0) }

    /// `UpdateParts` 0x27c7e8, with the game's `rand` stream.
    pub fn update_parts(&mut self, rng: &mut Rng) {
        let end = (self.pool.hw + 1).max(0) as usize;
        for i in 0..end {
            if self.pool.recs[i][1] & FLAG_DEAD != 0 { continue; }
            let ty = self.pool.recs[i][0] as usize;
            match self.table.get(ty).copied().flatten() {
                Some(f) => f(self, i, rng),
                None => {
                    self.stats.unported_kills[ty.min(PART_TYPES - 1)] += 1;
                    self.kill_part(i);
                }
            }
        }
    }

    /// The mobys (and the joints) the live records hold by pointer, for [`Particles::moby_frames`] /
    /// [`Particles::joint_frames`]: type 68's / 61's moby and joint list, type 74's, 67's, 14's, 79's, 39's and 31's moby, type 78's target.
    pub fn moby_refs(&self) -> (Vec<usize>, Vec<(usize, u8)>) {
        let (mut mobys, mut joints) = (Vec::new(), Vec::new());
        for (_, r) in self.pool.live() {
            if let Some((m, l)) = type68::joint_of(r).or_else(|| type61::joint_of(r)) {
                mobys.push(m);
                joints.push((m, l));
            }
            mobys.extend(type74::moby_of(r).or_else(|| type78::moby_of(r)).or_else(|| type67::moby_of(r)).or_else(|| type14::moby_of(r)).or_else(|| type79::moby_of(r)).or_else(|| type39::moby_of(r)).or_else(|| type31::moby_of(r)).or_else(|| type69::moby_of(r)).or_else(|| type71::moby_of(r)));
        }
        mobys.sort_unstable();
        mobys.dedup();
        joints.sort_unstable();
        joints.dedup();
        (mobys, joints)
    }

    /// Live records per type.
    pub fn live_by_type(&self) -> [u32; PART_TYPES] {
        let mut n = [0u32; PART_TYPES];
        for (_, r) in self.pool.live() { n[(r[0] as usize).min(PART_TYPES - 1)] += 1; }
        n
    }
}

/// The view data `FastBSphereCheck` (level01 0x2221f0) reads from 0x16d140.., written by the camera matrix
/// builder (`fun_001f2260`, 0x218a48) and `UpdateViewContext` (0x219580) during the previous frame's render.
#[derive(Clone, Copy, Debug)]
pub struct BSphereView {
    /// 0x16d140/150/160 = the rotation-only view 0x167100, row j = camera-space image of world axis j
    /// (camera x right = −left, y down = −up, z forward).
    pub rows: [[F; 3]; 3],
    /// 0x16d170 = camera position 0x167240 × 1024 (`fun_001f9a68(1024, ...)`), w = 1024.
    pub cam: [F; 3],
    /// 0x16d180.xy = (tan_x, tan_y) = (0.63, 0.63·0.775).
    pub tan: [F; 2],
    /// 0x16d1a0.xy = 1/cos(atan(tan)) (`FastArcTan` + `fast_cos` on the PS2; √(1 + tan²) here).
    pub sec: [F; 2],
}

impl BSphereView {
    /// From a camera given as game-space eye and (forward, left, up) rows.
    pub fn from_camera(eye: [f32; 3], forward: [f32; 3], left: [f32; 3], up: [f32; 3], tan_x: f32, tan_y: f32) -> Self {
        let rows = [0, 1, 2].map(|j| [(-left[j]).to_bits(), (-up[j]).to_bits(), forward[j].to_bits()]);
        let cam = eye.map(|v| ps2v::mul(v.to_bits(), ps2v::K1024));
        let sec = [tan_x, tan_y].map(|t| (1.0 + t * t).sqrt().to_bits());
        BSphereView { rows, cam, tan: [tan_x.to_bits(), tan_y.to_bits()], sec }
    }

    /// `FastBSphereCheck(far, sphere) == −1` (outside): beyond `far` in view depth, entirely behind the eye
    /// (`c.z + r ≤ 0`, as the sign bit of `0 − (r + c.z)` is clear), or outside a side plane
    /// (`tan·c.z − (|c| − r·sec) < 0` in x or y). The 0/1 distinction (intersecting / fully inside, using
    /// 0x16d190/0x16d1b0) is not modelled: its callers here only test for −1. Everything is in integer
    /// units (×1024) with the VU0 operation order of 0x2221f0..0x2222c8.
    pub fn culled(&self, far: f32, sphere: [f32; 4]) -> bool {
        use ps2v::{add, mul, neg, sub};
        let w = ps2v::K1024;
        let v1 = sphere.map(|x| mul(x.to_bits(), w)); // vmulw.xyzw vf1, vf1, vf24
        let d = [0, 1, 2].map(|k| sub(v1[k], self.cam[k])); // vsub.xyz vf2, vf1, vf24
        let (r, nr) = (add(0, v1[3]), sub(0, v1[3])); // vaddw.x / vsubw.y vf3, vf0, vf1
        let c = [0, 1, 2].map(|k| add(add(mul(self.rows[0][k], d[0]), mul(self.rows[1][k], d[1])), mul(self.rows[2][k], d[2])));
        let far_i = mul(far.to_bits(), w); // vmulw.y vf19, vf4, vf24
        let (z_plus, z_minus) = (add(r, c[2]), add(nr, c[2])); // vaddz.xy vf1, vf3, vf2
        let behind = sub(0, z_plus); // vsub.xy vf3, vf19, vf1 (x lane: vf19.x = 0)
        let beyond = sub(far_i, z_minus);
        if neg(beyond) || !neg(behind) { return true; }
        for k in 0..2 {
            let t = mul(self.tan[k], c[2]); // vmulz.xy vf5, vf20, vf2
            let rs = mul(self.sec[k], r); // vmulw.xy vf8, vf22, vf1
            let side = sub(t, sub(c[k] & ps2v::MAX, rs)); // vsub vf7 = vf5 − (|c| − vf8)
            if neg(side) { return true; }
        }
        false
    }

    /// `FastBSphereCheck(far, sphere)` 0x2221f0 with its three results: −1 outside ([`BSphereView::culled`]), 1 fully
    /// inside, 0 intersecting. Fully inside (0x2222a0..0x2222d4, from the disassembly): `r + c.z` below the far plane
    /// 0x16d1b0.x = 0x16cf64 (745472 = 728·1024), `c.z − r` at least the near plane 0x16d1b4 = 0x16cf60 (32), and in x
    /// and y `4·tan·c.z − (|c| + sec·r·k) ≥ 0` with the guard factor 0x16d190 = 0x16d0c8 / 0x16d0c0 = 4 and k =
    /// 0x16d1a8 = (sec(atan 4·tan_x) / sec_x + sec(atan 4·tan_y) / sec_y) / 2 (`UpdateViewContext` 0x219580). Native
    /// `f32` for the inside part [the −1 part is the PS2-exact `culled`].
    pub fn check(&self, far: f32, sphere: [f32; 4]) -> i32 {
        if self.culled(far, sphere) { return -1; }
        let f = |b: F| f32::from_bits(b);
        let v = sphere.map(|x| x * 1024.0);
        let d = [0, 1, 2].map(|k| v[k] - f(self.cam[k]));
        let c = [0, 1, 2].map(|k| f(self.rows[0][k]) * d[0] + f(self.rows[1][k]) * d[1] + f(self.rows[2][k]) * d[2]);
        let r = v[3];
        let (tan, sec) = ([f(self.tan[0]), f(self.tan[1])], [f(self.sec[0]), f(self.sec[1])]);
        let sec4 = tan.map(|t| (1.0 + 16.0 * t * t).sqrt());
        let k = (sec4[0] / sec[0] + sec4[1] / sec[1]) * 0.5;
        let inside_depth = r + c[2] - 745_472.0 < 0.0 && c[2] - r - 32.0 >= 0.0;
        let inside_sides = (0..2).all(|i| tan[i] * c[2] * 4.0 - (c[i].abs() + sec[i] * r * k) >= 0.0);
        if inside_depth && inside_sides { 1 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_takes_lowest_free_and_tracks_hw() {
        let mut p = PartPool::new();
        assert_eq!((p.hint, p.hw, p.count), (0, -1, 0));
        for k in 0..5 { assert_eq!(p.create_part(6), Some(k)); }
        assert_eq!((p.hint, p.hw, p.count), (5, 4, 5));
        p.kill_part(1);
        p.kill_part(3);
        assert_eq!((p.hint, p.hw, p.count), (1, 4, 3));
        assert_eq!(p.recs[3][1], FLAG_DEAD);
        // Refill hole 1: 1 <= hw, so the new hint is the first clear bit from bit 0 of byte (2 >> 3) = 0 → 3.
        assert_eq!(p.create_part(7), Some(1));
        assert_eq!(p.hint, 3);
        assert_eq!(p.create_part(7), Some(3));
        assert_eq!(p.hint, 5);
        // Killing the top record walks hw down past dead slots.
        p.kill_part(4);
        assert_eq!(p.hw, 3);
        for k in [3, 2, 1, 0] { p.kill_part(k); }
        assert_eq!((p.hint, p.hw, p.count), (0, -1, 0));
    }

    #[test]
    fn allocator_scan_crosses_full_bytes_and_reports_full() {
        let mut p = PartPool::new();
        for k in 0..20 { assert_eq!(p.create_part(1), Some(k)); }
        p.kill_part(2);
        assert_eq!(p.create_part(1), Some(2));
        // Bytes 0 and 1 are full, byte 2 has bits 0..3 set: the scan ends at 20.
        assert_eq!(p.hint, 20);
        let mut q = PartPool::new();
        for _ in 0..POOL_RECORDS { assert!(q.create_part(0).is_some()); }
        assert_eq!(q.hint, 0x800);
        assert_eq!(q.create_part(0), None);
        q.kill_part(1000);
        assert_eq!(q.create_part(0), Some(1000));
        assert_eq!(q.hint, 0x800, "no clear bit left: the scan runs to 0x800");
    }

    #[test]
    fn create_clears_only_the_first_half() {
        let mut p = PartPool::new();
        let i = p.create_part(6).unwrap();
        p.recs[i] = [0xab; RECORD_SIZE];
        p.kill_part(i);
        let j = p.create_part(9).unwrap();
        assert_eq!(i, j);
        assert_eq!(p.recs[j][0], 9);
        assert!(p.recs[j][1..0x20].iter().all(|&b| b == 0));
        assert!(p.recs[j][0x20..].iter().all(|&b| b == 0xab), "0x20..0x3f are stale");
    }

    #[test]
    fn update_parts_kills_unported_types_and_counts_them() {
        let mut s = Particles::new(None, Vec::new());
        s.create_part(42);
        s.create_part(63);
        s.update_parts(&mut Rng::new());
        assert_eq!(s.pool.count, 0);
        assert_eq!(s.stats.unported_kills[42], 1);
        assert_eq!(s.stats.unported_kills[63], 1);
        assert!(s.table[25].is_some() && s.table[47].is_some() && s.table[60].is_some());
    }

    #[test]
    fn dec_timer_and_ticks() {
        let mut r = [0u8; RECORD_SIZE];
        rec::set_i16(&mut r, 10, 2);
        assert_eq!(fast_dec_timer(&mut r, 10), 0);
        assert_eq!(fast_dec_timer(&mut r, 10), 2);
        assert_eq!(rec::i16(&r, 10), 0);
        assert_eq!(fast_dec_timer(&mut r, 10), 1);
        rec::set_i16(&mut r, 10, -5);
        assert_eq!(fast_dec_timer(&mut r, 10), 2);
        assert_eq!(rec::i16(&r, 10), 0);
        assert_eq!(TimeBase::NTSC.ticks(150), 150);
        assert_eq!(TimeBase::NTSC.ticks(0), 0);
    }

    #[test]
    fn bsphere_check_depth_and_sides() {
        // Camera at the origin looking along +X (forward X, left Y, up Z).
        let v = BSphereView::from_camera([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.63, 0.63 * 0.775);
        assert!(!v.culled(240.0, [100.0, 0.0, 0.0, 20.0]));
        assert!(!v.culled(240.0, [259.0, 0.0, 0.0, 20.0]), "near side of the sphere within 240");
        assert!(v.culled(240.0, [261.0, 0.0, 0.0, 20.0]));
        assert!(v.culled(240.0, [-30.0, 0.0, 0.0, 20.0]), "behind");
        assert!(!v.culled(240.0, [-10.0, 0.0, 0.0, 20.0]), "straddles the eye");
        // Left of the frustum: y = 0.63·100 + 20·√(1+0.63²) + 1 → outside; −1 → inside.
        let edge = 63.0 + 20.0 * (1.0f32 + 0.63 * 0.63).sqrt();
        assert!(v.culled(240.0, [100.0, edge + 1.0, 0.0, 20.0]));
        assert!(!v.culled(240.0, [100.0, edge - 1.0, 0.0, 20.0]));
    }
}
