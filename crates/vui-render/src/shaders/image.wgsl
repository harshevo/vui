// Pictures with rounded corners.
//
// The texture is an sRGB view, so sampling decodes to linear light and the filtering (mips,
// anisotropy, bilinear) happens there, which is what makes minified photos and screenshots look
// right. The result is encoded back to sRGB and blended in encoded space like everything else.
// Colour is premultiplied by alpha in linear light before upload, so what comes out of the sampler
// is premultiplied linear light; it is un-premultiplied here, encoded, and premultiplied again.

struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

struct ImageInst {
    rect: vec4<f32>,    // x, y, w, h in device pixels
    radii: vec4<f32>,
    uv: vec4<f32>,      // u0, v0, u1, v1: the part of the image to show
    tint: vec4<f32>,    // premultiplied, used in mask mode
    ring: vec4<f32>,    // premultiplied hairline colour
    clip: vec4<f32>,
    params: vec4<f32>,  // x: opacity, y: corner smoothness, z: ring width
    flags: vec4<f32>,   // x: mode (0 colour, 1 mask), y: sampling (0 smooth, 1 sharp, 2 nearest), zw: texture size
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> images: array<ImageInst>;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var smp_linear: sampler;
@group(1) @binding(2) var smp_nearest: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) px: vec2<f32>,
    @location(1) @interpolate(flat) idx: u32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let im = images[ii];
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let pad = 1.0;
    let p = im.rect.xy - vec2<f32>(pad) + corner * (im.rect.zw + vec2<f32>(2.0 * pad));
    var o: VsOut;
    o.pos = vec4<f32>(p.x / g.viewport.x * 2.0 - 1.0, 1.0 - p.y / g.viewport.y * 2.0, 0.0, 1.0);
    o.px = p;
    o.idx = ii;
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let im = images[in.idx];
    let half = im.rect.zw * 0.5;
    let p = in.px - (im.rect.xy + half);
    let smooth_k = im.params.y;

    let box_outer = sd_box(p, half, im.radii, smooth_k);
    let cov_outer = coverage(box_outer.x, box_outer.y);

    // Source coordinate, and its gradients taken before any branching so sampling stays valid.
    let t = clamp((in.px - im.rect.xy) / im.rect.zw, vec2<f32>(0.0), vec2<f32>(1.0));
    var uv = mix(im.uv.xy, im.uv.zw, t);
    let ddx = dpdx(uv);
    let ddy = dpdy(uv);

    let tex_size = im.flags.zw;
    let sampling = u32(im.flags.y);
    if (sampling == 1u) {
        // Sharp bilinear: when a texel covers several screen pixels, sample its centre everywhere
        // except within half a screen pixel of its border, where the hardware blends it with its
        // neighbour. Edges stay crisp but are still antialiased.
        let per_texel = max(im.rect.zw / max(abs(im.uv.zw - im.uv.xy) * tex_size, vec2<f32>(1e-4)), vec2<f32>(1.0));
        let texel = uv * tex_size;
        let base = floor(texel);
        let from_centre = texel - base - vec2<f32>(0.5);
        let flat_range = vec2<f32>(0.5) - 0.5 / per_texel;
        let f = (from_centre - clamp(from_centre, -flat_range, flat_range)) * per_texel + vec2<f32>(0.5);
        uv = (base + f) / tex_size;
    }

    // Stay half a texel inside the crop so a cropped picture never blends in the texels beside it.
    let crop_lo = min(im.uv.xy, im.uv.zw);
    let crop_hi = max(im.uv.xy, im.uv.zw);
    let crop_mid = (crop_lo + crop_hi) * 0.5;
    let inset = vec2<f32>(0.5) / tex_size;
    uv = clamp(uv, min(crop_lo + inset, crop_mid), max(crop_hi - inset, crop_mid));

    var c: vec4<f32>;
    if (sampling == 2u) {
        c = textureSampleGrad(tex, smp_nearest, uv, ddx, ddy);
    } else {
        c = textureSampleGrad(tex, smp_linear, uv, ddx, ddy);
    }

    var colour = vec4<f32>(0.0);
    if (c.a > 0.0) {
        let straight = clamp(c.rgb / c.a, vec3<f32>(0.0), vec3<f32>(1.0));
        colour = vec4<f32>(linear_to_srgb(straight) * c.a, c.a);
    }
    if (im.flags.x > 0.5) {
        colour = im.tint * c.a;
    }
    colour = colour * im.params.x;

    var cov_inner = cov_outer;
    let rw = im.params.z;
    if (rw > 0.0) {
        let inner_half = max(half - vec2<f32>(rw), vec2<f32>(0.0));
        let inner_radii = max(im.radii - vec4<f32>(rw), vec4<f32>(0.0));
        let box_inner = sd_box(p, inner_half, inner_radii, smooth_k);
        cov_inner = coverage(box_inner.x, box_inner.y);
    }
    // The picture everywhere inside; the ring over the picture along the edge.
    var col = colour * cov_inner;
    if (rw > 0.0) {
        col = col + (im.ring + colour * (1.0 - im.ring.a)) * (cov_outer - cov_inner);
    }

    let cx = clamp(min(in.px.x - im.clip.x, im.clip.x + im.clip.z - in.px.x) + 0.5, 0.0, 1.0);
    let cy = clamp(min(in.px.y - im.clip.y, im.clip.y + im.clip.w - in.px.y) + 0.5, 0.0, 1.0);
    return col * (cx * cy);
}
