//! Content-free cross-thread notices, tied to the vault's lock generation.
//! The short state lock never contains file, UI, network or Shell operations.
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
struct State {
    generation: u64,
    enabled: bool,
    pending: u32,
    unread: u32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            generation: 1,
            enabled: false,
            pending: 0,
            unread: 0,
        }
    }
}
#[derive(Default)]
pub struct NotificationSignal {
    state: Mutex<State>,
    foreground: AtomicBool,
}
impl NotificationSignal {
    pub fn invalidate(&self, generation: u64) {
        if let Ok(mut s) = self.state.lock()
            && generation >= s.generation
        {
            *s = State {
                generation,
                ..Default::default()
            };
        }
    }
    pub fn configure(&self, generation: u64, enabled: bool) {
        if let Ok(mut s) = self.state.lock()
            && generation == s.generation
        {
            s.enabled = enabled;
            if !enabled {
                s.pending = 0;
                s.unread = 0;
            }
        }
    }
    pub fn enabled(&self) -> bool {
        self.state.lock().is_ok_and(|s| s.enabled)
    }
    pub fn set_foreground(&self, foreground: bool) {
        self.foreground.store(foreground, Ordering::Release);
    }
    pub fn received(&self, generation: u64) {
        if let Ok(mut s) = self.state.lock()
            && s.enabled
            && s.generation == generation
        {
            s.pending = s.pending.saturating_add(1).min(10_000);
        }
    }
    pub fn set_unread(&self, generation: u64, count: u32) {
        if let Ok(mut s) = self.state.lock()
            && s.enabled
            && s.generation == generation
        {
            s.unread = count.min(10_000);
        }
    }
    pub fn unread(&self) -> u32 {
        self.state
            .lock()
            .map_or(0, |s| if s.enabled { s.unread } else { 0 })
    }
    pub fn take(&self) -> u32 {
        self.state.lock().map_or(0, |mut s| {
            let count = std::mem::take(&mut s.pending);
            if s.enabled && !self.foreground.load(Ordering::Acquire) {
                count
            } else {
                0
            }
        })
    }
    #[cfg(test)]
    pub(crate) fn pending_for_test(&self) -> u32 {
        self.state.lock().map_or(0, |s| s.pending)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locking_and_foreground_suppress_content_free_notifications() {
        let s = NotificationSignal::default();
        s.received(1);
        assert_eq!(s.take(), 0);
        s.configure(1, true);
        s.received(1);
        s.received(1);
        assert_eq!(s.pending_for_test(), 2);
        assert_eq!(s.take(), 2);
        assert_eq!(s.take(), 0);
        s.set_foreground(true);
        s.received(1);
        assert_eq!(s.take(), 0);
        s.set_foreground(false);
        s.received(1);
        s.set_unread(1, 7);
        s.invalidate(2);
        assert_eq!(s.take(), 0);
        assert_eq!(s.unread(), 0);
        s.configure(1, true);
        s.received(1);
        assert!(!s.enabled());
        assert_eq!(s.take(), 0);
        s.configure(2, true);
        s.received(1);
        s.set_unread(1, 8);
        assert_eq!(s.take(), 0);
        assert_eq!(s.unread(), 0);
        s.received(2);
        assert_eq!(s.take(), 1);
    }
}
