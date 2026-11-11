//! Things that open over the page: a dropdown menu, a select, a context menu, a dialog, toasts and
//! tooltips.

use super::*;
use crate::{Overlay, Sort};

const SORTS: [(Sort, &str); 3] = [(Sort::Newest, "Newest first"), (Sort::Oldest, "Oldest first"), (Sort::Name, "By name")];

/// The sort choices, then two commands.
fn sort_menu(g: &Gallery) -> Vec<Element<Msg>> {
    let mut items = vec![menu_heading("Sort").into_element()];
    items.extend(SORTS.map(|(s, name)| menu_check(name, g.sort == s).key(name).on_click(Msg::Sort(s)).into_element()));
    items.push(menu_separator().into_element());
    items.push(menu_item("Copy link").icon(icon_set::COPY).detail("⌘L").key("copy").on_click(Msg::Open(None)).into_element());
    items.push(
        menu_item("Delete").icon(icon_set::TRASH).text_color(Tone::Danger).icon_color(Tone::Danger).on_click(Msg::Open(None)).into_element(),
    );
    items
}

pub fn view(g: &Gallery) -> Div<Msg> {
    let open = |o: Overlay| g.overlay == Some(o);
    let toggle = |o: Overlay| Msg::Open(if open(o) { None } else { Some(o) });
    let sort_name = SORTS.iter().find(|(s, _)| *s == g.sort).map_or("", |(_, n)| n);
    page(
        "Overlays",
        (
            section(
                "Dropdown menu",
                flow().children((
                    dropdown(
                        button(sort_name).key("sort").trailing(icon_set::CHEVRON_DOWN).on_click(toggle(Overlay::Menu)),
                        open(Overlay::Menu),
                        Msg::Open(None),
                        sort_menu(g),
                    ),
                    captioned("Opens under its button, closes on a choice, a press outside or Escape", spacer()),
                )),
            ),
            section(
                "Select",
                column().width(240.0).child(
                    select(
                        [(1, "Compact".to_string()), (2, "Comfortable".to_string()), (3, "Spacious".to_string())],
                        &g.density,
                        open(Overlay::Select),
                        |o| Msg::Open(o.then_some(Overlay::Select)),
                        Msg::Density,
                    )
                    .key("density"),
                ),
            ),
            section(
                "Context menu",
                context_menu(
                    card().key("canvas").height(96.0).align_items(Align::Center).justify(Justify::Center).child(label("Right-click here").color(Tone::Text3)),
                    g.context,
                    Msg::Context,
                    Msg::CloseContext,
                    (
                        menu_item("Cut").detail("⌘X").on_click(Msg::CloseContext),
                        menu_item("Copy").detail("⌘C").on_click(Msg::CloseContext),
                        menu_item("Paste").detail("⌘V").on_click(Msg::CloseContext),
                    ),
                ),
            ),
            section(
                "Dialog, toast and tooltips",
                flow().children((
                    button("Delete note…").danger().key("ask").on_click(Msg::Open(Some(Overlay::Dialog))),
                    button("Save").key("save").on_click(Msg::Toast),
                    icon_button(icon_set::COPY, "Copy").key("copy-tip").tooltip("Copy to the clipboard"),
                    icon_button(icon_set::SEARCH, "Search").tooltip("Search everything (⌘K)"),
                )),
            ),
            open(Overlay::Dialog).then(|| {
                dialog(
                    Msg::Open(None),
                    (
                        title("Delete this note?"),
                        label("It moves to the Trash, where you can bring it back for 30 days.").color(Tone::Text2),
                        dialog_actions((
                            button("Cancel").key("cancel").on_click(Msg::Open(None)),
                            button("Delete").danger().key("delete").on_click(Msg::Open(None)),
                        )),
                    ),
                )
            }),
            toasts(g.toasts.iter().map(|&n| toast(format!("Saved · version {n}"), Some(icon_set::CHECK)).key(n as u64)).collect::<Vec<_>>()),
        ),
    )
}
