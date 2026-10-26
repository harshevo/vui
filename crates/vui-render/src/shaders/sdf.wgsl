// Shared by the shape and image shaders: rounded-box distance, edge coverage and colour transfer.

// Signed distance to a rounded box centred on the origin. With smooth > 0 the corner uses a
// superellipse norm instead of a circle, which is what makes continuous corners look continuous.
// Returns (distance, gradient length). The gradient is analytic: screen derivatives are wrong at a
// corner that sits on an odd pixel, where the 2x2 fragment group straddles it.
fn sd_box(p: vec2<f32>, half: vec2<f32>, radii: vec4<f32>, smooth_k: f32) -> vec2<f32> {
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
    var grad = 1.0;
    if (smooth_k <= 0.001) {
        corner = length(qp);
    } else {
        let n = mix(2.0, 5.0, smooth_k);
        corner = pow(pow(qp.x, n) + pow(qp.y, n), 1.0 / n);
        if (corner > 0.0) {
            grad = length(vec2<f32>(pow(qp.x, n - 1.0), pow(qp.y, n - 1.0))) / pow(corner, n - 1.0);
        }
    }
    return vec2<f32>(inside + corner - r, grad);
}

// Pixel coverage from a distance. Dividing by the gradient length keeps the edge one pixel wide
// even where the field is not a true Euclidean distance.
fn coverage(d: f32, grad: f32) -> f32 {
    return clamp(0.5 - d / max(grad, 1e-4), 0.0, 1.0);
}


fn linear_to_srgb1(c: f32) -> f32 {
    if (c <= 0.0031308) {
        return c * 12.92;
    }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(linear_to_srgb1(c.x), linear_to_srgb1(c.y), linear_to_srgb1(c.z));
}
