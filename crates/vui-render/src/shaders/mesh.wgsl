// Triangles with a colour per vertex: vector shapes, lines and chart marks. The CPU has already
// tessellated them in device pixels with a transparent fringe for soft edges, so the shader only moves
// them into clip space and passes the premultiplied colour through.

struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var o: VsOut;
    o.pos = vec4<f32>(in.pos.x / g.viewport.x * 2.0 - 1.0, 1.0 - in.pos.y / g.viewport.y * 2.0, 0.0, 1.0);
    o.color = in.color;
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
