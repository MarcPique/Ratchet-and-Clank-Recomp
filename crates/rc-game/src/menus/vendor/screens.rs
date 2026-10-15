//! The vendor's screens (`VendorDrawScreens` 0x2b3130, read from the disassembly; docs/plan/interaction.md §9.3):
//! where each of the six screens goes on the display and what its 512×128 render target holds.
//!
//! **Place.** Screen s takes three joints of the vendor, 4s, 4s + 1 and 4s + 2 (`MobyGetBoneMatrix` 0x264630: the
//! joint's pose translation × scale / 1024, turned by the vendor's rows, plus its position): p0 the corner, p1 and p2
//! along its two edges. With the margins m of the screen, the quad is c = p0 + â·m0 + b̂·m1, a = â·(|a| − 2·m2),
//! b = b̂·(|b| − 2·m3) (a = p1 − p0, b = p2 − p0). While powering on / off (8 frames) the quad shrinks about its
//! centre to f of its edges. `FUN_002adc38` projects c and c + a + b with the camera: an axis-aligned screen
//! rectangle, so a screen is always drawn flat, facing the camera, over the monitor's extent.
//!
//! **Target.** Each screen is drawn into a 512×128 target (cleared black) with the camera's view but zoom 1.0 and
//! the centre (256, 64) (`SetRenderToTextureView`), then its texels (0, 0)..(W − 1, H − 1) (W × H the unshrunk
//! rectangle) are drawn on the (shrunk) rectangle. The content is laid out in target pixels ([`content`]); the
//! engine draws the target's 3D part (the item model, the salesman) and composes.
//!
//! **Static** (`FUN_002b2cd8`, [`Statics`]): per screen a burst counter (+2 per frame; the ticker's always runs, at
//! alpha 0x30 between bursts) that draws FX 0x1a noise at a random offset, fading in, holding and fading out
//! (alpha `2·(0x80 − |c − 0x80|)` ≤ 0x80), and on screens 1..5 a scan bar (FX 0x1c) rolling down, fading in and out
//! the same way (`0x100 − |c − 0xfe|` ≤ 0x50); idle screens start a burst with probability 1/700 and a bar with 1/360 per frame (the game's `rand`).
//! Its draws sample with CLAMP_1 = 0 (`VU1_addGSregister(8, 0)`: REPEAT), so the texel ranges past the 32×32 noise
//! and the 16×16 bar tile (the engine sets it, crate::vendor_render). Native `f32`.

use super::layout::VendorLayout;
use crate::menus::MenuDraw;
use crate::hud::Draw;
use crate::rng::Rng;

pub type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: V3, k: f32) -> V3 { [a[0] * k, a[1] * k, a[2] * k] }
fn len(a: V3) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
fn norm_to(a: V3, l: f32) -> V3 {
    let n = len(a);
    if n == 0.0 { [0.0; 3] } else { scale(a, l / n) }
}
fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

/// The screens' kinds in order (jump table 0x20a570).
pub const TICKER: usize = 0;
pub const ITEM: usize = 1;
pub const SALESMAN: usize = 2;
pub const BUTTONS: usize = 3;
pub const PROMPT: usize = 4;
pub const STRIP: usize = 5;
/// The popup's target (the static's screen index 6).
pub const POPUP: usize = 6;
/// The render target size.
pub const TARGET: (i32, i32) = (512, 128);

/// A screen's quad in the world: corner and two edges.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quad {
    pub c: V3,
    pub a: V3,
    pub b: V3,
}

impl Quad {
    /// From the three joint points and the screen's margins.
    pub fn new(p: [V3; 3], m: [f32; 4]) -> Quad {
        let (a0, b0) = (sub(p[1], p[0]), sub(p[2], p[0]));
        let (la, lb) = (len(a0), len(b0));
        let c = add(add(p[0], norm_to(a0, m[0])), norm_to(b0, m[1]));
        Quad { c, a: norm_to(a0, la - 2.0 * m[2]), b: norm_to(b0, lb - 2.0 * m[3]) }
    }

    /// Shrunk about its centre to `f` of each edge (the power-on / off).
    pub fn shrink(&self, f: f32) -> Quad {
        let (la, lb) = (len(self.a), len(self.b));
        let c = add(add(self.c, norm_to(self.a, la * (1.0 - f) * 0.5)), norm_to(self.b, lb * (1.0 - f) * 0.5));
        Quad { c, a: norm_to(self.a, la * f), b: norm_to(self.b, lb * f) }
    }

    pub fn far(&self) -> V3 { add(add(self.c, self.a), self.b) }

    /// The four corners in strip order (c, c + a, c + b, c + a + b).
    pub fn corners(&self) -> [V3; 4] { [self.c, add(self.c, self.a), add(self.c, self.b), self.far()] }
}

/// The camera the screens are projected with (the main view): eye, rows (forward, left, up), the projection's
/// tangents and the target size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: V3,
    pub rows: [V3; 3],
    pub tan_x: f32,
    pub tan_y: f32,
    pub size: (f32, f32),
}

impl View {
    /// The game view (zoom 0.63, NTSC 0.775) on the 512×416 draw buffer.
    pub fn game(eye: V3, rows: [V3; 3]) -> View { View { eye, rows, tan_x: 0.63, tan_y: 0.63 * 0.775, size: (512.0, 416.0) } }

    /// The render-to-texture view (`SetRenderToTextureView(1.0, …, 512, 128)`): same camera, zoom 1.0.
    pub fn target(&self) -> View { View { tan_x: 1.0, tan_y: 0.775, size: (TARGET.0 as f32, TARGET.1 as f32), ..*self } }

    /// Game pixel of a world point (x right, y down); None behind the camera.
    pub fn project(&self, p: V3) -> Option<[f32; 2]> {
        let d = sub(p, self.eye);
        let z = dot(d, self.rows[0]);
        if z <= 1e-4 { return None; }
        let (x, y) = (-dot(d, self.rows[1]), -dot(d, self.rows[2]));
        let (hw, hh) = (self.size.0 * 0.5, self.size.1 * 0.5);
        Some([hw + hw * x / (z * self.tan_x), hh + hh * y / (z * self.tan_y)])
    }

    /// `FUN_002adc38(p, q)`: the rectangle from the projection of `p` to that of `q` (x, y, w, h; w / h may be
    /// negative).
    pub fn rect(&self, p: V3, q: V3) -> Option<[f32; 4]> {
        let (a, b) = (self.project(p)?, self.project(q)?);
        Some([a[0], a[1], b[0] - a[0], b[1] - a[1]])
    }
}

/// A screen as drawn this frame: the full rectangle (texel extent W × H of the target) and the rectangle it is drawn
/// on (the power-on / off shrinks it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placed {
    pub full: [f32; 4],
    pub drawn: [f32; 4],
    pub quad: Quad,
}

impl Placed {
    /// Target texel (u, v) → display pixel (`DrawBoneQuads`: texels 0..W − 1 over the drawn width).
    pub fn to_screen(&self, u: f32, v: f32) -> [f32; 2] {
        let (tw, th) = ((self.full[2] - 1.0).max(1.0), (self.full[3] - 1.0).max(1.0));
        [self.drawn[0] + u * self.drawn[2] / tw, self.drawn[1] + v * self.drawn[3] / th]
    }
    pub fn texel_size(&self) -> (i32, i32) { (self.full[2] as i32, self.full[3] as i32) }
}

/// The screen placement: `joints(j)` = the world point of vendor joint j (`MobyGetBoneMatrix`), `f` the power factor
/// (1 at rest).
pub fn place(layout: &VendorLayout, s: usize, joints: &dyn Fn(usize) -> V3, view: &View, f: f32) -> Option<Placed> {
    let p = [joints(4 * s), joints(4 * s + 1), joints(4 * s + 2)];
    let q = Quad::new(p, layout.margins[s]);
    let full = view.rect(q.c, q.far())?;
    let (qd, drawn) = if f < 1.0 {
        let q2 = q.shrink(f);
        (q2, view.rect(q2.c, q2.far())?)
    } else {
        (q, full)
    };
    Some(Placed { full, drawn, quad: qd })
}

/// The glass quad of screen s (1..5; `VendorDrawGlassQuads` 0x2b3700): the unshrunk quad's corners, FX 0x19,
/// RGBA 0x80808080, UV (0, 0)..(1, 0.984).
pub fn glass(layout: &VendorLayout, s: usize, joints: &dyn Fn(usize) -> V3) -> Quad {
    Quad::new([joints(4 * s), joints(4 * s + 1), joints(4 * s + 2)], layout.margins[s])
}
pub const GLASS_FX: usize = crate::menus::screen_static::GLASS_FX;
pub const GLASS_V1: f32 = 0.984_375;

/// The static counters 0x1caba0[7] / 0x1cabc0[7].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statics {
    pub burst: [i32; 7],
    pub bar: [i32; 7],
}

/// One FX draw of the static into a target (target pixels; `additive` = ALPHA 0x68 with FIX = alpha, else 0x44).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxDraw {
    pub fx: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub u: i32,
    pub v: i32,
    pub u1: i32,
    pub v1: i32,
    pub rgba: u32,
    pub additive: bool,
}

pub const NOISE_FX: usize = crate::menus::screen_static::NOISE_FX;
pub const BAR_FX: usize = crate::menus::screen_static::BAR_FX;

impl Statics {
    /// `FUN_002b2cd8(w, h, s)` for this frame: the draws over the target's content (w × h = the drawn size).
    pub fn step(&mut self, w: f32, h: f32, s: usize, rng: &mut Rng) -> Vec<FxDraw> {
        let mut out = Vec::new();
        let c0 = self.burst[s];
        let run = if c0 != 0 {
            self.burst[s] = c0 + 2;
            true
        } else {
            s == 0
        };
        if run {
            let c = if s == 0 { self.burst[s].max(0x18) } else { self.burst[s] };
            let fx = rng.randi(200) as f32;
            let fy = rng.randi(200) as f32;
            // The shared curve (crate::menus::screen_static::burst_alpha: fade in over 32 frames, hold, fade out); the
            // ticker's floor 0x18 keeps it at 0x30.
            let alpha = crate::menus::screen_static::burst_alpha(c);
            // DrawBoneQuads(0, 0, w, h, fx, fy, (int)(w + fx), (int)(h + fy)): texels fx .. fx + (w + fx).
            let (u, v) = (fx as i32, fy as i32);
            out.push(FxDraw { fx: NOISE_FX, x: 0.0, y: 0.0, w, h, u, v, u1: u + (w + fx) as i32, v1: v + (h + fy) as i32, rgba: (alpha as u32) << 24 | 0x80_8080, additive: true });
            if self.burst[s] > 0xff { self.burst[s] = 0; }
        }
        if self.burst[s] == 0 && rng.randi(700) == 0 { self.burst[s] = 2; }
        if s > 0 {
            let c = self.bar[s];
            if c == 0 {
                if rng.randi(360) == 0 { self.bar[s] = 2; }
            } else {
                self.bar[s] = c + 2;
                // abs() again: fades in over 38 frames (4 → 0x50), holds, fades out over the last 38.
                let a = (0x100 - (c - 0xfe).abs()).min(0x50);
                let y = -((0x200 - self.bar[s]) as f32) * 0.031_25;
                let h2 = h + 16.0;
                out.push(FxDraw { fx: BAR_FX, x: 0.0, y, w, h: h2, u: 0, v: 0, u1: w as i32, v1: (h2 * 1.5) as i32, rgba: (a as u32) << 24 | 0x50_5050, additive: false });
                if self.bar[s] > 0x1ff { self.bar[s] = 0; }
            }
        }
        if s == POPUP {
            out.push(FxDraw { fx: GLASS_FX, x: 0.0, y: 0.0, w, h, u: 0, v: 0, u1: 0x40, v1: 0x40, rgba: 0x8080_8080, additive: false });
        }
        out
    }
}

/// The static draws as HUD FX quads (the texel range as `(u, v, tw, th)`).
pub fn fx_draws(fx: &[FxDraw], out: &mut Vec<MenuDraw>) {
    for d in fx {
        out.push(MenuDraw::Hud(Draw::FxQuad { fx: d.fx, x: d.x as i32, y: d.y as i32, w: d.w as i32, h: d.h as i32, u: d.u, v: d.v, tw: d.u1 - d.u, th: d.v1 - d.v, rgba: d.rgba }));
    }
}
