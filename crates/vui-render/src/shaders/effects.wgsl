// The built-in effects: soft shadows, a sweeping shimmer and a breathing glow ring, each around a
// rounded box. One instance per command; colours arrive premultiplied in encoded sRGB.

struct Effect {
    area: vec4<f32>,     // where the quad is drawn, device pixels
    clip: vec4<f32>,     // x0 y0 x1 y1, device pixels
    shape: vec4<f32>,    // the rounded box, device pixels
    params: vec4<f32>,   // x: kind, y: radius (px), z: a, w: b
    color: vec4<f32>,
    extra: vec4<f32>,    // x: c, y: time (s), z: scale, w: unused
    viewport: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> effects: array<Effect>;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) px: vec2<f32>,
    @location(1) @interpolate(flat) index: u32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    let e = effects[ii];
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let px = e.area.xy + corner * e.area.zw;
    var out: Out;
    out.pos = vec4<f32>(px.x / e.viewport.x * 2.0 - 1.0, 1.0 - px.y / e.viewport.y * 2.0, 0.0, 1.0);
    out.px = px;
    out.index = ii;
    return out;
}

fn rounded_box(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let rr = min(r, min(half.x, half.y));
    let q = abs(p) - half + rr;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

// Abramowitz and Stegun 7.1.26: good to 1.5e-7, plenty for a shadow.
fn erf(x: f32) -> f32 {
    let s = sign(x);
    let a = abs(x);
    let t = 1.0 / (1.0 + 0.3275911 * a);
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * exp(-a * a);
    return s * y;
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    let e = effects[in.index];
    if in.pos.x < e.clip.x || in.pos.y < e.clip.y || in.pos.x > e.clip.z || in.pos.y > e.clip.w {
        discard;
    }
    let kind = u32(e.params.x + 0.5);
    let scale = e.extra.z;
    let centre = e.shape.xy + e.shape.zw * 0.5;
    let half = e.shape.zw * 0.5;
    let radius = e.params.y;
    let time = e.extra.y;

    if kind == 0u {
        // Shadow: the box blurred by a Gaussian of sigma blur/2, dropped and grown.
        let sigma = max(e.params.z * 0.5 * scale, 0.5);
        let drop = e.params.w * scale;
        let spread = e.extra.x * scale;
        let d = rounded_box(in.px - centre - vec2<f32>(0.0, drop), max(half + spread, vec2<f32>(0.0)), radius + spread);
        let cover = 0.5 - 0.5 * erf(d / (sigma * 1.41421356));
        // Nothing under the surface itself, so translucent surfaces stay clean.
        let inside = clamp(0.5 - rounded_box(in.px - centre, half, radius), 0.0, 1.0);
        return e.color * cover * (1.0 - inside);
    }

    let d = rounded_box(in.px - centre, half, radius);
    if kind == 1u {
        // Shimmer: a soft band crossing the box from left to right, then resting a little.
        let period = e.params.z;
        let band = e.params.w * scale;
        let t = fract(time / period);
        let travel = e.shape.z + band * 4.0;
        let x = e.shape.x - band * 2.0 + t * travel * 1.25;
        let k = (in.px.x - x) / band;
        let along = exp(-k * k);
        let cover = clamp(0.5 - d, 0.0, 1.0);
        return e.color * along * cover;
    }

    // Glow ring: a crisp ring on the edge with a soft falloff outside it, breathing when asked.
    let width = e.params.z * scale;
    let blur = max(e.params.w * scale, 0.5);
    let period = e.extra.x;
    var breathe = 1.0;
    if period > 0.0 {
        breathe = 0.7 + 0.3 * sin(time / period * 6.2831853);
    }
    let ring = clamp(width * 0.5 + 0.5 - abs(d), 0.0, 1.0);
    let out = max(d, 0.0) / blur;
    let halo = exp(-out * out) * 0.45 * step(0.0, d);
    return e.color * clamp(ring + halo, 0.0, 1.0) * breathe;
}
