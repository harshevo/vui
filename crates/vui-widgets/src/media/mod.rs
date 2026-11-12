//! Native views placed by layout: a web page, a video and a PDF.
//!
//! Each takes its space like any element and paints a placeholder there. The window backend puts
//! the platform's own view over the placeholder (WKWebView, an AVPlayerLayer and PDFKit's PDFView on
//! macOS) and moves it as layout moves the element, cut to any scroll view around it. Frames drawn
//! without a window (tests, goldens, the CPU renderer's snapshots) show only the placeholder, so they
//! look the same everywhere.
//!
//! The native view sits above vui's drawing: a popover or a toast that overlaps it is hidden under
//! it, and transforms (`.scale`, `.rotate`) do not reach it.

mod embed;

use vui_core::{EmbedKind, size};
use vui_icons::set;
use vui_runtime::{Base, Role, builder};

pub use embed::{EmbedState, EmbedWidget};

/// A web page.
pub struct WebView<M: 'static> {
    base: Base<M>,
    widget: EmbedWidget,
}

builder!(WebView<M> => EmbedWidget);

/// A web page from `url` (`https:` or `file:`), loaded again when the URL changes. It fills the
/// width it is given at 16:10 unless sized.
pub fn web_view<M: 'static>(url: impl Into<String>) -> WebView<M> {
    let url = url.into();
    let label = embed::host(&url);
    WebView { base: Base::new(), widget: EmbedWidget { kind: EmbedKind::Web { url }, icon: set::GLOBE, label, natural: size(640.0, 400.0), role: Role::Group } }
}

/// A video.
pub struct Video<M: 'static> {
    base: Base<M>,
    widget: EmbedWidget,
}

builder!(Video<M> => EmbedWidget);

/// A video from a URL or a file path, paused on its first frame until `.playing(true)`. It fills the
/// width it is given at 16:9 unless sized.
pub fn video<M: 'static>(url: impl Into<String>) -> Video<M> {
    let url = url.into();
    let label = embed::file_name(&url);
    let kind = EmbedKind::Video { url, playing: false, looping: false, muted: false };
    Video { base: Base::new(), widget: EmbedWidget { kind, icon: set::VIDEO, label, natural: size(640.0, 360.0), role: Role::Image } }
}

impl<M: 'static> Video<M> {
    fn set(mut self, f: impl FnOnce(&mut bool, &mut bool, &mut bool)) -> Self {
        if let EmbedKind::Video { playing, looping, muted, .. } = &mut self.widget.kind {
            f(playing, looping, muted);
        }
        self
    }

    /// Plays (true) or pauses (false). Keep it in the app's state and the video follows.
    pub fn playing(self, on: bool) -> Self {
        self.set(|playing, _, _| *playing = on)
    }

    /// Starts again from the beginning when it ends.
    pub fn looping(self) -> Self {
        self.set(|_, looping, _| *looping = true)
    }

    pub fn muted(self, on: bool) -> Self {
        self.set(|_, _, muted| *muted = on)
    }
}

/// A PDF document.
pub struct PdfView<M: 'static> {
    base: Base<M>,
    widget: EmbedWidget,
}

builder!(PdfView<M> => EmbedWidget);

/// A PDF from a URL or a file path, scaled to fit its width and scrollable. It fills the width it is
/// given at the shape of a Letter page unless sized.
pub fn pdf_view<M: 'static>(url: impl Into<String>) -> PdfView<M> {
    let url = url.into();
    let label = embed::file_name(&url);
    let kind = EmbedKind::Pdf { url, page: 0 };
    PdfView { base: Base::new(), widget: EmbedWidget { kind, icon: set::FILE_TEXT, label, natural: size(612.0, 792.0), role: Role::Image } }
}

impl<M: 'static> PdfView<M> {
    /// Shows page `page`, counted from 0.
    pub fn page(mut self, page: usize) -> Self {
        if let EmbedKind::Pdf { page: p, .. } = &mut self.widget.kind {
            *p = page;
        }
        self
    }
}
