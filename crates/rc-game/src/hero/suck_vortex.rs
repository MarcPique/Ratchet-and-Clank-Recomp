//! **The Suck Cannon's vortex** (level01 `0x306510` reset, `0x306528` start, `0x3067d0` the per-tick update while
//! sucking, `0x307850` the fade, `0x3078b8` the collapse, draw callback `0x306158`; read from the decompiler output, the
//! lag clamp's lost argument from the disassembly at 0x306ee0). The globals it keeps (0x2008a0.., 0x1ff5e0.., gp−0x4d40..)
//! are [`Vortex`], the cannon's.
//!
//! * **The path**: 10 nodes from the mouth along the aim (Ratchet's aim rows 0x13f990, the view's in first person), each
//!   segment 1.7 long (0x161f04, set by the start): node 0 at the mouth; each next segment turns from where it was last
//!   tick toward the desired direction by a spring angle (`0x20cf28(0, θ, 0.06, 0.2, 0, &v_i)`, θ the angle between
//!   them), never trailing it by more than 18° (0.314159), the outer half (i > 5) blended back toward last tick's by
//!   i/10; the desired direction itself is the aim plus a small wobble (a 0.1 offset turned about the aim axis by
//!   `randf(45°, 315°)` more every 60 ticks, eased across the 60) and then each segment's own. Around node i a ring of
//!   20 points (the circle in the node's frame, radius `0.1 + 0.3·i + 0.1·sin(spin + 0.7·i)`, spin +0.1 rad a tick).
//! * **The strength** per node (0x200968, s16): a line of 17 (10 segments) from the mouth along the aim (flags 0x14,
//!   Ratchet ignored): clear → node 0 +4, each next 32 less; blocked at distance d → the nodes past `d/1.7` fade (capped
//!   at 64, −4 a tick), the ones before it 32 more each back to the mouth. The draw's alpha (0..64), the strands'
//!   spawn node (the furthest node at 64 or more) and the vacuum's reach (`0x307a50`: nodes above 7) come from it.
//! * **The strands** (0x1ff5e0, 200 × 0x18): at most 5 new ones a tick, each a smoke puff (particle type 23: size
//!   `randf(60000, 180000)`, spin 6, colour 0x7f404040, timed-fade phase 2 over 200) born at the furthest strong node that
//!   runs down the path at `randf(0.1, 0.4)` a tick, turning about it along the rings at `randf(0.1, 0.8)` points a tick,
//!   its timer held at 200 (100 on the second node, `(1 − f)·100` ≥ 60 on the first, where it heads straight for the
//!   mouth shrinking 5 % a tick), killed at the mouth; and up to 2 loose puffs a tick (spin 1..5, grow 1..1.03, colour
//!   0x404040) between the mouth ring and the strong node's ring, drifting outward 0.01 and up 0.02 a tick.
//! * **The draw** (`0x306158`, draw list 2, [`Vortex::quads`]): the tube between consecutive rings, 9 × 20
//!   `FastDrawQuadReal` quads facing the camera only (back faces skipped), FX 0x15, additive (ALPHA 0x48), colour
//!   0x404040 with each ring's strength as alpha (clamped 0..64; the mouth ring's inner edge 0x20, the last ring's
//!   outer 0), ST scrolling: s from `+0.001` a frame (wrapping at 7) plus 0.3 a ring, t from `+0.006` a ring a frame
//!   (halved by each collapse) plus 0.05 a quad.
//! * **When not sucking** (the cannon's states 1, 2, 4, 5: `0x307850`): the strengths fade (capped at 64, −4 a tick)
//!   and the tube is drawn while node 0's is above 0. **The collapse** (the stop of state 3, `0x3078b8`): the scroll
//!   slows by half, the next start re-inits, every strand's puff is let go (70 ticks, flung 0.03 outward plus 0.1 along
//!   its segment).
//!
//! Native `f32`; the rand draws are the game's, in its order. [L]: `FUN_00274ac8` turns the old segment *toward* the
//! desired direction (the tail trails the aim: a turn away would diverge); the lerps are `a + (b − a)·t`; the view's
//! second row in first person is `up × forward` (only rows 0 and 2 are read); the scroll advances once a tick (the
//! draw callback's state part, at registration).

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{add_rot, sub_rot};
use crate::moby_update::services::World;
use crate::particles::rec;
use crate::ps2v::Pf;

pub const NODES: usize = 10;
pub const RING: usize = 20;
pub const STRANDS: usize = 200;
/// 0x161f04: the segment length (set by the start).
pub const SEG: f32 = 1.7;
/// The draw's texture.
pub const FX: usize = 0x15;

type V3 = [f32; 3];
fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn scale(a: V3, k: f32) -> V3 { a.map(|x| x * k) }
fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn len(a: V3) -> f32 { dot(a, a).sqrt() }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
/// `FastVecNormalize(l, out, v)` (zero stays zero).
fn with_len(a: V3, l: f32) -> V3 { let n = len(a); if n == 0.0 { [0.0; 3] } else { scale(a, l / n) } }
fn lerp(t: f32, a: V3, b: V3) -> V3 { add(a, scale(sub(b, a), t)) }
/// `v·M` with the rows of M.
fn mul(v: V3, m: [V3; 3]) -> V3 { add(add(scale(m[0], v[0]), scale(m[1], v[1])), scale(m[2], v[2])) }
/// `FUN_00274ac8(angle, out, v, axis)`: `v` turned about `axis` (right-handed, Rodrigues).
fn rotate(v: V3, axis: V3, a: f32) -> V3 {
    if len(axis) < 1e-5 { return v; }
    let k = with_len(axis, 1.0);
    let (s, c) = a.sin_cos();
    add(add(scale(v, c), scale(cross(k, v), s)), scale(k, dot(k, v) * (1.0 - c)))
}
/// The circle table 0x1ff4a0 (20 points in the y–z plane, x = 0): point k at `(0, sin, −cos)` of `18°·(k − 4)`.
fn circle(k: usize) -> V3 {
    let a = (k as f32 - 4.0) * std::f32::consts::PI / 10.0;
    [0.0, a.sin(), -a.cos()]
}

/// One strand (0x1ff5e0 + 0x18·i).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Strand {
    /// +0x00 the node it runs toward the mouth from, +0x02 the ring point it turns about.
    pub node: i16,
    pub point: i16,
    /// +0x04 progress along the segment, +0x08 the blend to the next ring point, +0x0c its speed, +0x10 the speed along.
    pub along: f32,
    pub blend: f32,
    pub ring_speed: f32,
    pub speed: f32,
    /// +0x14 its puff (a particle record).
    pub part: Option<usize>,
}

/// The vortex's globals (module doc).
#[derive(Clone, Debug, PartialEq)]
pub struct Vortex {
    /// 0x2008a0: the path nodes.
    pub nodes: [V3; NODES],
    /// 0x200968: the strengths.
    pub alpha: [i16; NODES],
    /// 0x200980 + 0x140·i: the rings.
    pub rings: [[V3; RING]; NODES],
    /// 0x200940 + 4·i: the spring angle rates.
    pub rate: [f32; NODES],
    /// 0x161ee0 / 0x161ef0: the wobble offsets (in the aim frame), 0x161f00 its angle, gp−0x4d2c its tick.
    pub wob_a: V3,
    pub wob_b: V3,
    pub wob_angle: f32,
    pub wob_t: i32,
    /// gp−0x4d28: the rings' spin.
    pub spin: f32,
    /// gp−0x4d38 / gp−0x4d34 / gp−0x4d30: the draw's s, t and t speed.
    pub scroll_s: f32,
    pub scroll_t: f32,
    pub scroll_speed: f32,
    /// The draw's s at its start and each ring's t at its start (the scroll state of this tick's draw).
    pub draw_s: f32,
    pub draw_t: [f32; NODES],
    /// gp−0x4d40 / gp−0x4d3c: re-init the path / forget the strands at the next start.
    pub restart: bool,
    pub forget: bool,
    pub strands: Vec<Strand>,
    /// The tick the draw callback was registered for (`FUN_0021b198(0x306158, cannon)`).
    pub drawn: Option<u64>,
}

impl Default for Vortex {
    fn default() -> Self {
        Vortex {
            nodes: [[0.0; 3]; NODES], alpha: [0; NODES], rings: [[[0.0; 3]; RING]; NODES], rate: [0.0; NODES], wob_a: [0.0; 3], wob_b: [0.0; 3],
            wob_angle: 0.0, wob_t: 0, spin: 0.0, scroll_s: 0.0, scroll_t: 0.0, scroll_speed: 0.006, draw_s: 0.0, draw_t: [0.0; NODES],
            restart: true, forget: true, strands: vec![Strand::default(); STRANDS], drawn: None,
        }
    }
}

/// One quad of the draw (corners, ST, colours; FX [`FX`], additive).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub corners: [V3; 4],
    pub st: [[f32; 2]; 4],
    pub rgba: [u32; 4],
}

impl Vortex {
    /// `0x306510` (the cannon's state 0).
    pub fn reset(&mut self) {
        self.forget = true;
        self.alpha[0] = 0;
        self.restart = true;
    }

    /// `0x306528(cannon, mouth)`: on a restart, the straight path along the aim, strengths 0, a new wobble.
    fn start(&mut self, rows: [V3; 3], mouth: V3, rng: &mut crate::rng::Rng) {
        if !self.restart { return; }
        self.restart = false;
        let step = with_len(rows[0], SEG);
        self.nodes[0] = mouth;
        self.alpha[0] = 0;
        for k in 1..NODES {
            self.nodes[k] = add(self.nodes[k - 1], step);
            self.alpha[k] = 0;
        }
        self.wob_a = [0.0; 3];
        let a = rng.rand_angle();
        self.wob_b = rotate([0.0, 0.0, 0.1], [1.0, 0.0, 0.0], a);
        self.scroll_speed = 0.006;
        self.wob_t = 0;
        self.spin = 0.0;
        self.wob_angle = a;
        if self.forget {
            self.forget = false;
            for s in &mut self.strands { s.part = None; }
        }
    }

    /// `0x3067d0(cannon, mouth)` while sucking: the path, the strengths, the rings, the strands; registers the draw.
    /// `rows` are the aim rows, `hero` Ratchet's moby (the line ignores it), `tick` the draw's tick.
    pub fn update(&mut self, w: &mut World, rows: [V3; 3], mouth: V3, hero: Option<MobyId>, tick: u64) {
        self.start(rows, mouth, w.rng);
        let seg = SEG;
        let mut step = with_len(rows[0], seg);
        self.nodes[0] = mouth;
        // The strengths from the line along the aim.
        let end = add(mouth, with_len(rows[0], seg * 10.0));
        let p4 = |v: V3| [Pf::f(v[0]), Pf::f(v[1]), Pf::f(v[2]), Pf::ZERO];
        match w.coll_line(p4(mouth), p4(end), 0x14, hero) {
            None => {
                self.alpha[0] = self.alpha[0].wrapping_add(4);
                for k in 1..NODES { self.alpha[k] = self.alpha[k - 1].wrapping_sub(0x20); }
            }
            Some(o) => {
                let d = len(sub(o.point, mouth));
                let n = (d / seg) as i32;
                if n < NODES as i32 {
                    for k in n.max(0) as usize..NODES {
                        let a = self.alpha[k].min(0x40) - 4;
                        self.alpha[k] = a.max(0);
                    }
                }
                let mut k = n.min(NODES as i32) - 1;
                while k >= 0 {
                    let next = self.alpha.get(k as usize + 1).copied().unwrap_or(0);
                    self.alpha[k as usize] = next + 0x20;
                    k -= 1;
                }
            }
        }
        // The wobble.
        let wa = add(mul(self.wob_a, rows), step);
        let wb = add(mul(self.wob_b, rows), step);
        let t60 = w.ticks(0x3c);
        let f = self.wob_t as f32 / t60 as f32;
        self.wob_t += 1;
        step = add(wa, scale(sub(wb, wa), f));
        if t60 <= self.wob_t {
            self.wob_a = self.wob_b;
            let r = w.rng.randf(45.0, 315.0);
            let a = add_rot(self.wob_angle, r * 0.017_453_292);
            self.wob_b = rotate([0.0, 0.0, 0.1], [1.0, 0.0, 0.0], a);
            self.wob_t = 0;
            self.wob_angle = a;
        }
        self.spin = add_rot(self.spin, 0.1);
        // Ring 0 at the mouth.
        let mut up = rows[2];
        let side = with_len(cross(step, up), 1.0);
        let u2 = cross(up, side);
        for j in 0..RING { self.rings[0][j] = add(self.nodes[0], mul(scale(circle(j), 0.1), [u2, side, up])); }
        // Nodes 1..9.
        for i in 1..NODES {
            let old = sub(self.nodes[i], self.nodes[i - 1]);
            let dir = step;
            let c = dot(old, dir) / (len(old) * seg);
            let target = std::f32::consts::FRAC_PI_2 - c.clamp(-1.0, 1.0).asin();
            let mut v = spring(0.0, target, 0.06, 0.2, 0.0, &mut self.rate[i]);
            let diff = sub_rot(target, v);
            if 0.314_159_27 < diff.abs() { v = add_rot(v, sub_rot(diff, 0.314_159_27)); }
            let mut new = rotate(old, cross(old, dir), v);
            new = with_len(new, seg);
            if 2.0 < len(new) { new = scale(new, 2.0 / len(new)); }
            if 5 < i { new = lerp(i as f32 / 10.0, new, old); }
            self.nodes[i] = add(self.nodes[i - 1], new);
            let fwd = with_len(dir, 1.0);
            let side = with_len(cross(fwd, up), 1.0);
            up = cross(side, fwd);
            let r = i as f32 * 0.3 + 0.1 + add_rot(self.spin + i as f32 * 0.7, 0.0).sin() * 0.1;
            for j in 0..RING { self.rings[i][j] = add(self.nodes[i], mul(scale(circle(j), r), [fwd, side, up])); }
            step = with_len(new, seg);
        }
        self.register(tick);
        self.strands(w, mouth, step);
    }

    /// The strands (module doc); `last` is the last desired step (the loose puffs' spawn velocity, overwritten at once).
    fn strands(&mut self, w: &mut World, mouth: V3, last: V3) {
        let mut born = 0;
        let mut loose = 0;
        for i in 0..STRANDS {
            if self.strands[i].part.is_none() && born < 5 {
                let k = (0..NODES).rev().find(|&k| 0x3f < self.alpha[k]);
                if let Some(k) = k.filter(|&k| 0 < k) {
                    if 1 < k && loose < 2 {
                        let j = w.rng.randi(0x14) as usize;
                        let t = w.rng.randf(0.2, 1.0);
                        let pos = lerp(t, self.rings[0][j], self.rings[k][j]);
                        let centre = lerp(t, self.nodes[0], self.nodes[k]);
                        let size = w.rng.randf(60000.0, 180000.0);
                        let spin = w.rng.randi(5) + 1;
                        if let Some(p) = spawn23(w, 1.03, size, pos, spin, last, 0x40_4040) {
                            loose += 1;
                            if let Some(sys) = w.particles.as_deref_mut() {
                                let r = &mut sys.pool.recs[p];
                                rec::set_v3(r, 0x30, with_len(sub(pos, centre), 0.01));
                                rec::set_ff(r, 0x38, rec::ff(r, 0x38) + 0.02);
                            }
                        }
                    }
                    let pos = self.nodes[k];
                    let size = w.rng.randf(60000.0, 180000.0);
                    let p = spawn23(w, 1.0, size, pos, 6, [0.0; 3], 0x7f40_4040);
                    if let (Some(p), Some(sys)) = (p, w.particles.as_deref_mut()) {
                        let r = &mut sys.pool.recs[p];
                        rec::set_i16(r, 0xa, 200);
                        rec::set_u32(r, 0x24, 2);
                        r[0x2a] = 0x7f;
                        r[0x2b] = 200;
                        born += 1;
                    }
                    let point = w.rng.randi(0x14) as i16;
                    let ring_speed = w.rng.randf(0.1, 0.8);
                    let speed = w.rng.randf(0.1, 0.4);
                    self.strands[i] = Strand { node: k as i16, point, along: 0.0, blend: 0.0, ring_speed, speed, part: p };
                }
            }
            self.strand_step(w, i, mouth);
        }
    }

    fn strand_step(&mut self, w: &mut World, i: usize, mouth: V3) {
        let Some(p) = self.strands[i].part else { return };
        let Some(sys) = w.particles.as_deref_mut() else { return };
        let s = &mut self.strands[i];
        let n = s.node as usize;
        let mut seg = sub(self.nodes[n - 1], self.nodes[n]);
        let mut l = len(seg);
        s.along += s.speed;
        rec::set_i16(&mut sys.pool.recs[p], 0xa, 200);
        if l < s.along {
            s.node -= 1;
            if s.node < 1 {
                sys.kill_part(p);
                s.part = None;
                return;
            }
            s.along -= l;
            let n = s.node as usize;
            seg = sub(self.nodes[n - 1], self.nodes[n]);
            l = len(seg);
        }
        let n = s.node as usize;
        let f = s.along / l;
        let r = &mut sys.pool.recs[p];
        if n == 2 {
            rec::set_i16(r, 0xa, 100);
        } else if n < 2 {
            let t = ((1.0 - f) * 100.0) as i32;
            rec::set_i16(r, 0xa, if t < 0x3d { 0x3c } else { t as i16 });
        }
        if n == 1 {
            let at = rec::v3(r, 0x10);
            rec::set_v3(r, 0x10, add(at, with_len(sub(mouth, at), s.speed)));
            rec::set_ff(r, 0xc, rec::ff(r, 0xc) * (1.0 - 0.050_000_012));
            return;
        }
        let at = add(self.nodes[n], scale(seg, f));
        let rp = s.point as usize;
        let o1 = lerp(f, sub(self.rings[n][rp], self.nodes[n]), sub(self.rings[n - 1][rp], self.nodes[n - 1]));
        let rp2 = (rp + 19) % RING;
        let o2 = lerp(f, sub(self.rings[n][rp2], self.nodes[n]), sub(self.rings[n - 1][rp2], self.nodes[n - 1]));
        let o = lerp(s.blend, o1, o2);
        s.blend += s.ring_speed;
        if 1.0 <= s.blend {
            s.blend -= 1.0;
            s.point = rp2 as i16;
        }
        rec::set_v3(r, 0x10, add(at, o));
    }

    /// `0x307850`: the fade when not sucking; the draw while node 0 is above 0.
    pub fn fade(&mut self, tick: u64) {
        if 0 < self.alpha[0] {
            for a in &mut self.alpha { *a = (*a).min(0x40) - 4; }
            self.register(tick);
        }
    }

    /// `0x3078b8`: the collapse at the stop (module doc), then the fade.
    pub fn collapse(&mut self, w: &mut World, tick: u64) {
        self.scroll_speed *= 0.5;
        self.forget = true;
        self.restart = true;
        for i in 0..STRANDS {
            let Some(p) = self.strands[i].part.take() else { continue };
            let Some(sys) = w.particles.as_deref_mut() else { continue };
            let s = self.strands[i];
            let n = (s.node as usize).clamp(1, NODES - 1);
            let mut e = sub(self.nodes[n - 1], self.nodes[n]);
            let l = len(e);
            e = scale(e, s.along / l);
            let d = add(self.nodes[n], e);
            let r = &mut sys.pool.recs[p];
            let c = with_len(sub(rec::v3(r, 0x10), d), 0.03);
            e = scale(e, 0.1 / l);
            rec::set_v3(r, 0x30, add(c, e));
            rec::set_i16(r, 0xa, 0x46);
        }
        self.fade(tick);
    }

    /// The draw callback's registration for `tick` (`FUN_0021b198(0x306158, cannon)`) and its scroll state: s +0.001 a
    /// frame (wrapping at 7), each ring's t +`scroll_speed` (wrapping at 7).
    fn register(&mut self, tick: u64) {
        self.drawn = Some(tick);
        self.draw_s = self.scroll_s;
        self.scroll_s += 0.001;
        if 7.0 < self.scroll_s { self.scroll_s -= 7.0; }
        for i in 1..NODES {
            self.draw_t[i] = self.scroll_t;
            self.scroll_t += self.scroll_speed;
            if 7.0 < self.scroll_t { self.scroll_t -= 7.0; }
        }
    }

    /// `0x306158`: the tube's quads seen from `cam` (module doc).
    pub fn quads(&self, cam: V3) -> Vec<Quad> {
        let mut out = Vec::new();
        let colour = |a: i16| ((a.clamp(0, 0x40) as u32) << 24) | 0x40_4040;
        for i in 1..NODES {
            let s = self.draw_s + 0.3 * i as f32;
            let mut t = self.draw_t[i];
            for j in 0..RING {
                let j2 = (j + 1) % RING;
                let v = [self.rings[i][j], self.rings[i][j2], self.rings[i - 1][j], self.rings[i - 1][j2]];
                let n = cross(sub(v[2], v[1]), sub(v[0], v[1]));
                if dot(n, sub(v[1], cam)) < 0.0 {
                    let (mut outer, mut inner) = (colour(self.alpha[i]), colour(self.alpha[i - 1]));
                    if i == 1 { inner = 0x2040_4040; }
                    if i == 9 { outer = 0x0040_4040; }
                    let st = [[s, t], [s, t + 0.05], [s - 0.3, t], [s - 0.3, t + 0.05]];
                    out.push(Quad { corners: v, st, rgba: [outer, outer, inner, inner] });
                }
                t += 0.05;
            }
        }
        out
    }

    /// The vacuum's reach test (`0x307a50`): `p` within `max(0.3·k + 0.1, 2)` of node k, for the nodes from the
    /// mouth while their strength is above 7 (node 0 above 7 at all).
    pub fn in_vacuum(&self, p: V3) -> bool {
        if self.alpha[0] <= 7 { return false; }
        for k in 0..NODES {
            if 0 < k && self.alpha[k] <= 7 { break; }
            let r = (k as f32 * 0.3 + 0.1).max(2.0);
            if len(sub(p, self.nodes[k])) < r { return true; }
        }
        false
    }
}

/// `FUN_0020cf28(cur, target, k, damp, max, &rate)`: a spring toward `target` (the rate never overshoots the
/// difference), returns `cur + rate`.
pub fn spring(cur: f32, target: f32, k: f32, damp: f32, max: f32, rate: &mut f32) -> f32 {
    let d = sub_rot(target, cur);
    *rate += k * d - damp * *rate;
    if max != 0.0 { *rate = rate.clamp(-max, max); }
    let a = d.abs();
    if a < *rate { *rate = a; } else if *rate < -a { *rate = -a; }
    add_rot(cur, *rate)
}

/// `PartType23Spawn(0, 1, hi, size, pos, spin, vel, rgba)` through the moby world's particles (its draws made without
/// them too; the pool full → None).
fn spawn23(w: &mut World, hi: f32, size: f32, pos: V3, spin: i32, vel: V3, rgba: u32) -> Option<usize> {
    *w.svc.fx.part_spawns.entry(23).or_default() += 1;
    match w.particles.as_deref_mut() {
        Some(sys) => crate::particles::type23::spawn(sys, w.rng, 0.0, 1.0, hi, size, [pos[0], pos[1], pos[2], 1.0], spin, [vel[0], vel[1], vel[2], 0.0], rgba),
        None => {
            for _ in 0..3 { w.rng.randf_sym(0.0, 0.0); }
            w.rng.randi(2);
            w.rng.randf(1.0, hi);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_spring_rotation() {
        let c0 = circle(0);
        assert!((c0[1] + 0.951).abs() < 1e-3 && (c0[2] + 0.309).abs() < 1e-3, "{c0:?}");
        assert!((len(circle(7)) - 1.0).abs() < 1e-6);
        let mut r = 0.0;
        let v = spring(0.0, 1.0, 0.06, 0.2, 0.0, &mut r);
        assert!((v - 0.06).abs() < 1e-6);
        let t = rotate([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2);
        assert!((t[1] - 1.0).abs() < 1e-6, "right-handed");
        // Turning toward the aim: the old segment (x) turned about x × y toward y.
        let t = rotate([1.0, 0.0, 0.0], cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), 0.3);
        assert!(t[1] > 0.0);
    }

    #[test]
    fn vacuum_reach() {
        let mut v = Vortex::default();
        for k in 0..NODES { v.nodes[k] = [k as f32 * SEG, 0.0, 0.0]; }
        v.alpha = [0x40, 0x40, 0x40, 0x40, 0x20, 0, 0, 0, 0, 0];
        assert!(v.in_vacuum([4.0 * SEG, 1.5, 0.0]), "node 4 is above 7");
        assert!(!v.in_vacuum([6.0 * SEG, 0.0, 0.0]), "node 5 at 0 ends the reach");
        v.alpha[0] = 7;
        assert!(!v.in_vacuum([0.0; 3]));
    }
}
