//! Treemaps: a set of things drawn as tiles whose areas are their weights, grouped under headings
//! and coloured by a second number (a day's move, a score). [`layout`] places the tiles with the
//! squarified method, which keeps them near square so that labels fit; [`tile_at`] answers which tile
//! is under a point. The [`treemap`] widget adds hover, click and arrow-key movement.

mod widget;

#[cfg(test)]
mod tests;

pub use widget::{TreemapView, treemap};

use vui_core::{Point, Rect, rect};

/// The visible space between neighbouring tiles, in points.
pub const GAP: f32 = 2.0;
/// The space between groups, and the height of a group's heading.
pub const GROUP_GAP: f32 = 8.0;
pub const HEADING: f32 = 18.0;
/// A group is given a heading only when it is at least this wide and tall.
const HEADED: (f32, f32) = (72.0, 52.0);

#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub label: String,
    /// The line under the label: "+1.2%", "₹2.1 lakh cr".
    pub note: String,
    pub weight: f64,
    /// What the colour says. NaN is drawn plain, as "no data".
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub label: String,
    pub tiles: Vec<Tile>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Treemap {
    pub groups: Vec<Group>,
    /// The value at which the colour is as strong as it gets.
    pub cap: f64,
    pub gap: f32,
}

impl Default for Treemap {
    fn default() -> Treemap {
        Treemap::new()
    }
}

impl Treemap {
    pub fn new() -> Treemap {
        Treemap { groups: Vec::new(), cap: 0.03, gap: GAP }
    }

    /// Starts a group: the tiles added next sit under this heading.
    pub fn group(mut self, label: &str) -> Treemap {
        self.groups.push(Group { label: label.to_string(), tiles: Vec::new() });
        self
    }

    /// Adds a tile to the latest group (an unnamed one is made if there is none).
    pub fn tile(mut self, label: &str, note: &str, weight: f64, value: f64) -> Treemap {
        if self.groups.is_empty() {
            self.groups.push(Group { label: String::new(), tiles: Vec::new() });
        }
        if let Some(g) = self.groups.last_mut() {
            g.tiles.push(Tile { label: label.to_string(), note: note.to_string(), weight, value });
        }
        self
    }

    pub fn cap(mut self, value: f64) -> Treemap {
        self.cap = if value.is_finite() && value > 0.0 { value } else { 0.03 };
        self
    }

    pub fn gap(mut self, points: f32) -> Treemap {
        self.gap = points.clamp(0.0, 8.0);
        self
    }

    /// How many tiles there are, drawn or not.
    pub fn len(&self) -> usize {
        self.groups.iter().map(|g| g.tiles.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Tile `i` in the order they were added.
    pub fn get(&self, i: usize) -> Option<&Tile> {
        self.groups.iter().flat_map(|g| g.tiles.iter()).nth(i)
    }

    /// The tile numbers in a group, in the order added.
    fn numbers(&self) -> Vec<Vec<usize>> {
        let mut at = 0;
        self.groups
            .iter()
            .map(|g| {
                let v = (at..at + g.tiles.len()).collect();
                at += g.tiles.len();
                v
            })
            .collect()
    }
}

fn counts(weight: f64) -> bool {
    weight.is_finite() && weight > 0.0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// The tile's number in the order added.
    pub index: usize,
    pub group: usize,
    pub rect: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Laid {
    pub tiles: Vec<Placed>,
    /// Each headed group's heading: the group number and where its label goes.
    pub headings: Vec<(usize, Rect)>,
}

/// Cuts `area` into one rectangle per item, each with an area in proportion to its weight, as near
/// square as the squarified method can make them. Items that are not a positive number are left out.
/// Rectangles come back biggest first.
pub fn squarify(items: &[(usize, f64)], area: Rect) -> Vec<(usize, Rect)> {
    let mut items: Vec<(usize, f64)> = items.iter().copied().filter(|(_, w)| counts(*w)).collect();
    if items.is_empty() || area.w <= 0.0 || area.h <= 0.0 {
        return Vec::new();
    }
    items.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let total: f64 = items.iter().map(|i| i.1).sum();
    let scale = f64::from(area.w) * f64::from(area.h) / total;
    let areas: Vec<(usize, f64)> = items.iter().map(|(k, w)| (*k, w * scale)).collect();
    let (mut x, mut y, mut w, mut h) = (f64::from(area.x), f64::from(area.y), f64::from(area.w), f64::from(area.h));
    let mut out = Vec::with_capacity(areas.len());
    let mut i = 0;
    while i < areas.len() {
        let side = w.min(h);
        let mut end = i + 1;
        let mut sum = areas[i].1;
        let mut best = worst(&areas[i..end], sum, side);
        while end < areas.len() {
            let more = sum + areas[end].1;
            let now = worst(&areas[i..=end], more, side);
            if now > best {
                break;
            }
            sum = more;
            best = now;
            end += 1;
        }
        if w >= h {
            // A column down the left side.
            let cw = sum / h;
            let mut cy = y;
            for (k, a) in &areas[i..end] {
                let ch = a / cw;
                out.push((*k, rect(x as f32, cy as f32, cw as f32, ch as f32)));
                cy += ch;
            }
            x += cw;
            w -= cw;
        } else {
            // A row along the top.
            let rh = sum / w;
            let mut cx = x;
            for (k, a) in &areas[i..end] {
                let cw = a / rh;
                out.push((*k, rect(cx as f32, y as f32, cw as f32, rh as f32)));
                cx += cw;
            }
            y += rh;
            h -= rh;
        }
        i = end;
    }
    out
}

/// The worst (largest) ratio of long side to short side among the rectangles of a row.
fn worst(row: &[(usize, f64)], sum: f64, side: f64) -> f64 {
    let (mut max, mut min) = (f64::MIN, f64::MAX);
    for (_, a) in row {
        max = max.max(*a);
        min = min.min(*a);
    }
    let s2 = side * side;
    (s2 * max / (sum * sum)).max(sum * sum / (s2 * min))
}

/// Places every tile with a weight inside `area`: the groups first, then the tiles in each group's
/// room under its heading. Tiles that would be under a point wide or tall are left out.
pub fn layout(map: &Treemap, area: Rect) -> Laid {
    let mut laid = Laid::default();
    let numbers = map.numbers();
    let named = map.groups.iter().any(|g| !g.label.is_empty());
    let weights: Vec<(usize, f64)> = map.groups.iter().enumerate().map(|(gi, g)| (gi, g.tiles.iter().map(|t| t.weight).filter(|w| counts(*w)).sum())).collect();
    let half = map.gap / 2.0;
    let group_half = if named { GROUP_GAP / 2.0 } else { 0.0 };
    // Grown by half a gap, so the tiles' own insets leave the outer edge where `area` ends.
    let whole = area.inflate(half, half).inflate(group_half, group_half);
    let mut groups = squarify(&weights, whole);
    groups.sort_by_key(|g| g.0);
    for (gi, outer) in groups {
        let mut body = outer.inflate(-group_half, -group_half);
        if named && !map.groups[gi].label.is_empty() && body.w >= HEADED.0 && body.h >= HEADED.1 {
            laid.headings.push((gi, rect(body.x + half, body.y, body.w - half * 2.0, HEADING)));
            body = rect(body.x, body.y + HEADING, body.w, body.h - HEADING);
        }
        let items: Vec<(usize, f64)> = numbers[gi].iter().map(|n| (*n, map.get(*n).map_or(0.0, |t| t.weight))).collect();
        for (index, r) in squarify(&items, body) {
            let r = r.inflate(-half, -half);
            if r.w >= 1.0 && r.h >= 1.0 {
                laid.tiles.push(Placed { index, group: gi, rect: r });
            }
        }
    }
    laid.tiles.sort_by_key(|p| p.index);
    laid
}

/// The tile under `p`, if any.
pub fn tile_at(laid: &Laid, p: Point) -> Option<usize> {
    laid.tiles.iter().find(|t| t.rect.contains(p)).map(|t| t.index)
}

/// The tile nearest to `from` in a direction (`dx`, `dy` one of -1, 0, 1), for arrow keys: tiles
/// beyond it on that side, the nearest first, with a sideways step costing double.
pub fn neighbour(laid: &Laid, from: usize, dx: i32, dy: i32) -> Option<usize> {
    let here = laid.tiles.iter().find(|t| t.index == from)?.rect;
    let (cx, cy) = (here.x + here.w / 2.0, here.y + here.h / 2.0);
    laid.tiles
        .iter()
        .filter(|t| t.index != from)
        .filter_map(|t| {
            let (tx, ty) = (t.rect.x + t.rect.w / 2.0, t.rect.y + t.rect.h / 2.0);
            let (along, across) = if dx != 0 { ((tx - cx) * dx as f32, (ty - cy).abs()) } else { ((ty - cy) * dy as f32, (tx - cx).abs()) };
            (along > 0.5).then_some((t.index, along + across * 2.0))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|t| t.0)
}

/// How strongly a value colours its tile, 0 to 1.
pub fn heat(value: f64, cap: f64) -> f32 {
    if value.is_finite() { (value.abs() / cap).clamp(0.0, 1.0) as f32 } else { 0.0 }
}
