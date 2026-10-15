//! Reverb zones (docs/plan/audio.md "Reverb zones and level sounds"): the game's rules for which reverb plays where
//! (EE side, level01 addresses; the same code in every overlay), and the native reverb effect the mixer applies.
//!
//! **The game's rules** (EE):
//! * The request state `0x13e5b0..0x13e5bb` ([`ReverbRequest`]): owner, depth (s32), libsd effect type, delay, feedback
//!   and a dirty mask. `FUN_002a1a90(owner, type, depth, delay, feedback)` ([`ReverbRequest::set`]) stores a value and
//!   marks it dirty only when it changed (1 type, 4 depth, 2 delay / feedback).
//! * **Reverb boxes** (sound instance class 3, `SndInstReverbBoxUpdate` 0x319f18, [`ReverbBox`]), pvars `{u8 type,
//!   u8 delay, u8 feedback, u8 inside, s32 depth}`: the **hero** position 0x13f3d0 in box space (the inverse rows
//!   +0x50 applied to `hero − position`). Inside the unit cube (`|x|, |y|, |z| ≤ 1`): depth `trunc(depth·(x + 1)·0.5)`
//!   (at most the box's depth), a ramp from 0 on the −x face to the full depth on the +x face; inside = 1. Leaving it
//!   (inside was 1): through the +x side (`x > 0`) the box's full depth stays set; otherwise the reverb is switched
//!   off (type 0, depth 0). The boxes come in pairs at a cave's mouths with +x pointing in.
//! * **Env sample points** (gameplay section 0x88): `EnvNearestSamplePoint` 0x264e98, called only by `HeroTeleport`
//!   0x2368e0 (the level start's placement at 0x13e090 by `GameStateUpdate`, the vendor / ship / camera-trigger
//!   teleports): the nearest point **within 8 units** of the hero (strictly nearer wins; the first on a tie) sets
//!   the reverb directly when its +0x27 is set (depth +0x20, type +0x24, delay +0x25, feedback +0x26; dirty |= 7),
//!   and the level's music track from +0x28 while the music is idle ([`ReverbRequest::apply_env_point`]).
//! * **Sending** (the head of `sound_update` 0x2a0638, [`ReverbRequest::command`]): dirty & 8 → `SetReverbEx(2, 0,
//!   0, 0, 0)` (off); else dirty & 0x13 → `SetReverbEx(2, type, depth, delay, feedback)`; else dirty & 4 →
//!   `snd_AutoReverb(2, depth, 12, 3)` (a depth glide); then dirty = 0. Bit 8 is set by `StartPssMovie` and the
//!   level unload (`fun_00231608`), bit 0x10 (resend) by `MovieExitToGameplay` and the level load's end. The
//!   checkpoint record (`0x29ac10`) saves depth / type / delay / feedback and the death reload (`0x29adc8`) puts them
//!   back with dirty |= 7 ([`ReverbRequest::checkpoint`] / [`ReverbRequest::restore`]).
//!
//! **The effect** ([`ReverbFx`], the IOP side): a native stereo reverb in the style of a Moorer reverb (four damped
//! feedback combs per channel into two series all-passes), not the SPU2's reverb engine. What reaches it is the send
//! of the voices whose tone is flagged "to reverb" (tone flag 1: every level-bank tone, no global-bank tone, no VAG
//! stream), after their envelope and volume, as on the SPU2. Its wet output is scaled by `depth / 0x8000` (the libsd
//! effect volume) and added to the dry mix before the master volume. Each libsd effect type is a [`Preset`] whose
//! numbers were derived once from libsd v3.03's preset table (global `irx` entry 21) as *results*: the same-side and
//! cross-side loop times, the decay time (loop time and wall gain), the damping (the loop's IIR coefficient) and the
//! diffusion (the two all-passes' times and gains), with the 24 kHz internal rate taken into account. The table
//! itself is not used and no register is modelled. The effect runs at 48 kHz on `f32`, band-limited to ≈ 11 kHz at
//! its input as the SPU2's half-rate reverb is (a result-level reproduction, docs/plan/hardware_fidelity_layers.md).

use super::voices::SndCommand;
use rc_formats::sound_bank::{EnvSamplePoint, SoundInstance};

/// libsd effect types (`SD_REV_MODE_*`).
pub mod mode {
    pub const OFF: u8 = 0;
    pub const ROOM: u8 = 1;
    pub const STUDIO_A: u8 = 2;
    pub const STUDIO_B: u8 = 3;
    pub const STUDIO_C: u8 = 4;
    pub const HALL: u8 = 5;
    pub const SPACE: u8 = 6;
    pub const ECHO: u8 = 7;
    pub const DELAY: u8 = 8;
    pub const PIPE: u8 = 9;
    /// Names for logs.
    pub const NAMES: [&str; 10] = ["off", "room", "studio A", "studio B", "studio C", "hall", "space", "echo", "delay", "pipe"];
}

/// The dirty bits of 0x13e5bb.
pub mod dirty {
    pub const TYPE: u8 = 1;
    pub const DELAY_FEEDBACK: u8 = 2;
    pub const DEPTH: u8 = 4;
    /// Switch the reverb off (movie start, level unload).
    pub const OFF: u8 = 8;
    /// Resend everything (movie end, level load end).
    pub const RESEND: u8 = 0x10;
}

/// The libsd core the game's reverb commands address (`SND_CORE_1`).
pub const CORE: u8 = 2;
/// `snd_AutoReverb(2, depth, 12, 3)`: the glide time and the channel mask (both sides).
pub const AUTO_DELTA: u16 = 12;
pub const AUTO_CHANNELS: u8 = 3;
/// `EnvNearestSamplePoint`'s search radius.
pub const ENV_POINT_RADIUS: f32 = 8.0;

/// The EE's reverb request (0x13e5b0..0x13e5bb) and the checkpoint copy of it (0x1bb6ec..0x1bb6f2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReverbRequest {
    /// 0x13e5b0: the box instance that set it last (None: 0, e.g. the +x exit).
    pub owner: Option<usize>,
    /// 0x13e5b4.
    pub depth: i32,
    /// 0x13e5b8.
    pub kind: u8,
    /// 0x13e5b9 / 0x13e5ba.
    pub delay: u8,
    pub feedback: u8,
    /// 0x13e5bb.
    pub dirty: u8,
    /// 0x1bb6ec..0x1bb6f2: `(depth, type, delay, feedback)` as the checkpoint record saved them.
    pub saved: Option<(i32, u8, u8, u8)>,
}

impl ReverbRequest {
    /// `FUN_002a1a90(owner, type, depth, delay, feedback)`.
    pub fn set(&mut self, owner: Option<usize>, kind: u8, depth: i32, delay: u8, feedback: u8) {
        self.owner = owner;
        if self.kind != kind {
            self.dirty |= dirty::TYPE;
            self.kind = kind;
        }
        if self.depth != depth {
            self.dirty |= dirty::DEPTH;
            self.depth = depth;
        }
        if self.delay != delay || self.feedback != feedback {
            self.dirty |= dirty::DELAY_FEEDBACK;
            self.delay = delay;
            self.feedback = feedback;
        }
    }

    /// `EnvNearestSamplePoint`'s reverb part: the point's values stored directly, dirty |= 7, when its +0x27 is set.
    pub fn apply_env_point(&mut self, p: &EnvSamplePoint) {
        if p.reverb_enable == 0 { return; }
        self.depth = p.reverb_depth;
        self.kind = p.reverb_type;
        self.delay = p.reverb_delay;
        self.feedback = p.reverb_feedback;
        self.dirty |= dirty::TYPE | dirty::DELAY_FEEDBACK | dirty::DEPTH;
    }

    /// The head of `sound_update`: the command for this frame's dirty bits (None when clean), then clean.
    pub fn command(&mut self) -> Option<SndCommand> {
        let d = std::mem::take(&mut self.dirty);
        if d & dirty::OFF != 0 {
            Some(SndCommand::SetReverb { kind: mode::OFF, depth: 0, delay: 0, feedback: 0 })
        } else if d & (dirty::TYPE | dirty::DELAY_FEEDBACK | dirty::RESEND) != 0 {
            Some(SndCommand::SetReverb { kind: self.kind, depth: self.depth, delay: self.delay, feedback: self.feedback })
        } else if d & dirty::DEPTH != 0 {
            Some(SndCommand::AutoReverb { depth: self.depth, delta: AUTO_DELTA, channels: AUTO_CHANNELS })
        } else {
            None
        }
    }

    /// The checkpoint record `0x29ac10`: depth, type, delay and feedback saved.
    pub fn checkpoint(&mut self) { self.saved = Some((self.depth, self.kind, self.delay, self.feedback)); }

    /// The death reload `0x29adc8`: the saved values back, dirty |= 7 (nothing without a record).
    pub fn restore(&mut self) {
        let Some((depth, kind, delay, feedback)) = self.saved else { return };
        (self.depth, self.kind, self.delay, self.feedback) = (depth, kind, delay, feedback);
        self.dirty |= dirty::TYPE | dirty::DELAY_FEEDBACK | dirty::DEPTH;
    }
}

/// `EnvNearestSamplePoint`'s search: the nearest point within [`ENV_POINT_RADIUS`] of `pos` (`d ≤ 8`, then strictly
/// nearer than the best so far: the first wins a tie). None: the game prints a warning and changes nothing.
pub fn nearest_env_point(points: &[EnvSamplePoint], pos: [f32; 3]) -> Option<usize> {
    let mut best = (999_999.0f32, None);
    for (i, p) in points.iter().enumerate() {
        let d = ((pos[0] - p.position[0]).powi(2) + (pos[1] - p.position[1]).powi(2) + (pos[2] - p.position[2]).powi(2)).sqrt();
        if d <= ENV_POINT_RADIUS && d < best.0 { best = (d, Some(i)); }
    }
    best.1
}

/// A reverb box (sound instance class 3) and its pvars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReverbBox {
    /// The sound instance index.
    pub instance: usize,
    pub center: [f32; 3],
    /// The inverse rows (+0x50): world offset → box space.
    pub inverse: [[f32; 4]; 3],
    /// pvar +0 / +1 / +2 / +4.
    pub kind: u8,
    pub delay: u8,
    pub feedback: u8,
    pub depth: i32,
    /// pvar +3: the hero was inside at the last update.
    pub inside: bool,
}

impl ReverbBox {
    /// Class 3 instances only; None for another class or a short pvar block.
    pub fn from_instance(index: usize, s: &SoundInstance, pvars: &[u8]) -> Option<Self> {
        if s.o_class != 3 || pvars.len() < 8 { return None; }
        Some(ReverbBox {
            instance: index,
            center: s.position(),
            inverse: s.inverse,
            kind: pvars[0],
            delay: pvars[1],
            feedback: pvars[2],
            inside: pvars[3] != 0,
            depth: i32::from_le_bytes(pvars[4..8].try_into().unwrap()),
        })
    }

    /// The hero position in box space.
    pub fn local(&self, hero: [f32; 3]) -> [f32; 3] {
        let v = [hero[0] - self.center[0], hero[1] - self.center[1], hero[2] - self.center[2]];
        let r = &self.inverse;
        std::array::from_fn(|k| v[0] * r[0][k] + v[1] * r[1][k] + v[2] * r[2][k])
    }

    /// `SndInstReverbBoxUpdate` 0x319f18 for the hero at `hero` (0x13f3d0).
    pub fn update(&mut self, hero: [f32; 3], req: &mut ReverbRequest) {
        let l = self.local(hero);
        if l.iter().all(|x| x.abs() <= 1.0) {
            let depth = ((self.depth as f32 * (l[0] + 1.0)) * 0.5) as i32;
            req.set(Some(self.instance), self.kind, depth.min(self.depth), self.delay, self.feedback);
            self.inside = true;
            return;
        }
        if !self.inside { return; }
        if 0.0 < l[0] {
            req.set(None, self.kind, self.depth, self.delay, self.feedback);
        } else {
            req.set(Some(self.instance), mode::OFF, 0, 0, 0);
        }
        self.inside = false;
    }
}

// ---------------------------------------------------------------------------------------------------
// The native effect

/// A libsd effect type's character (see the module docs for where the numbers come from).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    /// The same-side and cross-side loop times (ms).
    pub loops_ms: [f32; 2],
    /// Decay time to −60 dB (s).
    pub rt60: f32,
    /// The loop's one-pole low-pass pole at 48 kHz (0 = no damping).
    pub damp: f32,
    /// The two diffusing all-passes: times (ms) and gains.
    pub ap_ms: [f32; 2],
    pub ap_gain: [f32; 2],
}

/// The libsd types' character: ROOM, STUDIO_A/B/C (small / medium / large), HALL, SPACE, PIPE. ECHO and DELAY are a
/// single delay line ([`echo_delay_samples`]).
pub fn preset(kind: u8) -> Option<Preset> {
    let p = |same: f32, cross: f32, rt60: f32, iir: f32, ap_ms: [f32; 2], ap_gain: [f32; 2]| Preset {
        loops_ms: [same, cross],
        rt60,
        // The loop filter `y += (x − y)·iir` at 24 kHz; the same cut-off at 48 kHz has the square-rooted pole.
        damp: (1.0 - iir).sqrt(),
        ap_ms,
        ap_gain,
    };
    Some(match kind {
        mode::ROOM => p(69.3, 64.0, 0.78, 0.855, [20.8, 15.2], [0.69, 0.65]),
        mode::STUDIO_A => p(33.3, 70.5, 0.93, 0.882, [8.5, 6.2], [0.64, 0.62]),
        mode::STUDIO_B => p(68.0, 150.5, 0.88, 0.882, [29.5, 21.2], [0.64, 0.62]),
        mode::STUDIO_C => p(112.3, 244.7, 2.17, 0.870, [37.8, 28.2], [0.68, 0.65]),
        mode::HALL => p(169.7, 341.2, 1.69, 0.750, [70.2, 52.2], [0.75, 0.72]),
        mode::SPACE => p(198.0, 471.5, 2.91, 0.984, [138.2, 93.5], [0.75, 0.66]),
        mode::PIPE => p(4.2, 40.5, 0.72, 0.882, [3.8, 3.2], [0.75, 0.66]),
        _ => return None,
    })
}

/// ECHO / DELAY: the delay parameter (0..127) as samples at 48 kHz: `(delay + 1)` steps of 128 samples at the 24 kHz
/// internal rate, 0.68 s at 127 (the preset's loop) [M: the libsd mapping is inferred; no level uses these types].
pub fn echo_delay_samples(delay: u8) -> usize { (delay.min(127) as usize + 1) * 256 }

/// The effect's output rate.
const RATE: f32 = super::OUTPUT_RATE as f32;
/// Input band limit (the SPU2's reverb runs at 24 kHz): one-pole low-pass at ≈ 11 kHz.
const INPUT_POLE: f32 = 0.237;
/// Wet scale of the comb sum (the preset's steady-state level: its four comb taps at ≈ 0.6 over a loop gain of
/// ≈ 0.5–0.8 give roughly the send's level back).
const COMB_OUT: f32 = 0.5;
/// Stereo decorrelation: the right channel's lines are this much longer (samples).
const SPREAD: usize = 23;
/// The four combs per channel: the same-side loop, the cross-side loop, and two in between (mutually detuned).
const COMB_SCALE: [(usize, f32); 4] = [(0, 1.0), (1, 1.0), (0, 0.781), (1, 0.853)];

#[derive(Clone, Debug)]
struct Line {
    buf: Vec<f32>,
    pos: usize,
}

impl Line {
    fn new(n: usize) -> Self { Line { buf: vec![0.0; n.max(1)], pos: 0 } }
    fn read(&self) -> f32 { self.buf[self.pos] }
    fn write_advance(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.buf.len() { self.pos = 0; }
    }
}

#[derive(Clone, Debug)]
struct Comb {
    line: Line,
    gain: f32,
    damp: f32,
    lp: f32,
}

impl Comb {
    fn run(&mut self, x: f32) -> f32 {
        let y = self.line.read();
        self.lp = y + (self.lp - y) * self.damp;
        self.line.write_advance(x + self.lp * self.gain);
        y
    }
}

#[derive(Clone, Debug)]
struct Allpass {
    line: Line,
    gain: f32,
}

impl Allpass {
    fn run(&mut self, x: f32) -> f32 {
        let d = self.line.read();
        let v = x + d * self.gain;
        self.line.write_advance(v);
        d - v * self.gain
    }
}

#[derive(Clone, Debug)]
enum Network {
    Reverb { combs: Box<[[Comb; 4]; 2]>, aps: Box<[[Allpass; 2]; 2]> },
    Echo { lines: [Line; 2], feedback: f32 },
}

impl Network {
    fn new(kind: u8, delay: u8, feedback: u8) -> Option<Self> {
        if kind == mode::ECHO || kind == mode::DELAY {
            let n = echo_delay_samples(delay);
            let fb = if kind == mode::ECHO { feedback.min(127) as f32 / 128.0 } else { 0.0 };
            return Some(Network::Echo { lines: [Line::new(n), Line::new(n + SPREAD)], feedback: fb });
        }
        let p = preset(kind)?;
        let ms = |t: f32| ((t * RATE / 1000.0).round() as usize).max(1);
        let combs = std::array::from_fn(|ch| {
            std::array::from_fn(|k| {
                let (which, scale) = COMB_SCALE[k];
                let n = ms(p.loops_ms[which] * scale) + ch * SPREAD;
                Comb { line: Line::new(n), gain: 10f32.powf(-3.0 * n as f32 / (p.rt60 * RATE)), damp: p.damp, lp: 0.0 }
            })
        });
        let aps = std::array::from_fn(|ch| std::array::from_fn(|k| Allpass { line: Line::new(ms(p.ap_ms[k]) + ch * (SPREAD / 2)), gain: p.ap_gain[k] }));
        Some(Network::Reverb { combs: Box::new(combs), aps: Box::new(aps) })
    }

    /// One stereo sample of send in, wet out (before the depth).
    fn run(&mut self, x: [f32; 2]) -> [f32; 2] {
        match self {
            Network::Reverb { combs, aps } => std::array::from_fn(|ch| {
                // Combs 0 and 2 take this side's send, 1 and 3 the other side's (the SPU's same / cross paths).
                let mut sum = 0.0;
                for (k, c) in combs[ch].iter_mut().enumerate() {
                    sum += c.run(x[if COMB_SCALE[k].0 == 0 { ch } else { 1 - ch }]);
                }
                let mut y = sum * COMB_OUT;
                for a in &mut aps[ch] { y = a.run(y); }
                y
            }),
            Network::Echo { lines, feedback } => std::array::from_fn(|ch| {
                let y = lines[ch].read();
                lines[ch].write_advance(x[ch] + y * *feedback);
                y
            }),
        }
    }
}

/// The reverb effect of the mixer (the IOP / SPU side of `SetReverbEx` and `snd_AutoReverb`).
#[derive(Clone, Debug)]
pub struct ReverbFx {
    /// `RC_REVERB=0` (a developer switch for A/B captures): no wet output, nothing processed.
    pub enabled: bool,
    pub kind: u8,
    pub delay: u8,
    pub feedback: u8,
    /// The effect volume now and the glide's target (0..0x7fff units).
    pub depth: f32,
    target: f32,
    step: f32,
    glide_left: u32,
    lp: [f32; 2],
    net: Option<Network>,
}

impl Default for ReverbFx {
    fn default() -> Self { ReverbFx { enabled: true, kind: mode::OFF, delay: 0, feedback: 0, depth: 0.0, target: 0.0, step: 0.0, glide_left: 0, lp: [0.0; 2], net: None } }
}

/// `snd_AutoReverb`'s glide in output samples: `delta` 989snd ticks of 200 samples [M: the unit is inferred].
fn glide_samples(delta: u16) -> u32 { delta as u32 * super::SAMPLES_PER_IOP_TICK as u32 }

impl ReverbFx {
    /// `SetReverbEx(core, type, depth, delay, feedback)`: a new type (or a new echo delay) rebuilds the effect (the
    /// tail is cut, as libsd re-initialises the work area); the depth is set at once; type 0 switches it off.
    pub fn set(&mut self, kind: u8, depth: i32, delay: u8, feedback: u8) {
        let rebuild = kind != self.kind || self.net.is_none() || ((kind == mode::ECHO || kind == mode::DELAY) && delay != self.delay);
        (self.kind, self.delay, self.feedback) = (kind, delay, feedback);
        if rebuild {
            self.net = Network::new(kind, delay, feedback);
            self.lp = [0.0; 2];
        } else if let Some(Network::Echo { feedback: fb, .. }) = &mut self.net {
            *fb = if kind == mode::ECHO { feedback.min(127) as f32 / 128.0 } else { 0.0 };
        }
        self.depth = depth.clamp(0, 0x7fff) as f32;
        self.target = self.depth;
        self.glide_left = 0;
    }

    /// `snd_AutoReverb(core, depth, delta, channels)`: glide the depth linearly to `depth` over `delta` ticks.
    pub fn auto(&mut self, depth: i32, delta: u16) {
        self.target = depth.clamp(0, 0x7fff) as f32;
        let n = glide_samples(delta).max(1);
        self.step = (self.target - self.depth) / n as f32;
        self.glide_left = n;
    }

    /// Is anything audible coming out (a type set and enabled)?
    pub fn active(&self) -> bool { self.enabled && self.net.is_some() }

    /// One output sample: `send` (the reverb-flagged voices' sum, left / right) in, the wet signal out (already
    /// scaled by the depth), to add to the dry mix.
    pub fn run(&mut self, send: [i32; 2]) -> [i32; 2] {
        if !self.enabled { return [0, 0]; }
        let Some(net) = self.net.as_mut() else { return [0, 0] };
        if self.glide_left > 0 {
            self.glide_left -= 1;
            self.depth = if self.glide_left == 0 { self.target } else { self.depth + self.step };
        }
        let x: [f32; 2] = std::array::from_fn(|ch| {
            self.lp[ch] = send[ch] as f32 + (self.lp[ch] - send[ch] as f32) * INPUT_POLE;
            self.lp[ch]
        });
        let y = net.run(x);
        let g = self.depth / 32768.0;
        y.map(|v| (v * g).round() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_box(kind: u8, depth: i32) -> ReverbBox {
        // A box of half size 2 centred at (10, 0, 0), local x along world x.
        let inverse = [[0.5, 0.0, 0.0, 0.0], [0.0, 0.5, 0.0, 0.0], [0.0, 0.0, 0.5, 0.0]];
        ReverbBox { instance: 7, center: [10.0, 0.0, 0.0], inverse, kind, delay: 0, feedback: 0, depth, inside: false }
    }

    #[test]
    fn box_ramps_and_exits() {
        let mut b = cube_box(mode::STUDIO_C, 5000);
        let mut r = ReverbRequest::default();
        // Outside and never inside: nothing.
        b.update([5.0, 0.0, 0.0], &mut r);
        assert_eq!((r.kind, r.depth, r.dirty), (0, 0, 0));
        // −x face: depth 0, the type set.
        b.update([8.0, 0.0, 0.0], &mut r);
        assert_eq!((r.kind, r.depth, r.dirty, r.owner), (mode::STUDIO_C, 0, dirty::TYPE, Some(7)));
        assert_eq!(r.command(), Some(SndCommand::SetReverb { kind: mode::STUDIO_C, depth: 0, delay: 0, feedback: 0 }));
        // Halfway: trunc(5000·1·0.5); a depth change alone glides.
        b.update([10.0, 0.0, 0.0], &mut r);
        assert_eq!(r.depth, 2500);
        assert_eq!(r.command(), Some(SndCommand::AutoReverb { depth: 2500, delta: 12, channels: 3 }));
        // Out through +x: the full depth stays.
        b.update([12.5, 0.0, 0.0], &mut r);
        assert_eq!((r.kind, r.depth, r.owner), (mode::STUDIO_C, 5000, None));
        assert!(!b.inside);
        assert_eq!(r.command(), Some(SndCommand::AutoReverb { depth: 5000, delta: 12, channels: 3 }));
        // Back in and out through −x: off.
        b.update([11.0, 0.0, 0.0], &mut r);
        b.update([7.0, 0.0, 0.0], &mut r);
        assert_eq!((r.kind, r.depth), (mode::OFF, 0));
        assert_eq!(r.command(), Some(SndCommand::SetReverb { kind: 0, depth: 0, delay: 0, feedback: 0 }));
        assert_eq!(r.command(), None);
    }

    #[test]
    fn dirty_priorities_and_checkpoint() {
        let mut r = ReverbRequest::default();
        r.set(None, mode::HALL, 3000, 0, 0);
        r.dirty |= dirty::OFF;
        assert_eq!(r.command(), Some(SndCommand::SetReverb { kind: 0, depth: 0, delay: 0, feedback: 0 }));
        r.dirty |= dirty::RESEND;
        assert_eq!(r.command(), Some(SndCommand::SetReverb { kind: mode::HALL, depth: 3000, delay: 0, feedback: 0 }));
        // Unchanged values set nothing dirty.
        r.set(Some(1), mode::HALL, 3000, 0, 0);
        assert_eq!(r.command(), None);
        r.checkpoint();
        r.set(None, mode::OFF, 0, 0, 0);
        let _ = r.command();
        r.restore();
        assert_eq!((r.kind, r.depth, r.dirty), (mode::HALL, 3000, 7));
        let p = EnvSamplePoint { reverb_enable: 1, reverb_type: mode::STUDIO_C, reverb_depth: 1500, ..Default::default() };
        r.apply_env_point(&p);
        assert_eq!(r.command(), Some(SndCommand::SetReverb { kind: mode::STUDIO_C, depth: 1500, delay: 0, feedback: 0 }));
        let off = EnvSamplePoint { reverb_enable: 0, reverb_type: mode::HALL, ..Default::default() };
        r.apply_env_point(&off);
        assert_eq!(r.command(), None);
    }

    #[test]
    fn env_point_radius() {
        let pt = |x: f32| EnvSamplePoint { position: [x, 0.0, 0.0], ..Default::default() };
        let pts = [pt(9.0), pt(-5.0), pt(5.0)];
        assert_eq!(nearest_env_point(&pts, [0.0; 3]), Some(1));
        assert_eq!(nearest_env_point(&pts, [20.0, 0.0, 0.0]), None);
        assert_eq!(nearest_env_point(&pts, [17.0, 0.0, 0.0]), Some(0));
    }

    /// The decay: an impulse through each preset falls by ≈ 60 dB over its RT60 (energy in 50 ms windows).
    #[test]
    fn presets_decay_at_their_rt60() {
        for kind in [mode::ROOM, mode::STUDIO_A, mode::STUDIO_B, mode::STUDIO_C, mode::HALL, mode::SPACE, mode::PIPE] {
            let p = preset(kind).unwrap();
            let mut fx = ReverbFx::default();
            fx.set(kind, 0x7fff, 0, 0);
            let n = (p.rt60 * RATE * 1.3) as usize;
            let mut out = Vec::with_capacity(n);
            for i in 0..n { out.push(fx.run(if i == 0 { [20000, 20000] } else { [0, 0] })[0] as f64); }
            let win = |t: f32| {
                let a = (t * RATE) as usize;
                out[a..a + 2400].iter().map(|x| x * x).sum::<f64>().max(1e-9)
            };
            let early = win(0.05 + p.loops_ms[1] / 1000.0);
            let late = win(p.rt60 * 0.5 + 0.05 + p.loops_ms[1] / 1000.0);
            let db = 10.0 * (early / late).log10();
            // Half the RT60 later: about 30 dB down (the damping makes it a little more).
            assert!((20.0..50.0).contains(&db), "{}: {db:.1} dB", mode::NAMES[kind as usize]);
        }
    }

    #[test]
    fn off_is_silent_and_glide_is_linear() {
        let mut fx = ReverbFx::default();
        assert_eq!(fx.run([10000, 10000]), [0, 0]);
        fx.set(mode::STUDIO_B, 0, 0, 0);
        assert!(fx.active());
        fx.auto(2400, AUTO_DELTA);
        for _ in 0..1200 { fx.run([0, 0]); }
        assert!((fx.depth - 1200.0).abs() < 1.0, "{}", fx.depth);
        for _ in 0..1200 { fx.run([0, 0]); }
        assert_eq!(fx.depth, 2400.0);
        fx.set(mode::OFF, 0, 0, 0);
        assert!(!fx.active());
        let mut e = ReverbFx::default();
        e.set(mode::ECHO, 0x4000, 3, 64);
        let mut first = None;
        for i in 0..4000 {
            let y = e.run(if i == 0 { [16000, 0] } else { [0, 0] });
            if first.is_none() && y[0] != 0 { first = Some(i); }
        }
        assert_eq!(first, Some(echo_delay_samples(3)));
    }
}
