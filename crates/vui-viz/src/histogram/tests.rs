use super::*;
use vui_core::{Rect, rect};

fn area() -> Rect {
    rect(0.0, 0.0, 216.0, 56.0)
}

#[test]
fn values_land_in_their_bin_and_missing_ones_are_skipped() {
    let h = Histogram::new(&[0.5, 1.5, 1.6, 2.5, f64::NAN, f64::INFINITY], 3, 0.0, 3.0);
    assert_eq!(h.counts, vec![1, 2, 1]);
    assert_eq!(h.total(), 4);
}

#[test]
fn the_extremes_fold_into_the_end_bins() {
    let h = Histogram::new(&[-100.0, 0.1, 9.9, 500.0], 4, 0.0, 10.0);
    assert_eq!(h.counts, vec![2, 0, 0, 2]);
}

#[test]
fn of_trims_outliers_from_the_range_but_still_counts_them() {
    let mut values: Vec<f64> = (1..=100).map(f64::from).collect();
    values.push(100_000.0);
    let h = Histogram::of(&values, 10);
    assert!(h.max < 200.0, "one absurd value does not stretch the range to {}", h.max);
    assert_eq!(h.total(), 101, "it still counts");
    assert!(h.counts[9] >= 2, "in the last bin");
}

#[test]
fn of_copes_with_nothing_and_with_one_value() {
    let h = Histogram::of(&[], 8);
    assert_eq!((h.total(), h.counts.len()), (0, 8));
    let h = Histogram::of(&[7.0, 7.0, 7.0], 8);
    assert!(h.max > h.min && h.total() == 3);
    assert_eq!(Histogram::of(&[f64::NAN], 4).total(), 0);
}

#[test]
fn the_lit_part_follows_the_chosen_range() {
    let h = Histogram::new(&[0.5, 1.5, 2.5, 3.5], 4, 0.0, 4.0).lit(1.0, 3.0);
    assert_eq!(h.lit_count(), 2);
    let all = Histogram::new(&[0.5, 1.5], 4, 0.0, 4.0);
    assert_eq!(all.lit_count(), 2, "everything is lit until a range is chosen");
    assert_eq!(h.clone().lit(3.0, 1.0), h, "the ends can be given either way round");
}

#[test]
fn bars_line_up_with_the_slider_and_scale_to_the_biggest() {
    let h = Histogram::new(&[0.5, 0.6, 0.7, 1.5, 2.5, 2.6], 3, 0.0, 3.0);
    let p = plot(area());
    assert_eq!((p.x, p.right()), (INSET, 216.0 - INSET));
    let (a, b, c) = (bar(&h, area(), 0), bar(&h, area(), 1), bar(&h, area(), 2));
    assert!((a.h - 56.0).abs() < 0.01, "the biggest bin is full height");
    assert!((b.h - 56.0 / 3.0).abs() < 0.01 && (c.h - 56.0 * 2.0 / 3.0).abs() < 0.01);
    assert!(a.bottom() == 56.0 && b.bottom() == 56.0, "they stand on one line");
    // The bars tile the plot with a small gap between neighbours and none at the ends' outside.
    assert!((b.x - a.right() - 2.0).abs() < 0.01);
    assert!((a.x - (INSET + 1.0)).abs() < 0.01);
}

#[test]
fn a_bin_with_something_in_it_is_never_invisible() {
    let mut counts = vec![0u32; 5];
    counts[0] = 100_000;
    counts[1] = 1;
    let h = Histogram { counts, min: 0.0, max: 5.0, low: 0.0, high: 5.0, marker: None };
    assert!(bar(&h, area(), 1).h >= 2.0);
    assert_eq!(bar(&h, area(), 2).h, 0.0);
}

#[test]
fn a_position_names_its_bin() {
    let h = Histogram::new(&[], 4, 0.0, 4.0);
    assert_eq!(bin_at(&h, area(), 0.0), None, "the inset is not a bar");
    assert_eq!(bin_at(&h, area(), INSET + 1.0), Some(0));
    assert_eq!(bin_at(&h, area(), 216.0 - INSET - 1.0), Some(3));
    assert_eq!(bin_at(&h, area(), 215.0), None);
}

#[test]
fn the_marker_is_clamped_to_the_range() {
    let h = Histogram::new(&[], 4, 0.0, 4.0).marker(Some(9.0));
    assert_eq!(h.fraction(h.marker.unwrap()), 1.0);
    assert_eq!(Histogram::new(&[], 4, 0.0, 4.0).marker(Some(f64::NAN)).marker, None);
}
