use crate::runtime::{Command, Event, Runtime, View};
use crate::vault::MessageStatus;
use eframe::egui::{self, Color32, RichText, Vec2};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

mod features;
mod integration;
mod media;
mod rich;
use integration::{Integration, Picker};

const MINT: Color32 = Color32::from_rgb(113, 222, 193);
const BACKGROUND: Color32 = Color32::from_rgb(15, 23, 29);
const PANEL: Color32 = Color32::from_rgb(21, 32, 39);
const MUTED: Color32 = Color32::from_rgb(154, 175, 184);
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
            .with_inner_size([960.0, 640.0])
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
    ctx.set_theme(egui::Theme::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(12, 19, 24);
    visuals.selection.bg_fill = Color32::from_rgb(29, 91, 80);
    visuals.selection.stroke.color = Color32::WHITE;
    visuals.widgets.active.bg_fill = Color32::from_rgb(35, 99, 85);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(39, 61, 68);
    visuals.hyperlink_color = MINT;
    visuals.override_text_color = Some(Color32::from_rgb(230, 239, 242));
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(14.0, 9.0);
        style.spacing.interact_size = Vec2::new(40.0, 36.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
    });
    #[cfg(windows)]
    {
        let mut fonts = egui::FontDefinitions::default();
        for (name, file) in [("Segoe UI", "segoeui.ttf"), ("Segoe Emoji", "seguiemj.ttf")] {
            if let Ok(data) = std::fs::read(format!("C:\\Windows\\Fonts\\{file}")) {
                fonts
                    .font_data
                    .insert(name.into(), egui::FontData::from_owned(data).into());
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push(name.into());
            }
        }
        ctx.set_fonts(fonts);
    }
}

struct App {
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
        if let Some(runtime) = &self.runtime {
            runtime.lock();
            self.generation = runtime.generation();
        }
        self.clear_locked_ui(ctx);
    }

    fn clear_locked_ui(&mut self, ctx: &egui::Context) {
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

    fn auth(&mut self, ui: &mut egui::Ui) {
        ui.add_space((ui.ctx().content_rect().height() * 0.07).clamp(12.0, 54.0));
        let width = 420.0_f32.min(ui.available_width() - 32.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(width);
            ui.label(RichText::new("HUSH↔LINK").size(32.0).color(MINT).strong());
            ui.add_space(8.0);
            ui.label(RichText::new("Ein Gespräch. Direkt zwischen euch.").color(MUTED));
            ui.add_space(24.0);
            let existing = self.view.profile_exists;
            ui.heading(if existing { "Tresor entsperren" } else { "Dein privater Raum" });
            ui.add_space(8.0);
            ui.label(if existing {
                "Entsperre dein lokales Profil. Kontakte und Nachrichten bleiben auf diesem PC verschlüsselt gespeichert."
            } else {
                "Lege dein lokales Profil an. Hushlink verbindet zwei PCs über Bluetooth – ohne Konto und ohne Internet."
            });
            ui.add_space(14.0);
            ui.add_enabled_ui(!self.busy && self.runtime.is_some(), |ui| {
                if !existing {
                    ui.label("Dein Anzeigename");
                    ui.add(egui::TextEdit::singleline(&mut *self.name).id_salt("profile_name").hint_text("Wie möchtest du heissen?").desired_width(width).char_limit(64));
                }
                ui.label("Passwort");
                let response = ui.add(egui::TextEdit::singleline(&mut *self.password).id_salt("unlock_password").password(true).desired_width(width));
                if self.focus_auth { response.request_focus(); self.focus_auth = false; }
                if !existing {
                    ui.label("Passwort bestätigen");
                    ui.add(egui::TextEdit::singleline(&mut *self.confirmation).id_salt("confirm_password").password(true).desired_width(width));
                    ui.label(RichText::new("Mindestens 12 Zeichen. Verwende möglichst eine lange, einzigartige Passphrase.").small().color(MUTED));
                    ui.label(RichText::new("Ohne Wiederherstellung: Ein vergessenes Passwort bedeutet den Verlust deines Profils und Verlaufs.").color(Color32::from_rgb(237, 194, 131)));
                }
                let enter = ui.input_mut(take_plain_enter);
                if ui.add_sized([width, 42.0], egui::Button::new(if existing { "Entsperren" } else { "Profil erstellen" }).fill(Color32::from_rgb(26, 85, 73))).clicked() || enter {
                    if !existing && (self.name.trim().is_empty() || self.name.trim().len() > crate::protocol::MAX_NAME || self.name.chars().any(char::is_control) || self.password.chars().count() < 12 || *self.password != *self.confirmation) {
                        self.error = "Bitte gib einen Namen (höchstens 80 UTF-8-Bytes) und zweimal dasselbe Passwort mit mindestens 12 Zeichen ein.".into();
                    } else {
                        let command = if existing { Command::Unlock { password: self.password.clone() } } else { Command::Create { name: self.name.trim().into(), password: self.password.clone() } };
                        if self.command(command) { self.busy = true; self.password.zeroize(); self.confirmation.zeroize(); clear_ui_memory(ui.ctx()); }
                    }
                }
            });
            if self.busy { ui.spinner(); ui.label("Dein Tresor wird vorbereitet …"); }
            self.show_error(ui);
            ui.add_space(10.0);
            ui.label(RichText::new(&self.view.vault_path).small().color(MUTED));
            if ui.button("Speicherort und App-Einstellungen …").clicked() { self.settings_open = true; }
            if ui.button("Verschlüsselte Sicherung wiederherstellen …").clicked(){self.rich.restore_open=true;}
            ui.label(RichText::new("Alle privaten Daten liegen in einer verschlüsselten Tresordatei. Nur Update-Prüfungen benötigen Internet und sind abschaltbar.").small().color(MUTED));
            ui.add_space(22.0);
            ui.label(RichText::new("Lokal gespeichert · Ende-zu-Ende verschlüsselt · Keine Telemetrie").small().color(MUTED));
        });
    }

    fn show_error(&self, ui: &mut egui::Ui) {
        if !self.error.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(Color32::from_rgb(255, 165, 155), &self.error);
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(RichText::new("HUSH↔LINK").size(22.0).strong().color(MINT));
        ui.label(RichText::new(&self.view.profile_name).color(MUTED));
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Sperren").clicked() {
                self.lock(ui.ctx());
            }
            if ui.button("Einstellungen").clicked() {
                self.settings_open = true;
            }
        });
        if !self.view.unlocked {
            return;
        }
        ui.separator();
        ui.label(RichText::new("VERBINDUNG").small().color(MUTED));
        if ui.button("Verbindungsassistent …").clicked() {
            self.features.wizard = true;
        }
        ui.label(&self.view.availability);
        ui.label(RichText::new(&self.view.status).small().color(MUTED));
        ui.add_enabled_ui(!self.busy, |ui| {
            if self.view.listening {
                if ui.button("Empfang beenden").clicked() {
                    self.command(Command::StopListening);
                }
                ui.label(
                    RichText::new(
                        "Empfang ist aktiv. Beende ihn, wenn du keine Verbindung erwartest.",
                    )
                    .small()
                    .color(MINT),
                );
            } else if ui.button("Verbindung empfangen").clicked() {
                self.command(Command::StartListening);
            }
            if ui
                .add_enabled(
                    !self.view.discovering,
                    egui::Button::new(if self.view.discovering {
                        "Suche läuft …"
                    } else {
                        "Geräte suchen"
                    }),
                )
                .clicked()
            {
                self.command(Command::Discover);
            }
        });
        if self.view.discovering {
            ui.spinner();
        }
        if !self.view.devices.is_empty() {
            ui.add_space(6.0);
            let devices = self.view.devices.clone();
            for device in &devices {
                ui.group(|ui| {
                    ui.label(&device.name);
                    ui.label(RichText::new(&device.address).small().color(MUTED));
                    if ui
                        .add_enabled(
                            !self.busy && self.view.peer.is_none(),
                            egui::Button::new("Verbinden"),
                        )
                        .clicked()
                    {
                        self.command(Command::Connect {
                            address: device.address.clone(),
                        });
                    }
                });
            }
        }
        ui.add_space(8.0);
        if ui.small_button("Windows-Bluetooth-Einstellungen").clicked()
            && crate::transport::open_bluetooth_settings().is_err()
        {
            self.error = "Öffne Windows → Einstellungen → Bluetooth und Geräte.".into();
        }
        ui.label(RichText::new("Zum ersten Finden: Bluetooth in Windows einschalten und gegebenenfalls beide PCs dort koppeln. Auf dem anderen PC Hushlink-Empfang aktivieren.").small().color(MUTED));
        ui.separator();
        ui.label(RichText::new("KONTAKTE").small().color(MUTED));
        if self.view.contacts.is_empty() {
            ui.label(
                RichText::new("Noch keine bestätigten Kontakte. Beginne mit einer Verbindung.")
                    .color(MUTED),
            );
        }
        let contacts = self.view.contacts.clone();
        egui::ScrollArea::vertical()
            .id_salt("contacts")
            .show(ui, |ui| {
                for contact in &contacts {
                    if ui
                        .selectable_label(
                            self.selected.as_deref() == Some(&contact.id),
                            format!(
                                "{}{}{}",
                                if contact.favorite { "★ " } else { "" },
                                contact.name,
                                if contact.unread > 0 {
                                    format!(" ({})", contact.unread)
                                } else {
                                    String::new()
                                }
                            ),
                        )
                        .clicked()
                    {
                        self.selected = Some(contact.id.clone());
                        self.delete_requested = false;
                    }
                    if self.selected.as_deref() == Some(&contact.id)
                        && ui.small_button("Kontakt verwalten …").clicked()
                    {
                        self.open_contact(&contact.id);
                    }
                    if self.selected.as_deref() == Some(&contact.id)
                        && self.view.peer.is_none()
                        && ui.small_button("Erneut verbinden").clicked()
                    {
                        self.command(Command::Connect {
                            address: contact.address.clone(),
                        });
                    }
                }
            });
    }

    fn chat(&mut self, ui: &mut egui::Ui) {
        self.rich.sync(self.selected.as_deref());
        self.sync_media();
        ui.add_space(8.0);
        if let Some(pending) = &self.view.pending_incoming {
            let name = pending.name.clone();
            let address = pending.address.clone();
            ui.group(|ui| {
                ui.heading("Verbindungsanfrage");
                ui.label(RichText::new(&address).small().color(MUTED));
                ui.label(format!(
                    "{name} möchte sich verbinden. Ein Gerätename ist noch kein Identitätsnachweis."
                ));
                ui.horizontal(|ui| {
                    if ui.button("Annehmen").clicked() {
                        self.command(Command::Respond { accept: true });
                    }
                    if ui.button("Ablehnen").clicked() {
                        self.command(Command::Respond { accept: false });
                    }
                });
            });
        }
        if let Some(device) = &self.view.key_change {
            let name = device.name.clone();
            ui.group(|ui| {
                ui.colored_label(Color32::from_rgb(255, 180, 140), "Identität hat sich geändert – Verbindung blockiert");
                ui.label(format!("Für {name} wurde ein anderer Schlüssel erkannt. Kläre unabhängig, ob das Profil absichtlich neu erstellt wurde. Bei Zweifel brich ab."));
                if ui.button("Neue Identität bewusst erneut prüfen").clicked() { self.command(Command::AllowKeyChange); }
                if ui.button("Abbrechen").clicked() { self.command(Command::Disconnect); }
            });
        }
        if let Some(peer) = self.view.peer.clone()
            && !peer.ready
        {
            ui.group(|ui| {
                    ui.heading("Vergleicht euren Sicherheitscode");
                    if ui.button("QR-Code und Kameravergleich …").clicked(){self.media.qr_open=true;}
                    ui.label(format!("Gegenstelle: {}", peer.name));
                    ui.label("Vergleicht alle Gruppen auf beiden PCs über einen unabhängigen Weg, zum Beispiel persönlich. Erst nach Bestätigung auf beiden Geräten wird der Chat freigegeben.");
                    ui.add_space(6.0);
                    ui.label(RichText::new(&peer.fingerprint).monospace().size(17.0).color(MINT));
                    if peer.known { ui.label(RichText::new("Die Identität stimmt mit deinem gespeicherten Kontakt überein.").color(MUTED)); }
                    if peer.local_confirmed {
                        ui.label("Von dir bestätigt. Warte auf die Bestätigung der anderen Person …");
                    } else {
                        ui.checkbox(&mut self.fingerprint_checked, "Alle Gruppen stimmen auf beiden Geräten überein");
                        if ui.add_enabled(self.fingerprint_checked, egui::Button::new("Kontakt bestätigen")).clicked() { self.command(Command::ConfirmPeer); self.fingerprint_checked = false; }
                    }
                    if ui.button("Verbindung trennen").clicked() { self.command(Command::Disconnect); }
                });
            self.show_error(ui);
            return;
        }
        let title = self
            .selected
            .as_ref()
            .and_then(|id| {
                self.view
                    .contacts
                    .iter()
                    .find(|c| &c.id == id)
                    .map(|c| c.name.clone())
            })
            .or_else(|| self.view.peer.as_ref().map(|p| p.name.clone()));
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(title.as_deref().unwrap_or("Deine Gespräche"));
                let connected = self
                    .view
                    .peer
                    .as_ref()
                    .is_some_and(|p| p.ready && self.selected.as_deref() == Some(&p.id));
                ui.label(
                    RichText::new(if connected {
                        "Verbunden · Sicherheitscode beidseitig bestätigt"
                    } else {
                        "Offline · Verlauf bleibt lokal verschlüsselt"
                    })
                    .small()
                    .color(if connected { MINT } else { MUTED }),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.view.peer.is_some() && ui.button("Trennen").clicked() {
                    self.command(Command::Disconnect);
                }
                if self.selected.is_some() && ui.button("Verlauf löschen").clicked() {
                    self.delete_requested = true;
                }
            });
        });
        ui.separator();
        self.show_error(ui);
        if self.delete_requested {
            ui.group(|ui| {
                ui.label("Diesen Verlauf auf diesem PC löschen? Sichere physische Löschung auf SSDs wird nicht zugesichert. Der Verlauf auf dem anderen PC bleibt bestehen.");
                ui.horizontal(|ui| {
                    if ui.button("Lokal löschen").clicked() {
                        if let Some(id) = &self.selected { self.command(Command::DeleteHistory { contact_id: id.clone() }); }
                        self.delete_requested = false;
                    }
                    if ui.button("Abbrechen").clicked() { self.delete_requested = false; }
                });
            });
        }
        let ready = self
            .view
            .peer
            .as_ref()
            .is_some_and(|p| p.ready && self.selected.as_deref() == Some(&p.id));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(ready, egui::Button::new("Datei senden …"))
                .clicked()
            {
                self.pick(Picker::Send, ui.ctx());
            }
            self.clipboard_button(ui, ready);
            if ui.button("Dateiübertragungen").clicked() {
                self.integration.transfers_open = true;
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.rich.query)
                    .hint_text("Verlauf durchsuchen …")
                    .char_limit(256),
            );
            ui.checkbox(&mut self.rich.all_contacts, "Alle Kontakte");
            ui.checkbox(&mut self.rich.pinned_only, "Nur gemerkte");
        });
        if self.view.peer_typing && ready {
            ui.label(RichText::new("Gegenstelle schreibt …").small().color(MINT));
        }
        let pending: Vec<String> = self
            .view
            .pending_actions
            .iter()
            .filter(|(_, c)| self.selected.as_deref() == Some(c))
            .map(|(id, _)| id.clone())
            .collect();
        if !pending.is_empty() {
            ui.collapsing(format!("{} unbestätigte Änderungen",pending.len()),|ui|{ui.label("Nach einem Abbruch neu verbinden und erneut übertragen. Wiederholungen werden ohne doppelte Nachrichten bestätigt.");for id in &pending{if ui.add_enabled(ready,egui::Button::new(format!("Änderung {} erneut übertragen",&id[..8]))).clicked(){self.command(Command::RetryAction{id:id.clone()});}}});
        }
        self.image_offer(ui);
        let composer_height = 165.0;
        let height = (ui.available_height() - composer_height).max(100.0);
        let mut has_messages = false;
        let mut export_file: Option<String> = None;
        let mut retry_file: Option<String> = None;
        let mut rich_commands = Vec::new();
        let mut selected_result = None;
        egui::ScrollArea::vertical().id_salt("messages").stick_to_bottom(true).auto_shrink([false, false]).max_height(height).show(ui, |ui| {
            for message in &self.view.messages {
                if (!self.rich.all_contacts && self.selected.as_deref()!=Some(&message.contact_id)) || !self.rich.matches(message){continue;}
                has_messages = true;
                let layout = if message.outgoing { egui::Layout::right_to_left(egui::Align::TOP) } else { egui::Layout::left_to_right(egui::Align::TOP) };
                ui.with_layout(layout, |ui| {
                    egui::Frame::new().fill(if message.outgoing { Color32::from_rgb(25, 66, 62) } else { PANEL }).corner_radius(12.0).inner_margin(12.0).show(ui, |ui| {
                        ui.set_max_width((ui.available_width() * 0.85).max(120.0));
                        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                            if self.rich.all_contacts && let Some(c)=self.view.contacts.iter().find(|c|c.id==message.contact_id) && ui.small_button(&c.name).clicked(){selected_result=Some(c.id.clone());}
                            if let Some(id)=&message.reply_to{let quoted=self.view.messages.iter().find(|m|m.id==*id && m.contact_id==message.contact_id);ui.group(|ui|{ui.label(RichText::new("Antwort auf").small().color(MINT));ui.add(egui::Label::new(quoted.map_or("Nicht gespeicherte Nachricht",|m|m.text.as_str())).wrap().truncate());});}
                            ui.add(egui::Label::new(message.text.as_str()).wrap());
                            let same_peer=ready && self.selected.as_deref()==Some(&message.contact_id);
                            if let Some(command)=self.rich.controls(ui,message,same_peer){rich_commands.push(command);}
                            if let Some(id) = &message.attachment_id {
                                ui.label(RichText::new(format!("Dateianhang · {:.1} KiB", message.attachment_size as f64 / 1024.0)).small().color(MUTED));
                                if message.status == MessageStatus::Received && ui.button("Speichern unter …").clicked() {
                                    export_file = Some(id.clone());
                                }
                            }
                            if message.can_resume && ui.add_enabled(ready, egui::Button::new("Erneut anbieten / fortsetzen")).clicked() { retry_file = Some(message.id.clone()); }
                            let state = match message.status {
                                MessageStatus::Pending => "Ausstehend",
                                MessageStatus::Sent => "Gesendet",
                                MessageStatus::Received => "Empfangen",
                                MessageStatus::Failed => "Fehlgeschlagen",
                            };
                            let time = local_time(message.timestamp_ms);
                            ui.label(RichText::new(if message.outgoing { format!("{time} · {state}") } else { time }).size(11.0).color(MUTED));
                        });
                    });
                });
            }
            if !has_messages {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Ein ruhiger Ort für euer Gespräch.").size(21.0).color(MINT));
                    ui.add_space(10.0);
                    ui.label(if self.selected.is_some() { "Hier erscheint euer lokaler Chatverlauf. Verbinde dich zum Schreiben." } else { "Wähle einen Kontakt oder suche ein Gerät in deiner Nähe." });
                    ui.label(RichText::new("Bluetooth-Sichtbarkeit und Verkehrsmuster können erkennbar bleiben.").small().color(MUTED));
                });
            }
        });
        if let Some(id) = selected_result {
            self.selected = Some(id);
            self.rich.all_contacts = false;
        }
        for command in rich_commands {
            self.command(command);
        }
        if let Some(id) = retry_file {
            self.command(Command::RetryFile { id });
            self.integration.transfers_open = true;
        }
        if let Some(id) = export_file {
            self.pick(Picker::Export { id }, ui.ctx());
        }
        ui.separator();
        if let Some(target) = self.rich.reply.clone() {
            ui.horizontal(|ui| {
                ui.label("Antwort wird mit der ausgewählten Nachricht verknüpft.");
                if ui.small_button("Abbrechen").clicked() {
                    self.rich.reply = None;
                }
            });
            if !self
                .view
                .messages
                .iter()
                .any(|m| m.id == target && self.selected.as_deref() == Some(&m.contact_id))
            {
                self.rich.reply = None;
            }
        }
        ui.add_enabled_ui(ready, |ui| {
            let response = ui.add_sized(
                [ui.available_width(), 68.0],
                egui::TextEdit::multiline(&mut *self.composer)
                    .id_salt("composer")
                    .hint_text(if ready {
                        "Deine Nachricht …"
                    } else {
                        "Zum Schreiben eine bestätigte Verbindung aufbauen"
                    })
                    .return_key(Some(egui::KeyboardShortcut::new(
                        egui::Modifiers::SHIFT,
                        egui::Key::Enter,
                    ))),
            );
            if response.changed() && self.view.share_typing {
                self.command(Command::Typing {
                    active: !self.composer.is_empty(),
                });
            }
            self.clipboard_paste(ui.ctx(), response.has_focus());
            let enter = response.has_focus() && ui.input_mut(take_plain_enter);
            let valid = !self.composer.trim().is_empty() && self.composer.len() <= MAX_TEXT;
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Enter sendet · Shift+Enter neue Zeile")
                        .small()
                        .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if (ui
                        .add_enabled(
                            valid,
                            egui::Button::new("Senden").fill(Color32::from_rgb(26, 85, 73)),
                        )
                        .clicked()
                        || enter)
                        && valid
                        && self.command(if let Some(target) = &self.rich.reply {
                            Command::Conversation(crate::conversation::Action::Reply {
                                target: target.clone(),
                                text: self.composer.to_string(),
                            })
                        } else {
                            Command::Send {
                                text: self.composer.clone(),
                            }
                        })
                    {
                        self.composer.zeroize();
                        self.rich.reply = None;
                        self.command(Command::Typing { active: false });
                        response.request_focus();
                    }
                });
            });
            if self.composer.len() > MAX_TEXT {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    "Die Nachricht ist zu lang (höchstens 16 KiB Text). Bitte teile sie auf.",
                );
            }
        });
    }

    fn settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Einstellungen").open(&mut open).collapsible(false).resizable(true).vscroll(true).max_height((ctx.content_rect().height()-60.0).max(260.0)).default_width(470.0).show(ctx, |ui| {
            self.desktop_settings(ui);
            ui.separator();
            ui.add_enabled_ui(self.view.unlocked, |ui| {
            let mut notifications = self.view.notifications;
            if ui.checkbox(&mut notifications, "Private Windows-Benachrichtigungen aktivieren").changed() { self.command(Command::SetNotifications { enabled: notifications }); }
            ui.label(RichText::new("Ohne Namen, Dateinamen oder Nachrichtenvorschau. Nur bei entsperrtem Hushlink im Hintergrund; Windows kann Mitteilungen unterdrücken. Schliessen ins Tray sperrt weiterhin und trennt Verbindungen.").small().color(MUTED));
            ui.separator();
            ui.heading("Automatisch sperren");
            let mut minutes = self.view.inactivity_minutes.max(1);
            if ui.add(egui::Slider::new(&mut minutes, 1..=120).suffix(" Min.")).changed() { self.command(Command::SetInactivity { minutes }); }
            ui.label(RichText::new("Beim Sperren der Windows-Sitzung wird Hushlink ebenfalls gesperrt. Offene Verbindungen werden beendet.").small().color(MUTED));
            ui.separator();
            self.storage_settings(ui);
            ui.separator();
            self.rich_settings(ui);
            ui.separator();
            ui.heading("Passwort ändern");
            ui.label("Bisheriges Passwort");
            ui.add(egui::TextEdit::singleline(&mut *self.password).id_salt("old_password").password(true));
            ui.label("Neues Passwort (mindestens 12 Zeichen)");
            ui.add(egui::TextEdit::singleline(&mut *self.new_password).id_salt("new_password").password(true));
            ui.label("Neues Passwort bestätigen");
            ui.add(egui::TextEdit::singleline(&mut *self.new_confirmation).id_salt("new_confirmation").password(true));
            if ui.add_enabled(!self.busy, egui::Button::new("Passwort ändern")).clicked() {
                if self.new_password.chars().count() < 12 || *self.new_password != *self.new_confirmation { self.error = "Das neue Passwort muss mindestens 12 Zeichen haben und mit der Bestätigung übereinstimmen.".into(); }
                else if self.command(Command::ChangePassword { old: self.password.clone(), new: self.new_password.clone() }) {
                    self.password.zeroize(); self.new_password.zeroize(); self.new_confirmation.zeroize(); clear_ui_memory(ui.ctx());
                }
            }
            });
            self.show_error(ui);
            ui.separator();
            ui.label(RichText::new(format!("Hushlink {} · Lokaler Bluetooth-Chat", env!("CARGO_PKG_VERSION"))).small().color(MUTED));
        });
        if !open {
            self.settings_open = false;
            self.password.zeroize();
            self.new_password.zeroize();
            self.new_confirmation.zeroize();
            clear_ui_memory(ctx);
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
            egui::SidePanel::left("contacts_panel")
                .default_width(280.0)
                .min_width(230.0)
                .max_width(350.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(16.0))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("sidebar_scroll")
                        .show(ui, |ui| self.sidebar(ui));
                });
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(20.0))
                .show(ctx, |ui| {
                    if self.view.unlocked {
                        self.chat(ui);
                    }
                });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("auth_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.auth(ui));
            });
        }
        self.poll_media(ctx);
        self.settings(ctx);
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
