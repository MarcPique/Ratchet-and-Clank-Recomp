//! Player-facing text and motion options, independent of the PS2 save file.
use bevy::prelude::*;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessibilitySettings {
    /// 0 original, 1 large, 2 extra large.
    pub subtitle_size: u8,
    pub subtitle_contrast: bool,
    /// 0 full, 1 half, 2 off. Only the published view changes, not gameplay.
    pub camera_shake: u8,
}

impl Default for AccessibilitySettings {
    fn default() -> Self { Self { subtitle_size: 0, subtitle_contrast: false, camera_shake: 0 } }
}

impl AccessibilitySettings {
    pub fn startup() -> Self {
        let read = crate::render_settings::load_key;
        Self {
            subtitle_size: match read("subtitle_size").as_deref() { Some("large") => 1, Some("extra") => 2, _ => 0 },
            subtitle_contrast: read("subtitle_contrast").is_some_and(|v| v == "high"),
            camera_shake: match read("camera_shake").as_deref() { Some("half") => 1, Some("off") => 2, _ => 0 },
        }
    }

    pub fn save_changed(&self, old: &Self) {
        if self.subtitle_size != old.subtitle_size {
            crate::render_settings::save_key("subtitle_size", ["original", "large", "extra"][self.subtitle_size.min(2) as usize]);
        }
        if self.subtitle_contrast != old.subtitle_contrast {
            crate::render_settings::save_key("subtitle_contrast", if self.subtitle_contrast { "high" } else { "original" });
        }
        if self.camera_shake != old.camera_shake {
            crate::render_settings::save_key("camera_shake", ["full", "half", "off"][self.camera_shake.min(2) as usize]);
        }
    }
}
