//! How data pictures look: one set of colours and type sizes, taken from the theme's palette so light and
//! dark both work and nothing is hard-coded. Neutral ink, one accent, and ramps made of the two.

use vui_core::{Color, Palette, TextStyle};

#[derive(Clone, Debug, PartialEq)]
pub struct VizStyle {
    /// Background of the sheet the picture sits on, for knock-out outlines and tooltips.
    pub bg: Color,
    /// Strong, secondary and faint ink: text and marks.
    pub ink: Color,
    pub ink_2: Color,
    pub ink_3: Color,
    /// Hairlines: grid, axes and borders.
    pub line: Color,
    /// A faint fill: hover bands and bar tracks.
    pub wash: Color,
    pub accent: Color,
    pub label: TextStyle,
    pub label_mono: TextStyle,
    pub title: TextStyle,
}

impl VizStyle {
    pub fn from_palette(p: &Palette) -> VizStyle {
        VizStyle {
            bg: p.bg,
            ink: p.text,
            ink_2: p.text_2,
            ink_3: p.text_3,
            line: p.line,
            wash: p.wash,
            accent: p.accent,
            label: TextStyle::new(11.0, 14.0),
            label_mono: TextStyle::new(11.0, 14.0).mono(),
            title: TextStyle::new(13.0, 18.0).weight(500),
        }
    }

    /// Series colours, in the order they are assigned: ink, accent, then lighter mixes of each. Five
    /// distinguishable marks from one hue and the neutrals; past that, charts should label directly.
    pub fn series(&self, i: usize) -> Color {
        match i % 5 {
            0 => self.ink,
            1 => self.accent,
            2 => self.ink_3,
            3 => self.accent.lerp(self.bg, 0.5),
            _ => self.ink_2,
        }
    }

    /// A sequential ramp from the sheet's colour to the accent, for magnitudes.
    pub fn ramp(&self, t: f32) -> Color {
        self.bg.lerp(self.accent, 0.08 + 0.92 * t.clamp(0.0, 1.0))
    }

    /// A diverging ramp: the accent for positive, ink for negative, neutral at zero. `t` is -1 to 1.
    pub fn diverging(&self, t: f32) -> Color {
        let t = t.clamp(-1.0, 1.0);
        let end = if t >= 0.0 { self.accent } else { self.ink_2 };
        self.bg.lerp(end, 0.06 + 0.94 * t.abs())
    }
}

/// What a mark is painted with.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Ink {
    /// The next colour in the series order, by layer position.
    Auto,
    Strong,
    Accent,
    Mid,
    Soft,
    Color(Color),
}

impl Ink {
    pub fn resolve(self, style: &VizStyle, index: usize) -> Color {
        match self {
            Ink::Auto => style.series(index),
            Ink::Strong => style.ink,
            Ink::Accent => style.accent,
            Ink::Mid => style.ink_2,
            Ink::Soft => style.ink_3,
            Ink::Color(c) => c,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vui_core::Theme;

    #[test]
    fn light_and_dark_styles_keep_their_ramps_between_the_sheet_and_the_accent() {
        for theme in [Theme::light(), Theme::dark()] {
            let s = VizStyle::from_palette(&theme.palette);
            assert_eq!(
                s.ramp(1.0).to_rgba8()[..3],
                s.accent.lerp(s.bg, 0.0).to_rgba8()[..3]
            );
            assert!(s.ramp(0.0).contrast_ratio(s.bg) < s.ramp(1.0).contrast_ratio(s.bg));
            assert_eq!(
                s.diverging(0.0).to_rgba8(),
                s.bg.lerp(s.accent, 0.06).to_rgba8(),
                "neutral sits at the sheet colour"
            );
            assert_ne!(s.series(0), s.series(1));
            assert_eq!(s.series(0), s.series(5));
            assert_eq!(Ink::Accent.resolve(&s, 3), s.accent);
            assert_eq!(Ink::Auto.resolve(&s, 1), s.accent);
        }
    }
}
