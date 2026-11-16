use super::*;
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct Features {
    contact: Option<String>,
    alias: Zeroizing<String>,
    favorite: bool,
    forget: bool,
    pub(super) wizard: bool,
    receiver: Option<bool>,
    dropped: Vec<PathBuf>,
    drop_peer: Option<String>,
    read_pending: Option<String>,
}
impl Features {
    fn sync_drop(&mut self, unlocked: bool, target: Option<&str>) {
        if !unlocked || self.drop_peer.as_deref() != target {
            self.dropped.clear();
            self.drop_peer.zeroize();
            self.drop_peer = None;
        }
    }
    fn queue_files(
        &mut self,
        files: Vec<egui::DroppedFile>,
        unlocked: bool,
        target: Option<&str>,
    ) -> Result<(), &'static str> {
        self.sync_drop(unlocked, target);
        let target = target.filter(|_| unlocked).ok_or("Zum Ablegen von Dateien zuerst entsperren und mit dem ausgewählten bestätigten Kontakt verbinden.")?;
        if files.len() + self.dropped.len() > 16 || files.iter().any(|d| d.path.is_none()) {
            return Err(
                "Bitte höchstens 16 lokale Dateien ablegen. Dateien ohne lokalen Pfad werden nicht unterstützt.",
            );
        }
        self.drop_peer = Some(target.to_owned());
        for file in files {
            if let Some(path) = file.path
                && !self.dropped.contains(&path)
            {
                self.dropped.push(path);
            }
        }
        Ok(())
    }
    pub(super) fn clear(&mut self) {
        self.contact.zeroize();
        self.contact = None;
        self.alias.zeroize();
        self.forget = false;
        self.dropped.clear();
        self.drop_peer.zeroize();
        self.drop_peer = None;
        self.read_pending.zeroize();
        self.read_pending = None;
        self.wizard = false;
        self.receiver = None;
    }
}
impl Drop for Features {
    fn drop(&mut self) {
        self.clear();
    }
}
impl App {
    pub(super) fn open_contact(&mut self, id: &str) {
        if let Some(c) = self.view.contacts.iter().find(|c| c.id == id) {
            self.features.contact = Some(c.id.clone());
            *self.features.alias = c.alias.clone();
            self.features.favorite = c.favorite;
            self.features.forget = false;
        }
    }
    pub(super) fn feature_events(&mut self, ctx: &egui::Context) {
        let foreground = self.view.unlocked
            && ctx.input(|i| i.focused && !i.viewport().minimized.unwrap_or(false));
        if let Some(runtime) = &self.runtime {
            runtime.notifications().set_foreground(foreground);
        }
        if foreground
            && !self.design.groups_open
            && let Some(c) = self
                .view
                .contacts
                .iter()
                .find(|c| self.selected.as_deref() == Some(&c.id) && c.unread > 0)
            && self.features.read_pending.as_deref() != Some(&c.id)
        {
            let id = c.id.clone();
            if self.command(Command::MarkRead { id: id.clone() }) {
                self.features.read_pending = Some(id);
            }
        }
        if self
            .view
            .contacts
            .iter()
            .all(|c| self.features.read_pending.as_deref() != Some(&c.id) || c.unread == 0)
        {
            self.features.read_pending = None;
        }
        let target = self
            .view
            .peer
            .as_ref()
            .filter(|p| {
                !self.design.groups_open && p.ready && self.selected.as_deref() == Some(&p.id)
            })
            .map(|p| p.id.clone());
        self.features
            .sync_drop(self.view.unlocked, target.as_deref());
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        if !dropped.is_empty()
            && let Err(error) =
                self.features
                    .queue_files(dropped, self.view.unlocked, target.as_deref())
        {
            self.error = error.into();
        }
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            egui::Area::new(egui::Id::new("drop_hint"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .inner_margin(24.0)
                        .show(ui, |ui| {
                            ui.strong("Dateien ablegen – vor dem Senden noch einmal bestätigen");
                        });
                });
        }
    }
    pub(super) fn feature_windows(&mut self, ctx: &egui::Context) {
        self.contact_window(ctx);
        self.drop_window(ctx);
        self.connection_assistant(ctx);
    }
    fn contact_window(&mut self, ctx: &egui::Context) {
        let Some(contact) = self
            .view
            .contacts
            .iter()
            .find(|c| self.features.contact.as_deref() == Some(&c.id))
            .cloned()
        else {
            return;
        };
        let mut open = true;
        let mut removed = false;
        egui::Window::new("Kontakt verwalten").open(&mut open).collapsible(false).default_width(440.0).vscroll(true).max_height((ctx.content_rect().height()-60.0).max(220.0)).show(ctx, |ui| {
            ui.heading(&contact.name);
            ui.label("Dein eigener Kontaktname (leer = Name der Gegenstelle)");
            ui.add(egui::TextEdit::singleline(&mut *self.features.alias).char_limit(80));
            ui.checkbox(&mut self.features.favorite, "Als Favorit oben anzeigen");
            if ui.button("Kontakt speichern").clicked() {
                self.command(Command::EditContact { id: contact.id.clone(), alias: self.features.alias.to_string(), favorite: self.features.favorite });
            }
            ui.separator();
            ui.label(if contact.muted_until_ms>super::rich::now_ms(){"Benachrichtigungen für diesen Kontakt stumm"}else{"Benachrichtigungen für diesen Kontakt aktiv"});
            ui.horizontal_wrapped(|ui|{for (label,duration) in [("1 Stunde",3_600_000),("1 Tag",86_400_000),("Dauerhaft",u64::MAX),("Laut",0)]{if ui.button(label).clicked(){self.command(Command::MuteContact{id:contact.id.clone(),until_ms:if duration==0 || duration==u64::MAX{duration}else{super::rich::now_ms().saturating_add(duration)}});}}});
            ui.separator();
            ui.label(format!("Name der Gegenstelle: {}", contact.advertised_name));
            ui.label(format!("Bluetooth-Adresse: {}", contact.address));
            ui.colored_label(MINT, "Identität unabhängig bestätigt und verschlüsselt gespeichert");
            ui.collapsing("Bestätigte Geräteidentität ansehen", |ui| { ui.add(egui::Label::new(RichText::new(&contact.identity).monospace()).wrap()); });
            ui.label(RichText::new("Ein eigener Name oder ein Gerätename ersetzt keinen Sicherheitscodevergleich. Eine neue Identität verlangt weiterhin eine neue Prüfung.").small().color(MUTED));
            ui.separator();
            if ui.button("Kontakt und lokalen Verlauf entfernen …").clicked() { self.features.forget = true; }
            if self.features.forget {
                ui.colored_label(Color32::LIGHT_RED, "Kontakt, Nachrichten, Anhänge und Fortsetzungspunkte auf diesem PC entfernen? Beim nächsten Kontakt ist eine neue Identitätsprüfung nötig.");
                if ui.button("Bewusst lokal entfernen").clicked() && self.command(Command::ForgetContact { id: contact.id.clone() }) { removed = true; }
                if ui.button("Behalten").clicked() { self.features.forget = false; }
            }
            self.show_error(ui);
        });
        if !open || removed {
            self.features.contact.zeroize();
            self.features.contact = None;
            self.features.alias.zeroize();
            clear_ui_memory(ctx);
        }
    }
    fn drop_window(&mut self, ctx: &egui::Context) {
        if self.features.dropped.is_empty() {
            return;
        }
        let mut open = true;
        let mut send = None;
        let mut remove = None;
        let ready = self
            .view
            .peer
            .as_ref()
            .is_some_and(|p| p.ready && Some(&p.id) == self.features.drop_peer.as_ref());
        let busy = self.view.transfers.iter().any(|t| {
            t.outgoing
                && matches!(
                    t.state,
                    crate::runtime::FileTransferState::AwaitingConsent
                        | crate::runtime::FileTransferState::Transferring
                )
        });
        egui::Window::new("Abgelegte Dateien").open(&mut open).default_width(450.0).vscroll(true).max_height((ctx.content_rect().height()-60.0).max(220.0)).show(ctx, |ui| {
            let recipient = self.view.contacts.iter().find(|c| Some(&c.id) == self.features.drop_peer.as_ref()).map_or("bestätigter Kontakt", |c| c.name.as_str());
            ui.label(format!("Empfänger: {recipient}"));
            ui.label("Bis 20 MiB pro Datei. Jede Datei bewusst anbieten; die andere Person muss sie annehmen.");
            for (index, path) in self.features.dropped.iter().enumerate() {
                ui.group(|ui| {
                    ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                    ui.horizontal(|ui| {
                        if ui.add_enabled(ready && !busy && !self.busy, egui::Button::new("Jetzt anbieten")).clicked() { send = Some(index); }
                        if ui.small_button("Entfernen").clicked() { remove = Some(index); }
                    });
                });
            }
        });
        if let Some(index) = send {
            let path = self.features.dropped.remove(index);
            if self.command(Command::SendFile { path }) {
                self.integration.transfers_open = true;
            }
        } else if let Some(index) = remove {
            self.features.dropped.remove(index);
        }
        if !open {
            self.features.dropped.clear();
            self.features.drop_peer = None;
        }
    }
    fn connection_assistant(&mut self, ctx: &egui::Context) {
        if !self.features.wizard || !self.view.unlocked {
            return;
        }
        let mut open = true;
        egui::Window::new("Verbindungsassistent").open(&mut open).default_width(500.0).vscroll(true).max_height((ctx.content_rect().height()-60.0).max(240.0)).show(ctx, |ui| {
            ui.heading("1 · Bluetooth auf beiden PCs vorbereiten");
            ui.label(&self.view.availability);
            if self.view.bluetooth_checked && self.view.bluetooth_available {
                ui.colored_label(MINT, "Ein Adapter mit Classic-Unterstützung wurde von Windows erkannt.");
                if !self.view.radio_connectable { ui.colored_label(Color32::LIGHT_YELLOW, "Windows erlaubt gerade keine eingehenden Verbindungen. Bluetooth-Einstellungen und Richtlinien prüfen."); }
                if !self.view.radio_discoverable { ui.label("Windows meldet den PC als nicht auffindbar. Für die erste Suche gegebenenfalls die Bluetooth-Einstellungen auf diesem PC geöffnet lassen. Bereits gekoppelte Geräte können trotzdem erreichbar sein."); }
            }
            if ui.add_enabled(self.view.bluetooth_checked, egui::Button::new("Bluetooth erneut prüfen")).clicked() { self.command(Command::RefreshBluetooth); }
            if ui.button("Windows-Bluetooth-Einstellungen öffnen").clicked() && crate::transport::open_bluetooth_settings().is_err() { self.error="Öffne Windows → Einstellungen → Bluetooth und Geräte.".into(); }
            ui.label("Bluetooth auf beiden PCs einschalten. Bei Bedarf die PCs in Windows koppeln. Zum ersten Finden die Windows-Bluetooth-Seite auf dem empfangenden PC geöffnet lassen. Beide Geräte brauchen Bluetooth Classic; nur BLE genügt nicht.");
            ui.separator(); ui.heading("2 · Eine Person empfängt, die andere verbindet");
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(self.features.receiver == Some(true), "Dieser PC empfängt").clicked() { self.features.receiver = Some(true); }
                if ui.selectable_label(self.features.receiver == Some(false), "Dieser PC verbindet").clicked() { self.features.receiver = Some(false); }
            });
            if self.features.receiver == Some(true) {
                ui.label("Auf dem anderen PC „Dieser PC verbindet“ wählen und nach diesem PC suchen.");
                if self.view.listening { ui.colored_label(MINT, "Empfang ist tatsächlich aktiv."); }
                else if ui.add_enabled(self.view.peer.is_none(), egui::Button::new("Empfang bewusst starten")).clicked() { self.command(Command::StartListening); }
                if let Some(pending) = self.view.pending_incoming.clone() {
                    ui.label(format!("Anfrage: {} ({})", pending.name, pending.address));
                    if ui.button("Anfrage annehmen").clicked() { self.command(Command::Respond { accept: true }); }
                    if ui.button("Anfrage ablehnen").clicked() { self.command(Command::Respond { accept: false }); }
                }
            } else if self.features.receiver == Some(false) {
                ui.label("Auf der Gegenstelle zuerst den Empfang starten. Ein Suchergebnis allein bestätigt noch keinen Hushlink-Dienst.");
                if ui.add_enabled(!self.view.discovering, egui::Button::new("Geräte bewusst suchen")).clicked() { self.command(Command::Discover); }
                if self.view.discovering { ui.spinner(); }
                for device in self.view.devices.clone() { ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{} · {}", device.name, device.address));
                    if ui.add_enabled(self.view.peer.is_none() && !self.view.connecting, egui::Button::new("Verbinden")).clicked() { self.command(Command::Connect { address: device.address.clone() }); }
                }); }
            }
            if self.view.connecting { ui.spinner(); ui.label("Windows stellt die Verbindung her. Pairing-Hinweise von Windows beachten."); }
            ui.separator(); ui.heading("3 · Identität prüfen");
            if let Some(peer) = self.view.peer.clone() {
                if peer.ready { ui.colored_label(MINT, "Verbunden – beide Geräte haben die Identität bestätigt. Der Chat ist bereit."); }
                else {
                    ui.label("Alle Gruppen auf beiden PCs unabhängig vergleichen. Ein Gerätename und Windows-Pairing reichen dafür nicht.");
                    ui.add(egui::Label::new(RichText::new(&peer.fingerprint).monospace().color(MINT)).wrap());
                    if peer.local_confirmed { ui.label("Lokal bestätigt – warte auf die andere Person."); }
                    else if !peer.fingerprint.is_empty() {
                        ui.checkbox(&mut self.fingerprint_checked, "Alle Gruppen wurden unabhängig verglichen und stimmen überein");
                        if ui.add_enabled(self.fingerprint_checked, egui::Button::new("Identität bestätigen")).clicked() { self.command(Command::ConfirmPeer); self.fingerprint_checked = false; }
                    }
                }
            } else { ui.label("Dieser Schritt wird nach dem Verbindungsaufbau freigegeben. Bei Schlüsselwechsel die Warnung im Chat prüfen; der Assistent umgeht sie nicht."); }
            ui.separator(); ui.label(&self.view.status); self.show_error(ui);
            if ui.button("Vorgänge stoppen und Verbindung trennen").clicked() { self.command(Command::CancelBluetooth); }
            ui.label(RichText::new("Schliessen dieses Assistenten stoppt einen bewusst gestarteten Empfang nicht. Dafür „Vorgänge stoppen“ wählen.").small().color(MUTED));
        });
        self.features.wizard = open;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn file(name: &str) -> egui::DroppedFile {
        egui::DroppedFile {
            path: Some(PathBuf::from(name)),
            ..Default::default()
        }
    }
    #[test]
    fn dropped_files_require_trust_and_explicit_choice_and_disappear_on_lock_or_recipient_change() {
        let mut features = Features::default();
        assert!(
            features
                .queue_files(vec![file("synthetic.txt")], false, Some("peer-a"))
                .is_err()
        );
        assert!(
            features
                .queue_files(vec![file("synthetic.txt")], true, None)
                .is_err()
        );
        features
            .queue_files(
                vec![file("synthetic.txt"), file("synthetic.txt")],
                true,
                Some("peer-a"),
            )
            .unwrap();
        assert_eq!(features.dropped.len(), 1); // queued only; nothing reads or sends this path
        features.sync_drop(true, Some("peer-b"));
        assert!(features.dropped.is_empty());
        assert!(
            features
                .queue_files(vec![egui::DroppedFile::default()], true, Some("peer-a"))
                .is_err()
        );
        assert!(
            features
                .queue_files(
                    (0..17).map(|n| file(&format!("synthetic-{n}"))).collect(),
                    true,
                    Some("peer-a")
                )
                .is_err()
        );
        features
            .queue_files(vec![file("synthetic.txt")], true, Some("peer-a"))
            .unwrap();
        features.alias = Zeroizing::new("SYNTHETIC_PRIVATE_ALIAS".into());
        features.contact = Some("peer-a".into());
        features.clear();
        assert!(
            features.dropped.is_empty() && features.alias.is_empty() && features.contact.is_none()
        );
    }
}
