//! Popovers, menus, selects, context menus, dialogs, toasts and tooltips, driven the way a person
//! would: pressing, pressing outside, Escape, the arrow keys and Tab.

use std::time::Duration;

use vui_core::{NamedKey, pt};
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{App, Builder, Cx, IntoElement, Parent, Styled, column, label, row};
use vui_widgets::{
    Side, button, context_menu, dialog, dialog_actions, dropdown, menu_check, menu_item, menu_separator, select, toast, toasts,
};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    ToggleMenu,
    CloseMenu,
    Rename,
    Delete,
    SizeOpen(bool),
    Size(u32),
    Context(vui_core::Point),
    CloseContext,
    Ask,
    Cancel,
    Confirm,
    Saved,
    HideToast,
}

#[derive(Default)]
struct Overlays {
    menu: bool,
    size_open: bool,
    size: u32,
    context: Option<vui_core::Point>,
    asking: bool,
    toast: bool,
    log: Vec<Msg>,
}

impl App for Overlays {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        self.log.push(msg.clone());
        match msg {
            Msg::ToggleMenu => self.menu = !self.menu,
            Msg::CloseMenu | Msg::Rename | Msg::Delete => self.menu = false,
            Msg::SizeOpen(open) => self.size_open = open,
            Msg::Size(n) => {
                self.size = n;
                self.size_open = false;
            }
            Msg::Context(p) => self.context = Some(p),
            Msg::CloseContext => self.context = None,
            Msg::Ask => self.asking = true,
            Msg::Cancel | Msg::Confirm => self.asking = false,
            Msg::Saved => {
                self.toast = true;
                cx.after(Duration::from_secs(3), Msg::HideToast);
            }
            Msg::HideToast => self.toast = false,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().padding(16.0).gap(12.0).children((
            row().gap(8.0).children((
                dropdown(
                    button("Actions").key("actions").on_click(Msg::ToggleMenu),
                    self.menu,
                    Msg::CloseMenu,
                    (
                        menu_item("Rename").key("rename").detail("⌘R").on_click(Msg::Rename),
                        menu_separator(),
                        menu_item("Delete").key("delete").on_click(Msg::Delete),
                    ),
                )
                .side(Side::Below),
                button("Ask").key("ask").on_click(Msg::Ask),
                button("Save").key("save").on_click(Msg::Saved).tooltip("Saves the note"),
            )),
            select([(12, "Small".to_string()), (14, "Medium".to_string()), (18, "Large".to_string())], &self.size, self.size_open, Msg::SizeOpen, Msg::Size)
                .match_width(),
            context_menu(
                label("Right-click me").key("target"),
                self.context,
                Msg::Context,
                Msg::CloseContext,
                (menu_check("Pinned", true).key("pinned").on_click(Msg::CloseContext),),
            ),
            self.asking.then(|| {
                dialog(
                    Msg::Cancel,
                    (
                        label("Delete this note?"),
                        dialog_actions((button("Cancel").key("cancel").on_click(Msg::Cancel), button("Delete").key("confirm").danger().on_click(Msg::Confirm))),
                    ),
                )
            }),
            toasts((self.toast.then(|| toast("Saved", None).key("toast")),)),
        ))
    }
}

fn app() -> TestUi<Msg> {
    let mut ui = TestUi::with_app(600.0, 400.0, Overlays { size: 14, ..Overlays::default() });
    ui.settle();
    ui
}

fn shows(ui: &mut TestUi<Msg>, text: &str) -> bool {
    texts(&ui.scene()).iter().any(|t| t == text)
}

#[test]
fn a_dropdown_opens_under_its_button_and_closes_on_a_choice() {
    let mut ui = app();
    assert!(!shows(&mut ui, "Rename"));
    ui.click_key("actions").settle();
    assert!(shows(&mut ui, "Rename"));
    let (button, item) = (ui.rect("actions"), ui.rect("rename"));
    assert!(item.y >= button.y + button.h, "the menu is under the button");
    ui.click_key("rename");
    assert!(!ui.app::<Overlays>().menu);
    assert_eq!(ui.app::<Overlays>().log.last(), Some(&Msg::Rename));
}

#[test]
fn pressing_outside_or_the_button_again_closes_the_menu_without_pressing_through() {
    let mut ui = app();
    ui.click_key("actions");
    ui.click_key("ask");
    assert!(!ui.app::<Overlays>().menu);
    assert!(!ui.app::<Overlays>().asking, "the press that closed the menu did not reach Ask");
    ui.click_key("actions");
    assert!(ui.app::<Overlays>().menu);
    ui.click_key("actions");
    assert!(!ui.app::<Overlays>().menu, "the button closes what it opened");
}

#[test]
fn a_menu_is_driven_by_the_keyboard() {
    let mut ui = app();
    ui.click_key("actions");
    assert_eq!(ui.ui.focused_key(), Some("rename".into()), "focus moves into the menu");
    ui.key(NamedKey::ArrowDown);
    assert_eq!(ui.ui.focused_key(), Some("delete".into()));
    ui.key(NamedKey::Enter);
    assert_eq!(ui.app::<Overlays>().log.last(), Some(&Msg::Delete));
    ui.click_key("actions");
    ui.key(NamedKey::Escape);
    assert!(!ui.app::<Overlays>().menu);
}

#[test]
fn a_select_shows_the_choice_and_picks_another() {
    let mut ui = app();
    assert!(shows(&mut ui, "Medium"));
    assert!(!shows(&mut ui, "Large"));
    ui.click(pt(100.0, 70.0)).settle();
    assert!(ui.app::<Overlays>().size_open);
    assert!(shows(&mut ui, "Large"));
    let large = texts(&ui.scene());
    assert!(large.contains(&"Small".to_string()));
    ui.key(NamedKey::ArrowDown);
    ui.key(NamedKey::ArrowDown);
    ui.key(NamedKey::Enter);
    assert_eq!(ui.app::<Overlays>().size, 18);
    assert!(!ui.app::<Overlays>().size_open);
}

#[test]
fn a_context_menu_opens_where_the_secondary_click_landed() {
    let mut ui = app();
    let target = ui.center("target");
    ui.right_click(target).settle();
    assert_eq!(ui.app::<Overlays>().context, Some(target));
    let menu = ui.rect("pinned");
    assert!((menu.x - target.x).abs() < 12.0 && (menu.y - target.y).abs() < 12.0, "{menu:?} near {target:?}");
    ui.key(NamedKey::Escape);
    assert_eq!(ui.app::<Overlays>().context, None);
}

#[test]
fn a_context_menu_near_the_edge_stays_in_the_window() {
    let mut ui = app();
    ui.update_app(|a: &mut Overlays| a.context = Some(pt(590.0, 390.0)));
    ui.settle();
    let menu = ui.rect("pinned");
    assert!(menu.x + menu.w <= 600.0 && menu.y + menu.h <= 400.0, "{menu:?}");
}

#[test]
fn a_dialog_dims_the_page_traps_tab_and_closes_on_escape() {
    let mut ui = app();
    ui.click_key("ask");
    ui.settle();
    assert!(shows(&mut ui, "Delete this note?"));
    assert_eq!(ui.ui.focused_key(), Some("cancel".into()), "focus moves into the dialog");
    ui.tab();
    assert_eq!(ui.ui.focused_key(), Some("confirm".into()));
    ui.tab();
    assert_eq!(ui.ui.focused_key(), Some("cancel".into()), "Tab stays inside");
    // The page under the scrim cannot be pressed: the press dismisses instead.
    ui.click_key("save");
    assert!(!ui.app::<Overlays>().asking);
    assert!(!ui.app::<Overlays>().toast);
    ui.click_key("ask");
    ui.key(NamedKey::Escape);
    assert!(!ui.app::<Overlays>().asking);
}

#[test]
fn a_toast_shows_in_the_corner_and_leaves_on_its_timer() {
    let mut ui = app();
    ui.click_key("save");
    ui.settle();
    let toast = ui.rect("toast");
    assert!(toast.x + toast.w > 560.0 && toast.y + toast.h > 360.0, "{toast:?}");
    ui.advance(3.0);
    assert!(!shows(&mut ui, "Saved"));
}

#[test]
fn a_tooltip_shows_after_the_pointer_rests_and_hides_on_a_press() {
    let mut ui = app();
    ui.move_to_key("save");
    ui.advance(0.3);
    assert!(!shows(&mut ui, "Saves the note"));
    ui.advance(0.4);
    assert!(shows(&mut ui, "Saves the note"));
    let save = ui.center("save");
    ui.press(save);
    assert!(!shows(&mut ui, "Saves the note"));
}
