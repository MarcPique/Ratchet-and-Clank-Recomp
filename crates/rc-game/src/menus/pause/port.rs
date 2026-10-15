//! **Port-only** (not in the PS2 game): the "Advanced Options" page, for settings the PS2 never had (the graphics options and
//! their preset, anti-aliasing, switching the moby shadows off, the aspect, the render resolution and fullscreen).
//! Spec: docs/plan/menus.md "Port-only settings"; the graphics rows: docs/plan/graphics_options.md (every row's first
//! value is the game's own look).
//!
//! It is built entirely from the game's own machinery so it looks and behaves like the Options sub-pages:
//! a page record whose frame-moby seqs, filler widgets and "✕ Toggle / △ Exit" hint list are those of a
//! game sub-page ([`MODEL_FEW`] Subtitles for ≤ 2 rows, [`MODEL_MANY`] Camera for more), a small title label, and a list drawn and driven like the camera list (update 0x294cc0 /
//! draw 0x294e68: label left at 12 in yellow / light blue, value right-aligned at w − 12, rows h/(n+1),
//! Up/Down without wrap with sound 1, ✕ cycles the value with sound 0, generic Start/Select/R3 and △ keys).
//! Transitions, panels, fonts, colours and sounds come from [`PageMenu`] unchanged.
//!
//! Entry: one abbreviated item "Adv. Options" (action 3 → [`PAGE`]) inserted into the Options list 0x1b4f78 right before
//! Quit Game, so Quit Game stays the last (and, with the list's wrap, the Up-from-top) entry as on the disc;
//! the description label W1 (flags 0x90, table 0x1b4fc8) gets a patched copy of its id table with the port
//! description at the same index ([`Label::table`]). [`PageMenu::install_port_page`] does all of it; without
//! the call every record is exactly as read from the overlay. The front end's Options list gets the same entry,
//! opening [`PAGE_FRONT`] with frame animations copied from its Language sub-page.
//!
//! Records live at addresses no EE pointer can hold (0x7f00_xxxx), and the text ids are negative (the disc's
//! ids are 0..=21500), so nothing here can collide with or be mistaken for game data.
//!
//! **Adding an option**: one [`Entry`] in [`ENTRIES`] (a [`Setting`] variant, a label id and its value ids in
//! [`text`]); the engine maps the [`Setting`] to its resource and persistence. The engine can restrict a row to
//! some of its values ([`PageMenu::set_port_choices`], e.g. the sample counts the GPU supports): ✕ then skips
//! the others.

use super::super::{text as print, MenuAssets, MenuDraw, MenuInput, MenuSound, Overlay};
use super::options::keys;
use super::{Data, Item, MenuOut, Page, PageMenu, Widget, LIGHT_BLUE, YELLOW};
use crate::pad::button;
use rc_formats::font::Font;

/// The page and its two own widgets (title label, settings list).
pub const PAGE: u32 = 0x7f00_0000;
pub const PAGE_FRONT: u32 = 0x7f00_0004;
pub const TITLE_W: u32 = 0x7f00_0100;
pub const TITLE_FRONT_W: u32 = 0x7f00_0104;
pub const LIST_W: u32 = 0x7f00_0200;
/// The list's callbacks (dispatched like the game's by address).
pub const UPDATE: u32 = 0x7f00_1000;
pub const DRAW: u32 = 0x7f00_1004;
/// The page kind (no game code keys on it).
pub const KIND: i32 = 0x7f;

/// Game records the page is attached to / modelled on (level01).
pub const OPTIONS: u32 = 0x1b4dd0;
pub const OPTIONS_LIST: u32 = 0x1b4f78;
pub const OPTIONS_LABEL: u32 = 0x1b4fe8;
pub const QUIT_PAGE: u32 = 0x1b6060;
/// Subtitles 0x1b5788 (seqs 73..77: the one/two-row sub-page frames) and Camera 0x1b5ee0 (118..122, three rows). Only an
/// Options sub-page's frames will do: a page's frame animations start from its parent's layout, and △ plays them
/// backwards to it (another page's, e.g. Cheats', end on their own parent's layout under the Options page).
pub const MODEL_FEW: u32 = 0x1b5788;
pub const MODEL_MANY: u32 = 0x1b5ee0;
/// The rows the Camera frame's list panel shows at once; the list scrolls ([`PortList::top`]).
/// Leave the bottom line for the setting description or display confirmation.
pub const VISIBLE: usize = 3;
/// Extra width of the middle frame on the Advanced Options pages.
pub const PANEL_X_SCALE: f32 = 1.6;
// The in-game page already uses the wide three-row frame model. Its frame
// meshes and 2D coordinates share a 4:3 viewport, so only a small increase fits.
const GAME_PANEL_X_SCALE: f32 = 1.1;

/// The port's own strings (not in the game's text table). Ids are negative; [`MenuAssets::msg`] resolves them.
pub mod text {
    pub const ENTRY: i32 = -0x100;
    pub const TITLE: i32 = -0x101;
    pub const DESCRIPTION: i32 = -0x102;
    pub const ANTI_ALIASING: i32 = -0x110;
    pub const X2: i32 = -0x111;
    pub const X4: i32 = -0x112;
    pub const X8: i32 = -0x113;
    pub const SHADOWS: i32 = -0x120;
    pub const RESOLUTION: i32 = -0x130;
    pub const RES_WINDOW: i32 = -0x131;
    pub const RES_416: i32 = -0x132;
    pub const RES_720: i32 = -0x133;
    pub const RES_1080: i32 = -0x134;
    pub const RES_1440: i32 = -0x135;
    pub const RES_2160: i32 = -0x136;
    pub const RENDER_SCALE: i32 = -0x137;
    pub const SCALE_50: i32 = -0x138;
    pub const SCALE_67: i32 = -0x139;
    pub const SCALE_75: i32 = -0x13a;
    pub const SCALE_100: i32 = -0x13b;
    pub const SCALE_125: i32 = -0x13c;
    pub const SCALE_150: i32 = -0x13d;
    pub const SCALE_200: i32 = -0x13e;
    pub const FULLSCREEN: i32 = -0x140;
    pub const VSYNC: i32 = -0x141;
    pub const FPS_LIMIT: i32 = -0x142;
    pub const FPS_30: i32 = -0x143;
    pub const FPS_60: i32 = -0x144;
    pub const FPS_120: i32 = -0x145;
    pub const FPS_144: i32 = -0x146;
    pub const FPS_165: i32 = -0x147;
    pub const FPS_240: i32 = -0x148;
    pub const FPS_UNLIMITED: i32 = -0x149;
    pub const LANGUAGE: i32 = -0x14a;
    pub const ENGLISH: i32 = -0x14b;
    pub const FRENCH: i32 = -0x14c;
    pub const GERMAN: i32 = -0x14d;
    pub const SPANISH: i32 = -0x14e;
    pub const ITALIAN: i32 = -0x14f;
    pub const PERFORMANCE: i32 = -0x15a;
    pub const ASPECT: i32 = -0x150;
    pub const ASPECT_4_3: i32 = -0x151;
    pub const ASPECT_16_9: i32 = -0x152;
    pub const ASPECT_16_10: i32 = -0x153;
    pub const ASPECT_21_9: i32 = -0x154;
    pub const ASPECT_32_9: i32 = -0x155;
    pub const PRESET: i32 = -0x160;
    pub const ORIGINAL: i32 = -0x161;
    pub const ENHANCED: i32 = -0x162;
    pub const CUSTOM: i32 = -0x163;
    pub const HUD: i32 = -0x170;
    pub const SHARP_PIXELS: i32 = -0x171;
    pub const TEXTURES: i32 = -0x180;
    pub const SMOOTH: i32 = -0x181;
    pub const SHARP: i32 = -0x182;
    pub const DETAIL: i32 = -0x190;
    pub const FAR: i32 = -0x191;
    pub const FARTHER: i32 = -0x192;
    pub const MAXIMUM: i32 = -0x193;
    pub const MONITOR: i32 = -0x1a0;
    pub const SCREEN_CURRENT: i32 = -0x1a1;
    pub const SCREEN_1: i32 = -0x1a2;
    pub const SCREEN_2: i32 = -0x1a3;
    pub const SCREEN_3: i32 = -0x1a4;
    pub const SCREEN_4: i32 = -0x1a5;
    pub const SCREEN_5: i32 = -0x1b9;
    pub const SCREEN_6: i32 = -0x1ba;
    pub const SCREEN_7: i32 = -0x1bb;
    pub const SCREEN_8: i32 = -0x1bc;
    pub const WINDOWED: i32 = -0x1a6;
    pub const BORDERLESS: i32 = -0x1a7;
    pub const EXCLUSIVE: i32 = -0x1a8;
    pub const REFRESH: i32 = -0x1a9;
    pub const AUTO: i32 = -0x1aa;
    pub const HZ_75: i32 = -0x1ab;
    pub const HZ_90: i32 = -0x1ac;
    pub const CUSTOM_HEIGHT: i32 = -0x1ad;
    pub const RES_CUSTOM: i32 = -0x1ae;
    pub const POST_AA: i32 = -0x1af;
    pub const FXAA: i32 = -0x1b0;
    pub const SMAA: i32 = -0x1b1;
    pub const SHARPNESS: i32 = -0x1b2;
    pub const LOW: i32 = -0x1b3;
    pub const MEDIUM: i32 = -0x1b4;
    pub const HIGH: i32 = -0x1b5;
    pub const ANISOTROPY: i32 = -0x1b6;
    pub const X1: i32 = -0x1b7;
    pub const X16: i32 = -0x1b8;
    pub const CAMERA_SENSITIVITY: i32 = -0x1c0;
    pub const STICK_DEADZONE: i32 = -0x1c1;
    pub const SUBTITLE_SIZE: i32 = -0x1c2;
    pub const SUBTITLE_CONTRAST: i32 = -0x1c3;
    pub const CAMERA_SHAKE: i32 = -0x1c4;
    pub const SLOW: i32 = -0x1c5;
    pub const NORMAL: i32 = -0x1c6;
    pub const LARGE: i32 = -0x1c7;
    pub const EXTRA_LARGE: i32 = -0x1c8;
    pub const HALF: i32 = -0x1c9;
    pub const BUTTON_PROMPTS: i32 = -0x1ca;
    pub const PLAYSTATION: i32 = -0x1cb;
    pub const XBOX: i32 = -0x1cc;
    pub const NINTENDO: i32 = -0x1cd;
    pub const DUALSENSE: i32 = -0x1ce;
    pub const DUALSHOCK4: i32 = -0x1cf;
    pub const DUALSHOCK3: i32 = -0x1d0;
    pub const HUD_HIGH_RES: i32 = -0x1d1;
    pub const XBOX_SERIES: i32 = -0x1d2;
    pub const XBOX_ONE: i32 = -0x1d3;
    pub const XBOX_360: i32 = -0x1d4;
    pub const DLSS5: i32 = -0x1d5;
    pub const UNAVAILABLE: i32 = -0x1d6;
    pub const DLSS5_PRESET: i32 = -0x1d7;
    pub const CNN_E: i32 = -0x1d8;
    pub const CNN_F: i32 = -0x1d9;
    pub const TRANSFORMER_J: i32 = -0x1da;
    pub const TRANSFORMER_K: i32 = -0x1db;
    pub const ENABLE_DLSS5: i32 = -0x1dc;
    /// The game's own "on" / "off" (20314 / 20315, the Subtitles / HelpDesk toggle values).
    pub const ON: i32 = 20314;
    pub const OFF: i32 = 20315;

    pub fn get(id: i32) -> Option<&'static [u8]> {
        Some(match id {
            ENTRY => b"Adv. Options",
            TITLE => b"Advanced Options",
            DESCRIPTION => b"Settings of this port that the original game does not have",
            ANTI_ALIASING => b"Anti-aliasing",
            X2 => b"2x",
            X4 => b"4x",
            X8 => b"8x",
            SHADOWS => b"Shadows",
            RESOLUTION => b"Resolution",
            RES_WINDOW => b"Window",
            RES_416 => b"416p",
            RES_720 => b"720p",
            RES_1080 => b"1080p",
            RES_1440 => b"1440p",
            RES_2160 => b"2160p",
            RENDER_SCALE => b"Render scale",
            SCALE_50 => b"50%", SCALE_67 => b"67%", SCALE_75 => b"75%", SCALE_100 => b"100%",
            SCALE_125 => b"125%", SCALE_150 => b"150%", SCALE_200 => b"200%",
            FULLSCREEN => b"Fullscreen",
            VSYNC => b"V-Sync",
            FPS_LIMIT => b"FPS limit",
            FPS_30 => b"30",
            FPS_60 => b"60",
            FPS_120 => b"120",
            FPS_144 => b"144",
            FPS_165 => b"165",
            FPS_240 => b"240",
            FPS_UNLIMITED => b"Unlimited",
            LANGUAGE => b"Language",
            ENGLISH => b"English",
            FRENCH => b"French",
            GERMAN => b"German",
            SPANISH => b"Spanish",
            ITALIAN => b"Italian",
            PERFORMANCE => b"FPS display",
            ASPECT => b"Aspect ratio",
            ASPECT_4_3 => b"4:3",
            ASPECT_16_9 => b"16:9",
            ASPECT_16_10 => b"16:10",
            ASPECT_21_9 => b"21:9",
            ASPECT_32_9 => b"32:9",
            PRESET => b"Preset",
            ORIGINAL => b"Original",
            ENHANCED => b"Enhanced",
            CUSTOM => b"Custom",
            HUD => b"HUD",
            SHARP_PIXELS => b"Sharp pixels",
            TEXTURES => b"Texture filtering",
            SMOOTH => b"Smooth",
            SHARP => b"Sharp",
            DETAIL => b"Detail distance",
            FAR => b"Far",
            FARTHER => b"Farther",
            MAXIMUM => b"Maximum",
            MONITOR => b"Monitor", SCREEN_CURRENT => b"Current", SCREEN_1 => b"Monitor 1",
            SCREEN_2 => b"Monitor 2", SCREEN_3 => b"Monitor 3", SCREEN_4 => b"Monitor 4",
            SCREEN_5 => b"Monitor 5", SCREEN_6 => b"Monitor 6", SCREEN_7 => b"Monitor 7", SCREEN_8 => b"Monitor 8",
            WINDOWED => b"Windowed", BORDERLESS => b"Borderless", EXCLUSIVE => b"Exclusive",
            REFRESH => b"Refresh rate", AUTO => b"Auto", HZ_75 => b"75", HZ_90 => b"90",
            CUSTOM_HEIGHT => b"Custom height", RES_CUSTOM => b"Custom",
            POST_AA => b"Post anti-aliasing", FXAA => b"FXAA", SMAA => b"SMAA",
            SHARPNESS => b"Sharpness", LOW => b"Low", MEDIUM => b"Medium", HIGH => b"High",
            ANISOTROPY => b"Anisotropic filter", X1 => b"1x", X16 => b"16x",
            CAMERA_SENSITIVITY => b"Camera sensitivity", STICK_DEADZONE => b"Stick deadzone",
            SUBTITLE_SIZE => b"Subtitle size", SUBTITLE_CONTRAST => b"Subtitle contrast",
            CAMERA_SHAKE => b"Camera shake", SLOW => b"Slow", NORMAL => b"Normal",
            LARGE => b"Large", EXTRA_LARGE => b"Extra large", HALF => b"Half",
            BUTTON_PROMPTS => b"Button icons", PLAYSTATION => b"PlayStation", XBOX => b"Xbox", NINTENDO => b"Switch",
            DUALSENSE => b"DualSense", DUALSHOCK4 => b"DualShock 4", DUALSHOCK3 => b"DualShock 3",
            HUD_HIGH_RES => b"High resolution",
            XBOX_SERIES => b"Xbox Series", XBOX_ONE => b"Xbox One", XBOX_360 => b"Xbox 360",
            DLSS5 => b"DLSS 5", UNAVAILABLE => b"Unavailable",
            DLSS5_PRESET => b"DLSS 5 preset", CNN_E => b"CNN E", CNN_F => b"CNN F",
            TRANSFORMER_J => b"Transformer J", TRANSFORMER_K => b"Transformer K",
            ENABLE_DLSS5 => b"Enable DLSS 5",
            _ => return None,
        })
    }

    /// Port-only text in the disc language slots (0 English, 2 French, 3 German, 4 Spanish,
    /// 5 Italian). Use the game's single-byte glyph encoding; 0xa9 is the native ñ.
    pub fn get_localized(id: i32, lang: u32) -> Option<&'static [u8]> {
        let translated: Option<&'static [u8]> = Some(match (lang, id) {
            (2, ENTRY) => b"Opt. avancees", (2, TITLE) => b"Options avancees",
            (2, DESCRIPTION) => b"Reglages supplementaires du port PC",
            (2, PRESET) => b"Profil", (2, ORIGINAL) => b"Original", (2, ENHANCED) => b"Ameliore", (2, CUSTOM) => b"Personnalise",
            (2, ANTI_ALIASING) => b"Anticrenelage", (2, SHADOWS) => b"Ombres", (2, RESOLUTION) => b"Resolution",
            (2, BUTTON_PROMPTS) => b"Icones manette",
            (2, DLSS5_PRESET) => b"Profil DLSS 5", (2, ENABLE_DLSS5) => b"Activer DLSS 5",
            (2, RES_WINDOW) => b"Fenetre", (2, FULLSCREEN) => b"Plein ecran", (2, VSYNC) => b"Synchro V",
            (2, RENDER_SCALE) => b"Echelle de rendu",
            (2, MONITOR) => b"Moniteur", (2, SCREEN_CURRENT) => b"Actuel", (2, WINDOWED) => b"Fenetre",
            (2, BORDERLESS) => b"Sans bordure", (2, EXCLUSIVE) => b"Exclusif", (2, REFRESH) => b"Frequence",
            (2, CUSTOM_HEIGHT) => b"Hauteur perso.", (2, RES_CUSTOM) => b"Perso.",
            (2, POST_AA) => b"Anticrenelage final", (2, SHARPNESS) => b"Nettete",
            (2, LOW) => b"Faible", (2, MEDIUM) => b"Moyen", (2, HIGH) => b"Eleve",
            (2, ANISOTROPY) => b"Filtre anisotrope",
            (2, FPS_LIMIT) => b"Limite IPS", (2, FPS_UNLIMITED) => b"Illimite", (2, LANGUAGE) => b"Langue",
            (2, ENGLISH) => b"Anglais", (2, FRENCH) => b"Francais", (2, GERMAN) => b"Allemand",
            (2, PERFORMANCE) => b"Afficher IPS",
            (2, SPANISH) => b"Espagnol", (2, ITALIAN) => b"Italien", (2, ASPECT) => b"Format",
            (2, HUD) => b"Interface", (2, SHARP_PIXELS) => b"Pixels nets", (2, TEXTURES) => b"Filtrage textures",
            (2, HUD_HIGH_RES) => b"Haute resolution",
            (2, SMOOTH) => b"Doux", (2, SHARP) => b"Net", (2, DETAIL) => b"Distance details",
            (2, FAR) => b"Loin", (2, FARTHER) => b"Plus loin", (2, MAXIMUM) => b"Maximum",
            (3, ENTRY) => b"Erw. Optionen", (3, TITLE) => b"Erweiterte Optionen",
            (3, DESCRIPTION) => b"Zusaetzliche PC-Einstellungen",
            (3, BUTTON_PROMPTS) => b"Controller-Symbole",
            (3, DLSS5_PRESET) => b"DLSS 5 Profil", (3, ENABLE_DLSS5) => b"DLSS 5 aktivieren",
            (3, PRESET) => b"Profil", (3, ORIGINAL) => b"Original", (3, ENHANCED) => b"Verbessert", (3, CUSTOM) => b"Benutzerdef.",
            (3, ANTI_ALIASING) => b"Kantenglaettung", (3, SHADOWS) => b"Schatten", (3, RESOLUTION) => b"Aufloesung",
            (3, RES_WINDOW) => b"Fenster", (3, FULLSCREEN) => b"Vollbild", (3, VSYNC) => b"V-Sync",
            (3, RENDER_SCALE) => b"Renderskalierung",
            (3, MONITOR) => b"Monitor", (3, SCREEN_CURRENT) => b"Aktuell", (3, WINDOWED) => b"Fenster",
            (3, BORDERLESS) => b"Randlos", (3, EXCLUSIVE) => b"Exklusiv", (3, REFRESH) => b"Bildfrequenz",
            (3, CUSTOM_HEIGHT) => b"Eigene Hoehe", (3, RES_CUSTOM) => b"Eigene",
            (3, POST_AA) => b"Bild-Kantenglaettung", (3, SHARPNESS) => b"Schaerfe",
            (3, LOW) => b"Niedrig", (3, MEDIUM) => b"Mittel", (3, HIGH) => b"Hoch",
            (3, ANISOTROPY) => b"Anisotropie",
            (3, FPS_LIMIT) => b"FPS-Limit", (3, FPS_UNLIMITED) => b"Unbegrenzt", (3, LANGUAGE) => b"Sprache",
            (3, ENGLISH) => b"Englisch", (3, FRENCH) => b"Franzoesisch", (3, GERMAN) => b"Deutsch",
            (3, PERFORMANCE) => b"FPS-Anzeige",
            (3, SPANISH) => b"Spanisch", (3, ITALIAN) => b"Italienisch", (3, ASPECT) => b"Bildformat",
            (3, HUD) => b"Anzeige", (3, SHARP_PIXELS) => b"Scharfe Pixel", (3, TEXTURES) => b"Texturfilter",
            (3, HUD_HIGH_RES) => b"Hohe Aufloesung",
            (3, SMOOTH) => b"Weich", (3, SHARP) => b"Scharf", (3, DETAIL) => b"Detailabstand",
            (3, FAR) => b"Weit", (3, FARTHER) => b"Weiter", (3, MAXIMUM) => b"Maximum",
            (4, ENTRY) => b"Opc. avanzadas", (4, TITLE) => b"Opciones avanzadas",
            (4, DESCRIPTION) => b"Ajustes adicionales del port de PC",
            (4, PRESET) => b"Perfil", (4, ORIGINAL) => b"Original", (4, ENHANCED) => b"Mejorado", (4, CUSTOM) => b"Personalizado",
            (4, ANTI_ALIASING) => b"Antialiasing", (4, SHADOWS) => b"Sombras", (4, RESOLUTION) => b"Resolucion",
            (4, RES_WINDOW) => b"Ventana", (4, FULLSCREEN) => b"Pantalla completa", (4, VSYNC) => b"Sincronizacion V",
            (4, RENDER_SCALE) => b"Escala de imagen",
            (4, MONITOR) => b"Monitor", (4, SCREEN_CURRENT) => b"Actual", (4, WINDOWED) => b"Ventana",
            (4, BORDERLESS) => b"Sin bordes", (4, EXCLUSIVE) => b"Exclusiva", (4, REFRESH) => b"Frecuencia",
            (4, CUSTOM_HEIGHT) => b"Altura manual", (4, RES_CUSTOM) => b"Manual",
            (4, POST_AA) => b"Antialiasing final", (4, SHARPNESS) => b"Nitidez",
            (4, LOW) => b"Baja", (4, MEDIUM) => b"Media", (4, HIGH) => b"Alta",
            (4, ANISOTROPY) => b"Filtro anisotropico",
            (4, FPS_LIMIT) => b"Limite FPS", (4, FPS_UNLIMITED) => b"Sin limite", (4, LANGUAGE) => b"Idioma",
            (4, ENGLISH) => b"Ingles", (4, FRENCH) => b"Frances", (4, GERMAN) => b"Aleman",
            (4, PERFORMANCE) => b"Mostrar FPS",
            (4, SPANISH) => b"Espa\xa9ol", (4, ITALIAN) => b"Italiano", (4, ASPECT) => b"Formato",
            (4, HUD) => b"Interfaz", (4, SHARP_PIXELS) => b"Pixeles nitidos", (4, TEXTURES) => b"Filtrado texturas",
            (4, HUD_HIGH_RES) => b"Alta resolucion",
            (4, SMOOTH) => b"Suave", (4, SHARP) => b"Nitido", (4, DETAIL) => b"Distancia detalle",
            (4, FAR) => b"Lejos", (4, FARTHER) => b"Mas lejos", (4, MAXIMUM) => b"Maximo",
            (4, CAMERA_SENSITIVITY) => b"Sensibilidad camara", (4, STICK_DEADZONE) => b"Zona muerta mando",
            (4, SUBTITLE_SIZE) => b"Tama\xa9o subtitulos", (4, SUBTITLE_CONTRAST) => b"Contraste subtitulos",
            (4, CAMERA_SHAKE) => b"Sacudida camara", (4, SLOW) => b"Lenta", (4, NORMAL) => b"Normal",
            (4, LARGE) => b"Grande", (4, EXTRA_LARGE) => b"Muy grande", (4, HALF) => b"Media",
            (4, BUTTON_PROMPTS) => b"Iconos del mando",
            (4, UNAVAILABLE) => b"No disponible",
            (4, AUTO) => b"Automatico",
            (4, DLSS5_PRESET) => b"Perfil DLSS 5",
            (4, ENABLE_DLSS5) => b"Activa DLSS 5",
            (5, ENTRY) => b"Opz. avanzate", (5, TITLE) => b"Opzioni avanzate",
            (5, BUTTON_PROMPTS) => b"Icone controller",
            (5, DLSS5_PRESET) => b"Profilo DLSS 5", (5, ENABLE_DLSS5) => b"Attiva DLSS 5",
            (5, DESCRIPTION) => b"Impostazioni aggiuntive del port PC",
            (5, PRESET) => b"Profilo", (5, ORIGINAL) => b"Originale", (5, ENHANCED) => b"Migliorato", (5, CUSTOM) => b"Personalizzato",
            (5, ANTI_ALIASING) => b"Antialiasing", (5, SHADOWS) => b"Ombre", (5, RESOLUTION) => b"Risoluzione",
            (5, RES_WINDOW) => b"Finestra", (5, FULLSCREEN) => b"Schermo intero", (5, VSYNC) => b"Sincronia V",
            (5, RENDER_SCALE) => b"Scala rendering",
            (5, MONITOR) => b"Monitor", (5, SCREEN_CURRENT) => b"Attuale", (5, WINDOWED) => b"Finestra",
            (5, BORDERLESS) => b"Senza bordi", (5, EXCLUSIVE) => b"Esclusiva", (5, REFRESH) => b"Frequenza",
            (5, CUSTOM_HEIGHT) => b"Altezza manuale", (5, RES_CUSTOM) => b"Manuale",
            (5, POST_AA) => b"Antialiasing finale", (5, SHARPNESS) => b"Nitidezza",
            (5, LOW) => b"Bassa", (5, MEDIUM) => b"Media", (5, HIGH) => b"Alta",
            (5, ANISOTROPY) => b"Filtro anisotropico",
            (5, FPS_LIMIT) => b"Limite FPS", (5, FPS_UNLIMITED) => b"Illimitato", (5, LANGUAGE) => b"Lingua",
            (5, ENGLISH) => b"Inglese", (5, FRENCH) => b"Francese", (5, GERMAN) => b"Tedesco",
            (5, PERFORMANCE) => b"Mostra FPS",
            (5, SPANISH) => b"Spagnolo", (5, ITALIAN) => b"Italiano", (5, ASPECT) => b"Formato",
            (5, HUD) => b"Interfaccia", (5, SHARP_PIXELS) => b"Pixel nitidi", (5, TEXTURES) => b"Filtro texture",
            (5, HUD_HIGH_RES) => b"Alta risoluzione",
            (5, SMOOTH) => b"Morbido", (5, SHARP) => b"Nitido", (5, DETAIL) => b"Distanza dettagli",
            (5, FAR) => b"Lontano", (5, FARTHER) => b"Piu lontano", (5, MAXIMUM) => b"Massimo",
            _ => return get(id),
        });
        translated.or_else(|| get(id))
    }
}

/// A port setting (the engine owns the value's meaning and its persistence).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Setting {
    /// The graphics preset: value 0 Original, 1 Enhanced, 2 Custom (shown when the rows match no preset; ✕ skips it,
    /// the engine restricts the row).
    Preset,
    /// Anti-aliasing: value 0 Original (the game's own softening passes), 1 off, 2 2x, 3 4x, 4 8x multisampling.
    AntiAliasing,
    PostAa,
    Sharpness,
    Anisotropy,
    /// The moby shadows (docs/plan/shadows.md): value 0 on (the game's look, the default), 1 off.
    Shadows,
    /// The render resolution (the engine's game frame): value 0 fits the window, 1..5 a height of 416 (the PS2's lines), 720, 1080, 1440 or 2160 (the width
    /// follows the Aspect row).
    Resolution,
    CustomHeight,
    RenderScale,
    /// Windowed, borderless or exclusive fullscreen.
    Fullscreen,
    Monitor,
    RefreshRate,
    Vsync,
    FpsLimit,
    Performance,
    Dlss5,
    Dlss5Preset,
    Language,
    ButtonPrompts,
    CameraSensitivity,
    StickDeadzone,
    SubtitleSize,
    SubtitleContrast,
    CameraShake,
    /// The frame's aspect: value 0 the TV's 4:3, 1 16:10, 2 16:9 (the 3D view widens; the 2D screen stays 4:3, centred).
    Aspect,
    /// How the game's 2D screen reaches the frame: value 0 Original (scaled smoothly, as on a TV), 1 Sharp pixels.
    Hud,
    /// How world textures pick their detail level: value 0 Original (the PS2's rule), 1 Smooth, 2 Sharp.
    Textures,
    /// How far away models keep their full detail: value 0 Original (the game's distances), 1 Far (×2), 2 Farther (×4),
    /// 3 Maximum (always).
    Detail,
}

/// One row: the setting, its label and the ids of its values (✕ cycles through them).
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub setting: Setting,
    pub label: i32,
    pub values: &'static [i32],
}

/// The rows, in order.
pub const ENTRIES: &[Entry] = &[
    Entry { setting: Setting::Preset, label: text::PRESET, values: &[text::ORIGINAL, text::ENHANCED, text::CUSTOM] },
    Entry { setting: Setting::AntiAliasing, label: text::ANTI_ALIASING, values: &[text::ORIGINAL, text::OFF, text::X2, text::X4, text::X8] },
    Entry { setting: Setting::PostAa, label: text::POST_AA, values: &[text::OFF, text::FXAA, text::SMAA] },
    Entry { setting: Setting::Sharpness, label: text::SHARPNESS, values: &[text::OFF, text::LOW, text::MEDIUM, text::HIGH] },
    Entry { setting: Setting::Anisotropy, label: text::ANISOTROPY, values: &[text::X1, text::X2, text::X4, text::X8, text::X16] },
    Entry { setting: Setting::Detail, label: text::DETAIL, values: &[text::ORIGINAL, text::FAR, text::FARTHER, text::MAXIMUM] },
    Entry { setting: Setting::Textures, label: text::TEXTURES, values: &[text::ORIGINAL, text::SMOOTH, text::SHARP] },
    Entry { setting: Setting::Hud, label: text::HUD, values: &[text::ORIGINAL, text::SHARP_PIXELS, text::HUD_HIGH_RES] },
    Entry { setting: Setting::Shadows, label: text::SHADOWS, values: &[text::ON, text::OFF] },
    Entry { setting: Setting::Aspect, label: text::ASPECT, values: &[text::ASPECT_4_3, text::ASPECT_16_10, text::ASPECT_16_9, text::ASPECT_21_9, text::ASPECT_32_9] },
    Entry { setting: Setting::Resolution, label: text::RESOLUTION, values: &[text::RES_WINDOW, text::RES_416, text::RES_720, text::RES_1080, text::RES_1440, text::RES_2160, text::RES_CUSTOM] },
    Entry { setting: Setting::CustomHeight, label: text::CUSTOM_HEIGHT, values: &[text::RES_CUSTOM] },
    Entry { setting: Setting::RenderScale, label: text::RENDER_SCALE, values: &[text::SCALE_50, text::SCALE_67, text::SCALE_75, text::SCALE_100, text::SCALE_125, text::SCALE_150, text::SCALE_200] },
    Entry { setting: Setting::Fullscreen, label: text::FULLSCREEN, values: &[text::WINDOWED, text::BORDERLESS, text::EXCLUSIVE] },
    Entry { setting: Setting::Monitor, label: text::MONITOR, values: &[text::SCREEN_CURRENT, text::SCREEN_1, text::SCREEN_2, text::SCREEN_3, text::SCREEN_4, text::SCREEN_5, text::SCREEN_6, text::SCREEN_7, text::SCREEN_8] },
    Entry { setting: Setting::RefreshRate, label: text::REFRESH, values: &[text::AUTO, text::FPS_60, text::HZ_75, text::HZ_90, text::FPS_120, text::FPS_144, text::FPS_165, text::FPS_240] },
    Entry { setting: Setting::Vsync, label: text::VSYNC, values: &[text::ON, text::OFF] },
    Entry { setting: Setting::FpsLimit, label: text::FPS_LIMIT, values: &[text::FPS_30, text::FPS_60, text::FPS_120, text::FPS_144, text::FPS_165, text::FPS_240, text::FPS_UNLIMITED] },
    Entry { setting: Setting::Performance, label: text::PERFORMANCE, values: &[text::OFF, text::ON] },
    Entry { setting: Setting::Dlss5, label: text::DLSS5, values: &[text::OFF, text::ON, text::UNAVAILABLE] },
    Entry { setting: Setting::Dlss5Preset, label: text::DLSS5_PRESET, values: &[text::AUTO, text::CNN_E, text::CNN_F, text::TRANSFORMER_J, text::TRANSFORMER_K, text::ENABLE_DLSS5] },
    Entry { setting: Setting::Language, label: text::LANGUAGE, values: &[text::ENGLISH, text::FRENCH, text::GERMAN, text::SPANISH, text::ITALIAN] },
    Entry { setting: Setting::ButtonPrompts, label: text::BUTTON_PROMPTS, values: &[text::AUTO, text::DUALSENSE, text::DUALSHOCK4, text::DUALSHOCK3, text::XBOX_SERIES, text::XBOX_ONE, text::XBOX_360, text::NINTENDO, text::PLAYSTATION, text::XBOX] },
    Entry { setting: Setting::CameraSensitivity, label: text::CAMERA_SENSITIVITY, values: &[text::SLOW, text::NORMAL, text::HIGH] },
    Entry { setting: Setting::StickDeadzone, label: text::STICK_DEADZONE, values: &[text::ORIGINAL, text::LOW, text::MEDIUM] },
    Entry { setting: Setting::SubtitleSize, label: text::SUBTITLE_SIZE, values: &[text::ORIGINAL, text::LARGE, text::EXTRA_LARGE] },
    Entry { setting: Setting::SubtitleContrast, label: text::SUBTITLE_CONTRAST, values: &[text::ORIGINAL, text::HIGH] },
    Entry { setting: Setting::CameraShake, label: text::CAMERA_SHAKE, values: &[text::ON, text::HALF, text::OFF] },
];

/// The NTSC-U disc's language numbers; slot 1 is unused.
pub const LANGUAGES: [u32; 5] = [0, 2, 3, 4, 5];
pub fn language_index(lang: u32) -> u8 { LANGUAGES.iter().position(|&v| v == lang).unwrap_or(0) as u8 }
pub fn language_from_index(index: u8) -> u32 { LANGUAGES.get(index as usize).copied().unwrap_or(0) }

fn description(setting: Setting, lang: u32) -> &'static [u8] {
    match (lang, setting) {
        (2, Setting::Preset) => b"Choisir la qualite d'image", (2, Setting::AntiAliasing) => b"Lisser les contours 3D",
        (2, Setting::PostAa) => b"Lisser l'image finale", (2, Setting::Sharpness) => b"Accentuer les details",
        (2, Setting::Anisotropy) => b"Textures aux angles obliques",
        (2, Setting::Detail) => b"Details des objets lointains", (2, Setting::Textures) => b"Filtrage des textures",
        (2, Setting::Hud) => b"Filtrage de l'interface", (2, Setting::Shadows) => b"Ombres des personnages",
        (2, Setting::Aspect) => b"Elargir la vue 3D", (2, Setting::Resolution) => b"Hauteur de rendu interne",
        (2, Setting::RenderScale) => b"Taille du rendu 3D",
        (2, Setting::CustomHeight) => b"G/D 16 lignes, X/Carre 1",
        (2, Setting::Fullscreen) => b"Mode sans bordure", (2, Setting::Vsync) => b"Synchroniser l'affichage",
        (2, Setting::Monitor) => b"Choisir un moniteur", (2, Setting::RefreshRate) => b"Mode exclusif uniquement",
        (2, Setting::FpsLimit) => b"Frequence de rendu maximale", (2, Setting::Language) => b"Langue des textes et voix",
        (2, Setting::Performance) => b"IPS et duree des images",
        (3, Setting::Preset) => b"Bildqualitaet waehlen", (3, Setting::AntiAliasing) => b"3D-Kanten glaetten",
        (3, Setting::PostAa) => b"Fertiges Bild glaetten", (3, Setting::Sharpness) => b"Bilddetails hervorheben",
        (3, Setting::Anisotropy) => b"Texturen aus schraeger Sicht",
        (3, Setting::Detail) => b"Details entfernter Objekte", (3, Setting::Textures) => b"Texturfilterung",
        (3, Setting::Hud) => b"Anzeigefilter", (3, Setting::Shadows) => b"Figurenschatten",
        (3, Setting::Aspect) => b"3D-Sicht verbreitern", (3, Setting::Resolution) => b"Interne Renderhoehe",
        (3, Setting::RenderScale) => b"Interne Rendergroesse",
        (3, Setting::CustomHeight) => b"L/R 16 Zeilen, X/Viereck 1",
        (3, Setting::Fullscreen) => b"Randloses Vollbild", (3, Setting::Vsync) => b"Bildschirm synchronisieren",
        (3, Setting::Monitor) => b"Bildschirm waehlen", (3, Setting::RefreshRate) => b"Nur im Exklusivmodus",
        (3, Setting::FpsLimit) => b"Maximale Bildrate", (3, Setting::Language) => b"Text- und Sprachausgabe",
        (3, Setting::Performance) => b"FPS und Bildzeit anzeigen",
        (4, Setting::Preset) => b"Elegir calidad de imagen", (4, Setting::AntiAliasing) => b"Suavizar bordes 3D",
        (4, Setting::PostAa) => b"Suavizar imagen final", (4, Setting::Sharpness) => b"Realzar los detalles",
        (4, Setting::Anisotropy) => b"Texturas en angulos oblicuos",
        (4, Setting::Detail) => b"Detalle de objetos lejanos", (4, Setting::Textures) => b"Filtrado de texturas",
        (4, Setting::Hud) => b"Filtro de la interfaz", (4, Setting::Shadows) => b"Sombras de personajes",
        (4, Setting::Aspect) => b"Ampliar la vista 3D", (4, Setting::Resolution) => b"Altura de renderizado",
        (4, Setting::RenderScale) => b"Tama\xa9o de renderizado interno",
        (4, Setting::CustomHeight) => b"Izq/Der 16 lineas, X/Cuadrado 1",
        (4, Setting::Fullscreen) => b"Ventana sin bordes", (4, Setting::Vsync) => b"Sincronizar con pantalla",
        (4, Setting::Monitor) => b"Seleccionar pantalla", (4, Setting::RefreshRate) => b"Solo en modo exclusivo",
        (4, Setting::FpsLimit) => b"Frecuencia maxima de imagen", (4, Setting::Language) => b"Idioma de texto y voces",
        (4, Setting::Performance) => b"FPS y tiempo por imagen",
        (4, Setting::Dlss5) => b"Efecto neural opcional (complemento externo)",
        (4, Setting::Dlss5Preset) => b"Reconstruccion temporal del complemento",
        (4, Setting::CameraSensitivity) => b"Velocidad del giro de camara", (4, Setting::StickDeadzone) => b"Filtrar movimientos peque\xa9os",
        (4, Setting::SubtitleSize) => b"Aumentar tama\xa9o de subtitulos", (4, Setting::SubtitleContrast) => b"Mejorar lectura de subtitulos",
        (4, Setting::CameraShake) => b"Reducir movimiento de camara",
        (4, Setting::ButtonPrompts) => b"Iconos segun mando y controles",
        (5, Setting::Preset) => b"Scegliere qualita immagine", (5, Setting::AntiAliasing) => b"Smussare i bordi 3D",
        (5, Setting::PostAa) => b"Smussare immagine finale", (5, Setting::Sharpness) => b"Evidenziare i dettagli",
        (5, Setting::Anisotropy) => b"Texture ad angoli obliqui",
        (5, Setting::Detail) => b"Dettagli degli oggetti lontani", (5, Setting::Textures) => b"Filtro delle texture",
        (5, Setting::Hud) => b"Filtro interfaccia", (5, Setting::Shadows) => b"Ombre dei personaggi",
        (5, Setting::Aspect) => b"Ampliare la vista 3D", (5, Setting::Resolution) => b"Altezza rendering interna",
        (5, Setting::RenderScale) => b"Dimensione rendering interno",
        (5, Setting::CustomHeight) => b"S/D 16 righe, X/Quadrato 1",
        (5, Setting::Fullscreen) => b"Schermo senza bordi", (5, Setting::Vsync) => b"Sincronizzare il display",
        (5, Setting::Monitor) => b"Scegliere monitor", (5, Setting::RefreshRate) => b"Solo modo esclusivo",
        (5, Setting::FpsLimit) => b"Frequenza massima", (5, Setting::Language) => b"Lingua di testi e voci",
        (5, Setting::Performance) => b"FPS e tempo per immagine",
        (_, setting) => description_english(setting),
    }
}

fn description_english(setting: Setting) -> &'static [u8] {
    match setting {
        Setting::Preset => b"Set image quality",
        Setting::AntiAliasing => b"Smooth 3D edges",
        Setting::PostAa => b"Smooth final image edges",
        Setting::Sharpness => b"Sharpen the final image",
        Setting::Anisotropy => b"Texture detail at oblique angles",
        Setting::Detail => b"Distant model detail",
        Setting::Textures => b"Texture filtering",
        Setting::Hud => b"HUD scaling filter",
        Setting::Shadows => b"Character shadows",
        Setting::Aspect => b"Widen the 3D view",
        Setting::Resolution => b"Internal frame height",
        Setting::RenderScale => b"Internal rendering size",
        Setting::CustomHeight => b"Left/right 16 lines, X/square 1",
        Setting::Fullscreen => b"Borderless screen",
        Setting::Monitor => b"Choose a connected screen",
        Setting::RefreshRate => b"Exclusive fullscreen only",
        Setting::Vsync => b"Sync to the display",
        Setting::FpsLimit => b"Maximum render rate",
        Setting::Performance => b"FPS and frame time overlay",
        Setting::Dlss5 => b"Optional neural effect (external add-on)",
        Setting::Dlss5Preset => b"External add-on temporal reconstruction preset",
        Setting::Language => b"Text and voice language",
        Setting::CameraSensitivity => b"Camera turn speed",
        Setting::StickDeadzone => b"Ignore small stick movements",
        Setting::SubtitleSize => b"Enlarge subtitles",
        Setting::SubtitleContrast => b"Increase subtitle readability",
        Setting::CameraShake => b"Reduce camera shake",
        Setting::ButtonPrompts => b"Match the controller and remapped buttons",
    }
}

/// The list's run-time state: cursor, each row's value index and each row's selectable values (bit k = value k;
/// a missing entry = all).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PortList {
    pub cursor: i32,
    /// The first row shown ([`VISIBLE`] at a time; the cursor stays among them).
    pub top: i32,
    pub values: Vec<u8>,
    pub choices: Vec<u32>,
    /// Some while a display change needs ✕ confirmation or △ rollback.
    pub confirm_seconds: Option<u8>,
    pub confirm_action: Option<bool>,
    /// An arbitrary internal frame height, 240..=2160, when Resolution is Custom.
    pub custom_height: u16,
}

impl PortList {
    fn move_cursor(&mut self, rows: i32) -> bool {
        let old = self.cursor;
        self.cursor = self.cursor.saturating_add(rows).clamp(0, ENTRIES.len() as i32 - 1);
        self.top = self.top.clamp(self.cursor - VISIBLE as i32 + 1, self.cursor).max(0);
        self.cursor != old
    }
}

/// The value after `v` among `n` values that `choices` allows (wrapping); `v` when no other is allowed.
fn next_choice(v: u8, n: usize, choices: u32) -> u8 {
    (1..=n).map(|k| (v as usize + k) % n.max(1)).find(|&i| choices & (1 << i) != 0).unwrap_or(v as usize) as u8
}

fn list(m: &PageMenu) -> Option<&PortList> {
    match m.widgets.get(&LIST_W).map(|w| &w.data) {
        Some(Data::Port(p)) => Some(p),
        _ => None,
    }
}

fn list_mut(m: &mut PageMenu, w: u32) -> Option<&mut PortList> {
    match m.widgets.get_mut(&w).map(|w| &mut w.data) {
        Some(Data::Port(p)) => Some(p),
        _ => None,
    }
}

/// The row of a setting in [`ENTRIES`].
pub fn row_of(s: Setting) -> Option<usize> { ENTRIES.iter().position(|e| e.setting == s) }

impl PageMenu {
    /// Width of the middle frame, blended across the native page transition.
    pub fn port_panel_scale(&self) -> f32 {
        let scale = |p| match p { PAGE => GAME_PANEL_X_SCALE, PAGE_FRONT => PANEL_X_SCALE, _ => 1.0 };
        if matches!(self.current, PAGE | PAGE_FRONT) { return scale(self.current); }
        if self.kind == 1 {
            let t = (self.trans as f32 / 12.0).clamp(0.0, 1.0);
            if matches!(self.target, PAGE | PAGE_FRONT) { return 1.0 + (scale(self.target) - 1.0) * (1.0 - t); }
            if matches!(self.prev, PAGE | PAGE_FRONT) { return 1.0 + (scale(self.prev) - 1.0) * t; }
        }
        1.0
    }

    /// Adds the page, its widgets and the Options entry (module docs). `ov` is read for the description
    /// label's id table. False (nothing changed) when the Options page, its list or a model page is missing.
    pub fn install_port_page(&mut self, ov: &Overlay) -> bool {
        if self.pages.contains_key(&PAGE) { return true; }
        let ad = self.addrs.clone();
        let model = if ENTRIES.len() <= 2 { ad.port_model_few } else { ad.port_model_many };
        let (Some(model), true) = (self.pages.get(&model).cloned(), self.pages.contains_key(&ad.port_options)) else { return false };
        let Some(Data::List(opts)) = self.widgets.get(&ad.port_options_list).map(|w| &w.data) else { return false };
        let n = opts.items.len();
        let at = opts.items.iter().position(|it| it.action == 3 && it.arg == ad.port_quit).unwrap_or(n);
        // The title: the model's title label (flags 0xf, large, centred) with the port title.
        let Some(mut title) = self.widgets.get(&model.widgets[0]).cloned() else { return false };
        let Data::Label(l) = &mut title.data else { return false };
        l.id = text::TITLE as u32;
        l.flags = (l.flags & !8) | 0x10;
        title.addr = TITLE_W;
        let settings = Widget {
            addr: LIST_W,
            update: UPDATE,
            draw: DRAW,
            enter: 0,
            leave: 0,
            dflags: 0,
            moby: 0,
            rect: [0; 4],
            raw: [0; 8],
            data: Data::Port(PortList { cursor: 0, top: 0, values: vec![0; ENTRIES.len()], choices: vec![u32::MAX; ENTRIES.len()], confirm_seconds: None, confirm_action: None, custom_height: 900 }),
        };
        let mut widgets = model.widgets;
        widgets[0] = TITLE_W;
        widgets[3] = LIST_W;
        let page = Page { addr: PAGE, seqs: model.seqs, parent: ad.port_options, kind: KIND, focus: LIST_W, widgets, pending: 0 };
        // The Options description label: the disc table (one id per item, variant 0) with ours inserted.
        if let Some(Data::Label(l)) = self.widgets.get_mut(&ad.port_options_label).map(|w| &mut w.data) {
            let mut t: Vec<u32> = (0..n as u32).map(|k| ov.u32(l.id.wrapping_add(k.wrapping_mul(l.stride) & !3)).unwrap_or(0)).collect();
            t.insert(at, text::DESCRIPTION as u32);
            l.table = Some(t);
        }
        if let Some(Data::List(opts)) = self.widgets.get_mut(&ad.port_options_list).map(|w| &mut w.data) {
            opts.items.insert(at, Item { label: text::ENTRY as i16, action: 3, arg: PAGE, sublabel: 0, hl: 0 });
        }
        self.widgets.insert(TITLE_W, title);
        self.widgets.insert(LIST_W, settings);
        self.pages.insert(PAGE, page);
        // The front end has its own Options frame and list. Clone its Language sub-page so the
        // frame animation returns to the front Options layout when △ is pressed.
        let front_parent = self.pages.values().find(|p| p.widgets.iter().any(|w|
            self.widgets.get(w).is_some_and(|x| x.enter == super::saves::func::FRONT_OPTIONS_ENTER)
        )).map(|p| p.addr);
        if let Some(front_parent) = front_parent {
            let front_model = self.saves.front_options[0].iter().find(|it| it.action == 3)
                .and_then(|it| self.pages.get(&it.arg)).cloned();
            if let Some(front_model) = front_model {
                if let Some(mut front_title) = self.widgets.get(&front_model.widgets[2]).cloned() {
                    if let Data::Label(l) = &mut front_title.data {
                        l.id = text::TITLE as u32;
                        l.flags = (l.flags & !8) | 0x10;
                        front_title.addr = TITLE_FRONT_W;
                        let mut widgets = front_model.widgets;
                        widgets[2] = TITLE_FRONT_W;
                        widgets[3] = LIST_W;
                        self.pages.insert(PAGE_FRONT, Page { addr: PAGE_FRONT, seqs: front_model.seqs, parent: front_parent, kind: KIND, focus: LIST_W, widgets, pending: 0 });
                        self.widgets.insert(TITLE_FRONT_W, front_title);
                        for items in &mut self.saves.front_options {
                            items.push(Item { label: text::ENTRY as i16, action: 3, arg: PAGE_FRONT, sublabel: 0, hl: 0 });
                        }
                    }
                }
            }
        }
        true
    }

    /// The value index of a port setting (None without the page).
    pub fn port_value(&self, s: Setting) -> Option<u8> { list(self)?.values.get(row_of(s)?).copied() }

    /// Sets a port setting's value index (from the engine's current value), clamped to the row's values.
    pub fn set_port_value(&mut self, s: Setting, v: u8) {
        let Some(r) = row_of(s) else { return };
        let max = ENTRIES[r].values.len().saturating_sub(1) as u8;
        if let Some(slot) = list_mut(self, LIST_W).and_then(|p| p.values.get_mut(r)) { *slot = v.min(max); }
    }

    pub fn port_custom_height(&self) -> Option<u16> { Some(list(self)?.custom_height) }
    pub fn set_port_custom_height(&mut self, height: u16) {
        if let Some(p) = list_mut(self, LIST_W) { p.custom_height = height.clamp(240, 2160); }
    }

    /// Restricts a port setting to some of its values (bit k = value index k; ✕ skips the others). Value 0 stays
    /// selectable whatever the mask, so a row always has a choice.
    pub fn set_port_choices(&mut self, s: Setting, mask: u32) {
        let Some(r) = row_of(s) else { return };
        if let Some(p) = list_mut(self, LIST_W) {
            if p.choices.len() < ENTRIES.len() { p.choices.resize(ENTRIES.len(), u32::MAX); }
            p.choices[r] = mask | 1;
        }
    }

    pub fn set_port_confirmation(&mut self, seconds: Option<u8>) {
        if let Some(p) = list_mut(self, LIST_W) { p.confirm_seconds = seconds; }
    }

    /// Move the focused Advanced Options list by mouse-wheel rows. Pad navigation uses the same cursor helper.
    pub fn scroll_port(&mut self, rows: i32) -> bool {
        if !matches!(self.current, PAGE | PAGE_FRONT) || rows == 0 { return false; }
        let Some(p) = list_mut(self, LIST_W) else { return false };
        if p.confirm_seconds.is_some() { return false; }
        p.move_cursor(rows)
    }

    /// true = keep the new display; false = revert.
    pub fn take_port_confirmation(&mut self) -> Option<bool> { list_mut(self, LIST_W)?.confirm_action.take() }
}

/// The list update, as the camera list 0x294cc0.
pub fn update(m: &mut PageMenu, w: u32, inp: &MenuInput, out: &mut MenuOut) -> i32 {
    if !m.is_focus(w) { return 0; }
    if let Some(p) = list_mut(m, w) {
        if p.confirm_seconds.is_some() {
            if inp.pressed_u & button::CROSS != 0 { p.confirm_action = Some(true); out.sounds.push(MenuSound::Confirm); }
            if inp.pressed_u & (button::TRIANGLE | button::CIRCLE) != 0 { p.confirm_action = Some(false); out.sounds.push(MenuSound::Confirm); }
            return 0;
        }
    }
    if let Some(r) = keys(m, inp, 1) { return r; }
    let Some(p) = list_mut(m, w) else { return 0 };
    let start = p.cursor;
    if inp.pressed_u & button::UP != 0 { p.move_cursor(-1); }
    if inp.pressed_u & button::DOWN != 0 { p.move_cursor(1); }
    if p.cursor != start { out.sounds.push(MenuSound::Cursor); }
    let r = p.cursor as usize;
    if ENTRIES.get(r).is_some_and(|e| e.setting == Setting::CustomHeight) {
        let before = p.custom_height;
        let down = if inp.pressed_u & button::LEFT != 0 { 16 } else if inp.pressed_u & button::SQUARE != 0 { 1 } else { 0 };
        let up = if inp.pressed_u & button::RIGHT != 0 { 16 } else if inp.pressed_u & button::CROSS != 0 { 1 } else { 0 };
        p.custom_height = p.custom_height.saturating_sub(down).saturating_add(up).clamp(240, 2160);
        if p.custom_height != before { out.sounds.push(MenuSound::Confirm); }
        return 0;
    }
    if inp.pressed_u & button::CROSS != 0 {
        let choices = p.choices.get(r).copied().unwrap_or(u32::MAX);
        if ENTRIES.get(r).is_some_and(|e| e.setting == Setting::Dlss5Preset) && choices == 0b10_0001 {
            return 0;
        }
        if let (Some(e), Some(v)) = (ENTRIES.get(r), p.values.get_mut(r)) {
            *v = next_choice(*v, e.values.len(), choices);
            out.sounds.push(MenuSound::Confirm);
        }
    }
    0
}

/// The list draw, as the camera list 0x294e68 (panel-local).
pub fn draw(m: &mut PageMenu, w: u32, a: &MenuAssets, out: &mut Vec<MenuDraw>) -> u32 {
    let Some(wd) = m.widgets.get(&w) else { return 1 };
    let [_, _, ww, wh] = wd.rect;
    let Data::Port(p) = &wd.data else { return 1 };
    // The port widens the middle game frame; three native-style rows now fit side by side.
    let shown = ENTRIES.len().min(VISIBLE);
    let step = (wh - 24).max(0) / (shown as i32 + 1);
    let mut y = step - 8;
    let top = (p.top.max(0) as usize).min(ENTRIES.len() - shown);
    for (i, e) in ENTRIES.iter().enumerate().skip(top).take(shown) {
        let col = if i as i32 == p.cursor { YELLOW } else { LIGHT_BLUE };
        let label = a.msg(e.label);
        let v = p.values.get(i).copied().unwrap_or(0) as usize;
        let custom = format!("{}p", p.custom_height);
        let value = if e.setting == Setting::CustomHeight { custom.as_bytes() } else { a.msg(e.values.get(v).copied().unwrap_or(0)) };
        let font = if a.width(Font::Regular, label) + a.width(Font::Regular, value) + 32 <= ww { Font::Regular } else { Font::Small };
        let max_label = ww - 32 - a.width(font, value);
        let mut end = label.len();
        while end > 0 && a.width(font, &label[..end]) > max_label { end -= 1; }
        print(out, font, 0xc, y, col, &label[..end]);
        print(out, font, ww - 0xc - a.width(font, value), y, LIGHT_BLUE, value);
        y += step;
    }
    if let Some(seconds) = p.confirm_seconds {
        let labels = match a.lang {
            2 => ("Garder", "Annuler"), 3 => ("Behalten", "Zurueck"),
            4 => ("Guardar", "Revertir"), 5 => ("Conferma", "Annulla"),
            _ => ("Keep", "Revert"),
        };
        let hint = format!("X {}  Triangle {}  {seconds}s", labels.0, labels.1);
        print(out, Font::Small, 0xc, wh - 0x10, YELLOW, hint.as_bytes());
    } else if let Some(entry) = ENTRIES.get(p.cursor as usize) {
        let help = description(entry.setting, a.lang);
        let mut end = help.len();
        while end > 0 && a.width(Font::Small, &help[..end]) > ww - 24 { end -= 1; }
        print(out, Font::Small, 0xc, wh - 0x10, LIGHT_BLUE, &help[..end]);
    }
    2
}

#[cfg(test)]
mod tests {
    use super::{language_from_index, language_index, next_choice, PortList, ENTRIES, LANGUAGES, VISIBLE};

    #[test]
    fn wheel_cursor_scrolls_and_clamps() {
        let mut p = PortList::default();
        assert!(p.move_cursor(5));
        assert_eq!((p.cursor, p.top), (5, 6 - VISIBLE as i32));
        assert!(p.move_cursor(100));
        assert_eq!(p.cursor, ENTRIES.len() as i32 - 1);
        assert!(!p.move_cursor(1));
        assert!(p.move_cursor(-100));
        assert_eq!((p.cursor, p.top), (0, 0));
    }

    #[test]
    fn cycle_skips_unsupported_values() {
        // Off / 2x / 4x / 8x with 8x unsupported: 4x → Off.
        assert_eq!(next_choice(0, 4, 0b0111), 1);
        assert_eq!(next_choice(2, 4, 0b0111), 0);
        // WebGPU-only (off / 4x): off → 4x → off.
        assert_eq!(next_choice(0, 4, 0b0101), 2);
        assert_eq!(next_choice(2, 4, 0b0101), 0);
        // All allowed: plain wrap; nothing else allowed: stays.
        assert_eq!(next_choice(3, 4, u32::MAX), 0);
        assert_eq!(next_choice(0, 4, 0b0001), 0);
    }

    #[test]
    fn language_values_match_disc_slots() {
        for (index, language) in LANGUAGES.into_iter().enumerate() {
            assert_eq!(language_index(language), index as u8);
            assert_eq!(language_from_index(index as u8), language);
        }
        assert_eq!(language_from_index(99), 0);
    }

    #[test]
    fn port_text_follows_active_language() {
        assert_eq!(super::text::get_localized(super::text::TITLE, 4), Some(&b"Opciones avanzadas"[..]));
        assert_eq!(super::text::get_localized(super::text::TITLE, 2), Some(&b"Options avancees"[..]));
        assert_eq!(super::text::get_localized(super::text::TITLE, 0), Some(&b"Advanced Options"[..]));
        assert_eq!(super::text::get_localized(super::text::SPANISH, 4), Some(&b"Espa\xa9ol"[..]));
        assert_eq!(super::text::get_localized(super::text::SUBTITLE_SIZE, 4), Some(&b"Tama\xa9o subtitulos"[..]));
    }
}
