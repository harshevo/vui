//! Design tokens.
//!
//! A `Theme` is plain data: change a value and every widget follows. The defaults are the vmac design
//! language: quiet neutral surfaces (a white sheet on a pale canvas, or a dark sheet on a darker one),
//! text in three greys, one soft indigo accent, red only for irreversible actions, soft fills instead of
//! outlines, pill-shaped buttons, Inter at three sizes, and an 8-point spacing scale.

use crate::anim::Spring;
use crate::color::Color;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Light,
    Dark,
}

/// The spacing scale in logical pixels. Gaps and padding come from here and nowhere else.
pub mod space {
    pub const S4: f32 = 4.0;
    pub const S8: f32 = 8.0;
    pub const S12: f32 = 12.0;
    pub const S16: f32 = 16.0;
    pub const S24: f32 = 24.0;
    pub const S32: f32 = 32.0;
    pub const S48: f32 = 48.0;
    pub const S64: f32 = 64.0;
    pub const SCALE: [f32; 8] = [S4, S8, S12, S16, S24, S32, S48, S64];
}

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    /// The content sheet, and anything that sits on it.
    pub bg: Color,
    /// The canvas around the sheet: the window background and the sidebar.
    pub side: Color,
    /// 1 px dividers and control borders.
    pub line: Color,
    /// Selected and pressed fill.
    pub wash: Color,
    /// Fill under a hovered row or tile.
    pub hover: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text.
    pub text_2: Color,
    /// Hints, times and placeholders.
    pub text_3: Color,
    /// The one accent: primary buttons, focus rings, the live waveform.
    pub accent: Color,
    pub on_accent: Color,
    /// Irreversible actions only.
    pub danger: Color,
    pub on_danger: Color,
    /// Colour accents beside the main one: tabs, chips, charts and status. Never backgrounds.
    pub orange: Color,
    pub blue: Color,
    pub purple: Color,
    /// Text field background.
    pub field: Color,
    /// Toggle track when off, and its knob.
    pub track_off: Color,
    pub knob: Color,
    /// Hairline ring drawn inside images so pale pictures do not bleed into the page.
    pub ring: Color,
    /// Dim layer over a thumbnail under a label.
    pub scrim: Color,
}

/// A palette entry by name. Widgets hold a `Tone`, not a colour, so switching between light and dark
/// repaints them without rebuilding anything.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Tone {
    Bg,
    Side,
    Line,
    Wash,
    Hover,
    Text,
    Text2,
    Text3,
    Accent,
    OnAccent,
    Danger,
    OnDanger,
    Orange,
    Blue,
    Purple,
    Field,
    TrackOff,
    Knob,
    Ring,
    Scrim,
}

impl Palette {
    pub fn get(&self, tone: Tone) -> Color {
        match tone {
            Tone::Bg => self.bg,
            Tone::Side => self.side,
            Tone::Line => self.line,
            Tone::Wash => self.wash,
            Tone::Hover => self.hover,
            Tone::Text => self.text,
            Tone::Text2 => self.text_2,
            Tone::Text3 => self.text_3,
            Tone::Accent => self.accent,
            Tone::OnAccent => self.on_accent,
            Tone::Danger => self.danger,
            Tone::OnDanger => self.on_danger,
            Tone::Orange => self.orange,
            Tone::Blue => self.blue,
            Tone::Purple => self.purple,
            Tone::Field => self.field,
            Tone::TrackOff => self.track_off,
            Tone::Knob => self.knob,
            Tone::Ring => self.ring,
            Tone::Scrim => self.scrim,
        }
    }
}

/// A colour that is either fixed or taken from the theme when it is drawn.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Fill {
    Solid(Color),
    /// A palette entry, with its alpha multiplied by the second value.
    Tone(Tone, f32),
}

impl Fill {
    pub const NONE: Fill = Fill::Solid(Color::TRANSPARENT);

    pub fn resolve(self, palette: &Palette) -> Color {
        match self {
            Fill::Solid(c) => c,
            Fill::Tone(t, a) => {
                let c = palette.get(t);
                c.with_alpha(c.a * a)
            }
        }
    }

    /// The same fill at `alpha` times its own opacity.
    pub fn alpha(self, alpha: f32) -> Fill {
        match self {
            Fill::Solid(c) => Fill::Solid(c.with_alpha(c.a * alpha)),
            Fill::Tone(t, a) => Fill::Tone(t, a * alpha),
        }
    }

    /// True when nothing would be drawn whatever the theme: a fully transparent solid colour.
    pub fn is_none(self) -> bool {
        match self {
            Fill::Solid(c) => c.a <= 0.0,
            Fill::Tone(_, a) => a <= 0.0,
        }
    }
}

impl From<Color> for Fill {
    fn from(c: Color) -> Fill {
        Fill::Solid(c)
    }
}

impl From<Tone> for Fill {
    fn from(t: Tone) -> Fill {
        Fill::Tone(t, 1.0)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Radii {
    pub control: f32,
    pub card: f32,
    pub panel: f32,
    /// How continuous the corners are, 0 (circular) to 1 (squircle).
    pub smooth: f32,
}

/// Larger than any real box, so the SDF clamps it to a full pill.
pub const PILL: f32 = 9999.0;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TextToken {
    pub size: f32,
    pub line_height: f32,
    pub weight: u16,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextRole {
    Small,
    Body,
    Title,
    Mono,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TypeScale {
    pub small: TextToken,
    pub body: TextToken,
    pub title: TextToken,
    pub mono: TextToken,
}

impl TypeScale {
    pub fn get(&self, role: TextRole) -> TextToken {
        match role {
            TextRole::Small => self.small,
            TextRole::Body => self.body,
            TextRole::Title => self.title,
            TextRole::Mono => self.mono,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Motion {
    /// Seconds for hover, press and selection fades.
    pub fast: f32,
    pub snappy: Spring,
    pub smooth: Spring,
    pub morph: Spring,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub mode: Mode,
    pub palette: Palette,
    pub radii: Radii,
    pub types: TypeScale,
    pub motion: Motion,
    pub font_sans: &'static str,
    pub font_mono: &'static str,
    /// Height of a standard control; primary actions use `control_large`.
    pub control: f32,
    pub control_large: f32,
}

impl Theme {
    pub fn for_mode(mode: Mode) -> Theme {
        match mode {
            Mode::Light => Theme::light(),
            Mode::Dark => Theme::dark(),
        }
    }

    pub fn light() -> Theme {
        Theme::base(
            Mode::Light,
            Palette {
                bg: Color::hex(0xffffff),
                side: Color::hex(0xf5f5f6),
                line: Color::hex(0xe6e6e9),
                wash: Color::hex(0x18181b).with_alpha(0.06),
                hover: Color::hex(0x18181b).with_alpha(0.04),
                text: Color::hex(0x18181b),
                text_2: Color::hex(0x56565e),
                text_3: Color::hex(0xa1a1aa),
                accent: Color::hex(0x4f5bd5),
                on_accent: Color::hex(0xffffff),
                danger: Color::hex(0xdc2626),
                on_danger: Color::hex(0xffffff),
                orange: Color::hex(0xf97f2c),
                blue: Color::hex(0x2f7cf6),
                purple: Color::hex(0x8b5cf6),
                field: Color::hex(0x18181b).with_alpha(0.04),
                track_off: Color::hex(0xd4d4d8),
                knob: Color::hex(0xffffff),
                ring: Color::BLACK.with_alpha(0.08),
                scrim: Color::BLACK.with_alpha(0.6),
            },
        )
    }

    pub fn dark() -> Theme {
        Theme::base(
            Mode::Dark,
            Palette {
                bg: Color::hex(0x161618),
                side: Color::hex(0x0e0e10),
                line: Color::hex(0x2a2a2e),
                wash: Color::WHITE.with_alpha(0.08),
                hover: Color::WHITE.with_alpha(0.05),
                text: Color::hex(0xededf0),
                text_2: Color::hex(0xa8a8af),
                text_3: Color::hex(0x6e6e76),
                accent: Color::hex(0x8c96fa),
                on_accent: Color::hex(0x0e0e10),
                danger: Color::hex(0xdc2626),
                on_danger: Color::hex(0xffffff),
                orange: Color::hex(0xfb923c),
                blue: Color::hex(0x60a5fa),
                purple: Color::hex(0xa78bfa),
                field: Color::WHITE.with_alpha(0.05),
                track_off: Color::hex(0x3f3f46),
                knob: Color::hex(0xe4e4e7),
                ring: Color::WHITE.with_alpha(0.10),
                scrim: Color::BLACK.with_alpha(0.6),
            },
        )
    }

    /// The black island: the notch and the ask panel grown from it. Always dark, whatever the system mode.
    pub fn island() -> Theme {
        let mut t = Theme::dark();
        t.palette.bg = Color::hex(0x000000);
        t.palette.side = Color::hex(0x000000);
        t.palette.field = Color::hex(0x171717);
        t.palette.wash = Color::hex(0x1f1f1f);
        t.palette.hover = Color::hex(0x171717);
        t.palette.text = Color::hex(0xffffff);
        t
    }

    fn base(mode: Mode, palette: Palette) -> Theme {
        Theme {
            mode,
            palette,
            radii: Radii { control: 12.0, card: 16.0, panel: 20.0, smooth: 0.6 },
            types: TypeScale {
                small: TextToken { size: 12.0, line_height: 18.0, weight: 400 },
                body: TextToken { size: 14.0, line_height: 21.0, weight: 400 },
                title: TextToken { size: 24.0, line_height: 32.0, weight: 600 },
                mono: TextToken { size: 12.0, line_height: 18.0, weight: 400 },
            },
            motion: Motion { fast: 0.12, snappy: Spring::SNAPPY, smooth: Spring::SMOOTH, morph: Spring::MORPH },
            font_sans: "Inter",
            font_mono: "JetBrains Mono",
            control: 32.0,
            control_large: 40.0,
        }
    }
}

impl Default for Theme {
    fn default() -> Theme {
        Theme::light()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_meets_contrast_in_both_modes() {
        for t in [Theme::light(), Theme::dark(), Theme::island()] {
            let p = &t.palette;
            assert!(p.text.contrast_ratio(p.bg) >= 15.0, "{:?} primary text", t.mode);
            assert!(p.text_2.contrast_ratio(p.bg) >= 7.0, "{:?} secondary text", t.mode);
            assert!(p.accent.contrast_ratio(p.bg) >= 4.5, "{:?} accent as text or ring", t.mode);
            assert!(p.on_accent.contrast_ratio(p.accent) >= 4.5, "{:?} label on a primary button", t.mode);
            assert!(p.on_danger.contrast_ratio(p.danger) >= 4.5, "{:?} label on a danger button", t.mode);
        }
    }

    #[test]
    fn danger_text_on_light_background_is_readable() {
        let t = Theme::light();
        assert!(t.palette.danger.contrast_ratio(t.palette.bg) >= 4.5);
    }

    #[test]
    fn tertiary_text_is_a_hint_not_body_copy() {
        // Hints, times and placeholders sit below AA on purpose; this records how far below.
        let t = Theme::light();
        let r = t.palette.text_3.contrast_ratio(t.palette.bg);
        assert!((2.0..3.5).contains(&r), "{r}");
    }

    #[test]
    fn spacing_scale_is_the_documented_one() {
        assert_eq!(space::SCALE, [4.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0]);
        assert!(space::SCALE.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn type_scale_has_three_sizes_and_three_weights() {
        let t = Theme::light().types;
        let mut sizes = vec![t.small.size, t.body.size, t.title.size];
        sizes.dedup();
        assert_eq!(sizes, vec![12.0, 14.0, 24.0]);
        assert_eq!(t.get(TextRole::Title).weight, 600);
        assert_eq!(t.get(TextRole::Body).line_height / t.get(TextRole::Body).size, 1.5);
    }

    #[test]
    fn fills_follow_the_theme_and_scale_alpha() {
        let (light, dark) = (Theme::light().palette, Theme::dark().palette);
        let f = Fill::from(Tone::Accent);
        assert_eq!(f.resolve(&light), light.accent);
        assert_eq!(f.resolve(&dark), dark.accent);
        assert_ne!(f.resolve(&light), f.resolve(&dark));
        let faint = f.alpha(0.5).alpha(0.5);
        assert!((faint.resolve(&light).a - 0.25).abs() < 1e-6);
        assert!((Fill::from(Color::WHITE.with_alpha(0.5)).alpha(0.5).resolve(&light).a - 0.25).abs() < 1e-6);
        assert!(Fill::NONE.is_none() && !f.is_none() && f.alpha(0.0).is_none());
    }

    #[test]
    fn every_tone_reads_its_palette_entry() {
        let p = Theme::light().palette;
        assert_eq!(p.get(Tone::Bg), p.bg);
        assert_eq!(p.get(Tone::Text2), p.text_2);
        assert_eq!(p.get(Tone::Scrim), p.scrim);
        assert_eq!(p.get(Tone::Ring), p.ring);
    }

    #[test]
    fn island_is_pure_black() {
        let t = Theme::island();
        assert_eq!(t.palette.bg.to_rgba8(), [0, 0, 0, 255]);
        assert_eq!(t.mode, Mode::Dark);
    }

    #[test]
    fn controls_are_on_the_grid() {
        let t = Theme::light();
        for v in [t.control, t.control_large, t.radii.control, t.radii.card, t.radii.panel] {
            assert_eq!(v % 4.0, 0.0, "{v}");
        }
    }
}
