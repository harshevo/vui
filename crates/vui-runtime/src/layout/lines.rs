//! Flex lines: breaking items into lines, resolving how much each grows or shrinks, and placing
//! them along the main axis.

use super::*;

/// Splits items into lines. Without wrapping, or with room for everything, that is one line.
pub(crate) fn break_lines(items: &[Item], wrap: bool, room: f32, gap: f32) -> Vec<Range<usize>> {
    if items.is_empty() {
        return Vec::new();
    }
    if !wrap {
        let everything = 0..items.len();
        return vec![everything];
    }
    let mut lines = Vec::new();
    let (mut start, mut used) = (0, 0.0f32);
    for (i, it) in items.iter().enumerate() {
        let outer = it.hyp + it.m_main;
        if i == start {
            used = outer;
        } else if used + gap + outer > room + 0.01 {
            lines.push(start..i);
            start = i;
            used = outer;
        } else {
            used += gap + outer;
        }
    }
    lines.push(start..items.len());
    lines
}

/// Shares the free space of one line out by the items' grow or shrink factors, following the
/// flexbox algorithm: items that hit a limit are frozen at it and the rest share what remains.
pub(crate) fn resolve_line(line: &mut [Item], available: f32, gap: f32) {
    let gaps = gap * line.len().saturating_sub(1) as f32;
    let hypothetical: f32 = line.iter().map(|i| i.hyp + i.m_main).sum::<f32>() + gaps;
    let growing = available - hypothetical > 0.0;
    let factor = |i: &Item| if growing { i.grow } else { i.shrink };
    for it in line.iter_mut() {
        it.target = it.hyp;
        it.frozen = factor(it) <= 0.0 || (growing && it.base > it.hyp) || (!growing && it.base < it.hyp);
    }
    let claimed = |line: &[Item]| line.iter().map(|i| if i.frozen { i.target } else { i.base } + i.m_main).sum::<f32>() + gaps;
    let initial_free = available - claimed(line);
    while line.iter().any(|i| !i.frozen) {
        let mut free = available - claimed(line);
        let factors: f32 = line.iter().filter(|i| !i.frozen).map(factor).sum();
        if growing && factors < 1.0 {
            let scaled = initial_free * factors;
            if scaled.abs() < free.abs() {
                free = scaled;
            }
        }
        if free.abs() > f32::EPSILON {
            if growing {
                if factors > 0.0 {
                    for it in line.iter_mut().filter(|i| !i.frozen) {
                        it.target = it.base + free * it.grow / factors;
                    }
                }
            } else {
                let scaled: f32 = line.iter().filter(|i| !i.frozen).map(|i| i.shrink * i.base).sum();
                if scaled > 0.0 {
                    for it in line.iter_mut().filter(|i| !i.frozen) {
                        it.target = it.base + free * (it.shrink * it.base) / scaled;
                    }
                }
            }
        }
        let mut total = 0.0;
        for it in line.iter_mut().filter(|i| !i.frozen) {
            let clamped = clamp_size(it.target, it.min_main, it.max_main);
            it.violation = clamped - it.target;
            total += it.violation;
            it.target = clamped;
        }
        for it in line.iter_mut().filter(|i| !i.frozen) {
            it.frozen = total == 0.0 || (total > 0.0 && it.violation > 0.0) || (total < 0.0 && it.violation < 0.0);
        }
    }
}

/// Positions a line's items along the main axis: auto margins soak up the free space first, then
/// `justify` spreads what is left.
pub(crate) fn position_line(line: &mut [Item], room: f32, gap: f32, justify: Justify, main: Axis) {
    let n = line.len();
    let used: f32 = line.iter().map(|i| i.target + i.m_main).sum::<f32>() + gap * n.saturating_sub(1) as f32;
    let free = (room - used).max(0.0);
    let autos: usize = line.iter().map(|i| usize::from(i.auto.start(main)) + usize::from(i.auto.end(main))).sum();
    let per_auto = if autos > 0 { free / autos as f32 } else { 0.0 };
    let (mut cursor, between) = if autos > 0 {
        (0.0, 0.0)
    } else {
        match justify {
            Justify::Start => (0.0, 0.0),
            Justify::Center => (free / 2.0, 0.0),
            Justify::End => (free, 0.0),
            Justify::SpaceBetween if n > 1 => (0.0, free / (n - 1) as f32),
            Justify::SpaceBetween => (0.0, 0.0),
            Justify::SpaceAround => (free / n as f32 / 2.0, free / n as f32),
            Justify::SpaceEvenly => (free / (n + 1) as f32, free / (n + 1) as f32),
        }
    };
    for it in line.iter_mut() {
        cursor += if it.auto.start(main) { per_auto } else { it.margin.start(main) };
        it.main_pos = cursor;
        cursor += it.target;
        cursor += if it.auto.end(main) { per_auto } else { it.margin.end(main) };
        cursor += gap + between;
    }
}
