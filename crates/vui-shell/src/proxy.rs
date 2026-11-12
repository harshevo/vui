//! Sending a window's messages from other threads.

use std::sync::Arc;
use std::sync::mpsc::Sender;

/// Wakes a backend's event loop from any thread, so the window collects what was sent.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Sends a window's app messages from anywhere: other threads, audio callbacks, hotkey handlers. Each
/// arrives on the main thread and goes through the app's `update`, as if a widget had sent it.
pub struct Proxy<M> {
    tx: Sender<M>,
    wake: Wake,
}

impl<M> Clone for Proxy<M> {
    fn clone(&self) -> Proxy<M> {
        Proxy { tx: self.tx.clone(), wake: self.wake.clone() }
    }
}

impl<M> Proxy<M> {
    /// For backends: a proxy that queues messages on `tx` and calls `wake` after each.
    pub fn new(tx: Sender<M>, wake: Wake) -> Proxy<M> {
        Proxy { tx, wake }
    }
}

impl<M: Send + 'static> Proxy<M> {
    /// Queues `msg` for the app. False once the window is gone.
    pub fn send(&self, msg: M) -> bool {
        if self.tx.send(msg).is_err() {
            return false;
        }
        (self.wake)();
        true
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn sending_queues_and_wakes_until_the_window_is_gone() {
        let woken = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        let counter = woken.clone();
        let proxy = Proxy::new(tx, Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        let other = proxy.clone();
        assert!(std::thread::spawn(move || other.send(7)).join().unwrap());
        assert!(proxy.send(8));
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [7, 8]);
        assert_eq!(woken.load(Ordering::SeqCst), 2);
        drop(rx);
        assert!(!proxy.send(9), "the window is gone");
        assert_eq!(woken.load(Ordering::SeqCst), 2, "nothing to wake for");
    }
}
