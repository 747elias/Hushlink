//! Win32 tray and HKCU Run integration. No administrator privileges, package
//! identity, network connection, or unlocked profile are needed here.
use super::{DesktopEvent, startup_command_units};
use anyhow::{Context, Result, anyhow, bail, ensure};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::{
    cell::Cell,
    io,
    os::windows::ffi::OsStrExt,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
    },
    System::{LibraryLoader::GetModuleHandleW, Registry::*, Threading::GetCurrentProcessId},
    UI::{
        Controls::{LIM_SMALL, LoadIconMetric},
        Shell::*,
        WindowsAndMessaging::*,
    },
};

const CALLBACK: u32 = WM_APP + 67;
const TIMER: usize = 1;
const SHOW: usize = 101;
const LOCK: usize = 102;
const UPDATES: usize = 103;
const EXIT: usize = 104;
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Hushlink";
type Wake = Arc<dyn Fn() + Send + Sync>;

struct Shared {
    events: Sender<DesktopEvent>,
    waker: Mutex<Option<Wake>>,
    shutdown: AtomicBool,
    available: AtomicBool,
    window: AtomicUsize,
}

impl Shared {
    fn emit(&self, event: DesktopEvent) {
        if self.shutdown.load(Ordering::Acquire) {
            return;
        }
        if matches!(
            event,
            DesktopEvent::Show | DesktopEvent::CheckUpdates | DesktopEvent::Exit
        ) {
            // A repaint request alone is insufficient for a hidden native egui
            // window. Restore it before waking the UI, including for Exit.
            restore_main_window();
        }
        let _ = self.events.send(event);
        self.wake();
    }

    fn wake(&self) {
        let wake = self.waker.lock().ok().and_then(|value| value.clone());
        if let Some(wake) = wake {
            // Never unwind across a Win32 window-procedure ABI boundary, even
            // if an application-provided repaint callback unexpectedly panics.
            let _ = catch_unwind(AssertUnwindSafe(|| wake()));
        }
    }
}

pub struct DesktopIntegration {
    shared: Arc<Shared>,
    events: Receiver<DesktopEvent>,
}

impl DesktopIntegration {
    /// Waits at most five seconds for the native icon to be installed. Call
    /// from a background initializer; the UI must stay visible on any error.
    pub fn new() -> Result<Self> {
        let (sender, events) = unbounded();
        let shared = Arc::new(Shared {
            events: sender,
            waker: Mutex::new(None),
            shutdown: AtomicBool::new(false),
            available: AtomicBool::new(false),
            window: AtomicUsize::new(0),
        });
        let (ready, initialization) = mpsc::sync_channel(1);
        let worker_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("hushlink-tray".into())
            .spawn(move || run(worker_shared, ready, true))
            .context("Der Windows-Infobereich konnte nicht gestartet werden.")?;
        match initialization.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self { shared, events }),
            result => {
                shared.shutdown.store(true, Ordering::Release);
                match result {
                    Ok(Err(error)) => bail!(error),
                    Err(_) => bail!(
                        "Das Infobereich-Symbol konnte nicht rechtzeitig gestartet werden. Hushlink bleibt als Fenster geöffnet."
                    ),
                    Ok(Ok(())) => unreachable!(),
                }
            }
        }
    }

    pub fn try_event(&self) -> Option<DesktopEvent> {
        self.events.try_recv().ok()
    }

    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut value) = self.shared.waker.lock() {
            *value = Some(Arc::new(wake));
        }
        if !self.events.is_empty() {
            self.shared.wake();
        }
    }

    /// Check immediately before hiding a window. Explorer restart failures
    /// restore the main window and clear availability until the icon returns.
    pub fn is_available(&self) -> bool {
        self.shared.available.load(Ordering::Acquire)
            && !self.shared.shutdown.load(Ordering::Acquire)
    }
}

impl Drop for DesktopIntegration {
    fn drop(&mut self) {
        // The native timer notices this within 250 ms, even inside a menu loop.
        // Avoid joining a native thread, or posting to a potentially recycled
        // HWND, from the UI thread during shutdown.
        self.shared.available.store(false, Ordering::Release);
        self.shared.shutdown.store(true, Ordering::Release);
    }
}

struct WindowState {
    shared: Arc<Shared>,
    taskbar_created: u32,
    icon: HICON,
    icon_added: Cell<bool>,
    menu_open: Cell<bool>,
    use_shell: bool,
}

struct NativeWindow {
    window: HWND,
    instance: HINSTANCE,
    class_name: Vec<u16>,
    state: Box<WindowState>,
}

impl NativeWindow {
    fn create(shared: Arc<Shared>, use_shell: bool) -> Result<Self> {
        static CLASS_ID: AtomicU64 = AtomicU64::new(0);
        let class_name = wide(&format!(
            "Hushlink.Tray.{}.{}",
            std::process::id(),
            CLASS_ID.fetch_add(1, Ordering::Relaxed)
        ));
        // SAFETY: null selects this process's module, valid until process exit.
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        ensure!(
            !instance.is_null(),
            "Das Windows-App-Modul konnte nicht geladen werden."
        );
        // SAFETY: resource ID 1 is the original multiresolution Hushlink ICO,
        // embedded by build.rs. LoadIconMetric returns an owned, DPI-sized icon.
        let mut icon = ptr::null_mut();
        let icon_status =
            unsafe { LoadIconMetric(instance, ptr::without_provenance(1), LIM_SMALL, &mut icon) };
        ensure!(
            icon_status >= 0 && !icon.is_null(),
            "Das Hushlink-Infobereich-Symbol konnte nicht geladen werden."
        );
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            ..Default::default()
        };
        // SAFETY: class strings remain owned until matching UnregisterClassW.
        if unsafe { RegisterClassW(&class) } == 0 {
            let error = io::Error::last_os_error();
            // SAFETY: no window owns this icon yet.
            unsafe {
                DestroyIcon(icon);
            }
            return Err(error).context("Das Infobereich-Fenster konnte nicht registriert werden.");
        }
        let taskbar_name = wide("TaskbarCreated");
        // SAFETY: NUL-terminated input remains alive for the call.
        let taskbar_created = unsafe { RegisterWindowMessageW(taskbar_name.as_ptr()) };
        // SAFETY: this hidden top-level tool window receives Explorer's
        // TaskbarCreated broadcast. HWND_MESSAGE would silently miss it.
        let window = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                class_name.as_ptr(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null(),
            )
        };
        if window.is_null() || taskbar_created == 0 {
            let error = io::Error::last_os_error();
            // SAFETY: matching cleanup for resources created on this thread.
            unsafe {
                if !window.is_null() {
                    DestroyWindow(window);
                }
                UnregisterClassW(class_name.as_ptr(), instance);
                DestroyIcon(icon);
            }
            return Err(error).context("Das Infobereich-Fenster konnte nicht erstellt werden.");
        }
        let native = Self {
            window,
            instance,
            class_name,
            state: Box::new(WindowState {
                shared,
                taskbar_created,
                icon,
                icon_added: Cell::new(false),
                menu_open: Cell::new(false),
                use_shell,
            }),
        };
        // SAFETY: the Box is stable until Drop clears userdata before freeing
        // it. All callbacks run on this thread, using shared references + Cell.
        unsafe {
            SetWindowLongPtrW(
                window,
                GWLP_USERDATA,
                (&*native.state as *const WindowState) as isize,
            );
        }
        // SAFETY: callbacks use this window's own native thread, not eframe.
        ensure!(
            unsafe { SetTimer(window, TIMER, 250, None) } != 0,
            "Der Infobereich-Zeitgeber konnte nicht gestartet werden."
        );
        native
            .state
            .shared
            .window
            .store(window as usize, Ordering::Release);
        Ok(native)
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        self.state.shared.available.store(false, Ordering::Release);
        self.state.shared.window.store(0, Ordering::Release);
        // SAFETY: only the owning native thread destroys these resources, with
        // the userdata allocation alive throughout all synchronous callbacks.
        unsafe {
            if self.state.icon_added.get() {
                Shell_NotifyIconW(NIM_DELETE, &icon_data(self.window, &self.state));
            }
            KillTimer(self.window, TIMER);
            SetWindowLongPtrW(self.window, GWLP_USERDATA, 0);
            DestroyWindow(self.window);
            UnregisterClassW(self.class_name.as_ptr(), self.instance);
            DestroyIcon(self.state.icon);
        }
    }
}

fn run(
    shared: Arc<Shared>,
    ready: mpsc::SyncSender<std::result::Result<(), String>>,
    use_shell: bool,
) {
    let outcome = (|| -> Result<()> {
        let native = NativeWindow::create(Arc::clone(&shared), use_shell)?;
        ensure!(
            !shared.shutdown.load(Ordering::Acquire),
            "Infobereich-Start abgebrochen."
        );
        if use_shell {
            ensure!(
                add_icon(native.window, &native.state),
                "Windows hat das Infobereich-Symbol nicht angenommen. Hushlink bleibt als Fenster geöffnet."
            );
        }
        let _ = ready.send(Ok(()));
        let mut message = MSG::default();
        loop {
            // SAFETY: initialized output storage and an ABI-correct callback.
            let status = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
            if status <= 0 {
                if status < 0 {
                    return Err(io::Error::last_os_error())
                        .context("Die Windows-Infobereich-Warteschlange wurde beendet.");
                }
                break;
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        Ok(())
    })();
    if let Err(error) = outcome {
        let _ = ready.send(Err(error.to_string()));
    }
    if !shared.shutdown.load(Ordering::Acquire) {
        // Losing the tray worker must never strand an inaccessible hidden app.
        shared.emit(DesktopEvent::Show);
    }
}

fn icon_data(window: HWND, state: &WindowState) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        // HWND + ID intentionally permits both portable and installed copies.
        // For unsigned binaries, a GUID would bind the icon to its first path
        // and can fail after moving the EXE or installing a downloaded copy.
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: CALLBACK,
        hIcon: state.icon,
        ..Default::default()
    };
    for (destination, source) in data
        .szTip
        .iter_mut()
        .zip("Hushlink – verschlüsselter Bluetooth-Chat".encode_utf16())
    {
        *destination = source;
    }
    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    data
}

fn add_icon(window: HWND, state: &WindowState) -> bool {
    let data = icon_data(window, state);
    // SAFETY: fully initialized structure and a live owned HWND/icon.
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) } != 0;
    if added {
        // A version failure still leaves a functional legacy callback icon.
        unsafe {
            Shell_NotifyIconW(NIM_SETVERSION, &data);
        }
    }
    state.icon_added.set(added);
    state.shared.available.store(added, Ordering::Release);
    added
}

fn handle_command(state: &WindowState, command: usize) {
    let event = match command {
        SHOW => DesktopEvent::Show,
        LOCK => DesktopEvent::Lock,
        UPDATES => DesktopEvent::CheckUpdates,
        EXIT => DesktopEvent::Exit,
        _ => return,
    };
    state.shared.emit(event);
}

fn show_menu(window: HWND, state: &WindowState) {
    // SAFETY: this thread owns the menu and destroys it after TrackPopupMenu's
    // nested message loop returns. Shared state permits callback reentrancy.
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let entries = [
            (SHOW, "Öffnen"),
            (LOCK, "Sperren"),
            (UPDATES, "Updates prüfen"),
        ];
        let mut valid = true;
        for (command, label) in entries {
            let text = wide(label);
            valid &= AppendMenuW(menu, MF_STRING, command, text.as_ptr()) != 0;
        }
        valid &= AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null()) != 0;
        let exit = wide("Beenden");
        valid &= AppendMenuW(menu, MF_STRING, EXIT, exit.as_ptr()) != 0;
        let mut cursor = POINT::default();
        if valid && GetCursorPos(&mut cursor) != 0 {
            SetForegroundWindow(window);
            state.menu_open.set(true);
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                cursor.x,
                cursor.y,
                0,
                window,
                ptr::null(),
            );
            state.menu_open.set(false);
            handle_command(state, command as usize);
            // Standard Win32 notification-menu pattern: allow a subsequent
            // click outside the menu to dismiss it correctly.
            PostMessageW(window, WM_NULL, 0, 0);
        }
        DestroyMenu(menu);
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: userdata is a live, stable Box installed/cleared by NativeWindow.
    let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const WindowState;
    if !state.is_null() {
        // Shared references + Cell avoid aliased &mut during TrackPopupMenu's
        // nested dispatch, Shell calls, and synchronous window destruction.
        let state = unsafe { &*state };
        if message == state.taskbar_created {
            state.icon_added.set(false);
            state.shared.available.store(false, Ordering::Release);
            if state.use_shell && !add_icon(window, state) {
                state.shared.emit(DesktopEvent::Show);
            }
            return 0;
        }
        match message {
            CALLBACK => {
                let event = lparam as u32 & 0xffff;
                if matches!(event, WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_SELECT)
                    || event == (NIN_SELECT | NINF_KEY)
                {
                    state.shared.emit(DesktopEvent::Show);
                } else if matches!(event, WM_RBUTTONUP | WM_CONTEXTMENU) {
                    show_menu(window, state);
                }
                return 0;
            }
            WM_COMMAND => {
                handle_command(state, wparam & 0xffff);
                return 0;
            }
            WM_TIMER if wparam == TIMER => {
                if state.shared.shutdown.load(Ordering::Acquire) {
                    // SAFETY: end only this thread's native menu/message loops.
                    unsafe {
                        if state.menu_open.get() {
                            EndMenu();
                        }
                        PostQuitMessage(0);
                    }
                } else if state.use_shell && !state.icon_added.get() {
                    add_icon(window, state);
                }
                return 0;
            }
            WM_CLOSE => {
                // SAFETY: owner Drop handles actual HWND/resource destruction.
                unsafe {
                    PostQuitMessage(0);
                }
                return 0;
            }
            _ => {}
        }
    }
    // SAFETY: unhandled creation/destruction/default messages use Win32's proc.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn restore_main_window() {
    // SAFETY: callback checks both this process and the exact main-window title.
    // EnumWindows runs synchronously; it retains no callback or borrowed state.
    unsafe {
        EnumWindows(Some(restore_window), 0);
    }
}

unsafe extern "system" fn restore_window(window: HWND, _parameter: LPARAM) -> i32 {
    let mut process = 0;
    // SAFETY: live enumerated HWND and writable process-id output.
    unsafe {
        GetWindowThreadProcessId(window, &mut process);
    }
    // SAFETY: querying this process requires no handle ownership or privileges.
    if process != unsafe { GetCurrentProcessId() } {
        return 1;
    }
    let mut title = [0u16; 32];
    // SAFETY: valid output array with the supplied element count.
    let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32) };
    if length == 8 && title[..8].iter().copied().eq("Hushlink".encode_utf16()) {
        // SAFETY: ShowWindowAsync avoids blocking on eframe's UI thread. A tray
        // input action is an appropriate source of foreground activation.
        unsafe {
            ShowWindowAsync(
                window,
                if IsIconic(window) != 0 {
                    SW_RESTORE
                } else {
                    SW_SHOW
                },
            );
            SetForegroundWindow(window);
        }
        return 0;
    }
    1
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn startup_command() -> Result<Vec<u16>> {
    let executable =
        std::env::current_exe().context("Der Programmpfad konnte nicht ermittelt werden.")?;
    ensure!(
        executable.is_absolute(),
        "Der Programmpfad für den Autostart ist nicht absolut."
    );
    startup_command_units(&executable.as_os_str().encode_wide().collect::<Vec<_>>())
}

pub fn startup_enabled() -> Result<bool> {
    let key = wide(RUN_KEY);
    let value = wide(RUN_VALUE);
    // A Run command is documented as <=260 UTF-16 units; a larger foreign or
    // malformed value is never accepted as our enabled autostart registration.
    let mut stored = [0u16; 262];
    let mut bytes = size_of_val(&stored) as u32;
    // SAFETY: HKCU is predefined; output storage and all strings are bounded.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ | RRF_NOEXPAND,
            ptr::null_mut(),
            stored.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32))
            .context("Der Windows-Autostart konnte nicht gelesen werden.");
    }
    let expected = startup_command()?;
    let length = bytes as usize / size_of::<u16>();
    if !bytes.is_multiple_of(2) || length > stored.len() {
        return Ok(false);
    }
    Ok(stored[..length] == expected)
}

struct RegistryKey(HKEY);
impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: only successful open/create operations construct this owner.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

pub fn set_startup(enabled: bool) -> Result<()> {
    let key_name = wide(RUN_KEY);
    let value_name = wide(RUN_VALUE);
    let command = if enabled {
        Some(startup_command()?)
    } else {
        None
    };
    let mut key = ptr::null_mut();
    // SAFETY: per-user access, no privilege escalation; owned output handle is
    // closed on all paths. Disabling does not create a previously absent key.
    let status = unsafe {
        if enabled {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                key_name.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                ptr::null(),
                &mut key,
                ptr::null_mut(),
            )
        } else {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                key_name.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            )
        }
    };
    if !enabled && status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32))
            .context("Der Windows-Autostart konnte nicht geändert werden.");
    }
    let key = RegistryKey(key);
    let status = if let Some(command) = command {
        // SAFETY: UTF-16 REG_SZ, including its terminator, with exact byte size.
        unsafe {
            RegSetValueExW(
                key.0,
                value_name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * size_of::<u16>()) as u32,
            )
        }
    } else {
        // SAFETY: delete only Hushlink's own named value, never the Run key.
        unsafe { RegDeleteValueW(key.0, value_name.as_ptr()) }
    };
    if status != ERROR_SUCCESS && (status != ERROR_FILE_NOT_FOUND || enabled) {
        return Err(anyhow!(io::Error::from_raw_os_error(status as i32)))
            .context("Der Windows-Autostart konnte nicht gespeichert werden.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::atomic::AtomicUsize, time::Instant};

    #[test]
    fn native_queue_dispatches_without_eframe_and_cleans_up_on_owner_drop() -> Result<()> {
        let (events, receiver) = unbounded();
        let shared = Arc::new(Shared {
            events,
            waker: Mutex::new(None),
            shutdown: AtomicBool::new(false),
            available: AtomicBool::new(false),
            window: AtomicUsize::new(0),
        });
        let (ready, initialization) = mpsc::sync_channel(1);
        let worker_shared = Arc::clone(&shared);
        // Exercise the real Win32 queue/window/cleanup. Deliberately omit the
        // Explorer-owned Shell icon: Windows CI can have no interactive shell.
        // This is not a claim that a person clicked a real notification icon.
        let worker = thread::spawn(move || run(worker_shared, ready, false));
        initialization
            .recv_timeout(Duration::from_secs(5))?
            .map_err(|error| anyhow!(error))?;
        let wakes = Arc::new(AtomicUsize::new(0));
        let wake_counter = Arc::clone(&wakes);
        let integration = DesktopIntegration {
            shared: Arc::clone(&shared),
            events: receiver,
        };
        integration.set_waker(move || {
            wake_counter.fetch_add(1, Ordering::Release);
        });
        let window = shared.window.load(Ordering::Acquire) as HWND;
        ensure!(!window.is_null());
        // SAFETY: this test's worker owns the live HWND until shutdown below.
        ensure!(unsafe { PostMessageW(window, WM_COMMAND, LOCK, 0) } != 0);
        assert_eq!(
            integration.events.recv_timeout(Duration::from_secs(3))?,
            DesktopEvent::Lock
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while wakes.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(wakes.load(Ordering::Acquire) > 0);
        // Synthetic Shell callback through the native queue, including v4's
        // packed icon ID. The visible/hidden eframe event loop is not involved.
        ensure!(
            unsafe {
                PostMessageW(
                    window,
                    CALLBACK,
                    0,
                    ((1u32 << 16) | NIN_SELECT | NINF_KEY) as isize,
                )
            } != 0
        );
        assert_eq!(
            integration.events.recv_timeout(Duration::from_secs(3))?,
            DesktopEvent::Show
        );
        drop(integration);
        let deadline = Instant::now() + Duration::from_secs(3);
        while shared.window.load(Ordering::Acquire) != 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            shared.window.load(Ordering::Acquire),
            0,
            "native worker must stop without a UI frame"
        );
        worker
            .join()
            .map_err(|_| anyhow!("native tray worker panicked"))?;
        Ok(())
    }
}
