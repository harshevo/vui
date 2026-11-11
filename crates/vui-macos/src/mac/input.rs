//! Input: events from AppKit become vui events, and what the UI asks for back (cursor, keys,
//! clipboard, quit) is applied.

use super::*;

impl VuiView {
    /// Runs `f` on the host, unless there is none yet or a call into it is already under way (AppKit
    /// can call back into the view from inside one).
    pub(super) fn with_host<R>(&self, f: impl FnOnce(&mut dyn Host) -> R) -> Option<R> {
        let mut slot = self.ivars().host.try_borrow_mut().ok()?;
        let host = slot.as_mut()?;
        Some(f(host.as_mut()))
    }

    pub(super) fn send(&self, event: Event) {
        let effects = self.with_host(|h| h.handle(event)).unwrap_or_default();
        self.after_input(effects);
    }

    pub(super) fn deliver(&self) {
        let effects = self.with_host(|h| h.deliver()).unwrap_or_default();
        self.after_input(effects);
    }

    pub(super) fn shortcut(&self, c: char, mods: Modifiers) {
        self.send(Event::Key(KeyEvent::press(Key::Character(c), mods)));
    }

    pub(super) fn after_input(&self, effects: Vec<Effect>) {
        self.apply(effects);
        if let (Some(through), Some(window)) = (self.with_host(|h| h.passes_clicks()).flatten(), self.window())
            && window.ignoresMouseEvents() != through
        {
            window.setIgnoresMouseEvents(through);
        }
        self.sync_keys();
        if self.with_host(|h| h.needs_paint()).unwrap_or(false) {
            self.request_frame();
        } else if let Some(wake) = self.with_host(|h| h.next_wake()).flatten() {
            // An app timer set by this update, with nothing else to draw until it is due.
            self.wake_in(wake);
        }
        self.check_quit();
    }

    /// An overlay that can take the keyboard takes it when its app asks, and gives it back when the
    /// app lets go. The app the person is in stays active throughout.
    pub(super) fn sync_keys(&self) {
        let (Some(want), Some(window)) = (self.with_host(|h| h.takes_keys()).flatten(), self.window()) else {
            return;
        };
        // Only a `KeyPanel` among overlays; ordinary windows are left to the window system.
        let key_panel = window.styleMask().contains(NSWindowStyleMask::NonactivatingPanel) && window.canBecomeKeyWindow();
        if !key_panel || window.isKeyWindow() == want {
            return;
        }
        if want {
            window.makeKeyWindow();
            window.makeFirstResponder(Some(self));
        } else {
            // Taking the panel off the screen list hands the keyboard back to the active app's own
            // key window; it comes straight back on, still above the menu bar.
            window.orderOut(None);
            window.orderFrontRegardless();
        }
    }

    pub(super) fn apply(&self, effects: Vec<Effect>) {
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
                Effect::Open { id, dialog } => self.open_dialog(id, &dialog, false),
                Effect::Save { id, dialog } => self.open_dialog(id, &dialog, true),
                Effect::Notify(n) => self.notify(&n),
                Effect::FullScreen(on) => self.set_full_screen(on),
                Effect::Menus(menus) => self.set_menus(&menus),
                Effect::StatusItem(item) => self.set_status_item(item.as_ref()),
                _ => {}
            }
        }
    }

    pub(super) fn check_quit(&self) {
        if self.with_host(|h| h.quit_requested()).unwrap_or(false) {
            let app = NSApplication::sharedApplication(self.mtm());
            app.terminate(None);
        }
    }

    pub(super) fn point(&self, event: &NSEvent) -> vui_core::Point {
        let p = self.convertPoint_fromView(event.locationInWindow(), None);
        pt(p.x as f32, p.y as f32)
    }

    pub(super) fn pointer_down(&self, event: &NSEvent, button: PointerButton) {
        if let Some(window) = self.window() {
            window.makeFirstResponder(Some(self));
        }
        let pos = self.point(event);
        let click_count = event.clickCount().clamp(1, 255) as u8;
        self.send(Event::PointerDown { pos, button, mods: event_mods(event), click_count });
    }

    pub(super) fn pointer_up(&self, event: &NSEvent, button: PointerButton) {
        let pos = self.point(event);
        self.send(Event::PointerUp { pos, button, mods: event_mods(event) });
    }

    pub(super) fn pointer_moved(&self, event: &NSEvent) {
        let pos = self.point(event);
        self.send(Event::PointerMoved { pos, mods: event_mods(event) });
    }
}

pub(super) fn other_button(event: &NSEvent) -> PointerButton {
    match event.buttonNumber() {
        2 => PointerButton::Middle,
        n => PointerButton::Other(n.clamp(0, 255) as u8),
    }
}

pub(super) fn event_mods(event: &NSEvent) -> Modifiers {
    modifiers(event.modifierFlags().0)
}

pub(super) fn key_event(event: &NSEvent, pressed: bool) -> KeyEvent {
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
pub(super) fn string_of(object: &AnyObject) -> String {
    if let Some(s) = object.downcast_ref::<NSAttributedString>() {
        s.string().to_string()
    } else if let Some(s) = object.downcast_ref::<NSString>() {
        s.to_string()
    } else {
        String::new()
    }
}

pub(super) fn cursor(icon: CursorIcon) -> Retained<NSCursor> {
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

// ---- delegates -----------------------------------------------------------------------------------
