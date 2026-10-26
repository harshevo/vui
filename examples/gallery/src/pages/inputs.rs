//! Text fields: one line, several, search, password and the states between.

use super::*;

pub fn view(g: &Gallery) -> Div<Msg> {
    page(
        "Inputs",
        (
            section(
                "Text",
                column().gap(16.0).width(360.0).children((
                    field("Name", text_field(&g.name).key("name").on_change(Msg::Name)),
                    field("Email", text_field("").key("email").placeholder("name@example.com")),
                    field("Password", password_field(&g.secret).key("secret").on_change(Msg::Secret)),
                    search_field(&g.query).key("query").placeholder("Search memory").on_change(Msg::Query),
                )),
            ),
            section("Several lines", text_area(&g.notes).key("notes").name("Notes").rows(4).width(360.0).on_change(Msg::Notes)),
            section(
                "States",
                column().gap(12.0).width(360.0).children((
                    text_field("Read only").name("Read only").read_only(true),
                    text_field("Disabled").name("Disabled").disabled(true),
                    text_field("").placeholder("Large").large(),
                    text_field("cargo run --release").name("Command").mono(),
                )),
            ),
        ),
    )
}
