//! Icons: a set of layers of SVG path data on a 24 by 24 grid, drawn at any pixel size and weight.

use std::hash::{Hash, Hasher};

use vui_core::{ImageId, Images};

use crate::path::{self, PathError};
use crate::raster::{Canvas, Mask};

/// Icons are drawn on a grid this many units wide and tall.
pub const GRID: f32 = 24.0;

/// How finely curves are cut into lines, in output pixels.
const FLATNESS: f32 = 0.05;

/// How a layer's outline is painted.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A line of the icon's weight, with round ends and corners.
    Stroke,
    /// The inside of the outline.
    Fill,
    /// The inside, plus a line along the edge, which rounds off the corners of a solid shape.
    Both,
}

/// One outline of an icon.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub kind: Kind,
    /// SVG path data on the 24 by 24 grid.
    pub d: &'static str,
}

/// An icon's name and outlines. Build them with [`icon!`](crate::icon).
#[derive(Debug, PartialEq, Eq)]
pub struct IconData {
    pub name: &'static str,
    pub layers: &'static [Layer],
}

/// How heavy the lines of an icon are.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Weight {
    Light,
    #[default]
    Regular,
    Medium,
    Bold,
}

impl Weight {
    /// Line width in grid units.
    pub fn stroke(self) -> f32 {
        match self {
            Weight::Light => 1.25,
            Weight::Regular => 1.5,
            Weight::Medium => 1.75,
            Weight::Bold => 2.0,
        }
    }
}

/// A cheap handle to an icon's data.
#[derive(Copy, Clone)]
pub struct Icon(&'static IconData);

impl std::fmt::Debug for Icon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Icon({})", self.0.name)
    }
}

impl PartialEq for Icon {
    fn eq(&self, other: &Icon) -> bool {
        self.0.name == other.0.name
    }
}

impl Eq for Icon {}

impl Hash for Icon {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.name.hash(state);
    }
}

/// Defines an icon from layers of path data on the 24 by 24 grid:
///
/// ```
/// use vui_icons::{Icon, icon};
///
/// const TICK: Icon = icon!("tick", Stroke "M5 12.5l4.5 4.5L19 7.5");
/// assert_eq!(TICK.name(), "tick");
/// assert!(TICK.validate().is_ok());
/// ```
#[macro_export]
macro_rules! icon {
    ($name:expr, $($kind:ident $d:expr),+ $(,)?) => {
        $crate::Icon::new(&$crate::IconData {
            name: $name,
            layers: &[$($crate::Layer { kind: $crate::Kind::$kind, d: $d }),+],
        })
    };
}

impl Icon {
    pub const fn new(data: &'static IconData) -> Icon {
        Icon(data)
    }

    pub fn name(self) -> &'static str {
        self.0.name
    }

    pub fn layers(self) -> &'static [Layer] {
        self.0.layers
    }

    /// Checks that every layer's path data parses.
    pub fn validate(self) -> Result<(), PathError> {
        self.0.layers.iter().try_for_each(|layer| path::parse(layer.d).map(|_| ()))
    }

    /// Draws the icon into a square of `px` by `px` pixels. A layer that does not parse is left out;
    /// [`validate`](Icon::validate) says why.
    pub fn render(self, px: u32, weight: Weight) -> Mask {
        let size = px.max(1) as usize;
        let scale = size as f32 / GRID;
        let width = weight.stroke() * scale;
        let mut canvas = Canvas::new(size, size);
        for layer in self.0.layers {
            let Ok(segs) = path::parse(layer.d) else { continue };
            let segs: Vec<_> = segs.into_iter().map(|s| s.scaled(scale)).collect();
            let polys = path::flatten(&segs, FLATNESS);
            if matches!(layer.kind, Kind::Fill | Kind::Both) {
                canvas.fill(&polys, 1.0);
            }
            if matches!(layer.kind, Kind::Stroke | Kind::Both) {
                canvas.stroke(&polys, width, 1.0);
            }
        }
        canvas.finish()
    }

    /// The icon as a mask image in `images`, made once per size and weight and shared after that.
    pub fn image(self, images: &Images, px: u32, weight: Weight) -> ImageId {
        images.get_or_insert_with(&(self.0.name, px, weight), || self.render(px, weight).to_image())
    }
}
