//! Decorations: stretches of a field's text drawn in a tone of their own or underlined.
//!
//! The text is a single layout in a single colour, so a marked field draws it in pieces: the text is
//! cut where a mark starts or ends and where a line wraps, and each piece is laid out alone and put
//! exactly where the whole layout has that stretch. Underlines are drawn from the whole layout's
//! selection rectangles, so they follow wrapped lines too.

use std::collections::BTreeSet;

use super::*;

/// How an underline is drawn.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Underline {
    /// A straight line.
    Line,
    /// A wavy line, the usual sign of something wrong.
    Squiggle,
}

/// A stretch of text and how it is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoration {
    /// Byte positions in the field's text.
    pub range: Range<usize>,
    /// The tone the text takes; `None` leaves it as it was.
    pub tone: Option<Tone>,
    pub underline: Option<(Underline, Tone)>,
}

impl Decoration {
    pub fn new(range: Range<usize>) -> Decoration {
        Decoration { range, tone: None, underline: None }
    }

    pub fn tone(mut self, tone: Tone) -> Decoration {
        self.tone = Some(tone);
        self
    }

    pub fn underline(mut self, tone: Tone) -> Decoration {
        self.underline = Some((Underline::Line, tone));
        self
    }

    /// A wavy underline: where a formula stops making sense.
    pub fn squiggle(mut self, tone: Tone) -> Decoration {
        self.underline = Some((Underline::Squiggle, tone));
        self
    }
}

/// How far a squiggle's peaks and troughs are apart, and its pitch.
const WAVE: f32 = 1.5;
const PITCH: f32 = 2.0;

impl<M: 'static> TextFieldWidget<M> {
    /// Draws the text in pieces with the marks' tones and then their underlines. False, having drawn
    /// nothing, when there is nothing to mark or the text can't be marked (a password, a composition
    /// in progress), so the caller draws it plain.
    pub(super) fn paint_marked(&self, cx: &mut PaintCx, surface: &Surface, style: &TextStyle, origin: Point, editor: &Editor) -> bool {
        if self.marks.is_empty() || surface.masked || editor.preedit().is_some() {
            return false;
        }
        let layout = &surface.layout;
        let text = layout.text();
        let len = text.len();
        let bound = |i: usize| text.floor_char_boundary(i.min(len));
        let marks: Vec<(usize, usize, &Decoration)> = self.marks.iter().map(|m| (bound(m.range.start), bound(m.range.end), m)).filter(|(a, b, _)| a < b).collect();

        let mut cuts: BTreeSet<usize> = BTreeSet::from([0, len]);
        for (a, b, _) in &marks {
            cuts.insert(*a);
            cuts.insert(*b);
        }
        // Where the layout starts a new line, so no piece runs across a wrap.
        let mut line_top = None;
        for (i, _) in text.char_indices() {
            let y = layout.caret_rect(i).y;
            if line_top != Some(y) {
                cuts.insert(i);
                line_top = Some(y);
            }
        }
        let cuts: Vec<usize> = cuts.into_iter().collect();
        for pair in cuts.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let piece = text[a..b].trim_end_matches('\n');
            if piece.trim().is_empty() {
                continue;
            }
            let tone = marks.iter().rev().find(|(s, e, m)| *s <= a && b <= *e && m.tone.is_some()).and_then(|(_, _, m)| m.tone).unwrap_or(Tone::Text);
            let at = layout.caret_rect(a);
            let laid = cx.fonts().layout_cached(piece, style, None);
            cx.text(laid, pt(origin.x + at.x, origin.y + at.y), tone);
        }
        for (a, b, m) in &marks {
            let Some((kind, tone)) = m.underline else { continue };
            for r in layout.selection_rects(*a, *b) {
                let y = origin.y + r.bottom() - 2.0;
                match kind {
                    Underline::Line => cx.fill_rounded(rect(origin.x + r.x, y, r.w, 1.0), Corners::ZERO, tone),
                    Underline::Squiggle => {
                        let mut x = 0.0;
                        let mut up = false;
                        while x < r.w {
                            let w = PITCH.min(r.w - x);
                            cx.fill_rounded(rect(origin.x + r.x + x, y + if up { 0.0 } else { WAVE }, w, 1.0), Corners::ZERO, tone);
                            x += PITCH;
                            up = !up;
                        }
                    }
                }
            }
        }
        true
    }
}
