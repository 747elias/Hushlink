use super::*;
use egui::{Sense, Stroke, vec2};
use theme::{Icon, LINE, TEXT};

impl App {
    pub(super) fn onboarding(&mut self, ui: &mut egui::Ui, width: f32) {
        let step = self.design.onboarding_step.min(2);
        egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE)).corner_radius(22).inner_margin(28).show(ui, |ui| {
            ui.set_width((width - 56.0).max(230.0));
            ui.horizontal(|ui| {
                for n in 0..3 {
                    let (rect, _) = ui.allocate_exact_size(vec2(28.0, 5.0), Sense::hover());
                    ui.painter().rect_filled(rect, 3, if n <= step { MINT } else { LINE });
                }
                ui.label(RichText::new(format!("{} / 3", step + 1)).size(11.0).color(MUTED));
            });
            ui.add_space(20.0);
            let (rect, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 26.0, theme::SOFT_GREEN);
            theme::icon(ui.painter(), rect.shrink(13.0), [Icon::Chat, Icon::Smile, Icon::Lock][step], MINT);
            ui.add_space(16.0);
            ui.label(RichText::new(["Schön, dass du hier bist.", "Wie heisst du?", "Dein Raum. Dein Passwort."][step]).size(25.0).color(TEXT));
            ui.add_space(8.0);
            ui.label(RichText::new([
                "Ein privates Gespräch beginnt mit euch. Richte Hushlink in drei kleinen Schritten ein.",
                "Dieser Name wird deinem Gegenüber angezeigt. Dafür brauchst du weder Konto noch E-Mail-Adresse.",
                "Du entscheidest: keine Mindestlänge, keine Zeichenvorgaben. Auch ein leeres Passwort ist möglich.",
            ][step]).color(MUTED));
            ui.add_space(18.0);
            let mut next = false;
            let mut back = false;
            ui.add_enabled_ui(!self.busy && self.runtime.is_some(), |ui| {
                match step {
                    0 => {
                        for (title, text) in [
                            ("Direkt verbunden", "Zwei PCs in Bluetooth-Reichweite. Chatten ohne Internet oder Server."),
                            ("Deine Daten bleiben bei dir", "Profil, Kontakte, Nachrichten und Dateien liegen in einem verschlüsselten Tresor."),
                            ("Erst vergleichen, dann schreiben", "Vor dem ersten Chat bestätigt ihr den Sicherheitscode auf beiden PCs."),
                        ] {
                            ui.label(RichText::new(title).size(13.0).color(TEXT));
                            ui.label(RichText::new(text).size(12.0).color(MUTED));
                            ui.add_space(5.0);
                        }
                    }
                    1 => {
                        ui.label("Dein Anzeigename");
                        let response = auth::field(ui, &mut self.name, "profile_name", "Wie möchtest du heissen?", false);
                        if self.focus_auth { response.request_focus(); self.focus_auth = false; }
                        ui.add_space(14.0);
                        ui.label(RichText::new("Ein Ort für deine Gespräche").size(13.0));
                        ui.add(egui::Label::new(RichText::new(&self.view.vault_path).size(11.0).color(MUTED)).wrap());
                        if ui.button("Speicherort wählen …").clicked() { self.settings_open = true; }
                    }
                    _ => {
                        ui.label("Dein Passwort");
                        let response = auth::field(ui, &mut self.password, "onboarding_password", "Frei wählen oder leer lassen", !self.design.onboarding_show_password);
                        if self.focus_auth { response.request_focus(); self.focus_auth = false; }
                        ui.add_space(8.0);
                        ui.label("Zur Sicherheit wiederholen");
                        auth::field(ui, &mut self.confirmation, "onboarding_confirmation", "Noch einmal eingeben", !self.design.onboarding_show_password);
                        ui.checkbox(&mut self.design.onboarding_show_password, "Passwort anzeigen");
                        ui.add_space(10.0);
                        ui.label(RichText::new(if self.password.is_empty() {
                            "Ohne Passwort kann jede Person mit der Tresordatei deine Daten öffnen."
                        } else {
                            "Eine lange, einzigartige Passphrase schützt besser. Du bestimmst die Länge."
                        }).size(11.0).color(MUTED));
                        ui.label(RichText::new("Merke dir dein Passwort gut: Es gibt keine Passwortwiederherstellung.").size(11.0).color(MUTED));
                    }
                }
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if step > 0 { back = ui.button("Zurück").clicked(); }
                    next = theme::primary(ui, ["Los geht’s", "Weiter", "Hushlink starten"][step]).clicked();
                });
                if !self.settings_open && !self.rich.restore_open { next |= ui.input_mut(take_plain_enter); }
            });
            if back {
                self.design.onboarding_step = step - 1;
                self.password.zeroize();self.confirmation.zeroize();
                self.design.onboarding_show_password = false;
                self.error.zeroize();self.focus_auth = true;clear_ui_memory(ui.ctx());
            } else if next {
                if step == 1 && (self.name.trim().is_empty() || self.name.trim().len() > crate::protocol::MAX_NAME || self.name.chars().any(char::is_control)) {
                    self.error = "Bitte wähle einen gültigen Anzeigenamen.".into();
                } else if step == 2 {
                    if *self.password != *self.confirmation {
                        self.error = "Passwort und Wiederholung stimmen nicht überein.".into();
                    } else if self.command(Command::Create { name:self.name.trim().into(), password:self.password.clone() }) {
                        self.busy=true;self.password.zeroize();self.confirmation.zeroize();
                        self.design.onboarding_show_password=false;
                        self.design.connections_open=true;
                        clear_ui_memory(ui.ctx());
                    }
                } else {
                    self.design.onboarding_step = step + 1;self.focus_auth=true;self.error.zeroize();clear_ui_memory(ui.ctx());
                }
            }
            if self.busy { ui.spinner();ui.label("Dein privater Raum wird eingerichtet …"); }
            self.show_error(ui);
        });
    }
}
