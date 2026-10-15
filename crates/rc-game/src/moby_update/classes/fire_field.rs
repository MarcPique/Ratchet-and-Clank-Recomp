//! Fire and smoke fields, class 760 (`ParticleFieldUpdate` level01 0x2fdbc0, its draw callback `0x2fe080` on
//! list 2, the element spawners `0x2fe888` / `0x2fe9f0`) and the global smoke scroll class 809
//! (`TextureScrollUpdate` 0x2ba658). Spec: docs/plan/world_animation.md §3. Native `f32` (standard floats);
//! formulas, constants, order and the `rand` draws are the game's.
//!
//! **What it is.** The flames and smoke on the bombed buildings (Novalis's four sit on the damaged city roofs;
//! Veldin 00 has six, level 14 forty-six). Earlier docs called it "waterfall foam / mist"; the data says fire: the
//! flame quads are drawn additively in orange (RGB 0x80 0x40 0x20) with FX 46, a rising flame silhouette, their
//! texture scrolling upwards, over a grey smoke column (FX 47). (Novalis's waterfall foam is the ripple manager
//! 751's zone-5 spray, particle types 57 / 56: `rc_game::water`.) Not a particle emitter and not level geometry:
//! a moby without a class blob that owns a range of *elements* in global arrays (level01 0x1cbc60.., up to 1200)
//! and draws them itself, every frame, as camera-facing textured quads (`FastDrawQuadReal`), plus one smoke
//! "curtain" of six quads. The engine draws them (`rc-engine` water_render.rs, "Fire fields"); this module is the
//! state: element life cycle, scroll, respawns. Registered on levels 00, 01 and 14 (the only overlays whose class
//! table runs this update, for class 760 and 809 on all three; identical code and constants).
//!
//! **Pvar block** (P; the cuboid table is `0x1600ec`, `svc.volumes.cuboids`):
//!
//! | P+ | type | use |
//! |---|---|---|
//! | 0x00 | s16 | first element (written by the init: the global allocator `0x161390`) |
//! | 0x02 / 0x40 | s16 | flame elements / smoke elements |
//! | 0x04 | s32 | cuboid (−1: the init deletes the moby) |
//! | 0x08 / 0x0c | f32 | flame scroll speed range (`randf`) |
//! | 0x10 / 0x14, 0x18 / 0x1c | f32 | flame width / height ranges |
//! | 0x20 | f32 | particle rise speed (× dt) |
//! | 0x24 / 0x28 | f32 | particle size range |
//! | 0x2c / 0x2e | s16 | particle alpha range; 0x30 / 0x34 s32 life range (ticks) |
//! | 0x38 | u32 | flame RGB = +0x3c \| +0x3d << 8 \| +0x3e << 16 (written by the init) |
//! | 0x42 | s16 | ≠ 0: type-23 smoke particles near the camera, and no curtain |
//! | 0x44 | f32 | smoke element width base (init: +0x48 if there are smoke elements, else 3.0) |
//! | 0x48 | f32 | curtain width = max(\|cuboid row 1\|, \|row 0\|) (init) |
//! | 0x4c | f32 | curtain height; smoke element height base |
//! | 0x50 | f32 | \|cuboid row 2\| / 2 (init): the curtain sits that far below the cuboid centre |
//! | 0x54 / 0x58 | s32 | FX texture − 40 of the elements / the curtain |
//! | 0x5c | u8 | smoke alpha; 0x5d..0x5f smoke / curtain RGB |
//!
//! **Update (0x2fdbc0).** State 0 (the load pass): state 1, update distance 0xff, the cuboid lengths above, the
//! element range from the allocator, then per flame element `randi(2)` (the mirror flag), w = 1, [`respawn`] and
//! the fade-in timer cleared; per smoke element `randi(2)`, w = 1, [`smoke_init`]. State 1: unless the sphere
//! (cuboid centre + 9 z, r 10) fails `FastBSphereCheck(255)`: with +0x42 ≠ 0 and the camera within 128 (xy) of
//! the moby, one type-23 particle (below); then `RegisterDrawCallback2(0x2fe080, moby)`.
//!
//! **Draw callback (0x2fe080), per element in order** (the state part, [`draw_callback`]): scroll `s −= speed`,
//! `+ 8` once ≤ −8. Flame elements then run their life cycle: fade-in (`fade_in` counting down, alpha
//! `(1 − t/fade_in_ticks)·127`), then the wait (alpha 127), then the fade-out (`fade_out` from `0x161388`, alpha
//! `t/fade_out_ticks·127`); when the fade-out ends: [`respawn`] and a new mirror flag `randi(2)` (alpha 0 that
//! frame). The init leaves every flame element at the start of its wait ((j + 1)·60 ticks for the j-th of its
//! moby), so element j cycles every (j + 3)·60 ticks. Smoke elements never change (their alpha is P+0x5c).
//!
//! **809 (0x2ba658).** State 0: the element allocator `0x161390` = 0, update distance 0xff, and the timer
//! constants from the periods at 0x161368 (60, 60, 60 ticks on 00 / 01 / 14): wait, fade-in and fade-out ticks
//! and the two reciprocals. State 1: the global curtain scroll `0x161374 −= 0.0025` (`+ 8` once ≤ −8).
//!
//! **Type-23 smoke particles** (only level 00's instances 163 / 164 set +0x42; 0x2fdee0..0x2fe018): per particle the
//! field's draws (a point on the cuboid's top, 0.5 up; drift `randf_sym(0, dt/4)` ×2 and the rise P.20·dt; size
//! `randf(P.24, P.28)`; alpha `rand_range(P.2c, P.2e)`; spin `rand_range` of the gp pair), then `PartType23Spawn`
//! (`crate::particles::type23`, its own draws with a record) and the caller's patch: life `ticks(rand_range(P.30,
//! P.34))`, phase 3 from that alpha, and `randi(2)` → ALPHA 0x44.

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::services::{pvar as p, World};

/// The 760 update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2fdbc0;
/// Classes that run [`update`] (levels 00, 01, 14).
pub const CLASSES: [i16; 1] = [760];
/// The 809 update address in the level01 class table.
pub const SCROLL_UPDATE_FN: u32 = 0x2ba658;
/// Classes that run [`scroll_update`] (levels 00, 01, 14).
pub const SCROLL_CLASSES: [i16; 1] = [809];

/// `0x15ed60`: the NTSC speed scale.
const SPEED: f32 = 1.0;
/// `0x15ed6c`: dt.
const DT: f32 = 1.0 / 60.0;
/// gp−0x4f28: the field's view-test far distance (255 on 00 / 01 / 14).
const VIEW_FAR: f32 = 255.0;
/// gp−0x4f70 / −0x4f6c: the steady alpha (int) and the fade's full alpha (127 / 127.0).
const ALPHA_STEADY: i32 = 127;
const ALPHA_FULL: f32 = 127.0;
/// gp−0x4f4c: type-23 particles per tick (1).
const PARTICLES_PER_TICK: i32 = 1;
/// gp−0x4f54 / −0x4f50: the type-23 spawner's position jitter and speed range top (0.5, 1.01); gp−0x4f48 /
/// −0x4f44: its texture-frame range (0..2); gp−0x4f40: its RGB (0x606060).
const PART_JITTER: f32 = 0.5;
const PART_SPEED_HI: f32 = 1.01;
const PART_FRAMES: (i32, i32) = (0, 2);
const PART_RGB: u32 = 0x60_6060;
/// 0x161368 / 0x16136c / 0x161370: the wait, fade-in and fade-out periods (60 ticks each on 00 / 01 / 14).
const PERIODS: [i32; 3] = [60, 60, 60];
/// 0x161378: the curtain scroll step (0.0025 per tick).
const SCROLL_STEP: f32 = 0.0025;
/// The global element arrays hold 1200 entries (0x1d0760..0x1d1a20 / 4).
pub const MAX_ELEMENTS: usize = 1200;

/// One element (the arrays at 0x1cbc60 position, 0x1d0760 width, 0x1d1a20 height, 0x1d2ce0 mirror, 0x1d3190
/// kind, 0x1d3640 scroll, 0x1d4900 scroll speed, 0x1d5bc0 fade-in, 0x1d6520 fade-out, 0x1d6e80 wait).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Element {
    /// Position in the cuboid's local space, each in [−1, 1] (w = 1).
    pub pos: [f32; 4],
    pub width: f32,
    pub height: f32,
    /// 1: drawn with the mirrored basis (the quad's horizontal axis flipped).
    pub mirror: u8,
    /// 0 flame (life cycle, additive), 1 smoke (static, normal blend).
    pub kind: u8,
    pub scroll: f32,
    pub speed: f32,
    pub fade_in: i16,
    pub fade_out: i16,
    pub wait: i16,
    /// The alpha the last draw computed (0..127; the port's renderer draws [`drawn`]'s).
    pub alpha: i32,
}

/// The fire fields' globals, 760 / 809 (0x161368..0x161394) and the element arrays.
#[derive(Clone, Debug, Default)]
pub struct FireFieldState {
    /// 0x161390: the element allocator.
    pub alloc: i32,
    /// 0x161374: the smoke curtain's U scroll (809).
    pub scroll_u: f32,
    /// 0x16137c / 0x161380 / 0x161388: wait, fade-in and fade-out ticks (809's init; 0 before it).
    pub wait_ticks: i32,
    pub fade_in_ticks: i32,
    pub fade_out_ticks: i32,
    /// 0x161384 / 0x16138c: 1 / fade-in and 1 / fade-out ticks.
    pub inv_fade_in: f32,
    pub inv_fade_out: f32,
    pub elems: Vec<Element>,
    /// Respawns so far (stats).
    pub respawns: u64,
}

impl FireFieldState {
    fn elem(&mut self, i: i32) -> Option<&mut Element> {
        let i = usize::try_from(i).ok().filter(|&i| i < MAX_ELEMENTS)?;
        if self.elems.len() <= i { self.elems.resize(i + 1, Element::default()); }
        Some(&mut self.elems[i])
    }

    /// The elements of a 760 moby with pvar block `pv`: `[start, start + flames + smoke)`.
    pub fn range(pv: &[u8]) -> std::ops::Range<usize> {
        if pv.len() < 0x60 { return 0..0; }
        let start = p::i16(pv, 0) as i32;
        let n = p::i16(pv, 2) as i32 + p::i16(pv, 0x40) as i32;
        let a = start.max(0) as usize;
        a..(start + n).clamp(0, MAX_ELEMENTS as i32) as usize
    }
}

fn len3(v: [f32; 4]) -> f32 { (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() }

/// `fun_001f9d20` 0x221608 with a cuboid: `r0·v.x + r1·v.y + r2·v.z + r3·v.w` (xyz).
pub fn cuboid_point(m: &[[f32; 4]; 4], v: [f32; 4]) -> [f32; 3] {
    std::array::from_fn(|k| m[0][k] * v[0] + m[1][k] * v[1] + m[2][k] * v[2] + m[3][k] * v[3])
}

/// `FUN_002fe888(moby, i)`: a new flame element: position `randf_sym(0, 1)` ×3, width `randf(P.10, P.14)`,
/// height `randf(P.18, P.1c)`, scroll 0, speed `randf(P.08, P.0c)`, fade-in `0x161380`, fade-out 0, kind 0,
/// wait `(i − P.00 + 1)·0x16137c`. Six draws.
pub fn respawn(w: &mut World, id: MobyId, i: i32) {
    let pv = &w.m(id).pvars;
    let (w_lo, w_hi, h_lo, h_hi, s_lo, s_hi) = (p::ff(pv, 0x10), p::ff(pv, 0x14), p::ff(pv, 0x18), p::ff(pv, 0x1c), p::ff(pv, 8), p::ff(pv, 0xc));
    let start = p::i16(pv, 0) as i32;
    let x = w.rng.randf_sym(0.0, 1.0);
    let y = w.rng.randf_sym(0.0, 1.0);
    let z = w.rng.randf_sym(0.0, 1.0);
    let width = w.rng.randf(w_lo, w_hi);
    let height = w.rng.randf(h_lo, h_hi);
    let speed = w.rng.randf(s_lo, s_hi);
    let f = &mut w.svc.fire_fields;
    let (fade_in, wait) = (f.fade_in_ticks as i16, ((i - start + 1).wrapping_mul(f.wait_ticks)) as i16);
    f.respawns += 1;
    let Some(e) = f.elem(i) else { return };
    e.pos = [x, y, z, e.pos[3]];
    (e.width, e.height, e.scroll, e.speed) = (width, height, 0.0, speed);
    (e.fade_in, e.fade_out, e.kind, e.wait) = (fade_in, 0, 0, wait);
}

/// `FUN_002fe9f0(moby, i)`: a smoke element: position `randf_sym(0, 1)` ×3, width `randf(P.44, P.44 + 2)`, height
/// `randf(P.4c, P.4c + 4)`, scroll 0, speed `randf(0.003, 0.005)`, kind 1. Six draws.
pub fn smoke_init(w: &mut World, id: MobyId, i: i32) {
    let pv = &w.m(id).pvars;
    let (wb, hb) = (p::ff(pv, 0x44), p::ff(pv, 0x4c));
    let x = w.rng.randf_sym(0.0, 1.0);
    let y = w.rng.randf_sym(0.0, 1.0);
    let z = w.rng.randf_sym(0.0, 1.0);
    let width = w.rng.randf(wb, wb + 2.0);
    let height = w.rng.randf(hb, hb + 4.0);
    let speed = w.rng.randf(f32::from_bits(0x3b44_9ba6), f32::from_bits(0x3ba3_d70a));
    let Some(e) = w.svc.fire_fields.elem(i) else { return };
    e.pos = [x, y, z, e.pos[3]];
    (e.width, e.height, e.scroll, e.speed, e.kind) = (width, height, 0.0, speed, 1);
}

/// The moby's cuboid (P+0x04), if any.
fn cuboid(w: &World, id: MobyId) -> Option<[[f32; 4]; 4]> {
    let c = usize::try_from(p::i32(&w.m(id).pvars, 4)).ok()?;
    w.svc.volumes.cuboids.get(c).map(|s| s.matrix)
}

/// `ParticleFieldUpdate` (0x2fdbc0).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x60 { return; }
    match w.m(id).state {
        0 => init(w, id),
        1 => tick(w, id),
        _ => {}
    }
}

fn init(w: &mut World, id: MobyId) {
    {
        let m = w.mm(id);
        m.state = 1;
        m.update_dist = 0xff;
    }
    if p::i32(&w.m(id).pvars, 4) == -1 {
        w.delete_moby(id);
        return;
    }
    let Some(c) = cuboid(w, id) else { return };
    let n_flame = p::i16(&w.m(id).pvars, 2) as i32;
    let n_smoke = p::i16(&w.m(id).pvars, 0x40) as i32;
    {
        let pv = &mut w.mm(id).pvars;
        let mut l = len3(c[1]);
        let l0 = len3(c[0]);
        if l < l0 { l = l0; }
        p::set_ff(pv, 0x48, l);
        p::set_ff(pv, 0x44, if n_smoke != 0 { l } else { 3.0 });
        p::set_ff(pv, 0x50, len3(c[2]) * 0.5);
    }
    // FUN_002ba748: the old allocator value (as an s16), the allocator advanced.
    let start = w.svc.fire_fields.alloc as i16 as i32;
    w.svc.fire_fields.alloc = w.svc.fire_fields.alloc.wrapping_add(n_flame + n_smoke);
    p::set_i16(&mut w.mm(id).pvars, 0, start as i16);
    for i in start..start + n_flame {
        let mirror = w.rng.randi(2) as u8;
        if let Some(e) = w.svc.fire_fields.elem(i) {
            e.pos[3] = 1.0;
            e.mirror = mirror;
        }
        respawn(w, id, i);
        if let Some(e) = w.svc.fire_fields.elem(i) { e.fade_in = 0; }
    }
    for i in start + n_flame..start + n_flame + n_smoke {
        let mirror = w.rng.randi(2) as u8;
        if let Some(e) = w.svc.fire_fields.elem(i) {
            e.pos[3] = 1.0;
            e.mirror = mirror;
        }
        smoke_init(w, id, i);
    }
    // (The game also stores 1.0 into the cuboid's +0x3c, the w of its centre row; `cuboid_point` takes w = 1.)
    let pv = &mut w.mm(id).pvars;
    let rgb = (pv[0x3e] as u32) << 16 | (pv[0x3d] as u32) << 8 | pv[0x3c] as u32;
    p::set_u32(pv, 0x38, rgb);
}

fn tick(w: &mut World, id: MobyId) {
    let Some(c) = cuboid(w, id) else { return };
    let sphere = [c[3][0], c[3][1], c[3][2] + 9.0, 10.0];
    if w.view.is_some_and(|v| v.culled(VIEW_FAR, sphere)) { return; }
    let pos = w.m(id).position;
    let cam = [w.camera[0].to_f32(), w.camera[1].to_f32()];
    let dxy = ((pos[0] - cam[0]) * (pos[0] - cam[0]) + (pos[1] - cam[1]) * (pos[1] - cam[1])).sqrt();
    if p::i16(&w.m(id).pvars, 0x42) != 0 && dxy < 128.0 {
        for _ in 0..PARTICLES_PER_TICK { particle(w, id, &c); }
    }
    w.svc.draw_callbacks.register2(Callback::FireField760, id);
}

/// One type-23 smoke particle (0x2fdee0..0x2fe018): the draws of the field and of `PartType23Spawn` 0x282060.
fn particle(w: &mut World, id: MobyId, c: &[[f32; 4]; 4]) {
    let x = w.rng.randf_sym(0.0, 1.0);
    let y = w.rng.randf_sym(0.0, 1.0);
    let pos = { let q = cuboid_point(c, [x, y, 1.0, 1.0]); [q[0], q[1], q[2] + 0.5, 1.0] };
    let vel = [w.rng.randf_sym(0.0, DT * 0.25), w.rng.randf_sym(0.0, DT * 0.25), p::ff(&w.m(id).pvars, 0x20) * DT, 0.0];
    let pv = &w.m(id).pvars;
    let (s_lo, s_hi, a_lo, a_hi, l_lo, l_hi) = (p::ff(pv, 0x24), p::ff(pv, 0x28), p::i16(pv, 0x2c) as i32, p::i16(pv, 0x2e) as i32, p::i32(pv, 0x30), p::i32(pv, 0x34));
    let size = w.rng.randf(s_lo, s_hi);
    let a = w.rng.rand_range(a_lo, a_hi);
    let rgba = (a as u32) << 24 | PART_RGB;
    let spin = w.rng.rand_range(PART_FRAMES.0, PART_FRAMES.1);
    // PartType23Spawn(jitter, 1, speed, size, pos, spin, vel, rgba): the record (and, with it, its 3 × randf_sym(0,
    // jitter), randi(2), randf(1, speed)); without a particle system only its draws.
    *w.svc.fx.part_spawns.entry(23).or_default() += 1;
    let i = match w.particles.as_deref_mut() {
        Some(sys) => crate::particles::type23::spawn(sys, w.rng, PART_JITTER, 1.0, PART_SPEED_HI, size, pos, spin, vel, rgba),
        None => {
            for _ in 0..3 { w.rng.randf_sym(0.0, PART_JITTER); }
            w.rng.randi(2);
            w.rng.randf(1.0, PART_SPEED_HI);
            Some(usize::MAX)
        }
    };
    let Some(i) = i else {
        w.svc.fx.part_failed += 1;
        return;
    };
    // The caller's patch: life ticks(rand_range(P.30, P.34)), phase 3 (fade in over ticks(30), then out) from the
    // alpha, and randi(2) picks the 0x44 blend.
    let n = w.rng.rand_range(l_lo, l_hi);
    let life = w.ticks(n);
    let normal = w.rng.randi(2) != 0;
    if let Some(r) = w.particles.as_deref_mut().and_then(|s| s.pool.recs.get_mut(i)) {
        use crate::particles::rec;
        rec::set_i16(r, 0xa, life as i16);
        rec::set_u32(r, 0x24, 3);
        r[0x2a] = (rgba >> 24) as u8;
        r[0x2b] = r[0xa];
        if normal { r[3] = 0x44; }
    }
}

/// The draw callback `0x2fe080`'s state part (see the module doc), run by `draw_callbacks::run_frame`.
pub fn draw_callback(w: &mut World, id: MobyId) {
    let range = FireFieldState::range(&w.m(id).pvars);
    for i in range {
        let f = &mut w.svc.fire_fields;
        let k = f.step_consts();
        let Some(e) = f.elems.get_mut(i) else { break };
        if !step(k, e) {
            // The fade-out ended: a new element (drawn this frame at alpha 0) and a new mirror flag.
            respawn(w, id, i as i32);
            let m = w.rng.randi(2) as u8;
            if let Some(e) = w.svc.fire_fields.elems.get_mut(i) {
                e.mirror = m;
                e.alpha = 0;
            }
        }
    }
}

/// The element as the next [`draw_callback`] draws it (the renderer's view: the PS2 updates the element in the draw
/// and draws the result). A respawning element draws at alpha 0, so its new draws do not matter here.
pub fn drawn(f: &FireFieldState, e: &Element) -> Element {
    let mut e = *e;
    if !step(f.step_consts(), &mut e) { e.alpha = 0; }
    e
}

/// The step's constants: 1 / fade-in ticks, 1 / fade-out ticks, fade-out ticks.
type StepConsts = (f32, f32, i32);

impl FireFieldState {
    fn step_consts(&self) -> StepConsts { (self.inv_fade_in, self.inv_fade_out, self.fade_out_ticks) }
}

/// One element's step in `0x2fe080` (scroll, then a flame's life cycle). False: the fade-out ended (a respawn).
fn step((inv_in, inv_out, out_ticks): StepConsts, e: &mut Element) -> bool {
    use crate::moby_update::services::fast_dec_timer_s16 as dec;
    let s = e.scroll - e.speed * SPEED;
    e.scroll = if s <= -8.0 { s + 8.0 } else { s };
    if e.kind != 0 { return true; }
    e.alpha = if e.fade_in != 0 {
        dec(&mut e.fade_in);
        ((1.0 - e.fade_in as f32 * inv_in) * ALPHA_FULL) as i32
    } else if e.fade_out != 0 {
        if dec(&mut e.fade_out) != 0 { return false; }
        (e.fade_out as f32 * inv_out * ALPHA_FULL) as i32
    } else {
        if dec(&mut e.wait) != 0 { e.fade_out = out_ticks as i16; }
        ALPHA_STEADY
    };
    true
}

/// `TextureScrollUpdate` (0x2ba658), class 809.
pub fn scroll_update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 1;
            let [wait, fade_in, fade_out] = PERIODS.map(|n| w.svc.ticks(n));
            let f = &mut w.svc.fire_fields;
            f.alloc = 0;
            f.wait_ticks = wait;
            f.fade_in_ticks = fade_in;
            f.inv_fade_in = 1.0 / fade_in as f32;
            f.fade_out_ticks = fade_out;
            f.inv_fade_out = 1.0 / fade_out as f32;
        }
        1 => {
            let f = &mut w.svc.fire_fields;
            let u = f.scroll_u - SCROLL_STEP * SPEED;
            f.scroll_u = if u <= -8.0 { u + 8.0 } else { u };
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_range_is_start_plus_both_counts() {
        let mut pv = vec![0u8; 0x60];
        p::set_i16(&mut pv, 0, 24);
        p::set_i16(&mut pv, 2, 40);
        p::set_i16(&mut pv, 0x40, 4);
        assert_eq!(FireFieldState::range(&pv), 24..68);
    }

    #[test]
    fn cuboid_point_is_row_vector_form() {
        let m = [[2.0, 0.0, 0.0, 0.0], [0.0, 3.0, 0.0, 0.0], [0.0, 0.0, 4.0, 0.0], [10.0, 20.0, 30.0, 1.0]];
        assert_eq!(cuboid_point(&m, [1.0, -1.0, 0.5, 1.0]), [12.0, 17.0, 32.0]);
    }
}
