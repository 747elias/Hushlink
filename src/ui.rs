use crate::runtime::{Command, Event, Runtime, View};
use crate::vault::MessageStatus;
use eframe::egui::{self, Color32, RichText, Vec2};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

mod auth;
mod features;
mod integration;
mod media;
mod messenger;
mod onboarding;
mod rich;
mod settings;
mod theme;
mod timestamp;
use integration::{Integration, Picker};

const MINT: Color32 = Color32::from_rgb(48, 210, 158);
const BACKGROUND: Color32 = Color32::from_rgb(11, 21, 27);
const PANEL: Color32 = Color32::from_rgb(17, 29, 36);
const MUTED: Color32 = Color32::from_rgb(143, 161, 170);
const MAX_TEXT: usize = 16 * 1024;

pub fn run() -> eframe::Result {
    let Some(_instance) = crate::desktop::acquire_instance()
        .map_err(|error| eframe::Error::AppCreation(error.into()))?
    else {
        return Ok(());
    };
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/hushlink.png"))
        .map_err(|error| eframe::Error::AppCreation(Box::new(error)))?;
    let options = eframe::NativeOptions {
        centered: true,
        #[cfg(windows)]
        renderer: eframe::Renderer::Wgpu,
        #[cfg(windows)]
        wgpu_options: windows_graphics(),
        viewport: egui::ViewportBuilder::default()
            .with_title("Hushlink")
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([720.0, 540.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Hushlink",
        options,
        Box::new(|cc| {
            configure(&cc.egui_ctx);
            Ok(Box::new(App::new(&cc.egui_ctx)))
        }),
    )
}

#[cfg(windows)]
fn windows_graphics() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::{egui_wgpu, wgpu};
    let mut setup = egui_wgpu::WgpuSetupCreateNew::default();
    setup.instance_descriptor.backends = wgpu::Backends::DX12;
    // FXC uses Windows' built-in compiler; DynamicDxc would require distributing
    // an additional dxcompiler.dll. DX12 can also select the system WARP adapter.
    setup
        .instance_descriptor
        .backend_options
        .dx12
        .shader_compiler = wgpu::Dx12Compiler::Fxc;
    setup.power_preference = wgpu::PowerPreference::LowPower;
    egui_wgpu::WgpuConfiguration {
        wgpu_setup: setup.into(),
        ..Default::default()
    }
}

fn configure(ctx: &egui::Context) {
    theme::configure(ctx);
}

struct App {
    design: messenger::Presentation,
    features: features::Features,
    rich: rich::Rich,
    media: media::MediaUi,
    integration: Integration,
    runtime: Option<Runtime>,
    generation: u64,
    view: View,
    busy: bool,
    error: String,
    name: Zeroizing<String>,
    password: Zeroizing<String>,
    confirmation: Zeroizing<String>,
    new_password: Zeroizing<String>,
    new_confirmation: Zeroizing<String>,
    composer: Zeroizing<String>,
    selected: Option<String>,
    settings_open: bool,
    delete_requested: bool,
    fingerprint_checked: bool,
    last_activity: Instant,
    focus_auth: bool,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        let (runtime, error) = match Runtime::vault_path() {
            Ok(path) => (Some(Runtime::new(path)), String::new()),
            Err(_) => (None, "Der lokale App-Datenordner ist nicht verfügbar. Prüfe dein Windows-Benutzerprofil.".into()),
        };
        let generation = runtime.as_ref().map_or(0, Runtime::generation);
        Self {
            design: Default::default(),
            features: Default::default(),
            rich: Default::default(),
            media: Default::default(),
            integration: Integration::new(ctx),
            runtime,
            generation,
            view: View::default(),
            busy: false,
            error,
            name: Zeroizing::new(String::new()),
            password: Zeroizing::new(String::new()),
            confirmation: Zeroizing::new(String::new()),
            new_password: Zeroizing::new(String::new()),
            new_confirmation: Zeroizing::new(String::new()),
            composer: Zeroizing::new(String::new()),
            selected: None,
            settings_open: false,
            delete_requested: false,
            fingerprint_checked: false,
            last_activity: Instant::now(),
            focus_auth: true,
        }
    }

    fn command(&mut self, command: Command) -> bool {
        if let Some(runtime) = &self.runtime {
            match runtime.send(command) {
                Ok(()) => {
                    self.error.clear();
                    true
                }
                Err(e) => {
                    self.error = e;
                    false
                }
            }
        } else {
            false
        }
    }

    fn lock(&mut self, ctx: &egui::Context) {
        let was_unlocked = self.view.unlocked;
        if let Some(runtime) = &self.runtime {
            runtime.lock();
            self.generation = runtime.generation();
        }
        self.clear_locked_ui(ctx);
        if was_unlocked {
            ctx.request_discard("Tresor gesperrt");
        }
    }

    fn clear_locked_ui(&mut self, ctx: &egui::Context) {
        self.design.zeroize();
        self.features.clear();
        self.rich.zeroize();
        self.media.clear();
        self.rich.restore_source = None;
        self.rich.restore_destination = None;
        self.integration.clear_sensitive();
        let path = std::mem::take(&mut self.view.vault_path);
        self.view = View::default();
        self.view.vault_path = path;
        self.view.profile_exists = true;
        self.password.zeroize();
        self.confirmation.zeroize();
        self.new_password.zeroize();
        self.new_confirmation.zeroize();
        self.composer.zeroize();
        self.name.zeroize();
        self.error.zeroize();
        self.selected.zeroize();
        self.selected = None;
        self.busy = false;
        self.settings_open = false;
        self.delete_requested = false;
        self.fingerprint_checked = false;
        self.focus_auth = true;
        clear_ui_memory(ctx);
        ctx.request_repaint();
    }

    fn events(&mut self) {
        while let Some(event) = self.runtime.as_ref().and_then(Runtime::try_event) {
            match event {
                Event::State(view) => {
                    if view.unlocked && !self.view.unlocked {
                        self.password.zeroize();
                        self.confirmation.zeroize();
                        self.name.zeroize();
                        self.last_activity = Instant::now();
                    }
                    let changed_peer =
                        self.view.peer.as_ref().map(|p| &p.id) != view.peer.as_ref().map(|p| &p.id);
                    if changed_peer {
                        self.fingerprint_checked = false;
                    }
                    if let Some(peer) = &view.peer
                        && !peer.id.is_empty()
                        && (changed_peer || self.selected.is_none())
                    {
                        self.selected = Some(peer.id.clone());
                    }
                    if self.selected.as_ref().is_some_and(|id| {
                        !view.contacts.iter().any(|c| &c.id == id)
                            && view.peer.as_ref().is_none_or(|p| &p.id != id)
                    }) {
                        self.selected.zeroize();
                        self.selected = None;
                    }
                    self.integration.preferences_snapshot(&view);
                    self.view = view;
                }
                Event::Error(message) => {
                    self.integration.preferences_error();
                    self.error = message;
                    self.busy = false;
                }
                Event::Busy(busy) => self.busy = busy,
            }
        }
    }

    fn show_error(&self, ui: &mut egui::Ui) {
        if !self.error.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(Color32::from_rgb(255, 165, 155), &self.error);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let generation = self.runtime.as_ref().map_or(0, Runtime::generation);
        if generation != self.generation {
            self.generation = generation;
            self.clear_locked_ui(ctx);
        }
        self.events();
        self.integration_events(ctx);
        self.handle_close(ctx);
        self.feature_events(ctx);
        // Check expiry before accepting new input after a suspended/occluded frame.
        if self.view.unlocked
            && self.last_activity.elapsed()
                >= Duration::from_secs(u64::from(self.view.inactivity_minutes.max(1)) * 60)
        {
            self.lock(ctx);
        }
        if ctx.input(|i| !i.raw.events.is_empty()) {
            self.last_activity = Instant::now();
            if let Some(runtime) = &self.runtime {
                runtime.record_activity();
            }
        }
        if self.view.unlocked
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::L))
        {
            self.lock(ctx);
        }
        if self.view.unlocked {
            egui::SidePanel::left("navigation")
                .exact_width(68.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(theme::RAIL)
                        .inner_margin(egui::Margin::symmetric(8, 0)),
                )
                .show(ctx, |ui| self.navigation(ui));
            if self.view.unlocked {
                let narrow = ctx.content_rect().width() < 900.0;
                let width = if narrow { 260.0 } else { 320.0 };
                egui::SidePanel::left("contacts_panel")
                    .default_width(width)
                    .min_width(224.0)
                    .max_width(if narrow { 260.0 } else { 380.0 })
                    .resizable(true)
                    .show_separator_line(false)
                    .frame(egui::Frame::new().fill(PANEL))
                    .show(ctx, |ui| self.conversation_list(ui));
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(BACKGROUND))
                    .show(ctx, |ui| {
                        if self.view.unlocked {
                            self.messenger(ui);
                        }
                    });
            }
        } else {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(BACKGROUND)
                        .inner_margin(egui::Margin::symmetric(40, 24)),
                )
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt((
                            "auth_scroll",
                            self.view.profile_exists,
                            self.design.onboarding_step,
                        ))
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.auth_screen(ui));
                });
        }
        self.poll_media(ctx);
        self.settings_window(ctx);
        self.file_window(ctx);
        self.feature_windows(ctx);
        self.backup_windows(ctx);
        if self.view.unlocked {
            self.edit_window(ctx);
        }
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

fn local_time(timestamp_ms: u64) -> String {
    // Relative age avoids an incorrect timezone conversion without extra locale dependencies.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let seconds = now.saturating_sub(timestamp_ms / 1000);
    match seconds {
        0..=59 => "Gerade eben".into(),
        60..=3599 => format!("vor {} Min.", seconds / 60),
        3600..=86399 => format!("vor {} Std.", seconds / 3600),
        _ => format!("vor {} Tagen", seconds / 86400),
    }
}

// TextEdit retains an undo buffer. Drop all widget memory while retaining the
// visual options; resetting those as well would shrink controls after unlock.
fn clear_ui_memory(ctx: &egui::Context) {
    fn wipe_events(events: &mut [egui::Event]) {
        for event in events {
            match event {
                egui::Event::Text(text)
                | egui::Event::Paste(text)
                | egui::Event::Ime(egui::ImeEvent::Preedit(text) | egui::ImeEvent::Commit(text)) => {
                    text.zeroize()
                }
                _ => {}
            }
        }
    }
    ctx.input_mut(|input| {
        wipe_events(&mut input.events);
        wipe_events(&mut input.raw.events);
        input.events.clear();
        input.raw.events.clear();
        for file in &mut input.raw.dropped_files {
            file.name.zeroize();
            file.mime.zeroize();
            if let Some(bytes) = &mut file.bytes
                && let Some(unique) = std::sync::Arc::get_mut(bytes)
            {
                unique.zeroize();
            }
        }
        input.raw.dropped_files.clear();
        input.raw.hovered_files.clear();
    });
    ctx.memory_mut(|memory| {
        let options = memory.options.clone();
        *memory = egui::Memory::default();
        memory.options = options;
    });
}

// egui::consume_key uses logical matching and ignores extra Shift/Alt. Sending
// must match exactly, otherwise Shift+Enter both inserts a newline and sends.
fn take_plain_enter(input: &mut egui::InputState) -> bool {
    let mut pressed = false;
    input.events.retain(|event| {
        if let egui::Event::Key {
            key: egui::Key::Enter,
            pressed: true,
            repeat,
            modifiers,
            ..
        } = event
            && modifiers.matches_exact(egui::Modifiers::NONE)
        {
            pressed |= !repeat;
            false
        } else {
            true
        }
    });
    pressed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enter(modifiers: egui::Modifiers, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat,
            modifiers,
        }
    }

    #[test]
    fn system_light_theme_does_not_override_dark_ui_after_password_memory_clear() {
        let ctx = egui::Context::default();
        configure(&ctx);
        for _ in 0..2 {
            let input = egui::RawInput {
                system_theme: Some(egui::Theme::Light),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| assert!(ctx.style().visuals.dark_mode));
            clear_ui_memory(&ctx);
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_renderer_has_a_compiled_direct3d_backend() {
        assert!(
            eframe::wgpu::Instance::enabled_backend_features()
                .contains(eframe::wgpu::Backends::DX12)
        );
    }

    #[test]
    fn shift_enter_and_modified_enter_never_send_and_plain_enter_sends_once() {
        for modifiers in [
            egui::Modifiers::SHIFT,
            egui::Modifiers::ALT,
            egui::Modifiers::CTRL,
            egui::Modifiers::COMMAND,
        ] {
            let mut input = egui::InputState::default();
            input.events.push(enter(modifiers, false));
            assert!(!take_plain_enter(&mut input));
            assert_eq!(
                input.events.len(),
                1,
                "newline event must remain available to TextEdit"
            );
        }
        let mut input = egui::InputState::default();
        input.events.push(enter(egui::Modifiers::NONE, false));
        assert!(take_plain_enter(&mut input));
        assert!(!take_plain_enter(&mut input));
        input.events.push(enter(egui::Modifiers::NONE, true));
        assert!(
            !take_plain_enter(&mut input),
            "holding Enter must not trigger another send"
        );
    }
}
