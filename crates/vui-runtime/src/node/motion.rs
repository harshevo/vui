//! Time passing for a node: hover, press and focus fades, style transitions, scrolling and the
//! scrollbars, stepped by springs.

use super::*;

impl<M: 'static> Node<M> {
    /// True if anything in the subtree has an animation to run: a transition that was just started,
    /// or a widget that says so. The ticks keep it going once they have begun.
    pub fn animating(&self) -> bool {
        let a = &self.anim;
        let springs = [&a.hover, &a.press, &a.focus, &a.drop, &a.opacity];
        springs.iter().any(|s| !s.is_settled())
            || a.width.is_some()
            || a.height.is_some()
            || a.inset.iter().any(Option::is_some)
            || a.rise.is_some()
            || a.fill.is_some()
            || self.widget.animating(&*self.state)
            || self.children.iter().any(Node::animating)
    }

    /// Advances every spring and widget animation by `dt` seconds. Returns true if something in the
    /// subtree now needs a new layout.
    pub fn tick(&mut self, dt: f32, ts: &mut TickState) -> bool {
        let mut layout = false;

        advance(&mut self.anim.hover, dt, ts);
        advance(&mut self.anim.press, dt, ts);
        advance(&mut self.anim.focus, dt, ts);
        advance(&mut self.anim.drop, dt, ts);
        advance(&mut self.anim.opacity, dt, ts);
        if let Some(fill) = self.anim.fill.as_mut() {
            advance(&mut fill.t, dt, ts);
            if fill.t.is_settled() {
                self.anim.fill = None;
            }
        }

        if let Some(rise) = self.anim.rise.as_mut() {
            if advance(rise, dt, ts) {
                ts.arrange = true;
            }
            if rise.is_settled() {
                self.anim.rise = None;
                ts.arrange = true;
            }
        }

        let mut sized = false;
        let [top, right, bottom, left] = &mut self.anim.inset;
        for slot in [&mut self.anim.width, &mut self.anim.height, top, right, bottom, left] {
            if let Some(a) = slot.as_mut() {
                sized |= advance(a, dt, ts);
                if a.is_settled() {
                    *slot = None;
                    sized = true;
                }
            }
        }
        if sized {
            self.refresh_eff();
            layout = true;
        }

        if self.eff.overflow == Overflow::Scroll {
            let moved_x = advance(&mut self.scroll.x, dt, ts);
            let moved_y = advance(&mut self.scroll.y, dt, ts);
            if moved_x || moved_y {
                ts.arrange = true;
                self.scroll.poke();
            }
            self.scroll.idle += dt;
            let has_bars = self.scroll.max.x > 0.0 || self.scroll.max.y > 0.0;
            let show = has_bars && self.scroll.idle < BAR_HOLD;
            self.scroll.bars.set_target(if show { 1.0 } else { 0.0 });
            advance(&mut self.scroll.bars, dt, ts);
            advance(&mut self.scroll.thick, dt, ts);
            if show {
                ts.moving = true;
            }
        }

        let focused = ts.focus == Some(self.id);
        let mut cx = TickCx {
            env: ts.env,
            time: ts.time,
            window_focused: ts.window_focused,
            focused,
            layout: false,
            paint: false,
            wake: None,
        };
        if self.widget.tick(&mut *self.state, &mut cx, dt) {
            ts.moving = true;
            ts.paint = true;
        }
        layout |= cx.layout;
        ts.paint |= cx.paint;
        if let Some(w) = cx.wake {
            ts.wake = Some(ts.wake.map_or(w, |old| old.min(w)));
        }

        for child in &mut self.children {
            layout |= child.tick(dt, ts);
        }
        if layout {
            self.cache.clear();
            ts.layout = true;
        }
        layout
    }
}

/// Steps a spring. Returns true if its value changed.
pub(crate) fn advance(spring: &mut SpringValue, dt: f32, ts: &mut TickState) -> bool {
    let before = spring.value();
    if spring.step(dt) {
        ts.moving = true;
    }
    let changed = spring.value() != before;
    if changed {
        ts.paint = true;
    }
    changed
}

/// Blends two colours without the dark fringe that mixing straight-alpha colours gives next to
/// transparent ones.
pub(crate) fn mix(a: Color, b: Color, t: f32) -> Color {
    let (pa, pb) = (a.premultiplied(), b.premultiplied());
    let m = |i: usize| pa[i] + (pb[i] - pa[i]) * t;
    let alpha = m(3);
    if alpha <= 0.0 {
        return Color::TRANSPARENT;
    }
    Color::rgba(m(0) / alpha, m(1) / alpha, m(2) / alpha, alpha)
}

pub(crate) fn sides(s: &crate::style::Sides<Len>) -> [Len; 4] {
    [s.top, s.right, s.bottom, s.left]
}

/// Starts, steers or ends the animation of one length, given the old and new values.
pub(crate) fn retarget(current: Option<SpringValue>, old: Len, new: Len, spring: Option<Spring>) -> Option<SpringValue> {
    let (Some(spring), Len::Px(to)) = (spring, new) else {
        return None;
    };
    match (current, old) {
        (Some(mut a), _) => {
            a.spring = spring;
            a.set_target(to);
            Some(a)
        }
        (None, Len::Px(from)) if from != to => {
            let mut a = SpringValue::new(from, spring);
            a.set_target(to);
            Some(a)
        }
        _ => None,
    }
}
