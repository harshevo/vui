//! Meshes: triangles with a premultiplied colour per vertex, filled at pixel centres with the
//! top-left rule, so triangles that share an edge never paint a pixel twice. Their soft edges come
//! from the transparent fringe the tessellator already added, as on the GPU.

use vui_core::MeshCmd;

use super::canvas::{Canvas, Span};

type P = [f32; 2];

#[inline]
fn edge(a: P, b: P, p: P) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Whether the edge `a → b` of a clockwise (in y-down space) triangle is a top or a left edge.
#[inline]
fn top_left(a: P, b: P) -> bool {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    (dy == 0.0 && dx > 0.0) || dy < 0.0
}

pub(crate) fn mesh(canvas: &mut Canvas, cmd: &MeshCmd, scale: f32) {
    let (w, h) = (canvas.width, canvas.height);
    let mut bounds = canvas.bounds();
    if let Some(c) = cmd.clip {
        // The GPU's scissor: whole pixels, rounded outward.
        let x0 = ((c.x * scale).floor().max(0.0) as u32).min(w) as i32;
        let y0 = ((c.y * scale).floor().max(0.0) as u32).min(h) as i32;
        let x1 = (((c.x + c.w) * scale).ceil().max(0.0) as u32).min(w) as i32;
        let y1 = (((c.y + c.h) * scale).ceil().max(0.0) as u32).min(h) as i32;
        bounds = bounds.intersect(Span { x0, y0, x1, y1 });
    }
    if bounds.is_empty() {
        return;
    }
    let k = cmd.opacity.clamp(0.0, 1.0);
    let vertex = |i: u32| {
        let v = &cmd.mesh.vertices[i as usize];
        ([v.pos[0] * scale, v.pos[1] * scale], v.color.map(|c| c * k))
    };
    for tri in cmd.mesh.indices.as_chunks::<3>().0 {
        let (mut a, mut ca) = vertex(tri[0]);
        let (b, cb) = vertex(tri[1]);
        let (mut c, mut cc) = vertex(tri[2]);
        let mut area = edge(a, b, c);
        if area == 0.0 {
            continue;
        }
        if area < 0.0 {
            std::mem::swap(&mut a, &mut c);
            std::mem::swap(&mut ca, &mut cc);
            area = -area;
        }
        let span = Span::centres_in(a[0].min(b[0]).min(c[0]), a[1].min(b[1]).min(c[1]), a[0].max(b[0]).max(c[0]) + 1e-3, a[1].max(b[1]).max(c[1]) + 1e-3)
            .intersect(bounds);
        if span.is_empty() {
            continue;
        }
        let (tl_bc, tl_ca, tl_ab) = (top_left(b, c), top_left(c, a), top_left(a, b));
        let inside = |w: f32, tl: bool| w > 0.0 || (w == 0.0 && tl);
        for y in span.y0..span.y1 {
            for x in span.x0..span.x1 {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let (wa, wb, wc) = (edge(b, c, p), edge(c, a, p), edge(a, b, p));
                if !(inside(wa, tl_bc) && inside(wb, tl_ca) && inside(wc, tl_ab)) {
                    continue;
                }
                let colour = std::array::from_fn(|i| (wa * ca[i] + wb * cb[i] + wc * cc[i]) / area);
                canvas.blend(x, y, colour);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use vui_core::vector::Mesh;
    use vui_core::{pt, rect};

    #[test]
    fn two_triangles_sharing_an_edge_paint_each_pixel_once() {
        let mut m = Mesh::default();
        let red = [0.5, 0.0, 0.0, 0.5];
        let r = rect(0.0, 0.0, 4.0, 4.0);
        let ids = [pt(r.x, r.y), pt(r.x + r.w, r.y), pt(r.x + r.w, r.y + r.h), pt(r.x, r.y + r.h)].map(|p| m.vertex(p, red));
        m.indices.extend([ids[0], ids[1], ids[2], ids[0], ids[2], ids[3]]);
        let mut canvas = Canvas::new(4, 4);
        mesh(&mut canvas, &MeshCmd::new(Rc::new(m)), 1.0);
        assert!(canvas.pixels().as_chunks::<4>().0.iter().all(|p| *p == [128, 0, 0, 128]), "{:?}", canvas.pixels());
    }
}
