use super::*;

// ---- the pasteboard ------------------------------------------------------------------------------

/// The system clipboard: text, and pictures to paste.
pub struct Pasteboard;

impl Clipboard for Pasteboard {
    fn get(&self) -> Option<String> {
        let board = NSPasteboard::generalPasteboard();
        board.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string())
    }

    fn set(&self, text: &str) {
        let board = NSPasteboard::generalPasteboard();
        board.clearContents();
        board.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString });
    }

    /// PNG as it is; TIFF, which screenshots and many apps copy, converted to PNG.
    fn image(&self) -> Option<Vec<u8>> {
        let board = NSPasteboard::generalPasteboard();
        if let Some(png) = board.dataForType(unsafe { NSPasteboardTypePNG }) {
            return Some(png.to_vec());
        }
        let tiff = board.dataForType(unsafe { NSPasteboardTypeTIFF })?;
        let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
        let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &objc2_foundation::NSDictionary::new()) }?;
        Some(png.to_vec())
    }
}
