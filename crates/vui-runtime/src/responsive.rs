//! Responsive layout: styles that change with the window's width.
//!
//! An element can carry rules that restyle it while the window is in a range of widths, as a media
//! query does in CSS:
//!
//! ```
//! use vui_runtime::{Breakpoint, Builder, Styled, column, pct};
//!
//! // Full width on a small window; 15% of room on each side once it is wide.
//! let timeline = column::<()>().when(Breakpoint::Wide, |s| s.margin_x(pct(15.0)));
//! ```
//!
//! The UI resolves the rules itself, when the element is built and whenever the window is resized,
//! so the app does not track the window's size or rebuild its view for it. A rule only touches the
//! style properties it sets; everything else comes from the element's own style, whatever order the
//! calls were made in. When several rules match, later ones win.

use crate::style::{Style, Styled};

/// A range of window widths in logical pixels: from `min`, up to but not including `max`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Width {
    pub min: f32,
    pub max: f32,
}

impl Width {
    /// Windows `w` wide or wider.
    pub const fn at_least(w: f32) -> Width {
        Width { min: w, max: f32::INFINITY }
    }

    /// Windows narrower than `w`.
    pub const fn below(w: f32) -> Width {
        Width { min: 0.0, max: w }
    }

    /// Windows at least `min` and narrower than `max`.
    pub const fn between(min: f32, max: f32) -> Width {
        Width { min, max }
    }

    pub fn contains(self, width: f32) -> bool {
        width >= self.min && width < self.max
    }
}

/// The kit's standard window sizes. Compact is a narrow window or one beside others, Medium a
/// typical one, Wide one with room to spare.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Breakpoint {
    /// Narrower than 600.
    Compact,
    /// 600 up to 900.
    Medium,
    /// 900 and wider.
    Wide,
}

impl Breakpoint {
    /// Where Medium starts.
    pub const MEDIUM: f32 = 600.0;
    /// Where Wide starts.
    pub const WIDE: f32 = 900.0;

    /// The breakpoint a window this wide falls in.
    pub fn of(width: f32) -> Breakpoint {
        if width >= Breakpoint::WIDE {
            Breakpoint::Wide
        } else if width >= Breakpoint::MEDIUM {
            Breakpoint::Medium
        } else {
            Breakpoint::Compact
        }
    }

    /// The widths this breakpoint covers.
    pub const fn widths(self) -> Width {
        match self {
            Breakpoint::Compact => Width::below(Breakpoint::MEDIUM),
            Breakpoint::Medium => Width::between(Breakpoint::MEDIUM, Breakpoint::WIDE),
            Breakpoint::Wide => Width::at_least(Breakpoint::WIDE),
        }
    }

    /// This breakpoint and every wider one.
    pub const fn up(self) -> Width {
        Width::at_least(self.widths().min)
    }

    /// Every breakpoint narrower than this one.
    pub const fn down(self) -> Width {
        Width::below(self.widths().min)
    }
}

impl From<Breakpoint> for Width {
    fn from(b: Breakpoint) -> Width {
        b.widths()
    }
}

/// What a rule's closure is given: the element's style so far, with the whole [`Styled`]
/// vocabulary. Only the properties the closure changes end up in the rule.
pub struct Restyle(Style);

impl Styled for Restyle {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.0
    }
}

/// One rule: while the window's width is in `when`, the properties in `set` take their values
/// from `values`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rule {
    pub when: Width,
    set: u32,
    values: Style,
}

/// Every property a rule can set, in one list so recording and applying stay in step.
macro_rules! properties {
    ($($field:ident),+ $(,)?) => {
        impl Rule {
            /// The rule that turns `before` into `after`, or `None` when nothing changed.
            pub fn between(when: Width, before: &Style, after: Style) -> Option<Rule> {
                let mut set = 0u32;
                let mut bit = 1u32;
                $(
                    if before.$field != after.$field {
                        set |= bit;
                    }
                    bit <<= 1;
                )+
                let _ = bit;
                (set != 0).then_some(Rule { when, set, values: after })
            }

            /// Writes the rule's properties into `style`.
            pub fn apply(&self, style: &mut Style) {
                let mut bit = 1u32;
                $(
                    if self.set & bit != 0 {
                        style.$field = self.values.$field;
                    }
                    bit <<= 1;
                )+
                let _ = bit;
            }
        }
    };
}

properties!(
    direction, hidden, position, width, height, min_width, min_height, max_width, max_height, aspect_ratio, padding, margin, inset, gap,
    wrap, align_items, align_self, justify, grow, shrink, basis, overflow, follow_end, float,
);

/// An element's own style with the rules that match a window `width` wide applied over it.
pub fn resolve(style: Style, rules: &[Rule], width: f32) -> Style {
    let mut out = style;
    for rule in rules.iter().filter(|r| r.when.contains(width)) {
        rule.apply(&mut out);
    }
    out
}

/// Records a rule for a builder: runs `f` on the style so far and keeps what it changed.
pub(crate) fn record(rules: &mut Vec<Rule>, style: &Style, when: Width, f: impl FnOnce(Restyle) -> Restyle) {
    let after = f(Restyle(*style)).0;
    rules.extend(Rule::between(when, style, after));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Direction, Len, pct};

    #[test]
    fn breakpoints_cover_every_width_once() {
        assert_eq!(Breakpoint::of(0.0), Breakpoint::Compact);
        assert_eq!(Breakpoint::of(599.0), Breakpoint::Compact);
        assert_eq!(Breakpoint::of(600.0), Breakpoint::Medium);
        assert_eq!(Breakpoint::of(899.5), Breakpoint::Medium);
        assert_eq!(Breakpoint::of(900.0), Breakpoint::Wide);
        for w in [0.0, 300.0, 600.0, 750.0, 900.0, 2400.0] {
            let all = [Breakpoint::Compact, Breakpoint::Medium, Breakpoint::Wide];
            assert_eq!(all.iter().filter(|b| b.widths().contains(w)).count(), 1, "{w}");
            assert!(Breakpoint::of(w).widths().contains(w));
        }
        assert!(Breakpoint::Medium.up().contains(1600.0) && !Breakpoint::Medium.up().contains(599.0));
        assert!(Breakpoint::Wide.down().contains(899.0) && !Breakpoint::Wide.down().contains(900.0));
    }

    #[test]
    fn a_rule_keeps_only_what_it_changed() {
        let mut base = Style::default();
        base.gap.x = 4.0;
        let mut rules = Vec::new();
        record(&mut rules, &base, Width::at_least(900.0), |s| s.margin_x(pct(15.0)).row());
        record(&mut rules, &base, Width::below(600.0), |s| s);
        assert_eq!(rules.len(), 1, "a rule that changes nothing is dropped");

        // The element's style changes after the rule was recorded: the rule leaves those alone.
        base.gap.x = 12.0;
        let wide = resolve(base, &rules, 1200.0);
        assert_eq!((wide.margin.left, wide.margin.right), (Len::Pct(15.0), Len::Pct(15.0)));
        assert_eq!(wide.direction, Direction::Row);
        assert_eq!(wide.gap.x, 12.0);
        assert_eq!(resolve(base, &rules, 899.0), base);
    }

    #[test]
    fn later_rules_win() {
        let base = Style::default();
        let mut rules = Vec::new();
        record(&mut rules, &base, Breakpoint::Medium.up(), |s| s.grow(1.0));
        record(&mut rules, &base, Breakpoint::Wide.up(), |s| s.grow(2.0));
        assert_eq!(resolve(base, &rules, 700.0).grow, 1.0);
        assert_eq!(resolve(base, &rules, 1000.0).grow, 2.0);
        assert_eq!(resolve(base, &rules, 500.0).grow, 0.0);
    }
}
