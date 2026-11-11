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
mod pictures;
mod shading;
mod shapes;
mod triangles;

pub use canvas::Canvas;

use vui_core::scene::{Command, Scene};
use vui_core::{Fonts, Resources};

use crate::image::Image;

pub struct CpuRenderer {
    resources: Resources,
    canvas: Canvas,
    glyphs: glyphs::Glyphs,
    pictures: pictures::Pictures,
}

impl CpuRenderer {
    /// `resources` must be the same `Resources` that shaped the text and registered the images in the
    /// scenes you render.
    pub fn new(resources: Resources) -> CpuRenderer {
        CpuRenderer { resources, canvas: Canvas::new(0, 0), glyphs: Default::default(), pictures: Default::default() }
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
        let scale = scene.scale;
        for command in &scene.commands {
            match command {
                Command::Quad(q) => shapes::quad(&mut self.canvas, q, scale),
                Command::Image(i) => self.pictures.draw(&mut self.canvas, &self.resources.images, i, scale),
                Command::Text(t) => self.glyphs.draw(&mut self.canvas, &self.resources.fonts, t, scale),
                Command::Mesh(m) => triangles::mesh(&mut self.canvas, m, scale),
                Command::Custom(c) if c.pass == vui_core::effects::PASS => shapes::effect(&mut self.canvas, c, scale),
                Command::Custom(c) if c.pass == vui_core::shading::PASS => shading::shade(&mut self.canvas, c, scale),
                Command::Custom(_) => {}
            }
        }
        self.pictures.end_frame();
        &self.canvas
    }

    /// Renders `scene` and returns it as a straight-alpha image, like the GPU renderer's read-back.
    pub fn render_to_image(&mut self, scene: &Scene) -> Image {
        self.render(scene).to_image()
    }
}
