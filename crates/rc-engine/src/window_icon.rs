//! The Windows window and taskbar icons. The `.exe` resource covers Explorer and shortcuts;
//! winit's live window also needs explicit small and large icons.

use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use winit::platform::windows::WindowExtWindows;
use winit::window::Icon;

pub fn set_once(primary: Query<Entity, With<PrimaryWindow>>, mut done: Local<bool>, _main_thread: NonSendMarker) {
    if *done { return; }
    let Ok(entity) = primary.single() else { return };
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        let Some(window) = windows.get_window(entity) else { return };
        let small = Icon::from_rgba(include_bytes!("../assets/ratchet-clank-icon-64.rgba").to_vec(), 64, 64)
            .expect("valid 64x64 RGBA window icon");
        let large = Icon::from_rgba(include_bytes!("../assets/ratchet-clank-icon-256.rgba").to_vec(), 256, 256)
            .expect("valid 256x256 RGBA taskbar icon");
        window.set_window_icon(Some(small));
        window.set_taskbar_icon(Some(large));
        *done = true;
    });
}
