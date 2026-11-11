// Layers: a group of commands drawn into a texture of its own, laid over the frame moved, scaled
// or turned, and faded as one.
//
// The quad covers the transformed layer's upright bounds. Each pixel maps its centre back through
// the inverse transform into the layer's texture, is dropped outside the kept rect, and filters the
// four nearest texels by hand, in float, so the CPU renderer can do the very same sums. The texture
// holds premultiplied colour in encoded sRGB, like the frame, so filtering and blending happen in the
// space everything else blends in.

struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

struct Layer {
    rect: vec4<f32>,   // the upright bounds drawn, x, y, w, h in device pixels
    inv: vec4<f32>,    // a, b, c, d of the inverse transform
    inv_t: vec4<f32>,  // e, f of the inverse transform (to texels); z: opacity
    keep: vec4<f32>,   // x0, y0, x1, y1: the kept rect, in texels
    clip: vec4<f32>,   // x, y, w, h in device pixels
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> layers: array<Layer>;
@group(1) @binding(0) var tex: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) px: vec2<f32>,
    @location(1) @interpolate(flat) idx: u32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let l = layers[ii];
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let p = l.rect.xy + corner * l.rect.zw;
    var o: VsOut;
    o.pos = vec4<f32>(p.x / g.viewport.x * 2.0 - 1.0, 1.0 - p.y / g.viewport.y * 2.0, 0.0, 1.0);
    o.px = p;
    o.idx = ii;
    return o;
}

fn texel(p: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return textureLoad(tex, clamp(p, vec2<i32>(0), size - vec2<i32>(1)), 0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let l = layers[in.idx];
    // The pixel's centre: what the rasteriser hands over as the position.
    let p = floor(in.pos.xy) + vec2<f32>(0.5);
    let q = vec2<f32>(l.inv.x * p.x + l.inv.z * p.y + l.inv_t.x, l.inv.y * p.x + l.inv.w * p.y + l.inv_t.y);
    if (q.x < l.keep.x || q.y < l.keep.y || q.x >= l.keep.z || q.y >= l.keep.w) {
        return vec4<f32>(0.0);
    }
    let size = vec2<i32>(textureDimensions(tex));
    let s = q - vec2<f32>(0.5);
    let base = floor(s);
    let f = s - base;
    let i = vec2<i32>(base);
    let top = mix(texel(i, size), texel(i + vec2<i32>(1, 0), size), f.x);
    let bottom = mix(texel(i + vec2<i32>(0, 1), size), texel(i + vec2<i32>(1, 1), size), f.x);
    let col = mix(top, bottom, f.y);
    let cx = clamp(min(p.x - l.clip.x, l.clip.x + l.clip.z - p.x) + 0.5, 0.0, 1.0);
    let cy = clamp(min(p.y - l.clip.y, l.clip.y + l.clip.w - p.y) + 0.5, 0.0, 1.0);
    return col * (l.inv_t.z * cx * cy);
}
