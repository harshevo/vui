//! Pagination: previous, next, and the pages around the current one.

use vui_core::{Fill, Tone};
use vui_icons::set;
use vui_runtime::{Align, Breakpoint, Builder, Div, Parent, Role, Styled, label, row};

use crate::button::{button, icon_button};

/// Pages `1..=pages` with `page` current: previous and next, the first and last, and the pages next
/// to the current one, with gaps shown as "…". `on_page` hears the page asked for.
pub fn pagination<M: Clone + 'static>(page: usize, pages: usize, on_page: impl Fn(usize) -> M + 'static) -> Div<M> {
    let pages = pages.max(1);
    let page = page.clamp(1, pages);
    let mut bar = row().gap(4.0).align_items(Align::Center).role(Role::Group).name("Pages");
    bar = bar.child(icon_button(set::CHEVRON_LEFT, "Previous page").ghost().disabled(page == 1).on_click(on_page(page.saturating_sub(1).max(1))));
    for item in window(page, pages) {
        bar = match item {
            Some(n) => {
                let b = button(n.to_string()).ghost().min_width(32.0).name(format!("Page {n}"));
                // A narrow window shows the current page between the arrows, and no others.
                let b = if n == page { b.bg(Fill::Tone(Tone::Accent, 0.12)).hover_bg(Fill::NONE) } else { b.when(Breakpoint::Compact, |s| s.hidden(true)) };
                bar.child(b.on_click(on_page(n)))
            }
            None => bar.child(label("…").color(Tone::Text3).padding_x(4.0).when(Breakpoint::Compact, |s| s.hidden(true))),
        };
    }
    bar.child(icon_button(set::CHEVRON_RIGHT, "Next page").ghost().disabled(page == pages).on_click(on_page((page + 1).min(pages))))
}

/// The page numbers to show, `None` for a gap: always the first and last, and one either side of
/// the current page.
pub(crate) fn window(page: usize, pages: usize) -> Vec<Option<usize>> {
    let mut out = Vec::new();
    let mut last = 0;
    for n in 1..=pages {
        let near = n == 1 || n == pages || n.abs_diff(page) <= 1 || (page <= 3 && n <= 4) || (page + 2 >= pages && n + 3 >= pages);
        if near {
            if n > last + 1 {
                out.push(None);
            }
            out.push(Some(n));
            last = n;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::window;

    #[test]
    fn the_window_keeps_the_ends_and_the_neighbours() {
        assert_eq!(window(1, 1), [Some(1)]);
        assert_eq!(window(1, 5), [Some(1), Some(2), Some(3), Some(4), Some(5)]);
        assert_eq!(window(6, 12), [Some(1), None, Some(5), Some(6), Some(7), None, Some(12)]);
        assert_eq!(window(1, 12), [Some(1), Some(2), Some(3), Some(4), None, Some(12)]);
        assert_eq!(window(12, 12), [Some(1), None, Some(9), Some(10), Some(11), Some(12)]);
    }
}
