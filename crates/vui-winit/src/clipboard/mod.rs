//! The system clipboard, for text.
//!
//! On Windows it is the Win32 clipboard, called directly. On Linux, where the clipboard belongs to
//! whichever app last copied and is reached through the display server's selection protocol, it goes
//! through the standard tools: `wl-copy` and `wl-paste` on Wayland, `xclip` or `xsel` on X11. Where
//! none is installed, and on other platforms, a clipboard inside the process stands in, so copy and
//! paste still work within the app.

use std::io::Write;
use std::process::{Command, Stdio};

use vui_runtime::{Clipboard, MemoryClipboard};

#[cfg(windows)]
mod windows;

/// A command line: the program and its arguments.
#[cfg_attr(windows, allow(dead_code))]
type Tool = (&'static str, &'static [&'static str]);

/// The tools that read and write the clipboard, best first, for a Wayland or an X11 session.
#[cfg_attr(windows, allow(dead_code))]
pub fn tools(wayland: bool) -> (&'static [Tool], &'static [Tool]) {
    if cfg!(target_os = "macos") {
        return (&[("pbpaste", &[])], &[("pbcopy", &[])]);
    }
    const WL_READ: Tool = ("wl-paste", &["--no-newline", "--type", "text/plain"]);
    const WL_WRITE: Tool = ("wl-copy", &["--type", "text/plain"]);
    const XCLIP_READ: Tool = ("xclip", &["-selection", "clipboard", "-out"]);
    const XCLIP_WRITE: Tool = ("xclip", &["-selection", "clipboard", "-in"]);
    const XSEL_READ: Tool = ("xsel", &["--clipboard", "--output"]);
    const XSEL_WRITE: Tool = ("xsel", &["--clipboard", "--input"]);
    if wayland {
        // XWayland's clipboard is synchronised with Wayland's, so the X11 tools still help.
        (&[WL_READ, XCLIP_READ, XSEL_READ], &[WL_WRITE, XCLIP_WRITE, XSEL_WRITE])
    } else {
        (&[XCLIP_READ, XSEL_READ], &[XCLIP_WRITE, XSEL_WRITE])
    }
}

/// The clipboard a window installs in its UI.
pub struct SystemClipboard {
    /// Text copied while no system clipboard could be reached.
    local: MemoryClipboard,
    #[cfg_attr(windows, allow(dead_code))]
    wayland: bool,
}

impl SystemClipboard {
    pub fn new() -> SystemClipboard {
        SystemClipboard { local: MemoryClipboard::default(), wayland: std::env::var_os("WAYLAND_DISPLAY").is_some() }
    }
}

impl Default for SystemClipboard {
    fn default() -> SystemClipboard {
        SystemClipboard::new()
    }
}

impl Clipboard for SystemClipboard {
    fn get(&self) -> Option<String> {
        #[cfg(windows)]
        if let Some(text) = windows::get() {
            return Some(text);
        }
        #[cfg(not(windows))]
        for (program, args) in tools(self.wayland).0 {
            if let Ok(out) = Command::new(program).args(*args).stderr(Stdio::null()).output()
                && out.status.success()
            {
                return Some(String::from_utf8_lossy(&out.stdout).into_owned());
            }
        }
        self.local.get()
    }

    fn set(&self, text: &str) {
        self.local.set(text);
        #[cfg(windows)]
        windows::set(text);
        #[cfg(not(windows))]
        for (program, args) in tools(self.wayland).1 {
            if write_to(program, args, text) {
                break;
            }
        }
    }
}

/// Runs `program`, hands it `text` on standard input, and says whether it took it. The tools stay
/// running to serve the selection, so they are not waited for beyond writing.
#[cfg_attr(windows, allow(dead_code))]
fn write_to(program: &str, args: &[&str], text: &str) -> bool {
    let Ok(mut child) = Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
        return false;
    };
    let wrote = child.stdin.take().is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
    // Reap it once it exits, off the main thread.
    std::thread::spawn(move || child.wait());
    wrote
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_prefers_its_own_tools_and_x11_never_uses_them() {
        if cfg!(target_os = "macos") {
            return;
        }
        let (read, write) = tools(true);
        assert_eq!(read[0].0, "wl-paste");
        assert_eq!(write[0].0, "wl-copy");
        let (read, write) = tools(false);
        assert!(read.iter().chain(write).all(|(p, _)| !p.starts_with("wl-")));
        assert_eq!(read.len(), write.len());
    }

    #[test]
    fn missing_tools_are_reported() {
        assert!(!write_to("vui-no-such-clipboard-tool", &[], "x"));
    }
}
