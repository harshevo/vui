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
