//! Shaped text: its size and lines, and where carets, selections and clicks fall in it.

use super::*;

/// Shaped text, ready to measure and paint.
pub struct TextLayout {
    pub(crate) buffer: Buffer,
    pub(crate) size: Size,
    pub(crate) lines: usize,
    pub(crate) text: Arc<str>,
    /// Byte offset where each paragraph (split at `\n`) starts.
    pub(crate) line_starts: Vec<usize>,
    pub(crate) line_height: f32,
    /// Byte offset where each span starts, for rich text.
    pub(crate) span_starts: Vec<usize>,
}

/// One line of wrapped text: its top and height, how wide its glyphs reach, and the bytes
/// `start..end` it shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualLine {
    pub top: f32,
    pub height: f32,
    pub width: f32,
    pub start: usize,
    pub end: usize,
}

impl fmt::Debug for TextLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextLayout").field("text", &self.text).field("size", &self.size).field("lines", &self.lines).finish()
    }
}

impl TextLayout {
    /// The space the text occupies. Its width is the widest line, not the wrap width.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Visual lines after wrapping.
    pub fn line_count(&self) -> usize {
        self.lines
    }

    /// Which span the byte `index` is in, for rich text.
    pub fn span_at(&self, index: usize) -> Option<usize> {
        if self.span_starts.is_empty() {
            return None;
        }
        Some(self.span_starts.partition_point(|&start| start <= index).saturating_sub(1))
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Distance from the top of the layout to the first line's baseline.
    pub fn baseline(&self) -> f32 {
        self.buffer.layout_runs().next().map_or(self.line_height * 0.8, |r| r.line_y)
    }

    /// Byte offset in the text where visual line `index` (after wrapping) ends.
    pub(crate) fn end_of_visual_line(&self, index: usize) -> usize {
        let run = self.buffer.layout_runs().nth(index);
        match run {
            Some(run) => {
                let end = run.glyphs.last().map_or(0, |g| g.end);
                self.line_starts[run.line_i] + end
            }
            None => self.text.len(),
        }
    }

    /// The visual lines after wrapping, top to bottom: where each sits and which bytes it holds.
    pub fn visual_lines(&self) -> Vec<VisualLine> {
        self.buffer
            .layout_runs()
            .map(|run| {
                let base = self.line_starts.get(run.line_i).copied().unwrap_or(0);
                let start = run.glyphs.iter().map(|g| g.start).min().unwrap_or(0);
                let end = run.glyphs.iter().map(|g| g.end).max().unwrap_or(0);
                VisualLine { top: run.line_top, height: run.line_height, width: run.line_w, start: base + start, end: base + end }
            })
            .collect()
    }

    /// Splits a byte index into (paragraph, byte offset inside it).
    fn locate(&self, index: usize) -> (usize, usize) {
        let index = index.min(self.text.len());
        let line = self.line_starts.partition_point(|&s| s <= index).saturating_sub(1);
        (line, index - self.line_starts[line])
    }

    /// The caret for a byte index: a 1 px wide rectangle the height of its line.
    pub fn caret_rect(&self, index: usize) -> Rect {
        let (line_i, idx) = self.locate(index);
        let mut fallback = None;
        for run in self.buffer.layout_runs() {
            if run.line_i != line_i {
                continue;
            }
            for g in run.glyphs {
                if g.start <= idx && idx < g.end {
                    let x = if g.level.is_rtl() { g.x + g.w } else { g.x };
                    return rect(x, run.line_top, 1.0, run.line_height);
                }
            }
            match run.glyphs.last() {
                Some(g) if idx >= g.end => {
                    let x = if g.level.is_rtl() { g.x } else { g.x + g.w };
                    fallback = Some(rect(x, run.line_top, 1.0, run.line_height));
                }
                None => fallback = Some(rect(0.0, run.line_top, 1.0, run.line_height)),
                _ => {}
            }
        }
        fallback.unwrap_or_else(|| rect(0.0, 0.0, 1.0, self.line_height))
    }

    /// Rectangles covering the bytes `start..end`, one per visual line.
    pub fn selection_rects(&self, start: usize, end: usize) -> Vec<Rect> {
        if start >= end {
            return Vec::new();
        }
        let (l0, i0) = self.locate(start);
        let (l1, i1) = self.locate(end);
        let (c0, c1) = (Cursor::new(l0, i0), Cursor::new(l1, i1));
        let mut out = Vec::new();
        for run in self.buffer.layout_runs() {
            for (x, w) in run.highlight(c0, c1) {
                out.push(rect(x, run.line_top, w, run.line_height));
            }
        }
        out
    }

    /// The byte index of the character boundary nearest to `p` (relative to the layout's origin).
    pub fn hit(&self, p: Point) -> usize {
        match self.buffer.hit(p.x, p.y) {
            Some(c) => (self.line_starts.get(c.line).copied().unwrap_or(0) + c.index).min(self.text.len()),
            None if p.y < 0.0 => 0,
            None => self.text.len(),
        }
    }
}
