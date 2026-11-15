use super::*;
use crate::{conversation::Action, runtime::MessageView};

#[derive(Default, Zeroize, zeroize::ZeroizeOnDrop)]
pub(super) struct Rich {
    pub query: String,
    pub all_contacts: bool,
    pub pinned_only: bool,
    pub reply: Option<String>,
    pub edit: Option<String>,
    pub edit_text: String,
    target: Option<String>,
    #[zeroize(skip)]
    pub(super) restore_source: Option<std::path::PathBuf>,
    #[zeroize(skip)]
    pub(super) restore_destination: Option<std::path::PathBuf>,
    pub(super) restore_open: bool,
    pub(super) restore_password: String,
    pub(super) storage_open: bool,
    delete_item: Option<String>,
}
impl Rich {
    pub(super) fn sync(&mut self, target: Option<&str>) {
        if self.target.as_deref() != target {
            self.reply.zeroize();
            self.reply = None;
            self.edit.zeroize();
            self.edit = None;
            self.edit_text.zeroize();
            self.target = target.map(str::to_owned);
        }
    }
    pub(super) fn controls(
        &mut self,
        ui: &mut egui::Ui,
        m: &MessageView,
        ready: bool,
    ) -> Option<Command> {
        let mut command = None;
        theme::compact_icon_menu(ui, theme::Icon::More, "Nachrichtenaktionen", |ui| {
            if ui
                .button(if m.pinned {
                    "Nicht mehr merken"
                } else {
                    "Nachricht merken"
                })
                .clicked()
            {
                command = Some(Command::Pin {
                    id: m.id.clone(),
                    pinned: !m.pinned,
                });
                ui.close();
            }
            if ready && !m.action_pending && m.status == MessageStatus::Received {
                if ui.button("Antworten").clicked() {
                    self.reply = Some(m.id.clone());
                    ui.close();
                }
                ui.menu_button("Reagieren", |ui| {
                    for emoji in crate::conversation::REACTIONS {
                        if ui
                            .button(if emoji.is_empty() {
                                "Reaktion entfernen"
                            } else {
                                emoji
                            })
                            .clicked()
                        {
                            command = Some(Command::Conversation(Action::React {
                                target: m.id.clone(),
                                revision: m.reaction_revision + 1,
                                reaction: (*emoji).into(),
                            }));
                            ui.close();
                        }
                    }
                });
                if m.outgoing
                    && m.attachment_id.is_none()
                    && ui.button("Nachricht bearbeiten").clicked()
                {
                    self.edit = Some(m.id.clone());
                    self.edit_text.zeroize();
                    self.edit_text.clone_from(&m.text);
                    ui.close();
                }
            }
        });
        command
    }
    pub(super) fn reactions(ui: &mut egui::Ui, m: &MessageView) {
        if !m.reaction_local.is_empty() || !m.reaction_remote.is_empty() {
            ui.horizontal(|ui| {
                for (reaction, label) in [
                    (&m.reaction_local, "Deine Reaktion"),
                    (&m.reaction_remote, "Reaktion der Gegenstelle"),
                ] {
                    if !reaction.is_empty() {
                        egui::Frame::new()
                            .fill(Color32::from_black_alpha(22))
                            .corner_radius(12)
                            .inner_margin(egui::Margin::symmetric(7, 2))
                            .show(ui, |ui| {
                                ui.label(RichText::new(reaction).size(13.0))
                                    .on_hover_text(label);
                            });
                    }
                }
            });
        }
        if m.action_pending {
            ui.label(
                RichText::new("Änderung noch unbestätigt")
                    .size(10.0)
                    .color(Color32::from_rgb(239, 191, 118)),
            );
        }
    }
    pub(super) fn matches(&self, m: &MessageView) -> bool {
        (!self.pinned_only || m.pinned)
            && (self.query.is_empty()
                || Zeroizing::new(m.text.to_lowercase())
                    .contains(Zeroizing::new(self.query.to_lowercase()).as_str()))
    }
}
impl App {
    pub(super) fn rich_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Gespräch und Privatsphäre");
        let mut typing = self.view.share_typing;
        if ui
            .checkbox(
                &mut typing,
                "Schreibanzeige mit bestätigten Kontakten teilen",
            )
            .changed()
        {
            self.command(Command::SetShareTyping { enabled: typing });
        }
        ui.label(RichText::new("Teilt nur den Schreibstatus. Er verschwindet nach wenigen Sekunden; eingegebener Text wird erst beim Senden übertragen.").small().color(MUTED));
        let blocked = self.view.unknown_block_until_ms > now_ms();
        ui.label(if blocked {
            "Unbekannte Verbindungsanfragen sind derzeit gesperrt."
        } else {
            "Unbekannte Anfragen: höchstens eine alle zehn Sekunden, nur beim sichtbaren Anbieten."
        });
        ui.horizontal_wrapped(|ui| {
            for (label, duration) in [
                ("30 Minuten sperren", 1_800_000),
                ("Dauerhaft sperren", u64::MAX),
                ("Wieder zulassen", 0),
            ] {
                if ui.button(label).clicked() {
                    self.command(Command::BlockUnknown {
                        until_ms: if duration == 0 || duration == u64::MAX {
                            duration
                        } else {
                            now_ms().saturating_add(duration)
                        },
                    });
                }
            }
        });
        ui.label(RichText::new("Bekannte Bluetooth-Adressen dürfen weiterhin anfragen. Die verschlüsselte Identitätsprüfung bleibt erforderlich.").small().color(MUTED));
    }
    pub(super) fn storage_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sicherungen und Speicher");
        ui.label(format!(
            "Tresordatei: {:.1} MiB · Dateiinhalte: {:.1} MiB",
            self.view.storage_bytes as f64 / 1_048_576.0,
            self.view.attachment_bytes as f64 / 1_048_576.0
        ));
        if ui
            .button("Speicherübersicht und gezieltes Löschen …")
            .clicked()
        {
            self.rich.storage_open = true;
        }
        ui.label("Sicherungen enthalten ausschliesslich den verschlüsselten Tresor. Zum Wiederherstellen ist das zum Sicherungszeitpunkt gültige Passwort nötig.");
        if self.view.backup_folder.is_empty() {
            ui.label("Automatische Sicherung: aus");
        } else {
            ui.add(
                egui::Label::new(format!("Sicherungsordner: {}", self.view.backup_folder)).wrap(),
            );
            ui.label(if self.view.last_backup_ms == 0 {
                "Noch keine erfolgreiche Sicherung".into()
            } else {
                format!("Letzte Sicherung: {}", local_time(self.view.last_backup_ms))
            });
        }
        if ui.button("Sicherungsordner wählen …").clicked() {
            self.pick(Picker::BackupFolder, ui.ctx());
        }
        let mut retention = self.view.backup_retention.max(1);
        let mut interval = self.view.backup_interval_minutes.max(5);
        let changed = ui
            .add(egui::Slider::new(&mut retention, 1..=20).text("Sicherungen behalten"))
            .changed()
            | ui.add(
                egui::Slider::new(&mut interval, 5..=10080)
                    .logarithmic(true)
                    .text("Intervall in Minuten"),
            )
            .changed();
        if changed {
            self.command(Command::SetBackups {
                folder: self.view.backup_folder.clone(),
                retention,
                interval_minutes: interval,
            });
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.view.backup_folder.is_empty(),
                    egui::Button::new("Jetzt verschlüsselt sichern"),
                )
                .clicked()
            {
                self.command(Command::BackupNow);
            }
            if ui.button("Automatische Sicherung ausschalten").clicked() {
                self.command(Command::SetBackups {
                    folder: String::new(),
                    retention,
                    interval_minutes: interval,
                });
            }
        });
        ui.label(RichText::new("Automatische Sicherungen laufen nur bei entsperrtem Tresor. Nur von diesem Profil erzeugte Sicherungsdateien werden nach der Aufbewahrungsgrenze entfernt. Kopien und alte Passwörter können alte Verläufe weiterhin zugänglich machen.").small().color(MUTED));
    }
    pub(super) fn backup_windows(&mut self, ctx: &egui::Context) {
        if self.rich.storage_open && self.view.unlocked {
            let mut open = true;
            egui::Window::new("Lokaler Speicher").open(&mut open).vscroll(true).default_width(500.0).max_height((ctx.content_rect().height()-80.0).max(240.0)).show(ctx,|ui|{
                ui.label("Löschen betrifft diesen PC. Gegenstelle, Sicherungen und bereits exportierte Dateien behalten ihre Kopien. Sichere physische Löschung auf SSDs wird nicht zugesichert.");
                let disconnected=self.view.peer.is_none() && !self.view.connecting;
                ui.label(if disconnected{"Einträge zum lokalen Löschen auswählen."}else{"Vor dem Löschen die Verbindung trennen."});
                let mut chosen=None;
                for m in &self.view.messages{ui.horizontal(|ui|{let name=m.attachment_name.as_deref().unwrap_or(&m.text);let preview=Zeroizing::new(name.chars().take(64).collect::<String>());ui.label(preview.as_str());if m.attachment_id.is_some(){ui.label(format!("{:.1} KiB",m.attachment_size as f64/1024.0));}
if ui.add_enabled(disconnected && !m.action_pending,egui::Button::new("Löschen …")).clicked(){chosen=Some(m.id.clone());}});}
                if let Some(id)=chosen{self.rich.delete_item=Some(id);}
                if let Some(id)=self.rich.delete_item.clone(){ui.separator();ui.label("Diesen Eintrag samt zugehörigem Anhang lokal entfernen?");if ui.button("Bewusst löschen").clicked() && self.command(Command::DeleteMessage{id}){self.rich.delete_item=None;clear_ui_memory(ctx);}
if ui.button("Behalten").clicked(){self.rich.delete_item=None;}}
            });
            if !open {
                self.rich.storage_open = false;
                self.rich.delete_item = None;
                clear_ui_memory(ctx);
            }
        }
        if self.rich.restore_open && !self.view.unlocked {
            let mut open = true;
            egui::Window::new("Verschlüsselte Sicherung wiederherstellen").open(&mut open).collapsible(false).default_width(470.0).vscroll(true).max_height((ctx.content_rect().height()-80.0).max(220.0)).show(ctx,|ui|{
                ui.label("Die Sicherung wird geprüft und an einen neuen, freien Speicherort kopiert. Vorhandene Tresore und die Sicherung bleiben erhalten. Danach wird die neue Kopie ausgewählt.");
                if ui.button("Sicherungsdatei auswählen …").clicked(){self.pick(Picker::RestoreSource,ctx);}
                if let Some(p)=&self.rich.restore_source{ui.add(egui::Label::new(p.display().to_string()).wrap());}
                if ui.button("Neuen Zielpfad auswählen …").clicked(){self.pick(Picker::RestoreDestination,ctx);}
                if let Some(p)=&self.rich.restore_destination{ui.add(egui::Label::new(p.display().to_string()).wrap());}
                ui.label("Passwort dieser Sicherung");ui.add(egui::TextEdit::singleline(&mut self.rich.restore_password).password(true));
                if ui.add_enabled(!self.busy && self.rich.restore_source.is_some() && self.rich.restore_destination.is_some(),egui::Button::new("Prüfen und als neuen Tresor wiederherstellen")).clicked() && let (Some(source),Some(destination))=(self.rich.restore_source.clone(),self.rich.restore_destination.clone()) && self.command(Command::RestoreBackup{source,destination,password:Zeroizing::new(self.rich.restore_password.clone())}){self.rich.restore_password.zeroize();self.rich.restore_open=false;self.busy=true;clear_ui_memory(ctx);}
                self.show_error(ui);
            });
            if !open {
                self.rich.restore_open = false;
                self.rich.restore_password.zeroize();
                self.rich.restore_source = None;
                self.rich.restore_destination = None;
                clear_ui_memory(ctx);
            }
        }
    }
    pub(super) fn edit_window(&mut self, ctx: &egui::Context) {
        let Some(message) = self
            .view
            .messages
            .iter()
            .find(|m| self.rich.edit.as_deref() == Some(&m.id))
            .cloned()
        else {
            return;
        };
        let mut open = true;
        egui::Window::new("Nachricht bearbeiten").open(&mut open).collapsible(false).default_width(430.0).vscroll(true).max_height((ctx.content_rect().height()-80.0).max(220.0)).show(ctx,|ui|{
            ui.label("Die Gegenstelle sieht die bearbeitete Nachricht mit Kennzeichnung. Kopien des ursprünglichen Textes können bestehen bleiben.");
            ui.add_sized([ui.available_width(),110.0],egui::TextEdit::multiline(&mut self.rich.edit_text));
            let ready=self.view.peer.as_ref().is_some_and(|p|p.ready && p.id==message.contact_id);
            if ui.add_enabled(ready && !message.action_pending && !self.rich.edit_text.trim().is_empty() && self.rich.edit_text.len()<=MAX_TEXT,egui::Button::new("Änderung senden")).clicked() && self.command(Command::Conversation(Action::Edit{target:message.id.clone(),revision:message.edit_revision+1,text:self.rich.edit_text.clone()})){self.rich.edit=None;self.rich.edit_text.zeroize();clear_ui_memory(ctx);}
        });
        if !open {
            self.rich.edit = None;
            self.rich.edit_text.zeroize();
            clear_ui_memory(ctx);
        }
    }
}
pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis().min(u128::from(u64::MAX)) as u64)
}
