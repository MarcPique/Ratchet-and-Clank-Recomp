//! The Bomb Glove's ground reticle: the draw callback `0x2c23c0` its bombs register on list 1 every tick their
//! landing preview found a point (`rc_game::targeting`, registered in `Services::reticles` by
//! `rc_game::moby_update::classes::bomb`). Per registration two `FastDrawQuadReal` quads
//! ([`rc_game::targeting::reticle_quads`]: FX 0x11 and 0x12, 0x80808080, TEX1 bilinear, ALPHA 0x48 additive, Z tested
//! and not written) through the shared draw-callback material of crate::fx_draw, in the list-1 band (after the
//! mobys, before the particles). Not drawn in a cutscene (game mode 2), like the callback.

use crate::fx_draw::{FxAssets, FxGroup, FxPrimMaterial, FxSlots, PrimBuf, LIST1_BIAS};
use crate::game_camera::{GameFog, TfragFog};
use bevy::prelude::*;
use rc_game::targeting::{reticle_quads, RETICLE_ST};

/// After the other list-1 callbacks' slots (crate::fx_draw uses the band's first slots).
const BAND: f32 = LIST1_BIAS + 48.0;

pub struct ReticlePlugin;

impl Plugin for ReticlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReticleDraw>().add_systems(crate::level_switch::LevelUnload, crate::level_switch::reset::<ReticleDraw>).add_systems(PostUpdate, draw.before(bevy::asset::AssetEventSystems));
    }
}

#[derive(Resource, Default)]
struct ReticleDraw {
    slots: FxSlots,
    /// The tick counter the last draw used (redrawn once per tick, like the other callbacks).
    drawn: Option<u64>,
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    mut state: ResMut<ReticleDraw>,
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
    let mut groups = Vec::new();
    if let Some(p) = play.as_deref().filter(|p| p.svc.game_mode != 2) {
        // The registrations of the tick that just ran (its moby loop ran with the counter before the increment); the
        // spin reads the counter at the draw.
        let c = p.game.counter;
        for r in p.svc.reticles.of_tick(c.wrapping_sub(1)) {
            for (fx, q) in reticle_quads(r, c) {
                let mut prims = PrimBuf::default();
                prims.quad(q, RETICLE_ST, [r.style.rgba; 4]);
                groups.push(FxGroup { fx, additive: r.style.additive, subtract: false, prims });
            }
        }
    }
    let fog = fog.map(|f| f.uniform).unwrap_or_else(|| TfragFog::new(&level.0.fog));
    let tex = level.0.particles.textures.as_ref().map(|t| t.fx_textures.as_slice());
    let mut a = FxAssets { meshes: &mut meshes, images: &mut images, materials: &mut materials, fx: tex, fog };
    state.slots.show(&mut commands, &mut vis, &mut a, groups, BAND, "reticle");
}
