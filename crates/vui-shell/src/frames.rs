//! Frame timing and snapshot names, the same on every backend.

use std::path::{Path, PathBuf};

/// Seconds since the last frame, from the display's frame timestamps. The first frame after a pause steps a
/// single refresh, so springs do not jump by however long the window sat idle.
pub fn frame_step(last: Option<f64>, now: f64, refresh: f64) -> f32 {
    let refresh = if refresh > 0.0 { refresh } else { 1.0 / 60.0 };
    match last {
        Some(last) if now > last => (now - last).min(4.0 * refresh) as f32,
        _ => refresh as f32,
    }
}

/// Where window `n` writes its snapshot: the first at `path`, later ones numbered, so `frame.png`
/// becomes `frame-1.png`, `frame-2.png` and so on.
pub fn snapshot_path(path: &Path, n: usize) -> PathBuf {
    if n == 0 {
        return path.to_path_buf();
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match path.extension() {
        Some(ext) => format!("{stem}-{n}.{}", ext.to_string_lossy()),
        None => format!("{stem}-{n}"),
    };
    path.with_file_name(name)
}

/// Whether `VUI_RENDERER=cpu` asks for frames drawn on the CPU.
pub fn cpu_requested() -> bool {
    std::env::var("VUI_RENDERER").is_ok_and(|v| v.eq_ignore_ascii_case("cpu"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_steps_follow_the_display_but_never_jump() {
        let r = 1.0 / 120.0;
        assert!((frame_step(None, 10.0, r) - r as f32).abs() < 1e-6);
        assert!((frame_step(Some(10.0), 10.0 + r, r) - r as f32).abs() < 1e-6);
        assert!((frame_step(Some(1.0), 9.0, r) - 4.0 * r as f32).abs() < 1e-6, "long gaps are capped");
        assert!((frame_step(Some(1.0), 1.0, 0.0) - 1.0 / 60.0).abs() < 1e-6);
    }

    #[test]
    fn snapshots_are_numbered_after_the_first() {
        assert_eq!(snapshot_path(Path::new("/t/frame.png"), 0), Path::new("/t/frame.png"));
        assert_eq!(snapshot_path(Path::new("/t/frame.png"), 2), Path::new("/t/frame-2.png"));
        assert_eq!(snapshot_path(Path::new("shot"), 1), Path::new("shot-1"));
    }
}
