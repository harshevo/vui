//! A command palette: a search field over every command in the app, opened with a shortcut,
//! narrowed by typing and run with Return.

use std::rc::Rc;

use vui_core::{Shadow, Spring, Tone};
use vui_icons::{Icon, set};
use vui_runtime::{Align, Builder, Div, Element, IntoElement, Overflow, Parent, Radius, Role, Styled, column, div, pct, small};

use super::menu::{menu_heading, menu_item};
use crate::inputs::{Picker, PickerEvent, fuzzy_filter, search_field, steer};

/// One command in a [`command_palette`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Command {
    pub title: String,
    /// Commands of a group are listed together under its name while nothing has been typed.
    pub group: Option<String>,
    /// Shown at the right, such as "⌘N".
    pub shortcut: Option<String>,
    pub icon: Option<Icon>,
    /// More words it is found by, which are not shown: "settings" for "Preferences".
    pub keywords: String,
}

/// A command titled `title`: `command("New file").group("File").shortcut("⌘N").icon(icon_set::FILE)`.
pub fn command(title: impl Into<String>) -> Command {
    Command { title: title.into(), ..Command::default() }
}

impl Command {
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn keywords(mut self, words: impl Into<String>) -> Self {
        self.keywords = words.into();
        self
    }

    fn haystack(&self) -> String {
        if self.keywords.is_empty() { self.title.clone() } else { format!("{} {}", self.title, self.keywords) }
    }
}

/// The commands that match the query, best first, as indices into `commands`. While nothing has been
/// typed they keep their order, grouped by `group` in the order the groups first appear.
fn matches(query: &str, commands: &[Command]) -> Vec<usize> {
    if query.trim().is_empty() {
        let mut groups: Vec<Option<&str>> = Vec::new();
        for c in commands {
            if !groups.contains(&c.group.as_deref()) {
                groups.push(c.group.as_deref());
            }
        }
        return groups.iter().flat_map(|g| (0..commands.len()).filter(move |&i| commands[i].group.as_deref() == *g)).collect();
    }
    let hay: Vec<String> = commands.iter().map(Command::haystack).collect();
    fuzzy_filter(query, hay.iter().map(String::as_str))
}

/// Every command in `commands`, in a card near the top of the window over a dimmed page, while
/// `state.open`. Typing narrows them, Up and Down move the highlight, Return or a click runs one, and
/// Escape or a press outside closes the palette. Open it from a shortcut with
/// `Picker::apply(PickerEvent::Open(true))`; run the command whose index [`Picker::apply`] returns,
/// then [`Picker::reset`].
///
/// ```
/// use vui_widgets::{Picker, PickerEvent, command, command_palette};
///
/// #[derive(Clone)]
/// enum Msg { Palette(PickerEvent) }
///
/// let commands = [command("New file").group("File").shortcut("⌘N"), command("Toggle dark mode").group("View")];
/// let palette = command_palette(&Picker { open: true, ..Picker::new() }, &commands, Msg::Palette);
/// ```
pub fn command_palette<M: Clone + 'static>(state: &Picker, commands: &[Command], on: impl Fn(PickerEvent) -> M + 'static) -> Div<M> {
    if !state.open {
        return div().hidden(true);
    }
    let on: Rc<dyn Fn(PickerEvent) -> M> = Rc::new(on);
    let found = matches(&state.query, commands);
    let active = state.active.min(found.len().saturating_sub(1));
    let typed = on.clone();
    let steering: Vec<(usize, String)> = found.iter().map(|&i| (i, commands[i].title.clone())).collect();
    let field = search_field(state.query.clone())
        .pass_keys()
        .placeholder("Type a command")
        .autofocus()
        .large()
        .on_change(move |q| typed(PickerEvent::Query(q)));

    let grouped = state.query.trim().is_empty();
    let mut rows: Vec<Element<M>> = Vec::new();
    let mut last_group: Option<Option<&str>> = None;
    for (n, &i) in found.iter().enumerate() {
        let c = &commands[i];
        if grouped && last_group != Some(c.group.as_deref()) {
            if let Some(g) = &c.group {
                rows.push(menu_heading(g.clone()).into_element());
            }
            last_group = Some(c.group.as_deref());
        }
        let mut item = menu_item(c.title.clone()).on_click(on(PickerEvent::Pick(i, c.title.clone())));
        item = item.icon(c.icon.unwrap_or(set::CHEVRON_RIGHT));
        if let Some(keys) = &c.shortcut {
            item = item.detail(keys.clone());
        }
        rows.push(if n == active { item.bg(Tone::Hover).into_element() } else { item.into_element() });
    }
    if found.is_empty() {
        rows.push(small("No commands match").color(Tone::Text3).padding(12.0).into_element());
    }

    let card = column::<M>()
        .width(560.0)
        .max_width(pct(94.0))
        .bg(Tone::Bg)
        .border(1.0, Tone::Line)
        .rounded(Radius::Panel)
        .shadow(Shadow::FLOATING)
        .role(Role::Dialog)
        .name("Command palette")
        .trap_focus()
        .on_dismiss(on(PickerEvent::Open(false)))
        .on_key(steer(on.clone(), true, active, steering))
        .transition(Spring::SNAPPY)
        .appear()
        .children((
            div().padding(8.0).child(field.fill_width()),
            div().height(1.0).bg(Tone::Line),
            column().padding(6.0).max_height(360.0).overflow(Overflow::Scroll).role(Role::List).children(rows),
        ));
    div()
        .key("vui-command-palette")
        .fixed()
        .top(0.0)
        .left(0.0)
        .right(0.0)
        .bottom(0.0)
        .padding_top(88.0)
        .padding_x(8.0)
        .bg(Tone::Scrim)
        .align_items(Align::Center)
        .child(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_no_query_commands_are_grouped_in_order() {
        let commands = [command("a").group("X"), command("b").group("Y"), command("c").group("X"), command("d")];
        assert_eq!(matches("", &commands), vec![0, 2, 1, 3]);
    }

    #[test]
    fn keywords_find_commands_without_showing() {
        let commands = [command("Preferences").keywords("settings options"), command("Quit")];
        assert_eq!(matches("settings", &commands), vec![0]);
    }
}
