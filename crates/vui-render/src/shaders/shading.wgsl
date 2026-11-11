// Shaded fills: linear, radial and conic gradients (filled or as a border), an aurora of drifting
// colour, a beam of light running round an edge, a spotlight and film grain. One instance per
// command. Stops arrive in Oklab premultiplied by alpha; output is premultiplied encoded sRGB.
// `cpu/shading.rs` draws the same thing on the CPU: keep the two in step.

struct Shade {
    area: vec4<f32>,     // where the quad is drawn, device pixels
    clip: vec4<f32>,     // x0 y0 x1 y1, device pixels
    shape: vec4<f32>,    // the rounded box, device pixels
    head: vec4<f32>,     // kind, radius (px), time (s), scale
    params: vec4<f32>,   // per kind; see vui_core::shading
    c0: vec4<f32>,
    c1: vec4<f32>,
    c2: vec4<f32>,
    c3: vec4<f32>,
    pos: vec4<f32>,      // stop positions
    misc: vec4<f32>,     // stop count, ring width (px), viewport w, h
};

@group(0) @binding(0) var<storage, read> shades: array<Shade>;

const TAU: f32 = 6.2831853;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) px: vec2<f32>,
    @location(1) @interpolate(flat) index: u32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    let e = shades[ii];
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let px = e.area.xy + corner * e.area.zw;
    var out: Out;
    out.pos = vec4<f32>(px.x / e.misc.z * 2.0 - 1.0, 1.0 - px.y / e.misc.w * 2.0, 0.0, 1.0);
    out.px = px;
    out.index = ii;
    return out;
}

fn rounded_box(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let rr = min(r, min(half.x, half.y));
    let q = abs(p) - half + rr;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

// The colour at `t` along the stops, still in premultiplied Oklab.
fn ramp(e: Shade, t: f32) -> vec4<f32> {
    var cs = array<vec4<f32>, 4>(e.c0, e.c1, e.c2, e.c3);
    var ps = array<f32, 4>(e.pos.x, e.pos.y, e.pos.z, e.pos.w);
    let n = i32(e.misc.x + 0.5);
    if n <= 0 {
        return vec4<f32>(0.0);
    }
    if n == 1 || t <= ps[0] {
        return cs[0];
    }
    for (var i = 0; i < n - 1; i++) {
        if t <= ps[i + 1] {
            let span = ps[i + 1] - ps[i];
            var f = 0.0;
            if span > 1e-5 {
                f = clamp((t - ps[i]) / span, 0.0, 1.0);
            }
            return mix(cs[i], cs[i + 1], f);
        }
    }
    return cs[n - 1];
}

fn oklab_to_linear(lab: vec3<f32>) -> vec3<f32> {
    let l = lab.x + 0.39633778 * lab.y + 0.21580376 * lab.z;
    let m = lab.x - 0.105561346 * lab.y - 0.06385417 * lab.z;
    let s = lab.x - 0.08948418 * lab.y - 1.2914855 * lab.z;
    let l3 = l * l * l;
    let m3 = m * m * m;
    let s3 = s * s * s;
    return vec3<f32>(
        4.0767417 * l3 - 3.3077116 * m3 + 0.23096994 * s3,
        -1.268438 * l3 + 2.6097574 * m3 - 0.34131938 * s3,
        -0.0041960863 * l3 - 0.7034186 * m3 + 1.7076147 * s3,
    );
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3<f32>(0.0031308));
}

// Premultiplied Oklab to premultiplied encoded sRGB.
fn encode(lab: vec4<f32>) -> vec4<f32> {
    if lab.w <= 0.0 {
        return vec4<f32>(0.0);
    }
    let rgb = to_srgb(oklab_to_linear(lab.xyz / lab.w));
    return vec4<f32>(rgb * lab.w, lab.w);
}

fn hash(p: vec2<u32>) -> f32 {
    var v = p.x * 747796405u + p.y * 2891336453u;
    v = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
    v = (v >> 22u) ^ v;
    return f32(v) / 4294967295.0;
}

fn smooth01(x: f32) -> f32 {
    let s = clamp(x, 0.0, 1.0);
    return s * s * (3.0 - 2.0 * s);
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    let e = shades[in.index];
    if in.pos.x < e.clip.x || in.pos.y < e.clip.y || in.pos.x > e.clip.z || in.pos.y > e.clip.w {
        discard;
    }
    let kind = u32(e.head.x + 0.5);
    let radius = e.head.y;
    let time = e.head.z;
    let scale = e.head.w;
    let half = e.shape.zw * 0.5;
    let centre = e.shape.xy + half;
    let p = in.px - centre;
    let d = rounded_box(p, half, radius);

    var ring = e.misc.y;
    if kind == 4u {
        ring = e.params.x * scale;
    }
    var cover = clamp(0.5 - d, 0.0, 1.0);
    if ring > 0.0 {
        cover = clamp(0.5 - (abs(d + ring * 0.5) - ring * 0.5), 0.0, 1.0);
    }
    if cover <= 0.0 {
        discard;
    }

    if kind == 0u {
        let dir = vec2<f32>(cos(e.params.x), sin(e.params.x));
        let extent = max(abs(half.x * dir.x) + abs(half.y * dir.y), 0.5);
        let t = dot(p, dir) / (2.0 * extent) + 0.5;
        return encode(ramp(e, t)) * cover;
    }
    if kind == 1u {
        let o = e.shape.xy + e.params.xy * e.shape.zw;
        let r = max(e.params.z * max(e.shape.z, e.shape.w), 1.0);
        return encode(ramp(e, length(in.px - o) / r)) * cover;
    }
    if kind == 2u {
        let o = e.shape.xy + e.params.xy * e.shape.zw;
        let q = in.px - o;
        let t = fract((atan2(q.y, q.x) - e.params.z) / TAU);
        return encode(ramp(e, t)) * cover;
    }
    if kind == 3u {
        var cs = array<vec4<f32>, 4>(e.c0, e.c1, e.c2, e.c3);
        let n = i32(e.misc.x + 0.5);
        let tau = time / e.params.x * TAU;
        let size = max(e.params.y * max(e.shape.z, e.shape.w), 1.0);
        var acc = e.c0 * 0.15;
        var total = 0.15;
        for (var i = 0; i < n; i++) {
            let fi = f32(i);
            let phase = fi * 2.39996;
            let wobble = vec2<f32>(0.38 * sin(tau * (1.0 + 0.31 * fi) + phase), 0.34 * cos(tau * (0.8 + 0.23 * fi) + phase * 1.7));
            let at = e.shape.xy + e.shape.zw * (vec2<f32>(0.5) + wobble);
            let k = length(in.px - at) / size;
            let w = exp(-k * k);
            acc += cs[i] * w;
            total += w;
        }
        var lab = acc / total;
        let noise = hash(vec2<u32>(u32(max(in.px.x, 0.0)), u32(max(in.px.y, 0.0)))) - 0.5;
        lab.x += noise * e.params.z * lab.w;
        return encode(lab) * cover;
    }
    if kind == 4u {
        // 0 at the top, turning clockwise, like the head of the beam.
        let turn = fract(atan2(p.y, p.x) / TAU + 0.25);
        let head = fract(time / e.params.y);
        let behind = fract(head - turn);
        let tail = e.params.z;
        if behind > tail {
            discard;
        }
        let s = 1.0 - behind / tail;
        return encode(ramp(e, s)) * cover * smooth01(s);
    }
    if kind == 5u {
        let o = e.shape.xy + e.params.xy * scale;
        let q = length(in.px - o) / max(e.params.z * scale * 0.5, 1.0);
        return encode(e.c0) * cover * exp(-q * q * 2.5);
    }
    // Grain: light and dark specks.
    let v = (hash(vec2<u32>(u32(max(in.px.x, 0.0)), u32(max(in.px.y, 0.0)))) - 0.5) * 2.0 * e.params.x;
    if v > 0.0 {
        return vec4<f32>(v, v, v, v) * cover;
    }
    return vec4<f32>(0.0, 0.0, 0.0, -v) * cover;
}
