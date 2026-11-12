//! Opens the gallery in a window, or renders every page, light and dark, to PNG files with no window.
//!
//! ```sh
//! cargo run -p gallery --release -- --window                  # a live window
//! cargo run -p gallery --release -- target/gallery            # every page
//! cargo run -p gallery --release -- target/gallery buttons    # one page
//! ```

use std::path::PathBuf;

use gallery::{Gallery, Headless, Page};
use vui::core::size;

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().is_some_and(|a| a == "--window") {
        #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
        if let Err(e) = vui::run(vui::WindowOptions::new("vui").size(size(1040.0, 720.0)), |res| Gallery::new(res, Page::Foundations, false)) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
        eprintln!("vui has no window backend for this platform");
        return;
    }
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
            #[allow(clippy::print_stdout, reason = "the paths written are the tool's output")]
            {
                println!("{}", path.display());
            }
        }
    }
}
