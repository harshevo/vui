//! Pictures: GPU textures with correctly filtered mip chains, kept under a memory budget, and the
//! pipeline that draws them with rounded corners.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use vui_core::scene::{ImageCmd, ImageMode, Sampling};
use vui_core::{ImageId, Images};

/// One picture as the shader reads it. Device pixels, colours premultiplied.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct ImageInstance {
    rect: [f32; 4],
    radii: [f32; 4],
    uv: [f32; 4],
    tint: [f32; 4],
    ring: [f32; 4],
    clip: [f32; 4],
    params: [f32; 4],
    flags: [f32; 4],
}

const NO_CLIP: [f32; 4] = [-1.0e6, -1.0e6, 2.0e6, 2.0e6];

impl ImageInstance {
    pub(crate) fn new(c: &ImageCmd, texture_size: (u32, u32), scale: f32) -> ImageInstance {
        let s = |v: f32| v * scale;
        let [tl, tr, br, bl] = c.radii.to_array();
        let (ring_width, ring) = match c.ring {
            Some(r) => (s(r.width), r.color.premultiplied()),
            None => (0.0, [0.0; 4]),
        };
        ImageInstance {
            rect: [s(c.rect.x), s(c.rect.y), s(c.rect.w), s(c.rect.h)],
            radii: [s(tl), s(tr), s(br), s(bl)],
            uv: [c.uv.x, c.uv.y, c.uv.right(), c.uv.bottom()],
            tint: c.tint.premultiplied(),
            ring,
            clip: c.clip.map_or(NO_CLIP, |r| [s(r.x), s(r.y), s(r.w), s(r.h)]),
            params: [c.opacity, c.smooth, ring_width, 0.0],
            flags: [
                match c.mode {
                    ImageMode::Color => 0.0,
                    ImageMode::Mask => 1.0,
                },
                match c.sampling {
                    Sampling::Smooth => 0.0,
                    Sampling::Sharp => 1.0,
                    Sampling::Nearest => 2.0,
                },
                texture_size.0 as f32,
                texture_size.1 as f32,
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// What the GPU image cache has done so far; mainly for tests and diagnostics.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageStats {
    pub resident: usize,
    pub resident_bytes: usize,
    pub uploads: u64,
    pub evictions: u64,
}

struct GpuImage {
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
    bytes: usize,
    last_used: u64,
}

/// Fills a texture's mip chain on the GPU. Rendering into sRGB views averages in linear light, which
/// is why a minified checkerboard comes out the right grey and a minified photo keeps its brightness.
struct MipGenerator {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl MipGenerator {
    fn new(device: &wgpu::Device) -> MipGenerator {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vui mip shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/mip.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vui mip bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vui mip layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("vui mip pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: TEXTURE_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("vui mip sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        MipGenerator { pipeline, layout, sampler }
    }

    fn generate(&self, device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, levels: u32) {
        if levels <= 1 {
            return;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vui mips") });
        for level in 1..levels {
            let view = |mip| {
                texture.create_view(&wgpu::TextureViewDescriptor { base_mip_level: mip, mip_level_count: Some(1), ..Default::default() })
            };
            let (src, dst) = (view(level - 1), view(level));
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vui mip bind group"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&src) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vui mip pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}

pub(crate) struct ImagePipeline {
    pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    linear: wgpu::Sampler,
    nearest: wgpu::Sampler,
    globals: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: usize,
    bind_group: wgpu::BindGroup,
    mips: MipGenerator,
    resident: HashMap<ImageId, GpuImage>,
    frame: u64,
    resident_bytes: usize,
    pub(crate) budget: usize,
    uploads: u64,
    evictions: u64,
}

const INITIAL_CAPACITY: usize = 256;
const INSTANCE_SIZE: u64 = std::mem::size_of::<ImageInstance>() as u64;
/// The default texture memory the cache keeps resident.
pub(crate) const DEFAULT_BUDGET: usize = 192 * 1024 * 1024;

impl ImagePipeline {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> ImagePipeline {
        let source = format!("{}\n{}", include_str!("shaders/sdf.wgsl"), include_str!("shaders/image.wgsl"));
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vui image shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let buffer_entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vui image bindings"),
            entries: &[
                buffer_entry(
                    0,
                    wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                ),
                buffer_entry(
                    1,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vui image texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vui image layout"),
            bind_group_layouts: &[Some(&bind_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("vui image pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState { color: premultiplied, alpha: premultiplied }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("vui image sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 4,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("vui nearest sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vui image globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (instances, bind_group) = Self::make_instances(device, &bind_layout, &globals, INITIAL_CAPACITY);
        ImagePipeline {
            pipeline,
            bind_layout,
            texture_layout,
            linear,
            nearest,
            globals,
            instances,
            capacity: INITIAL_CAPACITY,
            bind_group,
            mips: MipGenerator::new(device),
            resident: HashMap::new(),
            frame: 0,
            resident_bytes: 0,
            budget: DEFAULT_BUDGET,
            uploads: 0,
            evictions: 0,
        }
    }

    fn make_instances(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, globals: &wgpu::Buffer, capacity: usize) -> (wgpu::Buffer, wgpu::BindGroup) {
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vui image instances"),
            size: capacity as u64 * INSTANCE_SIZE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vui image bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: instances.as_entire_binding() },
            ],
        });
        (instances, bind_group)
    }

    /// Makes sure `id` is on the GPU, uploading it if needed. Returns its texture size, or `None` if
    /// the image is not registered.
    pub(crate) fn ensure(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, images: &Images, id: ImageId) -> Option<(u32, u32)> {
        if images.size(id).is_none() {
            // Removed from the registry: forget any texture still held for it.
            if let Some(g) = self.resident.remove(&id) {
                self.resident_bytes -= g.bytes;
            }
            return None;
        }
        if let Some(g) = self.resident.get_mut(&id) {
            g.last_used = self.frame;
            return Some(g.size);
        }
        let mut data = images.get(id)?;
        let limit = device.limits().max_texture_dimension_2d;
        if data.width.max(data.height) > limit {
            data = data.fit_within(limit);
        }
        let (w, h) = (data.width, data.height);
        let levels = 32 - w.max(h).leading_zeros();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vui image"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let premultiplied;
        let pixels: &[u8] = if data.has_alpha() {
            premultiplied = data.premultiplied_linear();
            &premultiplied
        } else {
            &data.rgba
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.mips.generate(device, queue, &texture, levels);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vui image texture bind group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.linear) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.nearest) },
            ],
        });
        let bytes = (w as usize * h as usize * 4) * 4 / 3;
        self.resident_bytes += bytes;
        self.uploads += 1;
        self.resident.insert(id, GpuImage { bind_group, size: (w, h), bytes, last_used: self.frame });
        Some((w, h))
    }

    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, instances: &[ImageInstance], viewport_px: [f32; 2]) {
        if instances.len() > self.capacity {
            self.capacity = instances.len().next_power_of_two();
            let (buffer, bind_group) = Self::make_instances(device, &self.bind_layout, &self.globals, self.capacity);
            self.instances = buffer;
            self.bind_group = bind_group;
        }
        if !instances.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(instances));
        }
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&Globals { viewport: viewport_px, _pad: [0.0; 2] }));
    }

    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, id: ImageId, instance: u32) {
        let Some(image) = self.resident.get(&id) else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_bind_group(1, &image.bind_group, &[]);
        pass.draw(0..4, instance..instance + 1);
    }

    /// Ends the frame: evicts the least recently used textures while over budget, never ones this
    /// frame drew.
    pub(crate) fn end_frame(&mut self) {
        if self.resident_bytes > self.budget {
            let mut candidates: Vec<(u64, ImageId)> =
                self.resident.iter().filter(|(_, g)| g.last_used < self.frame).map(|(id, g)| (g.last_used, *id)).collect();
            candidates.sort_unstable_by_key(|(last_used, _)| *last_used);
            for (_, id) in candidates {
                if self.resident_bytes <= self.budget {
                    break;
                }
                if let Some(g) = self.resident.remove(&id) {
                    self.resident_bytes -= g.bytes;
                    self.evictions += 1;
                }
            }
        }
        self.frame += 1;
    }

    pub(crate) fn stats(&self) -> ImageStats {
        ImageStats { resident: self.resident.len(), resident_bytes: self.resident_bytes, uploads: self.uploads, evictions: self.evictions }
    }
}
