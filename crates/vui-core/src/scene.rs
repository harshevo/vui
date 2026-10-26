//! The scene: what a frame looks like, as a flat list of drawing commands.
//!
//! Widgets paint into a `Scene`; a renderer turns it into pixels. Keeping the scene a plain value
//! means painting is testable without a GPU (assert on the commands) and the same scene can be
//! rendered headless, to a window, or to a texture for a screenshot.

use std::rc::Rc;

use crate::color::Color;
use crate::geom::{Corners, Point, Rect, Size, rect};
use crate::image::ImageId;
use crate::text::TextLayout;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}

/// A rounded rectangle with an optional fill and inner border: the one primitive nearly every
/// control is made of. The border is drawn inside the rect, like CSS `box-sizing: border-box`.
#[derive(Clone, Debug, PartialEq)]
pub struct Quad {
    pub rect: Rect,
    pub radii: Corners,
    /// 0 gives circular corners; 1 gives fully continuous ("squircle") corners.
    pub smooth: f32,
    pub fill: Color,
    pub border: Option<Border>,
    /// Anything outside this rect (logical pixels) is cut away.
    pub clip: Option<Rect>,
}

impl Quad {
    pub fn new(rect: Rect) -> Quad {
        Quad { rect, radii: Corners::ZERO, smooth: 0.0, fill: Color::TRANSPARENT, border: None, clip: None }
    }

    pub fn fill(mut self, color: Color) -> Quad {
        self.fill = color;
        self
    }

    pub fn radii(mut self, radii: impl Into<Corners>) -> Quad {
        self.radii = radii.into();
        self
    }

    pub fn smooth(mut self, smooth: f32) -> Quad {
        self.smooth = smooth.clamp(0.0, 1.0);
        self
    }

    pub fn border(mut self, width: f32, color: Color) -> Quad {
        self.border = Some(Border { width, color });
        self
    }

    pub fn clip(mut self, clip: Option<Rect>) -> Quad {
        self.clip = clip;
        self
    }
}

/// How an image is sampled.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Sampling {
    /// Trilinear with anisotropic filtering: the right choice for photos and thumbnails.
    #[default]
    Smooth,
    /// Crisp texel edges with a one-pixel blend: keeps screenshot text sharp when zoomed in.
    Sharp,
    /// Nearest texel: pixel art.
    Nearest,
}

/// What the image's pixels mean.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ImageMode {
    /// Draw the picture.
    #[default]
    Color,
    /// Use only the alpha channel as a stencil and paint it in `tint`: how icons are drawn.
    Mask,
}

/// A picture with rounded corners, an optional hairline ring and a source crop.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageCmd {
    pub id: ImageId,
    pub rect: Rect,
    pub radii: Corners,
    pub smooth: f32,
    /// The part of the image to show, in normalised 0..1 coordinates.
    pub uv: Rect,
    pub opacity: f32,
    /// The colour for `ImageMode::Mask`.
    pub tint: Color,
    pub mode: ImageMode,
    pub sampling: Sampling,
    /// A hairline drawn over the image's edge, inside the rect.
    pub ring: Option<Border>,
    pub clip: Option<Rect>,
}

impl ImageCmd {
    pub fn new(id: ImageId, rect_: Rect) -> ImageCmd {
        ImageCmd {
            id,
            rect: rect_,
            radii: Corners::ZERO,
            smooth: 0.0,
            uv: rect(0.0, 0.0, 1.0, 1.0),
            opacity: 1.0,
            tint: Color::BLACK,
            mode: ImageMode::Color,
            sampling: Sampling::Smooth,
            ring: None,
            clip: None,
        }
    }

    pub fn radii(mut self, radii: impl Into<Corners>) -> ImageCmd {
        self.radii = radii.into();
        self
    }

    pub fn smooth(mut self, smooth: f32) -> ImageCmd {
        self.smooth = smooth.clamp(0.0, 1.0);
        self
    }

    pub fn uv(mut self, uv: Rect) -> ImageCmd {
        self.uv = uv;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> ImageCmd {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn mask(mut self, tint: Color) -> ImageCmd {
        self.mode = ImageMode::Mask;
        self.tint = tint;
        self
    }

    pub fn sampling(mut self, sampling: Sampling) -> ImageCmd {
        self.sampling = sampling;
        self
    }

    pub fn ring(mut self, width: f32, color: Color) -> ImageCmd {
        self.ring = Some(Border { width, color });
        self
    }

    pub fn clip(mut self, clip: Option<Rect>) -> ImageCmd {
        self.clip = clip;
        self
    }
}

/// Shaped text drawn with its top-left corner at `origin`.
#[derive(Clone, Debug)]
pub struct TextCmd {
    pub layout: Rc<TextLayout>,
    pub origin: Point,
    pub color: Color,
    /// Anything outside this rect (logical pixels) is cut away.
    pub clip: Option<Rect>,
}

#[derive(Clone, Debug)]
pub enum Command {
    Quad(Quad),
    Image(ImageCmd),
    Text(TextCmd),
}

#[derive(Clone, Debug)]
pub struct Scene {
    /// Logical size of the frame.
    pub size: Size,
    /// Device pixels per logical pixel.
    pub scale: f32,
    /// Colour the frame is cleared to before the commands run. `None` keeps what is already there.
    pub clear: Option<Color>,
    pub commands: Vec<Command>,
}

impl Scene {
    pub fn new(size: Size, scale: f32) -> Scene {
        Scene { size, scale, clear: None, commands: Vec::new() }
    }

    pub fn with_clear(mut self, color: Color) -> Scene {
        self.clear = Some(color);
        self
    }

    pub fn push(&mut self, command: Command) {
        self.commands.push(command);
    }

    pub fn quad(&mut self, quad: Quad) {
        self.commands.push(Command::Quad(quad));
    }

    pub fn image(&mut self, image: ImageCmd) {
        self.commands.push(Command::Image(image));
    }

    pub fn text(&mut self, layout: Rc<TextLayout>, origin: Point, color: Color, clip: Option<Rect>) {
        self.commands.push(Command::Text(TextCmd { layout, origin, color, clip }));
    }

    /// Frame size in device pixels, rounded up.
    pub fn pixel_size(&self) -> (u32, u32) {
        ((self.size.w * self.scale).ceil() as u32, (self.size.h * self.scale).ceil() as u32)
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{rect, size};

    #[test]
    fn quad_builder_sets_fields() {
        let q = Quad::new(rect(0.0, 0.0, 10.0, 10.0)).fill(Color::WHITE).radii(4.0).smooth(2.0).border(1.0, Color::BLACK);
        assert_eq!(q.radii, Corners::all(4.0));
        assert_eq!(q.smooth, 1.0, "smooth is clamped to 0..=1");
        assert_eq!(q.border.map(|b| b.width), Some(1.0));
    }

    #[test]
    fn pixel_size_rounds_up() {
        let s = Scene::new(size(100.5, 50.0), 2.0);
        assert_eq!(s.pixel_size(), (201, 100));
    }

    #[test]
    fn scene_collects_commands() {
        let mut s = Scene::new(size(10.0, 10.0), 1.0).with_clear(Color::WHITE);
        assert!(s.is_empty());
        s.quad(Quad::new(rect(0.0, 0.0, 1.0, 1.0)));
        assert_eq!(s.commands.len(), 1);
        assert_eq!(s.clear, Some(Color::WHITE));
    }
}
