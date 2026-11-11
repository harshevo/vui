//! The system around a window: the app's own menus with shortcuts, open and save sheets, full
//! screen, a notification and an item in the menu bar, all from `update` and plain messages.
//!
//! ```sh
//! cargo run -p vui --example notes
//! ```

use std::path::PathBuf;

use vui::prelude::*;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Open,
    Opened(Vec<PathBuf>),
    Export,
    Exported(Option<PathBuf>),
    ToggleHidden,
    FullScreen,
    Ping,
    Answered(NotificationResponse),
}

#[derive(Default)]
struct Notes {
    files: Vec<PathBuf>,
    exported: Option<PathBuf>,
    show_paths: bool,
}

impl App for Notes {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Open => cx.open_files(FileDialog::new().title("Pick notes to open").extensions(["md", "txt"]).multiple(), Msg::Opened),
            Msg::Opened(files) => self.files.extend(files),
            Msg::Export => cx.save_file(FileDialog::new().file_name("notes.md"), Msg::Exported),
            Msg::Exported(path) => {
                if let Some(path) = path {
                    let note = Notification::new("Exported").body(path.display().to_string()).action("Show full paths");
                    cx.notify_then(note, Msg::Answered);
                    self.exported = Some(path);
                }
            }
            Msg::ToggleHidden => self.show_paths = !self.show_paths,
            Msg::FullScreen => cx.toggle_full_screen(),
            Msg::Ping => cx.notify(Notification::new("Notes").body("Still here.").sound()),
            Msg::Answered(NotificationResponse::Action(0)) => self.show_paths = true,
            Msg::Answered(_) => {}
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let list = column().gap(4.0).children(self.files.iter().map(|f| {
            let name = if self.show_paths { f.display().to_string() } else { f.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()) };
            row().gap(8.0).padding((6.0, 8.0)).rounded(8.0).hover_bg(Tone::Hover).children((icon(icon_set::FILE), label(name).max_lines(1)))
        }).collect::<Vec<_>>());
        column().padding(24.0).gap(16.0).fill_height().children((
            row().gap(8.0).align_items(Align::Center).children((
                title("Notes").grow(1.0),
                button("Open…").on_click(Msg::Open),
                button("Export…").primary().on_click(Msg::Export),
            )),
            small("File ▸ Open (⌘O) and Export (⇧⌘S), View ▸ Full Screen (⌃⌘F), and the Notes item in the menu bar.").color(Tone::Text3),
            if self.files.is_empty() { label("No notes open yet.").color(Tone::Text2).into_element() } else { list.into_element() },
            small(self.exported.as_ref().map_or_else(String::new, |p| format!("Last exported to {}", p.display()))).color(Tone::Text3),
        ))
    }

    fn menus(&self) -> Vec<Menu<Msg>> {
        vec![
            Menu::new(
                "File",
                [
                    MenuItem::new("Open…", Msg::Open).shortcut(Shortcut::cmd('o')),
                    MenuItem::new("Export…", Msg::Export).shortcut(Shortcut::cmd_shift('s')).enabled(!self.files.is_empty()),
                ],
            ),
            Menu::new(
                "View",
                [
                    MenuItem::new("Show Full Paths", Msg::ToggleHidden).checked(self.show_paths),
                    MenuItem::separator(),
                    MenuItem::new("Full Screen", Msg::FullScreen).shortcut(Shortcut::new('f', Modifiers { cmd: true, ctrl: true, ..Modifiers::NONE })),
                ],
            ),
        ]
    }

    fn status_item(&self) -> Option<StatusItem<Msg>> {
        Some(StatusItem::new("Notes", [MenuItem::new("Open…", Msg::Open), MenuItem::separator(), MenuItem::new("Send a notification", Msg::Ping)]))
    }
}

#[cfg(target_os = "macos")]
fn main() {
    let window = WindowOptions::new("Notes").size(size(560.0, 420.0)).min_size(size(360.0, 280.0));
    if let Err(e) = vui::run(window, |_resources| Notes::default()) {
        eprintln!("{e}");
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("vui has a window backend for macOS only so far; the app still runs headless in tests.");
}
