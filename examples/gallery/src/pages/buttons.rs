//! Buttons: four weights, icons, three sizes and the states a button can be in.

use super::*;

pub fn view(_g: &Gallery) -> Div<Msg> {
    page(
        "Buttons",
        (
            section(
                "Variants",
                flow().children((
                    button("Primary").primary().key("primary"),
                    button("Secondary").key("secondary"),
                    button("Ghost").ghost().key("ghost"),
                    button("Danger").danger().key("danger"),
                )),
            ),
            section(
                "Icons",
                flow().children((
                    button("Ask").icon(icon_set::MIC).primary(),
                    button("Upload").icon(icon_set::UPLOAD),
                    button("Next").trailing(icon_set::ARROW_RIGHT),
                    icon_button(icon_set::PLUS, "Add"),
                    icon_button(icon_set::MORE, "More"),
                    icon_button(icon_set::SETTINGS, "Settings"),
                    icon_button(icon_set::X, "Close"),
                )),
            ),
            section(
                "Sizes",
                flow().children((button("Small").small(), button("Medium"), button("Large").large())),
            ),
            section(
                "States",
                flow().children((
                    button("Saving").primary().loading(true),
                    button("Saving").loading(true),
                    button("Disabled").primary().disabled(true),
                    button("Disabled").disabled(true),
                    button("Disabled").ghost().disabled(true),
                )),
            ),
        ),
    )
}
