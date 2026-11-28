// Layers: a group of commands drawn into a texture of its own, laid over the frame moved, scaled
// or turned, and faded as one.
//
// The quad covers the transformed layer's upright bounds. Each pixel maps its centre back through
// the inverse transform into the layer's texture, is bent there if the layer is elastic, is dropped
// outside the kept rect, and filters the four nearest texels by hand, in float, so the CPU renderer
// can do the very same sums. The texture holds premultiplied colour in encoded sRGB, like the frame,
// so filtering and blending happen in the space everything else blends in.
//
// Elastic (vui_core::elastic): a layer springing in or out across one side of its box. Turned so
// the side it hangs on is on the right, the box slides toward that side and is cut there; past its
// place its far side reaches further and the box stretches from that side; and its far side bows
// with the speed. Beyond its far side the layer moves with it.

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
    bend_from: vec4<f32>, // x0, y0, x1, y1: the elastic box at rest, in texels
    bend: vec4<f32>,      // x: the side it hangs on, y: how far out, z: its bow, w: 1 when bent
};

const PI: f32 = 3.14159265358979;

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

// A point in the frame where the box hangs on its right: x runs toward that side, y along it.
fn canon(p: vec2<f32>, edge: u32) -> vec2<f32> {
    switch edge {
        case 1u: { return vec2<f32>(-p.x, p.y); }
        case 2u: { return vec2<f32>(p.y, p.x); }
        case 3u: { return vec2<f32>(-p.y, p.x); }
        default: { return p; }
    }
}

fn uncanon(c: vec2<f32>, edge: u32) -> vec2<f32> {
    switch edge {
        case 1u: { return vec2<f32>(-c.x, c.y); }
        case 2u: { return vec2<f32>(c.y, c.x); }
        case 3u: { return vec2<f32>(c.y, -c.x); }
        default: { return c; }
    }
}

// The point of an elastic layer that shows at q (texels), and how much of the pixel it covers (z);
// z is 0 where nothing shows.
fn bend(q: vec2<f32>, l: Layer) -> vec3<f32> {
    let edge = u32(l.bend.x + 0.5);
    let out = l.bend.y;
    let bow = l.bend.z;
    if (out >= 1.0) {
        return vec3<f32>(0.0);
    }
    let c = canon(q, edge);
    let a = canon(l.bend_from.xy, edge);
    let b = canon(l.bend_from.zw, edge);
    let lo = min(a, b);
    let hi = max(a, b);
    let w = hi.x - lo.x;
    let h = hi.y - lo.y;
    if (w <= 0.0 || h <= 0.0) {
        return vec3<f32>(0.0);
    }
    let cover = clamp(hi.x - c.x + 0.5, 0.0, 1.0);
    if (cover <= 0.0) {
        return vec3<f32>(0.0);
    }
    let along = clamp((c.y - lo.y) / h, 0.0, 1.0);
    let far = lo.x + max(out, 0.0) * w - max(-out, 0.0) * w - bow * w * sin(PI * along);
    let near = hi.x + max(out, 0.0) * w;
    let span = near - far;
    if (span <= 1e-3) {
        return vec3<f32>(0.0);
    }
    var su = lo.x + (c.x - far) / span * w;
    if (c.x < far) {
        su = lo.x + (c.x - far);
    }
    return vec3<f32>(uncanon(vec2<f32>(su, c.y), edge), cover);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let l = layers[in.idx];
    // The pixel's centre: what the rasteriser hands over as the position.
    let p = floor(in.pos.xy) + vec2<f32>(0.5);
    var q = vec2<f32>(l.inv.x * p.x + l.inv.z * p.y + l.inv_t.x, l.inv.y * p.x + l.inv.w * p.y + l.inv_t.y);
    var cover = 1.0;
    if (l.bend.w > 0.5) {
        let bent = bend(q, l);
        if (bent.z <= 0.0) {
            return vec4<f32>(0.0);
        }
        q = bent.xy;
        cover = bent.z;
    }
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
    return col * (l.inv_t.z * cx * cy * cover);
}
