use super::*;
use egui::{Margin, Stroke};
use theme::{Icon, LINE, TEXT};

impl App {
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        if !self.view.unlocked {
            self.design.settings_section = 0;
        }
        let mut open = true;
        egui::Window::new("Einstellungen")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .min_width(350.0)
            .max_height((ctx.content_rect().height() - 48.0).max(300.0))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Ganz nach deinem Gefühl.")
                        .size(24.0)
                        .color(TEXT),
                );
                ui.add_space(6.0);
                ui.label(RichText::new("Dein Hushlink. Deine Einstellungen.").color(MUTED));
                ui.add_space(18.0);
                ui.horizontal_wrapped(|ui| {
                    for (index, title) in ["Allgemein", "Privatsphäre", "Sicherheit", "Speicher"]
                        .iter()
                        .enumerate()
                    {
                        ui.add_enabled_ui(index == 0 || self.view.unlocked, |ui| {
                            if theme::pill(ui, title, self.design.settings_section == index)
                                .clicked()
                            {
                                self.design.settings_section = index;
                                self.password.zeroize();
                                self.new_password.zeroize();
                                self.new_confirmation.zeroize();
                                clear_ui_memory(ctx);
                            }
                        });
                    }
                });
                ui.add_space(12.0);
                ui.painter().line_segment(
                    [ui.min_rect().left_bottom(), ui.min_rect().right_bottom()],
                    Stroke::new(1.0, LINE),
                );
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .id_salt(("settings-section", self.design.settings_section))
                    .max_height((ctx.content_rect().height() - 390.0).max(96.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        match self.design.settings_section {
                            0 => {
                                self.desktop_settings(ui);
                            }
                            1 => {
                                self.notification_settings(ui);
                                ui.add_space(20.0);
                                self.rich_settings(ui);
                            }
                            2 => self.security_settings(ui),
                            _ => self.storage_settings(ui),
                        }
                        self.show_error(ui);
                        ui.add_space(14.0);
                    });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    theme::icon(ui.painter(), r, Icon::Chat, MUTED);
                    ui.label(
                        RichText::new(format!(
                            "Hushlink {} · Auf deinem PC zuhause",
                            env!("CARGO_PKG_VERSION")
                        ))
                        .size(10.0)
                        .color(MUTED),
                    );
                });
            });
        if !open {
            self.settings_open = false;
            self.password.zeroize();
            self.new_password.zeroize();
            self.new_confirmation.zeroize();
            clear_ui_memory(ctx);
        }
    }
    fn notification_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Benachrichtigungen");
        ui.add_space(6.0);
        let mut enabled = self.view.notifications;
        if ui
            .checkbox(&mut enabled, "Private Windows-Benachrichtigungen")
            .changed()
        {
            self.command(Command::SetNotifications { enabled });
        }
        ui.label(RichText::new("Ohne Namen, Dateinamen oder Textvorschau. Windows kann Hinweise unterdrücken. Zum Empfang im Hintergrund Hushlink entsperrt minimieren; Schliessen ins Tray sperrt und trennt.").size(12.0).color(MUTED));
    }
    fn security_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Automatisch sperren");
        ui.add_space(6.0);
        let mut minutes = self.view.inactivity_minutes.max(1);
        if ui
            .add(egui::Slider::new(&mut minutes, 1..=120).suffix(" Min."))
            .changed()
        {
            self.command(Command::SetInactivity { minutes });
        }
        ui.label(RichText::new("Bei Inaktivität und beim Sperren von Windows werden Gespräche beendet und der Tresor gesperrt.").size(12.0).color(MUTED));
        ui.add_space(22.0);
        egui::Frame::new().fill(theme::INPUT).corner_radius(12).inner_margin(Margin::same(16)).show(ui,|ui|{
            ui.heading("Windows Hello");ui.add_space(6.0);ui.label(if self.view.hello_enabled{"Für diesen Tresor eingerichtet"}else{"Optional mit PIN oder Biometrie öffnen"});
            ui.label(RichText::new("Benötigt einen geeigneten Windows-Hello-Schlüsselanbieter mit erneuter Bestätigung. Das Passwort bleibt dein Rückweg, auch auf anderen PCs und für Sicherungen.").size(12.0).color(MUTED));
            ui.add_space(10.0);ui.label(RichText::new("Aktuelles Passwort").size(12.0).color(MUTED));ui.add(egui::TextEdit::singleline(&mut *self.password).password(true).id_salt("hello_password").desired_width(ui.available_width()));
            if ui.add_enabled(!self.busy && (cfg!(windows) || self.view.hello_enabled),egui::Button::new(if self.view.hello_enabled{"Windows Hello deaktivieren"}else{"Windows Hello einrichten"})).clicked() && self.command(Command::SetHello{enabled:!self.view.hello_enabled,password:self.password.clone()}){self.password.zeroize();self.busy=true;clear_ui_memory(ui.ctx());}
        });
        ui.add_space(22.0);
        ui.heading("Passwort ändern");
        ui.label(
            RichText::new("Dein Verlauf bleibt erhalten.")
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(8.0);
        for (label, id, value) in [
            ("Bisheriges Passwort", "old_password", &mut *self.password),
            ("Neues Passwort", "new_password", &mut *self.new_password),
            (
                "Neues Passwort wiederholen",
                "new_confirmation",
                &mut *self.new_confirmation,
            ),
        ] {
            ui.label(RichText::new(label).size(12.0).color(MUTED));
            ui.add(
                egui::TextEdit::singleline(value)
                    .password(true)
                    .id_salt(id)
                    .desired_width(ui.available_width()),
            );
            ui.add_space(6.0);
        }
        if ui
            .add_enabled(
                !self.busy,
                egui::Button::new("Passwort ändern").fill(theme::SOFT_GREEN),
            )
            .clicked()
        {
            if *self.new_password != *self.new_confirmation {
                self.error = "Das neue Passwort und die Bestätigung stimmen nicht überein.".into();
            } else if self.command(Command::ChangePassword {
                old: self.password.clone(),
                new: self.new_password.clone(),
            }) {
                self.password.zeroize();
                self.new_password.zeroize();
                self.new_confirmation.zeroize();
                clear_ui_memory(ui.ctx());
            }
        }
    }
}
