// Glass: shrink the frame under a shape, blur it across and down, then shade the shape from it.
// The steps and their maths follow `glass.rs` and `cpu/glass.rs`; keep the three in step.

struct Params {
    region: vec4<f32>,
    down: vec4<f32>,
    blur: vec4<f32>,
    a: vec4<f32>,
    radii: vec4<f32>,
    b: vec4<f32>,
    tint: vec4<f32>,
    look: vec4<f32>,
    look2: vec4<f32>,
    clip: vec4<f32>,
    area: vec4<f32>,
    spec: vec4<f32>,
    viewport: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> params: array<Params>;
@group(1) @binding(0) var src: texture_2d<f32>;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) index: u32,
};

// A triangle over the whole viewport, for the shrink, blur and copy steps.
@vertex
fn vs_full(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: Out;
    out.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.index = ii;
    return out;
}

// The glass quad, in the frame.
@vertex
fn vs_glass(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    let p = params[ii];
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let px = p.area.xy + corner * p.area.zw;
    var out: Out;
    out.pos = vec4<f32>(px.x / p.viewport.x * 2.0 - 1.0, 1.0 - px.y / p.viewport.y * 2.0, 0.0, 1.0);
    out.index = ii;
    return out;
}

@fragment
fn fs_shrink(in: Out) -> @location(0) vec4<f32> {
    let p = params[in.index];
    let f = i32(p.down.x);
    let ij = vec2<i32>(in.pos.xy);
    let lo = vec2<i32>(p.region.xy);
    let hi = vec2<i32>(p.region.zw) - vec2<i32>(1);
    var sum = vec4<f32>(0.0);
    for (var v = 0; v < f; v++) {
        for (var u = 0; u < f; u++) {
            let at = clamp(lo + ij * f + vec2<i32>(u, v), lo, hi);
            sum += textureLoad(src, at, 0);
        }
    }
    return sum / f32(f * f);
}

fn gauss(in: Out, step: vec2<i32>) -> vec4<f32> {
    let p = params[in.index];
    let taps = i32(p.down.w);
    let ij = vec2<i32>(in.pos.xy);
    let hi = vec2<i32>(i32(p.down.y) - 1, i32(p.down.z) - 1);
    if taps == 0 {
        return textureLoad(src, ij, 0);
    }
    let sigma = p.blur.x;
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var k = -taps; k <= taps; k++) {
        let w = exp(-f32(k * k) / (2.0 * sigma * sigma));
        sum += textureLoad(src, clamp(ij + step * k, vec2<i32>(0), hi), 0) * w;
        total += w;
    }
    return sum / total;
}

@fragment
fn fs_across(in: Out) -> @location(0) vec4<f32> {
    return gauss(in, vec2<i32>(1, 0));
}

@fragment
fn fs_down(in: Out) -> @location(0) vec4<f32> {
    return gauss(in, vec2<i32>(0, 1));
}

@fragment
fn fs_copy(in: Out) -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(in.pos.xy), 0);
}

fn rounded_box(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let rr = max(min(r, min(half.x, half.y)), 0.0);
    let q = abs(p) - half + rr;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return min(a, b);
    }
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

fn distance(px: vec2<f32>, p: Params) -> f32 {
    let da = rounded_box(px - p.a.xy, p.a.zw, p.radii.x);
    let db = rounded_box(px - p.b.xy, p.b.zw, p.radii.y);
    return smooth_min(da, db, p.radii.z);
}

fn grain(x: u32, y: u32) -> f32 {
    var v = x * 747796405u + y * 2891336453u + 1u;
    v = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
    v = (v >> 22u) ^ v;
    return f32(v >> 8u) / 16777215.0;
}

fn texel(ij: vec2<i32>, p: Params) -> vec4<f32> {
    let hi = vec2<i32>(i32(p.down.y) - 1, i32(p.down.z) - 1);
    return textureLoad(src, clamp(ij, vec2<i32>(0), hi), 0);
}

fn bilinear(px: vec2<f32>, p: Params) -> vec4<f32> {
    let q = (px - p.region.xy) / p.down.x - 0.5;
    let i0 = floor(q);
    let fr = q - i0;
    let ij = vec2<i32>(i0);
    let c00 = texel(ij, p);
    let c10 = texel(ij + vec2<i32>(1, 0), p);
    let c01 = texel(ij + vec2<i32>(0, 1), p);
    let c11 = texel(ij + vec2<i32>(1, 1), p);
    let top = c00 + (c10 - c00) * fr.x;
    let bottom = c01 + (c11 - c01) * fr.x;
    return top + (bottom - top) * fr.y;
}

@fragment
fn fs_glass(in: Out) -> @location(0) vec4<f32> {
    let p = params[in.index];
    let px = in.pos.xy;
    if px.x < p.clip.x || px.y < p.clip.y || px.x > p.clip.z || px.y > p.clip.w {
        discard;
    }
    let d = distance(px, p);
    let cover = clamp(0.5 - d, 0.0, 1.0) * p.look2.z;
    if cover <= 0.0 {
        discard;
    }
    let gx = distance(px + vec2<f32>(1.0, 0.0), p) - distance(px - vec2<f32>(1.0, 0.0), p);
    let gy = distance(px + vec2<f32>(0.0, 1.0), p) - distance(px - vec2<f32>(0.0, 1.0), p);
    let len = sqrt(gx * gx + gy * gy);
    var n = vec2<f32>(0.0);
    if len > 1.0e-6 {
        n = vec2<f32>(gx, gy) / len;
    }
    let refraction = p.look.y;
    let band = max(abs(refraction), 1.0);
    let t = clamp(-d / band, 0.0, 1.0);
    let bend = (1.0 - t) * (1.0 - t);
    let off = -n * refraction * bend;
    let disp = p.look.z;
    var c = vec3<f32>(0.0);
    if p.look2.w > 0.5 {
        c = vec3<f32>(bilinear(px + off * (1.0 + disp), p).r, bilinear(px + off, p).g, bilinear(px + off * (1.0 - disp), p).b);
    }
    let luma = c.r * 0.2126 + c.g * 0.7152 + c.b * 0.0722;
    c = clamp(vec3<f32>(luma) + (c - vec3<f32>(luma)) * p.look.x, vec3<f32>(0.0), vec3<f32>(1.0));
    let ta = p.tint.w;
    c = c * (1.0 - ta) + p.tint.rgb * ta;
    let scale = p.radii.w;
    let rt = clamp(-d / (1.5 * scale), 0.0, 1.0);
    let fresnel = (1.0 - rt) * (1.0 - rt);
    let light = 0.65 - 0.35 * n.y;
    let rim = clamp(p.look.w * fresnel * light * 0.6, 0.0, 1.0);
    let dd = (px - p.spec.xy) / max(p.spec.z, 1.0);
    let spec = clamp(p.look2.x * exp(-dot(dd, dd)) * 0.35, 0.0, 1.0);
    let noise = (grain(u32(px.x), u32(px.y)) - 0.5) * p.look2.y;
    c += (vec3<f32>(1.0) - c) * rim;
    c += (vec3<f32>(1.0) - c) * spec;
    c = clamp(c + vec3<f32>(noise), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(c * cover, cover);
}
