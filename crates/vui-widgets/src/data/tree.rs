//! Trees: nested rows that fold and unfold, such as files in folders.

use vui_core::{CursorIcon, Fill, Spring, Tone};
use vui_icons::{Icon, set};
use vui_runtime::{Align, Builder, Div, Focus, FocusGroup, IntoChildren, Parent, Role, Styled, column, div, label, row};

use crate::icon::icon;

/// Indent for each level.
const INDENT: f32 = 16.0;

/// The rows of a tree, one focus group: Up and Down move between rows.
pub fn tree<M: 'static>(rows: impl IntoChildren<M>) -> Div<M> {
    column().gap(1.0).role(Role::List).focus_group(FocusGroup::vertical()).children(rows)
}

/// One row of a [`tree`] at nesting `depth`. `expanded` is `Some` for a row that has children, which
/// gets a chevron; `on_toggle` folds or unfolds it, as do Left and Right. The app lists the rows of
/// unfolded branches after their parent.
pub fn tree_row<M: Clone + 'static>(text: impl Into<String>, depth: usize, expanded: Option<bool>, on_toggle: Option<M>) -> TreeRow<M> {
    TreeRow { text: text.into(), depth, expanded, on_toggle, icon: None, selected: false, on_click: None, key: None }
}

/// See [`tree_row`].
pub struct TreeRow<M: 'static> {
    text: String,
    depth: usize,
    expanded: Option<bool>,
    on_toggle: Option<M>,
    icon: Option<Icon>,
    selected: bool,
    on_click: Option<M>,
    key: Option<vui_runtime::Key>,
}

impl<M: Clone + 'static> TreeRow<M> {
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(mut self, msg: M) -> Self {
        self.on_click = Some(msg);
        self
    }

    pub fn key(mut self, key: impl Into<vui_runtime::Key>) -> Self {
        self.key = Some(key.into());
        self
    }
}

impl<M: Clone + 'static> vui_runtime::IntoElement<M> for TreeRow<M> {
    fn into_element(self) -> vui_runtime::Element<M> {
        let TreeRow { text, depth, expanded, on_toggle, icon: lead, selected, on_click, key } = self;
        let chevron = match expanded {
            Some(open) => {
                let c = icon(if open { set::CHEVRON_DOWN } else { set::CHEVRON_RIGHT }).square(14.0).color(Tone::Text3);
                let c = div().square(18.0).align_items(Align::Center).justify(vui_runtime::Justify::Center).rounded(vui_runtime::Radius::Px(4.0)).hover_bg(Tone::Hover).child(c);
                match on_toggle.clone() {
                    Some(m) => c.on_click(m),
                    None => c,
                }
            }
            None => div().square(18.0),
        };
        let fold = on_toggle.clone();
        let keys = move |k: &vui_core::KeyEvent| {
            use vui_core::{Key, NamedKey};
            let open = expanded?;
            match k.key {
                Key::Named(NamedKey::ArrowRight) if k.pressed && !open => fold.clone(),
                Key::Named(NamedKey::ArrowLeft) if k.pressed && open => fold.clone(),
                _ => None,
            }
        };
        let mut r = row()
            .height(28.0)
            .padding_left(4.0 + INDENT * depth as f32)
            .padding_right(8.0)
            .gap(4.0)
            .align_items(Align::Center)
            .rounded(vui_runtime::Radius::Px(6.0))
            .bg(if selected { Fill::Tone(Tone::Accent, 0.12) } else { Fill::NONE })
            .hover_bg(if selected { Fill::NONE } else { Fill::from(Tone::Hover) })
            .transition(Spring::SNAPPY)
            .focus_mode(Focus::Keyboard)
            .role(Role::ListItem)
            .name(text.clone())
            .cursor(CursorIcon::Pointer)
            .on_key(keys)
            .children((
                chevron,
                lead.map(|i| icon(i).square(16.0).color(if selected { Tone::Accent } else { Tone::Text2 })),
                label(text).color(if selected { Tone::Text } else { Tone::Text2 }).max_lines(1),
            ));
        if let Some(m) = on_click.or(on_toggle) {
            r = r.on_click(m);
        }
        if let Some(k) = key {
            r = r.key(k);
        }
        vui_runtime::IntoElement::into_element(r)
    }
}
