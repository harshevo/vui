//! Pictures: tiles that fade in, fits, a fine-detail downscale and round avatars.

use vui::core::Fit;

use super::*;

const TILE: (f32, f32) = (160.0, 120.0);

fn tile(id: Option<vui::core::ImageId>, name: &str) -> ImageTile<Msg> {
    image_tile(id).size(TILE.0, TILE.1).rounded(Radius::Card).ring().name(name.to_string())
}

pub fn view(g: &Gallery) -> Div<Msg> {
    let p = &g.pictures;
    let [dusk, lagoon, meadow, ember] = [0, 1, 2, 3].map(|i| p.scenes[i].1);
    page(
        "Images",
        (
            section(
                "Tiles",
                flow().gap(16.0).children((
                    tile(Some(dusk), "Dusk"),
                    tile(Some(lagoon), "Lagoon"),
                    tile(Some(meadow), "Meadow"),
                    tile(Some(ember), "Ember"),
                )),
            ),
            section(
                "Loading",
                flow().gap(16.0).align_items(Align::Start).children((
                    captioned("ThumbHash", tile(None, "Loading").placeholder(p.hash.clone())),
                    captioned("Blank", tile(None, "Waiting")),
                )),
            ),
            section(
                "Fit",
                flow().gap(16.0).align_items(Align::Start).children((
                    captioned("Cover", image_tile(Some(dusk)).size(120.0, 120.0).rounded(Radius::Card).ring()),
                    captioned("Contain", image_tile(Some(dusk)).fit(Fit::Contain).size(120.0, 120.0).rounded(Radius::Card).bg(Tone::Wash)),
                    captioned("Wide", image_tile(Some(lagoon)).size(280.0, 120.0).rounded(Radius::Panel).ring()),
                )),
            ),
            section(
                "Downscale",
                flow().gap(16.0).align_items(Align::Start).children((
                    captioned("1200 × 800", image_tile(Some(p.detail)).size(240.0, 160.0).rounded(Radius::Card).ring()),
                    captioned("at 120", image_tile(Some(p.detail)).size(120.0, 80.0).rounded(Radius::Card).ring()),
                    captioned("Transparent", image_tile(Some(p.disc)).fit(Fit::Contain).size(160.0, 120.0).rounded(Radius::Card).bg(Tone::Wash)),
                )),
            ),
            section(
                "Avatars",
                flow().gap(12.0).children((
                    avatar_image(dusk, 40.0),
                    avatar_image(lagoon, 32.0),
                    avatar_image(meadow, 24.0),
                    avatar_sized("Ada Lovelace", 40.0),
                    avatar("Grace Hopper"),
                    avatar_sized("Alan Turing", 24.0),
                )),
            ),
        ),
    )
}
