//! Optional bridge to a user-installed DLSS 5 feeder. No third-party runtime is shipped here.
//! The feeder re-reads `dlss5-feed.cfg` during play. Keeping `enabled=1` allows its Vulkan hook
//! to be installed at device creation; `mode=0/2` is the live off/on switch.

use std::path::{Path, PathBuf};

use bevy::prelude::Resource;
use rc_game::menus::pause::{port::Setting, PageMenu};

const PRESETS: [u8; 5] = [0, 5, 6, 10, 11];

#[derive(Resource, Clone)]
pub struct Bridge {
    cfg: Option<PathBuf>,
}

impl Bridge {
    pub fn discover() -> Self {
        let cfg = std::env::current_exe().ok().and_then(|exe| {
            let dir = exe.parent()?;
            // A partial injector installation must never advertise a working switch.
            ["dxgi.dll", "dlss5-feed.addon64", "renodx-dlss5.addon64", "nvngx_dlssnr.dll", "dlss5-feed.cfg"]
                .iter().all(|name| dir.join(name).is_file())
                .then(|| dir.join("dlss5-feed.cfg"))
        });
        Self { cfg }
    }

    pub fn requires_vulkan(&self) -> bool { self.cfg.is_some() }

    pub fn sync_menu(&self, menu: &mut PageMenu) {
        let mode = self.cfg.as_deref().and_then(read_ready_mode);
        menu.set_port_choices(Setting::Dlss5, if mode.is_some() { 0b011 } else { 0b100 });
        menu.set_port_value(Setting::Dlss5, match mode { Some(2) => 1, Some(_) => 0, None => 2 });
        let active = mode == Some(2);
        menu.set_port_choices(Setting::Dlss5Preset, if active { 0b1_1111 } else { 0b10_0000 });
        let preset = self.cfg.as_deref().and_then(read_preset).unwrap_or(0);
        menu.set_port_value(Setting::Dlss5Preset,
            if active { PRESETS.iter().position(|&v| v == preset).unwrap_or(0) as u8 } else { 5 });
    }

    pub fn apply_menu(&self, menu: &PageMenu) {
        let Some(cfg) = self.cfg.as_deref() else { return };
        let Some(choice) = menu.port_value(Setting::Dlss5) else { return };
        let desired = match choice { 0 => 0, 1 => 2, _ => return };
        if read_ready_mode(cfg).is_none() { return; }
        if read_mode(cfg) != Some(desired) {
            match write_number(cfg, "mode", desired) {
                Ok(()) => println!("DLSS 5 feeder: mode={desired} from Advanced Options"),
                Err(e) => eprintln!("DLSS 5 feeder: cannot change mode: {e}"),
            }
        }
        if desired == 2 && read_mode(cfg) == Some(2) {
            if let Some(preset) = menu.port_value(Setting::Dlss5Preset)
                .and_then(|index| PRESETS.get(index as usize).copied()) {
                if read_preset(cfg).unwrap_or(0) != preset {
                    match write_number(cfg, "preset", preset) {
                        Ok(()) => println!("DLSS 5 feeder: preset={preset} from Advanced Options"),
                        Err(e) => eprintln!("DLSS 5 feeder: cannot change preset: {e}"),
                    }
                }
            }
        }
    }
}

fn read_mode(path: &Path) -> Option<u8> {
    let text = std::fs::read_to_string(path).ok()?;
    read_number(&text, "mode")
}

fn read_preset(path: &Path) -> Option<u8> {
    let text = std::fs::read_to_string(path).ok()?;
    read_number(&text, "preset")
}

fn read_ready_mode(path: &Path) -> Option<u8> {
    let text = std::fs::read_to_string(path).ok()?;
    (read_number(&text, "enabled") == Some(1)).then(|| read_number(&text, "mode")).flatten()
}

fn read_number(text: &str, key: &str) -> Option<u8> {
    text.lines().filter_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| value.trim().parse().ok()).flatten()
    }).last()
}

fn write_number(path: &Path, key: &str, value: u8) -> std::io::Result<()> {
    let old = std::fs::read_to_string(path)?;
    let mut replaced = false;
    let mut next = String::with_capacity(old.len());
    for line in old.split_inclusive('\n') {
        let has_cr = line.ends_with("\r\n");
        let body = line.trim_end_matches(['\r', '\n']);
        if body.split_once('=').is_some_and(|(name, _)| name.trim() == key) {
            next.push_str(&format!("{key}={value}"));
            if line.ends_with('\n') { next.push_str(if has_cr { "\r\n" } else { "\n" }); }
            replaced = true;
        } else {
            next.push_str(line);
        }
    }
    if !replaced {
        if !next.is_empty() && !next.ends_with('\n') { next.push('\n'); }
        next.push_str(&format!("{key}={value}\n"));
    }
    let temp = path.with_extension("cfg.rc-next");
    std::fs::write(&temp, next)?;
    std::fs::rename(temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_only_selected_keys_and_preserves_other_feeder_settings() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("rc-dlss5-{}-{stamp}.cfg", std::process::id()));
        std::fs::write(&path, "enabled=1\r\nmode=0\r\npreset=0\r\nwork_resolution=85\r\n").unwrap();
        write_number(&path, "mode", 2).unwrap();
        write_number(&path, "preset", 10).unwrap();
        assert_eq!(read_mode(&path), Some(2));
        assert_eq!(read_preset(&path), Some(10));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "enabled=1\r\nmode=2\r\npreset=10\r\nwork_resolution=85\r\n");
        std::fs::remove_file(path).unwrap();
    }
}
