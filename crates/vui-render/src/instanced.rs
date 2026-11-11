//! A built-in pass that draws one quad per command, reading each command's floats from a storage
//! buffer: the effects pass and the shading pass are both this, with their own shader and layout.

use crate::custom::CustomPass;
use vui_core::CustomCmd;

/// Lays out one command's floats, in device pixels, appending them to the buffer.
pub(crate) type Layout = fn(&CustomCmd, [f32; 2], f32, &mut Vec<f32>);

pub(crate) struct InstancedPass {
    label: &'static str,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    buffer: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
    floats: Layout,
}

impl InstancedPass {
    /// `wgsl` must have entry points `vs` and `fs` and read its instances from a read-only storage
    /// buffer at group 0, binding 0. Output is premultiplied and blended source-over.
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat, label: &'static str, wgsl: &str, floats: Layout) -> InstancedPass {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(wgsl.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(label), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let blend = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
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
        InstancedPass { label, pipeline, layout, buffer: None, bind_group: None, floats }
    }
}

impl CustomPass for InstancedPass {
    fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, viewport: [f32; 2], scale: f32, commands: &[&CustomCmd]) {
        if commands.is_empty() {
            return;
        }
        let mut floats = Vec::new();
        for c in commands {
            (self.floats)(c, viewport, scale, &mut floats);
        }
        let bytes: Vec<u8> = floats.into_iter().flat_map(f32::to_ne_bytes).collect();
        if self.buffer.as_ref().is_none_or(|b| b.size() < bytes.len() as u64) {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(self.label),
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
