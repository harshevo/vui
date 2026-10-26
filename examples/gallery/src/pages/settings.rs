//! A settings window: titles on the left, controls on the right, thin lines between.

use super::*;
use crate::Mode;

fn group(rows: Vec<Element<Msg>>) -> Div<Msg> {
    let mut out = column::<Msg>();
    let last = rows.len().saturating_sub(1);
    for (i, row) in rows.into_iter().enumerate() {
        out = out.child(row);
        if i != last {
            out = out.child(separator());
        }
    }
    out
}

pub fn view(g: &Gallery) -> Div<Msg> {
    let volume = row().align_items(Align::Center).gap(12.0).children((
        slider(g.volume, 0.0, 100.0).key("volume").name("Volume").width(160.0).on_change(Msg::Volume),
        small(format!("{:.0}", g.volume)).color(Tone::Text2).width(24.0),
    ));
    page(
        "Settings",
        (
            section(
                "General",
                group(vec![
                    setting("Launch at login", "", toggle(g.login).key("login").on_change(Msg::Login)).into_element(),
                    setting("Appearance", "", segmented(Mode::ALL.map(|m| (m, m.name().to_string())), &g.mode, Msg::Mode)).into_element(),
                    setting("Volume", "", volume).into_element(),
                    setting("Name", "", text_field(&g.name).width(200.0).on_change(Msg::Name)).into_element(),
                ]),
            ),
            section(
                "Voice",
                group(vec![
                    setting("Push to talk", "Hold to speak", row().gap(4.0).children((kbd("⌃"), kbd("Space")))).into_element(),
                    setting("Alerts", "When a task finishes", toggle(g.alerts).on_change(Msg::Alerts)).into_element(),
                ]),
            ),
            section(
                "Memory",
                group(vec![
                    setting("Erase memory", "Removes everything it remembers", button("Erase").danger().key("erase").on_click(Msg::Erase)).into_element(),
                ]),
            ),
        ),
    )
}
