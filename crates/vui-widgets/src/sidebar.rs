//! The sidebar and the lists in it: navigation items, section headings and two-line rows.
//!
//! A sidebar is a column on the side colour. Its items sit in a [`list`]: one Tab stop that the arrow
//! keys move through, as in a source list everywhere else.

use vui_core::{Fill, Spring, Tone};
use vui_icons::Icon;
use vui_runtime::{
    Base, Builder, Div, Element, Focus, FocusGroup, IntoElement, Label, Parent, Radius, Role, Style, Styled, builder, column, label,
    row, small,
};

use crate::display::section_title;
use crate::item::{Height, ItemWidget, Place, Spec};

/// How wide a sidebar is unless told otherwise.
pub const SIDEBAR_WIDTH: f32 = 240.0;

/// A column on the side colour, 240 wide. Put a [`list`] in it, and a [`separator_vertical`]
/// between it and the content.
///
/// [`separator_vertical`]: crate::separator_vertical
pub fn sidebar<M: 'static>() -> Div<M> {
    column().width(SIDEBAR_WIDTH).shrink(0.0).padding(12.0).gap(16.0).bg(Tone::Side)
}

/// A column of items that is one Tab stop: Tab lands on the item that had focus (or the one marked
/// `tab_default`), and the arrow keys, Home and End move between items.
pub fn list<M: 'static>() -> Div<M> {
    column().gap(2.0).focus_group(FocusGroup::vertical()).role(Role::List)
}

/// A small grey heading over a group of items, lined up with the items' icons.
pub fn nav_heading<M: 'static>(text: impl Into<String>) -> Label<M> {
    section_title(text).padding_x(12.0).padding_y(4.0)
}

/// A heading with its items close under it: `nav_section("Recent").child(list().children(items))`.
/// Sections sit a [`sidebar`] gap apart, so a heading is nearer its own items than the group above.
pub fn nav_section<M: 'static>(heading: impl Into<String>) -> Div<M> {
    column().gap(2.0).child(nav_heading(heading))
}

// ---- navigation items ------------------------------------------------------------------------

/// A line in the sidebar: an icon and a name, and a count if it has one.
pub struct NavItem<M: 'static> {
    base: Base<M>,
    widget: ItemWidget<M>,
}

builder!(NavItem<M> => ItemWidget<M>);

impl<M: 'static> NavItem<M> {
    /// Marks the item for the page that is open: darker text on a grey wash.
    pub fn selected(mut self, selected: bool) -> Self {
        let spec = &mut self.widget.spec;
        spec.selected = Some(selected);
        spec.weight = if selected { 500 } else { 400 };
        spec.label_color = if selected { Tone::Text } else { Tone::Text2 }.into();
        spec.icon_color = if selected { Tone::Text } else { Tone::Text2 }.into();
        // Under the pointer the open page keeps its wash instead of lightening to the hover tint.
        self.bg(if selected { Fill::from(Tone::Wash) } else { Fill::NONE }).hover_bg(if selected { Tone::Wash } else { Tone::Hover })
    }

    /// Small grey text at the end of the line: an unread count.
    pub fn detail(mut self, text: impl Into<String>) -> Self {
        self.widget.spec.detail = text.into();
        self
    }

    pub fn trailing(mut self, icon: Icon) -> Self {
        self.widget.spec.trailing = Some(icon);
        self
    }
}

pub fn nav_item<M: 'static>(icon: Icon, label: impl Into<String>) -> NavItem<M> {
    let mut spec = Spec::new(label, Role::ListItem);
    spec.icon = Some(icon);
    spec.place = Place::Start;
    spec.height = Height::Control;
    spec.focus = Focus::Keyboard;
    let item = NavItem { base: Base::new(), widget: ItemWidget::new(spec) };
    item.padding_x(12.0).rounded(Radius::Control).hover_bg(Tone::Hover).press_bg(Tone::Wash).transition(Spring::SNAPPY).selected(false)
}

// ---- rows ------------------------------------------------------------------------------------

/// A row of a list: something at the start (an icon, an avatar, a picture), a title with a line under
/// it, and something small at the end such as a time.
pub struct ListRow<M: 'static> {
    row: Div<M>,
    title: String,
    subtitle: String,
    leading: Option<Element<M>>,
    trailing: String,
}

impl<M: 'static> Styled for ListRow<M> {
    fn style_mut(&mut self) -> &mut Style {
        self.row.style_mut()
    }
}

impl<M: 'static> Builder<M> for ListRow<M> {
    fn base_mut(&mut self) -> &mut Base<M> {
        self.row.base_mut()
    }
}

impl<M: 'static> ListRow<M> {
    /// The line under the title, in secondary text.
    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = text.into();
        self
    }

    pub fn leading(mut self, element: impl IntoElement<M>) -> Self {
        self.leading = Some(element.into_element());
        self
    }

    /// Small grey text at the end of the row.
    pub fn trailing(mut self, text: impl Into<String>) -> Self {
        self.trailing = text.into();
        self
    }

    /// Marks the row for what is open: a grey wash.
    pub fn selected(mut self, selected: bool) -> Self {
        let hover = if selected { Tone::Wash } else { Tone::Hover };
        self.row = self.row.bg(if selected { Fill::from(Tone::Wash) } else { Fill::NONE }).hover_bg(hover);
        self
    }
}

impl<M: 'static> IntoElement<M> for ListRow<M> {
    fn into_element(self) -> Element<M> {
        let mut text = column().grow(1.0).min_width(0.0).child(label(self.title).color(Tone::Text).max_lines(1));
        if !self.subtitle.is_empty() {
            text = text.child(small(self.subtitle).color(Tone::Text2).max_lines(1));
        }
        let mut row = self.row;
        if let Some(leading) = self.leading {
            row = row.child(leading);
        }
        row = row.child(text);
        if !self.trailing.is_empty() {
            row = row.child(small(self.trailing).color(Tone::Text3).no_wrap().shrink(0.0));
        }
        row.into_element()
    }
}

/// A row for a [`list`]. Give it `on_click` to make it open something.
pub fn list_row<M: 'static>(title: impl Into<String>) -> ListRow<M> {
    let title = title.into();
    let row = row()
        .align_items(vui_runtime::Align::Center)
        .gap(12.0)
        .padding_x(12.0)
        .padding_y(8.0)
        .rounded(Radius::Control)
        .hover_bg(Tone::Hover)
        .press_bg(Tone::Wash)
        .transition(Spring::SNAPPY)
        .focus_mode(Focus::Keyboard)
        .role(Role::ListItem)
        .name(title.clone());
    ListRow { row, title, subtitle: String::new(), leading: None, trailing: String::new() }
}
