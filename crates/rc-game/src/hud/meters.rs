//! The meter elements of the slot machine (docs/plan/hud_text.md §4.2): the tank meters of slot 4 (the oxygen
//! meter, the Morph-o-Ray's meter, the Suck Cannon's held count), the nearby-bolt alert of slot 7, Giant Clank's
//! energy bar (slot 0) and the boss meter of slot 6. Addresses are level01.elf (the boss draw: level18 0x23cd60, the
//! same code in levels 06 / 07 / 13). The coverage table of every function is in `crate::hud`'s module doc.

use super::{icon, scale_ticks, tween_color, Draw, HudState, Rot, SCREEN_H};
use rc_formats::font::Font;

/// gp−0x74b0.. 0x15f750 / 0x15f754 / 0x15f758: the bubble radii (the first 10 bubbles, the next, the rest).
pub const BUBBLE_R: [f32; 3] = [6.0, 4.5, 3.0];
/// 0x15f75c: the bubbles' vertical bands.
const BANDS: i32 = 6;
/// 0x15f760 / 0x15f764: the tank's size (the draws write it into the slot's +0x58 / +0x5c).
pub const TANK_W: i32 = 42;
pub const TANK_H: i32 = 192;
/// 0x15f768 / 0x15f76c: the bubbles' area as a fraction of the tank (x, y).
const TANK_SX: f32 = 0.5;
const TANK_SY: f32 = 0.9;
/// The bubbles (0x17eae0: 100 × {x, y, vx, vy}, in a ±17 × ±116 box), shared by the slot-4 tank meters.
pub const BUBBLES: usize = 100;
/// The oxygen fill (0x80684c2f) and the Suck Cannon's fill (0x80829e00) and lines (0x80000000).
const OXYGEN_FILL: u32 = 0x8068_4c2f;
const SUCK_FILL: u32 = 0x8082_9e00;
const BLACK: u32 = 0x8000_0000;
/// The low-air warning: Ratchet's class sound 11 (`0x236738(0xb, 0)`).
pub const LOW_AIR_SOUND: i32 = 11;
/// 0x15f7f0: the bolt alert's ramp steps; 0x15f8f8 / 0x15f8fc bar alpha / half width, 0x15f900 bar x, 0x15f904 /
/// 0x15f908 the "!" offset, 0x15f90c / 0x15f910 its colour tween, 0x15f914 its pulse (ticks), 0x15f918 the text.
const ALERT_STEPS: i32 = 8;
const ALERT_BAR_A: i32 = 80;
const ALERT_BAR_W: i32 = 16;
const ALERT_BAR_X: i32 = -28;
const ALERT_TEXT_D: (i32, i32) = (-44, 6);
const ALERT_FROM: u32 = 0x0000_00ff;
const ALERT_TO: u32 = 0x8000_00ff;
const ALERT_PULSE: i32 = 45;
/// Icon ids of the meters.
pub const ICON_TANK: u16 = 30015;
pub const ICON_MORPH: u16 = 30003;
pub const ICON_ALERT: u16 = 30010;
pub const ICON_GIANT: u16 = 30040;
pub const ICON_BOSS: u16 = 30050;
/// 0x15f8e8: the boss meter's centre x; 0x15f8f0 / 0x15f8f4: the end segments' squeeze.
const BOSS_X: i32 = 276;
const BOSS_SQUEEZE: [f32; 2] = [0.7, 0.7];
/// Level 02's counters (0x23cda8; level02 .data 0x15f91c / 0x15f920 / 0x15f944 / 0x15f948 / 0x15f950 / 0x15f954 /
/// 0x15f958 / 0x15f95c): the ramp divisors, the left / right x, the bar length (max < 10 / ≥ 10) and the count's
/// colour tween. The bar icon is the shared 0x7580 ([`icon::BAR`]); the format is "%d".
const COUNTER_STEPS: i32 = 8;
const COUNTER_X: [i32; 2] = [24, 480];
const COUNTER_LEN: [i32; 2] = [48, 70];
const COUNTER_TEXT_FROM: u32 = 0x00e0_8080;
const COUNTER_TEXT_TO: u32 = 0x80e0_8080;

/// The bubble radius by index: the inits (0x24b900, 0x24c1a8) switch at 10 and 20, the updates (0x24baf0, 0x24c398)
/// at 10 and 30 (both kept as the code has them).
fn radius(n: usize, second_until: usize) -> f32 {
    if n < 10 {
        BUBBLE_R[0]
    } else if n < second_until {
        BUBBLE_R[1]
    } else {
        BUBBLE_R[2]
    }
}

impl HudState {
    /// The bubbles of 0x24b900 / 0x24c1a8 (after `0x24b418`): bubble n of radius r (switching at 10 / 20) at a random
    /// x in ±(17 − r) and a random y in band n mod 6 of (r − 116 .. 116 − r), with a velocity of three draws each
    /// (x ±0.4, y ±0.6), drawn x, y, vx, vy, vx, vy, vx, vy (`randf` 0x26c9c8).
    pub(super) fn bubbles_init(&mut self) {
        for n in 0..BUBBLES {
            let r = radius(n, 20);
            let band = (232.0 - (r + r)) / BANDS as f32;
            let lo = (r - 116.0) + (n as i32 % BANDS) as f32 * band;
            let hi = lo + band;
            let rng = &mut self.rng;
            let x = rng.randf(r - 17.0, 17.0 - r);
            let y = rng.randf(lo, hi);
            let mut v = [rng.randf(-0.4, 0.4), rng.randf(-0.6, 0.6)];
            for _ in 0..2 {
                v[0] += rng.randf(-0.4, 0.4);
                v[1] += rng.randf(-0.6, 0.6);
            }
            self.bubbles[n] = [x, y, v[0], v[1]];
        }
    }

    /// The bubbles' step of 0x24baf0 / 0x24c398: every other bubble (from `vsync mod 2`) moves by its velocity and
    /// bounces off ±(17 − r) / ±(116 − r) (r switching at 10 / 30).
    pub(super) fn bubbles_step(&mut self) {
        let mut n = (self.vsync % 2) as usize;
        while n < BUBBLES {
            let r = radius(n, 30);
            let (xb, yb) = (r - 17.0, r - 116.0);
            let b = &mut self.bubbles[n];
            b[0] += b[2];
            b[1] += b[3];
            if b[0] < xb {
                b[0] = xb;
                b[2] = -b[2];
            }
            if -xb < b[0] {
                b[0] = -xb;
                b[2] = -b[2];
            }
            if b[1] < yb {
                b[1] = yb;
                b[3] = -b[3];
            }
            if -yb < b[1] {
                b[1] = -yb;
                b[3] = -b[3];
            }
            n += 2;
        }
    }

    /// The tank meters' draw: 0x24bc40 (`oxygen`: the fill, the low-air blink and its sound, three bubble sizes) and
    /// 0x24c4e8 (the Morph-o-Ray: no fill, no blink, every bubble the first size). The tank (icon frame 0, or 1 while
    /// blinking) is `HudSpriteRot90` 42 × 192 at the slot's place; the bubbles (frame 2 / 1, `fun_00200080` in 1/16
    /// pixels) are drawn where they lie below the level `116 − 232·(10000 − shown)/10000`.
    pub(super) fn draw_tank(&mut self, i: usize, oxygen: bool, out: &mut Vec<Draw>) {
        self.slots[i].size = (TANK_W, TANK_H);
        let (x, y) = self.place(i);
        let s = self.slots[i];
        let (px, py) = (x + s.offset.0, y + s.offset.1);
        let mut blink = false;
        if oxygen {
            let x0 = px * 16 + TANK_W * 0xd0 / 64;
            let y0 = py * 16 + TANK_H * 0xa0 / 256;
            out.push(Draw::Rect16 { x0, y0, x1: x0 + TANK_W * 0x260 / 64, y1: y0 + TANK_H * 0xea0 / 256, rgba: OXYGEN_FILL });
            // A third or less left: the tank blinks (45 of every 60 frames lit) and each lit start beeps.
            if s.shown * 3 <= 10000 {
                let period = (scale_ticks(1) as f32 * 60.0) as i32;
                let lit = (0.75f32 * 60.0) as i32;
                let v = self.vsync as i32;
                let prev = v.wrapping_sub(1) % period < lit;
                let now = v % period < lit;
                if !prev && now { self.sounds.push(LOW_AIR_SOUND); }
                blink = now;
            }
        }
        let tank = self.assets.icon_frame(s.icon, blink as i32);
        out.push(Draw::Sprite { frame: tank, x: px, y: py, w: TANK_W, h: TANK_H, alpha: 0x80, rot: Rot::R90 });
        let (w, h) = (TANK_W as f32, TANK_H as f32);
        let (hw, hh) = (w * TANK_SX * 0.5, h * TANK_SY * 0.5);
        let used = (10000 - s.shown) as f32 / 10000.0;
        let mut c = [(hw + px as f32 + (1.0 - TANK_SX) * 0.5 * w) * 16.0, (hh + py as f32 + (1.0 - TANK_SY) * 0.5 * h) * 16.0];
        let k = [(hw / 17.0) * 16.0, (-hh / 116.0) * 16.0];
        let mut level = 116.0 - (used + used) * 116.0;
        let steps = [BUBBLE_R[0] * 16.0, (BUBBLE_R[1] - BUBBLE_R[0]) * 16.0, (BUBBLE_R[2] - BUBBLE_R[1]) * 16.0];
        let frame = self.assets.icon_frame(s.icon, if oxygen { 2 } else { 1 });
        let mut size = 0;
        for n in 0..BUBBLES {
            let change = match n {
                0 => Some(0),
                10 if oxygen => Some(1),
                30 if oxygen => Some(2),
                _ => None,
            };
            if let Some(k) = change {
                c[0] -= steps[k];
                c[1] -= steps[k];
                level += if k == 0 { BUBBLE_R[0] } else { BUBBLE_R[k] - BUBBLE_R[k - 1] };
                size = (BUBBLE_R[k] * 32.0) as i32;
            }
            let b = self.bubbles[n];
            if b[1] <= level {
                let e = [b[0] * k[0] + c[0], b[1] * k[1] + c[1]];
                out.push(Draw::Sprite16 { frame, x: e[0] as i32, y: e[1] as i32, w: size, h: size, alpha: 0x80 });
            }
        }
    }

    /// 0x24ce50: the generic count (0x24b538), max 5 (10 with the gold cannon 0x13e529); while the Suck Cannon (item 9)
    /// is the hand item, on foot, the timer is held at `ScaleTicks(120) + 30`, else it is cut to 30.
    pub(super) fn update_suck(&mut self, i: usize) {
        self.update_counting(i);
        let gold = self.inputs.suck_gold;
        let (held, body) = (self.inputs.held_item, self.inputs.body);
        let s = &mut self.slots[i];
        s.max = if gold { 10 } else { 5 };
        if held == 9 && body == 0 {
            s.timer = scale_ticks(120) + 30;
        } else if s.timer >= 0x1f {
            s.timer = 0x1e;
        }
    }

    /// 0x24ced8: the tank's black inside, the fill from the bottom `shown / max` of the way up (0x80829e00), a black line
    /// 2 pixels high at every held moby's mark, then the tank (`HudSpriteRot90`, icon frame 0).
    pub(super) fn draw_suck(&mut self, i: usize, out: &mut Vec<Draw>) {
        self.slots[i].size = (TANK_W, TANK_H);
        let (x, y) = self.place(i);
        let s = self.slots[i];
        let (px, py) = (x + s.offset.0, y + s.offset.1);
        let x0 = px * 16 + TANK_W * 0xd0 / 64;
        let y0 = py * 16 + TANK_H * 0xa0 / 256;
        let x1 = x0 + TANK_W * 0x260 / 64;
        out.push(Draw::Rect16 { x0, y0, x1, y1: y0 + TANK_H * 0xea0 / 256, rgba: BLACK });
        let div = s.max << 8;
        if div != 0 {
            let base = py * 16 + TANK_H * 0xf40 / 256;
            out.push(Draw::Rect16 { x0, y0: base, x1, y1: base - (s.shown * TANK_H * 0xea0) / div, rgba: SUCK_FILL });
            for k in 1..s.shown {
                let yk = base - (k * TANK_H * 0xea0) / div;
                out.push(Draw::Rect16 { x0, y0: yk - 0x10, x1, y1: yk + 0x10, rgba: BLACK });
            }
        }
        let tank = self.assets.icon_frame(s.icon, 0);
        out.push(Draw::Sprite { frame: tank, x: px, y: py, w: TANK_W, h: TANK_H, alpha: 0x80, rot: Rot::R90 });
    }

    /// 0x24b458: the generic init with the size 32 × 32.
    pub(super) fn init_alert(&mut self, i: usize) {
        let s = &mut self.slots[i];
        s.offset = (0, 0);
        s.timer = scale_ticks(180) + 30;
        s.size = (0x20, 0x20);
        self.init_value(i);
    }

    /// 0x24ee20: while the alert flag 0x141398 is set the timer is held at `ScaleTicks(10)` and slide then alpha ramp up
    /// (8 steps); otherwise timer 0, +0x6c = 1 while alpha then slide ramp down, −6 once both are 0.
    pub(super) fn update_alert(&mut self, i: usize) {
        let on = self.inputs.bolt_alert;
        let s = &mut self.slots[i];
        if on {
            s.timer = scale_ticks(10);
            if s.slide < ALERT_STEPS {
                s.slide += 1;
            } else if s.alpha < ALERT_STEPS {
                s.alpha += 1;
            }
        } else {
            s.timer = 0;
            s.counter = 1;
            if s.alpha != 0 {
                s.alpha -= 1;
            } else if s.slide != 0 {
                s.slide -= 1;
            } else {
                s.counter = -6;
            }
        }
    }

    /// 0x24eed8: nothing as Giant Clank (body 2), while mounted (state 0x32) or with slide 0. At (anchor x, H − 50): the bar
    /// (alpha `80·s`, half width `16·s`) right of x − 28, the spinning bolt (icon 30031 frame `(tick mod 60)/2`, alpha
    /// `128·f`) at x − 32, and a large "!" at (x − 44, y + 6) pulsing `max(0, 4·sin(2π·(tick mod 45)/45 − π) − 3)·f` from
    /// 0x000000ff to 0x800000ff (shadow at +1, +1 from 0 to 0x80000000).
    #[allow(clippy::approx_constant)] // The code's own 6.28318 and 3.14159, not τ and π.
    pub(super) fn draw_alert(&self, i: usize, out: &mut Vec<Draw>) {
        let y = SCREEN_H - 0x32;
        let x = super::ANCHORS[i].0;
        if self.inputs.body == 2 || self.inputs.hero_state == crate::hero::scripted::MOUNTED { return; }
        let s = &self.slots[i];
        if s.slide == 0 { return; }
        let sf = (s.slide as f32 / ALERT_STEPS as f32).clamp(0.0, 1.0);
        let f = (s.alpha as f32 / ALERT_STEPS as f32).clamp(0.0, 1.0);
        let a_bolt = (f * 128.0) as i32;
        let bar_a = (ALERT_BAR_A as f32 * sf) as i32;
        let xb = x + ALERT_BAR_X;
        let hw = (ALERT_BAR_W as f32 * sf) as i32;
        let xl = xb - hw;
        let (cap, mid) = (self.assets.icon_frame(icon::BAR, 1), self.assets.icon_frame(icon::BAR, 0));
        out.push(Draw::Sprite { frame: cap, x: xb + 0x10, y, w: 0x20, h: 0x20, alpha: bar_a, rot: Rot::R180 });
        out.push(Draw::Sprite { frame: mid, x: xl, y, w: hw + 0x10, h: 0x20, alpha: bar_a, rot: Rot::None });
        out.push(Draw::Sprite { frame: cap, x: xl - 0x20, y, w: 0x20, h: 0x20, alpha: bar_a, rot: Rot::None });
        let bolt = self.assets.icon_frame(icon::BOLT, (self.inputs.tick % 60) as i32 >> 1);
        self.hud_frame(i, bolt, x - 0x20, y, 0, a_bolt, out);
        let (tx, ty) = (x + ALERT_TEXT_D.0, y + ALERT_TEXT_D.1);
        let p = scale_ticks(ALERT_PULSE);
        let phase = (self.inputs.tick % p.max(1) as u64) as f32 / p as f32;
        let pulse = ((phase * 6.28318 - 3.14159).sin() * 4.0 - 3.0).max(0.0);
        let shadow = tween_color(pulse * f, 0, 0x8000_0000);
        let colour = tween_color(pulse * f, ALERT_FROM, ALERT_TO);
        out.push(Draw::Text { font: Font::Large, x: tx + 1, y: ty + 1, rgba: shadow, text: b"!".to_vec() });
        out.push(Draw::Text { font: Font::Large, x: tx, y: ty, rgba: colour, text: b"!".to_vec() });
    }

    /// 0x24f248, Giant Clank's energy (slot 0): size 256 × 64 at the slot's place; the bar (icon 30040 frame 1,
    /// `HudSpriteSubRect`: the first `221·shown/max + 27` texels) under the frame (frame 0, 256 × 64) and the beam
    /// light (frame 2, 32 × 32, alpha 0x80 while the beam lockout 0x140986 runs, else 0).
    pub(super) fn draw_giant(&mut self, i: usize, out: &mut Vec<Draw>) {
        self.slots[i].size = (0x100, 0x40);
        let (x, y) = self.place(i);
        let s = self.slots[i];
        if s.max == 0 { return; }
        let w = (s.shown * 0xdd) / s.max + 0x1b;
        out.push(Draw::SpriteSub { frame: self.assets.icon_frame(ICON_GIANT, 1), x, y, w, h: 0x40, alpha: 0x80 });
        out.push(Draw::Sprite { frame: self.assets.icon_frame(ICON_GIANT, 0), x, y, w: 0x100, h: 0x40, alpha: 0x80, rot: Rot::None });
        let light = if self.inputs.beam_lock != 0 { 0x80 } else { 0 };
        out.push(Draw::Sprite { frame: self.assets.icon_frame(ICON_GIANT, 2), x, y, w: 0x20, h: 0x20, alpha: light, rot: Rot::None });
    }

    /// The boss meter's draw (level18 0x23cd60; levels 06 0x249738, 07, 13 the same code): `n` segments (3; 6 on levels
    /// 07 and 13, 7 on 18) centred on x 276 at y = H − 50 (level 13: 18, the top), nothing at slide 0. Back (icon 30050
    /// frame 3, `HudSpriteRot180` 32 × 18 per segment, alpha `128·slide/8`), the fill `32n·r` wide (r = shown / max, the
    /// first and last segments squeezed by 0.7) from red through yellow (r ≤ ½: 0x800000ff → 0x8000ffff) to green
    /// (0x8000ffff → 0x8000ff00), the segment frames (frame 1 × (n − 1), frame 2 the end) and the head (frames 4 then 0,
    /// 48 × 48 at x − 40, y − 8).
    pub(super) fn draw_boss(&self, i: usize, out: &mut Vec<Draw>) {
        let level = self.inputs.level;
        let mut y = SCREEN_H - 0x32;
        let n = match level {
            7 | 0xd => 6,
            0x12 => 7,
            _ => 3,
        };
        let s = &self.slots[i];
        if s.slide == 0 { return; }
        if level == 0xd { y = 0x12; }
        let sf = (s.slide as f32 * 0.125).clamp(0.0, 1.0);
        let a = (sf * 128.0) as i32;
        let x0 = BOSS_X - n * 0x10;
        let back = self.assets.icon_frame(ICON_BOSS, 3);
        for k in 0..n { out.push(Draw::Sprite { frame: back, x: x0 + 0x20 * k, y: y + 6, w: 0x20, h: 0x12, alpha: a, rot: Rot::R180 }); }
        let (mut w, mut colour) = (0, 0x8000_00ff);
        if s.shown != 0 {
            let mut r = s.shown as f32 / s.max as f32;
            let seg = 1.0 / n as f32;
            let last = (n - 1) as f32 * seg;
            if last < r {
                r = (r - last) * BOSS_SQUEEZE[0] + last;
            } else if r < seg {
                r = (r - seg) * BOSS_SQUEEZE[1] + seg;
            }
            w = ((n << 5) as f32 * r) as i32;
            colour = if 0.5 < r { tween_color((r + r) - 1.0, 0x8000_ffff, 0x8000_ff00) } else { tween_color(r + r, 0x8000_00ff, 0x8000_ffff) };
        }
        // `fun_00200e08(…, 0)`: whole pixels, one pixel up and left of a sprite corner.
        let px = |v: i32| v * 16 - 8;
        out.push(Draw::Rect16 { x0: px(x0), y0: px(y + 8), x1: px(x0 + w), y1: px(y + 0x1a), rgba: colour });
        let mut xk = x0;
        for _ in 0..n - 1 {
            out.push(Draw::Sprite { frame: self.assets.icon_frame(ICON_BOSS, 1), x: xk, y, w: 0x20, h: 0x20, alpha: a, rot: Rot::None });
            xk += 0x20;
        }
        out.push(Draw::Sprite { frame: self.assets.icon_frame(ICON_BOSS, 2), x: xk, y, w: 0x20, h: 0x20, alpha: a, rot: Rot::None });
        for f in [4, 0] { out.push(Draw::Sprite { frame: self.assets.icon_frame(ICON_BOSS, f), x: x0 - 0x28, y: y - 8, w: 0x30, h: 0x30, alpha: a, rot: Rot::None }); }
    }

    /// Level 02's counters (0x23cda8): on whichever side of the screen the slot's anchor sits (left of x 257), a
    /// stretched bar behind the element's icon and the count. The slide counter (`fade[0]`) grows the bar and its
    /// alpha (`128·grow`, `·0.7`), the alpha counter (`fade[1]`) shades the count's colour; nothing at slide 0.
    /// The bar is 32 tall at y = H − 50: cap / middle (icon 0x7580) / flipped cap, the icon (`HudFrame`) then the
    /// count (regular font, right-aligned) with a black shadow.
    pub(super) fn draw_aridia_counter(&self, i: usize, out: &mut Vec<Draw>) {
        let s = &self.slots[i];
        if s.slide == 0 { return; }
        let grow = (s.slide as f32 / COUNTER_STEPS as f32).clamp(0.0, 1.0);
        let shade = (s.alpha as f32 / COUNTER_STEPS as f32).clamp(0.0, 1.0);
        // `hud_screen_position` (0x24b108): the anchor + flags, only to pick the side.
        let left = self.align(i).0 < 0x101;
        let x = COUNTER_X[if left { 0 } else { 1 }];
        let y = SCREEN_H - 0x32;
        let alpha = ((grow * 128.0) as i32 as f32 * 0.7) as i32;
        let len = if s.max < 10 { COUNTER_LEN[0] } else { COUNTER_LEN[1] };
        let bar = x + 4;
        let width = (len as f32 * grow) as i32;
        let cap = self.assets.icon_frame(icon::BAR, 1);
        let mid = self.assets.icon_frame(icon::BAR, 0);
        if left {
            out.push(Draw::Sprite { frame: cap, x: x - 0x1c, y, w: 0x20, h: 0x20, alpha, rot: Rot::None });
            out.push(Draw::Sprite { frame: mid, x: bar, y, w: width, h: 0x20, alpha, rot: Rot::None });
            out.push(Draw::Sprite { frame: cap, x: bar + width, y, w: 0x20, h: 0x20, alpha, rot: Rot::R180 });
        } else {
            out.push(Draw::Sprite { frame: cap, x: bar, y, w: 0x20, h: 0x20, alpha, rot: Rot::R180 });
            let px = bar - width;
            out.push(Draw::Sprite { frame: mid, x: px, y, w: width, h: 0x20, alpha, rot: Rot::None });
            out.push(Draw::Sprite { frame: cap, x: px - 0x20, y, w: 0x20, h: 0x20, alpha, rot: Rot::None });
        }
        let icon_x = if left { x } else { x - 0x16 };
        self.hud_frame(i, self.assets.icon_frame(s.icon, 0), icon_x, y, 0, 0x80, out);
        let bright = tween_color(shade, COUNTER_TEXT_FROM, COUNTER_TEXT_TO);
        let dark = tween_color(shade, 0, 0x8000_0000);
        let text = s.shown.to_string().into_bytes();
        let (tx, ty) = if left { (x + 0x42, y + 8) } else { (x - 0x1a, y + 8) };
        self.text_right(tx + 1, ty + 1, dark, &text, out);
        self.text_right(tx, ty, bright, &text, out);
    }
}
