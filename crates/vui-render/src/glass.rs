//! The glass pass: what [`vui_core::glass`] describes, drawn by blurring what the frame already holds
//! behind each glass shape.
//!
//! A frame with glass in it is drawn into a texture of the renderer's own, in runs: everything before
//! a glass command, then the glass, then everything after, so each glass shows what was painted
//! behind it. For each one the part of the frame under it is shrunk by up to 8× with a box filter,
//! blurred with a Gaussian across and then down, and drawn back through the shape: bent along the
//! edge, colour-split there, saturated, tinted, lit on the rim and with a soft highlight. Then the
//! finished frame is copied to the window. Cost follows the area under the glass.
//!
//! [`Params`] is the one description both renderers draw from; the CPU twin (`cpu/glass.rs`) runs
//! the same steps in the same order, rounding to 8 bits where the GPU stores to a texture.

use bytemuck::{Pod, Zeroable};
use vui_core::CustomCmd;
use vui_core::glass::Shape;

/// The widest Gaussian, in taps either side of the centre.
pub(crate) const MAX_TAPS: u32 = 24;

/// One glass command in device pixels, as the shader reads it.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
pub(crate) struct Params {
    /// The part of the frame read, `x0 y0 x1 y1`, whole pixels inside the frame.
    pub region: [f32; 4],
    /// Shrink factor, the shrunk size `w h`, and taps either side.
    pub down: [f32; 4],
    /// The Gaussian's sigma in shrunk pixels.
    pub blur: [f32; 4],
    /// The first box: centre and half size.
    pub a: [f32; 4],
    /// Corner radii of the two boxes, the union's reach, and the scale.
    pub radii: [f32; 4],
    pub b: [f32; 4],
    /// Straight tint and how much of it covers.
    pub tint: [f32; 4],
    /// Saturation, refraction (px), dispersion, rim.
    pub look: [f32; 4],
    /// Specular, grain, the command's opacity, and 1 when the backdrop could be read.
    pub look2: [f32; 4],
    pub clip: [f32; 4],
    /// The quad the glass is drawn in, `x y w h`.
    pub area: [f32; 4],
    /// The highlight's centre and radius.
    pub spec: [f32; 4],
    pub viewport: [f32; 4],
}

impl Params {
    /// Where `cmd` reads and draws in a frame of `size` pixels. `None` when it draws nothing there.
    pub(crate) fn new(cmd: &CustomCmd, scale: f32, size: (u32, u32), backdrop: bool) -> Option<Params> {
        let s = Shape::read(cmd)?;
        let g = s.glass;
        let (w, h) = (size.0 as f32, size.1 as f32);
        let bounds = cmd.rect;
        let area = [(bounds.x * scale).floor() - 1.0, (bounds.y * scale).floor() - 1.0, (bounds.w * scale).ceil() + 2.0, (bounds.h * scale).ceil() + 2.0];
        let reach = s.reach * scale;
        let x0 = (area[0] - reach).floor().clamp(0.0, w);
        let y0 = (area[1] - reach).floor().clamp(0.0, h);
        let x1 = (area[0] + area[2] + reach).ceil().clamp(0.0, w);
        let y1 = (area[1] + area[3] + reach).ceil().clamp(0.0, h);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let sigma = g.blur * scale * 0.5;
        let f = (sigma / 3.0).floor().clamp(1.0, 8.0);
        let sigma_d = sigma / f;
        let taps = if sigma_d < 0.05 { 0.0 } else { (sigma_d * 3.0).ceil().min(MAX_TAPS as f32) };
        let dw = ((x1 - x0) / f).ceil();
        let dh = ((y1 - y0) / f).ceil();
        let half = |r: vui_core::Rect| [(r.x + r.w * 0.5) * scale, (r.y + r.h * 0.5) * scale, r.w * 0.5 * scale, r.h * 0.5 * scale];
        let both = s.a.union(s.b);
        let clip = cmd.clip.map_or([-1.0e6, -1.0e6, 1.0e7, 1.0e7], |k| [k.x * scale, k.y * scale, (k.x + k.w) * scale, (k.y + k.h) * scale]);
        Some(Params {
            region: [x0, y0, x1, y1],
            down: [f, dw, dh, taps],
            blur: [sigma_d, 0.0, 0.0, 0.0],
            a: half(s.a),
            radii: [s.ra * scale, s.rb * scale, s.k * scale, scale],
            b: half(s.b),
            tint: [g.tint.r, g.tint.g, g.tint.b, (g.opacity * g.tint.a).clamp(0.0, 1.0)],
            look: [g.saturation, g.refraction * scale, g.dispersion.clamp(0.0, 1.0), g.rim.max(0.0)],
            look2: [g.specular.max(0.0), g.grain.max(0.0), cmd.opacity.clamp(0.0, 1.0), if backdrop { 1.0 } else { 0.0 }],
            clip,
            area,
            spec: [(both.x + both.w * 0.3) * scale, (both.y + both.h * 0.18) * scale, both.w.min(both.h) * 0.45 * scale, 0.0],
            viewport: [w, h, 0.0, 0.0],
        })
    }

    pub(crate) fn shrunk(&self) -> (u32, u32) {
        (self.down[1] as u32, self.down[2] as u32)
    }
}

/// The weights of the Gaussian, centre first, not yet normalised.
pub(crate) fn weight(k: f32, sigma: f32) -> f32 {
    (-(k * k) / (2.0 * sigma * sigma)).exp()
}

/// A rounded box's signed distance at `p`, relative to its centre.
pub(crate) fn rounded_box(p: [f32; 2], half: [f32; 2], r: f32) -> f32 {
    let rr = r.min(half[0].min(half[1])).max(0.0);
    let q = [p[0].abs() - half[0] + rr, p[1].abs() - half[1] + rr];
    let out = [q[0].max(0.0), q[1].max(0.0)];
    (out[0] * out[0] + out[1] * out[1]).sqrt() + q[0].max(q[1]).min(0.0) - rr
}

/// The glass shape's signed distance at device point `p`.
pub(crate) fn distance(p: [f32; 2], pr: &Params) -> f32 {
    let da = rounded_box([p[0] - pr.a[0], p[1] - pr.a[1]], [pr.a[2], pr.a[3]], pr.radii[0]);
    let db = rounded_box([p[0] - pr.b[0], p[1] - pr.b[1]], [pr.b[2], pr.b[3]], pr.radii[1]);
    vui_core::glass::smooth_min(da, db, pr.radii[2])
}

/// A hash of a pixel, 0–1, the same on the GPU.
pub(crate) fn grain(x: u32, y: u32) -> f32 {
    let mut v = x.wrapping_mul(747_796_405).wrapping_add(y.wrapping_mul(2_891_336_453)).wrapping_add(1);
    v = ((v >> ((v >> 28) + 4)) ^ v).wrapping_mul(277_803_737);
    v = (v >> 22) ^ v;
    (v >> 8) as f32 / 16_777_215.0
}

/// Shades one pixel of glass from the blurred backdrop: `sample(x, y)` reads it at a device point.
/// Returns premultiplied colour.
pub(crate) fn shade(px: [f32; 2], pr: &Params, sample: impl Fn([f32; 2]) -> [f32; 4]) -> Option<[f32; 4]> {
    if px[0] < pr.clip[0] || px[1] < pr.clip[1] || px[0] > pr.clip[2] || px[1] > pr.clip[3] {
        return None;
    }
    let d = distance(px, pr);
    let cover = (0.5 - d).clamp(0.0, 1.0) * pr.look2[2];
    if cover <= 0.0 {
        return None;
    }
    let gx = distance([px[0] + 1.0, px[1]], pr) - distance([px[0] - 1.0, px[1]], pr);
    let gy = distance([px[0], px[1] + 1.0], pr) - distance([px[0], px[1] - 1.0], pr);
    let len = (gx * gx + gy * gy).sqrt();
    let n = if len > 1.0e-6 { [gx / len, gy / len] } else { [0.0, 0.0] };
    let refraction = pr.look[1];
    let band = refraction.abs().max(1.0);
    let t = (-d / band).clamp(0.0, 1.0);
    let bend = (1.0 - t) * (1.0 - t);
    let off = [-n[0] * refraction * bend, -n[1] * refraction * bend];
    let disp = pr.look[2];
    let at = |k: f32| sample([px[0] + off[0] * k, px[1] + off[1] * k]);
    let mid = at(1.0);
    let mut c = if pr.look2[3] > 0.5 { [at(1.0 + disp)[0], mid[1], at(1.0 - disp)[2]] } else { [0.0; 3] };

    let luma = c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722;
    let sat = pr.look[0];
    for v in &mut c {
        *v = (luma + (*v - luma) * sat).clamp(0.0, 1.0);
    }
    let ta = pr.tint[3];
    for (i, v) in c.iter_mut().enumerate() {
        *v = *v * (1.0 - ta) + pr.tint[i] * ta;
    }
    let scale = pr.radii[3];
    let rt = (-d / (1.5 * scale)).clamp(0.0, 1.0);
    let fresnel = (1.0 - rt) * (1.0 - rt);
    let light = 0.65 - 0.35 * n[1];
    let rim = (pr.look[3] * fresnel * light * 0.6).clamp(0.0, 1.0);
    let dx = (px[0] - pr.spec[0]) / pr.spec[2].max(1.0);
    let dy = (px[1] - pr.spec[1]) / pr.spec[2].max(1.0);
    let spec = (pr.look2[0] * (-(dx * dx + dy * dy)).exp() * 0.35).clamp(0.0, 1.0);
    let noise = (grain(px[0] as u32, px[1] as u32) - 0.5) * pr.look2[1];
    for v in &mut c {
        *v += (1.0 - *v) * rim;
        *v += (1.0 - *v) * spec;
        *v = (*v + noise).clamp(0.0, 1.0);
    }
    Some([c[0] * cover, c[1] * cover, c[2] * cover, cover])
}

/// Bilinear read of a shrunk, blurred backdrop held as 8-bit texels, at device point `p`.
pub(crate) fn bilinear(p: [f32; 2], pr: &Params, texel: impl Fn(i32, i32) -> [f32; 4]) -> [f32; 4] {
    let f = pr.down[0];
    let (dw, dh) = (pr.down[1] as i32, pr.down[2] as i32);
    let q = [(p[0] - pr.region[0]) / f - 0.5, (p[1] - pr.region[1]) / f - 0.5];
    let i0 = [q[0].floor(), q[1].floor()];
    let fr = [q[0] - i0[0], q[1] - i0[1]];
    let at = |x: i32, y: i32| texel(x.clamp(0, dw - 1), y.clamp(0, dh - 1));
    let (x, y) = (i0[0] as i32, i0[1] as i32);
    let (c00, c10, c01, c11) = (at(x, y), at(x + 1, y), at(x, y + 1), at(x + 1, y + 1));
    let mut out = [0.0; 4];
    for i in 0..4 {
        let top = c00[i] + (c10[i] - c00[i]) * fr[0];
        let bottom = c01[i] + (c11[i] - c01[i]) * fr[0];
        out[i] = top + (bottom - top) * fr[1];
    }
    out
}

fn texture(device: &wgpu::Device, format: wgpu::TextureFormat, size: (u32, u32), layout: &wgpu::BindGroupLayout, label: &'static str) -> Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
    });
    Texture { size: (size.0.max(1), size.1.max(1)), view, bind_group }
}

struct Texture {
    size: (u32, u32),
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

/// The GPU side: the frame textures glass reads from, two textures to blur in, and the pipelines.
pub(crate) struct GlassPipeline {
    shrink: wgpu::RenderPipeline,
    across: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    copy: wgpu::RenderPipeline,
    copy_over: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    params_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    params: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    /// One window-sized frame per depth.
    frames: Vec<Option<Texture>>,
    /// The shrunk backdrop, and the texture the first blur step writes.
    ping: Option<(Texture, Texture)>,
    format: wgpu::TextureFormat,
}

impl GlassPipeline {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> GlassPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vui glass shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/glass.wgsl").into()),
        });
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vui glass params"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vui glass texture"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vui glass layout"),
            bind_group_layouts: &[Some(&params_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let premultiplied = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add };
        let make = |vs: &str, fs: &str, blend: Option<wgpu::BlendState>, topology: wgpu::PrimitiveTopology| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("vui glass pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some(vs), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let list = wgpu::PrimitiveTopology::TriangleList;
        GlassPipeline {
            shrink: make("vs_full", "fs_shrink", None, list),
            across: make("vs_full", "fs_across", None, list),
            down: make("vs_full", "fs_down", None, list),
            copy: make("vs_full", "fs_copy", None, list),
            copy_over: make("vs_full", "fs_copy", Some(wgpu::BlendState { color: premultiplied, alpha: premultiplied }), list),
            glass: make("vs_glass", "fs_glass", Some(wgpu::BlendState { color: premultiplied, alpha: premultiplied }), wgpu::PrimitiveTopology::TriangleStrip),
            params_layout,
            texture_layout,
            params: None,
            frames: Vec::new(),
            ping: None,
            format,
        }
    }

    /// Uploads this frame's glass and makes sure its textures are big enough. Returns the view of
    /// the frame texture for `depth`, which the frame is drawn into instead of the window.
    pub(crate) fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, params: &[Params], size_px: (u32, u32), depth: usize) -> wgpu::TextureView {
        let bytes = bytemuck::cast_slice::<_, u8>(params);
        if self.params.as_ref().is_none_or(|(b, _)| b.size() < bytes.len() as u64) {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vui glass params"),
                size: (bytes.len() as u64).next_power_of_two().max(256),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vui glass params"),
                layout: &self.params_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            });
            self.params = Some((buffer, bind_group));
        }
        queue.write_buffer(&self.params.as_ref().expect("made above").0, 0, bytes);

        let need = params.iter().fold((1, 1), |(w, h), p| {
            let (pw, ph) = p.shrunk();
            (w.max(pw), h.max(ph))
        });
        if self.ping.as_ref().is_none_or(|(a, _)| a.size.0 < need.0 || a.size.1 < need.1) {
            let have = self.ping.as_ref().map_or((0, 0), |(a, _)| a.size);
            let size = (need.0.max(have.0), need.1.max(have.1));
            self.ping = Some((
                texture(device, self.format, size, &self.texture_layout, "vui glass backdrop"),
                texture(device, self.format, size, &self.texture_layout, "vui glass blur"),
            ));
        }
        while self.frames.len() <= depth {
            self.frames.push(None);
        }
        if self.frames[depth].as_ref().is_none_or(|f| f.size != size_px) {
            self.frames[depth] = Some(texture(device, self.format, size_px, &self.texture_layout, "vui glass frame"));
        }
        self.frames[depth].as_ref().expect("made above").view.clone()
    }

    /// Records the shrink and the two blur steps for glass `index`, reading the frame for `depth`.
    pub(crate) fn blur(&self, encoder: &mut wgpu::CommandEncoder, p: &Params, index: u32, depth: usize) {
        let (Some((_, params)), Some((a, b)), Some(Some(frame))) = (&self.params, &self.ping, self.frames.get(depth)) else { return };
        let (w, h) = p.shrunk();
        let taps = p.down[3] > 0.0;
        let steps: &[(&wgpu::RenderPipeline, &Texture, &Texture)] =
            if taps { &[(&self.shrink, frame, a), (&self.across, a, b), (&self.down, b, a)] } else { &[(&self.shrink, frame, a)] };
        for (pipeline, from, to) in steps {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vui glass blur"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &to.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(0.0, 0.0, w as f32, h as f32, 0.0, 1.0);
            pass.set_scissor_rect(0, 0, w, h);
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, params, &[]);
            pass.set_bind_group(1, &from.bind_group, &[]);
            pass.draw(0..3, index..index + 1);
        }
    }

    /// Draws glass `index` from its blurred backdrop, in the frame's pass.
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, index: u32) {
        let (Some((_, params)), Some((a, _))) = (&self.params, &self.ping) else { return };
        pass.set_pipeline(&self.glass);
        pass.set_bind_group(0, params, &[]);
        pass.set_bind_group(1, &a.bind_group, &[]);
        pass.draw(0..4, index..index + 1);
    }

    /// Copies the finished frame for `depth` to `view`.
    pub(crate) fn copy(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, depth: usize) {
        self.blit(encoder, view, depth, &self.copy);
    }

    /// Draws the finished frame for `depth` over what `view` holds.
    pub(crate) fn copy_over(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, depth: usize) {
        self.blit(encoder, view, depth, &self.copy_over);
    }

    fn blit(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, depth: usize, pipeline: &wgpu::RenderPipeline) {
        let (Some((_, params)), Some(Some(frame))) = (&self.params, self.frames.get(depth)) else { return };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("vui glass copy"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, params, &[]);
        pass.set_bind_group(1, &frame.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
