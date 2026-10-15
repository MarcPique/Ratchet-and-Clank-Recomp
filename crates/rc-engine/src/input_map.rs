//! Keyboard, mouse and gamepad → the 18 libpad2 bytes the game reads each tick (`sceScfPad2Read`: buttons
//! active-low in bytes 0/1, then rx, ry, lx, ly, then the 12 pressures), so `rc_game::pad` decodes exactly
//! what a PS2 pad would give it (docs/plan/player_controller.md §1, "Engine wiring").
//!
//! Encoding. Sticks: 127/128 = centre, 0 = full up/left, 255 = full down/right; the game's per-axis dead
//! zone (|b − 127| < 48) and scale (/76) are applied by `PadState`, not here. A gamepad stick is scaled per
//! axis by [`STICK_SCALE`] and clamped, then mapped linearly (`round(127.5 + 127.5·v)`, Bevy's y up → PS2 y
//! down): the DualShock 2's range, which reads full on both axes at a full diagonal; keys give full deflection
//! (0 / 255), or half deflection (`PadInput::axis_byte(±0.5)`) with Shift held. Buttons are digital: pressure
//! 0xff while held (`PadInput::press`).
//!
//! Scripted input (`RC_PLAY_SCRIPT`), for deterministic runs: see [`Script`]. With a script the devices are
//! ignored.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use rc_game::pad::{button, PadInput};

pub const CONTROLS: &str = "\
play controls (PS2 pad):  left stick = WASD / arrow keys (Shift: half stick = walk) | X = Space | [] = J | O = K | /\\ = L
                          L1 = Q | R1 = E or Ctrl (crouch) | L2 = Z | R2 = C | Start = Enter | Select = Backspace
                          d-pad = T/F/G/H | right stick = mouse with the right button held, or , and . | R3 = V or middle mouse
                          gamepad: sticks, South = X, East = O, West = [], North = /\\, bumpers L1/R1, triggers L2/R2,
                          Start/Select, stick clicks L3/R3, d-pad
engine keys:              Tab = fly camera <-> game camera (the fly camera takes the keys; the pad is neutral) |
                          R = respawn at the level's uid-0 moby | P = print the camera | Esc releases the cursor
                          F9 = dump the hero and the mobys near him | F10 = the respawn state (compare with work/scratch/respawn_probe.py) | N = noclip (dev builds)";

/// A gamepad stick's scale per axis before the byte, clamped (PCSX2's DualShock 2 default, 133 %). A DualShock 2
/// reads full on both axes at a full diagonal; a modern pad's round gate gives 0.71 on each, which the game's dead
/// zone and scale decode to a stick length of 0.80, under the walk / run table's 0.82 (`SPEED_TABLE`): held fully
/// toward a diagonal, Ratchet walked. Scaled, a full diagonal decodes to full on both axes, the right stick turns the
/// camera at full rate from about three quarters of its travel, and the game's dead zone (48 of 127) covers about
/// 28 % of the travel instead of 38 %.
pub const STICK_SCALE: f32 = 1.33;

const ACTIONS: [(&str, u32, KeyCode, GamepadButton); 16] = [
    ("cross", button::CROSS, KeyCode::Space, GamepadButton::South),
    ("square", button::SQUARE, KeyCode::KeyJ, GamepadButton::West),
    ("circle", button::CIRCLE, KeyCode::KeyK, GamepadButton::East),
    ("triangle", button::TRIANGLE, KeyCode::KeyL, GamepadButton::North),
    ("l1", button::L1, KeyCode::KeyQ, GamepadButton::LeftTrigger),
    ("r1", button::R1, KeyCode::KeyE, GamepadButton::RightTrigger),
    ("l2", button::L2, KeyCode::KeyZ, GamepadButton::LeftTrigger2),
    ("r2", button::R2, KeyCode::KeyC, GamepadButton::RightTrigger2),
    ("start", button::START, KeyCode::Enter, GamepadButton::Start),
    ("select", button::SELECT, KeyCode::Backspace, GamepadButton::Select),
    ("l3", button::L3, KeyCode::F24, GamepadButton::LeftThumb),
    ("r3", button::R3, KeyCode::KeyV, GamepadButton::RightThumb),
    ("up", button::UP, KeyCode::KeyT, GamepadButton::DPadUp),
    ("down", button::DOWN, KeyCode::KeyG, GamepadButton::DPadDown),
    ("left", button::LEFT, KeyCode::KeyF, GamepadButton::DPadLeft),
    ("right", button::RIGHT, KeyCode::KeyH, GamepadButton::DPadRight),
];

/// User-configurable input mapping. The game's own pad bytes and 60 Hz logic
/// remain unchanged; this only changes how physical devices produce them.
#[derive(Resource, Clone, Debug)]
pub struct InputSettings {
    keys: [KeyCode; 16],
    pads: [GamepadButton; 16],
    movement: [KeyCode; 4], // up, down, left, right
    /// 0 slow, 1 normal, 2 fast.
    pub camera_sensitivity: u8,
    /// 0 original game's deadzone, 1 +10%, 2 +20% on modern sticks.
    pub stick_deadzone: u8,
}

/// The icon family shown for the game's inline pad prompts. Auto follows the
/// connected pad; a manual choice handles adapters with missing vendor IDs.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptSettings {
    /// 0 auto, 1 PS5, 2 PS4, 3 PS3, 4 Xbox Series, 5 Xbox One,
    /// 6 Xbox 360, 7 Switch, 8 generic PlayStation, 9 generic Xbox.
    pub choice: u8,
    pub detected: crate::text_render::PromptStyle,
}

impl PromptSettings {
    pub fn startup() -> Self {
        let choice = match crate::render_settings::load_key("button_prompts").as_deref() {
            Some("dualsense") => 1, Some("dualshock4") => 2, Some("dualshock3") => 3,
            Some("xboxseries") => 4, Some("xboxone") => 5, Some("xbox360") => 6,
            Some("nintendo") => 7, Some("playstation") => 8, Some("xbox") => 9, _ => 0,
        };
        Self { choice, detected: crate::text_render::PromptStyle::PlayStation }
    }

    pub fn effective(self) -> crate::text_render::PromptStyle {
        use crate::text_render::PromptStyle;
        match self.choice { 1 => PromptStyle::DualSense, 2 => PromptStyle::DualShock4, 3 => PromptStyle::DualShock3,
            4 => PromptStyle::XboxSeries, 5 => PromptStyle::XboxOne, 6 => PromptStyle::Xbox360,
            7 => PromptStyle::Nintendo, 8 => PromptStyle::PlayStation, 9 => PromptStyle::Xbox, _ => self.detected }
    }

    pub fn save_changed(self, before: Self) {
        if self.choice != before.choice {
            crate::render_settings::save_key("button_prompts", ["auto", "dualsense", "dualshock4", "dualshock3", "xboxseries", "xboxone", "xbox360", "nintendo", "playstation", "xbox"][self.choice.min(9) as usize]);
        }
    }
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            keys: ACTIONS.map(|a| a.2), pads: ACTIONS.map(|a| a.3),
            movement: [KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD],
            camera_sensitivity: 1, stick_deadzone: 0,
        }
    }
}

const KEY_CHOICES: &[KeyCode] = &[
    KeyCode::Space, KeyCode::Enter, KeyCode::Backspace,
    KeyCode::Comma, KeyCode::Period, KeyCode::ShiftLeft,
    KeyCode::ShiftRight, KeyCode::ControlLeft, KeyCode::ControlRight,
    KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight,
    KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE,
    KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI, KeyCode::KeyJ,
    KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyO,
    KeyCode::KeyQ, KeyCode::KeyS, KeyCode::KeyT,
    KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY,
    KeyCode::KeyZ, KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2,
    KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
    KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9, KeyCode::F24,
];
const PAD_CHOICES: &[GamepadButton] = &[
    GamepadButton::South, GamepadButton::East, GamepadButton::West, GamepadButton::North,
    GamepadButton::LeftTrigger, GamepadButton::RightTrigger,
    GamepadButton::LeftTrigger2, GamepadButton::RightTrigger2,
    GamepadButton::Start, GamepadButton::Select,
    GamepadButton::LeftThumb, GamepadButton::RightThumb,
    GamepadButton::DPadUp, GamepadButton::DPadDown,
    GamepadButton::DPadLeft, GamepadButton::DPadRight,
];

fn named<T: Copy + std::fmt::Debug>(name: &str, choices: &[T]) -> Option<T> {
    choices.iter().copied().find(|key| format!("{key:?}").eq_ignore_ascii_case(name.trim()))
}

impl InputSettings {
    pub fn startup() -> Self {
        let mut s = Self::default();
        for (i, (name, ..)) in ACTIONS.iter().enumerate() {
            if let Some(key) = crate::render_settings::load_key(&format!("key_{name}")).and_then(|v| named(&v, KEY_CHOICES)) { s.keys[i] = key; }
            if let Some(pad) = crate::render_settings::load_key(&format!("pad_{name}")).and_then(|v| named(&v, PAD_CHOICES)) { s.pads[i] = pad; }
        }
        for (i, name) in ["move_up", "move_down", "move_left", "move_right"].iter().enumerate() {
            if let Some(key) = crate::render_settings::load_key(&format!("key_{name}")).and_then(|v| named(&v, KEY_CHOICES)) { s.movement[i] = key; }
        }
        s.camera_sensitivity = crate::render_settings::load_key("camera_sensitivity").as_deref().map_or(1, |v| match v { "slow" => 0, "fast" => 2, _ => 1 });
        s.stick_deadzone = crate::render_settings::load_key("stick_deadzone").as_deref().map_or(0, |v| match v { "small" => 1, "medium" => 2, _ => 0 });
        s
    }
    pub fn save_options(&self, old: &Self) {
        if self.camera_sensitivity != old.camera_sensitivity {
            crate::render_settings::save_key("camera_sensitivity", ["slow", "normal", "fast"][self.camera_sensitivity.min(2) as usize]);
        }
        if self.stick_deadzone != old.stick_deadzone {
            crate::render_settings::save_key("stick_deadzone", ["original", "small", "medium"][self.stick_deadzone.min(2) as usize]);
        }
    }

    /// Physical buttons behind text icons 0x10..0x19, in the original font's
    /// order: Cross, Circle, Triangle, Square, L1, R1, L2, R2, L3, R3.
    pub fn prompt_buttons(&self) -> [GamepadButton; 10] {
        [self.pads[0], self.pads[2], self.pads[3], self.pads[1], self.pads[4], self.pads[5], self.pads[6], self.pads[7], self.pads[10], self.pads[11]]
    }
}

fn extra_deadzone(v: f32, amount: u8) -> f32 {
    let edge = [0.0, 0.1, 0.2][amount.min(2) as usize];
    if v.abs() <= edge { 0.0 } else { v.signum() * ((v.abs() - edge) / (1.0 - edge)).min(1.0) }
}

/// Mouse pixels per frame for a full right-stick deflection.
const MOUSE_FULL: f32 = 12.0;

/// This frame's pad read (sampled in `PreUpdate`, used by every tick of the frame).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct PadFrame(pub PadInput);

/// A Bevy stick value in [−1, 1] (y up) → the raw libpad2 byte (y down).
fn raw_axis(v: f32) -> u8 { (127.5 + 127.5 * v.clamp(-1.0, 1.0)).round().clamp(0.0, 255.0) as u8 }

/// Keyboard + mouse + the first gamepad → one pad read.
pub fn sample(keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>, motion: &AccumulatedMouseMotion, gamepad: Option<&Gamepad>) -> PadInput {
    sample_with_settings(keys, mouse, motion, gamepad, &InputSettings::default())
}

pub fn sample_with_settings(keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>, motion: &AccumulatedMouseMotion, gamepad: Option<&Gamepad>, settings: &InputSettings) -> PadInput {
    let mut p = PadInput::neutral();
    let mut buttons = 0u32;

    // Keyboard left stick (per axis full or half deflection).
    let k = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let x = k(settings.movement[3], KeyCode::ArrowRight) as i32 - k(settings.movement[2], KeyCode::ArrowLeft) as i32;
    let y = k(settings.movement[1], KeyCode::ArrowDown) as i32 - k(settings.movement[0], KeyCode::ArrowUp) as i32;
    if x != 0 || y != 0 {
        let walk = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        let byte = |v: i32| if v == 0 { 0x7f } else if walk { PadInput::axis_byte(0.5 * v as f32) } else if v < 0 { 0 } else { 0xff };
        p.lx = byte(x);
        p.ly = byte(y);
    }
    // Right stick: mouse while the right button is held (the fly camera's grab locks the cursor), or , and .
    let rkx = keys.pressed(KeyCode::Period) as i32 - keys.pressed(KeyCode::Comma) as i32;
    if rkx != 0 { p.rx = if rkx < 0 { 0 } else { 0xff }; }
    if mouse.pressed(MouseButton::Right) && motion.delta != Vec2::ZERO {
        let sensitivity = [0.7, 1.0, 1.4][settings.camera_sensitivity.min(2) as usize];
        p.rx = raw_axis(motion.delta.x * sensitivity / MOUSE_FULL);
        p.ry = raw_axis(-motion.delta.y * sensitivity / MOUSE_FULL);
    }
    for (i, (_, b, _, _)) in ACTIONS.iter().enumerate() {
        if keys.pressed(settings.keys[i]) { buttons |= *b; }
    }
    if keys.pressed(KeyCode::ControlLeft) && settings.keys[5] == KeyCode::KeyE { buttons |= button::R1; }
    if mouse.pressed(MouseButton::Middle) { buttons |= button::R3; }

    if let Some(g) = gamepad {
        let (l, r) = (g.left_stick(), g.right_stick());
        if l != Vec2::ZERO {
            p.lx = raw_axis(extra_deadzone(l.x, settings.stick_deadzone) * STICK_SCALE);
            p.ly = raw_axis(-extra_deadzone(l.y, settings.stick_deadzone) * STICK_SCALE);
        }
        if r != Vec2::ZERO {
            let sensitivity = [0.7, 1.0, 1.4][settings.camera_sensitivity.min(2) as usize];
            p.rx = raw_axis(extra_deadzone(r.x, settings.stick_deadzone) * STICK_SCALE * sensitivity);
            p.ry = raw_axis(-extra_deadzone(r.y, settings.stick_deadzone) * STICK_SCALE * sensitivity);
        }
        for (i, (_, b, _, _)) in ACTIONS.iter().enumerate() {
            if g.pressed(settings.pads[i]) { buttons |= *b; }
        }
    }
    p.press(buttons)
}

/// Samples the devices into [`PadFrame`] (after Bevy's input systems).
fn detect_prompt_style(vendor: Option<u16>, product: Option<u16>, connected: bool) -> crate::text_render::PromptStyle {
    use crate::text_render::PromptStyle;
    match (vendor, product) {
        (Some(0x054c), Some(0x0ce6 | 0x0df2)) => PromptStyle::DualSense,
        (Some(0x054c), Some(0x05c4 | 0x09cc | 0x0ba0)) => PromptStyle::DualShock4,
        (Some(0x054c), Some(0x0268)) => PromptStyle::DualShock3,
        (Some(0x054c), _) => PromptStyle::PlayStation,
        (Some(0x057e), _) => PromptStyle::Nintendo,
        (Some(0x045e), Some(0x0b12 | 0x0b13)) => PromptStyle::XboxSeries,
        (Some(0x045e), Some(0x02d1 | 0x02dd | 0x02e3 | 0x02ea | 0x0b00 | 0x0b0a)) => PromptStyle::XboxOne,
        (Some(0x045e), Some(0x028e | 0x028f | 0x0291 | 0x02a9 | 0x0719)) => PromptStyle::Xbox360,
        _ if connected => PromptStyle::Xbox,
        _ => PromptStyle::PlayStation,
    }
}

pub fn sample_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    gamepads: Query<&Gamepad>,
    settings: Res<InputSettings>,
    mut prompts: ResMut<PromptSettings>,
    mut frame: ResMut<PadFrame>,
) {
    let gamepad = gamepads.iter().next();
    prompts.detected = detect_prompt_style(gamepad.and_then(Gamepad::vendor_id), gamepad.and_then(Gamepad::product_id), gamepad.is_some());
    crate::text_render::set_button_prompts(prompts.effective(), &settings.prompt_buttons());
    frame.0 = sample_with_settings(&keys, &mouse, &motion, gamepad, &settings);
}

/// `RC_PLAY_SCRIPT`: pad input per gameplay tick, `item,item,...` with `item = range:action`.
///
/// * `range`: `a-b` (ticks a through b, inclusive) or `a`. Tick t is the (t+1)-th gameplay tick since the
///   level started (`Game::counter` before the tick); in frame-exact mode (`RC_SCREENSHOT_FRAME` /
///   `RC_DETERMINISTIC=1`) tick t runs in update t+1, so `RC_SCREENSHOT_FRAME=N` shows the state after
///   ticks 0..N−1.
/// * `action`: `stick x y` (left stick, x right, y down, each in [−1, 1]: `stick 0 -1` = full forward;
///   `PadInput::stick`, the byte that decodes to exactly that value), `rstick x y` (right stick), or
///   `press B[+B...]` (hold buttons: `X`/`CROSS`, `SQUARE`, `CIRCLE`/`O`, `TRIANGLE`, `L1`, `R1`, `L2`, `R2`,
///   `L3`, `R3`, `START`, `SELECT`, `UP`, `DOWN`, `LEFT`, `RIGHT`).
///
/// Items whose ranges overlap combine (the last stick item wins). Ticks no item covers are neutral.
/// Example: `0-179:stick 0 -1` runs forward for 180 ticks; `60-90:press X` holds jump for 31 ticks.
#[derive(Clone, Debug, Default)]
pub struct Script {
    items: Vec<(u64, u64, Action)>,
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Stick(f32, f32),
    RStick(f32, f32),
    Press(u32),
}

fn button_named(s: &str) -> Option<u32> {
    Some(match s.to_ascii_uppercase().as_str() {
        "X" | "CROSS" => button::CROSS,
        "SQUARE" => button::SQUARE,
        "O" | "CIRCLE" => button::CIRCLE,
        "TRIANGLE" => button::TRIANGLE,
        "L1" => button::L1,
        "R1" => button::R1,
        "L2" => button::L2,
        "R2" => button::R2,
        "L3" => button::L3,
        "R3" => button::R3,
        "START" => button::START,
        "SELECT" => button::SELECT,
        "UP" => button::UP,
        "DOWN" => button::DOWN,
        "LEFT" => button::LEFT,
        "RIGHT" => button::RIGHT,
        _ => return None,
    })
}

impl Script {
    pub fn parse(s: &str) -> Result<Script, String> {
        let mut items = Vec::new();
        for item in s.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            let (range, action) = item.split_once(':').ok_or_else(|| format!("{item:?}: expected range:action"))?;
            let num = |v: &str| v.trim().parse::<u64>().map_err(|_| format!("{item:?}: bad tick {v:?}"));
            let (a, b) = match range.split_once('-') {
                Some((a, b)) => (num(a)?, num(b)?),
                None => (num(range)?, num(range)?),
            };
            if b < a { return Err(format!("{item:?}: empty range")); }
            let words: Vec<&str> = action.split_whitespace().collect();
            let f = |v: &str| v.parse::<f32>().map_err(|_| format!("{item:?}: bad number {v:?}"));
            let action = match words.as_slice() {
                ["stick", x, y] => Action::Stick(f(x)?, f(y)?),
                ["rstick", x, y] => Action::RStick(f(x)?, f(y)?),
                ["press", names] => Action::Press(
                    names.split('+').try_fold(0u32, |m, n| button_named(n).map(|b| m | b).ok_or_else(|| format!("{item:?}: unknown button {n:?}")))?,
                ),
                _ => return Err(format!("{item:?}: expected `stick x y`, `rstick x y` or `press B[+B]`")),
            };
            items.push((a, b, action));
        }
        Ok(Script { items })
    }

    /// The pad read of tick `t`.
    pub fn at(&self, t: u64) -> PadInput {
        let mut p = PadInput::neutral();
        for &(a, b, act) in &self.items {
            if t < a || t > b { continue; }
            p = match act {
                Action::Stick(x, y) => p.stick(x, y),
                Action::RStick(x, y) => p.rstick(x, y),
                Action::Press(m) => p.press(m),
            };
        }
        p
    }

    /// The last tick any item covers.
    pub fn end(&self) -> u64 { self.items.iter().map(|i| i.1).max().unwrap_or(0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_game::pad::{axis, PadState};

    #[test]
    fn script_ranges_and_actions() {
        let s = Script::parse("0-120:stick 0 -1, 60:press X+R1, 200-201:rstick 1 0").unwrap();
        assert_eq!(s.at(0), PadInput::neutral().stick(0.0, -1.0));
        assert_eq!(s.at(60), PadInput::neutral().stick(0.0, -1.0).press(button::CROSS | button::R1));
        assert_eq!(s.at(121), PadInput::neutral());
        assert_eq!(s.at(201).rx, PadInput::axis_byte(1.0));
        assert_eq!(s.end(), 201);
        assert!(Script::parse("5-1:press X").is_err());
        assert!(Script::parse("1:press Y").is_err());
        assert!(Script::parse("1:jump").is_err());
    }

    #[test]
    fn device_encoding_decodes_like_a_pad() {
        // Linear gamepad bytes: centre decodes to 0, full deflection to ±1, up/left negative.
        assert_eq!(axis(raw_axis(0.0)).to_f32(), 0.0);
        assert_eq!(axis(raw_axis(1.0)).to_f32(), 1.0);
        assert_eq!(axis(raw_axis(-1.0)).to_f32(), -1.0);
        assert_eq!(axis(raw_axis(0.3)).to_f32(), 0.0); // inside the game's dead zone (|b - 127| < 48)
        // Keyboard forward = stick up = ly byte 0.
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::Space);
        let p = sample(&keys, &ButtonInput::default(), &AccumulatedMouseMotion::default(), None);
        let mut pad = PadState::default();
        pad.update(Some(&p.bytes()), false);
        assert_eq!((pad.lx.to_f32(), pad.ly.to_f32()), (0.0, -1.0));
        assert!(pad.pressed & button::CROSS != 0);
        // Shift: half stick.
        keys.press(KeyCode::ShiftLeft);
        let p = sample(&keys, &ButtonInput::default(), &AccumulatedMouseMotion::default(), None);
        pad.update(Some(&p.bytes()), false);
        assert!((pad.ly.to_f32() + 0.5).abs() < 0.01);
    }

    #[test]
    fn remapped_button_and_deadzone_take_effect() {
        let mut settings = InputSettings::default();
        settings.keys[0] = KeyCode::KeyB;
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Space);
        let mouse = ButtonInput::<MouseButton>::default();
        let motion = AccumulatedMouseMotion::default();
        let mut pad = PadState::default();
        let raw = sample_with_settings(&keys, &mouse, &motion, None, &settings);
        pad.update(Some(&raw.bytes()), false);
        assert_eq!(pad.pressed & button::CROSS, 0);
        keys.press(KeyCode::KeyB);
        let raw = sample_with_settings(&keys, &mouse, &motion, None, &settings);
        pad.update(Some(&raw.bytes()), false);
        assert_ne!(pad.held & button::CROSS, 0);
        assert_eq!(extra_deadzone(0.15, 2), 0.0);
        assert!(extra_deadzone(0.6, 2) > 0.0);
    }

    #[test]
    fn prompts_follow_button_reassignment_and_manual_style() {
        let mut input = InputSettings::default();
        assert_eq!(input.prompt_buttons()[0], GamepadButton::South);
        input.pads[0] = GamepadButton::East;
        assert_eq!(input.prompt_buttons()[0], GamepadButton::East);
        let mut prompts = PromptSettings { choice: 0, detected: crate::text_render::PromptStyle::Nintendo };
        assert_eq!(prompts.effective(), crate::text_render::PromptStyle::Nintendo);
        prompts.choice = 6;
        assert_eq!(prompts.effective(), crate::text_render::PromptStyle::Xbox360);
        assert_eq!(detect_prompt_style(Some(0x054c), Some(0x0ce6), true), crate::text_render::PromptStyle::DualSense);
        assert_eq!(detect_prompt_style(Some(0x054c), Some(0x0df2), true), crate::text_render::PromptStyle::DualSense);
        assert_eq!(detect_prompt_style(Some(0x054c), Some(0x09cc), true), crate::text_render::PromptStyle::DualShock4);
        assert_eq!(detect_prompt_style(Some(0x054c), Some(0x0268), true), crate::text_render::PromptStyle::DualShock3);
        assert_eq!(detect_prompt_style(Some(0x057e), Some(0x2009), true), crate::text_render::PromptStyle::Nintendo);
        assert_eq!(detect_prompt_style(Some(0x045e), Some(0x0b12), true), crate::text_render::PromptStyle::XboxSeries);
        assert_eq!(detect_prompt_style(Some(0x045e), Some(0x02ea), true), crate::text_render::PromptStyle::XboxOne);
        assert_eq!(detect_prompt_style(Some(0x045e), Some(0x028e), true), crate::text_render::PromptStyle::Xbox360);
    }
}
