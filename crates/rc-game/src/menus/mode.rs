//! The game mode `0x15f5c4` and the main loop's per-mode bookkeeping (level01 `entry` 0x259c40; menus.md §1).
//!
//! Every frame the main loop runs `UpdatePad` (0x27c478) and then the current mode's update and render
//! (mode 1 alone skips the pad). After them `0x15f5c8` counts the frames spent in the mode: +1 when the mode
//! is unchanged, 0 after a change. Only modes 0 and 2 get the catch-up tick (a second `UpdatePad` + update
//! when the frame took longer than one field).
//!
//! **Game tick** [H]: `0x15f5cc` and the play time `0x15eea4` advance only in `FUN_002ab960`, called by the
//! updates of modes 0 (0x2aba68), 2 (0x2aca80), 6 (0x2a4080, substates 0/3/8; 4 through 0x2a33b0) and 5
//! (0x2b03b8, substates 0/2/3). Mode 3 (page menu), 4 (freeze dialog), 1, 7 and −1 never call it.

/// `0x15f5c4`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// 0: gameplay (update 0x2aba68, render 0x21aa30).
    #[default]
    Gameplay,
    /// 1: PSS movie (0x2ad498).
    Movie,
    /// 2: in-engine cutscene (0x2aca80).
    Cutscene,
    /// 3: page menu (0x28c990 / 0x21ab50 → 0x28d080(0)).
    Menu,
    /// 4: freeze dialog (`UpdateModeFreeze` 0x2249b0 / 0x21aac8).
    Freeze,
    /// 5: Gadgetron vendor (0x2b03b8 / 0x2b4020).
    Vendor,
    /// 6: ship / space (0x2a4080).
    Ship,
    /// 7: slideshow (0x2ad738 / 0x21ab78).
    Slideshow,
    /// −1: debug camera / menu (0x216198 / 0x2174e0).
    Debug,
}

impl Mode {
    pub fn raw(self) -> i32 {
        match self {
            Mode::Gameplay => 0,
            Mode::Movie => 1,
            Mode::Cutscene => 2,
            Mode::Menu => 3,
            Mode::Freeze => 4,
            Mode::Vendor => 5,
            Mode::Ship => 6,
            Mode::Slideshow => 7,
            Mode::Debug => -1,
        }
    }

    pub fn from_raw(v: i32) -> Option<Mode> {
        Some(match v {
            0 => Mode::Gameplay,
            1 => Mode::Movie,
            2 => Mode::Cutscene,
            3 => Mode::Menu,
            4 => Mode::Freeze,
            5 => Mode::Vendor,
            6 => Mode::Ship,
            7 => Mode::Slideshow,
            -1 => Mode::Debug,
            _ => return None,
        })
    }

    /// Does this mode's update run the game tick `FUN_002ab960`? Modes 5 and 6 do it only in some substates
    /// (see the module docs); they are not ported and answer `false` here.
    pub fn advances_tick(self) -> bool { matches!(self, Mode::Gameplay | Mode::Cutscene) }

    /// Does the main loop run `UpdatePad` before the update? (All but the movie player.)
    pub fn updates_pad(self) -> bool { self != Mode::Movie }

    /// Modes 0 and 2 may run a second (catch-up) tick after a slow frame.
    pub fn catches_up(self) -> bool { matches!(self, Mode::Gameplay | Mode::Cutscene) }
}

/// The mode globals the main loop keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModeState {
    /// 0x15f5c4.
    pub mode: Mode,
    /// 0x15f5c8: frames in the current mode.
    pub frames_in_mode: i32,
    /// 0x15f5d8: skip rendering once (set by every transition).
    pub skip_render: bool,
    /// The mode at the start of the frame (the main loop's `iVar5`).
    start: Mode,
}

impl ModeState {
    /// Called before the mode's update.
    pub fn begin_frame(&mut self) { self.start = self.mode; }

    /// A mode entry function writing `0x15f5c4`.
    pub fn set(&mut self, m: Mode) { self.mode = m; }

    /// After the update and render: `0x15f5c8 = unchanged ? +1 : 0`.
    pub fn end_frame(&mut self) {
        if self.mode == self.start { self.frames_in_mode += 1 } else { self.frames_in_mode = 0 }
    }
}

/// What `InLevelFrameUpdate` 0x2aba68's pause tests read (mode 0's frame, before the tick).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TriggerIn {
    /// 0x15f5c4 (raw) and 0x15f5c8.
    pub mode: i32,
    pub frames_in_mode: i32,
    /// 0x15ed84.
    pub level: i32,
    /// 0x13cae4 (pressed, after the lock) and 0x13cadc ≠ 0 (the pad is connected).
    pub pressed: u32,
    pub connected: bool,
    /// 0x1413d4 hero state, 0x1413dc movement group, 0x1413f4 body (2 Giant Clank), 0x1403fc the hand-swap state,
    /// 0x141401 fell out, 0x1415f8 HP.
    pub state: i32,
    pub group: i32,
    pub body: u8,
    pub swap_state: u8,
    pub fell: bool,
    pub hp: i32,
    /// 0x140940's class (+0xa6): the moby the hero rides (None: none).
    pub riding_class: Option<i16>,
    /// 0x167280 +0x86: the current camera's script lock (0x13: the triggers wait).
    pub camera_lock: i16,
    /// 0x16c4e0 & 0x10 (the debug step mode) and 0x16c5c4 (its countdown).
    pub debug_step: bool,
    pub c5c4: i32,
}

/// What the tests start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// `mode_freezeInit(kind, 0)`: 0 the Hoverboard's "Quit Race?" (group 22), 1 the vehicles' and the riders' "Quit?"
    /// (levels 8 / 12), 4 Giant Clank's (level 15).
    Freeze(i32),
    /// `EnterMenuMode(kind)`: 0 the pause menu (Start or the pad lost), 10 the map (Select / R3).
    Menu(i32),
}

/// The vehicles whose rider (group 9, state 0x32) gets kind 1 (0x140940 +0xa6).
pub const VEHICLE_CLASSES: [i16; 3] = [0x45, 0x563, 0x4da];

/// `InLevelFrameUpdate` 0x2aba68's Start / Select tests, in its order (the card dialog, the return page and the save
/// notice are the caller's): `go` = Start pressed or the pad lost.
/// * level 15, ≥ 8 frames, Giant Clank (0x1413f4 = 2), go, state ≠ 0x72 → kind 4 (nothing while the camera lock is 0x13);
/// * group 9, state 0x32 riding a vehicle ([`VEHICLE_CLASSES`]), ≥ 8 frames, go, state ≠ 0x72, camera lock ≠ 0x13 → kind 1;
/// * mode 0, level 8 or 12, go, state 0x32 → kind 1 (0x2abcc8);
/// * go, ≥ 8 frames, state ≠ 0x72, group 22 (the Hoverboard) → kind 0, "Quit Race?" (0x2abd14);
/// * go, ≥ 8 frames, state ≠ 0x72, swap state ≠ 2, state ∉ {0x32, 0x1d}, not fallen, HP ≠ 0, mode 0 → the pause menu
///   (kind 0);
/// * not the debug step, ≥ 8 frames, Select / R3 (0x500), group ≠ 22, state ∉ {0x72, 0x32, 0x1d}, not fallen, HP ≠ 0,
///   mode 0, not riding a vehicle, 0x16c5c4 = 0 → the map (kind 10).
pub fn in_level_trigger(t: &TriggerIn) -> Option<Trigger> {
    use crate::pad::button;
    let go = t.pressed & button::START != 0 || !t.connected;
    let settled = t.frames_in_mode >= 8;
    if t.level == 0xf && settled && t.body == 2 && go && t.state != 0x72 {
        return if t.camera_lock == 0x13 { None } else { Some(Trigger::Freeze(4)) };
    }
    let riding = t.group == 9 && t.state == 0x32 && t.riding_class.is_some_and(|c| VEHICLE_CLASSES.contains(&c));
    if riding && settled && go && t.state != 0x72 && t.camera_lock != 0x13 { return Some(Trigger::Freeze(1)); }
    if t.mode == 0 && (t.level == 8 || t.level == 0xc) && go && t.state == 0x32 { return Some(Trigger::Freeze(1)); }
    if go && settled && t.state != 0x72 {
        if t.group == 0x16 { return Some(Trigger::Freeze(0)); }
        let blocked = t.swap_state == 2 || t.state == 0x32 || t.state == 0x1d || t.fell || t.hp == 0 || t.mode != 0;
        if !blocked { return Some(Trigger::Menu(0)); }
    }
    let map = !t.debug_step
        && settled
        && t.pressed & (button::SELECT | button::R3) != 0
        && t.group != 0x16
        && ![0x72, 0x32, 0x1d].contains(&t.state)
        && !t.fell
        && t.hp != 0
        && t.mode == 0
        && !riding
        && t.c5c4 == 0;
    map.then_some(Trigger::Menu(10))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 0x2aba68's Start: on the Hoverboard (group 22) "Quit Race?" (kind 0), not the pause menu; the riders of levels 8 /
    /// 12 (state 0x32) "Quit?" (kind 1); on foot the pause menu.
    #[test]
    fn start_on_the_hoverboard_asks_to_quit_the_race() {
        let t = TriggerIn {
            mode: 0,
            frames_in_mode: 8,
            level: 5,
            pressed: crate::pad::button::START,
            connected: true,
            state: 0,
            group: 0x16,
            body: 0,
            swap_state: 0,
            fell: false,
            hp: 4,
            riding_class: None,
            camera_lock: 0,
            debug_step: false,
            c5c4: 0,
        };
        assert_eq!(in_level_trigger(&t), Some(Trigger::Freeze(0)));
        assert_eq!(in_level_trigger(&TriggerIn { state: 0x72, ..t }), None);
        assert_eq!(in_level_trigger(&TriggerIn { group: 0, ..t }), Some(Trigger::Menu(0)));
        assert_eq!(in_level_trigger(&TriggerIn { group: 0, level: 8, state: 0x32, ..t }), Some(Trigger::Freeze(1)));
    }

    #[test]
    fn raw_round_trip_and_tick_rule() {
        for v in -1..8 {
            assert_eq!(Mode::from_raw(v).unwrap().raw(), v);
        }
        assert!(Mode::from_raw(8).is_none());
        assert!(Mode::Gameplay.advances_tick() && Mode::Cutscene.advances_tick());
        for m in [Mode::Movie, Mode::Menu, Mode::Freeze, Mode::Slideshow, Mode::Debug] {
            assert!(!m.advances_tick(), "{m:?}");
        }
        assert!(!Mode::Movie.updates_pad() && Mode::Menu.updates_pad());
    }

    #[test]
    fn frames_in_mode_counts_and_resets() {
        let mut s = ModeState::default();
        for _ in 0..8 {
            s.begin_frame();
            s.end_frame();
        }
        assert_eq!(s.frames_in_mode, 8);
        s.begin_frame();
        s.set(Mode::Menu);
        s.end_frame();
        assert_eq!(s.frames_in_mode, 0);
        s.begin_frame();
        s.end_frame();
        assert_eq!((s.mode, s.frames_in_mode), (Mode::Menu, 1));
    }
}
