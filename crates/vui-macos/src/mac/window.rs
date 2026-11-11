//! The overlay panel that may become key, and the delegate that tells the view about its window.

use super::*;

define_class!(
    // SAFETY: NSPanel allows subclassing; the subclass adds no state and does not implement Drop.
    #[unsafe(super(NSPanel, NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "VuiKeyPanel"]
    /// An overlay panel that may become key. A borderless panel refuses by default, so a field in
    /// it could never be typed into.
    pub(super) struct KeyPanel;

    impl KeyPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            true
        }
    }
);

pub struct WindowIvars {
    pub(super) view: Weak<VuiView>,
    pub(super) quit_on_close: bool,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and the delegate does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "VuiWindowDelegate"]
    #[ivars = WindowIvars]
    pub(super) struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowDidBecomeKey:))]
        fn did_become_key(&self, _notification: &NSNotification) {
            if let Some(view) = self.ivars().view.load() {
                view.send(Event::WindowFocus(true));
            }
        }

        #[unsafe(method(windowDidResignKey:))]
        fn did_resign_key(&self, _notification: &NSNotification) {
            if let Some(view) = self.ivars().view.load() {
                view.send(Event::WindowFocus(false));
            }
        }

        #[unsafe(method(windowDidChangeOcclusionState:))]
        fn did_change_occlusion(&self, _notification: &NSNotification) {
            if let Some(view) = self.ivars().view.load() {
                view.request_frame();
            }
        }

        #[unsafe(method(windowWillClose:))]
        fn will_close(&self, _notification: &NSNotification) {
            if self.ivars().quit_on_close {
                NSApplication::sharedApplication(self.mtm()).terminate(None);
            }
        }
    }
);

impl WindowDelegate {
    pub(super) fn new(mtm: MainThreadMarker, view: &VuiView, quit_on_close: bool) -> Retained<WindowDelegate> {
        let this = Self::alloc(mtm).set_ivars(WindowIvars { view: Weak::from_retained(&view.retain()), quit_on_close });
        unsafe { msg_send![super(this), init] }
    }
}

// ---- the application -----------------------------------------------------------------------------
