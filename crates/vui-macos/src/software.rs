//! Showing frames drawn by the CPU renderer: each frame's pixels are copied into a bitmap that
//! becomes the contents of the view's layer, which Core Animation composites like any other.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::AllocAnyThread;
use objc2_app_kit::{NSBitmapFormat, NSBitmapImageRep, NSColorSpace, NSDeviceRGBColorSpace, NSImage, NSView};
use objc2_foundation::NSSize;
use objc2_quartz_core::{CALayer, CATransaction};
use vui_render::Canvas;

/// The layer a CPU-rendered view shows its frames on.
pub(crate) struct LayerPresenter {
    layer: Retained<CALayer>,
}

impl LayerPresenter {
    /// Gives `view` a plain layer of its own to show frames on.
    pub(crate) fn new(view: &NSView) -> LayerPresenter {
        let layer = CALayer::new();
        // A layer-hosting view: the layer is set before the view is asked to want one.
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        LayerPresenter { layer }
    }

    /// Shows `canvas`, drawn at `scale` device pixels per point.
    pub(crate) fn show(&self, canvas: &Canvas, scale: f32) {
        let (w, h) = (canvas.width as isize, canvas.height as isize);
        if w == 0 || h == 0 {
            return;
        }
        // SAFETY: null planes ask AppKit to allocate the pixels, sized by the numbers given, and
        // `bitmapData` points at them for as long as the rep lives; the copy below writes exactly
        // `w * h * 4` bytes into it.
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                w,
                h,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                // Alpha last and premultiplied: the canvas's own layout.
                NSBitmapFormat::empty(),
                w * 4,
                32,
            )
        };
        let Some(rep) = rep else { return };
        let data = rep.bitmapData();
        if data.is_null() {
            return;
        }
        let pixels = canvas.pixels();
        // SAFETY: see above; the rep's buffer holds exactly as many bytes as the canvas.
        unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), data, pixels.len()) };
        let rep = rep.bitmapImageRepByRetaggingWithColorSpace(&NSColorSpace::sRGBColorSpace()).unwrap_or(rep);
        let points = NSSize::new(w as f64 / scale as f64, h as f64 / scale as f64);
        let image = NSImage::initWithSize(NSImage::alloc(), points);
        image.addRepresentation(&rep);
        // A new frame replaces the old at once, not with Core Animation's implicit cross-fade.
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.layer.setContentsScale(scale as f64);
        // SAFETY: on macOS a layer's contents may be an NSImage.
        unsafe { self.layer.setContents(Some(&*Retained::cast_unchecked::<AnyObject>(image))) };
        CATransaction::commit();
    }
}
