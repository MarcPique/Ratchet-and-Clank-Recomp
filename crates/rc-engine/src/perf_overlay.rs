//! Small player-facing frame interval display, rendered on the final window camera.
//! The milliseconds are wall-clock frame intervals, not GPU timings.

use bevy::prelude::*;
use bevy::ui::UiTargetCamera;

#[derive(Component)]
struct PerfText;

pub struct PerfOverlayPlugin;

impl Plugin for PerfOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn).add_systems(Update, (hotkey, update).chain());
    }
}

fn spawn(mut commands: Commands, camera: Query<Entity, With<crate::display::PresentCamera>>) {
    let Ok(camera) = camera.single() else { return };
    commands.spawn((
        Text::new("FPS -- | -- ms"),
        TextFont { font_size: bevy::text::FontSize::Px(16.0), ..default() },
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, left: px(10), top: px(10), padding: UiRect::all(px(4)), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        UiTargetCamera(camera),
        Visibility::Hidden,
        PerfText,
        Name::new("FPS and frame time"),
    ));
}

fn hotkey(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<crate::display::DisplaySettings>) {
    if keys.just_pressed(KeyCode::F3) {
        let old = *settings;
        settings.show_stats = !settings.show_stats;
        settings.save_changed(&old);
    }
}

fn update(time: Res<Time<Real>>, settings: Res<crate::display::DisplaySettings>, mut q: Query<(&mut Text, &mut Visibility), With<PerfText>>, mut sample: Local<(f32, u32)>) {
    let Ok((mut text, mut visibility)) = q.single_mut() else { return };
    *visibility = if settings.show_stats { Visibility::Visible } else { Visibility::Hidden };
    if !settings.show_stats { *sample = (0.0, 0); return; }
    let dt = time.delta_secs();
    if !(0.0..=1.0).contains(&dt) || dt == 0.0 { return; }
    sample.0 += dt;
    sample.1 += 1;
    if sample.0 >= 0.25 {
        let frame_ms = sample.0 * 1000.0 / sample.1 as f32;
        **text = format!("FPS {:.0} | {:.1} ms", sample.1 as f32 / sample.0, frame_ms);
        *sample = (0.0, 0);
    }
}
