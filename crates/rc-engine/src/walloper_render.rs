//! The Walloper's arcs and fist glow: the draw callback `0x2d1768` its item update registers (`rc_game::hero::walloper`,
//! `Walloper::drawn`): per live arc the core and glow strips of `0x2d1830` (the Tesla Claw's strip code, FX 14 / 16)
//! and the fist's glow quad `0x2d1e08` (FX 8), all from [`rc_game::hero::walloper::draw_quads`] against the camera,
//! additive (ALPHA 0x48), Z tested and not written, through the shared draw-callback material of crate::fx_draw in the
//! list-2 band. The glow's flicker is the draw's `randi(4)`, drawn where the game draws it
//! (`rc_game::moby_update::classes::draw_callbacks`). A module of its own (the arcs are the Walloper's alone).

use crate::fx_draw::{FxAssets, FxGroup, FxPrimMaterial, FxSlots, PrimBuf, LIST1_BIAS};
use crate::game_camera::{GameFog, TfragFog};
use bevy::prelude::*;

/// After the Tesla Claw's slots.
const BAND: f32 = LIST1_BIAS + 96.0;

pub struct WalloperPlugin;

impl Plugin for WalloperPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WalloperDraw>().add_systems(crate::level_switch::LevelUnload, crate::level_switch::reset::<WalloperDraw>).add_systems(PostUpdate, draw.before(bevy::asset::AssetEventSystems));
    }
}

#[derive(Resource, Default)]
struct WalloperDraw {
    slots: FxSlots,
    drawn: Option<u64>,
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    mut state: ResMut<WalloperDraw>,
    play: Option<Res<crate::gameplay::Play>>,
    fog: Option<Res<GameFog>>,
    level: Res<crate::Level>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<FxPrimMaterial>>,
    mut vis: Query<&mut Visibility>,
) {
    let counter = play.as_deref().map(|p| p.game.counter);
    if counter == state.drawn && counter.is_some() { return; }
    state.drawn = counter;
    let mut groups: Vec<FxGroup> = Vec::new();
    if let Some(p) = play.as_deref().filter(|p| p.svc.game_mode != 2) {
        let w = &p.game.hero.walloper;
        if w.drawn == Some(p.game.counter.wrapping_sub(1)) && p.game.hero.items.slot.id == rc_game::hero::walloper::WALLOPER {
            let eye = p.game.camera.out.pos_f32();
            let g = rc_game::hero::physics::to_f32x3(p.game.hero.gravity_dir);
            let dim = p.svc.draw_callbacks.walloper_dim.unwrap_or(true);
            for q in rc_game::hero::walloper::draw_quads(w, eye, g, dim) {
                if groups.last().is_none_or(|l| l.fx != q.fx) { groups.push(FxGroup { fx: q.fx, additive: true, subtract: false, prims: PrimBuf::default() }); }
                groups.last_mut().unwrap().prims.quad(q.corners, q.st, q.rgba);
            }
        }
    }
    let fog = fog.map(|f| f.uniform).unwrap_or_else(|| TfragFog::new(&level.0.fog));
    let tex = level.0.particles.textures.as_ref().map(|t| t.fx_textures.as_slice());
    let mut a = FxAssets { meshes: &mut meshes, images: &mut images, materials: &mut materials, fx: tex, fog };
    state.slots.show(&mut commands, &mut vis, &mut a, groups, BAND, "walloper");
}
