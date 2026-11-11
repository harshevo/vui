//! Shaded boxes: gradients, an aurora, a beam running round an edge, spotlights, glow and shimmer.
//! Everything that moves is held still here, so the page shoots the same every time; drop `.still`
//! to let it move.

use super::*;

const PINK: Color = Color::hex(0xec4899);
const ORANGE: Color = Color::hex(0xf97316);
const TEAL: Color = Color::hex(0x14b8a6);

fn swatch(caption: &str, gradient: Gradient) -> Div<Msg> {
    captioned(caption, div().width(120.0).height(80.0).rounded(12.0).gradient(gradient))
}

fn tier(name: &str, price: &str, lit: Option<(f32, f32)>) -> Element<Msg> {
    let body = column().gap(8.0).children((
        label(name).color(Tone::Text2),
        title(price),
        small("per seat, billed yearly").color(Tone::Text3),
    ));
    let light = spotlight().reach(260.0).grow(1.0).basis(px(160.0)).padding(20.0).rounded(14.0).bg(Tone::Side).border(1.0, Tone::Line);
    match lit {
        Some((x, y)) => light.at(x, y).child(body).into_element(),
        None => light.child(body).into_element(),
    }
}

pub fn view(_g: &Gallery) -> Div<Msg> {
    page(
        "Effects",
        (
            section(
                "Aurora",
                aurora().still(3.0).rounded(16.0).padding(32.0).child(column().gap(8.0).children((
                    title("Build once, run anywhere"),
                    label("Soft colour drifting behind the content, blended in Oklab with a little grain.").color(Tone::Text2),
                    row().gap(8.0).children((button("Get started").primary(), button("Read the docs"))),
                ))),
            ),
            section(
                "Gradients",
                column().gap(16.0).children((
                    flow().children((
                        swatch("linear, 135°", Gradient::linear(135.0, [Fill::from(Tone::Accent), PINK.into(), ORANGE.into()])),
                        swatch("radial", Gradient::radial([Fill::from(Color::WHITE), TEAL.into(), Tone::Accent.into()]).at(0.3, 0.3).radius(0.9)),
                        swatch("conic", Gradient::conic([Fill::from(Tone::Accent), PINK.into(), ORANGE.into(), Tone::Accent.into()])),
                    )),
                    flow().children((
                        button("Upgrade").primary().gradient(Gradient::linear(90.0, [Tone::Accent, Tone::Purple])),
                        div()
                            .padding((12.0, 16.0))
                            .rounded(12.0)
                            .bg(Tone::Bg)
                            .gradient_border(2.0, Gradient::conic([Fill::from(Tone::Accent), PINK.into(), ORANGE.into(), Tone::Accent.into()]))
                            .child(label("Featured")),
                    )),
                )),
            ),
            section(
                "Border beam",
                flow().children((
                    border_beam(true).still(1.1).rounded(12.0).padding(16.0).bg(Tone::Side).border(1.0, Tone::Line).child(row().gap(12.0).align_items(Align::Center).children((
                        icon(icon_set::REFRESH),
                        column().gap(2.0).children((label("Syncing"), small("2 of 5 folders").color(Tone::Text3))),
                    ))),
                    border_beam(true).still(2.6).width(2.0).length(0.45).colors([Fill::from(TEAL.with_alpha(0.0)), TEAL.into(), Tone::Blue.into()]).rounded(Radius::Pill).padding((10.0, 18.0)).border(1.0, Tone::Line).child(label("Live")),
                )),
            ),
            section("Spotlight", row().wrap().gap(12.0).children((tier("Starter", "$8", Some((70.0, 30.0))), tier("Team", "$16", None), tier("Business", "$32", None)))),
            section(
                "Glow and shimmer",
                flow().children((
                    glow(true).rounded(12.0).padding(16.0).bg(Tone::Side).child(label("Listening")),
                    shimmer(true).rounded(8.0).padding((8.0, 12.0)).bg(Tone::Wash).child(label("Thinking…")),
                )),
            ),
        ),
    )
}
