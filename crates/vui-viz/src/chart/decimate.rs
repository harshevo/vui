//! Thinning a long series to what a screen can show, without losing the peaks. A year of one-second
//! prices is millions of points, but a chart 800 pixels wide can only show the lowest and highest of
//! each pixel column, and a line through those looks exactly like the full one.

use crate::scale::Linear;

/// Keeps, for each pixel column of `x`, the first, lowest, highest and last point, in their original
/// order. Short series come back as they are.
pub fn extremes(points: &[(f64, f64)], x: &Linear) -> Vec<(f64, f64)> {
    let columns = (x.range.1 - x.range.0).abs().max(1.0) as usize;
    if points.len() <= columns * 4 {
        return points.to_vec();
    }
    let column = |p: &(f64, f64)| (x.map(p.0) - x.range.0.min(x.range.1)).floor() as i64;
    let mut out = Vec::with_capacity(columns * 4);
    let mut i = 0;
    while i < points.len() {
        let c = column(&points[i]);
        let mut j = i;
        let (mut lo, mut hi) = (i, i);
        while j + 1 < points.len() && column(&points[j + 1]) == c {
            j += 1;
            if points[j].1 < points[lo].1 {
                lo = j;
            }
            if points[j].1 > points[hi].1 {
                hi = j;
            }
        }
        let mut keep = [i, lo, hi, j];
        keep.sort_unstable();
        for (n, &k) in keep.iter().enumerate() {
            if n == 0 || keep[n - 1] != k {
                out.push(points[k]);
            }
        }
        i = j + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_series_are_untouched() {
        let pts: Vec<(f64, f64)> = (0..50).map(|i| (f64::from(i), f64::from(i % 7))).collect();
        let x = Linear::new((0.0, 49.0), (0.0, 400.0));
        assert_eq!(extremes(&pts, &x), pts);
    }

    #[test]
    fn a_dense_series_keeps_its_spikes_and_the_first_and_last_point() {
        let n = 100_000;
        let mut pts: Vec<(f64, f64)> = (0..n)
            .map(|i| (f64::from(i), (f64::from(i) / 300.0).sin()))
            .collect();
        pts[41_234].1 = 9.0;
        pts[77_777].1 = -9.0;
        let x = Linear::new((0.0, f64::from(n - 1)), (0.0, 500.0));
        let thin = extremes(&pts, &x);
        assert!(thin.len() <= 500 * 4, "{}", thin.len());
        assert!(
            thin.iter().any(|p| p.1 == 9.0) && thin.iter().any(|p| p.1 == -9.0),
            "spikes survive"
        );
        assert_eq!(thin.first(), pts.first());
        assert_eq!(thin.last(), pts.last());
        assert!(
            thin.windows(2).all(|w| w[0].0 < w[1].0),
            "still in order, no repeats"
        );
    }
}
