//! U94 / U95 (census 2026-09-30 ids U95 580, U101 668): Aridia's sand sharks 580 (level02 `0x2d3e50`: 82 created
//! instances, the only copy) and their nests 668 (level02 `0x2dcb38`: 7). A creature of the shared layer (mode 0x20
//! header: damage record +0x20, flash +0x110, knockback +0x120, suck record +0x60, walker +0x180) that swims *under* the
//! sand of its area path: it roams about home with its fin out (state 2, sand puffs), comes up for a target in range
//! (0x13 / 0x15: the class sound 4), bites (6: a sphere of 0.5 at 0.75 ahead of its nose at key 13), dives again and
//! goes home (0x11 / 0x17 / 7), or lies in wait under a cuboid's sand (3) and springs from a moby's position when
//! Ratchet enters a cuboid (0xc / 0xd: the lob with its spin). One hit kills it (health 1): the death flight (0x18,
//! `SetDeathBits`, a sand splash at the landing, the dust of its five body segments), after which it respawns through
//! a nest of its group (0x1a, hidden under the nest; the nest launches it again: 0x1b). A hit alerts the whole group
//! (the alert `randf(240, 300)` ticks: range + 5, the surfacing). The Suck Cannon takes it ([`react::ARIDIA_580`]);
//! the Morph-o-Ray's keep (+0x2e = 2) respawns it.
//!
//! The nests 668: health 3, a knockback per hit (3), each `+0xd4` ticks (plus 15 ticks per death of Ratchet to its
//! mission, 20 at most) one of the group's sharks waiting in 0x1a is launched from 1.5 above it (at key 12 of sequence
//! 1) toward its home; destroyed (4): the beam explosion, four pieces (1764 ×2, 1765 ×2), `SetDeathBits`.
//!
//! **Pvars 580** (0x2e0): +0x20 damage record (health, +0x24 meter, +0x29 column, +0x2e the morph keep), +0x38 the
//! lure, +0x40 the jump velocity (+0x48 its z), +0x58 byte 10, +0x5c byte 1, +0x60 the suck record (+0xc8 its state),
//! +0x110 flash, +0x120 knockback (+0x15d, +0x16c air speed, +0x170 / +0x174 keys), +0x180 walker J (+0x198 its
//! vertical speed, +0x1a4 top speed), +0x1d0 target record (+0x210 moby + 1, +0x214 kind), +0x220 home, +0x230 the
//! spawn point, +0x240 turn velocity, +0x244 side offset, +0x248 side (degrees), +0x24c range, +0x250 range now, +0x254
//! speed, +0x258 a moby to wait for (state ≥ 5), +0x25c s16 the wait timer, +0x25e s16 the alert, +0x260 the side
//! timer, +0x26c / +0x270 the ambush cuboids, +0x274 a cuboid it keeps the target in, +0x278 the lair cuboid, +0x27c
//! the nest it respawns in, +0x280 the state (copy), +0x284 the side sign, +0x288 the counter moby (+0x14c), +0x28c
//! the ambush moby, +0x290 the area path, +0x294 the wall distance, +0x298 out of the area, +0x29c the splash made,
//! +0x2a0 the big-head cheat's.
//!
//! **Pvars 668** (0x100): +0x20 damage record, +0x60 flash, +0x70 knockback, +0xd4 the launch period, +0xd8 its timer,
//! +0xe0 the counter moby (+0x150 nests, +0x14c sharks).
//!
//! The level02 `$gp` words (gp = 0x166c00; gp−0x5300 = 0x161900): −0x5300 10 (byte), −0x52f8 / −0x52f4 7 / 15 (the
//! death flight's up / out ×dt), −0x52ec / −0x52e8 20 / 23 (its gravity / drag ×dt²), −0x52e4 1 (the fin's height),
//! −0x52e0.. (the dive puffs: 1 / 10 up, −0.1 / −1 down, 0 / 4 out, sizes 0.28 / 1, phases 1–10 / 15–25 / 60–90,
//! colours 0x60306058 / 0x20306058), −0x529c.. (the fin puffs: 0.5 / 0.5, 0.1 / −1, 0.1 / 0.1, sizes 0.14 / 0.42,
//! phases 20–30 / 30–60 / 20–30, colours 0x40306058 / 0x10306058), −0x5258 the freeze word (0; nothing writes it),
//! −0x5250 the Suck Cannon sequences (0, 0, 0, 10, 2, 11, 11, 7, 2), −0x5240 the segment origin (−3.25, 0, 0.2),
//! −0x5230 2 (the dust size), −0x522c 0.005, −0x5224 0.1, −0x5220 0 (the dust's timer), −0x521c / −0x5218 the nest's
//! launch puff colours (0x40306058 / 0x306058), −0x5214.. the segment counts (10, 10, 8, 8); level02 0x1d3230 the ten
//! segment points (xyz, size).
//!
//! ## Coverage 580
//!
//! **The tick** `0x2d5c88` (before the states):
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | +0x280 = state; state 0 → done | | [`pre`] |
//! | +0x2e = 2 (morphed, kept): 1; the counter moby's +0x14c −1; a nest (`0x2d5658`) → 0x1a at home 10 below without collision, else `DeleteMoby` | the Morph-o-Ray's respawn | [`pre`] |
//! | no lure: `FastDecTimer_s16(+0x25c)` counting and Ratchet within 10 (xy) → the alert; else lure 0; a lure → alert = `ticks(randf(180, 240))` | | [`pre`] |
//! | `FastDecTimer_s16(+0x25e)`: counting → range + 5 | | [`pre`] |
//! | `MobyGetHitMessage(m, 0x330000, 0)`, `0x25cae0(…, col 4)` | every weapon's hit | [`pre`] (`World::get_hit`, `damage::resolve`) |
//! | a hit out of 0x18: `0x2d5728` (the group's 580s: alert = max(alert, `randf(240, 300)` scaled)), +0x29c = 0, health −= damage | the pack's alert | [`pre`] ([`alert_group`]) |
//! | no health: health −= damage again; K: +0x15d 0, radius 0x200, gravity 20·dt², drag 23·dt², 15·dt / 7·dt, flags 9, air speed 2·dt, height 0.5; z at least `GroundHeight(0.5, pos + 2 z, 0)`; the counter moby's +0x14c −1; flags \| 0x20; untargetable; aim; `0x25eb80(angle, m, K, 6, 1, 0)`; height 0.85, keys 7 / 14; state 0x18; no collision; flash 0x78, `0x25fa80` | the death flight | [`pre`] (`knock::start`, `flash::start`) |
//! | +0xa4 = 0xff; `0x25fb60` flash update | | [`pre`] |
//! | `0x261f00(range, m, +0x1d0, 0, 0, area path)`: found and (farther than range from home, or more than 3 apart in z) → kind 2; no moby → Ratchet | the target search | [`pre`] (`target::acquire_in`) |
//!
//! **The update** `0x2d3e50`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | the freeze word gp−0x5258 ≠ 0 → nothing | (never written) | [`update`] |
//! | `REG_RCNT0` writes | a profiling timer | n/a |
//! | `0x264dd0(2.1, m, 0, +0x2a0)` (= `0x278720`) | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | drawn within 28 of the camera: `0x25c788` shadow probe, +0x7f = 0x16 | | [`update`] (`shadows::probe_down`) |
//! | 0: no area path (−1, or no points): `STUB_printf`, `DeleteMoby`; the counter moby's +0x14c +1; home and +0x230 = pos; meter 1, health 1, +0x5c 1, keep 1, column 1, alert 0, +0x58 10; `SeedJumpPattern(J)`, J: 0x200, 0.5, 2, 2, top speed +0x254·dt; `rand() & 1` → mirror; +0xd0 the sequence table; no ambush cuboid: a lair (+0x278) → J probe 0.2, 3 (blend 2); none: a moby to wait for (+0x258) → 0x1d hidden (mode & ~0x1000 \| 0x41, no collision), else 1 (blend 2); an ambush cuboid: no ambush moby → `DeleteMoby`, else 0xc, 10 up, hidden (mode & ~0x1000 \| 1), no collision (blend 0) | init | [`init`] |
//! | 1 → 0x12 (blend 0) | | [`update`] |
//! | 2: fin up (z + 1); `SpringTurn2(atan(home − pos) + side, 0.05, 0.3, 0.04)`; xy += dt ahead; side timer out → `ticks(60)`, side `randf(−s, s)`; drawn: `randi(255) & 1` → a fin puff (`0x2d5a68`); a lost target: the alert and `randi(19) == 0` → 0x15 (class sound 4, blend 0), +0x274 = −1; a kept cuboid: the target in it with no alert → stays; else `randi(9) == 0` forgets it; none: `randi(4) == 0` → the side (±, or ·+0x284), side timer `ticks(randf(30, 90))`, speed 4·dt, 0x11 (blend 0xc) | roam under the sand | [`roam`] |
//! | 3: a target in the lair cuboid → 5 (blend 0) | lie in wait | [`update`] |
//! | 5 / 0x16: (0x16: fin up, puffs) `SpringTurn2(atan(t − pos) + side, 0.05, 0.3, 0.2)`, `0x25b238(1, m, J, pos + 2·dir)` (= `0x26d9a8`); `0x2d63c0` (outside the area → 0x12, blend 0, +0x298 = 1); lost: no lair → alert ? 0xe : 0x12; a lair → 7; target (in the lair when there is one): 0x16 within 12 → 0x13 (class sound 4, blend 0); 5 within 1.5 → 6 (blend 1); the target out of the lair → 7 | the chase (5 on the surface, 0x16 under) | [`chase`] (`walker::step`) |
//! | 6: turn to the target (0.05, 0.3, 0.2); not blending, key 13: `0x25bf98(0.5, 1, 1, m, pos + 0.75·row 0 + 0.5 z, 1, 0, 1, 0)` (= `0x26e830`); wrapped and farther than 1.5 → 5 (blend 0); gravity 9.8·dt² onto `GroundHeight(0.5)` | the bite | [`bite`] (`attack::sphere_hit`) |
//! | 7 / 0x17: (0x17: fin up, puffs) turn home (0.02, 0.3, 0.1), step; home within 0.5: +0x298 = 0; 7: no lair → 0x12 (speed 1.5·dt, blend 0), a lair → 3 (blend 2); 0x17 → 2 (speed 1.5·dt, blend 0xc); out of the area: pos += 0.5·dt along row 0, done; a target: in the lair → 5 (speed); no lair and within range of home → 5 / 0x16 (speed, blend 0); lost: the alert and `randi(19) == 0` → 0x15 | go home | [`home`] |
//! | 8: `0x25ecc0` landed → 0xb (blend 4 at frame 8); below z 0 → the dust (`0x2d3dd0`), `DeleteMoby` | (entered by no code here) | [`update`] |
//! | 9: `0x2e7660(m, K)` (= `0x305260`) done → 1, record state 0 | the Suck Cannon's carried update | [`update`] (`react::carried`) |
//! | 0xc: Ratchet in cuboid +0x26c or +0x270: from the ambush moby's position, vz `randf(6, 10)·dt`, xy speed `dist / ((√(2·10.8·dt²·(z − home z)) + 2·vz) / 10.8·dt²)` toward home; shown, targetable, the class's collision; blend 0xb; 0xd | spring the ambush | [`ambush`] |
//! | 0xd: roll −= 4π·dt; pos += v; vz −= 10.8·dt²; above home z − 1: below home z + 0.6 → the dive puffs (`0x2d57f8`, 6); else collision, targetable, 0x17, roll 0, z = home z − 1 (blend 0xc) | the flight | [`flight`] |
//! | 0xe: turn to the target + side, step, `0x2d63c0`; `0x263710` (= `ClampToPath`) a wall within +0x294 → near; alert counting: near → 0xf (blend 5); within 1.5 → 6 (blend 1); a target → 5 (blend 0, 15); alert out → 0x12 | alerted, surfaced | [`alerted`] (`World::clamp_to_path_hit`) |
//! | 0xf: `0x25e428` turn (2π·dt² / 2π·dt); alert: within 1.5 → 6; a target → 5; alert out → 0x12 | at the wall | [`update`] (`turn::turn_toward_pvar`) |
//! | 0x11 / 0x12: fin up; the ground (at least home z); `Approach(4·dt)`; reached: blend 0xc, 0x11 → 0x16, 0x12 → 0x17; fin down; after tick 10: the dive puffs (6) at the ground point | dive | [`dive`] |
//! | 0x13 / 0x14 / 0x15: fin up; `Approach(ground + 1, 4·dt)` reached: 0x13 → 5, 0x14 → 7, 0x15 → wall distance `randf(2, 3.5)`, speed 4·dt, 0xe (blend 0); fin down; the dive puffs (6) | surface | [`surface`] |
//! | 0x18: `0x25ecc0`: landed with no splash → the splash (`0x2d57f8`, 0x1b puffs); & 0x140: `SetDeathBits(m, 0, −1)`, a nest → the dust, 0x1a (anim speed 0, at home 10 below, hidden, no collision), else the dust, `DeleteMoby`; below z 0 → the dust, `DeleteMoby` | the death | [`update`] ([`dust`], `crate_::set_death_bits`) |
//! | 0x19: fade out (alpha −8 a tick); then a nest → the dust, alpha 0x80, 0x1a (at home 10 below), else the dust, `DeleteMoby` | (entered by no code here) | [`update`] |
//! | 0x1a: no nest (`0x2d5658`: class 668 of the group below state 0x7f, a random one) → `DeleteMoby` | wait in the nest | [`update`] |
//! | 0x1b: a launch puff (`rand_vec(0, 0.2)` ·5·dt, `PartType04Spawn(pos, v, 0x40306058, 0x306058, ticks(rr(20, 40)), 100, rr(120, 150), 0)`), then 0xd's flight | launched by a nest | [`update`] (`fx::part04`) |
//! | 0x1d: the moby +0x258 at state 5 or more and Ratchet not in state 0x72 → shown, targetable, collision, +0x258 = −1, +0x25c = `ticks(5400)`, 0, z − 1 | wait for a moby | [`update`] |
//!
//! **Helpers:** the dust `0x2d3dd0` / `0x2d3c98` / `0x2d3a50` (five segments of the body, 10 / 10 / 8 / 8 / 8 type-23
//! puffs along each: size `(b − a)/n + a·210000`, growth 1 → `randf(1, 1.02)`, spin `rr(−2, 2)`, colour grey
//! `rr(64, 127)` with alpha `rr(64, 112)`, drift `(randf_sym(0, 0.0025) ×2, randf(0.0005, 0.005))`; taken: `randi(2)` →
//! normal blend, timer 0, phase 2 with the alpha) [`dust`]; the dive puffs `0x2d57f8` (type 2; halved over 0x400
//! particles, quartered over 0x5dc) [`dive_puffs`]; the fin puffs `0x2d5a68` (type 2; none over 0x6a4 particles)
//! [`fin_puffs`]; the nest search `0x2d5658` [`find_nest`]; the pack's alert `0x2d5728` [`alert_group`]; the area test
//! `0x2d63c0` [`area_check`].
//!
//! **The reaction table** (level02 0x1fb6dc: 0x2d61c0, 0x2d6210, 0x2d6290, 0x2d62e0, 0x2d6310, 0x2d6340):
//! [`react::ARIDIA_580`]: held 9, refused → 1; the swallow takes one off the counter moby's +0x14c; the delete slot
//! respawns it through a nest ([`respawn`]).
//!
//! ## Coverage 668 (`0x2dcb38`, its tick `0x2dcfe0`, the take `0x2dd2a8`, the launch `0x2d6118`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | tick: keep 2 → 1, the counter moby's +0x150 −1, `DeleteMoby` | the Morph-o-Ray | [`nest_pre`] |
//! | a hit (out5 ≠ 1) out of 4: out5 3 → health 0, else −= damage; K flags 4; health left: 4·dt / 3·dt, aim, `0x25eb80(…, 3, 1, 0)`, flash 0x78, damage ≥ 1 → 3 (blend 2); none: untargetable, flash 0xfa, 4, 11·dt / 3·dt, start (3), keys 9 / 19, blend 3, `SetDeathBits(m, 0, −1)`; `0x25fa80` | | [`nest_pre`] |
//! | +0xa4 = 0xff; `0x25fb60` flash update; `FastDecTimer_s16(+0xbc)` | | [`nest_pre`] |
//! | state ≥ 0x80 → done; drawn within 28: shadow, +0x7f = 0x16 | | [`nest_update`] |
//! | 0: +0xb4 −= the spawner's collected bolts (`0x14d790[+0xb1]`), at least 1; 1; health 3, meter 3, keep 1; timer `ticks(+0xd4)`; the counter moby's +0x150 +1 | init | [`nest_update`] |
//! | 1: timer out: take a waiting shark (`0x2dd2a8`: class 580 of the group in 0x1a; its +0xb8 = the nest, the bolts split) → 2 (blend 1), else the timer again | | [`nest_update`] ([`take_shark`]) |
//! | 2: wrapped → 1 (blend 0); at key 12: timer `ticks(+0xd4 + ticks(15·min(deaths, 20)))` (0x14ee90[+0xb0]); take a shark; the counter moby's +0x14c +1; launch it (`0x2d6118`: 0x1b, shown, blend 0xb, from 1.5 above the nest, 2·dt toward its home, 6·dt up, facing it) | spawn | [`nest_update`] ([`launch`]) |
//! | 3: `0x25ecc0` knockback; wrapped → 1 (blend 0) | | [`nest_update`] |
//! | 4: & 0x40: the counter moby's +0x150 −1; `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, m, +0x40, pos, 5, 2, 4, −1, 1, 1)`; `BreakFxB` 1764, 1764, 1765, 1765 (zero vectors); `DeleteMoby` | destroyed | [`nest_update`] (`fx::beam_explosion`, `fx::break_piece`) |
//! | the table (level02 0x1fb6ac, shared with 612: default wrappers) | no Suck Cannon reaction | n/a |
//!
//! Native `f32`; the rand draws at the game's points.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, ground, knock, react, target, turn, walker};
use crate::moby_update::services::World;
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 2;
/// The sharks' update in the level02 class table.
pub const UPDATE_FN: u32 = 0x2d_3e50;
pub const CLASSES: [i16; 1] = [580];
/// The nests' update.
pub const NEST_FN: u32 = 0x2d_cb38;
pub const NEST_CLASSES: [i16; 1] = [668];

/// Pvar offsets of 580 (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const KEEP: usize = 0x2e;
    pub const LURE: usize = 0x38;
    pub const VEL: usize = 0x40;
    pub const VZ: usize = 0x48;
    pub const SUCK: usize = 0x60;
    pub const FLASH: usize = 0x110;
    pub const K: usize = 0x120;
    pub const J: usize = 0x180;
    pub const JVZ: usize = 0x198;
    pub const TOP: usize = 0x1a4;
    pub const TGT: usize = 0x1d0;
    pub const TGT_MOBY: usize = 0x210;
    pub const TGT_KIND: usize = 0x214;
    pub const HOME: usize = 0x220;
    pub const SPAWN: usize = 0x230;
    pub const TURN_V: usize = 0x240;
    pub const SIDE: usize = 0x244;
    pub const SIDE_DEG: usize = 0x248;
    pub const RANGE: usize = 0x24c;
    pub const RANGE_NOW: usize = 0x250;
    pub const SPEED: usize = 0x254;
    pub const WAIT_MOBY: usize = 0x258;
    pub const WAIT_T: usize = 0x25c;
    pub const ALERT: usize = 0x25e;
    pub const SIDE_T: usize = 0x260;
    pub const AMBUSH_A: usize = 0x26c;
    pub const AMBUSH_B: usize = 0x270;
    pub const KEEP_CUBOID: usize = 0x274;
    pub const LAIR: usize = 0x278;
    pub const NEST: usize = 0x27c;
    pub const STATE_COPY: usize = 0x280;
    pub const SIDE_SIGN: usize = 0x284;
    pub const COUNTER: usize = 0x288;
    pub const AMBUSH_MOBY: usize = 0x28c;
    pub const AREA: usize = 0x290;
    pub const WALL_D: usize = 0x294;
    pub const OUT: usize = 0x298;
    pub const SPLASHED: usize = 0x29c;
    pub const SIZE: usize = 0x2a0;
}

/// Pvar offsets of 668.
pub mod nv {
    pub const D: usize = 0x20;
    pub const KEEP: usize = 0x2e;
    pub const FLASH: usize = 0x60;
    pub const K: usize = 0x70;
    pub const PERIOD: usize = 0xd4;
    pub const TIMER: usize = 0xd8;
    pub const COUNTER: usize = 0xe0;
    pub const SIZE: usize = 0xe4;
}

/// The level02 words (module doc).
pub mod k {
    /// gp−0x5258: the freeze word.
    pub const FREEZE: u32 = 0x16_19a8;
    pub const FIN: f32 = 1.0;
    pub const DEATH_UP: f32 = 7.0;
    pub const DEATH_OUT: f32 = 15.0;
    pub const DEATH_G: f32 = 20.0;
    pub const DEATH_DRAG: f32 = 23.0;
    pub const JUMP_G: f32 = 10.8;
    /// The ten body points (level02 0x1d3230: xyz, size) and the origin they are taken from (gp−0x5240).
    pub const SEGMENTS: [[f32; 4]; 10] = [
        [-2.882, -0.021, 0.264, 0.173],
        [-3.5, 0.006, 0.264, 0.361],
        [-3.146, 0.006, 0.363, 0.146],
        [-3.255, 0.006, 0.625, 0.215],
        [-3.143, 0.179, 0.522, 0.109],
        [-3.067, 0.238, 0.732, 0.174],
        [-3.143, -0.2, 0.522, 0.109],
        [-3.067, -0.208, 0.732, 0.174],
        [-2.62, -0.001, -0.22, 0.032],
        [-3.1, -0.001, 0.132, 0.177],
    ];
    pub const SEG_ORIGIN: [f32; 3] = [-3.25, 0.0, 0.2];
}

/// The exact words of the segment table (level02 0x1d3230).
const SEG_WORDS: [[u32; 4]; 10] = [
    [0xc038_72b0, 0xbcac_0831, 0x3e87_2b02, 0x3e31_26e9],
    [0xc060_0000, 0x3bc4_9ba6, 0x3e87_2b02, 0x3eb8_d4fe],
    [0xc049_5810, 0x3bc4_9ba6, 0x3eb9_db23, 0x3e15_8106],
    [0xc050_51ec, 0x3bc4_9ba6, 0x3f20_0000, 0x3e5c_28f6],
    [0xc049_26e9, 0x3e37_4bc7, 0x3f05_a1cb, 0x3ddf_3b64],
    [0xc044_49ba, 0x3e73_b646, 0x3f3b_645a, 0x3e32_2d0e],
    [0xc049_26e9, 0xbe4c_cccd, 0x3f05_a1cb, 0x3ddf_3b64],
    [0xc044_49ba, 0xbe54_fdf4, 0x3f3b_645a, 0x3e32_2d0e],
    [0xc027_ae14, 0xba83_126f, 0xbe61_47ae, 0x3d03_126f],
    [0xc046_6666, 0xba83_126f, 0x3e07_2b02, 0x3e35_3f7d],
];

/// The 580 states (module doc).
pub mod st {
    pub const INIT: u8 = 0;
    pub const START: u8 = 1;
    pub const ROAM: u8 = 2;
    pub const LAIR: u8 = 3;
    pub const CHASE: u8 = 5;
    pub const BITE: u8 = 6;
    pub const HOME: u8 = 7;
    pub const KNOCKED: u8 = 8;
    pub const HELD: u8 = 9;
    pub const AMBUSH: u8 = 0xc;
    pub const FLIGHT: u8 = 0xd;
    pub const ALERTED: u8 = 0xe;
    pub const WALL: u8 = 0xf;
    pub const DIVE_CHASE: u8 = 0x11;
    pub const DIVE_HOME: u8 = 0x12;
    pub const UP_CHASE: u8 = 0x13;
    pub const UP_HOME: u8 = 0x14;
    pub const UP_ALERT: u8 = 0x15;
    pub const UNDER_CHASE: u8 = 0x16;
    pub const UNDER_HOME: u8 = 0x17;
    pub const DYING: u8 = 0x18;
    pub const FADE: u8 = 0x19;
    pub const NESTED: u8 = 0x1a;
    pub const LAUNCHED: u8 = 0x1b;
    pub const WAIT: u8 = 0x1d;
}

/// The 668 states.
pub mod nst {
    pub const INIT: u8 = 0;
    pub const IDLE: u8 = 1;
    pub const SPAWN: u8 = 2;
    pub const KNOCKED: u8 = 3;
    pub const DYING: u8 = 4;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn blend(w: &mut World, id: MobyId, seq: u8, n: i32) {
    let t = w.ticks(n);
    c::blend_to(w, id, seq, 0, t);
}
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn kind(w: &World, id: MobyId) -> i32 { c::pi32(w, id, pv::TGT_KIND) }
fn alert(w: &World, id: MobyId) -> i16 { c::pi16(w, id, pv::ALERT) }
fn moby_at(w: &World, i: i32) -> Option<MobyId> { usize::try_from(i).ok().filter(|&m| m < w.table.mobys.len()) }
fn path_ok(w: &World, id: MobyId, o: usize) -> Option<usize> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&p| p < w.svc.splines.len()) }
fn hero_p(w: &World) -> c::V { crate::moby_update::classes::units::hero_pos(w) }
/// `PointInCuboid(p, index)` of a pvar cuboid index.
fn in_cuboid(w: &World, p: c::V, index: i32) -> bool { w.in_cuboid([p[0], p[1], p[2]], index) }

/// A counter moby's pvar word +`o` += `d` (the game's `*(moby[i].pvars + o) += d`).
fn count(w: &mut World, moby: i32, o: usize, d: i32) {
    let Some(m) = moby_at(w, moby) else { return };
    if w.m(m).pvars.len() < o + 4 { return; }
    let v = c::pi32(w, m, o).wrapping_add(d);
    c::set_pi32(w, m, o, v);
}

/// The group's members (`0x1abe40[group]`, the last entry has bit 15).
fn group(w: &World, id: MobyId) -> Vec<MobyId> {
    let g = w.m(id).group;
    if g < 0 { return Vec::new(); }
    let Some(Some(list)) = w.svc.groups.lists.get(g as usize) else { return Vec::new() };
    let mut out = Vec::new();
    for &e in list {
        out.push((e & 0x7fff) as MobyId);
        if e & 0x8000 != 0 { break; }
    }
    out.retain(|&m| m < w.table.mobys.len());
    out
}

/// `0x2d5658(m)`: a nest of the group (class 668 below state 0x7f; each later one replaces the pick when
/// `randi(255) & 1`).
pub fn find_nest(w: &mut World, id: MobyId) -> Option<MobyId> {
    let mut pick = None;
    for m in group(w, id) {
        if w.m(m).o_class == NEST_CLASSES[0] && w.m(m).state < 0x7f && (pick.is_none() || w.rng.randi(0xff) & 1 != 0) { pick = Some(m); }
    }
    pick
}

/// `0x2d5728(m)`: every 580 of the group: alert = `randf(240, 300)` (scaled) when that is longer.
fn alert_group(w: &mut World, id: MobyId) {
    for m in group(w, id) {
        if w.m(m).o_class != CLASSES[0] || w.m(m).pvars.len() < pv::SIZE { continue; }
        let r = w.rng.randf(240.0, 300.0);
        let f = w.svc.timing.scale(crate::ps2v::Pf::f(r)).to_f32();
        if (c::pi16(w, m, pv::ALERT) as f32) < f { c::set_pi16(w, m, pv::ALERT, f as i32 as i16); }
    }
}

/// Into the nest: 0x1a at home, 10 below, without collision (the respawns' shared tail).
fn to_nest(w: &mut World, id: MobyId) {
    set_state(w, id, st::NESTED);
    let h = c::pv4(w, id, pv::HOME);
    let m = w.mm(id);
    m.position = h;
    m.has_collision = false;
    m.position[2] -= 10.0;
}

/// `0x2d6340` (slot +0x14 of the reaction table): respawn through a nest, else the dust and `DeleteMoby`.
pub fn respawn(w: &mut World, id: MobyId) {
    let n = find_nest(w, id);
    c::set_pi32(w, id, pv::NEST, n.map_or(0, |m| m as i32 + 1));
    if n.is_none() {
        dust(w, id);
        w.delete_moby(id);
        return;
    }
    w.mm(id).anim.speed = 0.0;
    to_nest(w, id);
}

/// Slot +0x04's extra (`0x2d6210`): a swallowed shark leaves its counter.
pub fn swallowed(w: &mut World, id: MobyId) {
    let cm = c::pi32(w, id, pv::COUNTER);
    count(w, cm, 0x14c, -1);
}

/// `0x2d63c0(m)`: outside its area path → 0x12 (blend 0, `ticks(20)`), +0x298 = 1.
fn area_check(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    let inside = path_ok(w, id, pv::AREA).is_some_and(|a| c::region::point_in_polygon(w, a, p));
    if !inside {
        set_state(w, id, st::DIVE_HOME);
        blend(w, id, 0, 20);
        c::set_pi32(w, id, pv::OUT, 1);
    }
}

/// The live particle count (0x160178, `CreatePart` / `KillPart`).
fn live_parts(w: &World) -> i32 { w.particles.as_deref().map_or(0, |p| p.pool.count) }

/// `0x2d57f8(pos, n)`: the dive puffs (type 2): `n` halved over 0x400 live particles, quartered over 0x5dc.
pub fn dive_puffs(w: &mut World, p: c::V, n: i32) {
    let live = live_parts(w);
    let n = if live > 0x5dc { n / 4 } else if live > 0x400 { n / 2 } else { n };
    for _ in 0..n.max(0) {
        let a = w.rng.rand_angle();
        let mut q = p;
        q[2] += w.rng.randf(-0.25, -0.75);
        q[0] += w.rng.randf(-0.25, 0.25);
        q[1] += w.rng.randf(-0.25, 0.25);
        let r1 = w.rng.randf(0.0, 4.0);
        let vx = a.cos() * r1 * c::DT;
        let r2 = w.rng.randf(0.0, 4.0);
        let vy = a.sin() * r2 * c::DT;
        let up = w.rng.randf(1.0, 10.0) * c::DT;
        let down = w.rng.randf(-0.1, -1.0) * c::DT;
        let t0 = w.rng.randf(1.0, 10.0) as i32;
        let t1 = w.rng.randf(15.0, 25.0) as i32;
        let t2 = w.rng.randf(60.0, 90.0) as i32;
        let s = crate::particles::type02::Spawn { pos: q, v1: [0.0, 0.0, up, 0.28], v2: [vx, vy, down, 1.0], c1: 0x6030_6058, c2: 0x2030_6058, t: [t0, t1, t2], def: -1 };
        fx::part02(w, &s);
    }
}

/// `0x2d5a68(pos)`: one fin puff (type 2), none over 0x6a4 live particles.
pub fn fin_puffs(w: &mut World, p: c::V) {
    if 0x6a5 <= live_parts(w) { return; }
    let a = w.rng.rand_angle();
    let mut q = p;
    q[2] += w.rng.randf(0.125, 0.0);
    q[0] += w.rng.randf(-0.125, 0.125);
    q[1] += w.rng.randf(-0.125, 0.125);
    let r1 = w.rng.randf(0.1, 0.1);
    let vx = a.cos() * r1 * c::DT;
    let r2 = w.rng.randf(0.1, 0.1);
    let vy = a.sin() * r2 * c::DT;
    let up = w.rng.randf(0.5, 0.5) * c::DT;
    let down = w.rng.randf(0.1, -1.0) * c::DT;
    let t0 = w.rng.randf(20.0, 30.0) as i32;
    let t1 = w.rng.randf(30.0, 60.0) as i32;
    let t2 = w.rng.randf(20.0, 30.0) as i32;
    let s = crate::particles::type02::Spawn { pos: q, v1: [0.0, 0.0, up, 0.14], v2: [vx, vy, down, 0.42], c1: 0x4030_6058, c2: 0x1030_6058, t: [t0, t1, t2], def: -1 };
    fx::part02(w, &s);
}

/// `0x2d3c98` / `0x2d3a50` for body points `j0`, `j1`: `n` type-23 dust puffs along the segment.
fn segment(w: &mut World, id: MobyId, j0: usize, j1: usize, n: i32) {
    let m = w.m(id);
    let (rows, pos) = (m.rows, m.position);
    let at = |j: usize| -> c::V {
        let s = SEG_WORDS[j].map(f32::from_bits);
        let v = [s[0] - k::SEG_ORIGIN[0], s[1] - k::SEG_ORIGIN[1], s[2] - k::SEG_ORIGIN[2]];
        let mut o = [0.0f32; 4];
        for i in 0..3 { o[i] = v[0] * rows[0][i] + v[1] * rows[1][i] + v[2] * rows[2][i] + pos[i]; }
        o[3] = pos[3];
        o
    };
    let (p0, p1) = (at(j0), at(j1));
    let a = f32::from_bits(SEG_WORDS[j0][3]) * 2.0;
    let b = f32::from_bits(SEG_WORDS[j1][3]) * 2.0;
    let nf = n as f32;
    let step = c::scale(c::sub(p1, p0), 1.0 / nf);
    let size = (b - a) / nf + a * 210_000.0;
    for i in 0..n {
        let pt = c::add(c::scale(step, i as f32), p0);
        let alpha = w.rng.rand_range(0x40, 0x70);
        let grey = w.rng.rand_range(0x40, 0x7f) as u32;
        let hi = w.rng.randf(1.0, f32::from_bits(0x3f82_8f5c));
        let spin = w.rng.rand_range(-2, 2);
        let vx = w.rng.randf_sym(0.0, f32::from_bits(0x3b23_d70a));
        let vy = w.rng.randf_sym(0.0, f32::from_bits(0x3b23_d70a));
        let vz = w.rng.randf(0.005 * 0.1, 0.005);
        let rgba = (alpha as u32) << 24 | grey | grey << 8 | grey << 16;
        puff23(w, 0.1, hi, size, pt, spin, [vx, vy, vz, 1.0], rgba, alpha as u8);
    }
}

/// One dust puff (`PartType23Spawn(0.1, 1, hi, size, p, spin, v, rgba)`), then on a taken record `randi(2)` →
/// normal blend, timer 0 (gp−0x5220), phase 2 with the alpha.
#[allow(clippy::too_many_arguments)]
fn puff23(w: &mut World, jitter: f32, hi: f32, size: f32, p: c::V, spin: i32, v: c::V, rgba: u32, alpha: u8) {
    let Some(sys) = w.particles.as_deref_mut() else { fx::part_unported(w, 23); return };
    *w.svc.fx.part_spawns.entry(23).or_default() += 1;
    let Some(i) = crate::particles::type23::spawn(sys, w.rng, jitter, 1.0, hi, size, p, spin, v, rgba) else {
        w.svc.fx.part_failed += 1;
        return;
    };
    let normal = w.rng.randi(2) != 0;
    let r = &mut w.particles.as_deref_mut().unwrap().pool.recs[i];
    use crate::particles::rec;
    if normal { r[3] = 0x44; }
    rec::set_i16(r, 0xa, 0);
    rec::set_u32(r, 0x24, 2);
    r[0x2a] = alpha;
    r[0x2b] = 0;
}

/// `0x2d3dd0(m)`: the dust of the five body segments.
pub fn dust(w: &mut World, id: MobyId) {
    for (j0, j1, n) in [(0, 1, 10), (2, 3, 10), (4, 5, 8), (6, 7, 8), (8, 9, 8)] { segment(w, id, j0, j1, n); }
}

fn store_target(w: &mut World, id: MobyId, t: &target::Target) {
    c::set_pv4(w, id, pv::TGT, t.pos);
    c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
    c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
    c::set_pv4(w, id, pv::TGT + 0x30, t.body);
    c::set_pi32(w, id, pv::TGT_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
}

/// `0x2d5c88`: the tick before the states (module doc). False: deleted.
fn pre(w: &mut World, id: MobyId) -> bool {
    let s = state(w, id);
    c::set_pi32(w, id, pv::STATE_COPY, s as i32);
    if s == st::INIT { return true; }
    if c::pu8(w, id, pv::KEEP) == 2 {
        c::set_pu8(w, id, pv::KEEP, 1);
        let cm = c::pi32(w, id, pv::COUNTER);
        count(w, cm, 0x14c, -1);
        let n = find_nest(w, id);
        c::set_pi32(w, id, pv::NEST, n.map_or(0, |m| m as i32 + 1));
        if n.is_none() {
            w.delete_moby(id);
            return false;
        }
        to_nest(w, id);
    }
    let mut do_alert = c::pi32(w, id, pv::LURE) != 0;
    if !do_alert {
        if c::dec_timer_pvar_s16(w, id, pv::WAIT_T) == 0 && c::dist2(c::pos(w, id), hero_p(w)) < 10.0 { do_alert = true; } else { c::set_pi32(w, id, pv::LURE, 0); }
    }
    if do_alert {
        let r = w.rng.randf(180.0, 240.0);
        let t = w.svc.timing.scale(crate::ps2v::Pf::f(r)).to_i32();
        c::set_pi16(w, id, pv::ALERT, t as i16);
        c::set_pi32(w, id, pv::LURE, 0);
    }
    let range = if c::dec_timer_pvar_s16(w, id, pv::ALERT) == 0 { c::pf(w, id, pv::RANGE) + 5.0 } else { c::pf(w, id, pv::RANGE) };
    c::set_pf(w, id, pv::RANGE_NOW, range);
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && state(w, id) != st::DYING {
        alert_group(w, id);
        c::set_pi32(w, id, pv::SPLASHED, 0);
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        if hp <= 0.0 {
            let kr = pv::K;
            c::set_pu8(w, id, kr + 0x3d, 0);
            c::set_pf(w, id, pv::D, hp - res.damage);
            c::set_pi32(w, id, kr + knock::k::RADIUS, 0x200);
            c::set_pf(w, id, kr + knock::k::GRAVITY, k::DEATH_G * c::DT2);
            c::set_pf(w, id, kr + knock::k::DRAG, k::DEATH_DRAG * c::DT2);
            c::set_pf(w, id, kr + knock::k::SPEED, k::DEATH_OUT * c::DT);
            c::set_pf(w, id, kr + knock::k::UP, k::DEATH_UP * c::DT);
            c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
            c::set_pf(w, id, kr + knock::k::AIR_SPEED, c::DT + c::DT);
            c::set_pf(w, id, kr + knock::k::ZOFF, 0.5);
            let mut p = c::pos(w, id);
            let g = ground::ground(w, [p[0], p[1], p[2] + 2.0, p[3]], 0.5, 0).z;
            if p[2] < g { p[2] = g; c::set_pos(w, id, p); }
            let cm = c::pi32(w, id, pv::COUNTER);
            if cm >= 0 {
                count(w, cm, 0x14c, -1);
                c::set_pu8(w, id, kr + 0x3d, 0);
            }
            let f = c::pi32(w, id, kr + knock::k::FLAGS) | 0x20;
            c::set_pi32(w, id, kr + knock::k::FLAGS, f);
            w.mm(id).mode &= !mode::TARGETABLE;
            let dir = res.hit.map(|h| h.dir.map(|x| f32::from_bits(x.0))).unwrap_or([0.0; 4]);
            let (mut sp, mut up) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
            let a = knock::aim(dir, &mut sp, &mut up);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, up);
            knock::start(w, id, kr, a, 6, 1, 0);
            c::set_pf(w, id, kr + knock::k::ZOFF, f32::from_bits(0x3f59_999a));
            c::set_pf(w, id, kr + knock::k::KEY_APEX, 7.0);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, 14.0);
            set_state(w, id, st::DYING);
            w.mm(id).has_collision = false;
            c::set_pu8(w, id, pv::FLASH + 7, 0x78);
            flash::start(w, id, pv::FLASH);
        }
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
    let t = target::acquire_in(w, id, range, path_ok(w, id, pv::AREA));
    store_target(w, id, &t);
    if t.kind != 2 {
        let tp = c::pv4(w, id, pv::TGT);
        if range < c::dist2(c::pv4(w, id, pv::HOME), tp) || 3.0 < (c::pos(w, id)[2] - tp[2]).abs() { c::set_pi32(w, id, pv::TGT_KIND, 2); }
    }
    if c::pi32(w, id, pv::TGT_MOBY) == 0 {
        let hm = w.hero_moby;
        c::set_pi32(w, id, pv::TGT_MOBY, hm.map_or(0, |m| m as i32 + 1));
        let hp = hero_p(w);
        c::set_pv4(w, id, pv::TGT, hp);
    }
    true
}

/// State 0 (`0x2d3e50` case 0). False: deleted.
fn init(w: &mut World, id: MobyId) -> bool {
    let has_points = path_ok(w, id, pv::AREA).is_some_and(|a| !w.svc.splines[a].is_empty());
    if !has_points {
        // STUB_printf(0x1fa8a0 / 0x1fa8c0, spawn id): the missing path message.
        w.delete_moby(id);
        return false;
    }
    let cm = c::pi32(w, id, pv::COUNTER);
    count(w, cm, 0x14c, 1);
    let p = c::pos(w, id);
    c::set_pv4(w, id, pv::HOME, p);
    c::set_pv4(w, id, pv::SPAWN, p);
    c::set_pi16(w, id, pv::D + 4, 1);
    c::set_pf(w, id, pv::D, 1.0);
    c::set_pu8(w, id, 0x5c, 1);
    c::set_pu8(w, id, pv::KEEP, 1);
    c::set_pu8(w, id, pv::D + 9, 1);
    c::set_pi16(w, id, pv::ALERT, 0);
    c::set_pu8(w, id, 0x58, 10);
    walker::seed(&mut w.mm(id).pvars, pv::J);
    let top = c::pf(w, id, pv::SPEED) * c::DT;
    c::set_pf(w, id, pv::J + 8, 2.0);
    c::set_pi32(w, id, pv::J, 0x200);
    c::set_pf(w, id, pv::J + 4, 0.5);
    c::set_pf(w, id, pv::TOP, top);
    c::set_pf(w, id, pv::J + 0xc, 2.0);
    if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
    // +0xd0 = gp−0x5250: the table's own sequences ([`react::ARIDIA_580`]).
    if c::pi32(w, id, pv::AMBUSH_A) < 0 {
        if c::pi32(w, id, pv::LAIR) < 0 {
            if 0 <= c::pi32(w, id, pv::WAIT_MOBY) {
                set_state(w, id, st::WAIT);
                let m = w.mm(id);
                m.has_collision = false;
                m.mode = (m.mode & !mode::TARGETABLE) | 0x41;
                return true;
            }
            set_state(w, id, st::START);
        } else {
            c::set_pf(w, id, pv::J + 0xc, 0.2);
            set_state(w, id, st::LAIR);
        }
        blend(w, id, 2, 3);
    } else {
        if c::pi32(w, id, pv::AMBUSH_MOBY) == -1 {
            w.delete_moby(id);
            return false;
        }
        set_state(w, id, st::AMBUSH);
        let m = w.mm(id);
        m.has_collision = false;
        m.position[2] += 10.0;
        m.mode = (m.mode & !mode::TARGETABLE) | 1;
        blend(w, id, 0, 3);
    }
    true
}

/// The alert's surfacing (`LAB_002d5070`): the alert counting and `randi(19) == 0` → 0x15 (class sound 4, blend 0
/// raw 0), +0x274 = −1.
fn alert_surface(w: &mut World, id: MobyId) {
    if alert(w, id) == 0 || w.rng.randi(0x13) != 0 { return; }
    set_state(w, id, st::UP_ALERT);
    w.play_sound(4, 0, id);
    c::blend_to(w, id, 0, 0, 0);
    c::set_pi32(w, id, pv::KEEP_CUBOID, -1);
}

/// Turn toward `t` + the side offset (`SpringTurn2(…, acc, 0.3, max)`), then the walker step toward 2·dir from here.
fn swim_to(w: &mut World, id: MobyId, t: c::V, side: bool, acc: f32, max: f32) {
    let p = c::pos(w, id);
    let mut h = c::atan(t[0] - p[0], t[1] - p[1]);
    if side { h += c::pf(w, id, pv::SIDE); }
    turn::spring_turn2_pvar(w, id, h, acc, f32::from_bits(0x3e99_999a), max, pv::TURN_V);
    let (cy, sy) = c::cs(c::yaw(w, id));
    let q = c::add([cy + cy, sy + sy, 0.0, c::pos(w, id)[3] * 0.0], c::pos(w, id));
    let mut out = [0.0; 4];
    walker::step(w, id, pv::J, 1.0, [q[0], q[1], q[2], 0.0], &mut out);
}

fn fin_up(w: &mut World, id: MobyId) { w.mm(id).position[2] += k::FIN; }
fn fin_down(w: &mut World, id: MobyId) { w.mm(id).position[2] -= k::FIN; }
fn fin_puff_maybe(w: &mut World, id: MobyId) {
    if w.rng.randi(0xff) & 1 != 0 {
        let p = c::pos(w, id);
        fin_puffs(w, p);
    }
}

/// State 2: roam under the sand (module doc).
fn roam(w: &mut World, id: MobyId) {
    fin_up(w, id);
    let h = c::pv4(w, id, pv::HOME);
    let p = c::pos(w, id);
    let a = c::add_rot(c::atan(h[0] - p[0], h[1] - p[1]), c::pf(w, id, pv::SIDE));
    turn::spring_turn2_pvar(w, id, a, f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3e99_999a), f32::from_bits(0x3d23_d70a), pv::TURN_V);
    let yaw = c::yaw(w, id);
    let m = w.mm(id);
    m.position[0] += yaw.cos() * c::DT;
    m.position[1] += yaw.sin() * c::DT;
    if c::dec_timer_pvar_i32(w, id, pv::SIDE_T) != 0 {
        let t = w.ticks(60);
        c::set_pi32(w, id, pv::SIDE_T, t);
        let s = c::pf(w, id, pv::SIDE_DEG) * 0.017_453_292;
        let r = w.rng.randf(-s, s);
        c::set_pf(w, id, pv::SIDE, r);
    }
    if w.m(id).visible != 0 { fin_puff_maybe(w, id); }
    fin_down(w, id);
    if kind(w, id) == 2 {
        alert_surface(w, id);
        return;
    }
    let keep = c::pi32(w, id, pv::KEEP_CUBOID);
    if 0 <= keep {
        let t = c::pv4(w, id, pv::TGT);
        if in_cuboid(w, t, keep) && alert(w, id) == 0 { return; }
        if w.rng.randi(9) != 0 { return; }
        c::set_pi32(w, id, pv::KEEP_CUBOID, -1);
        return;
    }
    if w.rng.randi(4) != 0 { return; }
    let mut s = c::pf(w, id, pv::SIDE_DEG) * 0.017_453_292;
    let sign = c::pi32(w, id, pv::SIDE_SIGN);
    if sign == 0 {
        if w.rng.randi(0x100) & 1 != 0 { s = -s; }
    } else {
        s *= sign as f32;
    }
    c::set_pf(w, id, pv::SIDE, s);
    let f = w.rng.randf(30.0, 90.0);
    let t = w.ticks(f as i32);
    c::set_pi32(w, id, pv::SIDE_T, t);
    c::set_pf(w, id, pv::TOP, c::DT * 4.0);
    set_state(w, id, st::DIVE_CHASE);
    blend(w, id, 0xc, 3);
}

/// States 5 / 0x16: the chase (module doc).
fn chase(w: &mut World, id: MobyId) {
    let under = state(w, id) == st::UNDER_CHASE;
    if under {
        fin_up(w, id);
        fin_puff_maybe(w, id);
    }
    let t = c::pv4(w, id, pv::TGT);
    swim_to(w, id, t, true, f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3e4c_cccd));
    area_check(w, id);
    // The game tests the state again: a shark the area check sent to 0x12 keeps its fin height.
    let _ = under;
    if state(w, id) == st::UNDER_CHASE { fin_down(w, id); }
    let s = state(w, id);
    let lair = c::pi32(w, id, pv::LAIR);
    if kind(w, id) == 2 {
        if lair < 0 {
            set_state(w, id, if alert(w, id) != 0 { st::ALERTED } else { st::DIVE_HOME });
            return;
        }
        set_state(w, id, st::HOME);
        return;
    }
    if !(lair < 0 || in_cuboid(w, t, lair)) {
        set_state(w, id, st::HOME);
        return;
    }
    let d = c::dist2(c::pos(w, id), t);
    if d < 12.0 && s == st::UNDER_CHASE {
        set_state(w, id, st::UP_CHASE);
        w.play_sound(4, 0, id);
        blend(w, id, 0, 3);
    } else if d < 1.5 && s == st::CHASE {
        set_state(w, id, st::BITE);
        blend(w, id, 1, 3);
    }
}

/// State 6: the bite (module doc).
fn bite(w: &mut World, id: MobyId) {
    let t = c::pv4(w, id, pv::TGT);
    let p = c::pos(w, id);
    turn::spring_turn2_pvar(w, id, c::atan(t[0] - p[0], t[1] - p[1]), f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3e99_999a), f32::from_bits(0x3e4c_cccd), pv::TURN_V);
    let a = &w.m(id).anim;
    if a.seq_a == a.seq_b && ground::passed_frame(w, id, 13.0) {
        let r = w.m(id).rows[0];
        let mut q = c::add(c::set_len3(r, 0.75), c::pos(w, id));
        q[2] += 0.5;
        attack::sphere_hit(w, 0.5, 1.0, 1.0, id, q, 1, 0, 1, 0);
    }
    if wrapped(w, id) && 1.5 < c::dist2(c::pos(w, id), t) {
        set_state(w, id, st::CHASE);
        blend(w, id, 0, 3);
    }
    let mut p = c::pos(w, id);
    let g = ground::ground(w, p, 0.5, 0).z;
    let vz = c::pf(w, id, pv::JVZ) - c::DT2 * 9.8;
    c::set_pf(w, id, pv::JVZ, vz);
    p[2] += vz;
    if p[2] < g {
        c::set_pf(w, id, pv::JVZ, 0.0);
        p[2] = g;
    }
    c::set_pos(w, id, p);
}

/// States 7 / 0x17: go home (module doc).
fn home(w: &mut World, id: MobyId) {
    let under = state(w, id) == st::UNDER_HOME;
    if under {
        fin_up(w, id);
        fin_puff_maybe(w, id);
    }
    let h = c::pv4(w, id, pv::HOME);
    swim_to(w, id, h, false, f32::from_bits(0x3ca3_d70a), f32::from_bits(0x3dcc_cccd));
    if under { fin_down(w, id); }
    if c::dist2(c::pos(w, id), h) < 0.5 {
        c::set_pi32(w, id, pv::OUT, 0);
        if state(w, id) == st::HOME {
            if c::pi32(w, id, pv::LAIR) < 0 {
                set_state(w, id, st::DIVE_HOME);
                c::set_pf(w, id, pv::TOP, c::DT * 1.5);
                blend(w, id, 0, 3);
            } else {
                set_state(w, id, st::LAIR);
                blend(w, id, 2, 3);
            }
        } else {
            set_state(w, id, st::ROAM);
            c::set_pf(w, id, pv::TOP, c::DT * 1.5);
            blend(w, id, 0xc, 5);
        }
    }
    if c::pi32(w, id, pv::OUT) != 0 {
        let r = w.m(id).rows[0];
        let p = c::add(c::pos(w, id), c::set_len3(r, c::DT * 0.5));
        c::set_pos(w, id, p);
        return;
    }
    if kind(w, id) == 2 {
        alert_surface(w, id);
        return;
    }
    let t = c::pv4(w, id, pv::TGT);
    let lair = c::pi32(w, id, pv::LAIR);
    let top = c::pf(w, id, pv::SPEED) * c::DT;
    if 0 <= lair {
        if in_cuboid(w, t, lair) {
            set_state(w, id, st::CHASE);
            c::set_pf(w, id, pv::TOP, top);
        }
        return;
    }
    if c::pf(w, id, pv::RANGE) <= c::len3(c::sub(t, h)) { return; }
    let s = if state(w, id) == st::HOME { st::CHASE } else { st::UNDER_CHASE };
    set_state(w, id, s);
    c::set_pf(w, id, pv::TOP, top);
    blend(w, id, 0, 3);
}

/// State 0xc: spring the ambush (module doc).
fn ambush(w: &mut World, id: MobyId) {
    let hp = hero_p(w);
    let hit = in_cuboid(w, hp, c::pi32(w, id, pv::AMBUSH_A)) || in_cuboid(w, hp, c::pi32(w, id, pv::AMBUSH_B));
    if !hit { return; }
    let Some(src) = moby_at(w, c::pi32(w, id, pv::AMBUSH_MOBY)) else { return };
    let s = c::pos(w, src);
    let h = c::pv4(w, id, pv::HOME);
    let g = c::DT2 * k::JUMP_G;
    let vz = w.rng.randf(6.0, 10.0) * c::DT;
    // `0x1f9988`: the VU `vsqrt` (of the magnitude).
    let fall = ((g + g) * (s[2] - h[2])).abs().sqrt();
    let speed = c::dist2(h, s) / ((fall + vz + vz) / g);
    let mut v = c::sub(h, s);
    v[2] = 0.0;
    v = c::set_len3(v, speed);
    v[2] = vz;
    c::set_pv4(w, id, pv::VEL, v);
    c::set_pos(w, id, s);
    let has = crate::moby_update::classes::units::class_collision(w, w.m(id).o_class);
    let m = w.mm(id);
    m.has_collision = has;
    m.mode = (m.mode & !1) | mode::TARGETABLE;
    c::blend_to(w, id, 0xb, 0, 0);
    set_state(w, id, st::FLIGHT);
}

/// State 0xd (and 0x1b after its puff): the flight (module doc).
fn flight(w: &mut World, id: MobyId) {
    let r = c::sub_rot(w.m(id).rotation[1], c::DT * 4.0 * PI);
    w.mm(id).rotation[1] = r;
    let v = c::pv4(w, id, pv::VEL);
    let p = c::add(c::pos(w, id), v);
    c::set_pos(w, id, p);
    c::set_pf(w, id, pv::VZ, v[2] - c::DT2 * k::JUMP_G);
    let hz = c::pf(w, id, pv::HOME + 8);
    if hz - k::FIN <= p[2] {
        if p[2] < hz + 0.6 { dive_puffs(w, p, 6); }
        return;
    }
    let has = crate::moby_update::classes::units::class_collision(w, w.m(id).o_class);
    let m = w.mm(id);
    m.has_collision = has;
    m.mode |= mode::TARGETABLE;
    m.rotation[1] = 0.0;
    m.position[2] = hz - k::FIN;
    set_state(w, id, st::UNDER_HOME);
    blend(w, id, 0xc, 1);
}

/// State 0xe: alerted, on the surface (module doc).
fn alerted(w: &mut World, id: MobyId) {
    let t = c::pv4(w, id, pv::TGT);
    swim_to(w, id, t, true, f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3e4c_cccd));
    area_check(w, id);
    let p = c::pos(w, id);
    let (o, hit) = w.clamp_to_path_hit(c::pi32(w, id, pv::AREA), crate::moby_update::services::pv(p), crate::moby_update::services::pv(t));
    let near = hit && c::dist3(p, o.map(|x| x.to_f32())) < c::pf(w, id, pv::WALL_D);
    if alert(w, id) == 0 {
        set_state(w, id, st::DIVE_HOME);
        return;
    }
    if near {
        set_state(w, id, st::WALL);
        blend(w, id, 5, 10);
    } else {
        near_or_chase(w, id, t);
    }
}

/// 0xe / 0xf with the alert counting and no wall: within 1.5 → 6 (blend 1, 10); a target → 5 (blend 0, 15).
fn near_or_chase(w: &mut World, id: MobyId, t: c::V) {
    if c::dist2(c::pos(w, id), t) < 1.5 {
        set_state(w, id, st::BITE);
        blend(w, id, 1, 10);
    } else if kind(w, id) != 2 {
        set_state(w, id, st::CHASE);
        blend(w, id, 0, 15);
    }
}

/// States 0x11 / 0x12: dive (module doc).
fn dive(w: &mut World, id: MobyId) {
    fin_up(w, id);
    let p = c::pos(w, id);
    let hz = c::pf(w, id, pv::HOME + 8);
    let g = ground::ground(w, p, 0.5, 0).z.max(hz);
    let mut z = p[2];
    turn::approach(g, c::DT * 4.0, &mut z);
    w.mm(id).position[2] = z;
    if g == z {
        blend(w, id, 0xc, 5);
        let s = if state(w, id) == st::DIVE_CHASE { st::UNDER_CHASE } else { st::UNDER_HOME };
        set_state(w, id, s);
    }
    fin_down(w, id);
    let t10 = w.ticks(10) as u64;
    if t10 < w.counter {
        dive_puffs(w, [p[0], p[1], g, p[3]], 6);
    }
}

/// States 0x13 / 0x14 / 0x15: surface (module doc).
fn surface(w: &mut World, id: MobyId) {
    fin_up(w, id);
    let p = c::pos(w, id);
    let g = ground::ground(w, p, 0.5, 0).z;
    let mut z = p[2];
    let r = turn::approach(g + k::FIN, c::DT * 4.0, &mut z);
    w.mm(id).position[2] = z;
    if r == 0.0 {
        match state(w, id) {
            st::UP_CHASE => { set_state(w, id, st::CHASE); blend(w, id, 0, 3); }
            st::UP_HOME => { set_state(w, id, st::HOME); blend(w, id, 0, 3); }
            _ => {
                let d = w.rng.randf(2.0, 3.5);
                c::set_pf(w, id, pv::WALL_D, d);
                c::set_pf(w, id, pv::TOP, c::DT * 4.0);
                set_state(w, id, st::ALERTED);
                blend(w, id, 0, 3);
            }
        }
    }
    fin_down(w, id);
    dive_puffs(w, [p[0], p[1], g, p[3]], 6);
}

/// `0x2d3e50`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    if w.svc.units.word(k::FREEZE) != 0 { return; }
    // 0x264dd0(2.1, m, 0, +0x2a0): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.1, id, 0, id, 0x2a0);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 28.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x16;
        }
    }
    if !pre(w, id) { return; }
    match state(w, id) {
        st::INIT => { init(w, id); }
        st::START => {
            set_state(w, id, st::DIVE_HOME);
            blend(w, id, 0, 3);
        }
        st::ROAM => roam(w, id),
        st::LAIR => {
            if kind(w, id) != 2 && in_cuboid(w, c::pv4(w, id, pv::TGT), c::pi32(w, id, pv::LAIR)) {
                set_state(w, id, st::CHASE);
                blend(w, id, 0, 3);
            }
        }
        st::CHASE | st::UNDER_CHASE => chase(w, id),
        st::BITE => bite(w, id),
        st::HOME | st::UNDER_HOME => home(w, id),
        st::KNOCKED => {
            if knock::update(w, id, pv::K) & 1 != 0 {
                set_state(w, id, 0xb);
                if w.m(id).anim.seq_b != 4 {
                    let t = w.ticks(3);
                    w.anim_blend(id, 4, 8, t);
                }
            } else if c::pos(w, id)[2] < 0.0 {
                dust(w, id);
                w.delete_moby(id);
            }
        }
        st::HELD => {
            if react::carried(w, id, pv::K) != 0 {
                set_state(w, id, st::START);
                c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
            }
        }
        st::AMBUSH => ambush(w, id),
        st::FLIGHT => flight(w, id),
        st::ALERTED => alerted(w, id),
        st::WALL => {
            let t = c::pv4(w, id, pv::TGT);
            let p = c::pos(w, id);
            let r = c::DT2 * 2.0 * PI;
            turn::turn_toward_pvar(w, id, c::atan(t[0] - p[0], t[1] - p[1]), r, r, c::DT * 2.0 * PI, pv::TURN_V);
            if alert(w, id) == 0 { set_state(w, id, st::DIVE_HOME); } else { near_or_chase(w, id, t); }
        }
        st::DIVE_CHASE | st::DIVE_HOME => dive(w, id),
        st::UP_CHASE | st::UP_HOME | st::UP_ALERT => surface(w, id),
        st::DYING => {
            let r = knock::update(w, id, pv::K);
            if r & 1 != 0 && c::pi32(w, id, pv::SPLASHED) == 0 {
                c::set_pi32(w, id, pv::SPLASHED, 1);
                let p = c::pos(w, id);
                dive_puffs(w, p, 0x1b);
            }
            if r & 0x140 == 0 {
                if c::pos(w, id)[2] < 0.0 {
                    dust(w, id);
                    w.delete_moby(id);
                }
            } else {
                set_death_bits(w, id, 0, -1);
                let n = find_nest(w, id);
                c::set_pi32(w, id, pv::NEST, n.map_or(0, |m| m as i32 + 1));
                dust(w, id);
                if n.is_some() {
                    w.mm(id).anim.speed = 0.0;
                    to_nest(w, id);
                    w.mm(id).mode |= 0x41;
                } else {
                    w.delete_moby(id);
                }
            }
        }
        st::FADE => {
            if 7 < w.m(id).alpha {
                w.mm(id).alpha -= 8;
                return;
            }
            let n = find_nest(w, id);
            c::set_pi32(w, id, pv::NEST, n.map_or(0, |m| m as i32 + 1));
            dust(w, id);
            if n.is_none() {
                w.delete_moby(id);
                return;
            }
            w.mm(id).alpha = 0x80;
            to_nest(w, id);
        }
        st::NESTED => {
            let n = find_nest(w, id);
            c::set_pi32(w, id, pv::NEST, n.map_or(0, |m| m as i32 + 1));
            if n.is_none() { w.delete_moby(id); }
        }
        st::LAUNCHED => {
            let v = w.rng.rand_vec(0.0, 0.2);
            let vel = [v[0] * c::DT * 5.0, v[1] * c::DT * 5.0, v[2] * c::DT * 5.0, 0.0];
            let a = w.rng.rand_range(20, 40);
            let life = w.ticks(a);
            let g = w.rng.rand_range(120, 150);
            let p = c::pos(w, id);
            let s = crate::particles::type04::Spawn { pos: p, vel, c1: 0x4030_6058, c2: 0x0030_6058, life, base: 100, growth: g as i16, additive: false };
            fx::part04(w, &s);
            flight(w, id);
        }
        st::WAIT => {
            let Some(m) = moby_at(w, c::pi32(w, id, pv::WAIT_MOBY)) else { return };
            if w.m(m).state < 5 || w.hero.state == 0x72 { return; }
            let has = crate::moby_update::classes::units::class_collision(w, w.m(id).o_class);
            let mm = w.mm(id);
            mm.mode = (mm.mode & !0x41) | mode::TARGETABLE;
            mm.has_collision = has;
            c::set_pi32(w, id, pv::WAIT_MOBY, -1);
            let t = w.ticks(0x1518);
            c::set_pi16(w, id, pv::WAIT_T, t as i16);
            set_state(w, id, st::INIT);
            w.mm(id).position[2] -= k::FIN;
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------------
// 668: the nests

/// `0x2dd2a8(nest)`: a shark of the group waiting in 0x1a: its +0xb8 = the nest, the nest's bolts split with it.
pub fn take_shark(w: &mut World, nest: MobyId) -> Option<MobyId> {
    let s = group(w, nest).into_iter().find(|&m| w.m(m).o_class == CLASSES[0] && w.m(m).state == st::NESTED)?;
    w.mm(s).parent = Some(nest);
    if w.m(nest).b4 < w.m(s).b4 { let b = w.m(nest).b4; w.mm(s).b4 = b; }
    let r = w.m(nest).b4.wrapping_sub(w.m(s).b4);
    w.mm(nest).b4 = if r < 1 { 1 } else { r };
    Some(s)
}

/// `0x2d6118(shark, from, vel)`: launched: 0x1b, shown and updated (mode & ~0x41), blend 0xb, at `from` with `vel`,
/// facing it.
pub fn launch(w: &mut World, id: MobyId, from: c::V, vel: c::V) {
    set_state(w, id, st::LAUNCHED);
    w.mm(id).mode &= !0x41;
    c::blend_to(w, id, 0xb, 0, 1);
    c::set_pos(w, id, from);
    c::set_pv4(w, id, pv::VEL, vel);
    c::set_yaw(w, id, c::atan(vel[0], vel[1]));
}

/// `0x2dcfe0`: the nest's tick (module doc). False: deleted.
fn nest_pre(w: &mut World, id: MobyId) -> bool {
    if c::pu8(w, id, nv::KEEP) == 2 {
        c::set_pu8(w, id, nv::KEEP, 1);
        let cm = c::pi32(w, id, nv::COUNTER);
        count(w, cm, 0x150, -1);
        w.delete_moby(id);
        return false;
    }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, nv::D, 0, 4);
    if hit.is_some() && res.out5 != 1 && state(w, id) != nst::DYING {
        if res.out5 == 3 { c::set_pf(w, id, nv::D, 0.0); } else { let h = c::pf(w, id, nv::D) - res.damage; c::set_pf(w, id, nv::D, h); }
        let kr = nv::K;
        c::set_pi32(w, id, kr + knock::k::FLAGS, 4);
        let dir = res.hit.map(|h| h.dir.map(|x| f32::from_bits(x.0))).unwrap_or([0.0; 4]);
        let fly = |w: &mut World, out: f32, up: f32, seq: u8| {
            let (mut sp, mut u) = (out * c::DT, up * c::DT);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, u);
            let a = knock::aim(dir, &mut sp, &mut u);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, u);
            knock::start(w, id, kr, a, seq, 1, 0);
        };
        if 0.0 < c::pf(w, id, nv::D) {
            fly(w, 4.0, 3.0, 3);
            c::set_pu8(w, id, nv::FLASH + 7, 0x78);
            if 1.0 <= res.damage {
                set_state(w, id, nst::KNOCKED);
                w.anim_blend(id, 2, 0, 0);
            }
        } else {
            w.mm(id).mode &= !mode::TARGETABLE;
            c::set_pu8(w, id, nv::FLASH + 7, 0xfa);
            set_state(w, id, nst::DYING);
            fly(w, 11.0, 3.0, 3);
            c::set_pf(w, id, kr + knock::k::KEY_APEX, 9.0);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, 19.0);
            c::blend_to(w, id, 3, 0, 0);
            set_death_bits(w, id, 0, -1);
        }
        flash::start(w, id, nv::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, nv::FLASH);
    // FastDecTimer__FRs(m + 0xbc): the moby's +0xbc / +0xbd as one s16.
    let m = w.mm(id);
    let mut t = i16::from_le_bytes([m.cmd, m.bbd]);
    crate::moby_update::services::fast_dec_timer_s16(&mut t);
    let [a, b] = t.to_le_bytes();
    m.cmd = a;
    m.bbd = b;
    true
}

/// `0x2dcb38`: the nest (module doc).
pub fn nest_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < nv::SIZE { return; }
    if !nest_pre(w, id) { return; }
    if 0x80 <= state(w, id) { return; }
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 28.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x16;
        }
    }
    match state(w, id) {
        nst::INIT => {
            let (lvl, b1) = (w.svc.level, w.m(id).spawn_flag);
            let got = w.svc.counters.spawner_bolts.get(&(lvl, b1)).copied().unwrap_or(0);
            let r = w.m(id).b4.wrapping_sub(got);
            w.mm(id).b4 = if r < 1 { 1 } else { r };
            set_state(w, id, nst::IDLE);
            c::set_pf(w, id, nv::D, 3.0);
            c::set_pi16(w, id, nv::D + 4, 3);
            c::set_pu8(w, id, nv::KEEP, 1);
            let t = w.ticks(c::pi32(w, id, nv::PERIOD));
            c::set_pi32(w, id, nv::TIMER, t);
            let cm = c::pi32(w, id, nv::COUNTER);
            count(w, cm, 0x150, 1);
        }
        nst::IDLE => {
            if c::dec_timer_pvar_i32(w, id, nv::TIMER) == 0 { return; }
            if take_shark(w, id).is_none() {
                let t = w.ticks(c::pi32(w, id, nv::PERIOD));
                c::set_pi32(w, id, nv::TIMER, t);
                return;
            }
            set_state(w, id, nst::SPAWN);
            blend(w, id, 1, 10);
        }
        nst::SPAWN => {
            if wrapped(w, id) {
                set_state(w, id, nst::IDLE);
                blend(w, id, 0, 10);
                return;
            }
            if ground::key_time(w, id) != 12.0 { return; }
            let deaths = w.missions.ammo_crate_gate(w.m(id).mission).min(0x14);
            let extra = w.ticks(deaths * 0xf);
            let t = w.ticks(c::pi32(w, id, nv::PERIOD) + extra);
            c::set_pi32(w, id, nv::TIMER, t);
            let Some(s) = take_shark(w, id) else { return };
            let cm = c::pi32(w, id, nv::COUNTER);
            count(w, cm, 0x14c, 1);
            let p = c::pos(w, id);
            let from = [p[0], p[1], p[2] + 1.5, p[3]];
            let h = if w.m(s).pvars.len() >= pv::SIZE { c::pv4(w, s, pv::HOME) } else { p };
            let mut v = c::set_len2(c::sub(h, p), c::DT + c::DT);
            v[2] = c::DT * 6.0;
            launch(w, s, from, v);
        }
        nst::KNOCKED => {
            knock::update(w, id, nv::K);
            if wrapped(w, id) {
                set_state(w, id, nst::IDLE);
                blend(w, id, 0, 20);
            }
        }
        nst::DYING => {
            if knock::update(w, id, nv::K) & 0x40 == 0 { return; }
            let cm = c::pi32(w, id, nv::COUNTER);
            count(w, cm, 0x150, -1);
            let p = c::pos(w, id);
            fx::beam_explosion(w, &NEST_BLAST, Some(id), p);
            let rot = w.m(id).rotation;
            for class in [1764, 1764, 1765, 1765] { fx::break_piece(w, id, class, p, rot, 0, 0); }
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// The nest's blast: `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, m, +0x40, pos, 5, 2, 4, −1, 1, 1, −1, 0)`.
pub const NEST_BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 1, sound: -1, shake: true };
