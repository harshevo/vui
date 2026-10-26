//! Unit conversions between AppKit and vui that need no AppKit types: UTF-16 text ranges, scroll
//! deltas and frame times.

use vui_core::{ScrollPhase, Vec2, vec2};

/// The byte offset in `s` of the character that starts at UTF-16 offset `units`. Offsets past the end,
/// or inside a surrogate pair, land at the next character boundary.
pub fn utf16_to_byte(s: &str, units: usize) -> usize {
    let mut seen = 0;
    for (byte, c) in s.char_indices() {
        if seen >= units {
            return byte;
        }
        seen += c.len_utf16();
    }
    s.len()
}

/// How long `s` is in UTF-16 code units, the unit AppKit's text ranges count in.
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// A UTF-16 range inside `s` as a byte range, clamped to the text.
pub fn utf16_range_to_bytes(s: &str, start: usize, len: usize) -> (usize, usize) {
    let a = utf16_to_byte(s, start);
    let b = utf16_to_byte(s, start.saturating_add(len));
    (a, b.max(a))
}

/// Points one line of a mouse wheel scrolls.
pub const LINE: f32 = 16.0;

/// How far content should move for a scroll event. Trackpads and Magic Mice report points; wheels
/// report lines. AppKit's deltas already follow the natural-scrolling setting, and a positive `y`
/// moves content down, as vui expects.
pub fn scroll_delta(precise: bool, dx: f64, dy: f64) -> Vec2 {
    let k = if precise { 1.0 } else { LINE };
    vec2(dx as f32 * k, dy as f32 * k)
}

/// `NSEventPhase` bits.
pub const PHASE_BEGAN: usize = 1;
pub const PHASE_STATIONARY: usize = 2;
pub const PHASE_CHANGED: usize = 4;
pub const PHASE_ENDED: usize = 8;
pub const PHASE_CANCELLED: usize = 16;
pub const PHASE_MAY_BEGIN: usize = 32;

/// The gesture phase of a scroll, from the event's `phase`, or its `momentumPhase` once the fingers
/// have lifted.
pub fn scroll_phase(phase: usize, momentum: usize) -> (ScrollPhase, bool) {
    let p = if phase != 0 { phase } else { momentum };
    let out = if p & PHASE_BEGAN != 0 || p & PHASE_MAY_BEGIN != 0 {
        ScrollPhase::Began
    } else if p & (PHASE_CHANGED | PHASE_STATIONARY) != 0 {
        ScrollPhase::Changed
    } else if p & PHASE_ENDED != 0 {
        ScrollPhase::Ended
    } else if p & PHASE_CANCELLED != 0 {
        ScrollPhase::Cancelled
    } else {
        ScrollPhase::None
    };
    (out, phase == 0 && momentum != 0)
}

/// Seconds since the last frame, from display-link timestamps. The first frame after a pause steps a
/// single refresh, so springs do not jump by however long the window sat idle.
pub fn frame_step(last: Option<f64>, now: f64, refresh: f64) -> f32 {
    let refresh = if refresh > 0.0 { refresh } else { 1.0 / 60.0 };
    match last {
        Some(last) if now > last => (now - last).min(4.0 * refresh) as f32,
        _ => refresh as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_become_byte_offsets() {
        let s = "aé😀b";
        assert_eq!(utf16_len(s), 5, "the emoji is a surrogate pair");
        assert_eq!(utf16_to_byte(s, 0), 0);
        assert_eq!(utf16_to_byte(s, 1), 1);
        assert_eq!(utf16_to_byte(s, 2), 3, "é is two bytes");
        assert_eq!(utf16_to_byte(s, 4), 7, "past the pair");
        assert_eq!(utf16_to_byte(s, 3), 7, "inside the pair rounds up to the next character");
        assert_eq!(utf16_to_byte(s, 99), s.len());
    }

    #[test]
    fn utf16_ranges_become_byte_ranges() {
        let s = "にほんご";
        assert_eq!(utf16_range_to_bytes(s, 1, 2), (3, 9));
        assert_eq!(utf16_range_to_bytes(s, 4, 0), (12, 12), "a caret at the end");
        assert_eq!(utf16_range_to_bytes(s, 9, 9), (12, 12));
    }

    #[test]
    fn wheels_scroll_by_lines_and_trackpads_by_points() {
        assert_eq!(scroll_delta(true, 0.0, -3.5), vec2(0.0, -3.5));
        assert_eq!(scroll_delta(false, 0.0, -1.0), vec2(0.0, -LINE));
    }

    #[test]
    fn scroll_phases() {
        assert_eq!(scroll_phase(PHASE_BEGAN, 0), (ScrollPhase::Began, false));
        assert_eq!(scroll_phase(PHASE_CHANGED, 0), (ScrollPhase::Changed, false));
        assert_eq!(scroll_phase(PHASE_ENDED, 0), (ScrollPhase::Ended, false));
        assert_eq!(scroll_phase(0, PHASE_CHANGED), (ScrollPhase::Changed, true), "momentum after the fingers lift");
        assert_eq!(scroll_phase(0, 0), (ScrollPhase::None, false), "a wheel tick");
    }

    #[test]
    fn frame_steps_follow_the_display_but_never_jump() {
        let r = 1.0 / 120.0;
        assert!((frame_step(None, 10.0, r) - r as f32).abs() < 1e-6);
        assert!((frame_step(Some(10.0), 10.0 + r, r) - r as f32).abs() < 1e-6);
        assert!((frame_step(Some(1.0), 9.0, r) - 4.0 * r as f32).abs() < 1e-6, "long gaps are capped");
        assert!((frame_step(Some(1.0), 1.0, 0.0) - 1.0 / 60.0).abs() < 1e-6);
    }
}
