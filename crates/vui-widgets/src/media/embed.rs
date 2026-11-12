//! The widget behind every native view: it takes the space, paints a placeholder (an icon and what
//! it shows, in a quiet frame) and asks the backend for the native view over it.

use std::sync::atomic::{AtomicU64, Ordering};

use vui_core::{EmbedKind, Quad, Size, TextStyle, Tone, pt, rect, size};
use vui_icons::{Icon, Weight};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Dirty, Env, PaintCx, Role, Widget};

use crate::paint_icon;

/// Ids for native views, unique for the life of the process.
static NEXT: AtomicU64 = AtomicU64::new(1);

/// A native view's placeholder and request.
pub struct EmbedWidget {
    pub(crate) kind: EmbedKind,
    pub(crate) icon: Icon,
    /// What the placeholder says it shows: a host name, a file name.
    pub(crate) label: String,
    /// Its size when nothing else decides it; the width also sets the aspect ratio.
    pub(crate) natural: Size,
    /// The accessibility role.
    pub(crate) role: Role,
}

/// The id that names the native view while the element is on screen.
pub struct EmbedState {
    pub id: u64,
}

impl<M: 'static> Widget<M> for EmbedWidget {
    type State = EmbedState;

    fn create_state(&self) -> EmbedState {
        EmbedState { id: NEXT.fetch_add(1, Ordering::Relaxed) }
    }

    fn update(&self, old: &EmbedWidget, _state: &mut EmbedState) -> Dirty {
        if self.natural != old.natural {
            Dirty::Layout
        } else if self.kind != old.kind || self.label != old.label || self.icon != old.icon {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    /// Fills the width it is given, at its natural aspect ratio, unless sized.
    fn measure(&self, _state: &mut EmbedState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let ratio = self.natural.w / self.natural.h.max(1.0);
        match (known.x, known.y) {
            (Some(w), Some(h)) => size(w, h),
            (Some(w), None) => size(w, w / ratio),
            (None, Some(h)) => size(h * ratio, h),
            (None, None) => {
                let w = match avail.x {
                    Avail::Definite(w) => w,
                    _ => self.natural.w,
                };
                size(w, w / ratio)
            }
        }
    }

    fn paint(&self, state: &EmbedState, cx: &mut PaintCx) {
        let c = cx.content;
        let (radii, smooth) = (cx.radii, cx.smooth);
        let (wash, line) = (cx.color(Tone::Wash), cx.color(Tone::Line));
        cx.quad(Quad::new(c).fill(wash).radii(radii).smooth(smooth).border(1.0, line));
        let icon = 24.0_f32.min(c.h * 0.4);
        let style = TextStyle::new(12.0, 16.0).weight(500);
        let label = (!self.label.is_empty() && c.w > 48.0).then(|| cx.fonts().layout_cached(&self.label, &style, Some(c.w - 24.0)));
        let text_h = label.as_ref().map_or(0.0, |l| l.size().h.min(16.0) + 8.0);
        let top = c.y + (c.h - icon - text_h) / 2.0;
        if icon >= 8.0 {
            paint_icon(cx, self.icon, cx.snap(rect(c.x + (c.w - icon) / 2.0, top, icon, icon)), Tone::Text3, Weight::Regular);
        }
        if let Some(label) = label {
            let w = label.size().w;
            let colour = cx.color(Tone::Text2);
            cx.text(label, pt((c.x + (c.w - w) / 2.0).round(), (top + icon + 8.0).round()), colour);
        }
        cx.embed(state.id, self.kind.clone());
    }

    fn access(&self, _state: &EmbedState, info: &mut AccessInfo) {
        info.role = Some(self.role);
        if info.label.is_none() {
            info.label = Some(self.label.clone());
        }
    }
}

/// The last part of a path or URL, without a query: `report.pdf`, `clip.mp4`.
pub(crate) fn file_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url).trim_end_matches('/');
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// A URL's host, for a web page's placeholder: `example.com`.
pub(crate) fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    if host.is_empty() { file_name(url) } else { host.trim_start_matches("www.").to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_name_what_they_show() {
        assert_eq!(host("https://www.example.com/a/b?q=1"), "example.com");
        assert_eq!(host("example.org"), "example.org");
        assert_eq!(host("file:///Users/me/page.html"), "page.html");
        assert_eq!(file_name("https://cdn.test/v/clip.mp4?token=x"), "clip.mp4");
        assert_eq!(file_name("/Users/me/Report.pdf"), "Report.pdf");
        assert_eq!(file_name(r"C:\docs\a.pdf"), "a.pdf");
    }
}
