//! Force-directed layout: nodes push each other apart, edges pull their ends together, and a faint
//! pull toward the middle keeps separate clusters from drifting away. Repulsion uses a Barnes-Hut
//! quadtree, so a step costs about n·log n and graphs of several thousand nodes stay smooth.
//!
//! The layout is a small simulation you step: [`Layout::step`] moves everything a little and says
//! whether it is still moving. It cools as it goes, can be warmed again, and nodes can be held in place
//! (which is how dragging one works). Starting positions are a spiral, so the same graph always settles
//! the same way.

use vui_core::{Point, pt};

mod tree;

use tree::*;

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

#[cfg(test)]
mod tests;
