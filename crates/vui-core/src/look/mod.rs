//! One look for every app: `look.toml` in the user's `vlook` folder sets the theme, its tokens, the
//! glass material, the notch and named extension colours, with small per-app overrides. Every app
//! loads and watches the same file, so a change made anywhere repaints everything.
//!
//! ```toml
//! theme = "porcelain"        # porcelain | graphite | vmac-classic | a file in themes/
//! mode = "system"            # system | light | dark
//! dark_theme = "graphite"    # the dark half when mode = "system"
//! [base]
//! radii = { control = 10, card = 16, panel = 20 }
//! [light]
//! accent = "#7DB4E6"
//! [glass.dark]
//! opacity = 0.62
//! [ext.light]
//! up = "#1E9E6A"
//! [apps.vtrade]
//! accent = "#2E6EA8"
//! ```
//!
//! Anything not named keeps the theme's value. Names that mean nothing are reported, not guessed at.
//! Colours are `#RRGGBB`, or `#RRGGBBAA` where a colour carries its own alpha. vmac's older
//! `look.json` colour names (`bg`, `side`, `line`, …) are read here too.

pub mod toml;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::color::Color;
use crate::theme::{Glass, Mode, Theme};
use toml::{Table, Value};

/// The built-in theme names.
pub const THEMES: [&str; 3] = ["vmac-classic", "porcelain", "graphite"];

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ModeChoice {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
    Dense,
}

/// What draws behind the notch's glass.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Backdrop {
    /// The system's glass (macOS 26 and later).
    #[default]
    Native,
    /// The screen under the notch, captured while it is open: full liquid glass, opt-in.
    Captured,
    Blur,
    Solid,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum NotchStyle {
    /// A black core with a liquid-glass skirt.
    #[default]
    Glass,
    /// All black, as the hardware.
    Hardware,
    Tinted,
}

/// The notch's settings.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NotchLook {
    pub style: NotchStyle,
    pub backdrop: Backdrop,
    /// Points over which the hardware black fades into glass.
    pub core_width: f32,
    pub wing_min: f32,
    pub wing_max: f32,
    /// Seconds after the pointer leaves.
    pub collapse_after: f32,
    pub peek_on_hover: bool,
    pub pour_animation: bool,
    pub pulse_alerts: bool,
}

impl Default for NotchLook {
    fn default() -> NotchLook {
        NotchLook {
            style: NotchStyle::Glass,
            backdrop: Backdrop::Native,
            core_width: 10.0,
            wing_min: 64.0,
            wing_max: 120.0,
            collapse_after: 4.0,
            peek_on_hover: true,
            pour_animation: true,
            pulse_alerts: true,
        }
    }
}

/// A look file as read: the document, with the named theme's file (if any) beneath it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Look {
    doc: Table,
}

impl Look {
    /// The look in `text`, and what in it could not be used.
    pub fn parse(text: &str) -> Result<(Look, Vec<String>), String> {
        let doc = toml::parse(text).map_err(|e| format!("look.toml: {e}"))?;
        let look = Look { doc };
        Ok((look.clone(), look.problems()))
    }

    /// The look in folder `dir` (`look.toml`, and `themes/<name>.toml` when the theme isn't built
    /// in). An empty look when there is no file.
    pub fn load_from(dir: &Path) -> Result<(Look, Vec<String>), String> {
        let text = match std::fs::read_to_string(dir.join("look.toml")) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Look::default(), Vec::new())),
            Err(e) => return Err(format!("look.toml: {e}")),
        };
        let (mut look, mut problems) = Look::parse(&text)?;
        for name in look.names() {
            if THEMES.contains(&name.as_str()) || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                continue;
            }
            let file = dir.join("themes").join(format!("{name}.toml"));
            match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| toml::parse(&t)) {
                Ok(saved) => look.doc = under(saved, std::mem::take(&mut look.doc)),
                Err(e) => problems.push(format!("themes/{name}.toml: {e}")),
            }
        }
        problems.extend(look.problems());
        problems.sort();
        problems.dedup();
        Ok((look, problems))
    }

    /// The look in the user's folder, with its problems logged; an empty look if it can't be read.
    pub fn load() -> Look {
        let Some(dir) = dir() else { return Look::default() };
        match Look::load_from(&dir) {
            Ok((look, problems)) => {
                for p in problems {
                    eprintln!("vlook: {p}");
                }
                look
            }
            Err(e) => {
                eprintln!("vlook: {e}");
                Look::default()
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.doc.is_empty()
    }

    fn str(&self, key: &str) -> Option<&str> {
        self.doc.get(key).and_then(Value::as_str)
    }

    fn table(&self, path: &[&str]) -> Option<&Table> {
        let mut t = &self.doc;
        for key in path {
            t = t.get(*key)?.as_table()?;
        }
        Some(t)
    }

    /// The theme names this look uses.
    fn names(&self) -> Vec<String> {
        [self.str("theme"), self.str("dark_theme")].into_iter().flatten().map(str::to_string).collect()
    }

    pub fn mode(&self) -> ModeChoice {
        match self.str("mode") {
            Some("light") => ModeChoice::Light,
            Some("dark") => ModeChoice::Dark,
            _ => ModeChoice::System,
        }
    }

    /// The mode drawn when the system is in `system`.
    pub fn resolve_mode(&self, system: Mode) -> Mode {
        match self.mode() {
            ModeChoice::System => system,
            ModeChoice::Light => Mode::Light,
            ModeChoice::Dark => Mode::Dark,
        }
    }

    /// The theme named for `mode`, if the look names one.
    pub fn theme_name(&self, mode: Mode) -> Option<&str> {
        let theme = self.str("theme");
        match mode {
            Mode::Dark if self.mode() == ModeChoice::System => self.str("dark_theme").or(theme),
            _ => theme,
        }
    }

    /// `app`'s theme when the system is in `system`. `own` is the app's theme for a mode, used when
    /// the look names no theme (or names `vmac-classic`, which each app draws its own way).
    pub fn theme(&self, app: &str, system: Mode, own: impl Fn(Mode) -> Theme) -> Theme {
        let mode = self.resolve_mode(system);
        let mut theme = self.theme_name(mode).and_then(|n| named(n, mode)).unwrap_or_else(|| own(mode));
        self.apply(&mut theme, app);
        theme
    }

    /// This look's tokens over `theme`, as `app` draws them.
    pub fn apply(&self, theme: &mut Theme, app: &str) {
        self.apply_reporting(theme, app, &mut Vec::new());
    }

    pub fn notch(&self) -> NotchLook {
        let mut n = NotchLook::default();
        self.notch_reporting(&mut n, &mut Vec::new());
        n
    }

    pub fn density(&self, app: &str) -> Density {
        let named = self.table(&["apps", app]).and_then(|t| t.get("density")).or_else(|| self.table(&["base"]).and_then(|t| t.get("density")));
        match named.and_then(Value::as_str) {
            Some("compact") => Density::Compact,
            Some("dense") => Density::Dense,
            _ => Density::Comfortable,
        }
    }

    /// Everything that can't be used, for every app and mode.
    fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        for (key, value) in &self.doc {
            match key.as_str() {
                "schema_version" => {
                    if value.as_f64() != Some(1.0) {
                        p.push("schema_version: only 1 is known".into());
                    }
                }
                "theme" | "dark_theme" => {
                    if value.as_str().is_none() {
                        p.push(format!("{key} should be a theme's name"));
                    }
                }
                "mode" => {
                    if !matches!(value.as_str(), Some("system" | "light" | "dark")) {
                        p.push("mode should be system, light or dark".into());
                    }
                }
                "base" | "light" | "dark" | "glass" | "ext" | "notch" | "apps" => {
                    if value.as_table().is_none() {
                        p.push(format!("{key} should be a table"));
                    }
                }
                _ => p.push(format!("{key} is not a setting")),
            }
        }
        for mode in [Mode::Light, Mode::Dark] {
            let mut theme = Theme::for_mode(mode);
            let apps: Vec<String> = self.table(&["apps"]).map(|t| t.keys().cloned().collect()).unwrap_or_default();
            self.apply_reporting(&mut theme, "", &mut p);
            for app in apps {
                self.apply_reporting(&mut theme, &app, &mut p);
            }
        }
        self.notch_reporting(&mut NotchLook::default(), &mut p);
        p.sort();
        p.dedup();
        p
    }

    fn apply_reporting(&self, theme: &mut Theme, app: &str, p: &mut Vec<String>) {
        let half = if theme.mode == Mode::Dark { "dark" } else { "light" };
        if let Some(base) = self.table(&["base"]) {
            for (key, v) in base {
                match key.as_str() {
                    "radii" => radii(theme, v, p),
                    "control" => number(v, "base.control", 16.0..=128.0, p, |n| theme.control = n),
                    "control_large" => number(v, "base.control_large", 16.0..=128.0, p, |n| theme.control_large = n),
                    "motion_fast" => number(v, "base.motion_fast", 0.0..=5.0, p, |n| theme.motion.fast = n),
                    "text" => text(theme, v, p),
                    // Read by apps that use them; vui's theme has no slot for these.
                    "font_ui" | "font_mono" | "spacing" | "density" => {}
                    _ => p.push(format!("base.{key} is not a token")),
                }
            }
        }
        if let Some(colours) = self.table(&[half]) {
            for (name, v) in colours {
                colour_token(theme, name, v, half, p);
            }
        }
        if let Some(glass) = self.table(&["glass", half]) {
            for (name, v) in glass {
                glass_token(&mut theme.glass, name, v, p);
            }
        }
        if let Some(ext) = self.table(&["ext", half]) {
            for (name, v) in ext {
                match v.as_str().and_then(colour) {
                    Some(c) => theme.ext.set(name, c),
                    None => p.push(format!("ext.{half}.{name} should be a colour like #1E9E6A")),
                }
            }
        }
        if !app.is_empty()
            && let Some(own) = self.table(&["apps", app])
        {
            for (name, v) in own {
                match name.as_str() {
                    "accent" | "on_accent" => colour_token(theme, name, v, &format!("apps.{app}"), p),
                    "density" => {}
                    _ => p.push(format!("apps.{app}.{name}: only accent and density can differ per app")),
                }
            }
        }
    }

    fn notch_reporting(&self, n: &mut NotchLook, p: &mut Vec<String>) {
        let Some(t) = self.table(&["notch"]) else { return };
        for (key, v) in t {
            let name = format!("notch.{key}");
            match key.as_str() {
                "style" => match v.as_str() {
                    Some("glass") => n.style = NotchStyle::Glass,
                    Some("hardware") => n.style = NotchStyle::Hardware,
                    Some("tinted") => n.style = NotchStyle::Tinted,
                    _ => p.push(format!("{name} should be glass, hardware or tinted")),
                },
                "backdrop" => match v.as_str() {
                    Some("native") => n.backdrop = Backdrop::Native,
                    Some("captured") => n.backdrop = Backdrop::Captured,
                    Some("blur") => n.backdrop = Backdrop::Blur,
                    Some("solid") => n.backdrop = Backdrop::Solid,
                    _ => p.push(format!("{name} should be native, captured, blur or solid")),
                },
                "core_width" => number(v, &name, 0.0..=40.0, p, |x| n.core_width = x),
                "wing_min" => number(v, &name, 24.0..=240.0, p, |x| n.wing_min = x),
                "wing_max" => number(v, &name, 24.0..=320.0, p, |x| n.wing_max = x),
                "collapse_after" => number(v, &name, 0.0..=60.0, p, |x| n.collapse_after = x),
                "peek_on_hover" | "pour_animation" | "pulse_alerts" => match v.as_bool() {
                    Some(b) => match key.as_str() {
                        "peek_on_hover" => n.peek_on_hover = b,
                        "pour_animation" => n.pour_animation = b,
                        _ => n.pulse_alerts = b,
                    },
                    None => p.push(format!("{name} should be true or false")),
                },
                _ => p.push(format!("{name} is not a notch setting")),
            }
        }
        if n.wing_max < n.wing_min {
            p.push("notch.wing_max is below notch.wing_min".into());
            n.wing_max = n.wing_min;
        }
    }
}

/// `top` laid over `bottom`, table by table.
fn under(bottom: Table, top: Table) -> Table {
    let mut out = bottom;
    for (k, v) in top {
        let merged = match (out.remove(&k), v) {
            (Some(Value::Table(b)), Value::Table(t)) => Value::Table(under(b, t)),
            (_, v) => v,
        };
        out.insert(k, merged);
    }
    out
}

/// A built-in theme in `mode`. Porcelain is light and Graphite dark; asked for the other mode,
/// each gives its partner.
pub fn named(name: &str, mode: Mode) -> Option<Theme> {
    match (name, mode) {
        ("vmac-classic", m) => Some(Theme::for_mode(m)),
        ("porcelain", Mode::Light) | ("graphite", Mode::Light) => Some(porcelain()),
        ("porcelain", Mode::Dark) | ("graphite", Mode::Dark) => Some(graphite()),
        _ => None,
    }
}

/// White canvas, beige surfaces, light blue accents.
pub fn porcelain() -> Theme {
    let mut t = Theme::light();
    let p = &mut t.palette;
    p.bg = Color::hex(0xFFFFFF);
    p.side = Color::hex(0xF4EEE3);
    p.line = Color::hex(0xE6DCCB);
    p.wash = Color::hex(0x7DB4E6).with_alpha(0.18);
    p.hover = Color::hex(0x1D1B17).with_alpha(0.04);
    p.text = Color::hex(0x1D1B17);
    p.text_2 = Color::hex(0x5C574E);
    p.text_3 = Color::hex(0x9A9387);
    p.accent = Color::hex(0x7DB4E6);
    // The light blue is too faint for white text, so text on it is dark.
    p.on_accent = Color::hex(0x10273D);
    p.success = Color::hex(0x1E9E6A);
    p.danger = Color::hex(0xD9534F);
    p.blue = Color::hex(0x2E6EA8);
    p.purple = Color::hex(0x8C7CF0);
    p.field = Color::hex(0xEFE7D9);
    p.track_off = Color::hex(0xE6DCCB);
    t.ext.set("surface", Color::hex(0xFAF7F1));
    // Frosted glass on floating panels, palette, menus and the ticket (plan §15).
    t.glass.floating = true;
    t
}

/// Dark blue-grey surfaces with the same light blue.
pub fn graphite() -> Theme {
    let mut t = Theme::dark();
    let p = &mut t.palette;
    p.bg = Color::hex(0x14171D);
    p.side = Color::hex(0x0D0F13);
    p.line = Color::hex(0x272C36);
    p.wash = Color::hex(0x86BDF2).with_alpha(0.14);
    p.hover = Color::WHITE.with_alpha(0.04);
    p.text = Color::hex(0xE9EBEF);
    p.text_2 = Color::hex(0xA5ACB8);
    p.text_3 = Color::hex(0x687180);
    p.accent = Color::hex(0x86BDF2);
    p.on_accent = Color::hex(0x0D1A27);
    p.success = Color::hex(0x2BC48A);
    p.danger = Color::hex(0xF2686A);
    p.blue = Color::hex(0xA9D1F7);
    p.purple = Color::hex(0xA99BFF);
    p.field = Color::hex(0x0F1217);
    p.track_off = Color::hex(0x272C36);
    t.ext.set("surface", Color::hex(0x14171D));
    t.glass.floating = true;
    t
}

/// A colour written `#RRGGBB` or `#RRGGBBAA`.
pub fn colour(text: &str) -> Option<Color> {
    let hex = text.strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        6 => Some(Color::hex(n)),
        8 => Some(Color::hex(n >> 8).with_alpha((n & 0xff) as f32 / 255.0)),
        _ => None,
    }
}

fn number(v: &Value, name: &str, range: std::ops::RangeInclusive<f32>, p: &mut Vec<String>, set: impl FnOnce(f32)) {
    match v.as_f64().map(|n| n as f32) {
        Some(n) if n.is_finite() && range.contains(&n) => set(n),
        Some(_) => p.push(format!("{name} must be between {} and {}", range.start(), range.end())),
        None => p.push(format!("{name} should be a number")),
    }
}

/// A colour by the shared name (`canvas`, `surface_raised`, …) or vmac's older one (`bg`, `side`, …).
fn colour_token(theme: &mut Theme, name: &str, v: &Value, table: &str, p: &mut Vec<String>) {
    let Some(c) = v.as_str().and_then(colour) else {
        return p.push(format!("{table}.{name} should be a colour like #4F5BD5"));
    };
    let pal = &mut theme.palette;
    let slot = match name {
        "canvas" | "bg" => &mut pal.bg,
        "surface_raised" | "side" => &mut pal.side,
        "border" | "line" => &mut pal.line,
        "accent_soft" | "wash" => &mut pal.wash,
        "hover" => &mut pal.hover,
        "text" => &mut pal.text,
        "text_mid" | "text_2" => &mut pal.text_2,
        "text_soft" | "text_3" => &mut pal.text_3,
        "accent" => &mut pal.accent,
        "on_accent" => &mut pal.on_accent,
        "accent_ink" => &mut pal.blue,
        "danger" => &mut pal.danger,
        "on_danger" => &mut pal.on_danger,
        "surface_sunken" | "field" => &mut pal.field,
        "track_off" => &mut pal.track_off,
        "knob" => &mut pal.knob,
        "ring" => &mut pal.ring,
        "scrim" => &mut pal.scrim,
        // vui draws panels on `bg`; the plain surface is kept for apps that want it.
        "surface" => return theme.ext.set("surface", c),
        _ => return p.push(format!("{table}.{name} is not a colour token")),
    };
    *slot = c;
}

fn glass_token(g: &mut Glass, name: &str, v: &Value, p: &mut Vec<String>) {
    let full = format!("glass.{name}");
    match name {
        "tint" => match v.as_str().and_then(colour) {
            Some(c) => g.tint = c,
            None => p.push(format!("{full} should be a colour")),
        },
        "opacity" => number(v, &full, 0.0..=1.0, p, |n| g.opacity = n),
        "blur" => number(v, &full, 0.0..=100.0, p, |n| g.blur = n),
        "saturation" => number(v, &full, 0.0..=3.0, p, |n| g.saturation = n),
        "refraction" => number(v, &full, 0.0..=40.0, p, |n| g.refraction = n),
        "dispersion" => number(v, &full, 0.0..=1.0, p, |n| g.dispersion = n),
        "rim" => number(v, &full, 0.0..=1.0, p, |n| g.rim = n),
        "specular" => number(v, &full, 0.0..=1.0, p, |n| g.specular = n),
        "grain" => number(v, &full, 0.0..=0.2, p, |n| g.grain = n),
        "floating" => match v.as_bool() {
            Some(b) => g.floating = b,
            None => p.push(format!("{full} should be true or false")),
        },
        _ => p.push(format!("{full} is not a glass setting")),
    }
}

fn radii(theme: &mut Theme, v: &Value, p: &mut Vec<String>) {
    let Some(map) = v.as_table() else { return p.push("base.radii should be a table".into()) };
    for (name, v) in map {
        let r = &mut theme.radii;
        let (slot, range) = match name.as_str() {
            "control" => (&mut r.control, 0.0..=128.0),
            "card" => (&mut r.card, 0.0..=128.0),
            "panel" => (&mut r.panel, 0.0..=128.0),
            "smooth" => (&mut r.smooth, 0.0..=1.0),
            "notch_card" => continue, // the notch's own
            _ => {
                p.push(format!("base.radii.{name} is not a radius"));
                continue;
            }
        };
        number(v, &format!("base.radii.{name}"), range, p, |n| *slot = n);
    }
}

/// Text sizes by role; the line height keeps its proportion to the size.
fn text(theme: &mut Theme, v: &Value, p: &mut Vec<String>) {
    let Some(map) = v.as_table() else { return p.push("base.text should be a table of sizes".into()) };
    for (name, v) in map {
        let t = &mut theme.types;
        let token = match name.as_str() {
            "small" => &mut t.small,
            "body" => &mut t.body,
            "title" => &mut t.title,
            "mono" => &mut t.mono,
            _ => {
                p.push(format!("base.text.{name} is not a text role"));
                continue;
            }
        };
        number(v, &format!("base.text.{name}"), 6.0..=96.0, p, |n| {
            token.line_height *= n / token.size;
            token.size = n;
        });
    }
}

/// The user's look folder: `$VLOOK_DIR`, else `~/Library/Application Support/vlook`.
pub fn dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("VLOOK_DIR") {
        return Some(PathBuf::from(d));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/vlook"))
}

/// Notices changes to a look folder by polling: cheap enough to run every few hundred
/// milliseconds, and it needs nothing from the platform.
#[derive(Debug)]
pub struct Watch {
    dir: PathBuf,
    seen: Option<Vec<Stamp>>,
}

type Stamp = Option<(SystemTime, u64)>;

impl Watch {
    pub fn new(dir: impl Into<PathBuf>) -> Watch {
        Watch { dir: dir.into(), seen: None }
    }

    fn stamps(&self) -> Vec<Stamp> {
        let mut files = vec![self.dir.join("look.toml")];
        if let Ok(list) = std::fs::read_dir(self.dir.join("themes")) {
            let mut themes: Vec<PathBuf> = list.flatten().map(|e| e.path()).collect();
            themes.sort();
            files.extend(themes);
        }
        files.iter().map(|f| std::fs::metadata(f).ok().map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()))).collect()
    }

    /// The look again when a file changed since the last poll (the first poll always reads it).
    /// A look that doesn't parse is an error, and the app keeps the last good one.
    pub fn poll(&mut self) -> Option<Result<(Look, Vec<String>), String>> {
        let now = self.stamps();
        if self.seen.as_ref() == Some(&now) {
            return None;
        }
        self.seen = Some(now);
        Some(Look::load_from(&self.dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"
schema_version = 1
theme = "porcelain"
mode = "system"
dark_theme = "graphite"
[base]
radii = { control = 10, card = 16, panel = 20, notch_card = 22 }
control = 32
density = "comfortable"
font_ui = "Inter"
[light]
accent = "#7DB4E6"
accent_ink = "#2E6EA8"
surface = "#FAF7F1"
[dark]
accent_soft = "#86BDF224"
[glass.dark]
opacity = 0.7
floating = true
[notch]
backdrop = "blur"
wing_max = 100
[ext.light]
up = "#1E9E6A"
[apps.vtrade]
accent = "#2E6EA8"
[apps.vcoding]
density = "compact"
"##;

    #[test]
    fn a_look_sets_the_theme_its_tokens_glass_ext_and_per_app_accent() {
        let (look, problems) = Look::parse(SAMPLE).unwrap();
        assert!(problems.is_empty(), "{problems:?}");
        let light = look.theme("vmac", Mode::Light, Theme::for_mode);
        assert_eq!(light.palette.side, porcelain().palette.side, "porcelain is the light half");
        assert_eq!(light.radii.control, 10.0);
        assert_eq!(light.palette.accent, colour("#7DB4E6").unwrap());
        assert_eq!(light.ext.get("up"), colour("#1E9E6A"));
        assert_eq!(light.ext.get("surface"), colour("#FAF7F1"));
        let dark = look.theme("vmac", Mode::Dark, Theme::for_mode);
        assert_eq!(dark.palette.bg, graphite().palette.bg, "graphite is the dark half");
        assert_eq!(dark.glass.opacity, 0.7);
        assert!(dark.glass.floating && light.glass.floating, "porcelain and graphite float on glass");
        assert!(!Theme::light().glass.floating, "the plain theme stays solid");
        assert_eq!(dark.palette.wash.a, 0x24 as f32 / 255.0);
        assert_eq!(look.theme("vtrade", Mode::Light, Theme::for_mode).palette.accent, colour("#2E6EA8").unwrap(), "vtrade's own accent");
        assert_eq!(look.density("vcoding"), Density::Compact);
        assert_eq!(look.density("vmac"), Density::Comfortable);
        let notch = look.notch();
        assert_eq!((notch.backdrop, notch.wing_max), (Backdrop::Blur, 100.0));
    }

    #[test]
    fn no_file_means_each_app_draws_its_own_and_problems_are_named() {
        let own = |m| {
            let mut t = Theme::for_mode(m);
            t.palette.accent = Color::hex(0xF9913F);
            t
        };
        assert_eq!(Look::default().theme("vmac", Mode::Dark, own), own(Mode::Dark));
        let (_, problems) = Look::parse("colour = 1\n[light]\naccent = \"blue\"\nshine = \"#ffffff\"\n[apps.vmac]\nradius = 3\n[notch]\nwing_min = 2").unwrap();
        assert_eq!(
            problems,
            [
                "apps.vmac.radius: only accent and density can differ per app",
                "colour is not a setting",
                "light.accent should be a colour like #4F5BD5",
                "light.shine is not a colour token",
                "notch.wing_min must be between 24 and 240",
            ]
        );
        assert!(Look::parse("theme = ").is_err());
    }

    #[test]
    fn a_saved_theme_lies_under_the_file_and_the_watch_sees_saves() {
        let dir = std::env::temp_dir().join(format!("vlook-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("themes")).unwrap();
        let mut watch = Watch::new(&dir);
        let (look, _) = watch.poll().unwrap().unwrap();
        assert!(look.is_empty(), "no file yet");
        assert!(watch.poll().is_none(), "nothing changed");

        std::fs::write(dir.join("themes/sea.toml"), "[light]\naccent = \"#0A7EA4\"\ntext = \"#102030\"\n").unwrap();
        std::fs::write(dir.join("look.toml"), "theme = \"sea\"\n[light]\ntext = \"#000000\"\n").unwrap();
        let (look, problems) = watch.poll().unwrap().unwrap();
        assert!(problems.is_empty(), "{problems:?}");
        let t = look.theme("vtrade", Mode::Light, Theme::for_mode);
        assert_eq!(t.palette.accent, colour("#0A7EA4").unwrap(), "from the saved theme");
        assert_eq!(t.palette.text, colour("#000000").unwrap(), "the file wins");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
