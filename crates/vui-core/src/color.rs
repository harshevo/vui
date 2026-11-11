//! Colour.
//!
//! `Color` holds sRGB-encoded components with straight (non-premultiplied) alpha, exactly as a designer
//! writes them. The renderer blends in this encoded space, like a browser or Figma does, so the
//! numbers in the design tokens are the numbers on screen. Conversions to linear light are provided
//! for the few places that need them (image filtering, contrast checks).

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

impl Color {
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color { r, g, b, a: 1.0 }
    }

    pub const fn rgb8(r: u8, g: u8, b: u8) -> Color {
        Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    /// `Color::hex(0x2563eb)`.
    pub const fn hex(rgb: u32) -> Color {
        Color::rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    }

    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa` (the leading `#` is optional).
    pub fn parse_hex(s: &str) -> Option<Color> {
        let s = s.strip_prefix('#').unwrap_or(s);
        let digits: Vec<u8> = s.chars().map(|c| c.to_digit(16).map(|d| d as u8)).collect::<Option<_>>()?;
        let byte = |hi: u8, lo: u8| hi * 16 + lo;
        match digits.as_slice() {
            [r, g, b] => Some(Color::rgb8(byte(*r, *r), byte(*g, *g), byte(*b, *b))),
            [r1, r2, g1, g2, b1, b2] => Some(Color::rgb8(byte(*r1, *r2), byte(*g1, *g2), byte(*b1, *b2))),
            [r1, r2, g1, g2, b1, b2, a1, a2] => Some(
                Color::rgb8(byte(*r1, *r2), byte(*g1, *g2), byte(*b1, *b2)).with_alpha(byte(*a1, *a2) as f32 / 255.0),
            ),
            _ => None,
        }
    }

    /// Replaces the alpha.
    pub const fn with_alpha(self, a: f32) -> Color {
        Color { a, ..self }
    }

    /// Multiplies the alpha, e.g. to fade a widget.
    pub fn mul_alpha(self, k: f32) -> Color {
        Color { a: (self.a * k).clamp(0.0, 1.0), ..self }
    }

    /// Interpolates in encoded sRGB space, which is what the GPU blend does too.
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let m = |a: f32, b: f32| a + (b - a) * t;
        Color { r: m(self.r, o.r), g: m(self.g, o.g), b: m(self.b, o.b), a: m(self.a, o.a) }
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Premultiplied components, as the shaders output them.
    pub fn premultiplied(self) -> [f32; 4] {
        [self.r * self.a, self.g * self.a, self.b * self.a, self.a]
    }

    /// Source-over compositing of `self` on `below`, in encoded space.
    pub fn over(self, below: Color) -> Color {
        let a = self.a + below.a * (1.0 - self.a);
        if a <= 0.0 {
            return Color::TRANSPARENT;
        }
        let c = |s: f32, d: f32| (s * self.a + d * below.a * (1.0 - self.a)) / a;
        Color { r: c(self.r, below.r), g: c(self.g, below.g), b: c(self.b, below.b), a }
    }

    pub fn to_linear_rgb(self) -> [f32; 3] {
        [srgb_to_linear(self.r), srgb_to_linear(self.g), srgb_to_linear(self.b)]
    }

    /// WCAG relative luminance.
    pub fn luminance(self) -> f32 {
        let [r, g, b] = self.to_linear_rgb();
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// WCAG contrast ratio between two opaque colours, from 1 to 21.
    pub fn contrast_ratio(self, o: Color) -> f32 {
        let (a, b) = (self.luminance(), o.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    pub fn to_rgba8(self) -> [u8; 4] {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [q(self.r), q(self.g), q(self.b), q(self.a)]
    }

    /// `#rrggbb`, or `#rrggbbaa` when it is not opaque.
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.to_rgba8();
        if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
    }

    /// Hue in degrees (0 to 360), saturation and value (0 to 1), of the encoded sRGB values: what a
    /// colour picker's square and strip show.
    pub fn to_hsv(self) -> [f32; 3] {
        let (r, g, b) = (self.r.clamp(0.0, 1.0), self.g.clamp(0.0, 1.0), self.b.clamp(0.0, 1.0));
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let hue = if delta <= 1e-6 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        [hue, if max <= 1e-6 { 0.0 } else { delta / max }, max]
    }

    /// The opaque colour with this hue (degrees), saturation and value (0 to 1).
    pub fn from_hsv(hue: f32, saturation: f32, value: f32) -> Color {
        let (h, s, v) = (hue.rem_euclid(360.0) / 60.0, saturation.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
        let c = v * s;
        let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        Color::rgb(r + m, g + m, b + m)
    }
}

impl Default for Color {
    fn default() -> Color {
        Color::TRANSPARENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn hex_round_trip() {
        let c = Color::hex(0x2563eb);
        assert_eq!(c.to_rgba8(), [0x25, 0x63, 0xeb, 255]);
        assert_eq!(Color::parse_hex("#2563eb"), Some(c));
        assert_eq!(Color::parse_hex("2563EB"), Some(c));
        assert_eq!(Color::parse_hex("#fff"), Some(Color::WHITE));
        assert_eq!(Color::parse_hex("#00000080").map(|c| c.to_rgba8()), Some([0, 0, 0, 128]));
        assert_eq!(Color::parse_hex("#12"), None);
        assert_eq!(Color::parse_hex("#gggggg"), None);
    }

    #[test]
    fn srgb_transfer_round_trips() {
        for i in 0..=255 {
            let c = i as f32 / 255.0;
            assert!(close(linear_to_srgb(srgb_to_linear(c)), c), "{i}");
        }
        assert!(close(srgb_to_linear(0.5), 0.2140));
    }

    #[test]
    fn contrast_matches_wcag() {
        assert!(close(Color::BLACK.contrast_ratio(Color::WHITE), 21.0));
        assert!(close(Color::WHITE.contrast_ratio(Color::WHITE), 1.0));
        // known pair: #767676 on white is the 4.54:1 AA boundary
        let r = Color::hex(0x767676).contrast_ratio(Color::WHITE);
        assert!((4.5..4.6).contains(&r), "{r}");
    }

    #[test]
    fn over_composites_in_encoded_space() {
        let half_black = Color::BLACK.with_alpha(0.5);
        let out = half_black.over(Color::WHITE);
        assert!(close(out.r, 0.5) && close(out.a, 1.0));
        // transparent over transparent stays transparent
        assert_eq!(Color::TRANSPARENT.over(Color::TRANSPARENT), Color::TRANSPARENT);
    }

    #[test]
    fn premultiplied_scales_rgb_by_alpha() {
        let p = Color::rgba(1.0, 0.5, 0.0, 0.5).premultiplied();
        assert_eq!(p, [0.5, 0.25, 0.0, 0.5]);
    }

    #[test]
    fn lerp_and_alpha() {
        let m = Color::BLACK.lerp(Color::WHITE, 0.25);
        assert!(close(m.r, 0.25));
        assert!(close(Color::WHITE.mul_alpha(0.5).a, 0.5));
        assert_eq!(Color::WHITE.mul_alpha(4.0).a, 1.0);
    }
}

#[cfg(test)]
mod hsv_tests {
    use super::Color;

    #[test]
    fn hsv_round_trips() {
        for c in [Color::hex(0x4f5bd5), Color::hex(0xec4899), Color::hex(0x14b8a6), Color::WHITE, Color::BLACK, Color::hex(0x808080)] {
            let [h, s, v] = c.to_hsv();
            assert_eq!(Color::from_hsv(h, s, v).to_hex(), c.to_hex());
        }
        assert_eq!(Color::from_hsv(120.0, 1.0, 1.0).to_hex(), "#00ff00");
        assert_eq!(Color::hex(0xff0000).with_alpha(0.5).to_hex(), "#ff000080");
    }
}
