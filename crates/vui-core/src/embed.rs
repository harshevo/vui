//! Native views a frame asks the platform to show over part of the window: a web page, a video, a
//! PDF. The scene draws a placeholder where each goes, so frames rendered without a window (tests,
//! goldens, the CPU renderer) look the same everywhere; a backend that has the native view puts it
//! over the placeholder.

use crate::geom::Rect;

/// What a native view shows.
#[derive(Clone, Debug, PartialEq)]
pub enum EmbedKind {
    /// A web page: an `https:` or `file:` URL.
    Web { url: String },
    /// A video from a URL or a file path, playing or paused.
    Video { url: String, playing: bool, looping: bool, muted: bool },
    /// A PDF from a URL or a file path, scrolled to `page` (counted from 0).
    Pdf { url: String, page: usize },
}

/// One native view in a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Embed {
    /// Stays the same for as long as the element is on screen, so the backend keeps one native view
    /// per element and moves it rather than making a new one.
    pub id: u64,
    pub kind: EmbedKind,
    /// Where it goes, in window coordinates.
    pub rect: Rect,
    /// The part of the window it may show in: a scroll view's viewport, say. `None` for all of it.
    pub clip: Option<Rect>,
    /// Corner radius.
    pub radius: f32,
    pub opacity: f32,
}

impl Embed {
    /// The part of it that shows: its rect cut to the clip. `None` when nothing does.
    pub fn visible(&self) -> Option<Rect> {
        let shown = match self.clip {
            Some(clip) => self.rect.intersection(clip)?,
            None => self.rect,
        };
        (!shown.is_empty() && self.opacity > 0.0).then_some(shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rect;

    fn web(r: Rect, clip: Option<Rect>) -> Embed {
        Embed { id: 1, kind: EmbedKind::Web { url: "https://example.com".into() }, rect: r, clip, radius: 0.0, opacity: 1.0 }
    }

    #[test]
    fn what_shows_is_cut_to_the_clip() {
        assert_eq!(web(rect(0.0, 0.0, 100.0, 50.0), None).visible(), Some(rect(0.0, 0.0, 100.0, 50.0)));
        assert_eq!(web(rect(0.0, 40.0, 100.0, 50.0), Some(rect(0.0, 0.0, 100.0, 60.0))).visible(), Some(rect(0.0, 40.0, 100.0, 20.0)));
        assert_eq!(web(rect(0.0, 80.0, 100.0, 50.0), Some(rect(0.0, 0.0, 100.0, 60.0))).visible(), None, "scrolled out of view");
        let mut faded = web(rect(0.0, 0.0, 10.0, 10.0), None);
        faded.opacity = 0.0;
        assert_eq!(faded.visible(), None);
    }
}
