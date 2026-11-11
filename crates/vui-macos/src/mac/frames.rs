//! Frames: the display link while something moves, a slow timer while the window is hidden, and
//! nothing at all while the UI is idle.

use super::*;

/// The system's Reduce Motion and Increase Contrast settings. macOS has no system-wide text size
/// for apps, so text keeps the theme's.
pub(super) fn system_preferences() -> vui_core::Preferences {
    let workspace = NSWorkspace::sharedWorkspace();
    vui_core::Preferences {
        reduce_motion: workspace.accessibilityDisplayShouldReduceMotion(),
        high_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
        ..vui_core::Preferences::DEFAULT
    }
}

impl VuiView {
    pub(super) fn scale(&self) -> f32 {
        self.window().map_or(1.0, |w| w.backingScaleFactor() as f32)
    }

    pub(super) fn resized(&self) {
        let bounds = self.bounds();
        let (points, scale) = (size(bounds.size.width as f32, bounds.size.height as f32), self.scale());
        // Draw the new size before AppKit shows it, so the content never stretches during a live resize.
        self.with_host(|h| {
            h.resize(points, scale);
            h.redraw();
        });
    }

    pub(super) fn system_mode(&self) -> Mode {
        let names = unsafe { NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) };
        let best = self.effectiveAppearance().bestMatchFromAppearancesWithNames(&names);
        match best {
            Some(name) if &*name == unsafe { NSAppearanceNameDarkAqua } => Mode::Dark,
            _ => Mode::Light,
        }
    }

    /// Hands the UI the system's accessibility settings as they are now.
    pub(super) fn refresh_preferences(&self) {
        let prefs = system_preferences();
        let effects = self.with_host(|h| h.set_preferences(prefs)).unwrap_or_default();
        self.after_input(effects);
    }

    /// Advances the UI to display-link time `now` and draws, then decides when the next frame is.
    pub(super) fn frame(&self, now: f64, refresh: f64) {
        let dt = frame_step(self.ivars().last_frame.get(), now, refresh);
        self.ivars().last_frame.set(Some(now));
        let Some((result, effects)) = self.with_host(|h| h.frame(dt)) else { return };
        self.apply(effects);
        if result.hidden || !result.animating {
            // Out of sight, frames wait for the window to come back into view: the window
            // delegate asks for one then.
            self.pause();
            if let Some(wake) = result.wake.filter(|_| !result.hidden) {
                self.wake_in(wake);
            }
        } else if !self.visible() {
            self.wake_in(HIDDEN_STEP as f32);
        }
        self.check_quit();
    }

    pub(super) fn visible(&self) -> bool {
        self.window().is_some_and(|w| w.occlusionState().contains(NSWindowOcclusionState::Visible))
    }

    pub(super) fn request_frame(&self) {
        if !self.visible() && self.ivars().timer.borrow().is_none() {
            self.wake_in(0.0);
        }
        if let Some(link) = self.ivars().link.borrow().as_ref() {
            link.setPaused(false);
        }
    }

    pub(super) fn pause(&self) {
        if let Some(link) = self.ivars().link.borrow().as_ref() {
            link.setPaused(true);
        }
        self.ivars().last_frame.set(None);
    }

    /// Asks for a frame in `seconds`, for a widget that wants waking while nothing moves.
    pub(super) fn wake_in(&self, seconds: f32) {
        if let Some(old) = self.ivars().timer.take() {
            old.invalidate();
        }
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                seconds.max(0.001) as f64,
                self,
                sel!(onTimer:),
                None,
                false,
            )
        };
        self.ivars().timer.replace(Some(timer));
    }

    pub(super) fn start_display_link(&self) {
        let link = unsafe { self.displayLinkWithTarget_selector(self, sel!(onFrame:)) };
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        self.ivars().link.replace(Some(link));
    }
}
