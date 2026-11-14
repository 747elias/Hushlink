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
        ui.horizontal_wrapped(|ui| {
            if ui
                .small_button(if m.pinned { "★ Lösen" } else { "☆ Merken" })
                .clicked()
            {
                command = Some(Command::Pin {
                    id: m.id.clone(),
                    pinned: !m.pinned,
                });
            }
            if ready && !m.action_pending && m.status == MessageStatus::Received {
                if ui.small_button("Antworten").clicked() {
                    self.reply = Some(m.id.clone());
                }
                ui.menu_button("Reaktion", |ui| {
                    for emoji in crate::conversation::REACTIONS {
                        if ui
                            .button(if emoji.is_empty() { "Entfernen" } else { emoji })
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
                    && ui.small_button("Bearbeiten").clicked()
                {
                    self.edit = Some(m.id.clone());
                    self.edit_text.zeroize();
                    self.edit_text.clone_from(&m.text);
                }
            }
        });
        if !m.reaction_local.is_empty() || !m.reaction_remote.is_empty() {
            ui.label(format!(
                "Du: {}   Gegenstelle: {}",
                if m.reaction_local.is_empty() {
                    "–"
                } else {
                    &m.reaction_local
                },
                if m.reaction_remote.is_empty() {
                    "–"
                } else {
                    &m.reaction_remote
                }
            ));
        }
        if m.edited {
            ui.label(RichText::new("Bearbeitet").small().color(MUTED));
        }
        if m.action_pending {
            ui.label(
                RichText::new("Änderung noch unbestätigt")
                    .small()
                    .color(Color32::YELLOW),
            );
        }
        command
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
        egui::Window::new("Nachricht bearbeiten").open(&mut open).collapsible(false).default_width(430.0).show(ctx,|ui|{
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
