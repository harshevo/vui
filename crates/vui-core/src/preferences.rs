//! The user's accessibility settings: less motion, more contrast, larger text. The platform reads
//! them from the system and the runtime follows them; a theme adapts to them with
//! [`Theme::adapted`].

use crate::color::Color;
use crate::theme::{Mode, TextToken, Theme};

/// Accessibility settings the interface follows.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Preferences {
    /// Springs and transitions jump to where they are going, keyframes hold their first pose, and
    /// effects hold still.
    pub reduce_motion: bool,
    /// Stronger lines and secondary text, and an accent that stands out from the page.
    pub high_contrast: bool,
    /// Text, and the controls holding it, at this multiple of the theme's size: 1 is as designed.
    pub text_scale: f32,
}

impl Preferences {
    pub const DEFAULT: Preferences = Preferences { reduce_motion: false, high_contrast: false, text_scale: 1.0 };
}

impl Default for Preferences {
    fn default() -> Preferences {
        Preferences::DEFAULT
    }
}

/// The contrast against the page that text and the accent are held to with `high_contrast`: WCAG
/// AA for body text.
const TEXT_CONTRAST: f32 = 4.5;
/// The contrast lines and control edges are held to: WCAG's 3:1 for interface parts.
const LINE_CONTRAST: f32 = 3.0;

impl Theme {
    /// This theme as `prefs` ask for it: higher contrast and scaled text. Motion is the runtime's.
    pub fn adapted(&self, prefs: &Preferences) -> Theme {
        let mut t = self.clone();
        if prefs.high_contrast {
            let p = &mut t.palette;
            let (page, ink) = (p.bg, p.text);
            p.line = until_contrast(p.line, ink, page, LINE_CONTRAST);
            p.text_2 = until_contrast(p.text_2, ink, page, 7.0);
            p.text_3 = until_contrast(p.text_3, ink, page, TEXT_CONTRAST);
            p.accent = until_contrast(p.accent, ink, page, TEXT_CONTRAST);
            p.danger = until_contrast(p.danger, ink, page, TEXT_CONTRAST);
            p.ring = p.ring.with_alpha((p.ring.a * 2.0).min(1.0));
            p.wash = p.wash.with_alpha((p.wash.a * 2.0).min(1.0));
            p.hover = p.hover.with_alpha((p.hover.a * 2.0).min(1.0));
            p.track_off = until_contrast(p.track_off, ink, page, LINE_CONTRAST);
            if t.mode == Mode::Dark {
                p.side = p.side.lerp(page, 0.5);
            }
        }
        let k = prefs.text_scale.clamp(0.75, 2.5);
        if k != 1.0 {
            let scale = |tok: &mut TextToken| {
                tok.size = (tok.size * k).round();
                tok.line_height = (tok.line_height * k).round();
            };
            scale(&mut t.types.small);
            scale(&mut t.types.body);
            scale(&mut t.types.title);
            scale(&mut t.types.mono);
            t.control = (t.control * k).round();
            t.control_large = (t.control_large * k).round();
        }
        t
    }
}

/// `color` moved towards `ink` until it stands `ratio` apart from `page`, or as far as `ink` itself.
fn until_contrast(color: Color, ink: Color, page: Color, ratio: f32) -> Color {
    let solid = color.with_alpha(1.0).over(page);
    if solid.contrast_ratio(page) >= ratio {
        return solid;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..16 {
        let mid = (lo + hi) / 2.0;
        if solid.lerp(ink, mid).contrast_ratio(page) >= ratio {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    solid.lerp(ink, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_contrast_meets_wcag_in_both_modes() {
        let prefs = Preferences { high_contrast: true, ..Preferences::DEFAULT };
        for theme in [Theme::light(), Theme::dark()] {
            let p = theme.adapted(&prefs).palette;
            assert!(p.text_3.contrast_ratio(p.bg) >= TEXT_CONTRAST - 0.01, "hints are readable");
            assert!(p.accent.contrast_ratio(p.bg) >= TEXT_CONTRAST - 0.01, "the accent stands out");
            assert!(p.line.contrast_ratio(p.bg) >= LINE_CONTRAST - 0.01, "lines show");
            assert_eq!(p.text, theme.palette.text, "primary text is already as strong as it gets");
        }
    }

    #[test]
    fn text_scale_grows_type_and_controls() {
        let t = Theme::light();
        let big = t.adapted(&Preferences { text_scale: 1.5, ..Preferences::DEFAULT });
        assert_eq!(big.types.body.size, (t.types.body.size * 1.5).round());
        assert!(big.control > t.control);
        assert_eq!(t.adapted(&Preferences::DEFAULT), t, "the defaults change nothing");
    }
}
