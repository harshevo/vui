//! The renderer's built-in effects pass: shadows, shimmer and glow, described in
//! [`vui_core::effects`]. Every renderer has it, so widgets can use it without the app adding it.

use crate::custom::CustomPass;
use vui_core::CustomCmd;

const FLOATS: usize = 28;

pub struct EffectsPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    buffer: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
}

impl EffectsPass {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> EffectsPass {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("effects"), source: wgpu::ShaderSource::Wgsl(include_str!("shaders/effects.wgsl").into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("effects"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("effects"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let blend = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("effects"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState { color: blend, alpha: blend }), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        EffectsPass { pipeline, layout, buffer: None, bind_group: None }
    }
}

/// One command's slot in the storage buffer, in device pixels with premultiplied colour.
pub(crate) fn floats(c: &CustomCmd, viewport: [f32; 2], scale: f32) -> [f32; FLOATS] {
    let d = |i: usize| c.data.get(i).copied().unwrap_or(0.0);
    let r = c.rect;
    let clip = c.clip.map_or([0.0, 0.0, f32::MAX, f32::MAX], |k| [k.x * scale, k.y * scale, (k.x + k.w) * scale, (k.y + k.h) * scale]);
    let a = (d(9) * c.opacity).clamp(0.0, 1.0);
    [
        r.x * scale, r.y * scale, r.w * scale, r.h * scale,
        clip[0], clip[1], clip[2], clip[3],
        d(1) * scale, d(2) * scale, d(3) * scale, d(4) * scale,
        d(0), d(5) * scale, d(10), d(11),
        d(6) * a, d(7) * a, d(8) * a, a,
        d(12), d(13), scale, 0.0,
        viewport[0], viewport[1], 0.0, 0.0,
    ]
}

impl CustomPass for EffectsPass {
    fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, viewport: [f32; 2], scale: f32, commands: &[&CustomCmd]) {
        if commands.is_empty() {
            return;
        }
        let bytes: Vec<u8> = commands.iter().flat_map(|c| floats(c, viewport, scale)).flat_map(f32::to_ne_bytes).collect();
        if self.buffer.as_ref().is_none_or(|b| b.size() < bytes.len() as u64) {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("effects"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("effects"),
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            }));
            self.buffer = Some(buffer);
        }
        queue.write_buffer(self.buffer.as_ref().expect("made above"), 0, &bytes);
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, index: usize) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, self.bind_group.as_ref(), &[]);
        pass.draw(0..4, index as u32..index as u32 + 1);
    }
}
