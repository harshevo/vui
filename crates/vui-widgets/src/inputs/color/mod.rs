//! The colour picker: a square for saturation and brightness, a strip of hues, the colour as hex,
//! and a row of swatches.

mod pad;

use std::rc::Rc;

use vui_core::shading::Gradient;
use vui_core::{Color, Fill, Spring, Tone};
use vui_runtime::{Align, Builder, Div, Element, IntoElement, Parent, Radius, Role, Styled, column, div, row};

use super::text_field::text_field;
use pad::{Ground, pad};
pub use pad::{Pad, PadState, PadWidget};

/// The swatches a picker offers until it is given its own.
const SWATCHES: [u32; 8] = [0x4f5bd5, 0x2563eb, 0x14b8a6, 0x16a34a, 0xeab308, 0xf97316, 0xdc2626, 0xec4899];

/// A picker for `color`: drag in the square for saturation and brightness, along the strip for the
/// hue, type a hex code, or pick a swatch. `on_change` hears the new colour; alpha is kept.
///
/// ```
/// use vui_core::Color;
/// use vui_widgets::color_picker;
///
/// #[derive(Clone)]
/// enum Msg { Accent(Color) }
/// let picker = color_picker(Color::hex(0x4f5bd5), Msg::Accent);
/// ```
pub fn color_picker<M: Clone + 'static>(color: Color, on_change: impl Fn(Color) -> M + 'static) -> ColorPicker<M> {
    ColorPicker { color, on_change: Rc::new(on_change), swatches: SWATCHES.iter().map(|&c| Color::hex(c)).collect() }
}

/// See [`color_picker`].
pub struct ColorPicker<M: 'static> {
    color: Color,
    on_change: Rc<dyn Fn(Color) -> M>,
    swatches: Vec<Color>,
}

impl<M: Clone + 'static> ColorPicker<M> {
    /// The swatches under the picker; none hides the row.
    pub fn swatches(mut self, colors: impl IntoIterator<Item = Color>) -> Self {
        self.swatches = colors.into_iter().collect();
        self
    }
}

impl<M: Clone + 'static> IntoElement<M> for ColorPicker<M> {
    fn into_element(self) -> Element<M> {
        let ColorPicker { color, on_change, swatches } = self;
        let [h, s, v] = color.to_hsv();
        let alpha = color.a;
        let (on_sv, on_hue, on_hex) = (on_change.clone(), on_change.clone(), on_change.clone());

        let square = div()
            .height(160.0)
            .rounded(Radius::Card)
            .overflow(vui_runtime::Overflow::Hidden)
            .bg(Color::from_hsv(h, 1.0, 1.0))
            .child(
                div().fill().gradient(Gradient::linear(90.0, [Color::WHITE, Color::WHITE.with_alpha(0.0)])).child(
                    div().fill().gradient(Gradient::linear(180.0, [Color::BLACK.with_alpha(0.0), Color::BLACK])).child(
                        pad(s, 1.0 - v, true, Ground::Plain)
                            .fill_height()
                            .name("Saturation and brightness")
                            .on_change(move |x, y| on_sv(Color::from_hsv(h, x, 1.0 - y).with_alpha(alpha))),
                    ),
                ),
            );
        let hues = pad(h / 360.0, 0.5, false, Ground::Hues)
            .height(14.0)
            .name("Hue")
            .on_change(move |x, _| on_hue(Color::from_hsv(x * 360.0, s, v).with_alpha(alpha)));
        let hex = text_field(color.to_hex())
            .mono()
            .grow(1.0)
            .name("Hex")
            .on_change(move |t| on_hex(Color::parse_hex(&t).unwrap_or(color)));
        let preview = div().square(32.0).rounded(Radius::Control).bg(color).border(1.0, Tone::Line);
        let chips: Vec<Div<M>> = swatches
            .into_iter()
            .map(|c| {
                let chosen = c.to_hex() == color.with_alpha(1.0).to_hex();
                div()
                    .square(22.0)
                    .rounded(Radius::Pill)
                    .bg(c)
                    .border(if chosen { 2.0 } else { 1.0 }, if chosen { Fill::from(Tone::Text) } else { Fill::Tone(Tone::Text, 0.12) })
                    .transition(Spring::SNAPPY)
                    .cursor(vui_core::CursorIcon::Pointer)
                    .focusable(true)
                    .role(Role::Button)
                    .name(c.to_hex())
                    .on_click(on_change(c.with_alpha(alpha)))
            })
            .collect();
        let swatch_row = (!chips.is_empty()).then(|| row().gap(8.0).wrap().children(chips));
        column()
            .width(240.0)
            .gap(12.0)
            .role(Role::Group)
            .name("Colour")
            .children((square, hues, row().gap(8.0).align_items(Align::Center).children((preview, hex)), swatch_row))
            .into_element()
    }
}

impl<M: Clone + 'static> vui_runtime::IntoChildren<M> for ColorPicker<M> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.push(self.into_element());
    }
}
