use super::*;
use crate::runtime::GroupCommand;
use egui::{Sense, vec2};
use theme::Icon;

impl App {
    fn group_command(&mut self, c: GroupCommand) -> bool {
        self.command(Command::Group(c))
    }
    fn group_name(&self, id: &str) -> String {
        if id == self.view.group_identity {
            return "Du".into();
        }
        self.view
            .contacts
            .iter()
            .find(|c| c.id == id)
            .map_or_else(|| id.into(), |c| c.name.clone())
    }
    pub(super) fn group_list(&mut self, ui: &mut egui::Ui) {
        egui::TopBottomPanel::bottom("groups_sync").show_separator_line(false).frame(egui::Frame::new().fill(PANEL).inner_margin(16)).show_inside(ui,|ui|{
            ui.label(RichText::new(format!("{} Übertragungen ausstehend",self.view.group_pending)).size(12.0).color(MUTED));
            if ui.add_enabled(self.view.bluetooth_available || self.view.group_sync,egui::Button::new(if self.view.group_sync{"Synchronisierung pausieren"}else{"Gruppen synchronisieren"})).clicked(){self.group_command(GroupCommand::Sync(!self.view.group_sync));}
            ui.label(RichText::new(if self.view.group_sync{"Aktiv · bestätigte Kontakte werden automatisch verbunden und angenommen."}else{"Pausiert · zum Austausch auf allen PCs Empfang oder Synchronisierung einschalten."}).size(11.0).color(if self.view.group_sync{MINT}else{MUTED}));
            if !self.view.bluetooth_available {ui.label(RichText::new(&self.view.availability).size(11.0).color(MUTED));}
        });
        egui::Frame::new().inner_margin(20).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Gruppen").size(27.0));
                if theme::icon_button(ui, Icon::Plus, "Neue Gruppe", false).clicked() {
                    self.design.group_create = true;
                    self.design.group_edit = None;
                    self.design.group_name.zeroize();
                    self.design.group_members.zeroize();
                }
            });
        });
        let mut chosen = None;
        egui::ScrollArea::vertical().auto_shrink([false,false]).show(ui,|ui|{
            if self.view.groups.is_empty(){egui::Frame::new().inner_margin(20).show(ui,|ui|{ui.heading("Zusammen schreiben.");ui.label(RichText::new("Erstelle eine Gruppe aus bestätigten Kontakten. Jede Person nimmt die Einladung auf ihrem PC an.").color(MUTED));if theme::primary(ui,"Gruppe erstellen").clicked(){self.design.group_create=true;}});}
            for g in &self.view.groups {
                let label=format!("{}{}\n{} Mitglieder · {}",g.roster.name,if g.unread>0{format!("  ({})",g.unread)}else{String::new()},g.roster.members.len(),if g.closed{"Geschlossen"}else if g.active{"Beigetreten"}else if g.departed.contains(&self.view.group_identity){"Verlassen/abgelehnt"}else{"Einladung / neue Mitgliederliste"});
                if ui.add_sized([ui.available_width(),74.0],egui::Button::new(RichText::new(label).size(13.0)).selected(self.design.selected_group.as_deref()==Some(&g.roster.id))).clicked(){chosen=Some(g.roster.id.clone());}
            }
        });
        if let Some(id) = chosen {
            self.design.selected_group = Some(id.clone());
            self.design.group_composer.zeroize();
            self.design.group_delete = false;
            self.group_command(GroupCommand::Read { id });
        }
    }
    pub(super) fn group_chat(&mut self, ui: &mut egui::Ui) {
        let group = self
            .view
            .groups
            .iter()
            .find(|g| self.design.selected_group.as_deref() == Some(&g.roster.id))
            .cloned();
        let Some(g) = group else {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading("Ein Raum für eure Runde.");
                ui.label("Wähle eine Gruppe oder erstelle eine neue.");
                ui.label(
                    RichText::new("Direkte verschlüsselte Zustellung. Kein zentraler Vermittler.")
                        .color(MUTED),
                );
            });
            return;
        };
        let local = self.view.group_identity.clone();
        let known: Vec<_> = self.view.contacts.iter().map(|c| c.id.clone()).collect();
        let all_known = g
            .roster
            .members
            .iter()
            .all(|m| m == &local || known.contains(m));
        egui::TopBottomPanel::top("group_header")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(20, 14)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::hover());
                    theme::avatar(ui.painter(), r.center(), 21.0, &g.roster.name, &g.roster.id);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&g.roster.name).size(21.0));
                        ui.label(
                            RichText::new(format!(
                                "{} Mitglieder · {} beigetreten · Liste {}",
                                g.roster.members.len(),
                                g.accepted
                                    .iter()
                                    .filter(|m| !g.departed.contains(m))
                                    .count(),
                                g.roster.revision
                            ))
                            .size(11.0)
                            .color(MUTED),
                        );
                    });
                    theme::icon_menu(ui, Icon::More, "Gruppenmenü", |ui| {
                        if g.active
                            && g.roster.owner == local
                            && ui.button("Name / Mitglieder ändern …").clicked()
                        {
                            self.design.group_name = g.roster.name.clone();
                            self.design.group_members = g
                                .roster
                                .members
                                .iter()
                                .filter(|m| *m != &local)
                                .cloned()
                                .collect();
                            self.design.group_edit = Some(g.roster.id.clone());
                            self.design.group_create = true;
                            ui.close();
                        }
                        if g.active
                            && ui
                                .button(if g.roster.owner == local {
                                    "Gruppe schliessen …"
                                } else {
                                    "Gruppe verlassen …"
                                })
                                .clicked()
                        {
                            self.design.group_delete = true;
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.view.peer.is_none(),
                                egui::Button::new("Lokalen Verlauf löschen"),
                            )
                            .clicked()
                        {
                            self.group_command(GroupCommand::ClearHistory {
                                id: g.roster.id.clone(),
                            });
                            ui.close();
                        }
                        if !g.active
                            && ui
                                .add_enabled(
                                    self.view.peer.is_none(),
                                    egui::Button::new("Gruppe lokal entfernen"),
                                )
                                .clicked()
                        {
                            self.group_command(GroupCommand::Remove {
                                id: g.roster.id.clone(),
                            });
                            ui.close();
                        }
                    });
                });
            });
        let ready = g.active && !g.closed && all_known;
        egui::TopBottomPanel::bottom("group_composer").show_separator_line(false).frame(egui::Frame::new().fill(theme::SURFACE).inner_margin(16)).show_inside(ui,|ui|{
            ui.add_enabled_ui(ready,|ui|{
                let response=ui.add(egui::TextEdit::multiline(&mut self.design.group_composer).desired_rows(2).desired_width(ui.available_width()).char_limit(MAX_TEXT).hint_text("Nachricht an die Gruppe …").return_key(Some(egui::KeyboardShortcut::new(egui::Modifiers::SHIFT,egui::Key::Enter))));
                let enter=response.has_focus() && ui.input_mut(take_plain_enter);
                if (theme::primary(ui,"An Gruppe senden").clicked() || enter) && ready && !self.design.group_composer.trim().is_empty() && self.design.group_composer.len()<=MAX_TEXT && self.group_command(GroupCommand::Send{id:g.roster.id.clone(),text:self.design.group_composer.clone()}){self.design.group_composer.zeroize();}
            });
            ui.label(RichText::new("Enter sendet · Shift+Enter neue Zeile · Offline-Mitglieder bleiben ausstehend").size(10.0).color(MUTED));
        });
        egui::ScrollArea::vertical().id_salt(("group_history",&g.roster.id)).auto_shrink([false,false]).show(ui,|ui|{
            ui.add_space(12.0);self.show_error(ui);
            egui::Frame::new().fill(PANEL).corner_radius(12).inner_margin(14).show(ui,|ui|{
                ui.horizontal_wrapped(|ui|{for member in &g.roster.members {let name=Zeroizing::new(self.group_name(member));ui.label(RichText::new(format!("{}{} · {}",name.as_str(),if *member==g.roster.owner{" (Besitzer)"}else{""},if g.departed.contains(member){"ausgetreten"}else if g.accepted.contains(member){"beigetreten"}else{"wartet"})).size(12.0).color(if g.accepted.contains(member){MINT}else{MUTED})).on_hover_text(member);}});
                if !all_known {ui.label("Vor dem Beitritt alle Mitglieder unabhängig als Kontakte prüfen. Unbekannte Identitäten stehen oben vollständig im Tooltip.");if ui.button("Kontakte / Verbindungssuche öffnen").clicked(){self.design.groups_open=false;self.design.connections_open=true;}}
                if g.closed{ui.label("Diese Gruppe wurde geschlossen. Gespeicherter Verlauf bleibt lokal lesbar.");}
                else if !g.active && !g.departed.contains(&local){
                    ui.label("Einladung oder geänderte Mitgliederliste. Mit dem Beitritt teilst du Nachrichten direkt mit allen aufgeführten Mitgliedern.");
                    ui.horizontal_wrapped(|ui|{if ui.add_enabled(all_known,egui::Button::new("Gruppe beitreten").fill(theme::SOFT_GREEN)).clicked(){self.group_command(GroupCommand::Consent{id:g.roster.id.clone(),accept:true});}
if ui.button("Ablehnen").clicked(){self.group_command(GroupCommand::Consent{id:g.roster.id.clone(),accept:false});}});
                }
                if self.design.group_delete {
                    ui.label("Wirklich verlassen/schliessen? Andere Geräte und Sicherungen behalten ihre Kopien. Die Mitteilung wird erst bei Verbindung zugestellt.");
                    ui.horizontal(|ui|{if ui.button("Bestätigen").clicked(){if g.roster.owner==local{self.group_command(GroupCommand::Close{id:g.roster.id.clone()});}else{self.group_command(GroupCommand::Consent{id:g.roster.id.clone(),accept:false});}self.design.group_delete=false;}
if ui.button("Abbrechen").clicked(){self.design.group_delete=false;}});
                }
            });
            ui.add_space(12.0);
            for m in &self.view.group_messages {
                if m.group!=g.roster.id {continue;}
                ui.push_id((&m.sender,&m.id),|ui|{
                    let own=m.sender==local;
                    egui::Frame::new().fill(if own{theme::OUTGOING}else{theme::SURFACE}).corner_radius(12).inner_margin(12).show(ui,|ui|{
                        ui.set_width((ui.available_width()-24.0).clamp(100.0,560.0));
                        let name=Zeroizing::new(self.group_name(&m.sender));ui.label(RichText::new(name.as_str()).size(11.0).color(MINT));
                        ui.add(egui::Label::new(&m.text).wrap());
                        ui.label(RichText::new(format!("{} · {}",super::timestamp::clock(m.timestamp_ms),if own{format!("{} / {} empfangen · {} beendet · {} ausstehend",m.delivered.len(),m.recipients.len(),m.cancelled.len(),m.recipients.len().saturating_sub(m.delivered.len()+m.cancelled.len()))}else{"Auf diesem PC gespeichert".into()})).size(10.0).color(MUTED));
                    });ui.add_space(6.0);
                });
            }
            if self.view.group_messages.iter().all(|m|m.group!=g.roster.id){ui.add_space(20.0);ui.label(RichText::new("Euer Gespräch beginnt hier.").color(MUTED));}
            if g.unread>0 && ui.ctx().input(|i|i.focused){self.group_command(GroupCommand::Read{id:g.roster.id.clone()});}
        });
    }
    pub(super) fn group_editor(&mut self, ctx: &egui::Context) {
        if !self.view.unlocked || !self.design.group_create {
            return;
        }
        let mut open = true;
        egui::Window::new(if self.design.group_edit.is_some(){"Gruppe bearbeiten"}else{"Neue Gruppe"}).open(&mut open).default_width(430.0).collapsible(false).vscroll(true).max_height((ctx.content_rect().height()-80.0).max(220.0)).show(ctx,|ui|{
            ui.label("Gruppenname");ui.add(egui::TextEdit::singleline(&mut self.design.group_name).char_limit(80));
            ui.add_space(10.0);ui.label("Bestätigte Kontakte auswählen (mit dir höchstens acht):");
            for contact in &self.view.contacts {
                let mut checked=self.design.group_members.contains(&contact.id);
                if ui.checkbox(&mut checked,&contact.name).changed(){if checked{self.design.group_members.push(contact.id.clone());}else{self.design.group_members.retain(|id|id!=&contact.id);}}
            }
            if self.view.contacts.is_empty(){ui.label("Zuerst mit mindestens einem Kontakt verbinden und den Sicherheitscode vergleichen.");}
            ui.add_space(10.0);ui.label(RichText::new("Alle Mitglieder prüfen einander vor dem Beitritt als Kontakte. Neue Listen müssen erneut angenommen werden. Offline-Geräte erfahren Änderungen erst bei Verbindung.").size(12.0).color(MUTED));
            let valid=!self.design.group_name.trim().is_empty() && self.design.group_name.len()<=80 && !self.design.group_members.is_empty() && self.design.group_members.len()<crate::groups::MAX_MEMBERS;
            if ui.add_enabled(valid,egui::Button::new("Einladungen speichern").fill(theme::SOFT_GREEN)).clicked(){
                let command=if let Some(id)=&self.design.group_edit {GroupCommand::Update{id:id.clone(),name:self.design.group_name.trim().into(),members:self.design.group_members.clone()}}else{GroupCommand::Create{name:self.design.group_name.trim().into(),members:self.design.group_members.clone()}};
                if self.group_command(command){self.design.group_create=false;self.design.group_name.zeroize();self.design.group_members.zeroize();self.design.group_edit=None;clear_ui_memory(ctx);}
            }
            self.show_error(ui);
        });
        if !open {
            self.design.group_create = false;
            self.design.group_name.zeroize();
            self.design.group_members.zeroize();
            self.design.group_edit = None;
            clear_ui_memory(ctx);
        }
    }
}
