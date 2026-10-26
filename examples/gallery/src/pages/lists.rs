//! The sidebar's items, list rows, badges and keys.

use super::*;

const ROWS: [(&str, &str, &str); 3] = [
    ("Trip to Lisbon", "Flights, hotel and a table for two", "9:41"),
    ("Rust ownership", "Notes from the borrowing chapter", "Mon"),
    ("Invoice 1042", "Sent to Studio Nord, due in 14 days", "Sun"),
];

/// A real sidebar, boxed so it reads as a sample on the page.
fn nav() -> Div<Msg> {
    sidebar().border(1.0, Tone::Line).rounded(Radius::Card).children((
        list().children((
            nav_item(icon_set::SPARKLES, "Ask").selected(true),
            nav_item(icon_set::CLOCK, "History").detail("12"),
            nav_item(icon_set::LIBRARY, "Memory"),
            nav_item(icon_set::CUBE, "Plugins"),
        )),
        nav_section("Recent").child(list().children((
            nav_item(icon_set::MESSAGE, "Trip to Lisbon"),
            nav_item(icon_set::MESSAGE, "Rust ownership"),
        ))),
    ))
}

fn rows(g: &Gallery) -> Div<Msg> {
    column().width(360.0).gap(2.0).key("rows").children(
        ROWS.iter()
            .enumerate()
            .map(|(i, (name, detail, time))| {
                list_row(*name)
                    .key(*name)
                    .subtitle(*detail)
                    .trailing(*time)
                    .leading(avatar(name))
                    .selected(g.row == i)
                    .on_click(Msg::Row(i))
            })
            .collect::<Vec<_>>(),
    )
}

pub fn view(g: &Gallery) -> Div<Msg> {
    page(
        "Lists",
        (
            section("Sidebar", row().gap(32.0).align_items(Align::Start).children((nav(), rows(g)))),
            section(
                "Badges",
                flow().gap(16.0).children((
                    badge("Neutral"),
                    badge("Live").accent(),
                    badge("Failed").danger(),
                    row().gap(8.0).align_items(Align::Center).children((dot(Tone::Accent), small("Listening").color(Tone::Text2))),
                )),
            ),
            section(
                "Keys",
                flow().gap(16.0).children((
                    row().gap(4.0).children((kbd("⌃"), kbd("Space"))),
                    row().gap(4.0).children((kbd("⌘"), kbd("K"))),
                    kbd("Esc"),
                )),
            ),
        ),
    )
}
