//! The overlay panel that may become key, and the delegate that tells the view about its window.

use super::*;

pub struct KeyPanelIvars {
    /// The app asked for the keyboard ([`VuiView::sync_keys`]).
    pub(super) wants_keys: Cell<bool>,
}

define_class!(
    // SAFETY: NSPanel allows subclassing; the subclass's state is a `Cell` and it does not implement Drop.
    #[unsafe(super(NSPanel, NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "VuiKeyPanel"]
    #[ivars = KeyPanelIvars]
    /// An overlay panel that may become key, but only while its app asks for the keyboard. A
    /// borderless panel refuses by default, so a field in it could never be typed into; one that
    /// always accepted would become key on any click, and the app's own window would hear itself
    /// go behind.
    pub(super) struct KeyPanel;

    impl KeyPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            self.ivars().wants_keys.get()
        }
    }
);

impl KeyPanel {
    /// A borderless, non-activating panel over `frame`.
    pub(super) fn new(mtm: MainThreadMarker, frame: NSRect, style: NSWindowStyleMask) -> Retained<KeyPanel> {
        let this = Self::alloc(mtm).set_ivars(KeyPanelIvars { wants_keys: Cell::new(false) });
        // SAFETY: NSPanel's designated initialiser, sent to a fresh instance of its subclass.
        unsafe { msg_send![super(this), initWithContentRect: frame, styleMask: style, backing: NSBackingStoreType::Buffered, defer: false] }
    }
}

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
                view.refresh_preferences();
                view.reinstall_menus();
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
            } else if let Some(view) = self.ivars().view.load() {
                let effects = view.with_host(|h| h.closing()).unwrap_or_default();
                view.after_input(effects);
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
