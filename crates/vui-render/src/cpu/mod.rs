//! The CPU renderer: the same [`Scene`] drawn into memory with no GPU at all.
//!
//! It is the fallback for machines where no GPU adapter can be had (virtual machines, remote
//! sessions, CI) and is what `VUI_RENDERER=cpu` selects. Every command the GPU renderer draws is
//! drawn here from the same maths: rounded boxes, borders and continuous corners from the same
//! distance field, text from the same swash glyph images, pictures from a linear-light mip chain,
//! meshes at pixel centres, and the built-in shadow, shimmer and glow. Custom shader passes the app
//! added are GPU-only and are skipped.
//!
//! Frames match the GPU's to within a few levels per channel: the golden tests compare the two.

mod canvas;
mod glyphs;
mod layers;
mod pictures;
mod shading;
mod shapes;
mod triangles;

pub use canvas::Canvas;

use vui_core::scene::{Command, Scene};
use vui_core::{Fonts, LayerCmd, Resources};

use crate::image::Image;
use crate::layer;

pub struct CpuRenderer {
    resources: Resources,
    canvas: Canvas,
    glyphs: glyphs::Glyphs,
    pictures: pictures::Pictures,
    /// One canvas per depth that layers are drawn into.
    layers: Vec<Canvas>,
}

impl CpuRenderer {
    /// `resources` must be the same `Resources` that shaped the text and registered the images in the
    /// scenes you render.
    pub fn new(resources: Resources) -> CpuRenderer {
        CpuRenderer { resources, canvas: Canvas::new(0, 0), glyphs: Default::default(), pictures: Default::default(), layers: Vec::new() }
    }

    pub fn fonts(&self) -> &Fonts {
        &self.resources.fonts
    }

    /// Draws `scene` into the renderer's canvas, sized to the scene, and returns it. With no clear
    /// colour the scene draws over the previous frame, as on the GPU.
    pub fn render(&mut self, scene: &Scene) -> &Canvas {
        let (w, h) = scene.pixel_size();
        self.canvas.resize(w, h);
        if let Some(c) = scene.clear {
            self.canvas.clear(c.premultiplied());
        }
        let mut canvas = std::mem::replace(&mut self.canvas, Canvas::new(0, 0));
        self.draw(&mut canvas, &scene.commands, scene.scale, 0);
        self.canvas = canvas;
        self.pictures.end_frame();
        &self.canvas
    }

    fn draw(&mut self, canvas: &mut Canvas, commands: &[Command], scale: f32, depth: usize) {
        for command in commands {
            match command {
                Command::Quad(q) => shapes::quad(canvas, q, scale),
                Command::Image(i) => self.pictures.draw(canvas, &self.resources.images, i, scale),
                Command::Text(t) => self.glyphs.draw(canvas, &self.resources.fonts, t, scale),
                Command::Mesh(m) => triangles::mesh(canvas, m, scale),
                Command::Custom(c) if c.pass == vui_core::effects::PASS => shapes::effect(canvas, c, scale),
                Command::Custom(c) if c.pass == vui_core::shading::PASS => shading::shade(canvas, c, scale),
                Command::Custom(_) => {}
                Command::Layer(l) => self.layer(canvas, l, scale, depth),
            }
        }
    }

    /// Draws a layer's commands into the canvas for `depth`, then composites them over `canvas`.
    fn layer(&mut self, canvas: &mut Canvas, l: &LayerCmd, scale: f32, depth: usize) {
        let size = (canvas.width, canvas.height);
        let Some(region) = layer::region(l.bounds, scale, size) else { return };
        if depth >= layer::MAX_DEPTH || layer::device_inverse(&l.transform, scale).is_none() {
            return;
        }
        while self.layers.len() <= depth {
            self.layers.push(Canvas::new(0, 0));
        }
        let mut sub = std::mem::replace(&mut self.layers[depth], Canvas::new(0, 0));
        sub.resize(size.0, size.1);
        let [x, y, w, h] = region.map(|v| v as i32);
        sub.clear_span(canvas::Span { x0: x, y0: y, x1: x + w, y1: y + h });
        self.draw(&mut sub, &l.commands, scale, depth + 1);
        layers::composite(canvas, &sub, l, scale, region);
        self.layers[depth] = sub;
    }

    /// Renders `scene` and returns it as a straight-alpha image, like the GPU renderer's read-back.
    pub fn render_to_image(&mut self, scene: &Scene) -> Image {
        self.render(scene).to_image()
    }
}
