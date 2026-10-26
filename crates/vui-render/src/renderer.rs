//! The renderer: turns a [`Scene`] into pixels.

use std::ops::Range;

use glyphon::{TextArea, TextBounds};
use vui_core::scene::{Command, Scene, TextCmd};
use vui_core::{CustomCmd, Fonts, ImageId, Resources};

use crate::context::{Gpu, GpuError};
use crate::custom::CustomPass;
use crate::image::Image;
use crate::image_pipeline::{ImageInstance, ImagePipeline, ImageStats};
use crate::quad::{QuadInstance, QuadPipeline};
use crate::text::TextPipeline;

/// A run of commands of one kind. Runs are drawn in order, which keeps painter's-algorithm layering
/// between shapes and text.
enum Batch {
    Quads(Range<u32>),
    Image { instance: u32, id: ImageId },
    Text { renderer: usize, commands: Range<usize> },
    /// The `index`-th command of this frame for custom pass `pass`.
    Custom { pass: usize, index: usize },
}

struct Pass {
    name: &'static str,
    pass: Box<dyn CustomPass>,
    /// This frame's commands for the pass, as indices into the scene.
    commands: Vec<usize>,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    resources: Resources,
    quads: QuadPipeline,
    images: ImagePipeline,
    text: TextPipeline,
    scratch_quads: Vec<QuadInstance>,
    scratch_images: Vec<ImageInstance>,
    batches: Vec<Batch>,
    passes: Vec<Pass>,
}

impl Renderer {
    /// Builds the pipelines for rendering into textures of `format`. The shaders blend in encoded
    /// sRGB space, so pass a non-sRGB format (`Rgba8Unorm` or `Bgra8Unorm`). `resources` must be the
    /// same `Resources` that shaped the text and registered the images in the scenes you render.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat, resources: Resources) -> Renderer {
        let quads = QuadPipeline::new(&device, format);
        let images = ImagePipeline::new(&device, format);
        let text = TextPipeline::new(&device, &queue, format);
        Renderer {
            device,
            queue,
            format,
            resources,
            quads,
            images,
            text,
            scratch_quads: Vec::new(),
            scratch_images: Vec::new(),
            batches: Vec::new(),
            passes: Vec::new(),
        }
    }

    /// A renderer on a window-less device, drawing into `Rgba8Unorm` textures. Returns the GPU too,
    /// so tests can create their own textures.
    pub fn headless(resources: Resources) -> Result<(Renderer, Gpu), GpuError> {
        let gpu = Gpu::headless()?;
        let renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), wgpu::TextureFormat::Rgba8Unorm, resources);
        Ok((renderer, gpu))
    }

    /// Adds a custom shader pass that draws the scene's `CustomCmd`s named `name`. A pass added under a
    /// name that is already taken replaces the old one.
    pub fn add_pass(&mut self, name: &'static str, pass: Box<dyn CustomPass>) {
        match self.passes.iter_mut().find(|p| p.name == name) {
            Some(p) => p.pass = pass,
            None => self.passes.push(Pass { name, pass, commands: Vec::new() }),
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn fonts(&self) -> &Fonts {
        &self.resources.fonts
    }

    /// How much texture memory pictures may keep resident, in bytes. The least recently drawn
    /// pictures are evicted first, and are re-uploaded if they are drawn again.
    pub fn set_image_budget(&mut self, bytes: usize) {
        self.images.budget = bytes;
    }

    pub fn image_stats(&self) -> ImageStats {
        self.images.stats()
    }

    /// Draws `scene` into `view`, which must be `size_px` pixels and of the renderer's format.
    pub fn render(&mut self, scene: &Scene, view: &wgpu::TextureView, size_px: (u32, u32)) {
        self.plan(scene);
        let viewport = [size_px.0 as f32, size_px.1 as f32];
        self.quads.upload(&self.device, &self.queue, &self.scratch_quads, viewport);
        self.images.upload(&self.device, &self.queue, &self.scratch_images, viewport);
        self.text.begin_frame(&self.queue, size_px);
        for batch in &self.batches {
            if let Batch::Text { renderer, commands } = batch {
                let areas: Vec<TextArea<'_>> = scene.commands[commands.clone()]
                    .iter()
                    .filter_map(|c| match c {
                        Command::Text(t) => Some(text_area(t, scene.scale, size_px)),
                        _ => None,
                    })
                    .collect();
                self.text.prepare(*renderer, &self.device, &self.queue, &self.resources.fonts, areas);
            }
        }
        for p in &mut self.passes {
            let commands: Vec<&CustomCmd> = p
                .commands
                .iter()
                .filter_map(|&i| match &scene.commands[i] {
                    Command::Custom(c) => Some(c),
                    _ => None,
                })
                .collect();
            if !commands.is_empty() {
                p.pass.prepare(&self.device, &self.queue, viewport, scene.scale, &commands);
            }
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vui frame") });
        {
            let load = match scene.clear {
                Some(c) => {
                    let p = c.premultiplied();
                    wgpu::LoadOp::Clear(wgpu::Color { r: p[0] as f64, g: p[1] as f64, b: p[2] as f64, a: p[3] as f64 })
                }
                None => wgpu::LoadOp::Load,
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for batch in &self.batches {
                match batch {
                    Batch::Quads(range) => self.quads.draw(&mut pass, range.clone()),
                    Batch::Image { instance, id } => self.images.draw(&mut pass, *id, *instance),
                    Batch::Text { renderer, .. } => self.text.draw(*renderer, &mut pass),
                    Batch::Custom { pass: p, index } => self.passes[*p].pass.draw(&mut pass, *index),
                }
            }
        }
        self.queue.submit([encoder.finish()]);
        self.text.end_frame();
        self.images.end_frame();
    }

    /// Splits the scene into batches and converts the quads to instances.
    fn plan(&mut self, scene: &Scene) {
        self.scratch_quads.clear();
        self.scratch_images.clear();
        self.batches.clear();
        for p in &mut self.passes {
            p.commands.clear();
        }
        let mut text_runs = 0;
        let mut i = 0;
        while i < scene.commands.len() {
            match &scene.commands[i] {
                Command::Quad(_) => {
                    let start = self.scratch_quads.len() as u32;
                    while let Some(Command::Quad(q)) = scene.commands.get(i) {
                        self.scratch_quads.push(QuadInstance::from_quad(q, scene.scale));
                        i += 1;
                    }
                    self.batches.push(Batch::Quads(start..self.scratch_quads.len() as u32));
                }
                Command::Image(cmd) => {
                    i += 1;
                    if let Some(tex) = self.images.ensure(&self.device, &self.queue, &self.resources.images, cmd.id) {
                        let instance = self.scratch_images.len() as u32;
                        self.scratch_images.push(ImageInstance::new(cmd, tex, scene.scale));
                        self.batches.push(Batch::Image { instance, id: cmd.id });
                    }
                }
                Command::Text(_) => {
                    let start = i;
                    while matches!(scene.commands.get(i), Some(Command::Text(_))) {
                        i += 1;
                    }
                    self.batches.push(Batch::Text { renderer: text_runs, commands: start..i });
                    text_runs += 1;
                }
                Command::Custom(cmd) => {
                    if let Some(p) = self.passes.iter().position(|p| p.name == cmd.pass) {
                        let index = self.passes[p].commands.len();
                        self.passes[p].commands.push(i);
                        self.batches.push(Batch::Custom { pass: p, index });
                    }
                    i += 1;
                }
            }
        }
    }

    /// Renders `scene` into a fresh texture and reads it back as a straight-alpha RGBA image.
    pub fn render_to_image(&mut self, scene: &Scene) -> Result<Image, GpuError> {
        let (width, height) = scene.pixel_size();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vui offscreen"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.render(scene, &view, (width, height));
        self.read_texture(&texture, width, height)
    }

    fn read_texture(&self, texture: &wgpu::Texture, width: u32, height: u32) -> Result<Image, GpuError> {
        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vui readback"),
            size: padded as u64 * height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vui readback") });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(height) },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| GpuError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| GpuError::Readback(e.to_string()))?
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        let mapped = slice.get_mapped_range().map_err(|e| GpuError::Readback(e.to_string()))?;

        let swap_rb = matches!(self.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut rgba = Vec::with_capacity((unpadded * height) as usize);
        for row in 0..height as usize {
            let start = row * padded as usize;
            let (pixels, _) = mapped[start..start + unpadded as usize].as_chunks::<4>();
            for px in pixels {
                let (r, g, b, a) = if swap_rb { (px[2], px[1], px[0], px[3]) } else { (px[0], px[1], px[2], px[3]) };
                // The frame holds premultiplied colour; images are straight alpha.
                if a == 255 || a == 0 {
                    rgba.extend_from_slice(&[r, g, b, a]);
                } else {
                    let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                    rgba.extend_from_slice(&[un(r), un(g), un(b), a]);
                }
            }
        }
        drop(mapped);
        buffer.unmap();
        Ok(Image { width, height, rgba })
    }
}

fn text_area(t: &TextCmd, scale: f32, size_px: (u32, u32)) -> TextArea<'_> {
    let bounds = match t.clip {
        Some(c) => TextBounds {
            left: ((c.x * scale).floor() as i32).max(0),
            top: ((c.y * scale).floor() as i32).max(0),
            right: (((c.x + c.w) * scale).ceil() as i32).min(size_px.0 as i32),
            bottom: (((c.y + c.h) * scale).ceil() as i32).min(size_px.1 as i32),
        },
        None => TextBounds { left: 0, top: 0, right: size_px.0 as i32, bottom: size_px.1 as i32 },
    };
    let [r, g, b, a] = t.color.to_rgba8();
    TextArea {
        buffer: t.layout.buffer(),
        left: t.origin.x * scale,
        top: t.origin.y * scale,
        scale,
        bounds,
        default_color: glyphon::Color::rgba(r, g, b, a),
        custom_glyphs: &[],
    }
}
