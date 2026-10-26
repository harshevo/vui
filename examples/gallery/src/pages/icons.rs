//! Every icon in the set.

use super::*;

const SIZES: [f32; 4] = [16.0, 20.0, 24.0, 32.0];

fn cell(icon_: Icon) -> Div<Msg> {
    column().width(88.0).align_items(Align::Center).gap(8.0).children((
        icon(icon_).square(20.0).color(Tone::Text),
        small(icon_.name()).color(Tone::Text3).no_wrap(),
    ))
}

pub fn view(_g: &Gallery) -> Div<Msg> {
    page(
        "Icons",
        (
            section("Set", row().wrap().gap(16.0).children(icon_set::ALL.iter().map(|i| cell(*i)).collect::<Vec<_>>())),
            section(
                "Sizes",
                row().align_items(Align::Center).gap(24.0).children(SIZES.map(|s| icon(icon_set::WAVEFORM).square(s).color(Tone::Text))),
            ),
        ),
    )
}
