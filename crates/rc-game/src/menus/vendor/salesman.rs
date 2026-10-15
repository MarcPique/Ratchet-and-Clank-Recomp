//! The salesman on the vendor's right-hand screen (docs/plan/interaction.md §9.3): the class-12 moby, a 3D
//! character (92 joints) whose ten sequences come from the global lump `vendor.bin` (toc 0x198), and his voice lines
//! (`vendor_audio`, toc 0x1a0: stream ids 10000 + n). Level01: created in `VendorModeUpdate` 0x2b03b8 once the read
//! started by `DrawWorld_Mode5` is done; behaviour `FUN_002aee20` (per frame) and `FUN_002af248` (the line requests).
//!
//! **Voice** (the dialogue player 0x151720): a request stores the stream id in 0x1516ec and sets 0x161190; the stream
//! loads and waits in state 3 (0x15172a); the talk sequence starts when it is ready and the line itself starts
//! (`continue_audio_stream_if_ready` 0x279e78: state 4) at a key time of the talk sequence ([`VendorLayout::talk_keys`]:
//! frame 6..10 of it), so the lips follow the line. The port's stream is ready on the frame after the request [L:
//! the disc read time is not modelled].
//!
//! **Lines**: id = 10000 + 6·(3k + c) + v with the set k = `randi(2)`, the kind c (0 greeting on creation, 1 idle
//! every 600 frames, 2 a remark on a cursor move: 1 in 4, at most one per 360 frames, only while he idles) and v =
//! language 0x15ed88 − 1 (≥ 0; the files hold 0 English, 1 French, 2 German, 3 Spanish, 4 Italian).
//!
//! **States** (+0x20 of the salesman): 10 greeting (seq 1, then the talk seq 3k+4 once ready), 11 / 12 wait for a
//! remark / idle line to load (then seq 3k+6 / 3k+5), 4..6 talking (the line starts at the key time; when the
//! sequence ends, seq 2 and state 2), 2 idle (seq 2; after 600 frames seq 3, state 3), 3 (when seq 3 ends, seq 0,
//! state 0), 0 idle (after 600 frames an idle line, state 12). Blends are `ticks(18)` (12 into a greeting's talk).
//! Native `f32`.

use rc_formats::moby_anim::{self, AnimState, MobyAnimClass, MobyFrame};

/// The class of the salesman moby.
pub const CLASS: i16 = 12;
/// Stream ids of the voice lines (`vendor_audio` entry = id − `VOICE_BASE`).
pub const VOICE_BASE: i32 = 10000;
/// Frames between idle actions (`ticks(600)`), between remarks (`ticks(360)`).
const IDLE_TICKS: i32 = 600;
const REMARK_TICKS: i64 = 360;

/// The dialogue stream state 0x15172a as the salesman sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stream {
    #[default]
    Idle,
    /// Requested this frame (loads until the next).
    Loading,
    /// 3: loaded, waiting to be continued.
    Ready,
    /// 4: playing.
    Playing,
}

/// What the salesman asks of the audio this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SalesmanOut {
    /// Start this stream id now (`continue_audio_stream_if_ready`).
    pub start_voice: Option<i32>,
}

/// The salesman (the moby at 0x1ca964 and the globals 0x1ca970..0x1ca97c, 0x161190 / 0x161194).
#[derive(Clone, Debug, PartialEq)]
pub struct Salesman {
    pub anim: AnimState,
    pub snapshot: Option<MobyFrame>,
    /// +0x20.
    pub state: u8,
    /// 0x1ca974: frames in the idle states.
    pub timer: i32,
    /// 0x1ca97c: the line set k.
    pub k: i32,
    /// 0x1ca970: the frame counter at the last remark.
    pub last_remark: i64,
    /// 0x1ca978: language − 1.
    pub lang: i32,
    /// 0x161190 (a line was requested) / 0x161194 (it is ready to start).
    pub requested: bool,
    pub ready: bool,
    /// 0x1516ec: the requested stream id.
    pub voice: i32,
    pub stream: Stream,
}

fn ticks(n: i32) -> i32 { n }

impl Salesman {
    /// `InitMobyInstance(0x1ca954, 12)` and `FUN_002af248(1)`: the greeting. `rng` is the game's stream.
    pub fn new(class: &MobyAnimClass, lang: i32, rng: &mut crate::rng::Rng) -> Salesman {
        let mut s = Salesman {
            anim: AnimState::spawn(class),
            snapshot: None,
            state: 0,
            timer: 0,
            k: 0,
            last_remark: 0,
            lang: (lang - 1).max(0),
            requested: false,
            ready: false,
            voice: 0,
            stream: Stream::Idle,
        };
        moby_anim::hard_cut(&mut s.anim, class, 0, 0);
        s.request(class, 1, rng);
        s
    }

    fn blend(&mut self, class: &MobyAnimClass, seq: u8, ticks: i32) {
        moby_anim::set_sequence(&mut self.anim, class, seq, 0, ticks, &mut self.snapshot);
    }

    fn line(&mut self, kind: i32) {
        self.voice = VOICE_BASE + 6 * (3 * self.k + kind) + self.lang;
        self.requested = true;
        self.ready = false;
        self.stream = Stream::Loading;
    }

    /// `FUN_002af248(kind)`: 1 greeting, 0 a remark on a cursor move, 2 / 3 a purchase (the idle timer restarts).
    /// `frame` is the vsync counter 0x15f3f8.
    pub fn request(&mut self, class: &MobyAnimClass, kind: i32, rng: &mut crate::rng::Rng) { self.request_at(class, kind, rng, 0) }

    pub fn request_at(&mut self, class: &MobyAnimClass, kind: i32, rng: &mut crate::rng::Rng, frame: i64) {
        match kind {
            1 => {
                self.k = rng.randi(2);
                self.line(0);
                if self.anim.seq_b != 1 { self.blend(class, 1, 10); }
                self.state = 10;
            }
            0 => {
                if !(self.state == 2 || self.state == 0) { return; }
                if rng.randi(4) != 0 { return; }
                if self.last_remark + REMARK_TICKS >= frame { return; }
                self.last_remark = frame;
                self.k = rng.randi(2);
                self.line(2);
                self.state = 11;
            }
            2 | 3 if self.state == 2 || self.state == 0 => self.timer = 0,
            _ => {}
        }
    }

    /// The stream's state machine between frames: a requested line is ready one frame after the request.
    fn stream_step(&mut self) {
        if self.stream == Stream::Loading { self.stream = Stream::Ready; }
    }

    /// `MobyAnimKeyTime` 0x263920: the key time (1/16 frames of the source timeline) the animation is at.
    pub fn key_time(&self, class: &MobyAnimClass) -> f32 {
        let s = &self.anim;
        let time = |seq: u8, frame: u8| -> f32 {
            if seq == moby_anim::SNAPSHOT_SEQ { return self.snapshot.as_ref().map_or(0.0, |f| f.header.time as f32); }
            class.frame(seq, frame).map_or(0.0, |f| f.header.time as f32)
        };
        let a = if s.seq_a == moby_anim::SNAPSHOT_SEQ { time(s.seq_b, s.frame_b) } else { time(s.seq_a, s.frame_a) };
        if s.t == 0.0 {
            a * 0.0625
        } else if s.seq_a == s.seq_b && s.frame_a <= s.frame_b {
            (a * (1.0 - s.t) + time(s.seq_b, s.frame_b) * s.t) * 0.0625
        } else {
            a * 0.0625 + s.t
        }
    }

    /// `FUN_002765b0(time, m)`: the key time has just reached `time` (within one step of speed · rate).
    fn at_key(&self, class: &MobyAnimClass, time: f32) -> bool {
        let r4 = |x: f32| (x * 10000.0).round() / 10000.0;
        let kt = self.key_time(class);
        time <= kt && r4(kt - time) < r4(self.anim.speed * self.anim.rate)
    }

    fn done(&self) -> bool { self.anim.flags & 2 != 0 }

    /// One frame: `MobyAnimAdvance`, then `FUN_002aee20`. `talk_keys` = [`super::layout::VendorLayout::talk_keys`].
    pub fn frame(&mut self, class: &MobyAnimClass, talk_keys: &[[i32; 2]; 3]) -> SalesmanOut {
        let mut out = SalesmanOut::default();
        self.stream_step();
        moby_anim::advance(&mut self.anim, class);
        let k = self.k.clamp(0, 1) as usize;
        match self.state {
            0 => {
                if self.timer <= ticks(IDLE_TICKS) {
                    self.timer += 1;
                    return out;
                }
                self.line(1);
                if self.anim.seq_b != 2 { self.blend(class, 2, ticks(18)); }
                self.state = 12;
                self.timer = 1;
            }
            2 => {
                if self.timer <= ticks(IDLE_TICKS) {
                    self.timer += 1;
                    return out;
                }
                if self.anim.seq_b != 3 { self.blend(class, 3, ticks(18)); }
                self.state = 3;
                self.timer = 1;
            }
            3 => {
                if !self.done() { return out; }
                if self.anim.seq_b != 0 { self.blend(class, 0, ticks(18)); }
                self.state = 0;
            }
            4..=6 => {
                let key = talk_keys[(self.state - 4) as usize][k] as f32;
                if self.at_key(class, key) && self.ready {
                    self.ready = false;
                    if self.stream == Stream::Ready {
                        self.stream = Stream::Playing;
                        out.start_voice = Some(self.voice);
                    }
                }
                if self.done() {
                    if self.anim.seq_b != 2 { self.blend(class, 2, ticks(18)); }
                    self.state = 2;
                    self.timer = 0;
                }
            }
            10 => {
                if self.stream == Stream::Ready && self.requested {
                    self.requested = false;
                    self.ready = true;
                }
                if !self.done() { return out; }
                let seq = (3 * self.k + 4) as u8;
                if self.anim.seq_b != seq { self.blend(class, seq, ticks(12)); }
                self.state = 4;
            }
            11 | 12 => {
                if self.stream != Stream::Ready || !self.requested { return out; }
                self.ready = true;
                self.requested = false;
                let (seq, st) = if self.state == 11 { (3 * self.k + 6, 6) } else { (3 * self.k + 5, 5) };
                if self.anim.seq_b != seq as u8 { self.blend(class, seq as u8, ticks(18)); }
                self.state = st;
            }
            _ => {}
        }
        out
    }
}
