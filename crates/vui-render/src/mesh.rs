//! The triangle-mesh pipeline: vector shapes, lines and chart marks tessellated by `vui_core::vector`.

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use vui_core::scene::MeshCmd;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct MeshVertex {
    pos: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

const INITIAL: usize = 4096;

pub(crate) struct MeshPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    globals: wgpu::Buffer,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
}

/// What one mesh command needs from the frame: where its indices are and which part of the target it may touch.
pub(crate) struct MeshDraw {
    pub indices: Range<u32>,
    /// `x, y, width, height` in device pixels; `None` for the whole target.
    pub scissor: Option<[u32; 4]>,
}

impl MeshPipeline {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> MeshPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vui mesh shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/mesh.wgsl").into()),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vui mesh bindings"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vui mesh layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let attributes = [
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 8, shader_location: 1 },
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("vui mesh pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
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
            label: Some("vui mesh globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vui mesh bind group"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let (vertices, indices) = (Self::vertex_buffer(device, INITIAL), Self::index_buffer(device, INITIAL));
        MeshPipeline { pipeline, bind_group, globals, vertices, indices, vertex_capacity: INITIAL, index_capacity: INITIAL }
    }

    fn vertex_buffer(device: &wgpu::Device, n: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vui mesh vertices"),
            size: (n * std::mem::size_of::<MeshVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn index_buffer(device: &wgpu::Device, n: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vui mesh indices"),
            size: (n * 4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Appends a command's triangles, scaled to device pixels and faded by its opacity.
    pub(crate) fn add(cmd: &MeshCmd, scale: f32, viewport: [u32; 2], vertices: &mut Vec<MeshVertex>, indices: &mut Vec<u32>) -> Option<MeshDraw> {
        let scissor = match cmd.clip {
            Some(c) => {
                let x0 = ((c.x * scale).floor().max(0.0) as u32).min(viewport[0]);
                let y0 = ((c.y * scale).floor().max(0.0) as u32).min(viewport[1]);
                let x1 = (((c.x + c.w) * scale).ceil().max(0.0) as u32).min(viewport[0]);
                let y1 = (((c.y + c.h) * scale).ceil().max(0.0) as u32).min(viewport[1]);
                if x1 <= x0 || y1 <= y0 {
                    return None;
                }
                Some([x0, y0, x1 - x0, y1 - y0])
            }
            None => None,
        };
        let base = vertices.len() as u32;
        let start = indices.len() as u32;
        let k = cmd.opacity.clamp(0.0, 1.0);
        vertices.extend(cmd.mesh.vertices.iter().map(|v| MeshVertex {
            pos: [v.pos[0] * scale, v.pos[1] * scale],
            color: [v.color[0] * k, v.color[1] * k, v.color[2] * k, v.color[3] * k],
        }));
        indices.extend(cmd.mesh.indices.iter().map(|i| i + base));
        Some(MeshDraw { indices: start..indices.len() as u32, scissor })
    }

    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[MeshVertex], indices: &[u32], viewport_px: [f32; 2]) {
        if vertices.len() > self.vertex_capacity {
            self.vertex_capacity = vertices.len().next_power_of_two();
            self.vertices = Self::vertex_buffer(device, self.vertex_capacity);
        }
        if indices.len() > self.index_capacity {
            self.index_capacity = indices.len().next_power_of_two();
            self.indices = Self::index_buffer(device, self.index_capacity);
        }
        if !vertices.is_empty() {
            queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(vertices));
            queue.write_buffer(&self.indices, 0, bytemuck::cast_slice(indices));
        }
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&Globals { viewport: viewport_px, _pad: [0.0; 2] }));
    }

    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, draw: &MeshDraw, viewport: [u32; 2]) {
        if draw.indices.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        if let Some([x, y, w, h]) = draw.scissor {
            pass.set_scissor_rect(x, y, w, h);
        }
        pass.draw_indexed(draw.indices.clone(), 0, 0..1);
        if draw.scissor.is_some() {
            pass.set_scissor_rect(0, 0, viewport[0], viewport[1]);
        }
    }
}
