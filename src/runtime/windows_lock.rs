//! A native WTS message window on its own thread. Lock notifications remain
//! queued even when eframe stops painting an occluded/minimized window.
use super::{CancellationHub, windows_session_locked};
use std::{
    cell::Cell,
    ptr,
    sync::{
        Weak,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::{
        LibraryLoader::GetModuleHandleW,
        RemoteDesktop::{
            NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
            WTSUnRegisterSessionNotification,
        },
    },
    UI::WindowsAndMessaging::*,
};

const TIMER: usize = 1;
struct Watcher {
    hub: Weak<CancellationHub>,
    // Cell permits same-thread Win32 callback reentrancy without aliased &mut.
    registered: Cell<bool>,
}

pub(super) fn start(hub: Weak<CancellationHub>) {
    thread::spawn(move || run(hub, |_| {}));
}

fn run(hub: Weak<CancellationHub>, window_ready: impl FnOnce(HWND)) {
    static CLASS_ID: AtomicU64 = AtomicU64::new(0);
    let class_name: Vec<u16> = format!(
        "Hushlink.SessionGuard.{}\0",
        CLASS_ID.fetch_add(1, Ordering::Relaxed)
    )
    .encode_utf16()
    .collect();
    // SAFETY: null requests the current process module, owned for its lifetime.
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class_name.as_ptr(),
        ..Default::default()
    };
    // SAFETY: class and UTF-16 storage live until after the window is destroyed.
    if unsafe { RegisterClassW(&class) } == 0 {
        failed(&hub);
        return;
    }
    // SAFETY: a message-only window never displays content or takes focus. Its
    // callback handles absent userdata during the synchronous creation messages.
    let window = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    };
    if window.is_null() {
        failed(&hub);
        // SAFETY: this thread registered the unused class.
        unsafe {
            UnregisterClassW(class_name.as_ptr(), instance);
        }
        return;
    }
    let watcher = Box::new(Watcher {
        hub,
        registered: Cell::new(false),
    });
    // SAFETY: the stable Box allocation outlives all callbacks. Win32 dispatches
    // this HWND on its owning thread; we clear userdata before dropping the Box.
    unsafe {
        SetWindowLongPtrW(
            window,
            GWLP_USERDATA,
            (&*watcher as *const Watcher) as isize,
        );
    }
    refresh(window, &watcher);
    // SAFETY: the timer posts WM_TIMER to the same window thread. It supplements
    // event notification and closes the worker after its weak owner disappears.
    let timer = unsafe { SetTimer(window, TIMER, 200, None) };
    if timer != 0 {
        window_ready(window);
        let mut message = MSG::default();
        // SAFETY: initialized output message; DispatchMessage calls our ABI-correct
        // callback. GetMessage -1 (error) and 0 (quit) both leave fail closed.
        while unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    failed(&watcher.hub);
    // SAFETY: matching native cleanup, before Box/userdata/class storage expire.
    unsafe {
        KillTimer(window, TIMER);
        if watcher.registered.get() {
            WTSUnRegisterSessionNotification(window);
        }
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        DestroyWindow(window);
        UnregisterClassW(class_name.as_ptr(), instance);
    }
}

fn failed(hub: &Weak<CancellationHub>) {
    if let Some(hub) = hub.upgrade() {
        hub.watcher_ready.store(false, Ordering::Release);
        hub.observe_windows_state(true);
    }
}

fn refresh(window: HWND, watcher: &Watcher) {
    let Some(hub) = watcher.hub.upgrade() else {
        // SAFETY: ends only this dedicated thread's message loop.
        unsafe {
            PostQuitMessage(0);
        }
        return;
    };
    if !watcher.registered.get() {
        // SAFETY: valid owned HWND; receive notifications for this session only.
        // Retry transient service-startup failures without enabling an unlocked vault.
        let registered =
            unsafe { WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) } != 0;
        watcher.registered.set(registered);
        hub.watcher_ready.store(registered, Ordering::Release);
    }
    hub.observe_windows_state(!watcher.registered.get() || windows_session_locked());
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: only run() installs this stable Box pointer; creation/destruction
    // messages with absent userdata are passed through to DefWindowProcW.
    let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const Watcher;
    if !state.is_null() {
        // SAFETY: userdata remains live until after native dispatch stops. Shared
        // references plus Cell allow Win32 same-thread callback reentrancy.
        let watcher = unsafe { &*state };
        if message == WM_WTSSESSION_CHANGE {
            if let Some(hub) = watcher.hub.upgrade() {
                if wparam == WTS_SESSION_LOCK as usize {
                    // Always advance the generation, even if a queued unlock has
                    // not started KDF work yet. A later UNLOCK never resumes it.
                    hub.record_windows_lock();
                } else if wparam == WTS_SESSION_UNLOCK as usize {
                    hub.os_locked.store(false, Ordering::Release);
                }
            }
            return 0;
        }
        if message == WM_TIMER && wparam == TIMER {
            refresh(window, watcher);
            return 0;
        }
    }
    // SAFETY: all unhandled messages use the standard window procedure.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, mpsc},
        time::{Duration, Instant},
    };

    #[test]
    fn native_message_window_locks_without_ui_frames_and_exits_on_owner_drop()
    -> Result<(), Box<dyn std::error::Error>> {
        let hub = Arc::new(CancellationHub::default());
        hub.generation.store(1, Ordering::Release);
        let weak = Arc::downgrade(&hub);
        let (ready, window) = mpsc::channel();
        let worker = thread::spawn(move || {
            run(weak, |window| {
                let _ = ready.send(window as usize);
            })
        });
        let window = window.recv_timeout(Duration::from_secs(5))? as HWND;
        let registration_deadline = Instant::now() + Duration::from_secs(5);
        while !hub.watcher_ready.load(Ordering::Acquire) && Instant::now() < registration_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            hub.watcher_ready.load(Ordering::Acquire),
            "native WTS registration must succeed"
        );
        // Synthetic WTS message through the real native queue. This verifies
        // callback/lifecycle behavior, not actual Windows Win+L interaction.
        let generation = hub.generation.load(Ordering::Acquire);
        // SAFETY: the worker owns this live HWND until the final Arc is dropped.
        if unsafe { PostMessageW(window, WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK as usize, 0) } == 0
        {
            return Err("native test notification could not be posted".into());
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while hub.generation.load(Ordering::Acquire) == generation && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            hub.generation.load(Ordering::Acquire) > generation,
            "lock notification must be handled without an eframe update"
        );
        drop(hub);
        worker
            .join()
            .map_err(|_| "session watcher thread panicked")?;
        Ok(())
    }
}
