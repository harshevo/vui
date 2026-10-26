// Rounded rectangles drawn as signed-distance fields.
//
// Every control is made of these: a fill, an optional inner border, per-corner radii, continuous
// ("squircle") corners and a clip rect. One instanced draw covers a whole run of quads. Colours are
// sRGB-encoded and premultiplied, and blending happens in that encoded space on purpose, so the design
// token values are the values on screen.

struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

struct Quad {
    rect: vec4<f32>,    // x, y, w, h in device pixels
    fill: vec4<f32>,    // premultiplied
    border: vec4<f32>,  // premultiplied
    radii: vec4<f32>,   // top-left, top-right, bottom-right, bottom-left, device pixels
    params: vec4<f32>,  // x: border width, y: corner smoothness 0..1
    clip: vec4<f32>,    // x, y, w, h in device pixels
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> quads: array<Quad>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) px: vec2<f32>,
    @location(1) @interpolate(flat) idx: u32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let q = quads[ii];
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    // One pixel of margin so the antialiased edge is not cut off by the quad's own bounds.
    let pad = 1.0;
    let p = q.rect.xy - vec2<f32>(pad) + corner * (q.rect.zw + vec2<f32>(2.0 * pad));
    var o: VsOut;
    o.pos = vec4<f32>(p.x / g.viewport.x * 2.0 - 1.0, 1.0 - p.y / g.viewport.y * 2.0, 0.0, 1.0);
    o.px = p;
    o.idx = ii;
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let q = quads[in.idx];
    let half = q.rect.zw * 0.5;
    let p = in.px - (q.rect.xy + half);
    let smooth_k = q.params.y;
    let bw = q.params.x;

    let box_outer = sd_box(p, half, q.radii, smooth_k);
    let cov_outer = coverage(box_outer.x, box_outer.y);

    var cov_inner = cov_outer;
    if (bw > 0.0) {
        let inner_half = max(half - vec2<f32>(bw), vec2<f32>(0.0));
        let inner_radii = max(q.radii - vec4<f32>(bw), vec4<f32>(0.0));
        let box_inner = sd_box(p, inner_half, inner_radii, smooth_k);
        cov_inner = coverage(box_inner.x, box_inner.y);
    }

    var col = q.fill * cov_inner;
    if (bw > 0.0) {
        col = col + q.border * (cov_outer - cov_inner);
    }

    // Soft clip on the clip rect's edges.
    let cx = clamp(min(in.px.x - q.clip.x, q.clip.x + q.clip.z - in.px.x) + 0.5, 0.0, 1.0);
    let cy = clamp(min(in.px.y - q.clip.y, q.clip.y + q.clip.w - in.px.y) + 0.5, 0.0, 1.0);
    return col * (cx * cy);
}
