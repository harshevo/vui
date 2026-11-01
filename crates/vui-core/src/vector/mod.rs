//! Vector graphics: paths, fills, strokes and picking, all on the CPU, ending in a [`Mesh`] of coloured
//! triangles that the renderer draws in one pass among the quads, images and text.
//!
//! The pieces are small and independent. A [`Path`] is built in its own coordinates and flattened under
//! a [`Transform`]; [`fill_rings`], [`band`] and [`stroke_rings`] turn the flattened rings into
//! antialiased triangles; [`hit`] answers which shape is under a point. [`Vector`] ties them together
//! for the common case of "draw this path, in this paint, under this transform".

mod fill;
pub mod hit;
mod mesh;
mod path;
mod stroke;
mod transform;

pub use fill::{band, fill_rings, polygon, signed_area};
pub use hit::FillRule;
pub use mesh::{Mesh, Rgba, Vertex, scaled};
pub use path::{Path, Ring, Seg};
pub use stroke::{Cap, Join, Stroke, dash, stroke_rings};
pub use transform::Transform;

use crate::color::Color;
use crate::geom::Point;

/// Builds a mesh from paths. `px` is the size of one device pixel in the space the paths end up in, which
/// sets how soft edges are and how finely curves are cut.
pub struct Vector {
    pub mesh: Mesh,
    px: f32,
}

impl Vector {
    pub fn new(scale: f32) -> Vector {
        Vector { mesh: Mesh::new(), px: 1.0 / scale.max(0.1) }
    }

    pub fn px(&self) -> f32 {
        self.px
    }

    fn flatten(&self, path: &Path, t: &Transform) -> Vec<Ring> {
        path.flatten(t, self.px * 0.25)
    }

    pub fn fill(&mut self, path: &Path, t: &Transform, color: Color) -> &mut Self {
        let rings = self.flatten(path, t);
        fill_rings(&mut self.mesh, &rings, color.premultiplied(), self.px);
        self
    }

    /// Strokes `path`; `stroke.width` is in the path's own units and grows with the transform.
    pub fn stroke(&mut self, path: &Path, t: &Transform, stroke: &Stroke, color: Color) -> &mut Self {
        let rings = self.flatten(path, t);
        let k = t.scale_factor();
        let mut s = stroke.clone();
        s.width *= k;
        s.dash.iter_mut().for_each(|d| *d *= k);
        s.dash_offset *= k;
        stroke_rings(&mut self.mesh, &rings, &s, color.premultiplied(), self.px);
        self
    }

    /// A stroke whose width is in device-independent window units, whatever the transform: grid lines and
    /// outlines that should not thicken when a graph is zoomed.
    pub fn stroke_fixed(&mut self, path: &Path, t: &Transform, stroke: &Stroke, color: Color) -> &mut Self {
        let rings = self.flatten(path, t);
        stroke_rings(&mut self.mesh, &rings, stroke, color.premultiplied(), self.px);
        self
    }

    pub fn polyline(&mut self, points: &[Point], stroke: &Stroke, color: Color) -> &mut Self {
        let rings = [Ring { points: points.to_vec(), closed: false }];
        stroke_rings(&mut self.mesh, &rings, stroke, color.premultiplied(), self.px);
        self
    }

    pub fn band(&mut self, upper: &[Point], lower: &[Point], color: Color) -> &mut Self {
        band(&mut self.mesh, upper, lower, color.premultiplied(), self.px);
        self
    }

    pub fn finish(self) -> Mesh {
        self.mesh
    }
}
