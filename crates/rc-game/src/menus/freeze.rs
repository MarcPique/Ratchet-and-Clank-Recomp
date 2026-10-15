//! Game mode 4, the freeze dialog: `mode_freezeInit(kind, arg)` (L01 0x223640 = boot 0x1fbab8), `UpdateModeFreeze`
//! (L01 0x2249b0 = boot 0x1fce28), `DrawDialogText` (L01 0x2237d8 = boot 0x1fbc50) and the render `FreezeModeRender`
//! 0x21aac8 (the world, or the page menu when opened from it, then the black 0x40 and the dialog). Every kind:
//! 0 "Quit Race?" with the race results (the hoverboard races 5 / 16: Start on the Hoverboard, group 22), 1 the vehicles'
//! and the riders' (levels 8 / 12) "Quit?", 2 a "Continue" notice, 3 the memory-card dialog (its text and buttons follow the card monitor's status
//! 0x15eeb0, [`crate::memcard`]), 4 Giant Clank's "Quit?" (leaves the body), 5 the save notice shown once when auto-save
//! is on, 6 the PAL 60 Hz test. Spec: docs/plan/menus.md §1 "Freeze kinds", docs/plan/progression.md `## saves`.
//!
//! **Callers.** Kinds 3 / 5: the page menu, the card monitor, the mode-0 update ([`save_notice_due`]). Kinds 0 / 1 / 4:
//! `InLevelFrameUpdate` 0x2aba68's Start test ([`crate::menus::mode::in_level_trigger`]); kind 0 also from the races'
//! own code (L05 0x254608 / L16 0x225d70: `mode_freezeInit(0, 0)` when a race ends; the race classes call
//! [`Freeze::init`]`(KIND_RACE, 0, 0)` and fill [`Freeze::ctx`] every frame from their state). Kind 6: the PAL Options'
//! 60 Hz toggle (0x294830 entry flag 1, NTSC never). The caller applies [`FreezeOut`] (the engine's `menu_render`).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x223640 | previous mode 0x1734d4, mode 4, kind 0x1734c0, arg 0x1734d8; not from mode 3: sound group 0x1d and music paused | [`Freeze::init`] (the pause is [`FreezeOut::pause_sounds`]) |
//! | 0x223640 kind 0 | texts 0x1734c8 / cc / d0 = 0x4f6e / 0x5248 / 0x5249, 0x1734e8 = 0, fade 0, step 0x1734dc = 0, 0x1734e0 = 0x1734e4 = 0 | [`Freeze::init`] |
//! | 0x223640 kinds 1 / 4 | texts 0x5229 / 0x4ee0 / 0x524a, fade 0 | [`Freeze::init`] |
//! | 0x223640 kind 2 | texts 0x524a / 0 (0x1734d0 kept), fade 0 | [`Freeze::init`] (the kept 0x1734d0 is never drawn by kind 2: n/a) |
//! | 0x223640 kind 3 | fade 0x1734c4 = ticks(30), counter 0x1734e0 = 0, label fade 0x1734e4 = ticks(30) | ported |
//! | 0x223640 kind 5 | the help log gets 20011 (`fun_00226a70(0x4e2b)`), then as kind 3 | ported ([`FreezeOut::log`]) |
//! | 0x223640 kind 6 | fade ticks(30), step 0 | [`Freeze::init`] |
//! | 0x223640 other kinds | texts 0, fade 0x78 | [`Freeze::init`] (no caller) |
//! | 0x2249b0 entry | `sound_update`; fade −1 per frame | ported (the sound update is the engine's) |
//! | 0x2249b0 kind 0 step 0 | 0x1734e0 + 1 up to 8, then 0x1734e4 + 1 up to 8, then step 1 | [`Freeze::update_with`] |
//! | 0x2249b0 kind 0 step 1 | ✕ → step 2 (quit), else ○ / Start (0x820) → step 3 (go on) | [`Freeze::update_with`] |
//! | 0x2249b0 kind 0 steps 2 / 3 | 0x1734e4 − 1 to 0, then 0x1734e0 − 1 to 0, then: step 2: the race's sound 0x13fbd0 stopped (`FUN_0024b090(h, 0)`, −1), `update_resource_counter` (0x15f8f8 − 1), the hero's state ticks 0x13f4ec > ticks(4200) on level 5: 0x15ee38 + 1, level 16: 0x15ee3c + 1; mode 0; `HeroTeleport(0x141050, 0x141060, 0, 1)`. Step 3: mode 0; race stage 0x13fbea > 2: − 1 and the race moby 0x13fbe0 +0xbc = 3 | [`Freeze::update_with`] ([`FreezeOut::race_quit`], [`FreezeOut::race_rewind`]) |
//! | 0x2249b0 kind 1 | 0x15f608 = 2 (the occlusion shows everything); △ → mode 0 and 0x14095f \|= 1 (the vehicle quits); ✕ → mode 0 | [`Freeze::update_with`] ([`FreezeOut::all_visible`], [`FreezeOut::vehicle_quit`]) |
//! | 0x2249b0 kind 2 | 0x15f608 = 2; ✕ → mode 0 | [`Freeze::update_with`] |
//! | 0x2249b0 kind 3 | counter +1; after ticks(30) the label fade −1; per status 2, 3, 4, 5, 6, 9, 0xc, 0xd, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x17, 0x18: the ✕ / ○ / △ answers (flags 1, 2, 4, 8, 0x10, 0x20, 0x40), the page 0x10 opens, the new game without a card (`load_and_initialize_level_chunk`, level 0, 0x13e05a = 1) | [`Freeze::update`] |
//! | 0x2249b0 kind 4 | △: the hero moby 0x1413d0 +0x31 = 0, +0x94 = 0, +0x34 \|= 1, the body left (`FUN_00231450`), `HeroTeleport(0x141050, 0x141060, 0, 1)`, mode 0; ✕ → mode 0 | [`Freeze::update_with`] ([`FreezeOut::leave_body`]) |
//! | 0x2249b0 kind 5 | counter +1; after ticks(90) the label fade −1; after ticks(120) ✕ → mode 0, on level 1 `memcard_Save(0, −1)` | ported |
//! | 0x2249b0 kind 6 | step 0: the fade − 1 again, at 0 step 1; step 1: △ → the previous mode, ✕ → `FadeToBlack(4)`, 0x16040c = 0, step 2 with 0x1734e0 = ticks(600); step 2: 0x1734e0 − 1, at 0: 0x16040c = 1, step 3; △ → `FadeToBlack(4)`, 0x16040c = 1, previous mode; ✕ → 0x16040c = 0, previous mode; step 3: ✕ → previous mode; other steps → previous mode | [`Freeze::update_with`] ([`FreezeOut::video_mode`], [`FreezeOut::fade_to_black`]) |
//! | 0x2249b0 exit | leaving to a mode other than 3 / 4: sounds and music resumed, `snd_FlushSoundCommands` | ported ([`FreezeOut::resume_sounds`]) |
//! | 0x2237d8 kind 0 | the pulsing frame `fun_001f6060` (colour `FastTweenColor(sin((vsync % *0x15f4e0) / *0x15f4e0 · 2π − π) / 2 + ½, *0x15f4e4, *0x15f4e8)`), scaled by clamp(0x1734e0 / 8, 0.1, 1); the text colours fade in with 0x1734e4 / 8 (*0x15f4ec → *0x15f4f0, the green *0x15f4f4 → *0x15f4f8, the red *0x15f4fc → *0x15f500). Stage 0x13fbea < 3: the frame (185 ± 50, 256 ± 100), 0x4f6e / 0x5249 / 0x5248 centred at y 150, 174, 198. Else the results: the frame (205 ± 120, 256 ± 100), "1st / 2nd / 3rd / Nth " + 0x5240 at y 110 (green for 1st, else red); the best time of the level (0x15ee58 + 4·(level 16)) equal to this race's time 0x13fbe4: 0x50a3 at y 148 and the time "%d:%02d:%02d" (3600 ticks a minute, PAL 3000) at y 168 in green, else 0x50a2 at y 158 in red; the best score (0x15ee68 + …) set and equal to 0x13fbf8: 0x50a5 at y 196 and "%d" at y 216 in green, else 0x50a4 at y 206 in red; then 0x5249 / 0x5248 at y 254 / 278 | [`Freeze::draw`] (`draw_race`; `fast_sin` as `f32::sin` [L]) |
//! | 0x2237d8 kinds 1, 4 (and the default) | the pulsing frame (0x50, 0x9c, 0xb0, 0x150); 0x1734c8 at (256, 0x5a) in 0x8000c0c0, 0x1734cc at (256, 115) and 0x1734d0 at (256, 130) in 0x80ffa888 | [`Freeze::draw`] |
//! | 0x2237d8 kind 2 | the pulsing frame (100, 0xa0, 0xb0, 0x150); 0x1734c8 at (256, 0x7a) in 0x8000c0c0 | [`Freeze::draw`] |
//! | 0x2237d8 kind 3 | the status text (the table below) in a window x 0x60..0x1a0 centred vertically (measure pass, `DrawUIFrame` alpha (1 − fade/30)·80), colour `FastTweenColor(1 − fade/30, *0x15f4b0, *0x15f4b4)`; buttons at x 0xca / 0x135 (two) or 0x100 (one), y = bottom − 0x14, colour `FastTweenColor(1 − label/30, 0x20ffff, 0x8020ffff)` | [`Freeze::draw`] |
//! | 0x2237d8 kind 5 | frame (0x50, 0x154, 0x60, 0x1a0); 20011 split at its first line breaks, the top part from y 84, the bottom part bottom-aligned at 0x136; "✕ Continue" at (0x100, 0x140); the card icon 0x755d frame 0 (64×64 at x 0xe0) between them and its frame 1 turning once per 55 frames | ported |
//! | 0x2237d8 kind 6 | `DrawUIFrame(100, 300, 0x60, 0x1a0, (1 − fade/30)·80)`; window 0x208ac8 (y 100..300, x 96..416, anchor 256, y 104, lines 16, centred); steps 0..2: "✕ Yes" (0x524e) at (0xca, 0x118) and "△ No" (0x524b) at (0x135, 0x118), text 0x522a (steps 0, 1) / 0x522b (step 2); step 3: "✕ Continue" 0x524a at (256, 0x118), text 0x522c; text colour `FastTweenColor(f, *0x15f4b0, *0x15f4b4)`, buttons `FastTweenColor(f, 0x20ffff, 0x8020ffff)` | [`Freeze::draw`] |
//! | 0x1f6060 | the pulsing frame: `draw_rect_overlay(t, b, l, r, alpha \| 0x000004)` (the backing) and eight bevel bars | [`pulse_frame`] (`Draw::Rect16`) |
//! | 0x21aac8 | 0x15f5d8 = 0 only: the world (previous mode 0) or `PageMenuDraw(1)` (previous mode 3), the black 0x40, `DrawDialogText` | the engine (`menu_render::freeze_frame`) |
//! | L00 0x297f78 (mode-0 update) | the save notice: auto-save on, global flag 0x10 clear, ≥ 8 frames in mode 0, never completed, not dying, HP ≠ 0, tick > ticks(30), level ≠ 0 → kind 5, flag 0x10 = 1 | [`save_notice_due`] |
//!
//! Kind 3's texts by status (`fe` = the front end 0x15f5a8; ✕ = 0x10, ○ = 0x11, △ = 0x12 in the strings):
//!
//! | status | text | buttons |
//! |---|---|---|
//! | 2 card changed | 20390 | "✕ Continue" (hidden while fading in) |
//! | 3, 4 | 20396; fe with a save asked: 20395 | "△ Go back?"; fe + save: "✕ Yes" / "△ No" |
//! | 5 | 20400 + 20391 | "△ Go back?" |
//! | 6 | 20399 | "○ Yes" / "△ No" |
//! | 7, 8 | 20407 | — |
//! | 10, 11 | 20417 | — |
//! | 12 | 20391 (a save asked: as 13) | "△ Go back?" |
//! | 13 | 20397 | "○ Yes" / "△ No" |
//! | 14, 15 | 20408 | — |
//! | 17 / 18 / 20 / 21 | 20410 / 20412 / 20411 / 20413 | "✕ Continue" |
//! | 19 | load asked: 20391; else 20393 (fe: + 20394) | "△ Go back?" (fe: "✕ Yes" / "△ No") |
//! | 23 | 20401 | "✕ Yes" / "△ No" |
//! | 24 | 20398 + 20394 | "✕ Yes" / "△ No" |
//! | other | "" (0x15f508) | — |

use super::{scale_ticks, text, tween, MenuAssets, MenuDraw};
use crate::hud::{text as wtext, Draw, Rot};
use crate::memcard::{status, MemCard};
use crate::pad::button;
use rc_formats::font::Font;

/// The kinds (0x1734c0).
pub const KIND_RACE: i32 = 0;
pub const KIND_VEHICLE: i32 = 1;
pub const KIND_CONTINUE: i32 = 2;
pub const KIND_CARD: i32 = 3;
pub const KIND_GIANT: i32 = 4;
pub const KIND_SAVE_NOTICE: i32 = 5;
pub const KIND_VIDEO: i32 = 6;
/// 20011: the save notice.
pub const SAVE_NOTICE_MSG: i32 = 0x4e2b;
/// The card icon of the save notice.
pub const CARD_ICON: u16 = 0x755d;
/// `*0x15f4b0` / `*0x15f4b4`: the dialog text's colour tween ends.
pub const TEXT_FROM: u32 = 0x00ff_a888;
pub const TEXT_TO: u32 = 0x80ff_a888;
/// The buttons' colour tween ends.
pub const LABEL_FROM: u32 = 0x0020_ffff;
pub const LABEL_TO: u32 = 0x8020_ffff;
/// Screen height (`0x13e504`, NTSC).
pub const SCREEN_H: i32 = 416;

/// The overlay's small-data constants of `DrawDialogText` (gp−0x7758..−0x7700; the same values on every level).
pub mod gp {
    /// 0x15f4a8 / 0x15f4ac: the y of the second and third text lines of kinds 1 / 4 (and the frame's bottom − 0x1a).
    pub const LINE2_Y: i32 = 115;
    pub const LINE3_Y: i32 = 130;
    /// 0x15f4b8..0x15f4c8: kind 0's small frame (centre x, centre y, half width, half height) and its first text y.
    pub const SMALL: [i32; 4] = [256, 185, 100, 50];
    pub const SMALL_TEXT_Y: i32 = 150;
    /// 0x15f4cc..0x15f4dc: the results frame and its first text y.
    pub const RESULTS: [i32; 4] = [256, 205, 100, 120];
    pub const RESULTS_TEXT_Y: i32 = 110;
    /// 0x15f4e0: the frames' pulse period; 0x15f4e4 / 0x15f4e8 its colours.
    pub const PULSE_PERIOD: u32 = 180;
    pub const PULSE: [u32; 2] = [0x80e0_8060, 0x80d0_6050];
    /// 0x15f4ec / 0x15f4f0: kind 0's text; 0x15f4f4 / 0x15f4f8 green (a win, a record); 0x15f4fc / 0x15f500 red.
    pub const TEXT: [u32; 2] = [0x00ff_c0c0, 0x80ff_c0c0];
    pub const GREEN: [u32; 2] = [0x0060_ef60, 0x8060_ef60];
    pub const RED: [u32; 2] = [0x0060_60ef, 0x8060_60ef];
}

/// What kind 0 reads of the race (the race classes keep it; zero without a race): the stage 0x13fbea (s16; ≥ 3: the
/// results), the place 0x13fc10, the time 0x13fbe4 (ticks) and the score 0x13fbf8, the race's sound handle 0x13fbd0
/// (−1 none), the best times 0x15ee58 / 0x15ee5c and scores 0x15ee68 / 0x15ee6c (Blackwater City, Kalebo III), the
/// hero's state ticks 0x13f4ec and PAL 0x15ed80.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreezeCtx {
    pub race_stage: i16,
    pub place: i32,
    pub time: i32,
    pub score: i32,
    pub sound: i32,
    pub best_time: [i32; 2],
    pub best_score: [i32; 2],
    pub hero_state_ticks: i32,
    pub level: i32,
    pub pal: bool,
}

impl Default for FreezeCtx {
    fn default() -> Self { FreezeCtx { race_stage: 0, place: 0, time: 0, score: 0, sound: -1, best_time: [0; 2], best_score: [0; 2], hero_state_ticks: 0, level: 0, pal: false } }
}

/// The freeze globals (0x1734c0..0x1734e8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Freeze {
    /// 0x1734c0.
    pub kind: i32,
    /// 0x1734d8: the page kind 3's status 0x10 opens.
    pub arg: u32,
    /// 0x1734d4: the game mode before (`ModeState` raw value).
    pub prev: i32,
    /// 0x1734c4: the fade-in (ticks(30) → 0).
    pub fade: i32,
    /// 0x1734e0: frames since the dialog opened (kind 0: the frame's open count 0..8; kind 6: the test's countdown).
    pub t: i32,
    /// 0x1734e4: the buttons' fade-in (kind 0: the text's fade-in count 0..8).
    pub label: i32,
    /// 0x1734dc: kinds 0 / 6's step.
    pub step: i32,
    /// 0x1734c8 / 0x1734cc / 0x1734d0: the message ids kinds 1, 2, 4 draw (0: none).
    pub texts: [i32; 3],
    /// 0x1734e8 (kind 0 writes 0; no reader found).
    pub e8: i32,
    /// The race / hero values kind 0 reads, set by the caller before every update and draw.
    pub ctx: FreezeCtx,
}

/// Kind 0's quit (step 2's end).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RaceQuit {
    /// `FUN_0024b090(0x13fbd0, 0)` then 0x13fbd0 = −1: the race's sound handle stopped.
    pub stop_sound: Option<i32>,
    /// 0x13f4ec > ticks(4200): the quits counter of the level (0: 0x15ee38 on level 5, 1: 0x15ee3c on level 16).
    pub count: Option<usize>,
}

/// What a freeze frame asks the caller to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FreezeOut {
    /// The dialog closed: the game mode to go to (`0x15f5c4 = …`).
    pub mode: Option<i32>,
    /// Status 0x10: the page menu's target page (`0x1ba178 = arg`).
    pub target: Option<u32>,
    /// `load_and_initialize_level_chunk` + `FUN_002a29a0(0)` + `0x13e05a = 1`: a new game without saving.
    pub new_game: bool,
    /// Kind 5 on level 1: `memcard_Save(0, −1)`.
    pub save: bool,
    /// `mode_freezeInit` not from mode 3: `snd_PauseAllSoundsInGroup(0x1d)`, `music_Pause(0)`.
    pub pause_sounds: bool,
    /// Leaving to a mode other than 3 / 4: `snd_ContinueAllSoundsInGroup(0x1d)`, music resumed.
    pub resume_sounds: bool,
    /// The help log entry to add (kind 5: 20011).
    pub log: Option<i32>,
    /// Kinds 1 / 2: 0x15f608 = 2 this frame (the occlusion shows everything).
    pub all_visible: bool,
    /// Kind 0, ✕ and the frame closed: the race quit, `update_resource_counter` (0x15f8f8 − 1, ≥ 0) and
    /// `HeroTeleport(0x141050, 0x141060, 0, 1)` (the entry pose) with mode 0.
    pub race_quit: Option<RaceQuit>,
    /// Kind 0, ○ / Start and the frame closed with the race stage 0x13fbea > 2: the stage − 1 and the race moby
    /// (0x13fbe0) +0xbc = 3.
    pub race_rewind: bool,
    /// Kind 1 △: 0x14095f |= 1 (the ridden vehicle quits).
    pub vehicle_quit: bool,
    /// Kind 4 △: the hero moby 0x1413d0 hidden (+0x31 = 0), collision off (+0x94 = 0), +0x34 |= 1, the body left
    /// (`FUN_00231450`, `crate::hero::bodies::leave_body`) and `HeroTeleport(0x141050, 0x141060, 0, 1)`.
    pub leave_body: bool,
    /// Kind 6: `FadeToBlack(4)`.
    pub fade_to_black: Option<i32>,
    /// Kind 6: 0x16040c (the PAL 60 Hz switch: 0 on, 1 off).
    pub video_mode: Option<u8>,
}

impl Freeze {
    /// `mode_freezeInit(kind, arg)` from game mode `prev`.
    pub fn init(kind: i32, arg: u32, prev: i32) -> (Freeze, FreezeOut) {
        let mut out = FreezeOut { pause_sounds: prev != 3, ..Default::default() };
        let mut f = Freeze { kind, arg, prev, ..Default::default() };
        match kind {
            KIND_RACE => {
                f.texts = [0x4f6e, 0x5248, 0x5249];
                f.e8 = 0;
                f.fade = 0;
                f.step = 0;
                f.t = 0;
                f.label = 0;
            }
            KIND_VEHICLE | KIND_GIANT => {
                f.texts = [0x5229, 0x4ee0, 0x524a];
                f.fade = 0;
            }
            KIND_CONTINUE => {
                f.texts[0] = 0x524a;
                f.texts[1] = 0;
                f.fade = 0;
            }
            KIND_CARD | KIND_SAVE_NOTICE => {
                if kind == KIND_SAVE_NOTICE { out.log = Some(SAVE_NOTICE_MSG); }
                f.fade = scale_ticks(0x1e);
                f.t = 0;
                f.label = scale_ticks(0x1e);
            }
            KIND_VIDEO => {
                f.fade = scale_ticks(0x1e);
                f.step = 0;
            }
            _ => {
                f.texts = [0; 3];
                f.fade = 0x78;
            }
        }
        (f, out)
    }

    /// One frame of `UpdateModeFreeze`. `pressed` = 0x13cae4; `level` = 0x15ed84; kind 0 reads [`Freeze::ctx`].
    pub fn update(&mut self, pressed: u32, card: &mut MemCard, level: i32) -> FreezeOut {
        let mut out = FreezeOut::default();
        if self.fade != 0 { self.fade -= 1; }
        let (cross, circle, tri) = (pressed & button::CROSS != 0, pressed & button::CIRCLE != 0, pressed & button::TRIANGLE != 0);
        let fe = card.front_end;
        let prev = self.prev;
        match self.kind {
            KIND_RACE => self.update_race(pressed, level, &mut out),
            KIND_VEHICLE | KIND_GIANT => {
                if self.kind == KIND_VEHICLE { out.all_visible = true; }
                if tri {
                    out.mode = Some(0);
                    if self.kind == KIND_VEHICLE { out.vehicle_quit = true } else { out.leave_body = true }
                } else if cross {
                    out.mode = Some(0);
                }
            }
            KIND_CONTINUE => {
                out.all_visible = true;
                if cross { out.mode = Some(0); }
            }
            KIND_VIDEO => self.update_video(cross, tri, &mut out),
            KIND_CARD => {
                self.t += 1;
                if scale_ticks(0x1e) < self.t && self.label != 0 { self.label -= 1; }
                let f = &mut card.flags;
                match card.status {
                    status::CHANGED => {
                        if self.fade == 0 && cross {
                            *f &= !1;
                            out.mode = Some(prev);
                        }
                    }
                    status::POLL => {
                        if fe && *f & 2 != 0 {
                            if cross {
                                out.new_game = true;
                                *f &= !6;
                            }
                            if tri {
                                *f |= 0x20;
                                out.mode = Some(prev);
                                *f &= !6;
                            }
                        } else if !fe {
                            if tri {
                                *f |= 0x20;
                                out.mode = Some(prev);
                                *f &= !6;
                            }
                        } else if *f & 4 == 0 {
                            out.mode = Some(prev);
                        } else if tri {
                            *f = (*f | 0x20) & !6;
                            out.mode = Some(prev);
                        }
                    }
                    status::FORMAT_ASK | status::CREATE_ASK => {
                        if circle {
                            *f |= if card.status == status::FORMAT_ASK { 8 } else { 0x10 };
                        } else if tri {
                            *f = (*f & !6) | 0x20;
                            if !fe { out.mode = Some(prev); }
                        }
                    }
                    status::PRESENT => {
                        if fe && *f & 6 == 0 { out.mode = Some(prev); }
                    }
                    status::NO_DATA | status::RECHECK | status::UNFORMATTED => {
                        let skip = card.status == status::NO_DATA && *f & 2 != 0;
                        if !skip && tri {
                            *f &= !6;
                            out.mode = Some(prev);
                        }
                    }
                    status::FOUND => {
                        out.target = Some(self.arg);
                        out.mode = Some(prev);
                    }
                    status::FORMAT_FAILED | status::CREATE_FAILED | status::LOAD_FAILED | status::SAVE_FAILED => {
                        if cross {
                            *f ^= 0x40;
                            out.mode = Some(prev);
                            *f &= !6;
                        }
                    }
                    status::NO_SPACE => {
                        if fe && *f & 2 != 0 {
                            if cross {
                                out.new_game = true;
                                *f = (*f & !6) | 0x20;
                            } else if tri {
                                *f &= !6;
                                out.mode = Some(prev);
                            }
                        } else if tri {
                            *f &= !6;
                            out.mode = Some(prev);
                        }
                    }
                    status::UNFORMATTED_FE | status::NO_DATA_FE => {
                        if cross {
                            out.new_game = true;
                            *f = (*f & !6) | 0x20;
                        } else if tri {
                            *f |= 0x20;
                            out.mode = Some(prev);
                        }
                    }
                    _ => {}
                }
            }
            KIND_SAVE_NOTICE => {
                self.t += 1;
                if scale_ticks(0x5a) < self.t && self.label != 0 { self.label -= 1; }
                if scale_ticks(0x78) < self.t && cross {
                    out.mode = Some(0);
                    if level == 1 { out.save = true; }
                }
            }
            _ => {}
        }
        if out.new_game { out.mode = out.mode.or(Some(prev)); }
        if let Some(m) = out.mode {
            if (m - 3) as u32 > 1 { out.resume_sounds = true; }
        }
        out
    }

    /// Kind 0 (`UpdateModeFreeze` case 0): the frame opens over 8 + 8 frames, waits for ✕ (quit) or ○ / Start (go on),
    /// closes over as many and then acts.
    fn update_race(&mut self, pressed: u32, level: i32, out: &mut FreezeOut) {
        match self.step {
            0 => {
                if self.t < 8 {
                    self.t += 1;
                } else if self.label < 8 {
                    self.label += 1;
                } else {
                    self.step = 1;
                }
            }
            1 => {
                if pressed & button::CROSS != 0 {
                    self.step = 2;
                } else if pressed & (button::CIRCLE | button::START) != 0 {
                    self.step = 3;
                }
            }
            2 | 3 => {
                if self.label != 0 {
                    self.label -= 1;
                } else if self.t != 0 {
                    self.t -= 1;
                } else if self.step == 2 {
                    let c = &self.ctx;
                    let count = match level {
                        5 if c.hero_state_ticks > scale_ticks(0x1068) => Some(0),
                        0x10 if c.hero_state_ticks > scale_ticks(0x1068) => Some(1),
                        _ => None,
                    };
                    out.race_quit = Some(RaceQuit { stop_sound: (c.sound != -1).then_some(c.sound), count });
                    if c.sound != -1 { self.ctx.sound = -1; }
                    out.mode = Some(0);
                } else {
                    out.mode = Some(0);
                    if self.ctx.race_stage > 2 {
                        self.ctx.race_stage -= 1;
                        out.race_rewind = true;
                    }
                }
            }
            _ => {}
        }
    }

    /// Kind 6 (`UpdateModeFreeze` case 6): the PAL 60 Hz test.
    fn update_video(&mut self, cross: bool, tri: bool, out: &mut FreezeOut) {
        let prev = self.prev;
        match self.step {
            0 => {
                // The fade counts down a second time in this step.
                if self.fade == 0 {
                    self.step = 1;
                } else {
                    self.fade -= 1;
                    if self.fade == 0 { self.step = 1; }
                }
            }
            1 => {
                if tri {
                    out.mode = Some(prev);
                } else if cross {
                    out.fade_to_black = Some(4);
                    out.video_mode = Some(0);
                    self.step = 2;
                    self.t = scale_ticks(600);
                }
            }
            2 => {
                let expired = if self.t == 0 {
                    true
                } else {
                    self.t -= 1;
                    self.t == 0
                };
                if expired {
                    out.video_mode = Some(1);
                    self.step = 3;
                } else if tri {
                    out.fade_to_black = Some(4);
                    out.video_mode = Some(1);
                    out.mode = Some(prev);
                } else if cross {
                    out.video_mode = Some(0);
                    out.mode = Some(prev);
                }
            }
            3 => {
                if cross { out.mode = Some(prev); }
            }
            _ => out.mode = Some(prev),
        }
    }

    /// `DrawDialogText`, after the black at alpha 0x40. `vsync` = 0x15f3f8.
    pub fn draw(&self, a: &MenuAssets, card: &MemCard, vsync: u32, out: &mut Vec<MenuDraw>) {
        match self.kind {
            KIND_RACE => self.draw_race(a, vsync, out),
            KIND_CONTINUE => {
                pulse_frame(100, 0xa0, 0xb0, 0x150, pulse(vsync), out);
                if self.texts[0] != 0 { centred(a, out, 0x100, 0x7a, 0x8000_c0c0, a.msg(self.texts[0])); }
            }
            KIND_CARD => self.draw_card(a, card, out),
            KIND_SAVE_NOTICE => self.draw_notice(a, vsync, out),
            KIND_VIDEO => self.draw_video(a, out),
            _ => {
                pulse_frame(0x50, gp::LINE3_Y + 0x1a, 0xb0, 0x150, pulse(vsync), out);
                if self.texts[0] != 0 { centred(a, out, 0x100, 0x5a, 0x8000_c0c0, a.msg(self.texts[0])); }
                if self.texts[1] != 0 { centred(a, out, 0x100, gp::LINE2_Y, 0x80ff_a888, a.msg(self.texts[1])); }
                if self.texts[2] != 0 { centred(a, out, 0x100, gp::LINE3_Y, 0x80ff_a888, a.msg(self.texts[2])); }
            }
        }
    }

    /// Kind 0's draw: the small "Quit Race?" frame, or the results once the race stage is ≥ 3.
    fn draw_race(&self, a: &MenuAssets, vsync: u32, out: &mut Vec<MenuDraw>) {
        let frame_col = pulse(vsync);
        let s = (self.t as f32 * 0.125).clamp(0.1, 1.0);
        let (mut text, mut green, mut red) = (gp::TEXT[0], gp::GREEN[0], gp::RED[0]);
        if self.label != 0 {
            let f = (self.label as f32 * 0.125).clamp(0.0, 1.0);
            text = tween(f, gp::TEXT[0], gp::TEXT[1]);
            green = tween(f, gp::GREEN[0], gp::GREEN[1]);
            red = tween(f, gp::RED[0], gp::RED[1]);
        }
        let c = &self.ctx;
        let sized = |v: i32| (v as f32 * s) as i32;
        if c.race_stage < 3 {
            let [cx, cy, hw, hh] = gp::SMALL;
            pulse_frame(cy - sized(hh), cy + sized(hh), cx - sized(hw), cx + sized(hw), frame_col, out);
            let y = gp::SMALL_TEXT_Y;
            centred(a, out, 0x100, y, text, a.msg(0x4f6e));
            centred(a, out, 0x100, y + 0x18, text, a.msg(0x5249));
            centred(a, out, 0x100, y + 0x30, text, a.msg(0x5248));
            return;
        }
        let [cx, cy, hw, hh] = gp::RESULTS;
        pulse_frame(cy - sized(hh), cy + sized(hh), cx - sized(hw), cx + sized(hw), frame_col, out);
        let y = gp::RESULTS_TEXT_Y;
        // sprintf(buf, "    %s", msg(0x5240)), then the place's ordinal over its first 4 bytes and buf[3] = ' '.
        let mut buf = b"    ".to_vec();
        buf.extend_from_slice(a.msg(0x5240));
        let suffix: &[u8] = match c.place {
            1 => b"st",
            2 => b"nd",
            3 => b"rd",
            _ => b"th",
        };
        let mut head = c.place.to_string().into_bytes();
        head.extend_from_slice(suffix);
        head.push(0);
        for (i, &b) in head.iter().enumerate() {
            if let Some(d) = buf.get_mut(i) { *d = b; } else { buf.push(b); }
        }
        if let Some(d) = buf.get_mut(3) { *d = b' '; }
        if let Some(z) = buf.iter().position(|&b| b == 0) { buf.truncate(z); }
        centred(a, out, 0x100, y, if c.place == 1 { green } else { red }, &buf);
        let k = (c.level == 0x10) as usize;
        if c.best_time[k] == c.time {
            let minute = if c.pal { 3000 } else { 0xe10 };
            let second = minute / 60;
            let rem = c.time - (c.time / minute) * minute;
            let secs = rem / second;
            centred(a, out, 0x100, y + 0x26, green, a.msg(0x50a3));
            let t = format!("{}:{:02}:{:02}", c.time / minute, secs, ((rem - secs * second) * 100) / second);
            centred(a, out, 0x100, y + 0x3a, green, t.as_bytes());
        } else {
            centred(a, out, 0x100, y + 0x30, red, a.msg(0x50a2));
        }
        if c.best_score[k] == 0 || c.best_score[k] != c.score {
            centred(a, out, 0x100, y + 0x60, red, a.msg(0x50a4));
        } else {
            centred(a, out, 0x100, y + 0x56, green, a.msg(0x50a5));
            centred(a, out, 0x100, y + 0x6a, green, c.score.to_string().as_bytes());
        }
        centred(a, out, 0x100, y + 0x90, text, a.msg(0x5249));
        centred(a, out, 0x100, y + 0xa8, text, a.msg(0x5248));
    }

    /// Kind 6's draw.
    fn draw_video(&self, a: &MenuAssets, out: &mut Vec<MenuDraw>) {
        let f = 1.0 - self.fade as f32 / scale_ticks(0x1e) as f32;
        out.push(MenuDraw::Hud(Draw::UiFrame { top: 100, bottom: 300, left: 0x60, right: 0x1a0, alpha: (f * 80.0) as i32 }));
        let col = tween(f, TEXT_FROM, TEXT_TO);
        let lc = tween(f, LABEL_FROM, LABEL_TO);
        let body = match self.step {
            0..=2 => {
                centred(a, out, 0xca, 0x118, lc, a.msg(0x524e));
                centred(a, out, 0x135, 0x118, lc, a.msg(0x524b));
                if self.step == 2 { 0x522b } else { 0x522a }
            }
            3 => {
                centred(a, out, 0x100, 0x118, lc, a.msg(0x524a));
                0x522c
            }
            _ => 0,
        };
        if body != 0 {
            // DAT_00208ac8: y 100..300, x 96..416, anchor 256, y 104, lines 16, centred lines.
            let win = wtext::Window::new(100, 300, 96, 416, 256, 104, 16, wtext::CENTRE_LINES);
            out.push(MenuDraw::Hud(Draw::TextWindow { font: Font::Regular, window: win, rgba: col, text: a.msg(body).to_vec() }));
        }
    }

    fn fades(&self) -> (f32, f32) {
        let n = scale_ticks(0x1e) as f32;
        (1.0 - self.fade as f32 / n, 1.0 - self.label as f32 / n)
    }

    fn draw_card(&self, a: &MenuAssets, card: &MemCard, out: &mut Vec<MenuDraw>) {
        let msg = |id: i32| a.msg(id).to_vec();
        let join = |x: i32, y: i32| {
            let mut v = msg(x);
            v.extend_from_slice(&[1, 1]);
            v.extend_from_slice(&msg(y));
            v
        };
        let (fe, f) = (card.front_end, card.flags);
        // (text, centre button, left button, right button)
        let (body, centre, left, right): (Vec<u8>, i32, i32, i32) = match card.status {
            status::CHANGED => (msg(0x4fa6), if self.fade != 0 { 0 } else { 0x524a }, 0, 0),
            status::RECHECK | status::POLL => {
                if fe && f & 2 != 0 { (msg(0x4fab), 0, 0x524e, 0x524b) } else { (msg(0x4fac), 0x4fa8, 0, 0) }
            }
            status::UNFORMATTED => (join(0x4fb0, 0x4fa7), 0x4fa8, 0, 0),
            status::FORMAT_ASK => (msg(0x4faf), 0, 0x524f, 0x524b),
            status::FORMAT | status::FORMATTING => (msg(0x4fb7), 0, 0, 0),
            status::CHECK | status::CHECKING => (msg(0x4fc1), 0, 0, 0),
            status::NO_DATA if f & 2 == 0 => (msg(0x4fa7), 0x4fa8, 0, 0),
            status::NO_DATA | status::CREATE_ASK => (msg(0x4fad), 0, 0x524f, 0x524b),
            status::CREATE | status::CREATING => (msg(0x4fb8), 0, 0, 0),
            status::FORMAT_FAILED => (msg(0x4fba), 0x524a, 0, 0),
            status::CREATE_FAILED => (msg(0x4fbc), 0x524a, 0, 0),
            status::NO_SPACE => {
                if f & 4 != 0 {
                    (msg(0x4fa7), 0x4fa8, 0, 0)
                } else if fe {
                    (join(0x4fa9, 0x4faa), 0, 0x524e, 0x524b)
                } else {
                    (msg(0x4fa9), 0x4fa8, 0, 0)
                }
            }
            status::LOAD_FAILED => (msg(0x4fbb), 0x524a, 0, 0),
            status::SAVE_FAILED => (msg(0x4fbd), 0x524a, 0, 0),
            status::UNFORMATTED_FE => (msg(0x4fb1), 0, 0x524e, 0x524b),
            status::NO_DATA_FE => (join(0x4fae, 0x4faa), 0, 0x524e, 0x524b),
            // 0x15f508: an empty string.
            _ => (Vec::new(), 0, 0, 0),
        };
        let glyphs = &a.hud.glyphs[Font::Regular as usize];
        let mut win = wtext::Window::new(0, SCREEN_H as i16, 0x60, 0x1a0, 0x100, 0x68, 0x10, wtext::CENTRE_LINES | wtext::MEASURE_ONLY);
        wtext::layout(&mut win, &body, -1, glyphs, true);
        let h = win.height as i32;
        let y0 = (SCREEN_H - (h + 0x28)) >> 1;
        win.y_min = y0 as i16;
        win.y_max = (y0 + h + 0x28) as i16;
        win.y_start = (y0 + 4) as i16;
        win.flags ^= wtext::MEASURE_ONLY;
        let (ft, fl) = self.fades();
        out.push(MenuDraw::Hud(Draw::UiFrame { top: y0, bottom: y0 + h + 0x28, left: 0x60, right: 0x1a0, alpha: (ft * 80.0) as i32 }));
        out.push(MenuDraw::Hud(Draw::TextWindow { font: Font::Regular, window: win, rgba: tween(ft, TEXT_FROM, TEXT_TO), text: body }));
        let lc = tween(fl, LABEL_FROM, LABEL_TO);
        let y = win.y_max as i32 - 0x14;
        if left != 0 { centred(a, out, 0xca, y, lc, a.msg(left)); }
        if right != 0 { centred(a, out, 0x135, y, lc, a.msg(right)); }
        if centre != 0 { centred(a, out, 0x100, y, lc, a.msg(centre)); }
    }

    fn draw_notice(&self, a: &MenuAssets, vsync: u32, out: &mut Vec<MenuDraw>) {
        let n = scale_ticks(0x1e) as f32;
        let ft = 1.0 - self.fade as f32 / n;
        out.push(MenuDraw::Hud(Draw::UiFrame { top: 0x50, bottom: 0x154, left: 0x60, right: 0x1a0, alpha: (ft * 80.0) as i32 }));
        let col = tween(ft, TEXT_FROM, TEXT_TO);
        // The message split at its first line breaks (every 0x01 of the run becomes the end of the first part).
        let m = a.msg(SAVE_NOTICE_MSG);
        let mut i = 0;
        while i < m.len() && m[i] > 1 { i += 1; }
        let first = m[..i].to_vec();
        let mut j = i;
        while j < m.len() && m[j] == 1 { j += 1; }
        let second = m[j..].to_vec();
        let glyphs = &a.hud.glyphs[Font::Regular as usize];
        // DAT_00208ab0: y 80..340, x 96..416, anchor 256, y 84, lines 16, centred lines.
        let mut win = wtext::Window::new(80, 340, 96, 416, 256, 84, 16, wtext::CENTRE_LINES);
        let mut m1 = win;
        wtext::layout(&mut m1, &first, -1, glyphs, true);
        out.push(MenuDraw::Hud(Draw::TextWindow { font: Font::Regular, window: win, rgba: col, text: first }));
        let h1 = m1.height as i32;
        let y1 = win.y_start as i32;
        let mut m2 = win;
        m2.flags |= wtext::MEASURE_ONLY;
        wtext::layout(&mut m2, &second, -1, glyphs, true);
        win.y_start = (0x136 - m2.height as i32) as i16;
        out.push(MenuDraw::Hud(Draw::TextWindow { font: Font::Regular, window: win, rgba: col, text: second }));
        let lc = tween(1.0 - self.label as f32 / n, LABEL_FROM, LABEL_TO);
        centred(a, out, 0x100, 0x140, lc, a.msg(0x524a));
        let cy = (y1 + h1 + win.y_start as i32) >> 1;
        let icon = a.frame(CARD_ICON, 0);
        out.push(MenuDraw::Hud(Draw::Sprite { frame: icon, x: 0xe0, y: cy - 0x20, w: 0x40, h: 0x40, alpha: 0x80, rot: Rot::None }));
        let spin = a.frame(CARD_ICON, 1);
        let angle = ((vsync % 55) as f32 * -std::f32::consts::TAU) / 55.0;
        out.push(super::pause::map_page::rotated(spin, 256.0, cy as f32, 17.0, 17.0, angle, 0x40, 0x40));
    }
}

/// The frames' pulse: `FastTweenColor(sin((vsync % *0x15f4e0) / *0x15f4e0 · 6.28318 − 3.14159)·0.5 + 0.5, *0x15f4e4,
/// *0x15f4e8)` (`fast_sin` as `f32::sin` [L]; the game's own constants 6.28318 / 3.14159).
#[allow(clippy::approx_constant)]
fn pulse(vsync: u32) -> u32 {
    let p = gp::PULSE_PERIOD;
    let s = (((vsync % p) as f32 / p as f32) * 6.28318 - 3.14159).sin();
    tween(s * 0.5 + 0.5, gp::PULSE[0], gp::PULSE[1])
}

/// `fun_001f6060(top, bottom, left, right, rgba)`: the backing `draw_rect_overlay(t, b, l, r, alpha | 4)` and eight
/// bevel bars in `rgba` (each `draw_rect_overlay(top, bottom, left, right, c)`: a blended quad at `16·v − 8`, here
/// `Draw::Rect16`).
pub fn pulse_frame(t: i32, b: i32, l: i32, r: i32, rgba: u32, out: &mut Vec<MenuDraw>) {
    let rect = |top: i32, bottom: i32, left: i32, right: i32, c: u32| MenuDraw::Hud(Draw::Rect16 { x0: 16 * left, y0: 16 * top, x1: 16 * right, y1: 16 * bottom, rgba: c });
    out.push(rect(t, b, l, r, (rgba & 0xff00_0000) | 4));
    for (top, bottom, left, right) in [
        (t - 1, t + 1, l + 3, r + 5),
        (t - 3, t - 5, l - 1, r - 3),
        (t - 5, b - 3, l - 1, l + 1),
        (t + 3, b + 1, l - 3, l - 5),
        (b - 1, b + 1, l - 3, r - 3),
        (b + 3, b + 5, l + 3, r + 1),
        (t + 3, b + 3, r - 1, r + 1),
        (t + 1, b - 3, r + 3, r + 5),
    ] {
        out.push(rect(top, bottom, left, right, rgba));
    }
}

/// `font_print_center(x, y, rgba, text)` in the regular font.
fn centred(a: &MenuAssets, out: &mut Vec<MenuDraw>, x: i32, y: i32, rgba: u32, t: &[u8]) {
    let w = a.width(Font::Regular, t);
    text(out, Font::Regular, x - w / 2, y, rgba, t);
}

/// The inputs of the save notice's trigger (L00 0x297f78, the mode-0 update after the pause triggers).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoticeGate {
    /// 0x15f5c8: frames in mode 0.
    pub frames_in_mode: i32,
    /// 0x141401: the death flag.
    pub dying: bool,
    pub hp: i32,
    /// 0x15f5cc.
    pub tick: u64,
}

/// Whether the mode-0 update opens the save notice (kind 5) now: auto-save on (0x13d384), global flag 0x10 (0x13d398)
/// clear, ≥ 8 frames in mode 0, times completed 0, not dying, HP ≠ 0, tick > ticks(30), level ≠ 0. The caller then sets
/// flag 0x10 and calls [`Freeze::init`]`(5, 0, 0)`.
pub fn save_notice_due(card: &MemCard, gs: &crate::game_state::GameState, g: &NoticeGate) -> bool {
    card.checked
        && gs.global.flags[0x10] == 0
        && g.frames_in_mode > 7
        && gs.global.completes == 0
        && !g.dying
        && g.hp != 0
        && (scale_ticks(0x1e) as u64) < g.tick
        && gs.global.level != 0
}
