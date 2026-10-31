//! Type, colour, space and corners: the tokens everything else is made of.

use super::*;
use vui::core::Shadow;
use vui::widgets::glow;

/// Greys: the surfaces, then the text on them.
const NEUTRALS: [(&str, Tone); 8] = [
    ("Bg", Tone::Bg),
    ("Side", Tone::Side),
    ("Hover", Tone::Hover),
    ("Wash", Tone::Wash),
    ("Line", Tone::Line),
    ("Text 3", Tone::Text3),
    ("Text 2", Tone::Text2),
    ("Text", Tone::Text),
];

/// The only two colours that are not grey.
const SIGNALS: [(&str, Tone); 2] = [("Accent", Tone::Accent), ("Danger", Tone::Danger)];

const SPACES: [f32; 8] = [4.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0];

fn swatch(name: &str, tone: Tone) -> Div<Msg> {
    captioned(name, div().size(64.0, 40.0).rounded(Radius::Control).bg(tone).border(1.0, Tone::Line))
}

fn spec(text: &str) -> Label<Msg> {
    small(text).color(Tone::Text3).width(88.0)
}

fn type_row(name: &str, sample: impl IntoElement<Msg>) -> Div<Msg> {
    row().align_items(Align::Baseline).gap(24.0).children((spec(name), sample))
}

pub fn view(_g: &Gallery) -> Div<Msg> {
    page(
        "Foundations",
        (
            section(
                "Type",
                column().gap(16.0).children((
                    type_row("Title 24", title("Quiet by default")),
                    type_row("Body 14", label("Speak, and it listens")),
                    type_row("Medium 14", label("Remembers what you read").weight(500)),
                    type_row("Small 12", small("Updated two minutes ago").color(Tone::Text2)),
                    type_row("Mono 12", label("cargo test --workspace").mono()),
                )),
            ),
            section(
                "Colour",
                column().gap(16.0).children((
                    flow().gap(16.0).children(NEUTRALS.map(|(name, tone)| swatch(name, tone))),
                    flow().gap(16.0).children(SIGNALS.map(|(name, tone)| swatch(name, tone))),
                )),
            ),
            section(
                "Space",
                row().align_items(Align::End).gap(16.0).children(SPACES.map(|s| {
                    captioned(&format!("{s}"), div().square(s).bg(Tone::Accent).rounded(Radius::Px(2.0)))
                })),
            ),
            section(
                "Corners",
                row().gap(16.0).children([("Control 8", Radius::Control), ("Card 12", Radius::Card), ("Panel 16", Radius::Panel)].map(
                    |(name, radius)| captioned(name, div().size(96.0, 64.0).rounded(radius).bg(Tone::Wash)),
                )),
            ),
            section(
                "Markdown",
                markdown("### Release notes\n\nThe renderer now draws **soft shadows**, a *shimmer* and a glow, all in one `vui.effects` pass. See [the docs](https://example.com).\n\n- Shadows follow rounded corners\n- Glow can breathe\n\n> Replies from models show as text, never as markup.\n\n```\ncargo test -p gallery\n```").with(|d| d.max_width(560.0)),
            ),
            section(
                "Depth",
                row().gap(24.0).children((
                    captioned("Soft", div().size(96.0, 64.0).rounded(Radius::Card).bg(Tone::Bg).shadow(Shadow::SOFT)),
                    captioned("Raised", div().size(96.0, 64.0).rounded(Radius::Card).bg(Tone::Bg).shadow(Shadow::RAISED)),
                    captioned("Floating", div().size(96.0, 64.0).rounded(Radius::Card).bg(Tone::Bg).shadow(Shadow::FLOATING)),
                    captioned("Glow", glow(true).size(96.0, 64.0).rounded(Radius::Card).bg(Tone::Bg)),
                )),
            ),
        ),
    )
}
