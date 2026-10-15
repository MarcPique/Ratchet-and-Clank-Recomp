//! The "screen" effects the game lays over its monitors and menu panels: grainy noise that fades in and out at
//! random, fine scan lines, and the glass glare. Two routines draw them, both with the same textures and the same
//! noise curve:
//!
//! * **Menu panels** (`fun_00223e28`, boot 0x223e28, level01 copy 0x297830; called by `PageMenuDraw` 0x28d080
//!   after the widgets, for every live frame-moby slot (enable array 0x1b2840, slot 6 only with Goodies) — every
//!   page menu: the pause pages, Options and its sub-pages, the Weapons / Gadgets pages (over their 3D views), the
//!   ship planet select). [`PanelStatic`], one per frame moby (its pvar +0x48 / +0x4c), so each panel bursts on
//!   its own. Read from the disassembly (the decompile drops the draws' colour and texture arguments).
//! * **The Gadgetron vendor's monitors** (`FUN_002b2cd8`, crate::menus::vendor::screens::Statics).
//!
//! **The noise** (FX 0x1a, 32×32, a quarter of its texels grey 0x02..0x6b, the rest black): a burst counter
//! c runs 2, 4, … 0x100 (+2 per frame); the alpha is `min(2·(0x80 − |c − 0x80|), 0x80)` (`subtract_integer_with_clamp`
//! 0x221110 is `abs`), so each burst fades in over 32 frames, holds for 64 and fades out over 32 (128 frames,
//! 2.1 s). It is drawn at a random texel offset every frame (two `rand() % 200`, the game's stream) with ALPHA_1
//! 0x68 and FIX = that alpha: **added** (`Cd + Cs·FIX/128`), so the grain brightens what is under it. An idle
//! panel starts a burst with probability 1/2000 per frame (the vendor's screens 1/700).
//!
//! **Texture wrap.** Both routines set CLAMP_1 = 0 (`VU1_addGSregister(8, 0)`: REPEAT) before these draws: their
//! texel ranges run far past the textures (the noise's offsets up to 199, the scan lines 1.5 texels per pixel of a
//! 16×16 texture), so they tile.
//!
//! Native `i32`; the draws are plain data for the engine's static layer (crate::hud_render, `Hud2dHook::statics`).

use crate::rng::Rng;

/// `GetEffectTex` indices.
pub const GLASS_FX: usize = 0x19;
pub const NOISE_FX: usize = 0x1a;
pub const BAR_FX: usize = 0x1c;

/// The noise alpha of burst counter `c` (both routines): `min(2·(0x80 − |c − 0x80|), 0x80)`.
pub fn burst_alpha(c: i32) -> i32 { ((0x80 - (c - 0x80).abs()) * 2).min(0x80) }

/// What a static draw samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticTex {
    /// FX texture (`GetEffectTex`).
    Fx(usize),
    /// HUD frame (`GetIconFrame`).
    Frame(usize),
}

/// One draw of the effect (game pixels, texels; CLAMP_1 = 0). `additive` = ALPHA_1 0x68 with FIX = the alpha byte
/// of `rgba` (`Cd + Cs·FIX/128`); otherwise ALPHA_1 0x44 (`(Cs − Cd)·As/128 + Cd`). `pass`: where it goes in the
/// engine's static layer (0 under the noise, 1 the noise, 2 over it): the panels do not overlap, so drawing every
/// panel's pass 0, then every pass 1, then every pass 2 is the game's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticDraw {
    pub tex: StaticTex,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub u: i32,
    pub v: i32,
    pub tw: i32,
    pub th: i32,
    pub rgba: u32,
    pub additive: bool,
    pub pass: u8,
}

/// The vignette `fun_00223e28` draws first: icon 0xe99e (59806) frame 7, a 128×128 black frame whose alpha rises
/// towards the edges.
pub const VIGNETTE_ICON: u16 = 0xe99e;
pub const VIGNETTE_FRAME: i32 = 7;

/// One panel's burst state (the frame moby's pvar +0x48 running, +0x4c counter). Zero when the frame mobys are
/// spawned [L: `CreateMoby` hands out a cleared pvar block].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PanelStatic {
    pub on: bool,
    pub c: i32,
}

impl PanelStatic {
    /// `fun_00223e28(moby)` on the panel rect (pvar +0x50..+0x5c: x, y, w, h). `vignette` = the frame of icon
    /// 0xe99e / 7 and its texel size; `pal` = 0x15ed80 (the visible bottom edge 0x1c0 instead of 0x1a0).
    pub fn draw(&mut self, rect: [i32; 4], pal: bool, vignette: (usize, (i32, i32)), rng: &mut Rng, out: &mut Vec<StaticDraw>) {
        let [x, y, w, h] = rect;
        let bottom = if pal { 0x1c1 } else { 0x1a1 };
        if !(x < 0x200 && x + w >= 0 && y < bottom && y + h >= 0) { return; }
        let d = |tex, (x, y, w, h), (u, v, tw, th), rgba, additive, pass| StaticDraw { tex, x, y, w, h, u, v, tw, th, rgba, additive, pass };
        // CLAMP_1 = 0, ALPHA_1 0x44; `fun_00200080(frame, x·16, y·16, w·16, h·16, 0x80)`: the whole frame, RGBA
        // 0x807f7f7f.
        let (frame, (fw, fh)) = vignette;
        out.push(d(StaticTex::Frame(frame), (x, y, w, h), (0, 0, fw, fh), 0x807f_7f7f, false, 0));
        if !self.on {
            if rng.randi(2000) == 0 {
                self.c = 0;
                self.on = true;
            }
        } else {
            self.c += 2;
            let u = rng.randi(200);
            let v = rng.randi(200);
            let a = burst_alpha(self.c);
            // ALPHA_1 0x68 | FIX << 32; `DrawTexturedQuad(x, y, w, h, u, v, w, h, 0x808080, FX 0x1a)`.
            out.push(d(StaticTex::Fx(NOISE_FX), (x, y, w, h), (u, v, w, h), (a as u32) << 24 | 0x80_8080, true, 1));
            if self.c >= 0x100 { self.on = false; }
        }
        // ALPHA_1 0x44: the scan lines, always, `DrawTexturedQuad(x, y, w, h, 0, 0, w, 3h >> 1, 0x50606060, FX 0x1c)`.
        out.push(d(StaticTex::Fx(BAR_FX), (x, y, w, h), (0, 0, w, (h * 3) >> 1), 0x5060_6060, false, 2));
        // The glass: `DrawTexturedQuad(x + 1, y + 1, w + dw, h + dh, 1, 1, 0x3e, 0x3e, 0x80808080, FX 0x19)`, the
        // size trimmed by 2 below 0x4c pixels, by 1 below 0x97.
        let trim = |n: i32| if n < 0x4c { -2 } else if n < 0x97 { -1 } else { 0 };
        out.push(d(StaticTex::Fx(GLASS_FX), (x + 1, y + 1, w + trim(w), h + trim(h)), (1, 1, 0x3e, 0x3e), 0x8080_8080, false, 2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_fades_in_holds_and_fades_out() {
        assert_eq!([2, 4, 0x40, 0x80, 0xc0, 0xc2, 0xfe, 0x100].map(burst_alpha), [4, 8, 0x80, 0x80, 0x80, 0x7c, 4, 0]);
    }

    #[test]
    fn panel_draws_vignette_noise_lines_and_glass() {
        let mut rng = Rng::new();
        let mut p = PanelStatic { on: true, c: 0x3e };
        let mut out = Vec::new();
        p.draw([155, 44, 203, 35], false, (300, (128, 128)), &mut rng, &mut out);
        assert_eq!(out.len(), 4);
        assert_eq!((out[0].tex, out[0].pass, out[0].rgba), (StaticTex::Frame(300), 0, 0x807f_7f7f));
        let n = out[1];
        assert_eq!((n.tex, n.additive, n.pass, n.rgba >> 24, n.tw, n.th), (StaticTex::Fx(NOISE_FX), true, 1, 0x80, 203, 35));
        assert!(n.u < 200 && n.v < 200);
        assert_eq!((out[2].tex, out[2].th, out[2].rgba), (StaticTex::Fx(BAR_FX), 52, 0x5060_6060));
        // 203 ≥ 0x97: full width; 35 < 0x4c: two rows less.
        assert_eq!((out[3].x, out[3].y, out[3].w, out[3].h, out[3].u, out[3].tw), (156, 45, 203, 33, 1, 0x3e));
        assert_eq!(p.c, 0x40);
        // Off screen (y ≥ 0x1a1 on NTSC): nothing, no rand.
        let before = rng;
        out.clear();
        p.draw([155, 0x1a1, 203, 35], false, (300, (128, 128)), &mut rng, &mut out);
        assert!(out.is_empty() && rng == before);
    }

    #[test]
    fn a_burst_ends_after_128_frames() {
        let mut rng = Rng::new();
        let mut p = PanelStatic { on: true, c: 0 };
        let mut frames = 0;
        while p.on {
            let mut out = Vec::new();
            p.draw([0, 0, 64, 32], false, (0, (128, 128)), &mut rng, &mut out);
            frames += 1;
        }
        assert_eq!((frames, p.c), (128, 0x100));
    }
}
