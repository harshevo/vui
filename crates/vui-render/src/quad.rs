//! The rounded-rectangle pipeline.

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use vui_core::scene::Quad;

/// One quad as the shader reads it. Everything is in device pixels, colours premultiplied.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct QuadInstance {
    rect: [f32; 4],
    fill: [f32; 4],
    border: [f32; 4],
    radii: [f32; 4],
    params: [f32; 4],
    clip: [f32; 4],
}

const NO_CLIP: [f32; 4] = [-1.0e6, -1.0e6, 2.0e6, 2.0e6];

impl QuadInstance {
    pub(crate) fn from_quad(q: &Quad, scale: f32) -> QuadInstance {
        let s = |v: f32| v * scale;
        let (border_width, border) = match q.border {
            Some(b) => (s(b.width), b.color.premultiplied()),
            None => (0.0, [0.0; 4]),
        };
        let [tl, tr, br, bl] = q.radii.to_array();
        QuadInstance {
            rect: [s(q.rect.x), s(q.rect.y), s(q.rect.w), s(q.rect.h)],
            fill: q.fill.premultiplied(),
            border,
            radii: [s(tl), s(tr), s(br), s(bl)],
            params: [border_width, q.smooth, 0.0, 0.0],
            clip: q.clip.map_or(NO_CLIP, |c| [s(c.x), s(c.y), s(c.w), s(c.h)]),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

pub(crate) struct QuadPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: usize,
    bind_group: wgpu::BindGroup,
}

const INITIAL_CAPACITY: usize = 1024;
const INSTANCE_SIZE: u64 = std::mem::size_of::<QuadInstance>() as u64;

impl QuadPipeline {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> QuadPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vui quad shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/quad.wgsl").into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vui quad bindings"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                ),
                entry(
                    1,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vui quad layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("vui quad pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
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
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vui globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (instances, bind_group) = Self::make_instances(device, &bind_layout, &globals, INITIAL_CAPACITY);
        QuadPipeline { pipeline, bind_layout, globals, instances, capacity: INITIAL_CAPACITY, bind_group }
    }

    fn make_instances(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        globals: &wgpu::Buffer,
        capacity: usize,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vui quad instances"),
            size: capacity as u64 * INSTANCE_SIZE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vui quad bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: instances.as_entire_binding() },
            ],
        });
        (instances, bind_group)
    }

    /// Uploads this frame's quads and the viewport size.
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, instances: &[QuadInstance], viewport_px: [f32; 2]) {
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

    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, range: Range<u32>) {
        if range.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..4, range);
    }
}
