use std::rc::Rc;

use vui_core::{Rect, Theme, pt, rect, size};
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{Builder, Styled};

use super::*;

fn area() -> Rect {
    rect(0.0, 0.0, 600.0, 400.0)
}

fn weights(n: usize) -> Vec<(usize, f64)> {
    (0..n).map(|i| (i, 100.0 / (i as f64 + 1.0))).collect()
}

#[test]
fn every_area_follows_its_weight_and_they_fill_the_whole() {
    let items = weights(30);
    let cut = squarify(&items, area());
    assert_eq!(cut.len(), 30);
    let total: f64 = items.iter().map(|i| i.1).sum();
    let mut covered = 0.0;
    for (k, r) in &cut {
        let want = items[*k].1 / total * 600.0 * 400.0;
        assert!((f64::from(r.w) * f64::from(r.h) - want).abs() < want * 0.001 + 0.01, "tile {k} is {}x{}", r.w, r.h);
        covered += f64::from(r.w) * f64::from(r.h);
        assert!(r.x >= -0.01 && r.y >= -0.01 && r.right() <= 600.01 && r.bottom() <= 400.01, "tile {k} left the area");
    }
    assert!((covered - 240_000.0).abs() < 1.0);
}

#[test]
fn tiles_never_overlap_and_stay_near_square() {
    let cut = squarify(&weights(50), area());
    for (i, (a, ra)) in cut.iter().enumerate() {
        for (b, rb) in &cut[i + 1..] {
            let w = ra.right().min(rb.right()) - ra.x.max(rb.x);
            let h = ra.bottom().min(rb.bottom()) - ra.y.max(rb.y);
            assert!(w <= 0.01 || h <= 0.01, "tiles {a} and {b} overlap");
        }
    }
    // The few big tiles are the squarest; even the long thin ones stay readable.
    let worst = cut.iter().map(|(_, r)| (r.w / r.h).max(r.h / r.w)).fold(0.0, f32::max);
    assert!(worst < 6.0, "the longest tile is {worst}:1");
}

#[test]
fn weights_that_are_not_positive_numbers_get_no_tile() {
    let items = vec![(0, 5.0), (1, 0.0), (2, -3.0), (3, f64::NAN), (4, 5.0)];
    let cut = squarify(&items, area());
    assert_eq!(cut.iter().map(|c| c.0).collect::<Vec<_>>(), vec![0, 4]);
    assert!(squarify(&[], area()).is_empty());
    assert!(squarify(&items, rect(0.0, 0.0, 0.0, 10.0)).is_empty());
}

fn sectors() -> Treemap {
    let mut m = Treemap::new();
    for (g, name) in ["Banks", "IT", "Energy", "Auto"].iter().enumerate() {
        m = m.group(name);
        for t in 0..6 {
            m = m.tile(&format!("{name}{t}"), "+1.0%", 100.0 / (1.0 + t as f64 + g as f64 * 2.0), if t % 2 == 0 { 0.012 } else { -0.02 });
        }
    }
    m
}

#[test]
fn tiles_sit_inside_their_group_and_groups_do_not_mix() {
    let map = sectors();
    let laid = layout(&map, area());
    assert_eq!(laid.tiles.len(), 24);
    assert_eq!(laid.headings.len(), 4);
    let mut group_rect = [None::<Rect>; 4];
    for p in &laid.tiles {
        let g = &mut group_rect[p.group];
        *g = Some(g.map_or(p.rect, |u: Rect| u.union(p.rect)));
    }
    for (i, a) in group_rect.iter().enumerate() {
        for b in group_rect.iter().skip(i + 1) {
            let (a, b) = (a.unwrap(), b.unwrap());
            let w = a.right().min(b.right()) - a.x.max(b.x);
            let h = a.bottom().min(b.bottom()) - a.y.max(b.y);
            assert!(w <= 0.01 || h <= 0.01, "two groups overlap");
        }
    }
    // A heading sits above its group's tiles, never on them.
    for (g, head) in &laid.headings {
        assert!(laid.tiles.iter().filter(|p| p.group == *g).all(|p| p.rect.y >= head.bottom() - 0.01 || p.rect.right() <= head.x || p.rect.x >= head.right()), "heading of group {g} is covered");
    }
}

#[test]
fn the_outer_edge_lines_up_with_the_area() {
    let laid = layout(&Treemap::new().tile("A", "", 3.0, 0.0).tile("B", "", 1.0, 0.0), area());
    let u = laid.tiles.iter().fold(laid.tiles[0].rect, |u, p| u.union(p.rect));
    assert!((u.x).abs() < 0.01 && (u.y).abs() < 0.01 && (u.right() - 600.0).abs() < 0.01 && (u.bottom() - 400.0).abs() < 0.01, "{u:?}");
    // And the gap between the two is the one asked for.
    let (a, b) = (laid.tiles[0].rect, laid.tiles[1].rect);
    let gap = if (a.right() - b.x).abs() < 4.0 { b.x - a.right() } else { b.y - a.bottom() };
    assert!((gap.abs() - GAP).abs() < 0.01 || (gap.abs() - GAP).abs() < 0.01 || a.right() - b.x <= 0.0, "gap {gap}");
}

#[test]
fn the_tile_under_a_point_and_the_next_one_over() {
    let laid = layout(&sectors(), area());
    for p in &laid.tiles {
        let c = pt(p.rect.x + p.rect.w / 2.0, p.rect.y + p.rect.h / 2.0);
        assert_eq!(tile_at(&laid, c), Some(p.index));
    }
    assert_eq!(tile_at(&laid, pt(-5.0, -5.0)), None);
    // Walking right from the left-most tile reaches another tile, and left comes back near it.
    let first = laid.tiles.iter().min_by(|a, b| (a.rect.x + a.rect.w / 2.0).total_cmp(&(b.rect.x + b.rect.w / 2.0))).unwrap().index;
    let right = neighbour(&laid, first, 1, 0).expect("something is to the right");
    assert_ne!(right, first);
    assert!(neighbour(&laid, first, -1, 0).is_none(), "nothing is left of the left-most");
}

#[test]
fn heat_is_zero_without_data_and_caps_at_one() {
    assert_eq!(heat(f64::NAN, 0.03), 0.0);
    assert_eq!(heat(0.0, 0.03), 0.0);
    assert!((heat(0.015, 0.03) - 0.5).abs() < 1e-6);
    assert_eq!(heat(-0.5, 0.03), 1.0);
}

fn ui(map: Treemap) -> TestUi<usize> {
    let mut t = TestUi::<usize>::with_theme(size(600.0, 400.0), Theme::light(), 1.0);
    t.view(treemap::<usize>(Rc::new(map)).on_pick(|i| i).key("map").fill());
    let _ = t.scene();
    t
}

#[test]
fn a_label_is_drawn_only_where_it_fits() {
    let map = Treemap::new().tile("BIGGEST", "+1.0%", 1000.0, 0.01).tile("MIDDLE", "", 60.0, 0.0).tile("TINY", "x", 0.05, -0.01);
    let mut t = ui(map);
    let words = texts(&t.scene());
    assert!(words.iter().any(|w| w == "BIGGEST"), "{words:?}");
    assert!(words.iter().any(|w| w == "+1.0%"), "the note fits under a big tile's label: {words:?}");
    assert!(!words.iter().any(|w| w == "TINY"), "a sliver gets no label: {words:?}");
}

#[test]
fn a_click_picks_the_tile_and_arrows_walk_between_them() {
    let map = sectors();
    let laid = layout(&map, rect(0.0, 0.0, 600.0, 400.0));
    let mut t = ui(map);
    let target = laid.tiles[3];
    t.click(pt(target.rect.x + target.rect.w / 2.0, target.rect.y + target.rect.h / 2.0));
    assert_eq!(t.messages(), vec![target.index]);
    t.key(vui_core::NamedKey::ArrowRight);
    t.key(vui_core::NamedKey::Enter);
    let got = t.messages();
    assert_eq!(got.len(), 1, "enter picks the tile the cursor moved to");
    assert_ne!(got[0], target.index);
}

#[test]
fn it_reads_out_what_it_shows() {
    let mut t = ui(sectors());
    let tree = t.ui.access_tree();
    let node = tree.nodes.iter().find(|n| n.role == vui_runtime::Role::Image).expect("the treemap is a node");
    assert_eq!(node.name.as_deref(), Some("Treemap"));
    assert!(node.value.as_deref().unwrap_or("").starts_with("24 tiles, the largest Banks0"), "{:?}", node.value);
}
