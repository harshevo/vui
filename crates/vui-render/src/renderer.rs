//! The renderer: turns a [`Scene`] into pixels.

use vui_core::scene::{Command, Scene};

use crate::context::{Gpu, GpuError};
use crate::image::Image;
use crate::quad::{QuadInstance, QuadPipeline};

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    quads: QuadPipeline,
    scratch_quads: Vec<QuadInstance>,
}

impl Renderer {
    /// Builds the pipelines for rendering into textures of `format`. The shaders blend in encoded
    /// sRGB space, so pass a non-sRGB format (`Rgba8Unorm` or `Bgra8Unorm`).
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Renderer {
        let quads = QuadPipeline::new(&device, format);
        Renderer { device, queue, format, quads, scratch_quads: Vec::new() }
    }

    /// A renderer on a window-less device, drawing into `Rgba8Unorm` textures. Returns the GPU too,
    /// so tests can create their own textures.
    pub fn headless() -> Result<(Renderer, Gpu), GpuError> {
        let gpu = Gpu::headless()?;
        let renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), wgpu::TextureFormat::Rgba8Unorm);
        Ok((renderer, gpu))
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

    /// Draws `scene` into `view`, which must be `size_px` pixels and of the renderer's format.
    pub fn render(&mut self, scene: &Scene, view: &wgpu::TextureView, size_px: (u32, u32)) {
        self.scratch_quads.clear();
        for command in &scene.commands {
            match command {
                Command::Quad(q) => self.scratch_quads.push(QuadInstance::from_quad(q, scene.scale)),
            }
        }
        self.quads.upload(&self.device, &self.queue, &self.scratch_quads, [size_px.0 as f32, size_px.1 as f32]);

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
            self.quads.draw(&mut pass, 0..self.scratch_quads.len() as u32);
        }
        self.queue.submit([encoder.finish()]);
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
            let line = &mapped[row * padded as usize..row * padded as usize + unpadded as usize];
            for px in line.chunks_exact(4) {
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
