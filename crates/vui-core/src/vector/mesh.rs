//! Triangles with a colour per vertex: what every filled shape and stroked line becomes before the GPU
//! sees it. Colours are premultiplied. A fringe of vertices with zero alpha around each shape gives it a
//! one-pixel soft edge, so lines and fills are antialiased without any multisampling.

use crate::geom::{Point, Rect, pt, rect};

/// A premultiplied RGBA colour.
pub type Rgba = [f32; 4];

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Vertex {
    /// Window position in logical pixels.
    pub pos: [f32; 2],
    pub color: Rgba,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

pub fn scaled(c: Rgba, k: f32) -> Rgba {
    [c[0] * k, c[1] * k, c[2] * k, c[3] * k]
}

impl Mesh {
    pub fn new() -> Mesh {
        Mesh::default()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn vertex(&mut self, p: Point, color: Rgba) -> u32 {
        self.vertices.push(Vertex { pos: [p.x, p.y], color });
        (self.vertices.len() - 1) as u32
    }

    pub fn triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }

    /// Two triangles over four corners given in order around the quad.
    pub fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.triangle(a, b, c);
        self.triangle(a, c, d);
    }

    pub fn append(&mut self, other: &Mesh) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn bounds(&self) -> Rect {
        if self.vertices.is_empty() {
            return Rect::ZERO;
        }
        let (mut lo, mut hi) = (pt(f32::MAX, f32::MAX), pt(f32::MIN, f32::MIN));
        for v in &self.vertices {
            lo = pt(lo.x.min(v.pos[0]), lo.y.min(v.pos[1]));
            hi = pt(hi.x.max(v.pos[0]), hi.y.max(v.pos[1]));
        }
        rect(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y)
    }

    /// Adds a plain filled rectangle with a soft edge: for bars and cells, where a quad's rounded
    /// corners are not wanted and thousands of them should be one draw.
    pub fn rect(&mut self, r: Rect, color: Rgba, px: f32) {
        let f = (px / 2.0).min(r.w / 2.0).min(r.h / 2.0).max(0.0);
        let clear = [0.0; 4];
        let inner = [
            self.vertex(pt(r.x + f, r.y + f), color),
            self.vertex(pt(r.right() - f, r.y + f), color),
            self.vertex(pt(r.right() - f, r.bottom() - f), color),
            self.vertex(pt(r.x + f, r.bottom() - f), color),
        ];
        self.quad(inner[0], inner[1], inner[2], inner[3]);
        if f > 0.0 {
            let outer = [
                self.vertex(pt(r.x - f, r.y - f), clear),
                self.vertex(pt(r.right() + f, r.y - f), clear),
                self.vertex(pt(r.right() + f, r.bottom() + f), clear),
                self.vertex(pt(r.x - f, r.bottom() + f), clear),
            ];
            for i in 0..4 {
                let j = (i + 1) % 4;
                self.quad(inner[i], outer[i], outer[j], inner[j]);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod raster {
    //! A tiny software rasteriser for tests: samples a mesh's alpha at pixel centres, so coverage and
    //! areas can be asserted without a GPU.

    use super::*;

    pub fn coverage(mesh: &Mesh, w: usize, h: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; w * h];
        for t in mesh.indices.as_chunks::<3>().0 {
            let v = [mesh.vertices[t[0] as usize], mesh.vertices[t[1] as usize], mesh.vertices[t[2] as usize]];
            let (x0, x1) = (v.iter().map(|v| v.pos[0]).fold(f32::MAX, f32::min), v.iter().map(|v| v.pos[0]).fold(f32::MIN, f32::max));
            let (y0, y1) = (v.iter().map(|v| v.pos[1]).fold(f32::MAX, f32::min), v.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max));
            for y in (y0.floor().max(0.0) as usize)..((y1.ceil() as usize).min(h)) {
                for x in (x0.floor().max(0.0) as usize)..((x1.ceil() as usize).min(w)) {
                    let p = (x as f32 + 0.5, y as f32 + 0.5);
                    let d = (v[1].pos[1] - v[2].pos[1]) * (v[0].pos[0] - v[2].pos[0]) + (v[2].pos[0] - v[1].pos[0]) * (v[0].pos[1] - v[2].pos[1]);
                    if d.abs() < 1e-9 {
                        continue;
                    }
                    let a = ((v[1].pos[1] - v[2].pos[1]) * (p.0 - v[2].pos[0]) + (v[2].pos[0] - v[1].pos[0]) * (p.1 - v[2].pos[1])) / d;
                    let b = ((v[2].pos[1] - v[0].pos[1]) * (p.0 - v[2].pos[0]) + (v[0].pos[0] - v[2].pos[0]) * (p.1 - v[2].pos[1])) / d;
                    let c = 1.0 - a - b;
                    if a >= -1e-6 && b >= -1e-6 && c >= -1e-6 {
                        let alpha = a * v[0].color[3] + b * v[1].color[3] + c * v[2].color[3];
                        let cell = &mut out[y * w + x];
                        // Source over, as the GPU does it.
                        *cell = alpha + *cell * (1.0 - alpha);
                    }
                }
            }
        }
        out
    }

    pub fn sum(cov: &[f32]) -> f32 {
        cov.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_soft_rect_covers_its_area_with_a_fringe_outside() {
        let mut m = Mesh::new();
        m.rect(rect(2.0, 2.0, 6.0, 4.0), [1.0; 4], 1.0);
        let cov = raster::coverage(&m, 10, 8);
        assert!((cov[3 * 10 + 4] - 1.0).abs() < 1e-5, "inside is solid");
        assert_eq!(cov[0], 0.0, "far outside is empty");
        assert!((raster::sum(&cov) - 24.0).abs() < 2.0, "area is about 24 pixels: {}", raster::sum(&cov));
    }

    #[test]
    fn appending_offsets_the_indices() {
        let mut a = Mesh::new();
        a.rect(rect(0.0, 0.0, 2.0, 2.0), [1.0; 4], 0.0);
        let mut b = Mesh::new();
        b.rect(rect(5.0, 5.0, 2.0, 2.0), [1.0; 4], 0.0);
        let n = a.vertices.len() as u32;
        a.append(&b);
        assert!(a.indices.iter().skip(6).all(|i| *i >= n));
        assert_eq!(a.bounds(), rect(0.0, 0.0, 7.0, 7.0));
    }
}
