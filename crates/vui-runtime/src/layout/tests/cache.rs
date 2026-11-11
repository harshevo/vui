//! The measuring cache, checked on hand-made trees and on random ones.

use super::*;

fn tree_with_text() -> T {
    col().kids(vec![row().gap(8.0).kids(vec![txt("hello world"), txt("abc")]), txt("a longer line of text"), col().kids(vec![natural(30.0, 20.0)])])
}

#[test]
fn laying_out_again_asks_nothing_new() {
    let mut root = tree_with_text();
    let first = run(&mut root, 300.0, 200.0);
    assert!(first.measures > 0);
    let again = run(&mut root, 300.0, 200.0);
    assert_eq!(again.measures, 0, "every question had already been answered");
}

#[test]
fn clearing_the_caches_up_a_path_picks_up_a_change() {
    let mut root = tree_with_text();
    run(&mut root, 300.0, 200.0);
    let before = root.kids[0].kids[0].layout.size.w;
    assert!(close(before, 110.0), "{before}");
    root.kids[0].kids[0].text = Some("hi");
    root.cache.clear();
    root.kids[0].cache.clear();
    root.kids[0].kids[0].cache.clear();
    run(&mut root, 300.0, 200.0);
    assert!(close(root.kids[0].kids[0].layout.size.w, 20.0), "{}", root.kids[0].kids[0].layout.size.w);
}

/// A tree that doubles at every level: `2^levels` text leaves under alternating rows and columns.
fn balanced(levels: u32) -> T {
    if levels == 0 {
        return txt("ab cd");
    }
    let pair = vec![balanced(levels - 1), balanced(levels - 1)];
    if levels.is_multiple_of(2) { row().gap(2.0).kids(pair) } else { col().gap(2.0).kids(pair) }
}

fn measures_for(levels: u32) -> usize {
    let mut root = balanced(levels);
    run(&mut root, 4000.0, 4000.0).measures
}

#[test]
fn measuring_cost_grows_with_the_number_of_nodes_not_the_depth() {
    // 16 times the leaves should cost about 16 times the measures. Measuring that is exponential in
    // the depth would cost far more.
    let (shallow, deeper) = (measures_for(6), measures_for(10));
    assert!(deeper < shallow * 16 * 3, "{shallow} measures at 64 leaves, {deeper} at 1024");
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    fn pick<V: Copy>(&mut self, options: &[V]) -> V {
        options[self.below(options.len() as u64) as usize]
    }
}

fn random_tree(rng: &mut Rng, depth: u32) -> T {
    let mut t = if depth == 0 || rng.chance(25) {
        match rng.below(3) {
            0 => txt(rng.pick(&["a", "hello world", "the quick brown fox", "abcdefghij klm", ""])),
            1 => natural(rng.below(60) as f32, rng.below(40) as f32),
            _ => T::new(),
        }
    } else {
        let n = 1 + rng.below(4);
        T::new().kids((0..n).map(|_| random_tree(rng, depth - 1)).collect())
    };
    if rng.chance(50) {
        t = t.row();
    }
    if rng.chance(30) {
        t = t.gap(rng.below(12) as f32);
    }
    if rng.chance(30) {
        t = t.padding(rng.below(12) as f32);
    }
    if rng.chance(20) {
        t = t.wrap();
    }
    if rng.chance(30) {
        t = t.width(rng.below(120) as f32);
    }
    if rng.chance(30) {
        t = t.height(rng.below(80) as f32);
    }
    if rng.chance(15) {
        t = t.width(pct(rng.below(100) as f32));
    }
    if rng.chance(15) {
        t = t.max_width(rng.below(150) as f32);
    }
    if rng.chance(15) {
        t = t.min_height(rng.below(50) as f32);
    }
    if rng.chance(30) {
        t = t.grow(rng.below(3) as f32);
    }
    if rng.chance(20) {
        t = t.shrink(rng.below(3) as f32);
    }
    if rng.chance(20) {
        t = t.margin(rng.below(8) as f32);
    }
    if rng.chance(10) {
        t = t.margin_left_auto();
    }
    if rng.chance(10) {
        t = t.aspect_ratio(0.5 + rng.below(4) as f32 * 0.5);
    }
    if rng.chance(8) {
        t = t.hidden(true);
    }
    if rng.chance(8) {
        t = t.absolute().top(rng.below(10) as f32).left(rng.below(10) as f32);
    }
    if rng.chance(10) {
        t = t.overflow(Overflow::Scroll);
    }
    t = t.align_items(rng.pick(&[Align::Start, Align::Center, Align::End, Align::Stretch, Align::Baseline]));
    t.justify(rng.pick(&[Justify::Start, Justify::Center, Justify::End, Justify::SpaceBetween, Justify::SpaceAround, Justify::SpaceEvenly]))
}

fn same(a: &T, b: &T) -> bool {
    a.layout == b.layout && a.kids.len() == b.kids.len() && a.kids.iter().zip(&b.kids).all(|(x, y)| same(x, y))
}

fn sane(t: &T) -> bool {
    let l = t.layout;
    let finite = [l.origin.x, l.origin.y, l.size.w, l.size.h, l.content.w, l.content.h].iter().all(|v| v.is_finite());
    finite && l.size.w >= 0.0 && l.size.h >= 0.0 && t.kids.iter().all(sane)
}

#[test]
fn random_trees_lay_out_to_finite_sizes() {
    for seed in 1..=300u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut root = random_tree(&mut rng, 5);
        run(&mut root, 320.0, 240.0);
        assert!(sane(&root), "seed {seed}");
    }
}

#[test]
fn an_answer_from_the_cache_matches_a_fresh_one() {
    for seed in 1..=300u64 {
        let build = || random_tree(&mut Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1), 5);
        let mut worn = build();
        // Lay out at other sizes first, so the caches hold answers to other questions, then at the
        // size that is compared.
        run(&mut worn, 200.0, 500.0);
        run(&mut worn, 640.0, 120.0);
        let repeat = run(&mut worn, 320.0, 240.0).measures;
        let mut fresh = build();
        run(&mut fresh, 320.0, 240.0);
        assert!(same(&worn, &fresh), "seed {seed}: layout depends on what was asked before");
        let again = run(&mut worn, 320.0, 240.0).measures;
        assert_eq!(again, 0, "seed {seed}: asking the same thing twice measured {again} times (first {repeat})");
    }
}

#[test]
fn clearing_every_cache_changes_nothing() {
    for seed in 1..=100u64 {
        let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
        let mut root = random_tree(&mut rng, 5);
        run(&mut root, 320.0, 240.0);
        let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
        let mut other = random_tree(&mut rng, 5);
        run(&mut other, 320.0, 240.0);
        other.clear_caches();
        run(&mut other, 320.0, 240.0);
        assert!(same(&root, &other), "seed {seed}");
    }
}
