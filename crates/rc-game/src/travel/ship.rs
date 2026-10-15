//! The player's ship as a moby: its update `ShipUpdate` (level01 0x2a1c40; the loader installs it on the ship it
//! creates, `InitLevelRenderGlobals` 0x255958: classes 531 / 532 / 533 = `0x160548[ship]`, never in a level's class
//! table), its hide / show (`0x2a2450` / `0x2a2480`) and its draw callbacks: the ground shadow 0x2a2130, the engine
//! flames 0x2a2ab8, the fly-away trail 0x2a2d28 (boot `fun_0022e420`), the exhaust puffs `fun_0022f5b0` (0x2a3eb8).
//! The canopy glass 0x2a70a8 is drawn by `rc-engine`'s fx_draw (`Callback::ShipGlass`).
//!
//! | address | what | here |
//! |---|---|---|
//! | +0x74 | `ShipUpdate` runs on the loader's ship only (`svc.travel.moby`); the scene and flight actors of a ship class (`CreateMoby(531..533)`) get the class table's empty update | [`update`] (the early return) |
//! | 0x2a1c40 state 0x2a | the level-10 Clank boarding (scene 9 started): waits while game mode 2; then once each, guarded by the global flags 0x13d3d0..0x13d3d3: scene 4 when item 28 is owned (0x13d4dc), scene 5, movie 0xb, scene 6 when planet 11 is unlocked (0x13dd4b); then state 0, mode &= ~0x41, 0x13e058 = 1 (the take-off auto-skips), 0x15f630 = 1 | [`update`] (`cinematic::start_scene` / `start_movie`, `interact::set_global_flag`) |
//! | 0x2a1c6c | drawn (+0x31): +0xbc += 2; glow +0x90 = `v | v<<8 | v<<16`, v = `(int)(fast_cos((+0xbc − 0x80)·2π/256)·50) + 0x96` (533: red `v >> 1`); `RegisterDrawCallback2(0x2a70a8)` (the glass); xy distance to the camera 0x167240 < 32 → `RegisterDrawCallback(0x2a2130)` (the shadow) | [`update`] (`fast_cos` as `f32::cos` [L]) |
//! | 0x2a1d44 | the hatch flag (pvar +0xc, s16): off → on when the hero (0x13f3d0) is within xy 6 of the ship and within 4 (3-D) of the hatch `rows·0x1bdd00[ship] + pos`; on → off beyond 4.1; mode bit 1 (hidden) → off | [`update`] |
//! | 0x2a1e54 | flag on: `try_set_help_message(2, 0x53e4)` ("△ Enter ship"); △ (0x13cae4 & 0x10) with the lease: level 10 with the hero in the Clank body (0x1413f4 == 1) → state 0x2a, mode \|= 0x41, `DialogStreamStart(9)`; else `PromptRelease(2)` (Lombyte `OpenShipMenu`), 0x15f630 = 1, 0x13e058 = 0 | [`update`] (`Interact::try_prompt`, `Prompt::release`) |
//! | 0x2a2360 (level13 `0x298cb0`) | the adoption of a placed ship: 0x160540 = 1; the current ship hidden (`0x2a2450`); level 13: the landing spot 0x13e090 = (464.66, 580.68, 316.72), yaw 0x13e0a8 = 2.77; 0x13e056 = class − 0x213; 0x13e030 = it, +0x74 = `ShipUpdate`, mode &= ~3, +0x94 (collision) = class +0x10; `fun_00212ed8(m, 1, 0)`; 0x13e060..0x13e068 = its pvar words 0..2 (the fly-away set-up). Callers: Gemlik's director 1353 after the escape, and `InitLevelRenderGlobals` on level 13 with ship 2 and 0x160540 set (the placed class `0x160550` = ship 2's 533, instead of creating one) | [`adopt`], [`adopt_table`] |
//! | 0x2a2450 / 0x2a2480 | ship mode \|= 3 / &= ~3, +0x94 (collision) 0 / class +0x10 | `cinematic::EngineRequest::ShipHidden` (existing) |
//! | 0x2a2130 | the shadow: `fun_001fa820(32, bsphere/1024, &a)` (0x2222e8): `FastBSphereCheck(32, sphere)` 0x2221f0 ≥ 0 (not beyond 32 units of view depth, not behind, inside the side planes), then a = clamp(`vftoi0`(32·1024 − (c.z − r)·1024) >> 7, 0, 0x80) (the check's vf3.y: full up to 16 units of the sphere's near depth, 0 at 32); in the fly-away (mode 6, sub 3) past `ticks(150)`: a −= (tick − 150)·4, nothing at ≤ 0; one `FastDrawQuadReal`, FX 0, ALPHA 0x44, corners `rows·0x1bdd50[ship][k] + bsphere/1024` at z = the ship's z + 0.1 (mode 6: `GroundHeight(0.5, pos, 0)` + 0.1), RGBA `(a >> 1) << 24 \| 0x808080`, ST 0x1bdd30 | [`shadow_quads`] (the view: the last drawn frame's `BSphereView`; without one, a linear fade over the 32 units [L]) |
//! | 0x2a2ab8 | the flames: per flame i < `gp−0x6680[ship]` (8 / 2 / 2): intensity = +0xbc + (+0xb2 ≠ 0 ? `randi(+0xb2)` : 0) (drawn at render time: the rand is the frame's), RGBA `intensity << 24 \| (533 ? 0x308000 : 0x2058b0)`, corners `rows·(c_k·intensity·size/40 + p_i) + bsphere/1024` (p_i, size = table 0x1bde30 / 0x1bdeb0 / 0x1bded0 by ship, c_k = 0x1bdef0), FX 5, ALPHA 0x48, ST 0x1bde10 | [`flame_quads`] (the rand in `draw_callbacks::run_frame`) |
//! | 0x2a2d28 | the trail: per sample pair of the ring (0x13e0f0 / 0x13e2f0, head 0x13e080, count 0x13e084) two ribbons per side; FX 0x13 (FX 0 in the flight, mode 6 sub 4), ALPHA 0x48 | [`trail_quads`] |
//! | 0x2a3eb8 | the exhaust: 6 puffs, `PartType02Spawn(rows·0x1be0e0[ship][i] + pos, v1 = (randf(±dt), randf(±dt), z + randf(±dt/4), 0.4), v2 = v1 with w 0.6, 0x24c0c0c0, 0x14c0c0c0, ticks(4), ticks(4), ticks(4) + randi(ticks(4)), −1)` | [`exhaust`] |

use super::{ship_index, to_world, rotate, ShipGlobals, HATCH, TRAIL_COLOURS, TRAIL_SAMPLES, TRAIL_WIDTH};
use crate::moby_runtime::{Moby, MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::classes::units::{FxQuad, FxQuads};
use crate::moby_update::services::World;

/// The level-01 label of the update (the loader's `+0x74`).
pub const UPDATE_FN: u32 = 0x2a1c40;
/// The ship classes.
pub const CLASSES: [i16; 3] = [531, 532, 533];
/// Class 533 (ship 2): its glow and flames have no red.
pub const CLASS_533: i16 = 533;
/// "△ Enter ship" (21476).
pub const PROMPT: i32 = 0x53e4;
/// `try_set_help_message`'s owner of the ship.
pub const OWNER: i32 = crate::moby_update::interact::owner::SHIP;
/// The state of the level-10 Clank boarding.
pub const STATE_BOARDING: u8 = 0x2a;
/// Pvar s16 +0xc: the hero is at the hatch.
pub const PV_AT_HATCH: usize = 0xc;
/// The global flags of the boarding's scenes (0x13d3d0..0x13d3d3 = 0x13d388 + 0x48..).
pub const FLAG_SCENE4: usize = 0x48;
pub const FLAG_SCENE5: usize = 0x49;
pub const FLAG_MOVIE: usize = 0x4a;
pub const FLAG_SCENE6: usize = 0x4b;
/// Item 28 (0x13d4dc): owned starts scene 4.
pub const ITEM_SCENE4: usize = 28;
/// Planet 11 (Pokitaru, 0x13dd4b) unlocked starts scene 5, movie 0xb and scene 6.
pub const PLANET_SCENES: usize = 11;

/// 0x1bdd30: the shadow quad's ST.
pub const SHADOW_ST: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
/// 0x1bdd50 + ship·0x40: the shadow's corners in the ship's frame.
pub const SHADOW_CORNERS: [[[f32; 2]; 4]; 3] = [
    [[4.0, 1.6], [4.0, -1.6], [-4.0, 1.6], [-4.0, -1.6]],
    [[4.0, 2.0], [4.0, -2.0], [-4.0, 2.0], [-4.0, -2.0]],
    [[3.5, 1.5], [3.5, -1.5], [-3.5, 2.0], [-3.5, -2.0]],
];
/// `gp−0x6680` 0x160580[ship]: flames per ship.
pub const FLAME_COUNT: [usize; 3] = [8, 2, 2];
/// 0x1bde30 (ship 0, 8 flames) / 0x1bdeb0 (ship 1) / 0x1bded0 (ship 2): flame point (x, y, z) and size (w).
pub const FLAMES_0: [[f32; 4]; 8] = [
    [-3.3, 1.85, 0.13, 0.7], [-3.3, 1.42, 0.42, 0.7], [-3.3, 1.42, -0.15, 0.7], [-3.3, 0.8, 1.6, 0.7],
    [-3.3, -1.85, 0.13, 0.7], [-3.3, -1.42, 0.42, 0.7], [-3.3, -1.42, -0.15, 0.7], [-3.3, -0.8, 1.6, 0.7],
];
pub const FLAMES_1: [[f32; 4]; 2] = [[-4.75, 1.75, 0.5, 1.2], [-4.75, -1.75, 0.5, 1.2]];
pub const FLAMES_2: [[f32; 4]; 2] = [[-5.7, 0.9, -0.4, 1.15], [-5.7, -0.9, -0.4, 1.15]];
/// 0x1bdef0: the flame quad's corner offsets (in the ship's frame, scaled by intensity·size/40).
pub const FLAME_CORNERS: [[f32; 3]; 4] = [[0.0, -1.0, 1.0], [0.0, 1.0, 1.0], [0.0, -1.0, -1.0], [0.0, 1.0, -1.0]];
/// 0x1bde10: the flame quad's ST.
pub const FLAME_ST: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
/// 0x1bdf90: the trail ribbons' ST.
pub const TRAIL_ST: [[f32; 2]; 4] = [[0.0, 0.5], [1.0, 0.5], [0.0, 0.5], [1.0, 0.5]];
/// 0x1be0e0 + ship·0x60: the exhaust puffs' points in the ship's frame (the gear / engine vents).
pub const EXHAUST: [[[f32; 3]; 6]; 3] = [
    [[1.1, 0.0, 0.7], [2.1, 0.0, 0.7], [-0.6, -0.9, 0.7], [-1.6, -0.9, 0.7], [-0.6, 0.9, 0.7], [-1.6, 0.9, 0.7]],
    [[2.25, 0.0, 1.0], [3.5, 0.0, 1.0], [-0.25, -1.25, 1.0], [-1.25, -1.25, 1.0], [-0.25, 1.25, 1.0], [-1.25, 1.25, 1.0]],
    [[2.66, 0.0, 0.8], [4.33, 0.0, 0.8], [-2.0, -3.2, 0.8], [-2.0, -1.5, 0.8], [-2.0, 3.2, 0.8], [-2.0, 1.5, 0.8]],
];
/// FX textures (`GetEffectTex`).
pub const FX_SHADOW: usize = 0;
pub const FX_FLAME: usize = 5;
pub const FX_TRAIL: usize = 0x13;
pub const FX_TRAIL_FLIGHT: usize = 0;

/// The ship's draw-callback output the renderer reads (`DrawCallbacks::ship`).
#[derive(Clone, Debug, Default)]
pub struct ShipDraws {
    /// The shadow quads registered this tick, per moby (`0x2a2130`: computed at registration).
    pub shadow: std::collections::HashMap<MobyId, FxQuads>,
    /// The flames of the last frame's registrations (`0x2a2ab8`: their `randi(+0xb2)` is the frame's, drawn by
    /// `draw_callbacks::run_frame`).
    pub flames: std::collections::HashMap<MobyId, FxQuads>,
}

/// The ship globals as the moby loop sees them.
fn globals<'a>(w: &'a World) -> &'a ShipGlobals { &w.svc.travel }

fn pv16(m: &Moby, off: usize) -> i16 { m.pvars.get(off..off + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]])) }

fn set_pv16(m: &mut Moby, off: usize, v: i16) {
    if m.pvars.len() < off + 2 { m.pvars.resize(off + 2, 0); }
    m.pvars[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

/// `ShipUpdate` 0x2a1c40 (module docs).
pub fn update(w: &mut World, id: MobyId) {
    // The loader installs `ShipUpdate` on the ship it creates (+0x74) only: another moby of a ship class (the actors of
    // the mode-6 scenes and of the flight, `CreateMoby(531..533)`) runs the class table's entry, the empty update on
    // level 01 [L: on the other levels].
    if w.svc.travel.moby != Some(id) { return; }
    if w.m(id).state == STATE_BOARDING {
        boarding(w, id);
        return;
    }
    let ship = ship_index(globals(w).ship);
    if w.m(id).visible != 0 {
        let m = w.mm(id);
        m.cmd = m.cmd.wrapping_add(2);
        let f = ((m.cmd as i32 - 0x80) as f32 * 0.024_543_693).cos();
        let v = ((f * 50.0) as i32 + 0x96) as u32;
        m.glow = if m.o_class == CLASS_533 { v >> 1 | v << 8 | v << 16 } else { v | v << 8 | v << 16 };
        w.svc.draw_callbacks.register2(Callback::ShipGlass, id);
        let mat = w.joint_matrix(id, 0);
        w.svc.draw_callbacks.matrices.insert(id, mat);
        let p = w.m(id).position;
        let c = w.camera_point();
        let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
        if (dx * dx + dy * dy).sqrt() < 32.0 { register_shadow(w, id); }
    }
    // The hatch flag (pvar +0xc).
    let hero = w.hero_point();
    let hero = [hero[0], hero[1], hero[2]];
    let (pos, rows) = (w.m(id).position, w.m(id).rows);
    let hatch = to_world(&rows, pos, HATCH[ship]);
    let d3 = dist3(hatch, hero);
    let mut flag = pv16(w.m(id), PV_AT_HATCH);
    if flag == 0 {
        let (dx, dy) = (pos[0] - hero[0], pos[1] - hero[1]);
        if (dx * dx + dy * dy).sqrt() < 6.0 && d3 < 4.0 { flag = 1; }
    } else if d3 > 4.1 {
        flag = 0;
    }
    if w.m(id).mode & 1 != 0 { flag = 0; }
    set_pv16(w.mm(id), PV_AT_HATCH, flag);
    if flag == 0 { return; }
    let lease = w.svc.interact.try_prompt(OWNER, PROMPT);
    if !w.svc.interact.triangle() || lease == 0 { return; }
    if w.svc.level == 10 && w.hero.mode == 1 {
        let m = w.mm(id);
        m.state = STATE_BOARDING;
        m.mode |= 0x41;
        crate::cinematic::start_scene(w, 9, false);
    } else {
        let up = w.svc.interact.prompt_hud;
        w.svc.interact.prompt.release(OWNER, up);
        w.svc.travel.take_off = true;
        w.svc.travel.autoskip = false;
        w.svc.interact.handoffs.push(crate::moby_update::interact::Handoff::ShipMenu);
    }
}

/// State 0x2a: the boarding's scenes, then the take-off with the auto-skip.
fn boarding(w: &mut World, id: MobyId) {
    use crate::moby_update::interact::set_global_flag;
    if w.svc.game_mode == 2 { return; }
    let flag = |w: &World, i: usize| w.svc.interact.game.flags.get(i).copied().unwrap_or(0);
    let unlocked = w.svc.interact.game.planet_unlocked.get(PLANET_SCENES).is_some_and(|&b| b != 0);
    let owned = w.hero.owned.0.get(ITEM_SCENE4).is_some_and(|&b| b != 0);
    if flag(w, FLAG_SCENE4) == 0 && owned {
        crate::cinematic::start_scene(w, 4, false);
        set_global_flag(w, FLAG_SCENE4, 1);
        return;
    }
    if flag(w, FLAG_SCENE5) == 0 && unlocked {
        crate::cinematic::start_scene(w, 5, false);
        set_global_flag(w, FLAG_SCENE5, 1);
        return;
    }
    if flag(w, FLAG_MOVIE) == 0 && unlocked {
        crate::cinematic::start_movie(w, 0xb);
        set_global_flag(w, FLAG_MOVIE, 1);
        return;
    }
    if flag(w, FLAG_SCENE6) == 0 && unlocked {
        crate::cinematic::start_scene(w, 6, false);
        set_global_flag(w, FLAG_SCENE6, 1);
        return;
    }
    let m = w.mm(id);
    m.state = 0;
    m.mode &= !0x41;
    w.svc.travel.autoskip = true;
    w.svc.travel.take_off = true;
    w.svc.interact.handoffs.push(crate::moby_update::interact::Handoff::ShipMenu);
}

fn dist3(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }

/// `RegisterDrawCallback(0x2a2130, ship)` with its quad (draw only; the ground height is read now, as the draw would).
pub fn register_shadow(w: &mut World, id: MobyId) {
    let mode6 = w.svc.game_mode == 6;
    let ground = mode6.then(|| {
        let p = w.m(id).position;
        let v = [p[0], p[1], p[2], p[3]].map(crate::ps2v::Pf::f);
        w.ground_height(crate::ps2v::Pf::f(0.5), v, 0).to_f32()
    });
    let g = globals(w);
    // The fly-away's fade of the shadow: game mode 6, sub 3, the sub tick 0x13e054.
    let fly = (mode6 && g.sub == 3).then_some(g.tick as i32);
    let ship = ship_index(g.ship);
    let cam = w.camera_point();
    if let Some(q) = shadow_quads(w.m(id), ship, ground, cam, w.view, fly) {
        w.svc.draw_callbacks.ship.shadow.insert(id, q);
        w.svc.draw_callbacks.register(Callback::ShipShadow, id);
    }
}

/// `fun_001fa820(far, sphere, &a)` (level01 0x2222e8): −1 when `FastBSphereCheck(far, sphere)` culls it, else the
/// check's vf3.y (`far·1024 − (c.z − r)`, the ×1024 view depth of the sphere's near side under the far distance)
/// `vftoi0`, shifted right 7, clamped to 0..0x80.
pub fn depth_alpha(view: &crate::particles::BSphereView, far: f32, sphere: [f32; 4]) -> Option<i32> {
    use crate::ps2v::{add, mul, sub};
    if view.culled(far, sphere) { return None; }
    let w = crate::ps2v::K1024;
    let v1 = sphere.map(|x| mul(x.to_bits(), w));
    let d = [0, 1, 2].map(|k| sub(v1[k], view.cam[k]));
    let cz = add(add(mul(view.rows[0][2], d[0]), mul(view.rows[1][2], d[1])), mul(view.rows[2][2], d[2]));
    let near = add(sub(0, v1[3]), cz);
    let y = sub(mul(far.to_bits(), w), near);
    // vftoi0 (truncation; the check passed, so y ≥ 0), `dsrl32 7` of the y lane, pmaxw 0 / pminw 0x80.
    Some(((f32::from_bits(y) as i32) >> 7).clamp(0, 0x80))
}

/// The shadow quad of `0x2a2130` (module docs). `ground`: the mode-6 ground height; `view`: the last drawn frame's
/// `FastBSphereCheck` view (None: a linear fade over the 32 units [L]); `fly_tick`: the fly-away's sub-3 tick
/// (0x13e054) while mode 6 sub 3 runs.
pub fn shadow_quads(m: &Moby, ship: usize, ground: Option<f32>, cam: [f32; 3], view: Option<&crate::particles::BSphereView>, fly_tick: Option<i32>) -> Option<FxQuads> {
    let c = [m.bsphere[0] / 1024.0, m.bsphere[1] / 1024.0, m.bsphere[2] / 1024.0];
    let mut a = match view {
        Some(v) => depth_alpha(v, 32.0, [c[0], c[1], c[2], m.bsphere[3] / 1024.0])?,
        None => {
            let d = dist3(c, cam);
            if d >= 32.0 { return None; }
            ((1.0 - d / 32.0) * 128.0).clamp(0.0, 128.0) as i32
        }
    };
    if let Some(t) = fly_tick {
        if t > 150 {
            a += (t - 150) * -4;
            if a < 1 { return None; }
        }
    }
    let z = ground.map_or(m.position[2] + 0.1, |g| g + 0.1);
    let rgba = ((a >> 1) as u32) << 24 | 0x80_8080;
    let corners = SHADOW_CORNERS[ship].map(|[x, y]| {
        let r = rotate(&m.rows, [x, y, 0.0]);
        [r[0] + c[0], r[1] + c[1], z]
    });
    Some(FxQuads { fx: FX_SHADOW, additive: false, subtract: false, quads: vec![FxQuad { corners, st: SHADOW_ST, rgba: [rgba; 4] }] })
}

/// `RegisterDrawCallback(0x2a2ab8, ship)`: the flames (their rand runs in the frame's callbacks, [`flames_frame`]).
pub fn register_flames(svc: &mut crate::moby_update::Services, id: MobyId) { svc.draw_callbacks.register(Callback::ShipFlames, id); }

/// The state part of `0x2a2ab8` at draw time: per flame `randi(+0xb2)` when +0xb2 ≠ 0, then the quads.
pub fn flames_frame(w: &mut World, id: MobyId) {
    let ship = ship_index(globals(w).ship);
    let n = FLAME_COUNT[ship];
    let b2 = w.m(id).spawn_id as i32;
    let mut extra = Vec::with_capacity(n);
    for _ in 0..n { extra.push(if b2 != 0 { w.rng.randi(b2) } else { 0 }); }
    let ship_moby = globals(w).moby.unwrap_or(id);
    let rows = w.table.mobys.get(ship_moby).map_or(w.m(id).rows, |s| s.rows);
    let q = flame_quads(w.m(id), &rows, ship, &extra);
    w.svc.draw_callbacks.ship.flames.insert(id, q);
}

/// The flame quads of `0x2a2ab8`: `extra[i]` = flame i's `randi(+0xb2)` (0 without). `rows`: the ship moby's
/// (0x13e030) rows, which the callback uses whatever moby registered it.
pub fn flame_quads(m: &Moby, rows: &[[f32; 4]; 4], ship: usize, extra: &[i32]) -> FxQuads {
    let table: &[[f32; 4]] = match ship { 1 => &FLAMES_1, 2 => &FLAMES_2, _ => &FLAMES_0 };
    let base = [m.bsphere[0] / 1024.0, m.bsphere[1] / 1024.0, m.bsphere[2] / 1024.0, 0.0];
    let colour = if m.o_class == CLASS_533 { 0x30_8000 } else { 0x20_58b0 };
    let mut quads = Vec::new();
    for (i, e) in table.iter().take(FLAME_COUNT[ship]).enumerate() {
        let intensity = (m.cmd as i32 + extra.get(i).copied().unwrap_or(0)) as u32;
        let k = intensity as f32 * (e[3] / 40.0);
        let corners = FLAME_CORNERS.map(|c| to_world(rows, base, [c[0] * k + e[0], c[1] * k + e[1], c[2] * k + e[2]]));
        let rgba = intensity << 24 | colour;
        quads.push(FxQuad { corners, st: FLAME_ST, rgba: [rgba; 4] });
    }
    FxQuads { fx: FX_FLAME, additive: true, subtract: false, quads }
}

/// The trail ring (0x13e0f0 / 0x13e2f0) with its head (0x13e080) and count (0x13e084).
#[derive(Clone, Debug)]
pub struct Trail {
    pub a: [[f32; 3]; TRAIL_SAMPLES],
    pub b: [[f32; 3]; TRAIL_SAMPLES],
    pub head: usize,
    pub count: usize,
    /// The colours 0x1bdfb0 as the flight's variant 4 left their alpha bytes.
    pub colours: [[u32; 2]; 3],
}

impl Default for Trail {
    fn default() -> Self { Trail { a: [[0.0; 3]; TRAIL_SAMPLES], b: [[0.0; 3]; TRAIL_SAMPLES], head: 0, count: 0, colours: TRAIL_COLOURS } }
}

impl Trail {
    /// One sample (the fly-away's / the flight's push): head = (head + 1) & 31, count up to 32.
    pub fn push(&mut self, a: [f32; 3], b: [f32; 3]) {
        self.head = (self.head + 1) & (TRAIL_SAMPLES - 1);
        if self.count < TRAIL_SAMPLES { self.count += 1; }
        self.a[self.head] = a;
        self.b[self.head] = b;
    }
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
/// `FastVecNormalize(len, out, v)`: v scaled to length `len` (zero stays zero).
fn set_len(v: [f32; 3], len: f32) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { return [0.0; 3]; }
    [v[0] * len / l, v[1] * len / l, v[2] * len / l]
}

/// The trail of `0x2a2d28` (boot `fun_0022e420`): for each sample i (0 = the head) up to count − 1, with u =
/// (head − i + 31) & 31, on each ring R (A then B): d1 = B[u] − A[u], d2 = −d1, d3 = B[u+1] − A[u+1], d4 = −d3; e1 =
/// R[u+1] − R[u], e2 = R[u+2] − R[u+1] (e1 for the head); a ribbon across the motion (corners `R[u + k/2] +
/// norm(n_k)·(1 − f²)·width`, n = d1×e1, d2×e1, d3×e2, d4×e2) and one along the wing axis (n = d1..d4), f = (i − k/2 +
/// 1)/32, colours `FastTweenColor(f, head, tail)` per corner; FX 0x13 (`flight`: FX 0), ALPHA 0x48.
pub fn trail_quads(t: &Trail, ship: usize, flight: bool) -> FxQuads {
    let mut quads = Vec::new();
    let width = TRAIL_WIDTH[ship];
    let [c1, c2] = t.colours[ship];
    let wrap = |k: usize| k & (TRAIL_SAMPLES - 1);
    for i in 0..t.count.saturating_sub(1) {
        let u = (t.head + TRAIL_SAMPLES * 2 - i - 1) & (TRAIL_SAMPLES - 1);
        let d1 = sub3(t.b[u], t.a[u]);
        let d2 = sub3(t.a[u], t.b[u]);
        let d3 = sub3(t.b[wrap(u + 1)], t.a[wrap(u + 1)]);
        let d4 = sub3(t.a[wrap(u + 1)], t.b[wrap(u + 1)]);
        for ring in [&t.a, &t.b] {
            let e1 = sub3(ring[wrap(u + 1)], ring[u]);
            let e2 = if i == 0 { e1 } else { sub3(ring[wrap(u + 2)], ring[wrap(u + 1)]) };
            let ns = [cross(d1, e1), cross(d2, e1), cross(d3, e2), cross(d4, e2)];
            let mut rgba = [0u32; 4];
            let mut corners = [[0.0f32; 3]; 4];
            for k in 0..4 {
                let m = k >> 1;
                let f = (i as i32 - (m as i32 - 1)) as f32 * 0.031_25;
                rgba[k] = crate::particles::tween_color(f.to_bits(), c1, c2);
                corners[k] = add3(set_len(ns[k], (1.0 - f * f) * width), ring[wrap(u + m)]);
            }
            quads.push(FxQuad { corners, st: TRAIL_ST, rgba });
            let ds = [d1, d2, d3, d4];
            for k in 0..4 {
                let m = k >> 1;
                let f = (i as i32 - (m as i32 - 1)) as f32 * 0.031_25;
                corners[k] = add3(set_len(ds[k], (1.0 - f * f) * width), ring[wrap(u + m)]);
            }
            quads.push(FxQuad { corners, st: TRAIL_ST, rgba });
        }
    }
    FxQuads { fx: if flight { FX_TRAIL_FLIGHT } else { FX_TRAIL }, additive: true, subtract: false, quads }
}

/// The exhaust `fun_0022f5b0(z, moby)` (0x2a3eb8): six type-2 puffs at the ship's vents (module docs).
pub fn exhaust(w: &mut World, z: f32, id: MobyId) {
    let ship = ship_index(globals(w).ship);
    let dt = crate::moby_update::creature::DT;
    for p in EXHAUST[ship] {
        let x = w.rng.randf(-dt, dt);
        let y = w.rng.randf(-dt, dt);
        let zz = z + w.rng.randf(dt * -0.25, dt * 0.25);
        let v1 = [x, y, zz, 0.4];
        let v2 = [x, y, zz, 0.6];
        let (rows, pos) = (w.m(id).rows, w.m(id).position);
        let q = to_world(&rows, pos, p);
        let t = w.ticks(4);
        let t3 = w.ticks(4) + w.rng.randi(w.ticks(4));
        let a = crate::particles::type02::Spawn { pos: [q[0], q[1], q[2], 0.0], v1, v2, c1: 0x24c0_c0c0, c2: 0x14c0_c0c0, t: [t, t, t3], def: -1 };
        crate::moby_update::creature::fx::part02(w, &a);
    }
}

/// `0x2a2450`: the ship hidden (mode |= 3) without collision; `0x2a2480`: shown.
pub fn set_hidden(table: &mut MobyTable, g: &ShipGlobals, hidden: bool) {
    let Some(m) = g.moby.and_then(|id| table.mobys.get_mut(id)) else { return };
    if hidden { m.mode |= 3 } else { m.mode &= !3 }
    m.has_collision = !hidden;
}

/// The landing spot `0x2a2360` sets on level 13 (0x13e090 / 0x13e0a8).
pub const GEMLIK_LANDING: [f32; 3] = [f32::from_bits(0x43e8_547b), f32::from_bits(0x4411_2b85), f32::from_bits(0x439e_5c29)];
pub const GEMLIK_LANDING_YAW: f32 = f32::from_bits(0x4031_47ae);
/// Ship 0's class (0x213): the index is the class minus it.
const FIRST_SHIP_CLASS: i16 = 0x213;

/// `0x2a2360` without its sequence cut (module table): moby `id`, a placed ship of `level`, becomes the ship
/// (`has_collision`: its class has collision, +0x94 = class +0x10). The loader calls it as it builds the table; [`adopt`]
/// in a tick.
pub fn adopt_table(table: &mut MobyTable, travel: &mut ShipGlobals, level: u32, id: MobyId, has_collision: bool) {
    travel.adopted = true;
    // `0x2a2450`: the current ship hidden and without collision.
    if let Some(old) = travel.moby.filter(|&o| o != id && o < table.mobys.len()) {
        let m = &mut table.mobys[old];
        m.mode |= 3;
        m.has_collision = false;
    }
    if level == 13 {
        travel.landing_spot = Some(([GEMLIK_LANDING[0], GEMLIK_LANDING[1], GEMLIK_LANDING[2], 0.0], [0.0, 0.0, GEMLIK_LANDING_YAW, 0.0]));
    }
    let Some(m) = table.mobys.get_mut(id) else { return };
    travel.ship = m.o_class - FIRST_SHIP_CLASS;
    travel.moby = Some(id);
    m.mode &= !3;
    m.has_collision = has_collision;
    let word = |k: usize| m.pvars.get(4 * k..4 * k + 4).map_or(-1, |b| i32::from_le_bytes(b.try_into().unwrap()));
    travel.setup = super::FlyAwaySetup { path: word(0), cam_a: word(1), cam_b: word(2) };
}

/// `0x2a2360` in a tick (module table): [`adopt_table`], then `fun_00212ed8(m, 1, 0)`.
pub fn adopt(w: &mut World, id: MobyId) {
    let coll = crate::moby_update::classes::units::class_collision(w, w.m(id).o_class);
    let level = w.svc.level;
    adopt_table(w.table, &mut w.svc.travel, level, id, coll);
    crate::moby_update::creature::hard_cut(w, id, 1, 0);
}

#[cfg(test)]
mod adopt_tests {
    use super::*;
    use crate::moby_runtime::{ClassInfo, Moby};

    /// `0x2a2360` on Gemlik: the loader's ship hidden without collision; the placed 533 is the ship (index 2), shown,
    /// with its collision and its pvar fly-away set-up; the landing spot is the fixed one; 0x160540 set.
    #[test]
    fn a_placed_ship_is_adopted() {
        let info = ClassInfo { scale: 1.0, ..Default::default() };
        let old = Moby::init_instance(0, 532, Some(&info));
        let mut placed = Moby::init_instance(1, 533, Some(&info));
        placed.mode |= 3;
        placed.pvars = [7i32, 8, 9].iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut table = MobyTable::new(vec![old, placed], 1);
        let mut g = ShipGlobals { moby: Some(0), ship: 1, ..Default::default() };
        adopt_table(&mut table, &mut g, 13, 1, true);
        assert!(g.adopted);
        assert_eq!((g.moby, g.ship), (Some(1), 2));
        assert_eq!((table.mobys[0].mode & 3, table.mobys[0].has_collision), (3, false));
        assert_eq!((table.mobys[1].mode & 3, table.mobys[1].has_collision), (0, true));
        assert_eq!((g.setup.path, g.setup.cam_a, g.setup.cam_b), (7, 8, 9));
        let (p, r) = g.landing_spot.unwrap();
        assert_eq!(([p[0], p[1], p[2]], r[2]), (GEMLIK_LANDING, GEMLIK_LANDING_YAW));
    }
}
