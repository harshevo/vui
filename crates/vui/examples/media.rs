//! Native views placed by layout: a web page, a video and a PDF, side by side and resizing with the
//! window. On macOS they are WKWebView, an AVPlayerLayer and PDFKit's PDFView; elsewhere, and in the
//! golden frames, their placeholders show.
//!
//! ```sh
//! cargo run -p vui --example media -- https://example.com ~/Movies/clip.mp4 ~/Documents/report.pdf
//! ```

use vui::prelude::*;

#[derive(Clone, Debug)]
pub enum Msg {
    Toggle,
    NextPage,
}

pub struct Media {
    pub web: String,
    pub video: String,
    pub pdf: String,
    pub playing: bool,
    pub page: usize,
}

impl App for Media {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Toggle => self.playing = !self.playing,
            Msg::NextPage => self.page += 1,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().padding(24.0).gap(12.0).fill_height().children((
            row().gap(8.0).align_items(Align::Center).window_drag(true).children((
                title("Media").grow(1.0),
                button(if self.playing { "Pause" } else { "Play" }).key("play").on_click(Msg::Toggle),
                button("Next page").key("page").on_click(Msg::NextPage),
            )),
            web_view(self.web.clone()).rounded(10.0).basis(0.0).grow(1.0).min_height(120.0),
            row().gap(12.0).height(220.0).children((
                video(self.video.clone()).playing(self.playing).looping().rounded(10.0).grow(1.0).fill_height(),
                pdf_view(self.pdf.clone()).page(self.page).rounded(10.0).grow(1.0).fill_height(),
            )),
        ))
    }
}

#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
fn main() {
    let mut args = std::env::args().skip(1);
    let mut next = |fallback: &str| args.next().unwrap_or_else(|| fallback.to_string());
    let app = Media { web: next("https://example.com"), video: next("clip.mp4"), pdf: next("document.pdf"), playing: true, page: 0 };
    let window = WindowOptions::new("Media").size(size(900.0, 720.0)).min_size(size(480.0, 480.0));
    if let Err(e) = vui::run(window, |_resources| app) {
        eprintln!("{e}");
    }
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn main() {
    eprintln!("vui has no window backend for this platform; the app still runs headless in tests.");
}
