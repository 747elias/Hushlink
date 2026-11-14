use super::*;
use crate::{
    desktop::{self, DesktopEvent, DesktopIntegration},
    storage_location::{self, DesktopPreferences},
    updater::{self, VerifiedUpdate},
};
use crossbeam_channel::{Receiver, Sender, bounded};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

pub(super) enum Picker {
    Send,
    Export { id: String },
    OpenVault,
    NewVault,
    MoveVault,
}
impl Drop for Picker {
    fn drop(&mut self) {
        if let Picker::Export { id } = self {
            id.zeroize();
        }
    }
}
enum UiEvent {
    Tray(Result<DesktopIntegration, String>),
    Bootstrap(DesktopPreferences),
    Picker {
        generation: u64,
        picker: Picker,
        path: Option<PathBuf>,
    },
    Error(String),
    Installed,
    InstallFailed(String),
}
enum UpdateState {
    Checking,
    Current,
    Ready(Arc<VerifiedUpdate>),
    Error(String),
}
pub(super) struct Integration {
    tray: Option<DesktopIntegration>,
    events: Receiver<UiEvent>,
    sender: Sender<UiEvent>,
    update_requests: Sender<()>,
    update_state: Arc<Mutex<Option<UpdateState>>>,
    progress: Arc<Mutex<Option<(u64, u64)>>>,
    automatic: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    pub(super) prefs: DesktopPreferences,
    pub(super) transfers_open: bool,
    last_offer: Option<String>,
    dialog_busy: bool,
    pub(super) force_exit: bool,
    hidden: bool,
    initial_hide: bool,
    ready_update: Option<Arc<VerifiedUpdate>>,
    update_text: String,
    pending_prefs: Option<DesktopPreferences>,
    preferences_failed: bool,
    bootstrap_ready: bool,
    paused_for_session: bool,
}
impl Integration {
    pub(super) fn new(ctx: &egui::Context) -> Self {
        let (sender, events) = bounded(32);
        let (update_requests, requests) = bounded(2);
        let state = Arc::new(Mutex::new(None));
        let progress = Arc::new(Mutex::new(None));
        let automatic = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let tx = sender.clone();
        let wake = ctx.clone();
        thread::spawn(move || {
            let tray = DesktopIntegration::new().map_err(|e| e.to_string());
            if let Ok(tray) = &tray {
                let context = wake.clone();
                tray.set_waker(move || context.request_repaint());
            }
            let _ = tx.send(UiEvent::Tray(tray));
            wake.request_repaint();
        });
        let tx = sender.clone();
        let wake = ctx.clone();
        let auto = automatic.clone();
        let stopped = stop.clone();
        let cancelled = cancel.clone();
        let mailbox = state.clone();
        let transfer_progress = progress.clone();
        thread::spawn(move || {
            let mut prefs = DesktopPreferences::default();
            let config =
                Runtime::vault_path().and_then(|path| Ok(storage_location::load_config(&path)?));
            match config {
                Ok(config) => prefs = config.preferences,
                Err(error) => {
                    prefs.automatic_updates = false;
                    prefs.start_with_windows = false;
                    let _ = tx.try_send(UiEvent::Error(error.to_string()));
                }
            }
            if let Ok(enabled) = desktop::startup_enabled() {
                prefs.start_with_windows = enabled;
            }
            #[cfg(not(windows))]
            {
                prefs.automatic_updates = false;
                prefs.start_with_windows = false;
            }
            auto.store(prefs.automatic_updates, Ordering::Release);
            let _ = tx.try_send(UiEvent::Bootstrap(prefs));
            wake.request_repaint();
            let mut checked: Option<Instant> = None;
            let mut cached: Option<Arc<VerifiedUpdate>> = None;
            while !stopped.load(Ordering::Acquire) {
                let manual = requests.recv_timeout(Duration::from_secs(1)).is_ok();
                let due = auto.load(Ordering::Acquire)
                    && checked.is_none_or(|time| time.elapsed() >= Duration::from_secs(6 * 3600));
                if !manual && !due {
                    continue;
                }
                checked = Some(Instant::now());
                cancelled.store(false, Ordering::Release);
                set_update(&mailbox, UpdateState::Checking, &wake);
                match updater::check_for_update(env!("CARGO_PKG_VERSION")) {
                    Ok(Some(release)) => {
                        if !manual && !auto.load(Ordering::Acquire) {
                            continue;
                        }
                        if let Some(existing) = &cached
                            && existing.version() == release.version()
                        {
                            set_update(&mailbox, UpdateState::Ready(existing.clone()), &wake);
                            continue;
                        }
                        let result =
                            updater::download_update(&release, &cancelled, |done, total| {
                                if let Ok(mut value) = transfer_progress.lock() {
                                    *value = Some((done, total));
                                }
                                wake.request_repaint();
                            });
                        if let Ok(mut value) = transfer_progress.lock() {
                            *value = None;
                        }
                        match result {
                            Ok(verified) => {
                                let verified = Arc::new(verified);
                                cached = Some(verified.clone());
                                set_update(&mailbox, UpdateState::Ready(verified), &wake);
                            }
                            Err(error) => {
                                set_update(&mailbox, UpdateState::Error(error.to_string()), &wake)
                            }
                        }
                    }
                    Ok(None) => set_update(&mailbox, UpdateState::Current, &wake),
                    Err(error) => {
                        set_update(&mailbox, UpdateState::Error(error.to_string()), &wake)
                    }
                }
            }
        });
        Self {
            tray: None,
            events,
            sender,
            update_requests,
            update_state: state,
            progress,
            automatic,
            cancel,
            stop,
            prefs: DesktopPreferences::default(),
            transfers_open: false,
            last_offer: None,
            dialog_busy: false,
            force_exit: false,
            hidden: false,
            initial_hide: std::env::args().any(|arg| arg == "--tray"),
            ready_update: None,
            update_text: "Update-Prüfung wird vorbereitet …".into(),
            pending_prefs: None,
            preferences_failed: false,
            bootstrap_ready: false,
            paused_for_session: false,
        }
    }
    fn tray_ready(&self) -> bool {
        self.tray
            .as_ref()
            .is_some_and(DesktopIntegration::is_available)
    }
    pub(super) fn preferences_error(&mut self) {
        self.preferences_failed = true;
    }
    pub(super) fn preferences_snapshot(&mut self, view: &View) {
        let observed = DesktopPreferences {
            start_with_windows: view.start_with_windows,
            minimize_to_tray: view.minimize_to_tray,
            automatic_updates: view.automatic_updates,
        };
        if let Some(requested) = self.pending_prefs {
            if observed == requested {
                self.prefs = observed;
                self.paused_for_session = !observed.automatic_updates;
                self.automatic
                    .store(observed.automatic_updates, Ordering::Release);
                if observed.automatic_updates {
                    let _ = self.update_requests.try_send(());
                }
                self.pending_prefs = None;
                self.preferences_failed = false;
            } else if self.preferences_failed {
                self.prefs = observed;
                if !requested.automatic_updates {
                    self.automatic.store(false, Ordering::Release);
                    self.update_text = "Updates sind für diese Sitzung angehalten. Das dauerhafte Speichern der Einstellung ist fehlgeschlagen.".into();
                }
                self.pending_prefs = None;
                self.preferences_failed = false;
            }
        } else if self.bootstrap_ready {
            self.prefs = observed;
            self.automatic.store(
                observed.automatic_updates && !self.paused_for_session,
                Ordering::Release,
            );
        }
    }
    pub(super) fn clear_sensitive(&mut self) {
        self.transfers_open = false;
        self.last_offer.zeroize();
        self.last_offer = None;
        self.dialog_busy = false;
        // A lock can invalidate a queued settings command. Never leave the
        // controls waiting forever for an acknowledgement from that generation.
        self.pending_prefs = None;
        self.preferences_failed = false;
    }
}
impl Drop for Integration {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.cancel.store(true, Ordering::Release);
    }
}
fn set_update(mailbox: &Mutex<Option<UpdateState>>, state: UpdateState, ctx: &egui::Context) {
    if let Ok(mut value) = mailbox.lock() {
        *value = Some(state);
    }
    ctx.request_repaint();
}

impl App {
    pub(super) fn integration_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.integration.events.try_recv() {
            match event {
                UiEvent::Tray(result) => match result {
                    Ok(tray) => self.integration.tray = Some(tray),
                    Err(error) => {
                        self.integration.initial_hide = false;
                        #[cfg(windows)]
                        {
                            self.error = format!(
                                "Das Tray-Symbol konnte nicht gestartet werden. Das Fenster bleibt sichtbar. {error}"
                            );
                        }
                        #[cfg(not(windows))]
                        let _ = error;
                    }
                },
                UiEvent::Bootstrap(prefs) => {
                    self.integration.prefs = prefs;
                    self.integration.bootstrap_ready = true;
                }
                UiEvent::Picker {
                    generation,
                    mut picker,
                    path,
                } => {
                    self.integration.dialog_busy = false;
                    let current = self.runtime.as_ref().map_or(0, Runtime::generation);
                    if current != generation {
                        continue;
                    }
                    if let Some(path) = path {
                        match &mut picker {
                            Picker::Send => {
                                self.command(Command::SendFile { path });
                                self.integration.transfers_open = true;
                            }
                            Picker::Export { id, .. } => {
                                self.command(Command::ExportFile {
                                    attachment_id: std::mem::take(id),
                                    destination: path,
                                });
                            }
                            Picker::OpenVault | Picker::NewVault => {
                                self.command(Command::ChooseVault { path });
                            }
                            Picker::MoveVault => {
                                self.command(Command::RelocateVault { path });
                            }
                        }
                    }
                }
                UiEvent::Error(error) => {
                    self.error = error;
                    self.integration.dialog_busy = false;
                }
                UiEvent::Installed => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                UiEvent::InstallFailed(error) => {
                    self.integration.force_exit = false;
                    self.error = error;
                }
            }
        }
        while let Some(event) = self
            .integration
            .tray
            .as_ref()
            .and_then(DesktopIntegration::try_event)
        {
            match event {
                DesktopEvent::Show => {
                    self.integration.initial_hide = false;
                    self.integration.hidden = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                DesktopEvent::Lock => self.lock(ctx),
                DesktopEvent::Exit => {
                    self.integration.force_exit = true;
                    self.lock(ctx);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                DesktopEvent::CheckUpdates => {
                    self.integration.initial_hide = false;
                    let _ = self.integration.update_requests.try_send(());
                    self.settings_open = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
        // eframe unconditionally shows its native window after first paint.
        // A Visible(false) command in that same frame would be undone. Count
        // complete frames, not layout passes, and keep the failure fallback visible.
        if self.integration.initial_hide
            && self.integration.tray_ready()
            && ctx.cumulative_frame_nr() >= 2
        {
            self.lock(ctx);
            self.integration.hidden = true;
            self.integration.initial_hide = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        let update = self
            .integration
            .update_state
            .lock()
            .ok()
            .and_then(|mut value| value.take());
        if let Some(update) = update {
            match update {
                UpdateState::Checking => {
                    self.integration.update_text = "Suche nach Updates …".into()
                }
                UpdateState::Current => {
                    self.integration.update_text = "Hushlink ist aktuell.".into()
                }
                UpdateState::Ready(verified) => {
                    self.integration.update_text = format!(
                        "Version {} ist heruntergeladen und geprüft.",
                        verified.version()
                    );
                    self.integration.ready_update = Some(verified);
                }
                UpdateState::Error(error) => {
                    self.integration.update_text = format!("Update nicht möglich: {error}")
                }
            }
        }
        if let Some(offer) = self.view.transfers.iter().find(|transfer| {
            !transfer.outgoing
                && transfer.state == crate::runtime::FileTransferState::AwaitingConsent
        }) && self.integration.last_offer.as_deref() != Some(&offer.id)
        {
            self.integration.last_offer = Some(offer.id.clone());
            self.integration.transfers_open = true;
        }
    }
    pub(super) fn handle_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.integration.force_exit {
            if self.integration.prefs.minimize_to_tray && self.integration.tray_ready() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.lock(ctx);
                self.integration.hidden = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else {
                self.lock(ctx);
            }
        }
    }
    pub(super) fn pick(&mut self, picker: Picker, ctx: &egui::Context) {
        if self.integration.dialog_busy {
            return;
        }
        self.integration.dialog_busy = true;
        let generation = self.runtime.as_ref().map_or(0, Runtime::generation);
        let tx = self.integration.sender.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            #[cfg(windows)]
            let path = match &picker {
                Picker::Send => rfd::FileDialog::new()
                    .set_title("Datei senden (höchstens 20 MiB)")
                    .pick_file(),
                Picker::Export { .. } => rfd::FileDialog::new()
                    .set_title("Entschlüsselte Kopie speichern unter")
                    .set_file_name("Dateianhang")
                    .save_file(),
                Picker::OpenVault => rfd::FileDialog::new()
                    .set_title("Vorhandenen verschlüsselten Tresor öffnen")
                    .add_filter("Hushlink-Tresor", &["vault"])
                    .pick_file(),
                Picker::NewVault | Picker::MoveVault => rfd::FileDialog::new()
                    .set_title("Neue Tresordatei auswählen – Ziel darf nicht existieren")
                    .set_file_name("Hushlink.vault")
                    .add_filter("Hushlink-Tresor", &["vault"])
                    .save_file(),
            };
            #[cfg(not(windows))]
            let path: Option<PathBuf> = {
                let _ = tx.try_send(UiEvent::Error("Native Dateiauswahl ist in diesem Entwicklungsbuild nur unter Windows verfügbar.".into()));
                None
            };
            let _ = tx.send(UiEvent::Picker {
                generation,
                picker,
                path,
            });
            ctx.request_repaint();
        });
    }
    pub(super) fn desktop_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Windows und Updates");
        let mut prefs = self
            .integration
            .pending_prefs
            .unwrap_or(self.integration.prefs);
        let mut changed = false;
        ui.add_enabled_ui(
            cfg!(windows)
                && self.integration.bootstrap_ready
                && self.integration.pending_prefs.is_none(),
            |ui| {
                changed |= ui
                    .checkbox(
                        &mut prefs.start_with_windows,
                        "Mit Windows starten (gesperrt im Tray)",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut prefs.minimize_to_tray,
                        "Beim Schliessen sperren und im Tray weiterlaufen",
                    )
                    .changed();
            },
        );
        changed |= ui
            .add_enabled(
                self.integration.bootstrap_ready && self.integration.pending_prefs.is_none(),
                egui::Checkbox::new(
                    &mut prefs.automatic_updates,
                    "Updates automatisch prüfen und herunterladen",
                ),
            )
            .changed();
        ui.label(RichText::new("Nur Updates benötigen Internet. Nachrichten und Dateien werden ausschliesslich über Bluetooth übertragen.").small().color(MUTED));
        if changed && self.integration.pending_prefs.is_none() {
            if !prefs.automatic_updates {
                self.integration.paused_for_session = true;
                self.integration.automatic.store(false, Ordering::Release);
                self.integration.cancel.store(true, Ordering::Release);
            }
            if self.command(Command::SetDesktopSettings {
                start_with_windows: prefs.start_with_windows,
                minimize_to_tray: prefs.minimize_to_tray,
                automatic_updates: prefs.automatic_updates,
            }) {
                self.integration.pending_prefs = Some(prefs);
                self.integration.preferences_failed = false;
            }
        }
        if self.integration.pending_prefs.is_some() {
            ui.label("Einstellungen werden gespeichert …");
        }
        ui.label(
            RichText::new(&self.integration.update_text)
                .small()
                .color(MUTED),
        );
        if let Ok(value) = self.integration.progress.lock()
            && let Some((done, total)) = *value
        {
            ui.add(
                egui::ProgressBar::new(done as f32 / total.max(1) as f32).text(format!(
                    "Update: {} / {}",
                    bytes(done),
                    bytes(total)
                )),
            );
        }
        if ui.button("Jetzt nach Updates suchen").clicked() {
            let _ = self.integration.update_requests.try_send(());
        }
        if self.integration.ready_update.is_some()
            && ui.button("Update installieren und neu starten").clicked()
        {
            self.install_update(ui.ctx());
        }
        ui.separator();
        ui.heading("Verschlüsselter Tresor");
        ui.label("Profil, Schlüssel, Kontakte, Nachrichten und Dateianhänge liegen gemeinsam in einer verschlüsselten Datei.");
        ui.add(egui::Label::new(&self.view.vault_path).wrap());
        ui.add_enabled_ui(!self.integration.dialog_busy && !self.busy, |ui| {
            if self.view.unlocked {
                if ui.button("Tresor sicher an einen anderen Ort verschieben …").clicked() { self.pick(Picker::MoveVault, ui.ctx()); }
                ui.label(RichText::new("Verbindungen und Übertragungen vor dem Verschieben beenden. Bestehende Zieldateien werden nicht überschrieben.").small().color(MUTED));
            } else {
                if ui.button("Vorhandenen Tresor auswählen …").clicked() { self.pick(Picker::OpenVault, ui.ctx()); }
                if ui.button("Speicherort für neuen Tresor wählen …").clicked() { self.pick(Picker::NewVault, ui.ctx()); }
            }
        });
    }
    fn install_update(&mut self, ctx: &egui::Context) {
        let Some(update) = self.integration.ready_update.clone() else {
            return;
        };
        self.lock(ctx);
        self.integration.force_exit = true;
        self.integration.update_text = "Update wird gestartet …".into();
        let tx = self.integration.sender.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let event = match updater::launch_installer(&update) {
                Ok(()) => UiEvent::Installed,
                Err(error) => UiEvent::InstallFailed(error.to_string()),
            };
            let _ = tx.send(event);
            ctx.request_repaint();
        });
    }
    pub(super) fn file_window(&mut self, ctx: &egui::Context) {
        if !self.integration.transfers_open || !self.view.unlocked {
            return;
        }
        let mut open = true;
        egui::Window::new("Dateiübertragungen").open(&mut open).default_width(430.0).vscroll(true).max_height((ctx.content_rect().height()-60.0).max(220.0)).show(ctx, |ui| {
            ui.label("Dateien werden erst nach deiner Annahme empfangen und bleiben im verschlüsselten Tresor. Zum Öffnen bewusst eine entschlüsselte Kopie speichern.");
            let transfers = self.view.transfers.clone();
            for transfer in &transfers {
                ui.group(|ui| {
                    ui.strong(&transfer.name); ui.label(bytes(transfer.size));
                    use crate::runtime::FileTransferState::*;
                    match transfer.state {
                        AwaitingConsent if !transfer.outgoing => { ui.horizontal(|ui| {
                            if ui.button("Annehmen").clicked() { self.command(Command::RespondFile { id: transfer.id.clone(), accept: true }); }
                            if ui.button("Ablehnen").clicked() { self.command(Command::RespondFile { id: transfer.id.clone(), accept: false }); }
                        }); }
                        AwaitingConsent => { ui.label("Warte auf die Annahme der anderen Person …"); }
                        Transferring => { ui.label(if transfer.bytes_per_second==0{"Rate wird ermittelt …".into()}else{format!("{}/s · {}",bytes(transfer.bytes_per_second),transfer.eta_seconds.map_or("Restzeit unbekannt".into(),|seconds|format!("etwa {} Min. {} Sek.",seconds/60,seconds%60)))}); ui.add(egui::ProgressBar::new(transfer.transferred as f32/transfer.size.max(1) as f32).text(format!("{} / {}",bytes(transfer.transferred),bytes(transfer.size)))); }
                        Stored => { ui.colored_label(MINT, "Verschlüsselt gespeichert und bestätigt"); }
                        Cancelled => { ui.label("Abgebrochen"); }
                        Failed => { ui.colored_label(Color32::LIGHT_RED, "Übertragung fehlgeschlagen"); }
                    }
                    if matches!(transfer.state, AwaitingConsent | Transferring) && ui.small_button("Abbrechen").clicked() { self.command(Command::CancelFile { id: transfer.id.clone() }); }
                });
            }
            if transfers.is_empty() { ui.label("Noch keine Dateiübertragung in dieser Sitzung."); }
            ui.separator(); ui.heading("Gespeicherte Fortsetzungspunkte");
            ui.label("Nach neuer Verbindung bietet die sendende Person ihre fehlgeschlagene Datei im Verlauf erneut an. Die empfangende Person bestätigt noch einmal. Bereits gespeicherte Bytes werden geprüft und übersprungen. Explizites Abbrechen verwirft die empfangenen Bytes.");
            for partial in self.view.partials.clone() {
                ui.group(|ui| { ui.strong(&partial.name); ui.label(format!("{} von {} verschlüsselt gesichert", bytes(partial.saved), bytes(partial.size))); if ui.add_enabled(self.view.peer.is_none(), egui::Button::new("Fortsetzungspunkt löschen")).clicked() { self.command(Command::DiscardPartial { id: partial.id.clone() }); } });
            }
        });
        if !open {
            self.integration.transfers_open = false;
        }
    }
}
fn bytes(count: u64) -> String {
    if count >= 1024 * 1024 {
        format!("{:.1} MiB", count as f64 / (1024.0 * 1024.0))
    } else if count >= 1024 {
        format!("{:.1} KiB", count as f64 / 1024.0)
    } else {
        format!("{count} Byte")
    }
}
