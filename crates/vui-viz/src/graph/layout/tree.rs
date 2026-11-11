//! The Barnes-Hut tree: distant groups of nodes push as one, so a step costs n log n, not n squared.

use super::*;

/// The i-th point on a sunflower spiral: evenly spread, and always the same.
pub(super) fn spiral(i: usize) -> Point {
    let r = 10.0 * (i as f32 + 0.5).sqrt();
    let a = i as f32 * 2.399_963_1; // the golden angle
    pt(r * a.cos(), r * a.sin())
}

#[derive(Clone, Debug, Default)]
pub(super) struct Tree {
    cells: Vec<Cell>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Cell {
    /// The square this cell covers: its corner and side.
    x: f32,
    y: f32,
    size: f32,
    /// Total mass and mass-weighted position sums.
    mass: f32,
    sx: f32,
    sy: f32,
    /// The one body in a leaf, or `NONE`.
    body: u32,
    kids: [u32; 4],
}

impl Cell {
    pub(super) fn new(x: f32, y: f32, size: f32) -> Cell {
        Cell {
            x,
            y,
            size,
            mass: 0.0,
            sx: 0.0,
            sy: 0.0,
            body: NONE,
            kids: [NONE; 4],
        }
    }

    pub(super) fn is_leaf(&self) -> bool {
        self.kids == [NONE; 4]
    }

    pub(super) fn quadrant(&self, px: f32, py: f32) -> usize {
        let h = self.size / 2.0;
        usize::from(px >= self.x + h) | (usize::from(py >= self.y + h) << 1)
    }
}

impl Tree {
    pub(super) fn build(&mut self, xs: &[f32], ys: &[f32], mass: &[f32]) {
        self.cells.clear();
        let (mut x0, mut y0, mut x1, mut y1) = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        for (x, y) in xs.iter().zip(ys) {
            (x0, y0, x1, y1) = (x0.min(*x), y0.min(*y), x1.max(*x), y1.max(*y));
        }
        if xs.is_empty() {
            return;
        }
        let size = (x1 - x0).max(y1 - y0).max(1.0) * 1.001;
        self.cells.reserve(xs.len() * 2);
        self.cells.push(Cell::new(x0, y0, size));
        for i in 0..xs.len() {
            self.insert(i as u32, xs, ys, mass);
        }
    }

    pub(super) fn insert(&mut self, body: u32, xs: &[f32], ys: &[f32], mass: &[f32]) {
        let (px, py, m) = (xs[body as usize], ys[body as usize], mass[body as usize]);
        let mut at = 0usize;
        for depth in 0.. {
            let cell = &mut self.cells[at];
            let was_empty = cell.mass == 0.0;
            cell.mass += m;
            cell.sx += px * m;
            cell.sy += py * m;
            if was_empty {
                cell.body = body;
                return;
            }
            if cell.is_leaf() {
                // Two bodies in one cell: split it, unless they are on top of each other.
                let other = cell.body as usize;
                if depth >= MAX_DEPTH || (xs[other] == px && ys[other] == py) {
                    return;
                }
                cell.body = NONE;
                let q = cell.quadrant(xs[other], ys[other]);
                let kid = self.child(at, q);
                let (om, ox, oy) = (mass[other], xs[other], ys[other]);
                let kid_cell = &mut self.cells[kid];
                kid_cell.mass = om;
                kid_cell.sx = ox * om;
                kid_cell.sy = oy * om;
                kid_cell.body = other as u32;
            }
            let q = self.cells[at].quadrant(px, py);
            at = self.child(at, q);
        }
    }

    /// The child of `at` in quadrant `q`, made if it is not there yet.
    pub(super) fn child(&mut self, at: usize, q: usize) -> usize {
        if self.cells[at].kids[q] == NONE {
            let c = self.cells[at];
            let h = c.size / 2.0;
            let id = self.cells.len() as u32;
            self.cells.push(Cell::new(
                c.x + if q & 1 == 1 { h } else { 0.0 },
                c.y + if q & 2 == 2 { h } else { 0.0 },
                h,
            ));
            self.cells[at].kids[q] = id;
        }
        self.cells[at].kids[q] as usize
    }

    /// The summed pull toward everything else, each as `offset · mass / distance²` (so it falls off like
    /// 1/distance), lumping together cells that are small next to how far away they are.
    pub(super) fn push_on(&self, me: usize, px: f32, py: f32, theta: f32) -> (f32, f32) {
        if self.cells.is_empty() {
            return (0.0, 0.0);
        }
        let (mut fx, mut fy) = (0.0, 0.0);
        let mut stack = vec![0usize];
        while let Some(at) = stack.pop() {
            let c = &self.cells[at];
            if c.mass == 0.0 {
                continue;
            }
            let (cx, cy) = (c.sx / c.mass, c.sy / c.mass);
            let (dx, dy) = (cx - px, cy - py);
            let l2 = dx * dx + dy * dy;
            if c.is_leaf() {
                if c.body as usize == me && c.mass > 0.0 {
                    continue;
                }
                // Distinct points only: bodies on top of each other push on nothing.
                if l2 > 0.0 {
                    let l2 = l2.max(1.0);
                    fx += dx * c.mass / l2;
                    fy += dy * c.mass / l2;
                }
            } else if c.size * c.size < theta * theta * l2 {
                let l2 = l2.max(1.0);
                fx += dx * c.mass / l2;
                fy += dy * c.mass / l2;
            } else {
                stack.extend(c.kids.iter().filter(|k| **k != NONE).map(|k| *k as usize));
            }
        }
        (fx, fy)
    }
}
