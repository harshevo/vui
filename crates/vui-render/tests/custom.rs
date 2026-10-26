//! Custom shader passes: an app's own pipeline drawing in its place among the stock ones.

use vui_core::{Color, CustomCmd, Quad, Resources, Scene, rect, size};
use vui_render::CustomPass;
use vui_render::testing::renderer_for;

/// Fills each command's rect with the colour in its data: the smallest useful pass.
struct Solid {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    bind_group: Option<wgpu::BindGroup>,
}

const SHADER: &str = r#"
// Per command: rect in device pixels, premultiplied colour, viewport.
struct Draw { rect: vec4<f32>, color: vec4<f32>, viewport: vec4<f32> };
@group(0) @binding(0) var<storage, read> draws: array<Draw>;

struct Out { @builtin(position) pos: vec4<f32>, @location(0) color: vec4<f32> };

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    let d = draws[ii];
    let c = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let px = d.rect.xy + c * d.rect.zw;
    var out: Out;
    out.pos = vec4<f32>(px.x / d.viewport.x * 2.0 - 1.0, 1.0 - px.y / d.viewport.y * 2.0, 0.0, 1.0);
    out.color = d.color;
    return out;
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

impl Solid {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Solid {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState { color: blend, alpha: blend }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Solid { pipeline, layout, bind_group: None }
    }
}

impl CustomPass for Solid {
    fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, viewport: [f32; 2], scale: f32, commands: &[&CustomCmd]) {
        let draws: Vec<[f32; 12]> = commands
            .iter()
            .map(|c| {
                let (r, k) = (c.rect, c.opacity);
                let d = &c.data;
                [r.x * scale, r.y * scale, r.w * scale, r.h * scale, d[0] * k, d[1] * k, d[2] * k, d[3] * k, viewport[0], viewport[1], 0.0, 0.0]
            })
            .collect();
        if draws.is_empty() {
            self.bind_group = None;
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&draws);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buffer, 0, bytes);
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
        }));
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, index: usize) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, self.bind_group.as_ref(), &[]);
        pass.draw(0..4, index as u32..index as u32 + 1);
    }
}

fn renderer() -> vui_render::Renderer {
    let mut r = renderer_for(&Resources::bundled_only());
    let solid = Solid::new(r.device(), r.format());
    r.add_pass("solid", Box::new(solid));
    r
}

#[test]
fn a_custom_pass_draws_its_commands() {
    let mut scene = Scene::new(size(40.0, 20.0), 1.0).with_clear(Color::WHITE);
    scene.push(vui_core::Command::Custom(CustomCmd::new("solid", rect(0.0, 0.0, 20.0, 20.0), vec![1.0, 0.0, 0.0, 1.0])));
    let image = renderer().render_to_image(&scene).unwrap();
    assert_eq!(image.pixel(5, 10), [255, 0, 0, 255]);
    assert_eq!(image.pixel(30, 10), [255, 255, 255, 255], "only inside its rect");
}

#[test]
fn custom_drawing_keeps_its_place_among_quads() {
    let mut scene = Scene::new(size(40.0, 20.0), 2.0).with_clear(Color::WHITE);
    scene.quad(Quad::new(rect(0.0, 0.0, 40.0, 20.0)).fill(Color::BLACK));
    scene.push(vui_core::Command::Custom(CustomCmd::new("solid", rect(0.0, 0.0, 20.0, 20.0), vec![0.0, 0.0, 1.0, 1.0])));
    scene.quad(Quad::new(rect(10.0, 0.0, 30.0, 20.0)).fill(Color::rgba(0.0, 1.0, 0.0, 1.0)));
    let image = renderer().render_to_image(&scene).unwrap();
    assert_eq!(image.pixel(10, 20), [0, 0, 255, 255], "over the first quad, scaled to device pixels");
    assert_eq!(image.pixel(30, 20), [0, 255, 0, 255], "under the second");
}

#[test]
fn commands_for_a_missing_pass_are_skipped_and_opacity_reaches_the_pass() {
    let mut scene = Scene::new(size(20.0, 20.0), 1.0).with_clear(Color::WHITE);
    scene.push(vui_core::Command::Custom(CustomCmd::new("nobody", rect(0.0, 0.0, 20.0, 20.0), vec![])));
    let mut faded = CustomCmd::new("solid", rect(0.0, 0.0, 20.0, 20.0), vec![0.0, 0.0, 0.0, 1.0]);
    faded.opacity = 0.5;
    scene.push(vui_core::Command::Custom(faded));
    let image = renderer().render_to_image(&scene).unwrap();
    let p = image.pixel(10, 10);
    assert!((126..=129).contains(&p[0]), "half black over white: {p:?}");
}
