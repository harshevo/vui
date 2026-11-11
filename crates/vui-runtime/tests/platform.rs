//! Asking the system for things: dialogs answered by the backend reach the app as its messages,
//! menus follow state and send their messages, and notifications and full screen go out as effects.

use std::path::PathBuf;

use vui_runtime::platform::{ItemSpec, MenuSpec};
use vui_runtime::testing::TestUi;
use vui_runtime::{App, Cx, Effect, FileDialog, IntoElement, Menu, MenuItem, Notification, NotificationResponse, Shortcut, StatusItem, Parent, column, label};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Open,
    Opened(Vec<PathBuf>),
    Save,
    Saved(Option<PathBuf>),
    ToggleGrid,
    Ping,
    Big,
    Export,
    Exported(NotificationResponse),
}

#[derive(Default)]
struct Editor {
    files: Vec<PathBuf>,
    saved: Option<Option<PathBuf>>,
    grid: bool,
    answers: Vec<NotificationResponse>,
}

impl App for Editor {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Open => cx.open_files(FileDialog::new().extensions(["md"]).multiple(), Msg::Opened),
            Msg::Opened(files) => self.files = files,
            Msg::Save => cx.save_file(FileDialog::new().file_name("notes.md"), Msg::Saved),
            Msg::Saved(path) => self.saved = Some(path),
            Msg::ToggleGrid => self.grid = !self.grid,
            Msg::Ping => cx.notify(Notification::new("Done").body("Export finished")),
            Msg::Big => cx.toggle_full_screen(),
            Msg::Export => cx.notify_then(Notification::new("Exported").body("notes.md").action("Open").action("Show in Finder"), Msg::Exported),
            Msg::Exported(answer) => self.answers.push(answer),
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().child(label(format!("{} files", self.files.len())))
    }

    fn menus(&self) -> Vec<Menu<Msg>> {
        vec![
            Menu::new("File", [MenuItem::new("Open…", Msg::Open).shortcut(Shortcut::cmd('o')), MenuItem::new("Save…", Msg::Save).shortcut(Shortcut::cmd('s')).enabled(!self.files.is_empty())]),
            Menu::new("View", [MenuItem::new("Grid", Msg::ToggleGrid).checked(self.grid), MenuItem::separator(), MenuItem::new("Full Screen", Msg::Big)]),
        ]
    }

    fn status_item(&self) -> Option<StatusItem<Msg>> {
        Some(StatusItem::new("Ed", [MenuItem::new("Ping", Msg::Ping), MenuItem::new("Export", Msg::Export)]))
    }
}

fn editor() -> TestUi<Msg> {
    TestUi::with_app(300.0, 200.0, Editor::default())
}

fn item<'a>(menus: &'a [MenuSpec], menu: &str, title: &str) -> &'a ItemSpec {
    menus.iter().find(|m| m.title == menu).unwrap().items.iter().find(|i| matches!(i, ItemSpec::Item { title: t, .. } if t == title)).unwrap()
}

#[test]
fn menus_and_the_status_item_go_out_once_and_follow_state() {
    let mut t = editor();
    let effects = t.effects();
    assert!(effects.iter().any(|e| matches!(e, Effect::Menus(m) if m.len() == 2)), "{effects:?}");
    assert!(effects.iter().any(|e| matches!(e, Effect::StatusItem(Some(s)) if s.title == "Ed")));
    assert!(matches!(item(t.ui.menus(), "File", "Open…"), ItemSpec::Item { shortcut: Some(s), .. } if *s == Shortcut::cmd('o')));
    assert!(matches!(item(t.ui.menus(), "File", "Save…"), ItemSpec::Item { enabled: false, .. }));

    t.choose("View", "Grid");
    assert!(t.app::<Editor>().grid);
    assert!(matches!(item(t.ui.menus(), "View", "Grid"), ItemSpec::Item { checked: true, .. }), "the tick follows state");
    assert!(t.effects().iter().any(|e| matches!(e, Effect::Menus(_))), "and the change goes out");
    t.choose("Ed", "Ping");
    assert!(t.effects().iter().any(|e| matches!(e, Effect::Notify { notification: n, .. } if n.title == "Done" && n.actions.is_empty())));
    assert!(!t.effects().iter().any(|e| matches!(e, Effect::Menus(_))), "nothing changed, nothing sent");
}

#[test]
fn dialogs_are_answered_with_the_apps_messages() {
    let mut t = editor();
    t.effects();
    t.choose("File", "Open…");
    let id = t.effects().into_iter().find_map(|e| match e {
        Effect::Open { id, dialog } => {
            assert!(dialog.multiple && dialog.extensions == ["md"]);
            Some(id)
        }
        _ => None,
    });
    let files = vec![PathBuf::from("/tmp/a.md"), PathBuf::from("/tmp/b.md")];
    t.ui.answer(id.expect("an open dialog"), files.clone());
    assert_eq!(t.app::<Editor>().files, files);

    t.choose("File", "Save…");
    let id = t.effects().into_iter().find_map(|e| if let Effect::Save { id, .. } = e { Some(id) } else { None }).unwrap();
    t.ui.answer(id, Vec::new());
    assert_eq!(t.app::<Editor>().saved, Some(None), "cancelled");
    t.ui.answer(id, vec![PathBuf::from("/x")]);
    assert_eq!(t.app::<Editor>().saved, Some(None), "an answer counts once");
}

#[test]
fn notification_actions_are_answered_with_the_apps_messages() {
    let mut t = editor();
    t.effects();
    t.choose("Ed", "Ping");
    assert!(t.ui.notifications().is_empty(), "nobody waits on a plain notification");
    t.effects();
    t.choose("Ed", "Export");
    let id = t
        .effects()
        .into_iter()
        .find_map(|e| match e {
            Effect::Notify { id, notification } => {
                assert_eq!(notification.actions, ["Open", "Show in Finder"]);
                Some(id)
            }
            _ => None,
        })
        .expect("a notification");
    assert_eq!(t.ui.notifications().len(), 1);

    t.ui.answer(id, vec![PathBuf::from("/x")]);
    assert!(t.app::<Editor>().answers.is_empty(), "paths do not answer a notification");
    t.notification_action("Exported", "Show in Finder");
    assert_eq!(t.app::<Editor>().answers, [NotificationResponse::Action(1)]);
    assert!(t.ui.notifications().is_empty(), "answered once");
    t.ui.answer(id, NotificationResponse::Dismissed);
    assert_eq!(t.app::<Editor>().answers.len(), 1, "an answer counts once");

    t.choose("Ed", "Export").choose("Ed", "Export");
    t.respond("Exported", NotificationResponse::Clicked).respond("Exported", NotificationResponse::Dismissed);
    assert_eq!(t.app::<Editor>().answers[1..], [NotificationResponse::Clicked, NotificationResponse::Dismissed]);
}

#[test]
#[should_panic(expected = "no action")]
fn pressing_a_missing_action_panics() {
    let mut t = editor();
    t.choose("Ed", "Export").notification_action("Exported", "Delete");
}

#[test]
fn full_screen_goes_out_as_an_effect() {
    let mut t = editor();
    t.effects();
    t.choose("View", "Full Screen");
    assert!(t.effects().contains(&Effect::FullScreen(None)));
}
