//! The fonts vui ships with, as bytes.
//!
//! Inter (3.013) in three weights for text, and JetBrains Mono (variable) for paths, times and code.
//! Both are licensed under the SIL Open Font License 1.1; the licence texts are in `fonts/`.

pub static INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.otf");
pub static INTER_MEDIUM: &[u8] = include_bytes!("../fonts/Inter-Medium.otf");
pub static INTER_SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold.otf");
pub static JETBRAINS_MONO: &[u8] = include_bytes!("../fonts/JetBrainsMono-Variable.ttf");

/// Every bundled face.
pub fn all() -> [&'static [u8]; 4] {
    [INTER_REGULAR, INTER_MEDIUM, INTER_SEMIBOLD, JETBRAINS_MONO]
}

#[cfg(test)]
mod tests {
    #[test]
    fn fonts_are_present_and_look_like_fonts() {
        for bytes in super::all() {
            assert!(bytes.len() > 100_000);
            // OpenType with CFF outlines starts with "OTTO", TrueType with 0x00010000.
            assert!(bytes.starts_with(b"OTTO") || bytes.starts_with(&[0, 1, 0, 0]));
        }
    }
}
