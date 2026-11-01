//! Force-directed layout: nodes push each other apart, edges pull their ends together, and a faint
//! pull toward the middle keeps separate clusters from drifting away. Repulsion uses a Barnes-Hut
//! quadtree, so a step costs about n·log n and graphs of several thousand nodes stay smooth.
//!
//! The layout is a small simulation you step: [`Layout::step`] moves everything a little and says
//! whether it is still moving. It cools as it goes, can be warmed again, and nodes can be held in place
//! (which is how dragging one works). Starting positions are a spiral, so the same graph always settles
//! the same way.

use vui_core::{Point, pt};

const NONE: u32 = u32::MAX;
/// Below this many cell widths of depth, bodies that still share a cell are treated as one.
const MAX_DEPTH: u32 = 28;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Forces {
    /// How hard nodes push apart; larger spreads the graph out.
    pub charge: f32,
    /// The length an edge settles at when nothing else pulls on it.
    pub link_distance: f32,
    /// The pull toward the centre, per unit of distance.
    pub gravity: f32,
    /// How coarsely far-away nodes are lumped together: 0 is exact, 1 is fast and rough.
    pub theta: f32,
    /// Share of speed kept each step.
    pub momentum: f32,
}

impl Default for Forces {
    fn default() -> Forces {
        Forces {
            charge: 90.0,
            link_distance: 46.0,
            gravity: 0.045,
            theta: 0.85,
            momentum: 0.6,
        }
    }
}

const ALPHA_MIN: f32 = 0.002;
/// Cooling per step: the simulation reaches `ALPHA_MIN` from 1 in about 300 steps.
const ALPHA_DECAY: f32 = 0.0204;

#[derive(Clone, Debug)]
pub struct Layout {
    x: Vec<f32>,
    y: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    held: Vec<bool>,
    mass: Vec<f32>,
    links: Vec<Link>,
    degree: Vec<u32>,
    pub forces: Forces,
    alpha: f32,
    target: f32,
    tree: Tree,
}

#[derive(Copy, Clone, Debug)]
struct Link {
    a: u32,
    b: u32,
    strength: f32,
}

impl Layout {
    /// A layout of `n` nodes joined by `edges` (index pairs; repeats and loops are ignored).
    pub fn new(n: usize, edges: &[(usize, usize)]) -> Layout {
        let mut layout = Layout {
            x: vec![0.0; n],
            y: vec![0.0; n],
            vx: vec![0.0; n],
            vy: vec![0.0; n],
            held: vec![false; n],
            mass: vec![1.0; n],
            links: Vec::new(),
            degree: vec![0; n],
            forces: Forces::default(),
            alpha: 1.0,
            target: 0.0,
            tree: Tree::default(),
        };
        for i in 0..n {
            let p = spiral(i);
            layout.x[i] = p.x;
            layout.y[i] = p.y;
        }
        layout.set_edges(edges);
        layout
    }

    fn set_edges(&mut self, edges: &[(usize, usize)]) {
        let n = self.x.len();
        let mut seen = std::collections::HashSet::new();
        self.links.clear();
        self.degree.iter_mut().for_each(|d| *d = 0);
        for &(a, b) in edges {
            if a == b || a >= n || b >= n || !seen.insert((a.min(b), a.max(b))) {
                continue;
            }
            self.links.push(Link {
                a: a as u32,
                b: b as u32,
                strength: 0.0,
            });
            self.degree[a] += 1;
            self.degree[b] += 1;
        }
        for l in &mut self.links {
            l.strength = 1.0
                / self.degree[l.a as usize]
                    .min(self.degree[l.b as usize])
                    .max(1) as f32;
        }
        for (m, d) in self.mass.iter_mut().zip(&self.degree) {
            *m = 1.0 + 0.3 * (*d as f32).sqrt();
        }
    }

    /// Replaces the graph, keeping the positions of nodes that stay. `keep[i]` is the old index of new node
    /// `i`, if it has one; the rest start beside their neighbours. The layout warms up a little so the
    /// newcomers settle in without shaking everything else loose.
    pub fn rebuild(&mut self, n: usize, edges: &[(usize, usize)], keep: &[Option<usize>]) {
        let old = (
            std::mem::take(&mut self.x),
            std::mem::take(&mut self.y),
            std::mem::take(&mut self.held),
        );
        self.x = vec![0.0; n];
        self.y = vec![0.0; n];
        self.vx = vec![0.0; n];
        self.vy = vec![0.0; n];
        self.held = vec![false; n];
        self.mass = vec![1.0; n];
        self.degree = vec![0; n];
        let mut placed = vec![false; n];
        for (i, k) in keep.iter().enumerate().take(n) {
            if let Some(o) = *k
                && o < old.0.len()
            {
                (self.x[i], self.y[i], self.held[i], placed[i]) =
                    (old.0[o], old.1[o], old.2[o], true);
            }
        }
        self.set_edges(edges);
        // Newcomers start at the average of the neighbours already placed, nudged apart so two new nodes
        // on the same neighbour do not sit exactly on top of each other.
        let mut around: Vec<Vec<usize>> = vec![Vec::new(); n];
        for l in &self.links {
            around[l.a as usize].push(l.b as usize);
            around[l.b as usize].push(l.a as usize);
        }
        for i in 0..n {
            if placed[i] {
                continue;
            }
            let near: Vec<usize> = around[i].iter().copied().filter(|&j| placed[j]).collect();
            let spread = spiral(i);
            if near.is_empty() {
                (self.x[i], self.y[i]) = (spread.x, spread.y);
            } else {
                let k = near.len() as f32;
                self.x[i] =
                    near.iter().map(|&j| self.x[j]).sum::<f32>() / k + spread.x * 0.05 + 3.0;
                self.y[i] =
                    near.iter().map(|&j| self.y[j]).sum::<f32>() / k + spread.y * 0.05 - 3.0;
            }
            placed[i] = true;
        }
        self.reheat(0.35);
    }

    pub fn len(&self) -> usize {
        self.x.len()
    }

    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    pub fn position(&self, i: usize) -> Point {
        pt(self.x[i], self.y[i])
    }

    pub fn positions(&self) -> impl Iterator<Item = Point> + '_ {
        self.x.iter().zip(&self.y).map(|(x, y)| pt(*x, *y))
    }

    pub fn degree(&self, i: usize) -> usize {
        self.degree[i] as usize
    }

    /// Warms the simulation up to at least `alpha` (0 to 1) so it moves again.
    pub fn reheat(&mut self, alpha: f32) {
        self.alpha = self.alpha.max(alpha.clamp(0.0, 1.0));
    }

    /// Keeps the simulation at least this warm until it is set back to 0; dragging a node uses it.
    pub fn keep_warm(&mut self, target: f32) {
        self.target = target.clamp(0.0, 1.0);
        self.reheat(self.target);
    }

    /// Holds node `i` at `p`: forces no longer move it.
    pub fn hold(&mut self, i: usize, p: Point) {
        (self.x[i], self.y[i], self.vx[i], self.vy[i], self.held[i]) = (p.x, p.y, 0.0, 0.0, true);
    }

    pub fn release(&mut self, i: usize) {
        self.held[i] = false;
    }

    pub fn is_held(&self, i: usize) -> bool {
        self.held[i]
    }

    pub fn is_settled(&self) -> bool {
        self.alpha < ALPHA_MIN && self.target == 0.0
    }

    /// Moves everything one step. Returns true while it is still moving.
    pub fn step(&mut self) -> bool {
        if self.is_settled() || self.x.is_empty() {
            return false;
        }
        self.alpha += (self.target - self.alpha) * ALPHA_DECAY;
        self.pull_links();
        self.push_apart();
        self.pull_to_centre();
        let keep = self.forces.momentum;
        for i in 0..self.x.len() {
            if self.held[i] {
                (self.vx[i], self.vy[i]) = (0.0, 0.0);
                continue;
            }
            self.vx[i] *= keep;
            self.vy[i] *= keep;
            self.x[i] += self.vx[i];
            self.y[i] += self.vy[i];
        }
        !self.is_settled()
    }

    /// Steps until it settles or `limit` steps have run. For graphs drawn once, and for tests.
    pub fn settle(&mut self, limit: usize) -> usize {
        let mut n = 0;
        while n < limit && self.step() {
            n += 1;
        }
        n
    }

    fn pull_links(&mut self) {
        let want = self.forces.link_distance;
        for l in &self.links {
            let (a, b) = (l.a as usize, l.b as usize);
            let mut dx = self.x[b] + self.vx[b] - self.x[a] - self.vx[a];
            let mut dy = self.y[b] + self.vy[b] - self.y[a] - self.vy[a];
            if dx == 0.0 && dy == 0.0 {
                (dx, dy) = (0.01, 0.01);
            }
            let len = (dx * dx + dy * dy).sqrt();
            let k = (len - want) / len * self.alpha * l.strength;
            let (dx, dy) = (dx * k, dy * k);
            // The better connected end moves less.
            let bias = self.degree[a] as f32 / (self.degree[a] + self.degree[b]).max(1) as f32;
            self.vx[b] -= dx * bias;
            self.vy[b] -= dy * bias;
            self.vx[a] += dx * (1.0 - bias);
            self.vy[a] += dy * (1.0 - bias);
        }
    }

    fn push_apart(&mut self) {
        self.tree.build(&self.x, &self.y, &self.mass);
        let (charge, theta) = (self.forces.charge, self.forces.theta);
        for i in 0..self.x.len() {
            if self.held[i] {
                continue;
            }
            let (fx, fy) = self.tree.push_on(i, self.x[i], self.y[i], theta);
            let k = charge * self.alpha;
            self.vx[i] -= fx * k;
            self.vy[i] -= fy * k;
        }
    }

    fn pull_to_centre(&mut self) {
        let k = self.forces.gravity * self.alpha;
        for i in 0..self.x.len() {
            self.vx[i] -= self.x[i] * k;
            self.vy[i] -= self.y[i] * k;
        }
    }

    /// The push on node `i` from everything else with no approximation, for checking the tree against.
    #[cfg(test)]
    fn exact_push_on(&self, i: usize) -> (f32, f32) {
        let mut f = (0.0, 0.0);
        for j in (0..self.x.len()).filter(|j| *j != i) {
            let (dx, dy) = (self.x[j] - self.x[i], self.y[j] - self.y[i]);
            let l2 = (dx * dx + dy * dy).max(1.0);
            f = (f.0 + dx * self.mass[j] / l2, f.1 + dy * self.mass[j] / l2);
        }
        f
    }
}

/// The i-th point on a sunflower spiral: evenly spread, and always the same.
fn spiral(i: usize) -> Point {
    let r = 10.0 * (i as f32 + 0.5).sqrt();
    let a = i as f32 * 2.399_963_1; // the golden angle
    pt(r * a.cos(), r * a.sin())
}

#[derive(Clone, Debug, Default)]
struct Tree {
    cells: Vec<Cell>,
}

#[derive(Clone, Copy, Debug)]
struct Cell {
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
    fn new(x: f32, y: f32, size: f32) -> Cell {
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

    fn is_leaf(&self) -> bool {
        self.kids == [NONE; 4]
    }

    fn quadrant(&self, px: f32, py: f32) -> usize {
        let h = self.size / 2.0;
        usize::from(px >= self.x + h) | (usize::from(py >= self.y + h) << 1)
    }
}

impl Tree {
    fn build(&mut self, xs: &[f32], ys: &[f32], mass: &[f32]) {
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

    fn insert(&mut self, body: u32, xs: &[f32], ys: &[f32], mass: &[f32]) {
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
    fn child(&mut self, at: usize, q: usize) -> usize {
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
    fn push_on(&self, me: usize, px: f32, py: f32, theta: f32) -> (f32, f32) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(n: usize) -> Vec<(usize, usize)> {
        (0..n).map(|i| (i, (i + 1) % n)).collect()
    }

    fn dist(l: &Layout, a: usize, b: usize) -> f32 {
        l.position(a).distance(l.position(b))
    }

    #[test]
    fn it_cools_and_stops() {
        let mut l = Layout::new(40, &ring(40));
        let steps = l.settle(2000);
        assert!(steps > 100 && steps < 1000, "{steps}");
        assert!(l.is_settled() && !l.step());
        l.reheat(0.5);
        assert!(!l.is_settled() && l.step());
    }

    #[test]
    fn the_same_graph_always_settles_the_same_way() {
        let run = || {
            let mut l = Layout::new(30, &ring(30));
            l.settle(2000);
            l.positions().collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn linked_nodes_end_up_near_each_other_and_strangers_far() {
        // Two triangles with nothing between them.
        let edges = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)];
        let mut l = Layout::new(6, &edges);
        l.settle(2000);
        let inside = (dist(&l, 0, 1) + dist(&l, 1, 2) + dist(&l, 3, 4)) / 3.0;
        let across = (dist(&l, 0, 3) + dist(&l, 1, 4) + dist(&l, 2, 5)) / 3.0;
        assert!(inside < across * 0.7, "inside {inside}, across {across}");
        assert!(
            inside > 15.0 && inside < 120.0,
            "edges settle near their length: {inside}"
        );
    }

    #[test]
    fn a_long_chain_stretches_out_and_a_star_spreads_its_leaves() {
        let chain: Vec<(usize, usize)> = (0..9).map(|i| (i, i + 1)).collect();
        let mut l = Layout::new(10, &chain);
        l.settle(2000);
        assert!(
            dist(&l, 0, 9) > 3.0 * dist(&l, 0, 1),
            "the ends are far apart"
        );
        let star: Vec<(usize, usize)> = (1..9).map(|i| (0, i)).collect();
        let mut s = Layout::new(9, &star);
        s.settle(2000);
        let mut leaves: Vec<f32> = (1..9)
            .flat_map(|a| (a + 1..9).map(move |b| (a, b)))
            .map(|(a, b)| dist(&s, a, b))
            .collect();
        leaves.sort_by(f32::total_cmp);
        assert!(
            leaves[0] > 8.0,
            "no two leaves sit on each other: {}",
            leaves[0]
        );
    }

    #[test]
    fn a_held_node_stays_put_while_the_rest_move_around_it() {
        let mut l = Layout::new(12, &ring(12));
        l.hold(0, pt(300.0, -200.0));
        l.settle(2000);
        assert_eq!(l.position(0), pt(300.0, -200.0));
        assert!(l.is_held(0));
        let near = dist(&l, 0, 1);
        assert!(near < 200.0, "neighbours are pulled along to it: {near}");
        l.release(0);
        assert!(!l.is_held(0));
    }

    #[test]
    fn keeping_warm_keeps_it_moving_until_let_go() {
        let mut l = Layout::new(10, &ring(10));
        l.settle(2000);
        l.keep_warm(0.3);
        for _ in 0..50 {
            assert!(l.step());
        }
        l.keep_warm(0.0);
        assert!(l.settle(2000) > 0 && l.is_settled());
    }

    #[test]
    fn the_tree_agrees_with_adding_everything_up() {
        let mut l = Layout::new(500, &[]);
        l.tree.build(&l.x, &l.y, &l.mass);
        for i in [0, 17, 250, 499] {
            let (ex, ey) = l.exact_push_on(i);
            let (tx, ty) = l.tree.push_on(i, l.x[i], l.y[i], 0.85);
            // Forces from all sides nearly cancel, so judge the error against the size of what is added up.
            let total: f32 = (0..l.len())
                .filter(|j| *j != i)
                .map(|j| {
                    let (dx, dy) = (l.x[j] - l.x[i], l.y[j] - l.y[i]);
                    (dx * dx + dy * dy).sqrt() * l.mass[j] / (dx * dx + dy * dy).max(1.0)
                })
                .sum();
            let off = ((ex - tx).powi(2) + (ey - ty).powi(2)).sqrt();
            assert!(off < total * 0.03, "node {i}: off by {off} of {total}");
        }
        let (zx, zy) = l.tree.push_on(3, l.x[3], l.y[3], 0.0);
        let (ex, ey) = l.exact_push_on(3);
        assert!(
            (zx - ex).abs() < 1e-3 && (zy - ey).abs() < 1e-3,
            "theta 0 is exact"
        );
    }

    #[test]
    fn nodes_on_top_of_each_other_do_not_blow_up() {
        let mut l = Layout::new(20, &[]);
        for i in 0..20 {
            l.hold(i, pt(5.0, 5.0));
            l.release(i);
        }
        l.settle(2000);
        assert!(l.positions().all(|p| p.x.is_finite() && p.y.is_finite()));
    }

    #[test]
    fn repeated_loops_and_stray_edges_are_ignored() {
        let l = Layout::new(3, &[(0, 1), (1, 0), (2, 2), (0, 9)]);
        assert_eq!((l.degree(0), l.degree(1), l.degree(2)), (1, 1, 0));
    }

    #[test]
    fn rebuilding_keeps_old_nodes_where_they_were_and_seats_new_ones_beside_their_neighbours() {
        let mut l = Layout::new(4, &ring(4));
        l.settle(2000);
        let before: Vec<Point> = l.positions().collect();
        // Node 2 goes; a new node joins to old node 0 and another to nothing.
        l.rebuild(
            5,
            &[(0, 1), (1, 2), (2, 3), (3, 0), (3, 4)],
            &[Some(0), Some(1), Some(3), None, None],
        );
        assert_eq!(l.position(0), before[0]);
        assert_eq!(l.position(1), before[1]);
        assert_eq!(l.position(2), before[3]);
        assert!(
            l.position(3).distance(before[0]) < 30.0 || l.position(3).distance(before[3]) < 30.0
        );
        assert!(!l.is_settled(), "warm enough to settle the newcomers in");
        l.settle(2000);
        assert!(l.positions().all(|p| p.x.is_finite()));
    }

    #[test]
    fn a_few_thousand_nodes_settle_quickly() {
        let n = 3000;
        let edges: Vec<(usize, usize)> = (1..n).map(|i| (i, (i * 7919) % i)).collect();
        let mut l = Layout::new(n, &edges);
        let t = std::time::Instant::now();
        for _ in 0..30 {
            l.step();
        }
        let per_step = t.elapsed().as_secs_f32() / 30.0;
        assert!(
            per_step < 0.05,
            "{per_step}s per step: a frame could not keep up"
        );
    }
}
