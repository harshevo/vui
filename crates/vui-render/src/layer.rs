//! Layers on the GPU: each [`LayerCmd`] is drawn into a window-sized scratch target, the part it
//! keeps is copied into a texture of its own, and the frame composites that texture transformed and
//! faded (`shaders/layer.wgsl`).
//!
//! Drawing into a window-sized target keeps every command in window coordinates, which matters for
//! effects and custom passes whose data holds absolute positions. Nested layers use one scratch
//! target per depth; the textures they are copied into are pooled by size from frame to frame.

use bytemuck::{Pod, Zeroable};
use vui_core::{Affine, LayerCmd, Rect};

/// How deep layers may nest. A layer deeper than this is not drawn.
pub(crate) const MAX_DEPTH: usize = 8;

const NO_CLIP: [f32; 4] = [-1.0e6, -1.0e6, 2.0e6, 2.0e6];

/// The part of the window, in device pixels, a layer's texture holds: its bounds rounded out with a
/// texel to spare for filtering, cut to the window. `None` when that is nothing.
pub(crate) fn region(bounds: Rect, scale: f32, size_px: (u32, u32)) -> Option<[u32; 4]> {
    let (w, h) = (size_px.0 as f32, size_px.1 as f32);
    let x0 = ((bounds.x * scale).floor() - 1.0).clamp(0.0, w);
    let y0 = ((bounds.y * scale).floor() - 1.0).clamp(0.0, h);
    let x1 = (((bounds.x + bounds.w) * scale).ceil() + 1.0).clamp(0.0, w);
    let y1 = (((bounds.y + bounds.h) * scale).ceil() + 1.0).clamp(0.0, h);
    (x1 > x0 && y1 > y0 && bounds.w > 0.0 && bounds.h > 0.0).then_some([x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32])
}

/// The inverse of `transform` in device pixels: from a pixel of the frame to the point of the
/// untransformed window it shows. `None` when the transform squashes the layer flat.
pub(crate) fn device_inverse(transform: &Affine, scale: f32) -> Option<Affine> {
    let inv = transform.inverse()?;
    Some(Affine { e: inv.e * scale, f: inv.f * scale, ..inv })
}

/// One layer as the shader reads it.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct LayerInstance {
    pub rect: [f32; 4],
    pub inv: [f32; 4],
    pub inv_t: [f32; 4],
    pub keep: [f32; 4],
    pub clip: [f32; 4],
}

impl LayerInstance {
    /// `region` is what [`region`] gave: where the layer's texture sits in the window.
    pub(crate) fn new(layer: &LayerCmd, scale: f32, region: [u32; 4]) -> Option<LayerInstance> {
        let inv = device_inverse(&layer.transform, scale)?;
        let drawn = layer.transform.bounds(layer.bounds);
        let drawn = match layer.clip {
            Some(c) => drawn.intersection(c)?,
            None => drawn,
        };
        let s = |v: f32| v * scale;
        let (ox, oy) = (region[0] as f32, region[1] as f32);
        let b = layer.bounds;
        Some(LayerInstance {
            rect: [s(drawn.x).floor(), s(drawn.y).floor(), s(drawn.w).ceil() + 2.0, s(drawn.h).ceil() + 2.0],
            inv: [inv.a, inv.b, inv.c, inv.d],
            inv_t: [inv.e - ox, inv.f - oy, layer.opacity.clamp(0.0, 1.0), 0.0],
            keep: [s(b.x) - ox, s(b.y) - oy, s(b.x + b.w) - ox, s(b.y + b.h) - oy],
            clip: layer.clip.map_or(NO_CLIP, |c| [s(c.x), s(c.y), s(c.w), s(c.h)]),
        })
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

/// A texture one layer was copied into this frame.
struct Target {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
    used: bool,
}

pub(crate) struct LayerPipeline {
    pipeline: wgpu::RenderPipeline,
    globals_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    instances: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    targets: Vec<Target>,
    /// One window-sized target per depth, drawn into and copied out of.
    scratch: Vec<(wgpu::Texture, wgpu::TextureView)>,
    format: wgpu::TextureFormat,
}

impl LayerPipeline {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> LayerPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vui layer shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/layer.wgsl").into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vui layer bindings"),
            entries: &[
                entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                entry(
                    1,
                    wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                ),
            ],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vui layer texture"),
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
            label: Some("vui layer layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let blend = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("vui layer pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState { color: blend, alpha: blend }), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vui layer globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        LayerPipeline { pipeline, globals_layout, texture_layout, globals, instances: None, targets: Vec::new(), scratch: Vec::new(), format }
    }

    /// The window-sized target layers at `depth` are drawn into.
    pub(crate) fn scratch(&mut self, device: &wgpu::Device, depth: usize, size_px: (u32, u32)) -> wgpu::TextureView {
        while self.scratch.len() <= depth {
            self.scratch.push(make_texture(device, self.format, (1, 1), "vui layer scratch"));
        }
        let (texture, _) = &self.scratch[depth];
        if (texture.width(), texture.height()) != size_px {
            self.scratch[depth] = make_texture(device, self.format, size_px, "vui layer scratch");
        }
        self.scratch[depth].1.clone()
    }

    /// Copies `region` of the scratch target at `depth` into a texture of its own, and returns which.
    pub(crate) fn keep(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, depth: usize, region: [u32; 4]) -> usize {
        let size = (region[2], region[3]);
        let index = match self.targets.iter().position(|t| !t.used && t.size == size) {
            Some(i) => i,
            None => {
                let (texture, view) = make_texture(device, self.format, size, "vui layer");
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("vui layer texture"),
                    layout: &self.texture_layout,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
                });
                self.targets.push(Target { texture, bind_group, size, used: false });
                self.targets.len() - 1
            }
        };
        self.targets[index].used = true;
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.scratch[depth].0,
                mip_level: 0,
                origin: wgpu::Origin3d { x: region[0], y: region[1], z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo { texture: &self.targets[index].texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        );
        index
    }

    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, instances: &[LayerInstance], viewport_px: [f32; 2]) {
        if instances.is_empty() {
            return;
        }
        let bytes = bytemuck::cast_slice::<_, u8>(instances);
        if self.instances.as_ref().is_none_or(|(b, _)| b.size() < bytes.len() as u64) {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vui layer instances"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vui layer bindings"),
                layout: &self.globals_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: buffer.as_entire_binding() },
                ],
            });
            self.instances = Some((buffer, bind_group));
        }
        queue.write_buffer(&self.instances.as_ref().expect("made above").0, 0, bytes);
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&Globals { viewport: viewport_px, _pad: [0.0; 2] }));
    }

    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, target: usize, instance: u32) {
        let Some((_, bind_group)) = &self.instances else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_bind_group(1, &self.targets[target].bind_group, &[]);
        pass.draw(0..4, instance..instance + 1);
    }

    /// Lets go of the textures no layer used this frame, and frees the rest for the next.
    pub(crate) fn end_frame(&mut self) {
        self.targets.retain(|t| t.used);
        for t in &mut self.targets {
            t.used = false;
        }
    }
}

fn make_texture(device: &wgpu::Device, format: wgpu::TextureFormat, size: (u32, u32), label: &'static str) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
