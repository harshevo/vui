//! The Win32 clipboard, as UTF-16 text (`CF_UNICODETEXT`), through the few `user32` and
//! `kernel32` calls it takes.

use std::ffi::c_void;

type Handle = *mut c_void;

const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(owner: Handle) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(format: u32) -> Handle;
    fn SetClipboardData(format: u32, data: Handle) -> Handle;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalFree(memory: Handle) -> Handle;
    fn GlobalLock(memory: Handle) -> *mut c_void;
    fn GlobalUnlock(memory: Handle) -> i32;
}

/// Holds the clipboard open, and closes it when dropped. Another app may hold it for a moment, so
/// opening is tried a few times.
struct Open;

impl Open {
    fn new() -> Option<Open> {
        for _ in 0..5 {
            // SAFETY: a null owner ties the clipboard to this task; the guard closes it.
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return Some(Open);
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        None
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: opened in `new`.
        unsafe { CloseClipboard() };
    }
}

pub fn get() -> Option<String> {
    let _open = Open::new()?;
    // SAFETY: the clipboard is open; the handle it returns stays valid until it is closed, and its
    // memory holds a NUL-terminated UTF-16 string, which is read no further than the NUL.
    unsafe {
        let data = GetClipboardData(CF_UNICODETEXT);
        if data.is_null() {
            return None;
        }
        let ptr = GlobalLock(data) as *const u16;
        if ptr.is_null() {
            return None;
        }
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        GlobalUnlock(data);
        Some(text)
    }
}

pub fn set(text: &str) {
    let Some(_open) = Open::new() else { return };
    let wide: Vec<u16> = text.encode_utf16().chain([0]).collect();
    // SAFETY: the clipboard is open; the block is allocated big enough for `wide`, filled while
    // locked, and handed to the system, which owns it from then on unless the hand-over fails.
    unsafe {
        EmptyClipboard();
        let memory = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
        if memory.is_null() {
            return;
        }
        let ptr = GlobalLock(memory) as *mut u16;
        if ptr.is_null() {
            GlobalFree(memory);
            return;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        GlobalUnlock(memory);
        if SetClipboardData(CF_UNICODETEXT, memory).is_null() {
            GlobalFree(memory);
        }
    }
}
