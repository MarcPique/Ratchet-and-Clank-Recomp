//! The audio layer on the game's tick (docs/plan/audio.md §3.1, §3.2): the class sounds the gameplay code plays
//! (`PlayClassSound` level01 0x2a1618 / 0x2a16c0) and the per-tick sound step, all on the game's one `rand`
//! stream ([`crate::tick::Game::rng`]) at the game's points:
//!
//! * **Moby class sounds** (crates, bolts, platforms, …): the moby loop calls `PlayClassSound` through
//!   [`crate::moby_update::services::SoundSink`]; [`ClassSoundSink`] allocates the slot at once
//!   ([`AudioSystem::play_class_sound`], `SoundSlotAlloc` 0x2a13a0), so its pitch-bend `randi` lands in the moby
//!   order. The listener is the camera 0x167240 as the previous tick's camera update left it.
//! * **Ratchet's animation triggers**: `RatchetAnimAdvance` 0x247d48 plays the class sound of the first trigger
//!   whose time its key time passed this tick ([`ratchet_trigger`]), inside the hero update: [`HeroClassSounds`]
//!   is the hero's [`HeroSounds`] (`Game::tick_with_hero_sounds` → `hero_update_with_sounds` calls it right
//!   after the advance), so the pitch-bend draw lands where the game makes it (before the back items' draw
//!   0x247800 and the hero's physics). The owner is Ratchet's moby as the last write-back left it; the listener
//!   is the camera of the previous tick (the camera updates after the hero), as for the moby sounds.
//! * **Ratchet's voices** `0x236738(index, flags)` (`PlayClassSound` on his moby: the hurt / death voices of
//!   `hero::damage`): [`HeroClassSounds`] too ([`HeroSounds::voice`]).
//! * **The sound step** [`sound_step`]: after the camera, before the counter increment (level01 `FUN_002aba68`,
//!   docs/plan/trace_results_novalis.md "Second savestate"): the EE frame (`sound_update` 0x2a0638: occlusion
//!   origin = 3 draws every frame, the 6-origin batch per new occluded sound, the sound instances' plays) and its
//!   800 samples.
//!
//! * **The hand item's sounds** (the Swingshot's fire / hit / pull, the wrench's hit: `PlayClassSound(i, 0, item)`):
//!   [`HeroSounds::item_sound`], played by the tick right after the item's update (`hero::gadgets::flush_item_sounds`),
//!   with the gadget class's defs (the gadget blobs' defs with their parked remap: [`crate::audio::LevelAudio`]).
//!
//! Clank (601) and the back packs (607–609) have no class sound defs on any level (class header +0x0d = 0), so
//! the triggers of their `MobyAnimAdvance` never play or draw; the hand items' animation triggers (wrench 71, …) are
//! not routed (the hero's item code advances them).

use super::voices::{Listener, Owner};
use super::{AudioSystem, FrameInput};
use crate::follow_camera::CameraView;
use crate::hero::{AnimView, Hero, HeroSounds};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::services::{SoundEvent, SoundSink};
use crate::rng::Rng;
use rc_formats::moby_anim::MobyAnimClass;
use rc_formats::sound_bank::SoundOwner;
use std::ops::DerefMut;

/// Class 0x472 (1138): its sounds may use slots 26..29 like the hero's.
pub const PRIVILEGED_CLASS: i16 = 0x472;

/// The listener of the sound code: the camera 0x167240 with its rows 0x167450.. (forward, left, up).
pub fn listener_of(cam: &CameraView) -> Listener {
    Listener { pos: cam.pos_f32(), rows: cam.rows_f32(), underwater: false, water_height: 0.0 }
}

/// [`listener_of`] with the underwater flag 0x167494 ([`AudioSystem::underwater`]) and the water level 0x13f640 (the
/// hero's `water_level`), as `sound_update` reads them.
pub fn listener_with_water(cam: &CameraView, underwater: bool, hero: &Hero) -> Listener {
    Listener { underwater, water_height: hero.water_level.to_f32(), ..listener_of(cam) }
}

/// A slot owner's position (moby +0x10) from the moby table; None when the moby is deleted (state 0xfd /
/// 0xfe) or gone.
pub fn owner_position(table: &MobyTable, id: u32) -> Option<[f32; 3]> {
    let m = table.mobys.get(id as usize)?;
    if m.state >= crate::moby_runtime::state::DELETED_STATIC { return None; }
    Some([m.position[0], m.position[1], m.position[2]])
}

impl AudioSystem {
    /// `PlayClassSound(index, flags, moby)` (0x2a1618; 0x2a16c0 with another class's table): class sound
    /// `index` of `ev.sound_class` (−1 when the class has no such def), `SoundSlotAlloc` with the moby as the
    /// owner (privileged: the hero `hero` or class 0x472), volume 0x400; the slot remembers the class-sound
    /// index (+0xe). Draws the def's pitch bend from `rng` when it gets a slot.
    pub fn play_class_sound(&mut self, ev: &SoundEvent, hero: Option<MobyId>, listener: &Listener, rng: &mut Rng) -> i32 {
        self.stats.class_sounds += 1;
        let Ok(local) = usize::try_from(ev.index) else { return -1 };
        let Some(def) = self.data.sounds.def(SoundOwner::Class(ev.sound_class as i32), local).copied() else { return -1 };
        let owner = Owner { id: ev.moby as u32, privileged: Some(ev.moby) == hero || ev.o_class == PRIVILEGED_CLASS };
        let k = self.slots.play(&def, ev.flags as u8, Some(owner), Some(ev.pos), None, 0x400, listener, rng);
        if k >= 0 {
            self.slots.slots[k as usize].class_index = ev.index as u16;
            self.stats.class_slots += 1;
        }
        if let Some(log) = self.play_log.as_mut() { log.push((ev.tick, ev.sound_class, ev.index, ev.flags, k)); }
        k
    }
}

/// `g_footstep_level_base` (level01 0x1bdca0, one byte per level; the same 19 bytes in every level overlay, at a
/// per-overlay address: checked by `tests/ui/sound_conformance.rs`): the first footstep level def of the level, before
/// the moby-attached defs are added.
pub const FOOTSTEP_LEVEL_BASE: [u8; 19] = [0, 2, 6, 1, 0, 2, 2, 0, 2, 2, 1, 7, 0, 0, 4, 0, 0, 0, 0];
/// `0x15f574`: the level defs played at a moby (`PlayLevelSoundAtMoby` 0x2a1770 takes indices below it; 2 in every
/// overlay); the footsteps follow them.
pub const MOBY_LEVEL_DEFS: usize = 2;

/// The level def `PlayFootstepSound(class, foot, variant, …)` (0x2a1898) plays on `level`: `tbl[level] + class·4 +
/// foot·2 + variant + 0x15f574`. None for a level outside the table.
pub fn footstep_def(level: i32, class: u8, foot: u8, variant: u8) -> Option<usize> {
    let base = *FOOTSTEP_LEVEL_BASE.get(usize::try_from(level).ok()?)? as usize;
    Some(base + class as usize * 4 + foot as usize * 2 + variant as usize + MOBY_LEVEL_DEFS)
}

impl AudioSystem {
    /// `PlayFootstepSound(class, foot, variant, flags, owner)` (0x2a1898): level def [`footstep_def`] (refused when it
    /// is not below the level def count 0x15f5f0), `SoundSlotAlloc(def, flags, owner, 0, 0x400)`; the slot remembers
    /// the def index (+0xe). The owner is Ratchet (privileged). Draws the def's pitch bend when it gets a slot.
    #[allow(clippy::too_many_arguments)]
    pub fn play_footstep(&mut self, level: i32, class: u8, foot: u8, variant: u8, flags: u32, owner: MobyId, pos: [f32; 3], listener: &Listener, rng: &mut Rng, tick: u64) -> i32 {
        let Some(idx) = footstep_def(level, class, foot, variant) else { return -1 };
        let Some(def) = self.data.sounds.level_defs.get(idx).copied() else { return -1 };
        let k = self.slots.play(&def, flags as u8, Some(Owner { id: owner as u32, privileged: true }), Some(pos), None, 0x400, listener, rng);
        if k >= 0 { self.slots.slots[k as usize].class_index = idx as u16; }
        if let Some(log) = self.play_log.as_mut() { log.push((tick, FOOTSTEP_LOG_CLASS, idx as i32, flags, k)); }
        k
    }
}

/// The play log's "class" for a footstep (its index is the level def).
pub const FOOTSTEP_LOG_CLASS: i16 = -1;
/// The play log's "class" for a level sound (`PlayLevelSoundAtMoby`; its index is the level def).
pub const LEVEL_SOUND_LOG_CLASS: i16 = -2;

/// The level defs [`AudioSystem::play_level_sound_at_moby`] plays, by index (the same on every level: the callers in the
/// 19 overlays pass only these two).
pub mod level_sound {
    /// `Help_ComputeSize` 0x225a98: the help box opening (flags 1 = 2-D), when the help text or voice option is on.
    pub const HELP_OPEN: i32 = 0;
    /// The skill point jingle (flags 0): every "skill point earned" site (`if !0x13d408[k] { 0x13d408[k] = 1;
    /// PlayLevelSoundAtMoby(1, 0, 0); ShowBanner(0x53d6, −1) }`: the Blarg flyers' and the gunship's kills on Novalis,
    /// the weapon update `FUN_002d2450`, the classes of the other levels) and the debug cheat entry of `MenuInput`.
    pub const SKILL_POINT: i32 = 1;
}

impl AudioSystem {
    /// `PlayLevelSoundAtMoby(index, flags, moby)` 0x2a1770: level def `index` when it is below 0x15f574 (= 2: the
    /// defs before the footsteps), `SoundSlotAlloc(def, flags, moby, 0, 0x400)`; the slot remembers the index (+0xe) and
    /// the owner (+0x18). Every caller on the disc passes moby 0: no owner and no position, so the sound plays 2-D at
    /// fixed volume at the listener (`SoundSlotAlloc` adds flags 0x11). With a moby (`at` = its id and position +0x10)
    /// the sound follows it like a class sound (privileged for Ratchet `hero`). Level04's `0x27eca0` is the same code (a
    /// second copy in that overlay: the 466 family's skill point; G-AUD-007). Draws the def's pitch bend from `rng`
    /// when it gets a slot (defs 0 and 1 have none on any level, so these plays draw nothing). A negative index (the
    /// game would read before the defs; no caller passes one) is refused.
    #[allow(clippy::too_many_arguments)]
    pub fn play_level_sound_at_moby(&mut self, index: i32, flags: u32, at: Option<(MobyId, [f32; 3])>, hero: Option<MobyId>, listener: &Listener, rng: &mut Rng, tick: u64) -> i32 {
        let Ok(idx) = usize::try_from(index) else { return -1 };
        if idx >= MOBY_LEVEL_DEFS { return -1; }
        let Some(def) = self.data.sounds.level_defs.get(idx).copied() else { return -1 };
        let owner = at.map(|(id, _)| Owner { id: id as u32, privileged: Some(id) == hero });
        let k = self.slots.play(&def, flags as u8, owner, at.map(|(_, p)| p), None, 0x400, listener, rng);
        if k >= 0 { self.slots.slots[k as usize].class_index = idx as u16; }
        if let Some(log) = self.play_log.as_mut() { log.push((tick, LEVEL_SOUND_LOG_CLASS, index, flags, k)); }
        k
    }
}

impl AudioSystem {
    /// Level03's `0x27a618(index, flags, moby)`: level def `index + 0x15f574` (= 2 + index: the defs after the two
    /// [`AudioSystem::play_level_sound_at_moby`] takes, where the footsteps start on the other levels; on Kerwan the
    /// footsteps begin at def 3) when it is below the level def count 0x15f5f0, `SoundSlotAlloc(def, flags, moby, 0,
    /// 0x400)`; the slot remembers the def (+0xe) and the owner (+0x18). Its one caller is the air-traffic wreck
    /// (`classes::units::air_traffic`, index 0, flags 0). A negative index is refused.
    #[allow(clippy::too_many_arguments)]
    pub fn play_level_def_at_moby(&mut self, index: i32, flags: u32, at: Option<(MobyId, [f32; 3])>, hero: Option<MobyId>, listener: &Listener, rng: &mut Rng, tick: u64) -> i32 {
        let Ok(i) = usize::try_from(index) else { return -1 };
        let idx = i + MOBY_LEVEL_DEFS;
        let Some(def) = self.data.sounds.level_defs.get(idx).copied() else { return -1 };
        let owner = at.map(|(id, _)| Owner { id: id as u32, privileged: Some(id) == hero });
        let k = self.slots.play(&def, flags as u8, owner, at.map(|(_, p)| p), None, 0x400, listener, rng);
        if k >= 0 { self.slots.slots[k as usize].class_index = idx as u16; }
        if let Some(log) = self.play_log.as_mut() { log.push((tick, LEVEL_SOUND_LOG_CLASS, idx as i32, flags, k)); }
        k
    }
}

/// The moby loop's [`SoundSink`]: class sounds into `audio` with the listener of the tick (the previous tick's
/// camera).
pub struct ClassSoundSink<'a> {
    pub audio: &'a mut AudioSystem,
    pub listener: Listener,
    /// Ratchet's moby (privileged owner).
    pub hero: Option<MobyId>,
}

impl SoundSink for ClassSoundSink<'_> {
    fn play_class_sound(&mut self, ev: &SoundEvent, rng: &mut Rng) -> i32 { self.audio.play_class_sound(ev, self.hero, &self.listener, rng) }
    /// `SoundIsAlive(moby, slot)`: the slot is in use and still owned by `moby`.
    fn alive(&self, slot: i32, moby: MobyId) -> bool {
        usize::try_from(slot).ok().and_then(|i| self.audio.slots.slots.get(i)).is_some_and(|s| s.state != crate::audio::voices::state::FREE && s.owner.is_some_and(|o| o.id == moby as u32))
    }
    /// `release_voice_slot(slot)` when the slot still plays `moby`'s sound.
    fn release(&mut self, slot: i32, moby: MobyId) {
        if self.alive(slot, moby) { self.audio.slots.release(slot); }
    }
    /// The slot's owner (+0x18) and class-sound index (+0xe), whatever its state (a freed slot has no owner).
    fn slot_owner(&self, slot: i32) -> Option<(MobyId, u16)> {
        let s = self.audio.slots.slots.get(usize::try_from(slot).ok()?)?;
        s.owner.map(|o| (o.id as MobyId, s.class_index))
    }
    /// `PlayLevelSoundAtMoby(index, flags, moby)` ([`AudioSystem::play_level_sound_at_moby`]).
    fn play_level_sound(&mut self, index: i32, flags: u32, at: Option<(MobyId, [f32; 3])>, tick: u64, rng: &mut Rng) -> i32 {
        self.audio.play_level_sound_at_moby(index, flags, at, self.hero, &self.listener, rng, tick)
    }
    fn play_level_def(&mut self, index: i32, flags: u32, at: Option<(MobyId, [f32; 3])>, tick: u64, rng: &mut Rng) -> i32 {
        self.audio.play_level_def_at_moby(index, flags, at, self.hero, &self.listener, rng, tick)
    }
    /// `HeroTeleport`'s env sample point ([`AudioSystem::hero_teleported`]).
    fn hero_teleported(&mut self, pos: [f32; 3]) { self.audio.hero_teleported(pos); }
    /// `MusicRequestTrack(track, stinger)` from a class ([`crate::audio::music::Music::request`]).
    fn music_request(&mut self, track: i16, stinger: i16) { self.audio.music.request(track, stinger); }
    /// `SoundSetPitchBend(slot, pb)`: the slot's +0x14 (no owner test, as the game).
    fn set_pitch_bend(&mut self, slot: i32, pb: i32) {
        if let Some(s) = usize::try_from(slot).ok().and_then(|i| self.audio.slots.slots.get_mut(i)) { s.pb = pb; }
    }
    fn set_volume(&mut self, slot: i32, vol: i32) {
        if let Some(s) = usize::try_from(slot).ok().and_then(|i| self.audio.slots.slots.get_mut(i)) { s.vol = vol; }
    }
    /// The checkpoint record's reverb copy ([`AudioSystem::checkpoint_saved`]).
    fn checkpoint_saved(&mut self) { self.audio.checkpoint_saved(); }
    /// The voice handoff: the slot's owner (+0x18) and position (+0x20) rewritten by a class (no state test, as the
    /// game: a freed slot takes the owner too and ignores it).
    fn hand_over(&mut self, slot: i32, moby: MobyId, pos: [f32; 3]) {
        let hero = self.hero;
        if let Some(s) = usize::try_from(slot).ok().and_then(|i| self.audio.slots.slots.get_mut(i)) {
            s.owner = Some(crate::audio::voices::Owner { id: moby as u32, privileged: Some(moby) == hero });
            s.pos = pos;
        }
    }
}

/// The trigger check at the end of `RatchetAnimAdvance` (0x247d48): when key A and key B were the same
/// sequence before the advance and it has triggers (`+0x7e`), the first trigger word (lo16 class sound, hi16
/// time in 1/16 frames, signed) with `16·frame_a₀ + trunc(16·t₀) < time ≤ 16·frame_a₁ + trunc(16·t₁)` fires
/// (`before` / `after` = the views around the advance). Returns its class sound.
pub fn ratchet_trigger(class: &MobyAnimClass, before: &AnimView, after: &AnimView) -> Option<u16> {
    if before.seq_a != before.seq_b { return None; }
    let seq = class.sequence(after.seq_a)?;
    if seq.header.trigger_count == 0 { return None; }
    let lo = before.frame_a as i32 * 16 + (before.t * 16.0) as i32;
    let hi = after.frame_a as i32 * 16 + (after.t * 16.0) as i32;
    seq.triggers.iter().take(seq.header.trigger_count as usize).find(|&&w| {
        let time = (w as i32) >> 16;
        lo < time && time <= hi
    }).map(|&w| (w & 0xffff) as u16)
}

/// The hero's sounds on the audio layer ([`HeroSounds`]): `audio` gives the sound system when there is one (the
/// callers share it with the moby loop's sink and the sound step, e.g. through a `RefCell`); `class` is
/// Ratchet's class (its sequences' triggers), `listener` the camera the hero update sees (the previous tick's),
/// `hero` his moby, `counter` the tick counter 0x15f5cc.
pub struct HeroClassSounds<'a, F> {
    pub audio: F,
    pub class: &'a MobyAnimClass,
    /// The other bodies' classes by o_class (the hero moby is a body's while one is in: `crate::hero::bodies`; its
    /// advance plays its own class's triggers). None: only Ratchet's.
    pub body_classes: Option<&'a dyn Fn(i16) -> Option<&'a MobyAnimClass>>,
    pub listener: Listener,
    pub hero: MobyId,
    pub counter: u64,
}

impl<F> HeroClassSounds<'_, F> {
    /// `PlayClassSound(index, flags, Ratchet)` on `audio`.
    fn play(&self, audio: &mut AudioSystem, moby: &crate::moby_runtime::Moby, index: i32, flags: u32, rng: &mut Rng) -> i32 {
        let ev = SoundEvent {
            index,
            flags,
            moby: self.hero,
            o_class: moby.o_class,
            sound_class: moby.o_class,
            pos: [moby.position[0], moby.position[1], moby.position[2]],
            tick: self.counter,
        };
        audio.play_class_sound(&ev, Some(self.hero), &self.listener, rng)
    }
}

impl<F, A> HeroSounds for HeroClassSounds<'_, F>
where
    F: FnMut() -> Option<A>,
    A: DerefMut<Target = AudioSystem>,
{
    fn anim_advanced(&mut self, moby: &crate::moby_runtime::Moby, before: &AnimView, after: &AnimView, rng: &mut Rng) {
        // The hero moby's class: Ratchet's (o_class 0), or the body's.
        let class = if moby.o_class == 0 { Some(self.class) } else { self.body_classes.and_then(|f| f(moby.o_class)) };
        let Some(class) = class else { return };
        let Some(idx) = ratchet_trigger(class, before, after) else { return };
        let Some(mut a) = (self.audio)() else { return };
        self.play(&mut a, moby, idx as i32, 0, rng);
    }

    fn voice(&mut self, moby: &crate::moby_runtime::Moby, index: i32, flags: u32, rng: &mut Rng) -> i32 {
        let Some(mut a) = (self.audio)() else { return -1 };
        self.play(&mut a, moby, index, flags, rng)
    }

    fn item_sound(&mut self, o_class: i16, pos: [f32; 3], index: i32, flags: u32, rng: &mut Rng) -> i32 {
        let Some(mut a) = (self.audio)() else { return -1 };
        // The hand item is not in the moby table: the slot's owner is Ratchet's moby (the item hangs in his hand;
        // the sound follows him), privileged like the game's 0x1403e0 owner; the defs are the item class's.
        let ev = SoundEvent { index, flags, moby: self.hero, o_class, sound_class: o_class, pos, tick: self.counter };
        a.play_class_sound(&ev, Some(self.hero), &self.listener, rng)
    }

    fn moby_sound(&mut self, id: MobyId, o_class: i16, pos: [f32; 3], index: i32, flags: u32, rng: &mut Rng) -> i32 {
        let Some(mut a) = (self.audio)() else { return -1 };
        // A moby of the table the hand item created (the R.Y.N.O.'s missiles): its own sound, owned by it.
        let ev = SoundEvent { index, flags, moby: id, o_class, sound_class: o_class, pos, tick: self.counter };
        a.play_class_sound(&ev, Some(id), &self.listener, rng)
    }

    fn footstep(&mut self, moby: &crate::moby_runtime::Moby, level: i32, class: u8, foot: u8, variant: u8, rng: &mut Rng) -> i32 {
        let Some(mut a) = (self.audio)() else { return -1 };
        let pos = [moby.position[0], moby.position[1], moby.position[2]];
        a.play_footstep(level, class, foot, variant, 0, self.hero, pos, &self.listener, rng, self.counter)
    }

    /// `SoundSetPitchBend(slot, pb)` 0x2a1988 on a slot the hand item took (no owner test, as the game).
    fn set_pitch_bend(&mut self, slot: i32, pb: i32) {
        let Some(mut a) = (self.audio)() else { return };
        if let Some(s) = usize::try_from(slot).ok().and_then(|i| a.slots.slots.get_mut(i)) { s.pb = pb; }
    }

    /// `SoundIsAlive(item, slot)` on a slot the hero took (owner: Ratchet's moby, the hand item's stand-in).
    fn alive(&mut self, slot: i32) -> bool {
        let Some(a) = (self.audio)() else { return false };
        usize::try_from(slot).ok().and_then(|i| a.slots.slots.get(i)).is_some_and(|s| s.state != crate::audio::voices::state::FREE && s.owner.is_some_and(|o| o.id == self.hero as u32))
    }

    fn release(&mut self, _moby: &crate::moby_runtime::Moby, slot: i32) {
        let Some(mut a) = (self.audio)() else { return };
        // Only a slot that still plays Ratchet's sound (the game checks the slot's owner and state).
        let ours = usize::try_from(slot).ok().and_then(|i| a.slots.slots.get(i)).is_some_and(|s| s.owner.is_some_and(|o| o.id == self.hero as u32));
        if ours { a.slots.release(slot); }
    }

    fn release_of(&mut self, id: MobyId, slot: i32) {
        let Some(mut a) = (self.audio)() else { return };
        let ours = usize::try_from(slot).ok().and_then(|i| a.slots.slots.get(i)).is_some_and(|s| s.owner.is_some_and(|o| o.id == id as u32));
        if ours { a.slots.release(slot); }
    }
}

/// The tick's sound step (module docs): [`AudioSystem::tick_with`] with the listener = `cam` (this tick's
/// camera), the hero position for the music boxes, the counter 0x15f5cc and the moby table resolving the slot
/// owners. The 800 samples are appended to `out`. (Ratchet's own sounds are played inside the hero update:
/// [`HeroClassSounds`].)
pub fn sound_step(
    audio: &mut AudioSystem,
    table: &MobyTable,
    hero: &Hero,
    cam: &CameraView,
    rng: &mut Rng,
    counter: u64,
    out: &mut Vec<[i16; 2]>,
) {
    let listener = listener_with_water(cam, audio.underwater, hero);
    let input = FrameInput { listener, hero_pos: [hero.pos[0].to_f32(), hero.pos[1].to_f32(), hero.pos[2].to_f32()] };
    audio.tick_with(&input, counter as u32, rng, &|id| owner_position(table, id), out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_formats::moby_anim::{MobySequence, MobySequenceHeader};

    fn class_with(triggers: Vec<u32>) -> MobyAnimClass {
        let header = MobySequenceHeader { frame_count: 20, loop_sound: 0xff, trigger_count: triggers.len() as u8, ..Default::default() };
        let sequences = vec![Some(MobySequence { header, frames: vec![], triggers })];
        MobyAnimClass { joint_count: 0, skeleton: vec![], rest: vec![], parent_word: vec![], sequences }
    }

    fn view(seq_a: u8, frame_a: u8, t: f32) -> AnimView { AnimView { seq_a, seq_b: 0, frame_a, t, ..Default::default() } }

    #[test]
    fn trigger_fires_once_in_its_window() {
        // Class sound 26 at time 8 (frame 0.5), 30 at 160 (frame 10).
        let c = class_with(vec![8 << 16 | 26, 160 << 16 | 30]);
        assert_eq!(ratchet_trigger(&c, &view(0, 0, 0.0), &view(0, 0, 0.5)), Some(26));
        // Exclusive below, inclusive above.
        assert_eq!(ratchet_trigger(&c, &view(0, 0, 0.5), &view(0, 0, 0.9)), None);
        assert_eq!(ratchet_trigger(&c, &view(0, 9, 0.5), &view(0, 10, 0.0)), Some(30));
        // A blend (key A ≠ key B before the advance) never fires; a wrap back to frame 0 neither.
        assert_eq!(ratchet_trigger(&c, &view(0xff, 0, 0.0), &view(0, 0, 0.5)), None);
        assert_eq!(ratchet_trigger(&c, &view(0, 19, 0.5), &view(0, 0, 0.6)), None);
    }
}
