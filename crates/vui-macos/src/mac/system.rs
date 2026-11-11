//! What an app asks of macOS beyond its window: open and save sheets, notifications, full screen,
//! its menus in the menu bar with their shortcuts, and an item in the menu bar.

use std::path::PathBuf;

use objc2_app_kit::{NSControlStateValueOn, NSModalResponseOK, NSOpenPanel, NSSavePanel, NSStatusBar, NSVariableStatusItemLength};
use objc2_foundation::NSURL;
use vui_runtime::FileDialog;
use vui_runtime::platform::{ItemSpec, MenuSpec, Shortcut};

use super::*;

/// Marks the menu-bar items that hold the app's own menus, so they can be swapped.
const APP_MENU: isize = 0x7675_6920;

impl VuiView {
    pub(super) fn open_dialog(&self, id: u64, dialog: &FileDialog, save: bool) {
        let mtm = self.mtm();
        let panel: Retained<NSSavePanel> = if save {
            let panel = NSSavePanel::savePanel(mtm);
            if let Some(name) = &dialog.file_name {
                panel.setNameFieldStringValue(&NSString::from_str(name));
            }
            panel
        } else {
            let panel = NSOpenPanel::openPanel(mtm);
            panel.setCanChooseFiles(!dialog.folders);
            panel.setCanChooseDirectories(dialog.folders);
            panel.setAllowsMultipleSelection(dialog.multiple);
            Retained::into_super(panel)
        };
        if let Some(title) = &dialog.title {
            panel.setMessage(Some(&NSString::from_str(title)));
        }
        if let Some(dir) = &dialog.directory {
            panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(&NSString::from_str(&dir.to_string_lossy()))));
        }
        if !dialog.extensions.is_empty() {
            let types: Vec<Retained<NSString>> = dialog.extensions.iter().map(|e| NSString::from_str(e)).collect();
            // The content-type form needs another crate; the extension list still works.
            #[allow(deprecated)]
            panel.setAllowedFileTypes(Some(&NSArray::from_retained_slice(&types)));
        }
        let view = Weak::from_retained(&self.retain());
        let shown = panel.clone();
        let done = RcBlock::new(move |response: isize| {
            let paths = if response == NSModalResponseOK { chosen(&shown, save) } else { Vec::new() };
            if let Some(view) = view.load() {
                let effects = view.with_host(|h| h.answer(id, paths)).unwrap_or_default();
                view.after_input(effects);
            }
        });
        match self.window() {
            Some(window) => panel.beginSheetModalForWindow_completionHandler(&window, &done),
            None => {
                let response = panel.runModal();
                done.call((response,));
            }
        }
    }

    pub(super) fn set_full_screen(&self, on: Option<bool>) {
        let Some(window) = self.window() else { return };
        let is = window.styleMask().contains(NSWindowStyleMask::FullScreen);
        if on.is_none_or(|on| on != is) {
            window.toggleFullScreen(None);
        }
    }

    pub(super) fn notify(&self, n: &vui_runtime::Notification) {
        // UNUserNotificationCenter needs another crate; this older centre needs none.
        #[allow(deprecated)]
        {
            use objc2_foundation::{NSUserNotification, NSUserNotificationCenter, NSUserNotificationDefaultSoundName};
            let note = NSUserNotification::new();
            note.setTitle(Some(&NSString::from_str(&n.title)));
            if !n.body.is_empty() {
                note.setInformativeText(Some(&NSString::from_str(&n.body)));
            }
            if n.sound {
                note.setSoundName(Some(unsafe { NSUserNotificationDefaultSoundName }));
            }
            NSUserNotificationCenter::defaultUserNotificationCenter().deliverNotification(&note);
        }
    }

    /// Puts the app's menus in the menu bar in place of the ones before.
    pub(super) fn set_menus(&self, menus: &[MenuSpec]) {
        let mtm = self.mtm();
        let Some(bar) = NSApplication::sharedApplication(mtm).mainMenu() else { return };
        for i in (0..bar.numberOfItems()).rev() {
            if bar.itemAtIndex(i).is_some_and(|item| item.tag() == APP_MENU) {
                bar.removeItemAtIndex(i);
            }
        }
        for spec in menus {
            let holder = NSMenuItem::new(mtm);
            holder.setTitle(&NSString::from_str(&spec.title));
            holder.setTag(APP_MENU);
            holder.setSubmenu(Some(&self.menu(spec)));
            // File goes after the app menu, as on every Mac app; the rest before Window, which the
            // standard menu bar ends with.
            let at = if spec.title == "File" { 1.min(bar.numberOfItems()) } else { (bar.numberOfItems() - 1).max(0) };
            bar.insertItem_atIndex(&holder, at);
        }
    }

    /// Shows, changes or removes the app's item in the menu bar.
    pub(super) fn set_status_item(&self, spec: Option<&MenuSpec>) {
        let mtm = self.mtm();
        let mut slot = self.ivars().status.borrow_mut();
        let Some(spec) = spec else {
            if let Some(item) = slot.take() {
                NSStatusBar::systemStatusBar().removeStatusItem(&item);
            }
            return;
        };
        let item = slot.get_or_insert_with(|| NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength));
        if let Some(button) = item.button(mtm) {
            button.setTitle(&NSString::from_str(&spec.title));
        }
        item.setMenu(Some(&self.menu(spec)));
    }

    /// A menu whose items report back to this view.
    fn menu(&self, spec: &MenuSpec) -> Retained<NSMenu> {
        let mtm = self.mtm();
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(&spec.title));
        menu.setAutoenablesItems(false);
        for entry in &spec.items {
            match entry {
                ItemSpec::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
                ItemSpec::Item { id, title, shortcut, enabled, checked } => {
                    let (key, mods) = shortcut.map_or((String::new(), NSEventModifierFlags::empty()), key_equivalent);
                    let item = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            NSMenuItem::alloc(mtm),
                            &NSString::from_str(title),
                            Some(sel!(vuiMenuChosen:)),
                            &NSString::from_str(&key),
                        )
                    };
                    item.setKeyEquivalentModifierMask(mods);
                    item.setTag(*id as isize);
                    item.setEnabled(*enabled);
                    if *checked {
                        item.setState(NSControlStateValueOn);
                    }
                    let _: () = unsafe { msg_send![&*item, setTarget: self] };
                    menu.addItem(&item);
                }
            }
        }
        menu
    }

    pub(super) fn menu_chosen(&self, id: u64) {
        let effects = self.with_host(|h| h.choose_menu(id)).unwrap_or_default();
        self.after_input(effects);
    }

    /// Puts this window's app menus back in the menu bar: it has come to the front.
    pub(super) fn reinstall_menus(&self) {
        let effects = self.with_host(|h| h.menu_effects()).unwrap_or_default();
        self.apply(effects);
    }
}

fn key_equivalent(s: Shortcut) -> (String, NSEventModifierFlags) {
    let mut mods = NSEventModifierFlags::empty();
    for (on, flag) in [
        (s.mods.cmd, NSEventModifierFlags::Command),
        (s.mods.shift, NSEventModifierFlags::Shift),
        (s.mods.alt, NSEventModifierFlags::Option),
        (s.mods.ctrl, NSEventModifierFlags::Control),
    ] {
        if on {
            mods |= flag;
        }
    }
    (s.key.to_lowercase().collect(), mods)
}

fn chosen(panel: &NSSavePanel, save: bool) -> Vec<PathBuf> {
    let path = |url: &NSURL| url.path().map(|p| PathBuf::from(p.to_string()));
    if save {
        return panel.URL().and_then(|u| path(&u)).into_iter().collect();
    }
    let Some(open) = panel.downcast_ref::<NSOpenPanel>() else { return Vec::new() };
    open.URLs().iter().filter_map(|u| path(&u)).collect()
}
