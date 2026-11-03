//! The AppKit side: the application, a window, and the view that hosts a vui UI.
//!
//! The view is layer-backed; wgpu adds its Metal layer to it. Frames come from the view's display link,
//! which runs only while something moves and pauses when the UI is idle, so a still window costs
//! nothing. Keys go through the input method (`NSTextInputClient`) so dead keys, Japanese, Chinese and
//! Korean input, dictation and the emoji picker all type into vui's text fields.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::mpsc;

use block2::RcBlock;
use dispatch2::DispatchQueue;

use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject, Sel};
use objc2::{AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSApplicationActivationPolicy,
    NSBackingStoreType, NSColor, NSCursor, NSEvent, NSEventMask, NSEventModifierFlags, NSMainMenuWindowLevel, NSMenu, NSMenuItem, NSPanel,
    NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString, NSPasteboardTypeTIFF, NSResponder, NSScreen, NSTextInputClient, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSWindow, NSWindowCollectionBehavior, NSWindowDelegate, NSWindowOcclusionState, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSNotFound, NSNotification, NSPoint, NSRange, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize,
    NSString, NSTimer, NSUInteger,
};
use objc2_quartz_core::{CACurrentMediaTime, CADisplayLink};
use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle};
use vui_core::{
    Color, CursorIcon, Event, ImeEvent, Key, KeyEvent, Mode, Modifiers, PointerButton, Rect, Resources, Size, pt, size,
};
use vui_render::{CustomPass, Gpu, Renderer};
use vui_runtime::{App, Clipboard, Effect, Ui};

use crate::convert::{frame_step, scroll_delta, scroll_phase, utf16_len, utf16_range_to_bytes};
use crate::host::{Host, Snapshot, Window};
use crate::keys::{modifiers, named_key, shortcut_char, typed_text};
use crate::{Error, OverlayOptions, Place, Proxy, Screen, WindowOptions};

// ---- the view ------------------------------------------------------------------------------------

/// Seconds between frames that a timer drives while the window is out of sight.
const HIDDEN_STEP: f64 = 1.0 / 30.0;

pub struct ViewIvars {
    host: RefCell<Option<Box<dyn Host>>>,
    link: RefCell<Option<Retained<CADisplayLink>>>,
    /// Display-link time of the last frame; `None` while paused.
    last_frame: Cell<Option<f64>>,
    timer: RefCell<Option<Retained<NSTimer>>>,
    /// The key being shown to the input method. Whatever the input method does with it decides
    /// what reaches the UI; if it does nothing, the key goes through as it is.
    pending: RefCell<Option<KeyEvent>>,
    /// Text being composed, as the input method last set it.
    marked: RefCell<String>,
    /// Where the caret is, in view coordinates, for placing the candidate window.
    caret: Cell<Option<Rect>>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements beyond being used on the main thread, and the
    // view does not implement Drop.
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "VuiView"]
    #[ivars = ViewIvars]
    pub struct VuiView;

    impl VuiView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, new_size: NSSize) {
            let _: () = unsafe { msg_send![super(self), setFrameSize: new_size] };
            self.resized();
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        fn did_change_backing(&self) {
            self.resized();
        }

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn did_change_appearance(&self) {
            let mode = self.system_mode();
            let effects = self.with_host(|h| h.set_system_mode(mode)).unwrap_or_default();
            // What shows behind the view while a live resize catches up.
            if let (Some(window), Some(Some(canvas))) = (self.window(), self.with_host(|h| h.canvas())) {
                window.setBackgroundColor(Some(&ns_color(canvas)));
            }
            self.after_input(effects);
        }

        #[unsafe(method(onFrame:))]
        fn on_frame(&self, link: &CADisplayLink) {
            self.frame(link.targetTimestamp(), link.duration());
        }

        #[unsafe(method(onTimer:))]
        fn on_timer(&self, _timer: &NSTimer) {
            self.ivars().timer.replace(None);
            // The display link only runs while the view is on screen. Out of sight, the timer drives
            // the frame itself: a snapshot being taken, or a widget's wake-up.
            if self.visible() {
                self.request_frame();
            } else {
                self.frame(CACurrentMediaTime(), HIDDEN_STEP);
            }
        }

        // -- pointer --

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let pos = self.point(event);
            if self.with_host(|h| h.drags_window(pos)).unwrap_or(false)
                && let Some(window) = self.window()
            {
                window.performWindowDragWithEvent(event);
                return;
            }
            self.pointer_down(event, PointerButton::Primary);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.pointer_up(event, PointerButton::Primary);
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.pointer_down(event, PointerButton::Secondary);
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            self.pointer_up(event, PointerButton::Secondary);
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) {
            self.pointer_down(event, other_button(event));
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            self.pointer_up(event, other_button(event));
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.pointer_moved(event);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.pointer_moved(event);
        }

        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged(&self, event: &NSEvent) {
            self.pointer_moved(event);
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged(&self, event: &NSEvent) {
            self.pointer_moved(event);
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, event: &NSEvent) {
            self.pointer_moved(event);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.send(Event::PointerLeft);
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            let precise = event.hasPreciseScrollingDeltas();
            let delta = scroll_delta(precise, event.scrollingDeltaX(), event.scrollingDeltaY());
            let (phase, momentum) = scroll_phase(event.phase().0, event.momentumPhase().0);
            let pos = self.point(event);
            self.send(Event::Scroll { pos, delta, precise, phase, momentum });
        }

        #[unsafe(method(magnifyWithEvent:))]
        fn magnify(&self, event: &NSEvent) {
            let (phase, _) = scroll_phase(event.phase().0, 0);
            let pos = self.point(event);
            self.send(Event::Magnify { pos, delta: event.magnification() as f32, phase });
        }

        // -- keyboard --

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let key = key_event(event, true);
            let composing = !self.ivars().marked.borrow().is_empty();
            // Shortcuts skip the input method, unless it is composing and owns every key.
            if key.mods.cmd || (key.mods.ctrl && !composing) {
                self.send(Event::Key(key));
                return;
            }
            self.ivars().pending.replace(Some(key));
            self.interpretKeyEvents(&NSArray::from_slice(&[event]));
            if let Some(key) = self.ivars().pending.take() {
                self.send(Event::Key(key));
            }
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            self.send(Event::Key(key_event(event, false)));
        }

        // The Edit menu's commands arrive as actions; they are the same shortcuts the widgets know.
        #[unsafe(method(copy:))]
        fn copy(&self, _sender: Option<&AnyObject>) {
            self.shortcut('c', Modifiers::CMD);
        }

        #[unsafe(method(cut:))]
        fn cut(&self, _sender: Option<&AnyObject>) {
            self.shortcut('x', Modifiers::CMD);
        }

        #[unsafe(method(paste:))]
        fn paste(&self, _sender: Option<&AnyObject>) {
            self.shortcut('v', Modifiers::CMD);
        }

        #[unsafe(method(selectAll:))]
        fn select_all(&self, _sender: Option<&AnyObject>) {
            self.shortcut('a', Modifiers::CMD);
        }

        #[unsafe(method(undo:))]
        fn undo(&self, _sender: Option<&AnyObject>) {
            self.shortcut('z', Modifiers::CMD);
        }

        #[unsafe(method(redo:))]
        fn redo(&self, _sender: Option<&AnyObject>) {
            self.shortcut('z', Modifiers { shift: true, cmd: true, ..Modifiers::NONE });
        }
    }

    unsafe impl NSObjectProtocol for VuiView {}

    unsafe impl NSTextInputClient for VuiView {
        #[unsafe(method(insertText:replacementRange:))]
        fn insert_text(&self, string: &AnyObject, _range: NSRange) {
            let text = string_of(string);
            let was_composing = !self.ivars().marked.take().is_empty();
            match self.ivars().pending.take() {
                // A plain key typed plain text: send it as the key, so widgets that react to keys see it.
                Some(mut key) if !was_composing => {
                    key.text = typed_text(&text);
                    self.send(Event::Key(key));
                }
                // Composition finished, or text arrived on its own: dictation, the emoji picker.
                _ => self.send(Event::Ime(ImeEvent::Commit(text))),
            }
        }

        #[unsafe(method(doCommandBySelector:))]
        fn do_command(&self, _selector: Sel) {
            // The key was not text (an arrow, Return, Escape): the widgets give it its meaning.
            if let Some(mut key) = self.ivars().pending.take() {
                key.text = None;
                self.send(Event::Key(key));
            }
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        fn set_marked_text(&self, string: &AnyObject, selected: NSRange, _replacement: NSRange) {
            self.ivars().pending.replace(None);
            let text = string_of(string);
            if text.is_empty() {
                self.ivars().marked.replace(String::new());
                self.send(Event::Ime(ImeEvent::Cancel));
                return;
            }
            let cursor = utf16_range_to_bytes(&text, selected.location, selected.length);
            self.ivars().marked.replace(text.clone());
            self.send(Event::Ime(ImeEvent::Preedit { text, cursor: Some(cursor) }));
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            let text = self.ivars().marked.take();
            if !text.is_empty() {
                self.send(Event::Ime(ImeEvent::Commit(text)));
            }
        }

        #[unsafe(method(hasMarkedText))]
        fn has_marked_text(&self) -> bool {
            !self.ivars().marked.borrow().is_empty()
        }

        #[unsafe(method(markedRange))]
        fn marked_range(&self) -> NSRange {
            let marked = self.ivars().marked.borrow();
            if marked.is_empty() { NSRange::new(NSNotFound as NSUInteger, 0) } else { NSRange::new(0, utf16_len(&marked)) }
        }

        #[unsafe(method(selectedRange))]
        fn selected_range(&self) -> NSRange {
            let marked = self.ivars().marked.borrow();
            NSRange::new(utf16_len(&marked), 0)
        }

        #[unsafe(method_id(validAttributesForMarkedText))]
        fn valid_attributes(&self) -> Retained<NSArray<NSAttributedStringKey>> {
            NSArray::new()
        }

        #[unsafe(method_id(attributedSubstringForProposedRange:actualRange:))]
        fn attributed_substring(&self, _range: NSRange, _actual: *mut NSRange) -> Option<Retained<NSAttributedString>> {
            None
        }

        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        fn first_rect(&self, _range: NSRange, _actual: *mut NSRange) -> NSRect {
            let caret = self.ivars().caret.get().unwrap_or(Rect::ZERO);
            let local = NSRect::new(NSPoint::new(caret.x as f64, caret.y as f64), NSSize::new(caret.w as f64, caret.h as f64));
            let Some(window) = self.window() else { return local };
            let in_window = self.convertRect_toView(local, None);
            window.convertRectToScreen(in_window)
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn character_index(&self, _point: NSPoint) -> NSUInteger {
            NSNotFound as NSUInteger
        }
    }
);

impl VuiView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<VuiView> {
        let this = Self::alloc(mtm).set_ivars(ViewIvars {
            host: RefCell::new(None),
            link: RefCell::new(None),
            last_frame: Cell::new(None),
            timer: RefCell::new(None),
            pending: RefCell::new(None),
            marked: RefCell::new(String::new()),
            caret: Cell::new(None),
        });
        let view: Retained<VuiView> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setWantsLayer(true);
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::ActiveAlways
            | NSTrackingAreaOptions::InVisibleRect
            | NSTrackingAreaOptions::EnabledDuringMouseDrag;
        let area =
            unsafe { NSTrackingArea::initWithRect_options_owner_userInfo(NSTrackingArea::alloc(), NSRect::ZERO, options, Some(&view), None) };
        view.addTrackingArea(&area);
        view
    }

    /// Runs `f` on the host, unless there is none yet or a call into it is already under way (AppKit
    /// can call back into the view from inside one).
    fn with_host<R>(&self, f: impl FnOnce(&mut dyn Host) -> R) -> Option<R> {
        let mut slot = self.ivars().host.try_borrow_mut().ok()?;
        let host = slot.as_mut()?;
        Some(f(host.as_mut()))
    }

    fn send(&self, event: Event) {
        let effects = self.with_host(|h| h.handle(event)).unwrap_or_default();
        self.after_input(effects);
    }

    fn deliver(&self) {
        let effects = self.with_host(|h| h.deliver()).unwrap_or_default();
        self.after_input(effects);
    }

    fn shortcut(&self, c: char, mods: Modifiers) {
        self.send(Event::Key(KeyEvent::press(Key::Character(c), mods)));
    }

    fn after_input(&self, effects: Vec<Effect>) {
        self.apply(effects);
        if let (Some(through), Some(window)) = (self.with_host(|h| h.passes_clicks()).flatten(), self.window())
            && window.ignoresMouseEvents() != through
        {
            window.setIgnoresMouseEvents(through);
        }
        if self.with_host(|h| h.needs_paint()).unwrap_or(false) {
            self.request_frame();
        }
        self.check_quit();
    }

    fn apply(&self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Cursor(icon) => cursor(icon).set(),
                Effect::ImeArea(rect) => {
                    self.ivars().caret.set(rect);
                    if let Some(input) = self.inputContext() {
                        if rect.is_none() && !self.ivars().marked.borrow().is_empty() {
                            input.discardMarkedText();
                            self.ivars().marked.replace(String::new());
                        }
                        input.invalidateCharacterCoordinates();
                    }
                }
                _ => {}
            }
        }
    }

    fn check_quit(&self) {
        if self.with_host(|h| h.quit_requested()).unwrap_or(false) {
            let app = NSApplication::sharedApplication(self.mtm());
            app.terminate(None);
        }
    }

    fn point(&self, event: &NSEvent) -> vui_core::Point {
        let p = self.convertPoint_fromView(event.locationInWindow(), None);
        pt(p.x as f32, p.y as f32)
    }

    fn pointer_down(&self, event: &NSEvent, button: PointerButton) {
        if let Some(window) = self.window() {
            window.makeFirstResponder(Some(self));
        }
        let pos = self.point(event);
        let click_count = event.clickCount().clamp(1, 255) as u8;
        self.send(Event::PointerDown { pos, button, mods: event_mods(event), click_count });
    }

    fn pointer_up(&self, event: &NSEvent, button: PointerButton) {
        let pos = self.point(event);
        self.send(Event::PointerUp { pos, button, mods: event_mods(event) });
    }

    fn pointer_moved(&self, event: &NSEvent) {
        let pos = self.point(event);
        self.send(Event::PointerMoved { pos, mods: event_mods(event) });
    }

    fn scale(&self) -> f32 {
        self.window().map_or(1.0, |w| w.backingScaleFactor() as f32)
    }

    fn resized(&self) {
        let bounds = self.bounds();
        let (points, scale) = (size(bounds.size.width as f32, bounds.size.height as f32), self.scale());
        // Draw the new size before AppKit shows it, so the content never stretches during a live resize.
        self.with_host(|h| {
            h.resize(points, scale);
            h.redraw();
        });
    }

    fn system_mode(&self) -> Mode {
        let names = unsafe { NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) };
        let best = self.effectiveAppearance().bestMatchFromAppearancesWithNames(&names);
        match best {
            Some(name) if &*name == unsafe { NSAppearanceNameDarkAqua } => Mode::Dark,
            _ => Mode::Light,
        }
    }

    /// Advances the UI to display-link time `now` and draws, then decides when the next frame is.
    fn frame(&self, now: f64, refresh: f64) {
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

    fn visible(&self) -> bool {
        self.window().is_some_and(|w| w.occlusionState().contains(NSWindowOcclusionState::Visible))
    }

    fn request_frame(&self) {
        if !self.visible() && self.ivars().timer.borrow().is_none() {
            self.wake_in(0.0);
        }
        if let Some(link) = self.ivars().link.borrow().as_ref() {
            link.setPaused(false);
        }
    }

    fn pause(&self) {
        if let Some(link) = self.ivars().link.borrow().as_ref() {
            link.setPaused(true);
        }
        self.ivars().last_frame.set(None);
    }

    /// Asks for a frame in `seconds`, for a widget that wants waking while nothing moves.
    fn wake_in(&self, seconds: f32) {
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

    fn start_display_link(&self) {
        let link = unsafe { self.displayLinkWithTarget_selector(self, sel!(onFrame:)) };
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        self.ivars().link.replace(Some(link));
    }
}

fn other_button(event: &NSEvent) -> PointerButton {
    match event.buttonNumber() {
        2 => PointerButton::Middle,
        n => PointerButton::Other(n.clamp(0, 255) as u8),
    }
}

fn event_mods(event: &NSEvent) -> Modifiers {
    modifiers(event.modifierFlags().0)
}

fn key_event(event: &NSEvent, pressed: bool) -> KeyEvent {
    let mods = event_mods(event);
    let unmodified = event.charactersByApplyingModifiers(NSEventModifierFlags::empty()).map(|s| s.to_string()).unwrap_or_default();
    let key = match named_key(event.keyCode()) {
        Some(named) => Key::Named(named),
        None => Key::Character(shortcut_char(&unmodified).unwrap_or('\0')),
    };
    let text = if pressed && !mods.cmd && !mods.ctrl { event.characters().and_then(|s| typed_text(&s.to_string())) } else { None };
    let repeat = pressed && event.isARepeat();
    KeyEvent { key, mods, text, pressed, repeat }
}

/// The text of an `NSString` or `NSAttributedString`, the two things the input method hands over.
fn string_of(object: &AnyObject) -> String {
    if let Some(s) = object.downcast_ref::<NSAttributedString>() {
        s.string().to_string()
    } else if let Some(s) = object.downcast_ref::<NSString>() {
        s.to_string()
    } else {
        String::new()
    }
}

fn cursor(icon: CursorIcon) -> Retained<NSCursor> {
    match icon {
        CursorIcon::Arrow => NSCursor::arrowCursor(),
        CursorIcon::Pointer => NSCursor::pointingHandCursor(),
        CursorIcon::Text => NSCursor::IBeamCursor(),
        CursorIcon::Grab => NSCursor::openHandCursor(),
        CursorIcon::Grabbing => NSCursor::closedHandCursor(),
        CursorIcon::ResizeHorizontal => NSCursor::columnResizeCursor(),
        CursorIcon::ResizeVertical => NSCursor::rowResizeCursor(),
        CursorIcon::NotAllowed => NSCursor::operationNotAllowedCursor(),
        CursorIcon::Crosshair => NSCursor::crosshairCursor(),
    }
}

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

// ---- delegates -----------------------------------------------------------------------------------

pub struct WindowIvars {
    view: Weak<VuiView>,
    quit_on_close: bool,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and the delegate does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "VuiWindowDelegate"]
    #[ivars = WindowIvars]
    struct WindowDelegate;

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
    fn new(mtm: MainThreadMarker, view: &VuiView, quit_on_close: bool) -> Retained<WindowDelegate> {
        let this = Self::alloc(mtm).set_ivars(WindowIvars { view: Weak::from_retained(&view.retain()), quit_on_close });
        unsafe { msg_send![super(this), init] }
    }
}

// ---- the menu bar --------------------------------------------------------------------------------

fn item(mtm: MainThreadMarker, title: &str, action: Option<Sel>, key: &str, mods: NSEventModifierFlags) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &NSString::from_str(title), action, &NSString::from_str(key))
    };
    item.setKeyEquivalentModifierMask(mods);
    item
}

fn submenu(mtm: MainThreadMarker, title: &str, items: &[Retained<NSMenuItem>]) -> Retained<NSMenuItem> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
    for i in items {
        menu.addItem(i);
    }
    let holder = item(mtm, title, None, "", NSEventModifierFlags::empty());
    holder.setSubmenu(Some(&menu));
    holder
}

fn main_menu(mtm: MainThreadMarker, name: &str) -> Retained<NSMenu> {
    let cmd = NSEventModifierFlags::Command;
    let bar = NSMenu::new(mtm);
    bar.addItem(&submenu(
        mtm,
        name,
        &[
            item(mtm, &format!("Hide {name}"), Some(sel!(hide:)), "h", cmd),
            item(mtm, "Hide Others", Some(sel!(hideOtherApplications:)), "h", cmd | NSEventModifierFlags::Option),
            item(mtm, "Show All", Some(sel!(unhideAllApplications:)), "", NSEventModifierFlags::empty()),
            NSMenuItem::separatorItem(mtm),
            item(mtm, &format!("Quit {name}"), Some(sel!(terminate:)), "q", cmd),
        ],
    ));
    bar.addItem(&submenu(
        mtm,
        "Edit",
        &[
            item(mtm, "Undo", Some(sel!(undo:)), "z", cmd),
            item(mtm, "Redo", Some(sel!(redo:)), "z", cmd | NSEventModifierFlags::Shift),
            NSMenuItem::separatorItem(mtm),
            item(mtm, "Cut", Some(sel!(cut:)), "x", cmd),
            item(mtm, "Copy", Some(sel!(copy:)), "c", cmd),
            item(mtm, "Paste", Some(sel!(paste:)), "v", cmd),
            item(mtm, "Select All", Some(sel!(selectAll:)), "a", cmd),
        ],
    ));
    bar.addItem(&submenu(
        mtm,
        "Window",
        &[
            item(mtm, "Minimize", Some(sel!(performMiniaturize:)), "m", cmd),
            item(mtm, "Zoom", Some(sel!(performZoom:)), "", NSEventModifierFlags::empty()),
            item(mtm, "Close", Some(sel!(performClose:)), "w", cmd),
        ],
    ));
    bar
}

// ---- the application -----------------------------------------------------------------------------

thread_local! {
    /// The views of the windows opened so far, numbered as their `Proxy`s know them.
    static VIEWS: RefCell<Vec<Weak<VuiView>>> = const { RefCell::new(Vec::new()) };
}

/// From any thread: has the main thread hand window `id` the messages queued for it.
pub(crate) fn wake(id: usize) {
    DispatchQueue::main().exec_async(move || {
        if let Some(view) = VIEWS.with(|views| views.borrow().get(id).and_then(Weak::load)) {
            view.deliver();
        }
    });
}

type MakePass = Box<dyn Fn(&wgpu::Device, wgpu::TextureFormat) -> Box<dyn CustomPass>>;

/// The running app: its windows and overlays, which share one GPU device and one set of fonts and
/// pictures. Build it, open what the app shows, then `run`.
pub struct Application {
    mtm: MainThreadMarker,
    res: Resources,
    /// Until the first window picks a device to suit its surface.
    instance: Option<wgpu::Instance>,
    gpu: Option<Gpu>,
    passes: Vec<(&'static str, MakePass)>,
    /// Kept alive for as long as the app runs.
    windows: Vec<(Retained<NSWindow>, Retained<VuiView>, Retained<WindowDelegate>)>,
    snapshot: Option<PathBuf>,
    snapshot_quit: bool,
    snapshots: Rc<Cell<usize>>,
    /// The pointer watchers of overlays that let clicks through their empty space.
    monitors: Vec<Retained<AnyObject>>,
}

impl Application {
    /// Sets the process up as an app called `name`, with the standard menus.
    pub fn new(name: &str) -> Result<Application, Error> {
        let mtm = MainThreadMarker::new().ok_or(Error::NotMainThread)?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.setMainMenu(Some(&main_menu(mtm, name)));
        Ok(Application {
            mtm,
            res: Resources::new(),
            instance: Some(wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle())),
            gpu: None,
            passes: Vec::new(),
            windows: Vec::new(),
            snapshot: std::env::var_os("VUI_SNAPSHOT").map(PathBuf::from),
            snapshot_quit: std::env::var_os("VUI_SNAPSHOT_QUIT").is_some(),
            snapshots: Rc::new(Cell::new(0)),
            monitors: Vec::new(),
        })
    }

    /// The fonts and pictures every window draws with, for registering images.
    pub fn resources(&self) -> &Resources {
        &self.res
    }

    /// Gives windows opened from now on a custom shader pass, which `PaintCx::custom` commands named
    /// `name` draw with.
    pub fn pass(&mut self, name: &'static str, make: impl Fn(&wgpu::Device, wgpu::TextureFormat) -> Box<dyn CustomPass> + 'static) {
        self.passes.push((name, Box::new(make)));
    }

    /// The screen overlays go on, measured now: displays come and go.
    pub fn screen(&self) -> Screen {
        describe(&overlay_screen(self.mtm))
    }

    /// Every display's frame in points, in the coordinates screenshots use: the origin at the top left
    /// of the main display, y growing down.
    pub fn displays(&self) -> Vec<vui_core::Rect> {
        let screens = NSScreen::screens(self.mtm);
        let main = screens.firstObject().map_or(0.0, |s| s.frame().size.height);
        screens
            .iter()
            .map(|s| {
                let f = s.frame();
                vui_core::rect(f.origin.x as f32, (main - f.origin.y - f.size.height) as f32, f.size.width as f32, f.size.height as f32)
            })
            .collect()
    }

    /// Opens a window running `app`, centred on the screen and in front.
    pub fn window<A: App>(&mut self, options: WindowOptions, app: A) -> Result<Proxy<A::Msg>, Error> {
        let mtm = self.mtm;
        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(options.size.w as f64, options.size.h as f64));
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullSizeContentView;
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(NSWindow::alloc(mtm), content, style, NSBackingStoreType::Buffered, false)
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(&options.title));
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        window.setContentMinSize(NSSize::new(options.min_size.w as f64, options.min_size.h as f64));
        window.setAcceptsMouseMovedEvents(true);
        window.center();

        let proxy = self.attach(window, content, options.size, options.quit_on_close, false, app)?;
        let (window, view, _) = self.windows.last().expect("just attached");
        window.makeKeyAndOrderFront(None);
        window.makeFirstResponder(Some(&**view));
        view.start_display_link();
        view.request_frame();
        Ok(proxy)
    }

    /// Opens an overlay running `app`: see [`OverlayOptions`].
    pub fn overlay<A: App>(&mut self, options: OverlayOptions, app: A) -> Result<Proxy<A::Msg>, Error> {
        let mtm = self.mtm;
        let ns_screen = match options.place {
            Place::Display(n) => NSScreen::screens(mtm).iter().nth(n).unwrap_or_else(|| overlay_screen(mtm)),
            _ => overlay_screen(mtm),
        };
        let screen = ns_screen.frame();
        let (w, h) = (options.size.w as f64, options.size.h as f64);
        let origin = match options.place {
            Place::Top => NSPoint::new(screen.origin.x + ((screen.size.width - w) / 2.0).round(), screen.origin.y + screen.size.height - h),
            Place::Right => {
                let visible = ns_screen.visibleFrame();
                NSPoint::new(screen.origin.x + screen.size.width - w, visible.origin.y + visible.size.height - h)
            }
            Place::Display(_) => NSPoint::new(screen.origin.x, screen.origin.y + screen.size.height - h),
        };
        let frame = NSRect::new(origin, NSSize::new(w, h));
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(NSPanel::alloc(mtm), frame, style, NSBackingStoreType::Buffered, false);
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(true);
        panel.setHidesOnDeactivate(false);
        panel.setOpaque(false);
        panel.setHasShadow(false);
        panel.setMovable(false);
        panel.setIgnoresMouseEvents(options.click_through);
        // Above the menu bar, so it can cover the notch.
        panel.setLevel(NSMainMenuWindowLevel + 3);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        panel.setBackgroundColor(Some(&NSColor::clearColor()));

        let content = NSRect::new(NSPoint::new(0.0, 0.0), frame.size);
        let proxy = self.attach(panel.into_super(), content, options.size, false, true, app)?;
        let (window, view, _) = self.windows.last().expect("just attached");
        if options.through_empty {
            self.monitors.extend(follow_pointer(window, view));
        }
        let (window, view, _) = self.windows.last().expect("just attached");
        window.orderFrontRegardless();
        view.start_display_link();
        view.request_frame();
        Ok(proxy)
    }

    /// Puts a view running `app` in `window`, with a surface and a renderer.
    fn attach<A: App>(
        &mut self,
        window: Retained<NSWindow>,
        content: NSRect,
        size: Size,
        quit_on_close: bool,
        transparent: bool,
        app: A,
    ) -> Result<Proxy<A::Msg>, Error> {
        let mtm = self.mtm;
        let view = VuiView::new(mtm, content);
        window.setContentView(Some(&view));
        let delegate = WindowDelegate::new(mtm, &view, quit_on_close);
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        let scale = window.backingScaleFactor() as f32;
        let mut ui = Ui::with_app(self.res.clone(), size, scale, app);
        ui.set_clipboard(Rc::new(Pasteboard));
        ui.set_transparent(transparent);
        ui.set_system_mode(view.system_mode());
        if !transparent {
            window.setBackgroundColor(Some(&ns_color(ui.theme().palette.side)));
        }

        // The surface on the view, and, for the first window, a device that can present to it.
        let instance = match (&self.gpu, &self.instance) {
            (Some(gpu), _) => &gpu.instance,
            (None, Some(instance)) => instance,
            (None, None) => unreachable!("the instance moves into the GPU"),
        };
        let handle = AppKitWindowHandle::new(NonNull::from(&*view).cast());
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
            raw_window_handle: RawWindowHandle::AppKit(handle),
        };
        // SAFETY: the view outlives the surface: the app keeps every view until it exits, and the
        // surface is dropped with the host, which the view owns.
        let surface = unsafe { instance.create_surface_unsafe(target) }.map_err(|e| Error::Surface(e.to_string()))?;
        if self.gpu.is_none() {
            let instance = self.instance.take().expect("no device yet");
            self.gpu = Some(Gpu::for_surface(instance, &surface).map_err(|e| Error::Gpu(e.to_string()))?);
        }
        let gpu = self.gpu.as_ref().expect("made above");
        let format = Window::<A::Msg>::pick_format(&surface, &gpu.adapter);
        let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), format, self.res.clone());
        for (name, make) in &self.passes {
            renderer.add_pass(name, make(&gpu.device, format));
        }

        let n = self.windows.len();
        let path = self.snapshot.as_ref().map(|p| if n == 0 { p.clone() } else { numbered(p, n) });
        if path.is_some() {
            self.snapshots.set(self.snapshots.get() + 1);
        }
        let snapshot = Snapshot { path, pending: self.snapshots.clone(), quit: self.snapshot_quit };
        let (tx, inbox) = mpsc::channel();
        let host = Window::new(ui, surface, &gpu.adapter, renderer, inbox, snapshot);
        view.ivars().host.replace(Some(Box::new(host)));
        view.resized();

        let id = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            views.push(Weak::from_retained(&view));
            views.len() - 1
        });
        self.windows.push((window, view, delegate));
        Ok(Proxy { tx, window: id })
    }

    /// Brings the app forward and runs until it quits.
    pub fn run(self) {
        let app = NSApplication::sharedApplication(self.mtm);
        app.activate();
        app.run();
    }
}

/// `frame.png` → `frame-2.png`.
fn numbered(path: &std::path::Path, n: usize) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match path.extension() {
        Some(ext) => format!("{stem}-{n}.{}", ext.to_string_lossy()),
        None => format!("{stem}-{n}"),
    };
    path.with_file_name(name)
}

/// Watches the pointer, inside the app and out, and has `window` take clicks only while the pointer is
/// over something in it that takes them.
fn follow_pointer(window: &Retained<NSWindow>, view: &Retained<VuiView>) -> Vec<Retained<AnyObject>> {
    let check = {
        let (window, view) = (window.clone(), view.clone());
        move || {
            let at = NSEvent::mouseLocation();
            let frame = window.frame();
            let inside = at.x >= frame.origin.x
                && at.x < frame.origin.x + frame.size.width
                && at.y >= frame.origin.y
                && at.y < frame.origin.y + frame.size.height;
            let p = pt((at.x - frame.origin.x) as f32, (frame.origin.y + frame.size.height - at.y) as f32);
            let takes = inside && view.with_host(|h| h.takes_pointer(p)).unwrap_or(false);
            if window.ignoresMouseEvents() == takes {
                window.setIgnoresMouseEvents(!takes);
            }
        }
    };
    let mask = NSEventMask::MouseMoved | NSEventMask::LeftMouseDragged | NSEventMask::ScrollWheel;
    let outside = {
        let check = check.clone();
        RcBlock::new(move |_: NonNull<NSEvent>| check())
    };
    let inside = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        check();
        event.as_ptr()
    });
    let mut monitors = Vec::new();
    if let Some(m) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &outside) {
        monitors.push(m);
    }
    if let Some(m) = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &inside) } {
        monitors.push(m);
    }
    monitors
}

/// The built-in display if it has a notch, else the one with the menu bar.
fn overlay_screen(mtm: MainThreadMarker) -> Retained<NSScreen> {
    let screens = NSScreen::screens(mtm);
    let notched = screens.iter().find(|s| s.safeAreaInsets().top > 0.0);
    notched.or_else(|| screens.firstObject()).or_else(|| NSScreen::mainScreen(mtm)).expect("a Mac has a screen")
}

fn describe(screen: &NSScreen) -> Screen {
    let frame = screen.frame();
    let top = screen.safeAreaInsets().top;
    let notch = (top > 0.0).then(|| {
        let (left, right) = (screen.auxiliaryTopLeftArea(), screen.auxiliaryTopRightArea());
        size((frame.size.width - left.size.width - right.size.width) as f32, top as f32)
    });
    let visible = screen.visibleFrame();
    let menu_bar = (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
    Screen { size: size(frame.size.width as f32, frame.size.height as f32), notch, menu_bar: menu_bar.max(0.0) as f32 }
}

fn ns_color(c: Color) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(c.r as f64, c.g as f64, c.b as f64, c.a as f64)
}
