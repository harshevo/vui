//! One module per page, and the pieces they share.

use vui::prelude::*;

use crate::{Gallery, Msg, Page};

mod buttons;
mod controls;
mod foundations;
mod icons;
mod images;
mod inputs;
mod lists;
mod overlays;
mod settings;

/// The page that is open.
pub fn view(g: &Gallery) -> Div<Msg> {
    match g.page {
        Page::Foundations => foundations::view(g),
        Page::Buttons => buttons::view(g),
        Page::Controls => controls::view(g),
        Page::Inputs => inputs::view(g),
        Page::Lists => lists::view(g),
        Page::Images => images::view(g),
        Page::Icons => icons::view(g),
        Page::Overlays => overlays::view(g),
        Page::Settings => settings::view(g),
    }
}

/// A page: its title, 48 points of air, then sections 32 points apart, in a column no wider than the
/// eye likes. The column is keyed `page` so a screenshot can ask how tall the page is.
pub fn page(name: &str, sections: impl IntoChildren<Msg>) -> Div<Msg> {
    column().key("page").padding(32.0).gap(48.0).max_width(768.0).children((title(name), column().gap(32.0).children(sections)))
}

/// A small grey heading over what it names.
pub fn section(name: &str, body: impl IntoElement<Msg>) -> Div<Msg> {
    column().gap(16.0).children((section_title(name), body))
}

/// Things side by side, wrapping onto the next line when they do not fit.
pub fn flow() -> Div<Msg> {
    row().wrap().gap(12.0).align_items(Align::Center)
}

/// A control with a small grey caption under it.
pub fn captioned(caption: &str, body: impl IntoElement<Msg>) -> Div<Msg> {
    column().gap(8.0).align_items(Align::Start).children((body, small(caption).color(Tone::Text3)))
}
