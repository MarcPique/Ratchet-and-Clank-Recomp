//! Point lights on the world geometry: the game's `LightTfrags` (0x2a8e40), `LightTies` (0x2ab218) and
//! `LightShrubs` (0x29e7e8) point-light passes, done natively in the tfrag / tie / shrub vertex shaders
//! (docs/plan/tfrag_lighting.md §9 "Point lights on world geometry").
//!
//! The game relights, every frame, the visible instances whose point-light nibble list is not empty (or that are
//! dirty because a light just left them), always restarting from the baked (directional) result, so what is on screen
//! is `baked + the lights the list names, at their current colour, position and radius`; when the list empties the
//! instance is relit once more and shows the baked colours again. The port keeps the baked colours in the static
//! buffers and adds the lights in the shaders (`assets/shaders/world_lights.wgsl`) from data written here:
//! * the bank (8 slots × `{pos.xyz (Bevy world), radius; colour rgb, intensity}`, [`bank_bytes`]), rewritten when it
//!   changes;
//! * one nibble list per tfrag / tie / shrub (`0xffff` = no light: the shaders then take the unchanged path),
//!   rewritten only when an attachment changes. The lists follow the game's `UpdateAllPointLights` /
//!   `CreatePointLight` / `DetachPointLight` exactly (`rc_game::point_lights::WorldLightLists`).
//!
//! Both ride at the end of each renderer's existing per-frame buffer (`TfragLodState` modes, `TieLodState` words,
//! `ShrubSway` shears: `set_point_lights`), and the vertex normals in the existing static records: a new material
//! binding changes the bind-group layout, which reorders draws (the alpha-test and blended passes are order
//! dependent), so frames without any light would change.
//!
//! The bank is the game tick's (`Services::point_lights`, the same one the mobys merge); without a game tick there
//! are no lights. `RC_WORLD_LIGHTS=0` keeps every list empty (the baked colours only, for comparisons);
//! `RC_WORLD_LIGHTS_TRACE=1` prints, every frame, the wall-clock frame time, the bank and the listed counts.

use crate::tfrag_render::game_to_bevy;
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use rc_game::point_lights::{PointLights, WorldLightLists, WorldSpheres};

/// Bytes per slot of the bank.
const SLOT_BYTES: usize = 32;
/// Bytes of the whole bank (WGSL: 8 × `WorldLight`).
pub const BANK_BYTES: usize = 8 * SLOT_BYTES;

/// The bank as the shaders read it: per slot position (Bevy world) and radius, colour and intensity (the back
/// factor `w`: `d → max(d, d·w)`); free slots zero.
pub fn bank_bytes(bank: &PointLights) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 * SLOT_BYTES);
    for s in &bank.slots {
        let f: [f32; 8] = match s {
            Some(l) => {
                let p = game_to_bevy(l.pos);
                [p.x, p.y, p.z, l.radius, l.color[0], l.color[1], l.color[2], l.intensity]
            }
            None => [0.0; 8],
        };
        for x in f { v.extend_from_slice(&x.to_le_bytes()); }
    }
    v
}

/// The world's spheres and lists, and the last bank written. Kept in the update system's `Local`, not as a
/// resource: a Bevy 0.19 resource is an entity, and one more entity shifts the ids of everything spawned after it,
/// which reorders equal-distance blended draws (determinism.rs `stable_transparent_order`) and so changes frames
/// that have no light at all.
struct WorldLights {
    /// Keeps the import module (`#import ratchet_clank::world_lights::…`) loaded.
    _shader: Handle<bevy::shader::Shader>,
    enabled: bool,
    trace: bool,
    /// Wall clock of the previous update (the trace prints the frame time).
    last: std::time::Instant,
    spheres: WorldSpheres,
    lists: WorldLightLists,
    bank: Vec<u8>,
}

/// The WGSL module the world shaders import (`#import ratchet_clank::world_lights::…`).
const IMPORT_PATH: &str = "shaders/world_lights.wgsl";

pub struct WorldLightsPlugin;

impl Plugin for WorldLightsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, update);
    }
}

/// The shader module, and the run-time records' spheres exactly as the level loader stores them (the lighting
/// passes' own helpers).
fn setup(level: &crate::Level, assets: &AssetServer) -> WorldLights {
    let lv = &level.0;
    let t = &lv.ties;
    let ties = t
        .instances
        .iter()
        .zip(&t.class_of)
        .map(|(inst, c)| c.map_or([0.0, 0.0, 0.0, -1.0e30], |ci| rc_formats::tie_light::instance_centre(&t.classes[ci].class, inst)))
        .collect();
    let s = &lv.shrubs;
    let shrubs = s
        .instances
        .iter()
        .zip(&s.class_of)
        .map(|(inst, c)| c.map_or([0.0, 0.0, 0.0, -1.0e30], |ci| rc_formats::shrub_light::instance_centre(&s.classes[ci].class, inst)))
        .collect();
    let tfrags = lv.tfrags.iter().map(|t| t.header.bsphere).collect();
    let spheres = WorldSpheres { ties, tfrags, shrubs };
    let lists = WorldLightLists::new(&spheres);
    let enabled = !std::env::var("RC_WORLD_LIGHTS").is_ok_and(|v| v.trim() == "0");
    let trace = std::env::var("RC_WORLD_LIGHTS_TRACE").is_ok_and(|v| v.trim() == "1");
    WorldLights { _shader: assets.load(IMPORT_PATH), enabled, trace, last: std::time::Instant::now(), spheres, lists, bank: bank_bytes(&PointLights::default()) }
}

/// After the tick: `UpdateAllPointLights` on the lists, then the buffers that changed.
#[allow(clippy::too_many_arguments)]
fn update(
    play: Option<Res<crate::gameplay::Play>>,
    level: Res<crate::Level>,
    assets: Res<AssetServer>,
    mut state: Local<Option<WorldLights>>,
    generation: Res<crate::level_switch::LevelGeneration>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    tfrags: Option<ResMut<crate::tfrag_lod::TfragLodState>>,
    ties: Option<ResMut<crate::tie_lod::TieLodState>>,
    shrubs: Option<ResMut<crate::shrub_render::ShrubSway>>,
) {
    // A runtime level change (crate::level_switch): the new level's lights.
    if generation.is_changed() { *state = None; }
    let w = state.get_or_insert_with(|| setup(&level, &assets));
    let empty = PointLights::default();
    let bank = match &play {
        Some(p) if w.enabled => &p.svc.point_lights,
        _ => &empty,
    };
    let changed = w.lists.update(bank, &w.spheres);
    let bytes = bank_bytes(bank);
    let bank_changed = bytes != w.bank;
    w.bank = bytes;
    if changed || bank_changed {
        if let Some(mut t) = tfrags { t.set_point_lights(&w.lists.tfrags, &w.bank, &mut buffers); }
        if let Some(mut t) = ties { t.set_point_lights(&w.lists.ties, &w.bank, &mut buffers); }
        if let Some(mut s) = shrubs { s.set_point_lights(&w.lists.shrubs, &w.bank, &mut buffers); }
    }
    let frame_ms = w.last.elapsed().as_secs_f64() * 1e3;
    w.last = std::time::Instant::now();
    if w.trace {
        let n = |l: &[u16]| l.iter().filter(|&&x| x != 0xffff).count();
        let tick = play.as_ref().map_or(0, |p| p.game.counter);
        let lights: Vec<_> = bank.slots.iter().enumerate().filter_map(|(i, s)| s.map(|l| (i, l.pos, l.radius, l.color, l.intensity))).collect();
        println!("world lights: tick {tick}: frame {frame_ms:.2} ms; listed tfrags {} ties {} shrubs {}; bank {lights:?}", n(&w.lists.tfrags), n(&w.lists.ties), n(&w.lists.shrubs));
    }
}
