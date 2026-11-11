//! The view that hosts a vui UI: its state, and the AppKit overrides through which input, the
//! input method and the display link reach it.

use super::*;

/// Seconds between frames that a timer drives while the window is out of sight.
pub(super) const HIDDEN_STEP: f64 = 1.0 / 30.0;

pub struct ViewIvars {
    pub(super) host: RefCell<Option<Box<dyn Host>>>,
    pub(super) link: RefCell<Option<Retained<CADisplayLink>>>,
    /// Display-link time of the last frame; `None` while paused.
    pub(super) last_frame: Cell<Option<f64>>,
    pub(super) timer: RefCell<Option<Retained<NSTimer>>>,
    /// The key being shown to the input method. Whatever the input method does with it decides
    /// what reaches the UI; if it does nothing, the key goes through as it is.
    pub(super) pending: RefCell<Option<KeyEvent>>,
    /// Text being composed, as the input method last set it.
    pub(super) marked: RefCell<String>,
    /// Where the caret is, in view coordinates, for placing the candidate window.
    pub(super) caret: Cell<Option<Rect>>,
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
    pub(super) fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<VuiView> {
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
}
