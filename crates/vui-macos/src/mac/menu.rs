use super::*;

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

pub(super) fn main_menu(mtm: MainThreadMarker, name: &str) -> Retained<NSMenu> {
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
