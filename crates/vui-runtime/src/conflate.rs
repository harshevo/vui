//! Latest-value hand-off from a worker thread to the UI. A feed that produces faster than the UI
//! draws puts each new value in one slot; the UI is woken once per batch and takes only the newest.
//! Older values are dropped, not queued.
//!
//! ```ignore
//! let quotes = Conflated::new();
//! let sender = cx.sender();
//! let put = quotes.putter(move || sender.send(Msg::Quotes));
//! std::thread::spawn(move || loop { put.put(next_snapshot()); });
//! // in update: Msg::Quotes => if let Some(q) = self.quotes.take() { … }
//! ```

use std::sync::{Arc, Mutex};

/// The UI's end: takes the newest value.
pub struct Conflated<T> {
    slot: Arc<Mutex<Option<T>>>,
}

impl<T> Default for Conflated<T> {
    fn default() -> Self {
        Conflated { slot: Arc::new(Mutex::new(None)) }
    }
}

impl<T> Conflated<T> {
    pub fn new() -> Conflated<T> {
        Conflated::default()
    }

    /// The newest value put since the last take, if any.
    pub fn take(&self) -> Option<T> {
        self.slot.lock().ok()?.take()
    }

    /// The producer's end. `wake` runs when a value lands in an empty slot (an `AppSender::send`
    /// of the app's message, say); it returns false once nobody listens.
    pub fn putter(&self, wake: impl Fn() -> bool + Send + Sync + 'static) -> Putter<T> {
        Putter { slot: self.slot.clone(), wake: Arc::new(wake) }
    }
}

/// The producer's end; clone it for more threads.
pub struct Putter<T> {
    slot: Arc<Mutex<Option<T>>>,
    wake: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl<T> Clone for Putter<T> {
    fn clone(&self) -> Self {
        Putter { slot: self.slot.clone(), wake: self.wake.clone() }
    }
}

impl<T> Putter<T> {
    /// Replaces the waiting value. Wakes the UI only if none was waiting. False once the UI is gone.
    pub fn put(&self, value: T) -> bool {
        let was_empty = match self.slot.lock() {
            Ok(mut slot) => slot.replace(value).is_none(),
            Err(_) => return false,
        };
        !was_empty || (self.wake)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn a_burst_wakes_once_and_only_the_newest_is_taken() {
        let c = Conflated::new();
        let wakes = Arc::new(AtomicUsize::new(0));
        let w = wakes.clone();
        let put = c.putter(move || {
            w.fetch_add(1, Ordering::SeqCst);
            true
        });
        let worker = put.clone();
        std::thread::spawn(move || (1..=100).for_each(|i| assert!(worker.put(i)))).join().unwrap();
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        assert_eq!(c.take(), Some(100));
        assert_eq!(c.take(), None);
        put.put(101);
        assert_eq!(wakes.load(Ordering::SeqCst), 2, "an empty slot wakes again");
    }
}
