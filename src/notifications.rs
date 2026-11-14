//! Content-free cross-thread notification signal. No names or plaintext leave the actor.
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[derive(Default)]
pub struct NotificationSignal {
    enabled: AtomicBool,
    foreground: AtomicBool,
    pending: AtomicU32,
    unread: AtomicU32,
}
impl NotificationSignal {
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
        if !enabled {
            self.pending.store(0, Ordering::Release);
            self.unread.store(0, Ordering::Release);
        }
    }
    pub fn set_foreground(&self, foreground: bool) {
        self.foreground.store(foreground, Ordering::Release);
    }
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }
    pub fn received(&self) {
        if self.enabled.load(Ordering::Acquire) {
            let _ = self
                .pending
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                    Some(n.saturating_add(1).min(10_000))
                });
        }
    }
    pub fn set_unread(&self, count: u32) {
        self.unread.store(count.min(10_000), Ordering::Release);
    }
    pub fn unread(&self) -> u32 {
        if self.enabled.load(Ordering::Acquire) {
            self.unread.load(Ordering::Acquire)
        } else {
            0
        }
    }
    pub fn take(&self) -> u32 {
        let count = self.pending.swap(0, Ordering::AcqRel);
        if self.enabled.load(Ordering::Acquire) && !self.foreground.load(Ordering::Acquire) {
            count
        } else {
            0
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locking_and_foreground_suppress_content_free_notifications() {
        let signal = NotificationSignal::default();
        signal.received();
        assert_eq!(signal.take(), 0);
        signal.set_enabled(true);
        signal.received();
        signal.received();
        assert_eq!(signal.take(), 2);
        assert_eq!(signal.take(), 0);
        signal.set_foreground(true);
        signal.received();
        assert_eq!(signal.take(), 0);
        signal.set_foreground(false);
        signal.received();
        signal.set_unread(7);
        signal.set_enabled(false);
        assert_eq!(signal.take(), 0);
        assert_eq!(signal.unread(), 0);
    }
}
