//! Skeletons: grey shapes where content will be, shimmering while it loads.

use vui_core::Tone;
use vui_runtime::{Builder, Parent, Radius, Styled};

use crate::effects::{Shimmer, shimmer};

/// A block `width` by `height` standing in for content that is loading. Round it to match what it
/// stands for: `.rounded(Radius::Pill)` for an avatar, the default for a line of text.
pub fn skeleton<M: 'static>(width: impl Into<vui_runtime::style::Len>, height: f32) -> Shimmer<M> {
    let mut s = shimmer(true).color(vui_core::Fill::Tone(Tone::Text, 0.06)).height(height).bg(Tone::Wash).rounded(Radius::Px(6.0));
    s.style_mut().width = width.into();
    s.a11y_hidden(true)
}

/// Lines of text loading: `lines` bars, the last one shorter.
pub fn skeleton_text<M: 'static>(lines: usize) -> vui_runtime::Div<M> {
    vui_runtime::column().gap(8.0).children(
        (0..lines.max(1)).map(|i| skeleton(vui_runtime::pct(if i + 1 == lines.max(1) && lines > 1 { 60.0 } else { 100.0 }), 12.0)).collect::<Vec<_>>(),
    )
}
