//! **The Hydrodisplacer** (item 22 = 0x16, gadget class 1251; its update is `0x309cd0` on level 01, the same code in
//! every overlay's class table: L02 `0x2ed8a8`, L05 `0x31af58`, …) and **its use poses 0x38..0x3a** (hero states entered
//! only by this update, on a Hydrodisplacer pad 341). Read from the level01 decompiler output (0x309cd0, 0x30a4e0,
//! 0x30a610, 0x30a670) and the level05 SetState / physics / transitions (0x24cee8, 0x244a70, 0x255960; level00
//! 0x2223f8 / 0x217970 / 0x229b70 are the same cases). The pads are `crate::moby_update::classes::units::hydro_pad`; the
//! water they drive is the patch managers of `crate::water::managers` (they take the command bits 8 / 4 on `+0xbc`).
//!
//! **What it does.** Standing on a pad (Ratchet's ground moby `0x13f64c` of class 0x155 = 341, on the ground, movement
//! group 0 or 1) with the gadget out, ○ pressed and held: when the pad's linked water moby (pad pvar +0x00) can give
//! water (empty gadget: its `+0xbc` bit 1) or take it (full gadget: bit 0), Ratchet takes the use pose 0x38 (sequence
//! 0x46 to suck / 0x83 to pour), then 0x39 (0x47 / 0x84) with his class sound 0x14 (suck) / 0x15 (pour); the water moby
//! gets the command 8 (the gadget fills: 0x141400 = 1) or 4 (it empties: 0x141400 = 0). With two camera cuboids on the
//! pad (pvar +0x04 / +0x08) the change plays as a cutaway: fade to black over 15 ticks, the script camera cut to the
//! first cuboid, the letterbox, the fade back, the camera springing to the second cuboid (`t` → 1), the fade again, the
//! follow camera back (`CameraScript2(3)`), the fade back, and the end pose 0x3a (0x48 after sucking, 0x85 after
//! pouring); a full gadget runs the cutaway from the second cuboid to the first. Without cameras the command is sent at
//! once and the pose holds for 100 ticks. A full gadget drips (type-35 drops from its nozzle) and bulges (its joint
//! records: list 1 scaled 2.3; lists 2 and 3 pushed along y while not in 0x38 / 0x39).
//!
//! **Item pvars** ([`Hydro`]): +0x00 the hold timer (state 5), +0x04 the drip timer, +0x08 / +0x0c the cutaway's from /
//! to cuboids, +0x10 / +0x14 the cutaway's `t` and its spring velocity; the moby's +0x20 is its state, +0xbc the
//! cutaway's phase (`Gadgets::item_bc`).
//!
//! **Coverage** (`address | what | status`):
//!
//! | address | what | status |
//! |---|---|---|
//! | 0x309cd0 head | the slot being put away (`0x140404 == 3`) → `0x30a670` | ported ([`update`] → [`reset`]) |
//! | | full (0x141400): game mode 0 (0x15f5c4) and not in 0x38 / 0x39 → the drip `0x30a4e0` | ported ([`drip`]; hand item updates run only in mode 0 in the port) |
//! | | full: record 0 (0x140ce0..): joint list 1, k = 0.02·0x15ed64, d = 0.3·0x15ed64, scale 2.3; not in 0x38 / 0x39: records 1 / 2 (0x140d90 / 0x140e40): lists 2 / 3, translation target y = (1024 / item scale +0x2c)·−0.2, the same k / d | ported ([`bulge`]; the records run in the slot loop, `super::gadgets::hand_records`) |
//! | case 0 | `PlayClassSound(0, 0, item)`; state 1 | ported (item sound 0) |
//! | case 1 | the item's sequence wrapped (+0x70 & 2) → state 2 | ported |
//! | case 2 | `PadPressedWithin(○ mask 0x1403f0, ticks(8))`, ○ held (0x13cae0), a ground moby, air ticks 0x13f65e = 0, its class 0x155, `0x30a610` (the link can give / take), group 0x1413dc < 2 → state 3, `SetState(0x38, 0)`, `SetAnim(ticks(6), 0x46 / 0x83, 0)` | ported ([`ItemCall`](super::gadgets::ItemCall)s run by `gadgets::after_items`) |
//! | 0x30a610 | link = pad pvar +0x00; empty: link +0xbc & 2; full: link +0xbc & 1 | ported ([`link_ready`]; a link of −1: false [L], the game reads the table base − 0x100) |
//! | case 3 | off the pad (ground moby not 0x155) → `0x30a670`; Ratchet's sequence wrapped (0x13fde8 & 2): `SetState(0x39, 0)`, `SetAnim(ticks(6), 0x47 / 0x84, 0)`, `PlayClassSound(0x14 / 0x15, 0, hero moby)` | ported (Ratchet's class sound through `fx.item_voices`) |
//! | | pad cuboids +0x04 and +0x08 both set: +0xbc = 0, state 4, `t` = 0, `CameraScript(camera 0x167240, Euler 0x167250, 1, 0, 0)`; empty: from = +0x08, to = +0x04; full: from = +0x04, to = +0x08 | ported (`crate::cinematic::camera_script` through the moby world) |
//! | | else: state 5, timer = `trunc(100·0x15ed68)` (gp−0x4bc0 = 100.0), the link's +0xbc = 8 (empty → full, 0x141400 = 1) or 4 (full → empty, 0x141400 = 0) | ported |
//! | case 4 | off the pad → `0x30a670` (the state stays 4) | ported |
//! | phase 0 | fade 0x15f3fc `Approach(1, 1/ticks(15))`; reached 1: phase 1, `0x316dd0(from.centre)`, `0x316e28(from.euler)`, the command and the flip as case 3's else, letterbox 0x15f404 = 1 | ported |
//! | phase 1 | `t` = 0 and fade > 0: fade `Approach(0, 1/ticks(15))`; else `t` < 1: `Spring(1, 0.666·dt², 0.666·dt², 0.5·dt, &t, &tv)` (0x270830), camera position = lerp(t, from, to), Euler = (0, lerp_rot(y), lerp_rot(z)) → `0x316dd0` / `0x316e28`; `t` ≥ 1: fade up 1/15; at 1: phase 2, letterbox 0, `CameraScript2(3)` | ported |
//! | phase 2 | fade > 0: down 1/15; at 0: state 2, `SetState(0x3a, 0)`, `SetAnim(ticks(6), 0x48 (now full) / 0x85 (now empty), 0)` | ported |
//! | case 5 | off the pad, or the timer out (`FastDecTimer`): `0x30a670`, state 2 | ported |
//! | 0x30a670 | state 4: letterbox 0x15f404 = 0, fade 0x15f3fc = 0, `CameraScript2(3)`; group 0x13: `SetState(0, 1)` | ported ([`reset`]) |
//! | 0x30a4e0 | `FastDecTimer(+0x04)` out: the item's joint list 0 point; speed `randf(dt, 3·dt)`; v = the item's row +0xd0 at length −speed, + 0.8 × Ratchet's displacement 0x13f450 in x / y; v.z += disp.z + `randf(−0.5·dt, 2·dt)`; `PartType35Spawn(point, v, 2, rand_range(90, 120))`; timer = `rand_range(ticks(5), ticks(15))` | ported ([`drip`], `fx::PartSpawn::Drop35`) |
//! | SetState 0x38 / 0x39 / 0x3a | group 0x13, 0x1415d4 = 0; with `play`: `SetAnim(ticks(8), 0x46 / 0x47 / 0x48, 0)` | ported ([`entry`]) |
//! | physics 0x38..0x3a (L05 0x244a70) | no pad under him: nothing; else velocity = pad position − his, clamped to 2·dt (`0x2745f0`); yaw += clamp(pad yaw − yaw, ±9.424778·dt) | ported ([`physics`]; the pad's pose from `HeroWorld::ground_moby`) |
//! | transitions 0x38 / 0x39 | hand item 0x140408 ≠ 0x16 → `SetState(0, 1)` | ported ([`transitions`]) |
//! | transitions 0x3a | Ratchet's sequence wrapped → `SetState(0, 1)` | ported |
//! | sounds, particles, lights, stats, bolts, save flags | the class sounds and drops above; no light, stat, bolt or save write (0x141400 is the hero block's) | n/a |
//!
//! **Native.** Plain `f32` for the item's own math; the hero block's fields stay the hero's `Pf`.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::gadgets::ItemCall;
use super::items::{HitSink, ItemEnv};
use super::physics::{ticks, Env, DT};
use super::states::Ctx;
use super::Hero;
use crate::moby_runtime::MobyTable;
use crate::moby_update::creature::{add_rot, sub_rot, turn};
use crate::moby_update::services::pvar as p;
use crate::pad::button;
use crate::ps2v::Pf;
use crate::rng::Rng;

pub const HYDRODISPLACER: i32 = 0x16;
/// The pads' class (0x155).
pub const PAD_CLASS: i16 = 341;
/// The use poses.
pub const POSE_IN: i32 = 0x38;
pub const POSE_USE: i32 = 0x39;
pub const POSE_OUT: i32 = 0x3a;
/// Ratchet's sequences: sucking (empty gadget) and pouring (full gadget), in / use / out.
pub const SUCK: [u8; 3] = [0x46, 0x47, 0x48];
pub const POUR: [u8; 3] = [0x83, 0x84, 0x85];
/// Ratchet's class sounds of the use: sucking, pouring.
pub const SOUND_SUCK: i32 = 0x14;
pub const SOUND_POUR: i32 = 0x15;
/// The commands to the pad's water moby (`+0xbc`): the gadget fills (the pool drains), the gadget empties.
pub const CMD_FILL: u8 = 8;
pub const CMD_EMPTY: u8 = 4;
/// The hold without cameras (gp−0x4bc0 = 100.0 × 0x15ed68).
pub const HOLD: i32 = 100;
/// The fades' length (`1 / ticks(15)` a tick).
pub const FADE_TICKS: i32 = 15;
/// The full gadget's bulge: record 0's scale.
pub const BULGE_SCALE: f32 = 2.3;

/// The item moby's pvars (module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hydro {
    /// +0x00: the hold timer (state 5).
    pub timer: i32,
    /// +0x04: the drip timer.
    pub drip: i32,
    /// +0x08 / +0x0c: the cutaway's from / to cuboids.
    pub from: i32,
    pub to: i32,
    /// +0x10 / +0x14: the cutaway's `t` and its spring velocity.
    pub t: f32,
    pub tv: f32,
}

/// The pad Ratchet stands on (`0x13f64c` of class 0x155) and its pvars +0x00..+0x08: the link and the two cuboids.
#[derive(Clone, Copy, Debug)]
struct Pad {
    link: i32,
    cams: [i32; 2],
}

fn pad_under(hero: &Hero, table: &MobyTable) -> Option<Pad> {
    let m = table.mobys.get(hero.ground_moby?)?;
    if m.o_class != PAD_CLASS || m.pvars.len() < 0x0c { return None; }
    Some(Pad { link: p::i32(&m.pvars, 0), cams: [p::i32(&m.pvars, 4), p::i32(&m.pvars, 8)] })
}

/// `0x30a610`: the pad's water moby can give water (empty gadget: its +0xbc bit 1) or take it (full: bit 0).
fn link_ready(full: bool, table: &MobyTable, pad: Pad) -> bool {
    let Some(l) = usize::try_from(pad.link).ok().and_then(|l| table.mobys.get(l)) else { return false };
    if full { l.cmd & 1 != 0 } else { l.cmd & 2 != 0 }
}

/// The command and the flip (case 3's else, phase 0): the link's +0xbc = 4 (full → empty) or 8 (empty → full).
fn command(hero: &mut Hero, table: &mut MobyTable, link: i32) {
    let full = hero.gadgets.hydro_full;
    if let Some(l) = usize::try_from(link).ok().and_then(|l| table.mobys.get_mut(l)) { l.cmd = if full { CMD_EMPTY } else { CMD_FILL }; }
    hero.gadgets.hydro_full = !full;
}

fn seqs(full: bool) -> [u8; 3] { if full { POUR } else { SUCK } }

/// `0x309cd0`, the Hydrodisplacer's update (the slot loop's item update, with the slot ready or being put away).
pub fn update(hero: &mut Hero, table: &mut MobyTable, anim: &dyn super::anim::AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    if hero.items.slot.item.is_none() { return; }
    if hero.items.slot.state == 3 {
        reset(hero, table, env, hits, rng);
        return;
    }
    if hero.gadgets.hydro_full {
        let posing = matches!(hero.state, POSE_IN | POSE_USE);
        if !posing { drip(hero, env, rng); }
        bulge(hero, posing);
    }
    let state = hero.items.slot.item.as_ref().map_or(0, |it| it.mstate);
    match state {
        0 => {
            hero.fx.item_sounds.push(0);
            set_mstate(hero, 1);
        }
        1 => {
            if hero.items.slot.item.as_ref().is_some_and(|it| it.anim.flags & 2 != 0) { set_mstate(hero, 2); }
        }
        2 => {
            let mask = hero.items.slot.fire_mask;
            if env.pad.pressed_within(mask, ticks(8)).is_none() || env.pad.held & mask == 0 { return; }
            if hero.ground_moby.is_none() || hero.air_ticks != 0 { return; }
            let Some(pad) = pad_under(hero, table) else { return };
            if !link_ready(hero.gadgets.hydro_full, table, pad) { return; }
            if 1 < hero.group as u32 { return; }
            set_mstate(hero, 3);
            let s = seqs(hero.gadgets.hydro_full);
            hero.gadgets.calls.push(ItemCall::SetState { id: POSE_IN, play: false });
            hero.gadgets.calls.push(ItemCall::SetAnim { blend: ticks(6), seq: s[0], frame: 0 });
        }
        3 => {
            let Some(pad) = pad_under(hero, table) else { return reset(hero, table, env, hits, rng) };
            if anim.view().flags & 2 == 0 { return; }
            let full = hero.gadgets.hydro_full;
            hero.gadgets.calls.push(ItemCall::SetState { id: POSE_USE, play: false });
            hero.gadgets.calls.push(ItemCall::SetAnim { blend: ticks(6), seq: seqs(full)[1], frame: 0 });
            // `PlayClassSound(0x14 / 0x15, 0, 0x1413d0)`: Ratchet's class sound.
            hero.fx.item_voices.push(super::packs::SoundCmd::Voice { index: if full { SOUND_POUR } else { SOUND_SUCK }, flags: 0 });
            if pad.cams[0] != -1 && pad.cams[1] != -1 {
                hero.gadgets.item_bc = 0;
                set_mstate(hero, 4);
                let h = &mut hero.gadgets.hydro;
                h.t = 0.0;
                if full { (h.from, h.to) = (pad.cams[0], pad.cams[1]) } else { (h.from, h.to) = (pad.cams[1], pad.cams[0]) }
                let hero_ref: &Hero = hero;
                hits.world(table, hero_ref, rng, env.frame as u64, &mut |w| {
                    let (cam, euler) = ([w.camera[0].to_f32(), w.camera[1].to_f32(), w.camera[2].to_f32()], w.hero.loop_in.cam_euler);
                    crate::cinematic::camera_script(w, cam, euler, 1, 0, false);
                });
                return;
            }
            set_mstate(hero, 5);
            hero.gadgets.hydro.timer = HOLD;
            command(hero, table, pad.link);
        }
        4 => {
            let Some(pad) = pad_under(hero, table) else { return reset(hero, table, env, hits, rng) };
            cutaway(hero, table, env, hits, rng, pad);
        }
        5 => {
            let gone = pad_under(hero, table).is_none();
            if gone || super::guns::dec(&mut hero.gadgets.hydro.timer) {
                reset(hero, table, env, hits, rng);
                set_mstate(hero, 2);
            }
        }
        _ => {}
    }
}

fn set_mstate(hero: &mut Hero, s: u8) {
    if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = s; }
}

/// What the cutaway's phases do through the moby world.
#[derive(Clone, Copy, Debug, Default)]
struct Phase {
    fade: f32,
    next: Option<u8>,
    command: bool,
    end: bool,
}

/// Case 4: the cutaway (module doc, phases 0..2).
fn cutaway(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng, pad: Pad) {
    let phase = hero.gadgets.item_bc;
    let mut h = hero.gadgets.hydro;
    let step = 1.0 / ticks(FADE_TICKS) as f32;
    let mut out = Phase::default();
    let hero_ref: &Hero = hero;
    hits.world(table, hero_ref, rng, env.frame as u64, &mut |w| {
        let mut fade = w.svc.cinematic.fade;
        let shape = |w: &crate::moby_update::services::World, i: i32| w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i).map(|s| (s.centre(), s.euler));
        match phase {
            0 => {
                turn::approach(1.0, step, &mut fade);
                if 1.0 <= fade {
                    out.next = Some(1);
                    let (c, e) = shape(w, h.from).unwrap_or(([0.0; 3], [0.0; 3]));
                    crate::cinematic::camera_targets(w, Some(c), Some(e));
                    out.command = true;
                    crate::cinematic::letterbox(w, true);
                }
            }
            1 => {
                if h.t == 0.0 && 0.0 < fade {
                    turn::approach(0.0, step, &mut fade);
                } else if h.t < 1.0 {
                    let k = DT.to_f32() * DT.to_f32() * 0.666;
                    turn::spring(1.0, k, k, DT.to_f32() * 0.5, &mut h.t, &mut h.tv);
                    let (a, ea) = shape(w, h.from).unwrap_or(([0.0; 3], [0.0; 3]));
                    let (b, eb) = shape(w, h.to).unwrap_or(([0.0; 3], [0.0; 3]));
                    let pos = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * h.t);
                    let euler = [0.0, add_rot(ea[1], sub_rot(eb[1], ea[1]) * h.t), add_rot(ea[2], sub_rot(eb[2], ea[2]) * h.t)];
                    crate::cinematic::camera_targets(w, Some(pos), Some(euler));
                } else if fade < 1.0 {
                    turn::approach(1.0, step, &mut fade);
                } else {
                    out.next = Some(2);
                    crate::cinematic::letterbox(w, false);
                    crate::cinematic::camera_script2(w, 3);
                }
            }
            2 => {
                if 0.0 < fade { turn::approach(0.0, step, &mut fade); } else { out.end = true; }
            }
            _ => {}
        }
        crate::cinematic::set_fade(w, fade);
        out.fade = fade;
    });
    hero.gadgets.hydro = h;
    if let Some(n) = out.next { hero.gadgets.item_bc = n; }
    if out.command { command(hero, table, pad.link); }
    if out.end {
        set_mstate(hero, 2);
        // Now full after sucking: 0x48; now empty after pouring: 0x85.
        let seq = if hero.gadgets.hydro_full { SUCK[2] } else { POUR[2] };
        hero.gadgets.calls.push(ItemCall::SetState { id: POSE_OUT, play: false });
        hero.gadgets.calls.push(ItemCall::SetAnim { blend: ticks(6), seq, frame: 0 });
    }
}

/// `0x30a670`: a cutaway in progress (state 4) is undone (letterbox off, the fade cleared, the follow camera back);
/// Ratchet in the use poses (group 0x13) back to idle.
fn reset(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    if hero.items.slot.item.as_ref().is_some_and(|it| it.mstate == 4) {
        let hero_ref: &Hero = hero;
        hits.world(table, hero_ref, rng, env.frame as u64, &mut |w| {
            crate::cinematic::letterbox(w, false);
            crate::cinematic::set_fade(w, 0.0);
            crate::cinematic::camera_script2(w, 3);
        });
    }
    if hero.group == 0x13 { hero.gadgets.calls.push(ItemCall::SetState { id: 0, play: true }); }
}

/// The full gadget's joint records (module doc): record 0 every tick, records 1 / 2 outside 0x38 / 0x39.
fn bulge(hero: &mut Hero, posing: bool) {
    let s = 1.0f32; // 0x15ed64 (1.0 NTSC).
    let (k, d) = (s * 0.02, s * 0.3);
    let scale = hero.items.slot.item.as_ref().map_or(1.0, |it| it.scale);
    let g = &mut hero.gadgets.hand_joints;
    // The 32-bit stores of 1 / 2 / 3 at +0xa0 write the joint and clear the kind (+0xa2; the slot loop sets 5 again).
    g[0].joint = 1;
    g[0].kind = 0;
    g[0].k = k;
    g[0].d = d;
    g[0].scale = BULGE_SCALE;
    if !posing {
        let y = (1024.0 / scale) * -0.2;
        for (r, j) in [(1usize, 2i16), (2, 3)] {
            g[r].joint = j;
            g[r].kind = 0;
            g[r].trans_target[1] = y;
            g[r].k = k;
            g[r].d = d;
        }
    }
}

/// `0x30a4e0`: the full gadget's drip (module doc).
fn drip(hero: &mut Hero, env: &ItemEnv, rng: &mut Rng) {
    if !super::guns::dec(&mut hero.gadgets.hydro.drip) { return; }
    let point = super::guns::item_point(hero, env, 0);
    let dt = DT.to_f32();
    let speed = rng.randf(dt, dt * 3.0);
    let row = hero.items.slot.item.as_ref().map_or([0.0; 3], |it| [0, 1, 2].map(|k| f32::from_bits(it.rows[1][k])));
    let mut v = super::guns::with_len(row, -speed);
    let disp = [hero.disp[0].to_f32(), hero.disp[1].to_f32(), hero.disp[2].to_f32()];
    v[0] += disp[0] * 0.8;
    v[1] += disp[1] * 0.8;
    let jitter = rng.randf(dt * -0.5, dt + dt);
    v[2] = v[2] + disp[2] + jitter;
    let life = rng.rand_range(0x5a, 0x78);
    let r = super::fx::reserve(rng, 2);
    hero.fx.parts.push(super::fx::PartSpawn::Drop35 { pos: [point[0], point[1], point[2], 0.0], vel: [v[0], v[1], v[2], 0.0], kind: 2, life, rng: r });
    hero.gadgets.hydro.drip = rng.rand_range(ticks(5), ticks(15));
}

// ------------------------------------------------------------------------------------------------
// The use poses 0x38..0x3a.

/// SetState's entry of 0x38 / 0x39 / 0x3a.
pub(super) fn entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool, _old_sub: i32) -> Option<bool> {
    h.group = 0x13;
    h.f15d4 = 0;
    if play {
        let seq = SUCK[(id - POSE_IN).clamp(0, 2) as usize];
        h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(8)), seq, 0);
    }
    None
}

/// `0x2370b8` (L05 0x244a70) cases 0x38..0x3a: drawn to the pad's centre and turned to its yaw.
pub(super) fn physics(h: &mut Hero, env: &Env) -> bool {
    let Some((o_class, pos, yaw)) = h.ground_moby.and_then(|g| env.world.and_then(|w| w.moby_pose(g))) else { return true };
    if o_class != PAD_CLASS { return true; }
    let mut v = [Pf::f(pos[0]) - h.pos[0], Pf::f(pos[1]) - h.pos[1], Pf::f(pos[2]) - h.pos[2], Pf::ZERO];
    super::common::clamp_len_2745f0(&mut v, DT + DT);
    h.vel = v;
    let lim = DT.to_f32() * 9.424_778;
    let d = sub_rot(yaw, h.rot[2].to_f32()).clamp(-lim, lim);
    h.rot[2] = Pf::f(add_rot(h.rot[2].to_f32(), d));
    true
}

/// `0x242930` cases 0x38..0x3a.
pub(super) fn transitions(h: &mut Hero, c: &mut Ctx) {
    match h.state {
        POSE_IN | POSE_USE => {
            if h.items.slot.id != HYDRODISPLACER { h.set_state(c, 0, true); }
        }
        _ => {
            if c.anim.view().flags & 2 != 0 { h.set_state(c, 0, true); }
        }
    }
}

/// ○: the fire mask the update tests (the slot's +0x10).
pub const FIRE: u32 = button::CIRCLE;
