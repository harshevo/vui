//! Renders every page of the gallery, light and dark, to PNG files, with no window.
//!
//! ```sh
//! cargo run -p gallery --release -- target/gallery            # every page
//! cargo run -p gallery --release -- target/gallery buttons    # one page
//! ```

use std::path::PathBuf;

use gallery::{Headless, Page};

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().unwrap_or_else(|| "target/gallery".into()).into();
    let only = args.next().map(|name| name.to_lowercase());
    for page in Page::ALL {
        let name = page.name().to_lowercase();
        if only.as_ref().is_some_and(|only| *only != name) {
            continue;
        }
        for dark in [false, true] {
            let mut gallery = Headless::new(page, dark, 2.0);
            let path = out.join(format!("{name}_{}.png", if dark { "dark" } else { "light" }));
            gallery.image().save_png(&path).expect("the screenshot could not be written");
            println!("{}", path.display());
        }
    }
}
