//! Shaded boxes: gradients, an aurora, a beam running round an edge, spotlights, glow and shimmer.
//! In screenshots everything that moves is held still, so the page shoots the same every time.

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

/// A playing-card-sized panel for the transforms section.
fn panel(name: &str, note: &str, tone: Tone) -> Div<Msg> {
    column()
        .width(132.0)
        .height(92.0)
        .padding(14.0)
        .gap(6.0)
        .rounded(14.0)
        .bg(Tone::Bg)
        .border(1.0, Tone::Line)
        .shadow(Shadow::RAISED)
        .children((div().width(28.0).height(6.0).rounded(Radius::Pill).bg(tone), label(name), small(note).color(Tone::Text3)))
}

pub fn view(g: &Gallery) -> Div<Msg> {
    let held = |seconds: f32| g.still.then_some(seconds);
    page(
        "Effects",
        (
            section(
                "Aurora",
                held(3.0).map_or(aurora(), |t| aurora().still(t)).rounded(16.0).padding(32.0).child(column().gap(8.0).children((
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
                    held(1.1).map_or(border_beam(true), |t| border_beam(true).still(t)).rounded(12.0).padding(16.0).bg(Tone::Side).border(1.0, Tone::Line).child(row().gap(12.0).align_items(Align::Center).children((
                        icon(icon_set::REFRESH),
                        column().gap(2.0).children((label("Syncing"), small("2 of 5 folders").color(Tone::Text3))),
                    ))),
                    held(2.6).map_or(border_beam(true), |t| border_beam(true).still(t)).width(2.0).length(0.45).colors([Fill::from(TEAL.with_alpha(0.0)), TEAL.into(), Tone::Blue.into()]).rounded(Radius::Pill).padding((10.0, 18.0)).border(1.0, Tone::Line).child(label("Live")),
                )),
            ),
            section("Spotlight", row().wrap().gap(12.0).children((tier("Starter", "$8", Some((70.0, 30.0))), tier("Team", "$16", None), tier("Business", "$32", None)))),
            section(
                "Transforms",
                row().gap(28.0).padding((12.0, 8.0)).children((
                    panel("Turned", "rotate(-6)", Tone::Accent).rotate(-6.0),
                    panel("Scaled", "scale(0.9)", Tone::Purple).scale(0.9),
                    panel("Moved", "translate(0, 8)", Tone::Success).translate(0.0, 8.0),
                    panel("Corner", "rotate(4) at top left", Tone::Orange).rotate(4.0).transform_origin(0.0, 0.0),
                )),
            ),
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
