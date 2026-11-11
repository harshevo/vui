//! Switches, checkboxes, radio buttons, segments, sliders (single, range, marked and upright) and progress.

use super::*;
use crate::{Choice, Mode};

pub fn view(g: &Gallery) -> Div<Msg> {
    page(
        "Controls",
        (
            section(
                "Switch",
                flow().gap(32.0).children((
                    toggle(g.wifi).label("Wi-Fi").key("wifi").on_change(Msg::Wifi),
                    toggle(g.alerts).label("Alerts").key("alerts").on_change(Msg::Alerts),
                    toggle(true).label("Disabled").disabled(true),
                )),
            ),
            section(
                "Checkbox",
                flow().gap(32.0).children((
                    checkbox(g.agree).label("Select all").key("agree").on_change(Msg::Agree),
                    checkbox(true).label("Checked"),
                    checkbox(false).label("Unchecked"),
                    checkbox(true).label("Disabled").disabled(true),
                )),
            ),
            section(
                "Radio",
                radio_group(Choice::ALL.map(|c| (c, c.name().to_string())), &g.choice, Msg::Choice).key("choice").name("Thoroughness"),
            ),
            section(
                "Segmented",
                segmented(Mode::ALL.map(|m| (m, m.name().to_string())), &g.mode, Msg::Mode).key("mode").name("Appearance"),
            ),
            section(
                "Slider",
                column().gap(20.0).width(320.0).children((
                    slider(g.volume, 0.0, 100.0).key("volume").name("Volume").on_change(Msg::Volume),
                    slider(g.level, 0.0, 10.0).step(1.0).key("level").name("Level").on_change(Msg::Level),
                    slider(60.0, 0.0, 100.0).name("Disabled").disabled(true),
                )),
            ),
            section(
                "Range, marks and values",
                column().gap(28.0).width(320.0).children((
                    captioned(
                        &format!("${:.0} to ${:.0}", g.price.0, g.price.1),
                        range_slider(g.price.0, g.price.1, 0.0, 500.0).step(10.0).gap(20.0).show_value().format(|v| format!("${v:.0}")).key("price").name("Price").fill_width().on_change(Msg::Price),
                    ),
                    slider(g.level, 0.0, 10.0).step(1.0).marks().show_value().key("marked").name("Marked level").on_change(Msg::Level),
                )),
            ),
            section(
                "Upright",
                row().gap(28.0).children(
                    ["60", "250", "1k", "4k", "12k"]
                        .iter()
                        .enumerate()
                        .map(|(i, hz)| {
                            column().gap(8.0).align_items(Align::Center).children((
                                slider(g.bands[i], -12.0, 12.0).step(1.0).vertical().show_value().format(|v| format!("{v:+.0} dB")).key(format!("band{i}")).name(format!("{hz} Hz")).height(140.0).on_change(move |v| Msg::Band(i, v)),
                                small(*hz).color(Tone::Text3),
                            ))
                        })
                        .collect::<Vec<_>>(),
                ),
            ),
            section(
                "Progress",
                column().gap(20.0).width(320.0).children((
                    progress(0.4).name("Download"),
                    progress(1.0).name("Done"),
                    progress_indeterminate().name("Working"),
                    spinner().square(20.0).name("Loading"),
                )),
            ),
        ),
    )
}
