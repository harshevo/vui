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

// Signed distance to a rounded box centred on the origin. With smooth > 0 the corner uses a
// superellipse norm instead of a circle, which is what makes continuous corners look continuous.
fn sd_box(p: vec2<f32>, half: vec2<f32>, radii: vec4<f32>, smooth_k: f32) -> f32 {
    var r: f32;
    if (p.x < 0.0) {
        if (p.y < 0.0) { r = radii.x; } else { r = radii.w; }
    } else {
        if (p.y < 0.0) { r = radii.y; } else { r = radii.z; }
    }
    r = min(r, min(half.x, half.y));
    let q = abs(p) - half + vec2<f32>(r);
    let inside = min(max(q.x, q.y), 0.0);
    let qp = max(q, vec2<f32>(0.0));
    var corner: f32;
    if (smooth_k <= 0.001) {
        corner = length(qp);
    } else {
        let n = mix(2.0, 5.0, smooth_k);
        corner = pow(pow(qp.x, n) + pow(qp.y, n), 1.0 / n);
    }
    return inside + corner - r;
}

// Pixel coverage from a distance. Dividing by the gradient length keeps the edge one pixel wide
// even where the field is not a true Euclidean distance.
fn coverage(d: f32, grad: f32) -> f32 {
    return clamp(0.5 - d / max(grad, 1e-4), 0.0, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let q = quads[in.idx];
    let half = q.rect.zw * 0.5;
    let p = in.px - (q.rect.xy + half);
    let smooth_k = q.params.y;
    let bw = q.params.x;

    let d_outer = sd_box(p, half, q.radii, smooth_k);
    let grad_outer = length(vec2<f32>(dpdx(d_outer), dpdy(d_outer)));
    let cov_outer = coverage(d_outer, grad_outer);

    var cov_inner = cov_outer;
    if (bw > 0.0) {
        let inner_half = max(half - vec2<f32>(bw), vec2<f32>(0.0));
        let inner_radii = max(q.radii - vec4<f32>(bw), vec4<f32>(0.0));
        let d_inner = sd_box(p, inner_half, inner_radii, smooth_k);
        let grad_inner = length(vec2<f32>(dpdx(d_inner), dpdy(d_inner)));
        cov_inner = coverage(d_inner, grad_inner);
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
