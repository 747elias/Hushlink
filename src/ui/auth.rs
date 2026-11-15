use super::*;
use egui::{Align, FontId, Margin, Rect, Sense, Stroke, vec2};
use theme::{INPUT, Icon, LINE, SURFACE, TEXT};

impl App {
    pub(super) fn auth_screen(&mut self, ui: &mut egui::Ui) {
        if self.runtime.is_some() && self.view.vault_path.is_empty() {
            ui.spinner();
            ui.label("Dein Profil wird gesucht …");
            self.show_error(ui);
            return;
        }
        let wide = ui.available_width() > 860.0;
        let width = ui.available_width();
        ui.add_space(if wide {
            (ui.available_height() * 0.07).clamp(8.0, 44.0)
        } else {
            12.0
        });
        if wide {
            let card_width = 392.0;
            let hero_width = (width - card_width - 90.0).max(280.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(hero_width, 0.0),
                    egui::Layout::top_down(Align::LEFT),
                    |ui| self.auth_intro(ui),
                );
                ui.add_space(60.0);
                ui.allocate_ui_with_layout(
                    vec2(card_width, 0.0),
                    egui::Layout::top_down(Align::LEFT),
                    |ui| self.auth_card(ui, card_width),
                );
            });
        } else {
            ui.vertical_centered(|ui| {
                ui.set_max_width(408.0);
                brand(ui);
                ui.add_space(24.0);
                self.auth_card(ui, ui.available_width());
            });
        }
    }
    fn auth_intro(&self, ui: &mut egui::Ui) {
        brand(ui);
        ui.add_space(54.0);
        ui.label(
            RichText::new("Dein Gespräch.\nEuer eigener Raum.")
                .size(42.0)
                .color(TEXT),
        );
        ui.add_space(16.0);
        ui.add(
            egui::Label::new(
                RichText::new("Direkt von PC zu PC. Ohne Konto,\nohne Cloud und ohne Umwege.")
                    .size(17.0)
                    .color(MUTED),
            )
            .wrap(),
        );
        ui.add_space(28.0);
        let (r, _) =
            ui.allocate_exact_size(vec2(ui.available_width().min(390.0), 190.0), Sense::hover());
        let center = r.center();
        ui.painter()
            .circle_filled(center, 80.0, Color32::from_rgb(15, 39, 36));
        ui.painter()
            .circle_stroke(center, 95.0, Stroke::new(1.0, LINE));
        let first = Rect::from_center_size(center + vec2(-24.0, -18.0), vec2(154.0, 74.0));
        let second = Rect::from_center_size(center + vec2(48.0, 38.0), vec2(116.0, 58.0));
        ui.painter().rect_filled(first, 18, SURFACE);
        ui.painter().rect_filled(second, 16, theme::OUTGOING);
        for (row, factor) in [(0, 0.70), (1, 0.48)] {
            let a = first.left_top() + vec2(22.0, 26.0 + row as f32 * 18.0);
            ui.painter().line_segment(
                [a, a + vec2(first.width() * factor, 0.0)],
                Stroke::new(4.0, Color32::from_rgb(68, 89, 95)),
            );
        }
        theme::icon(
            ui.painter(),
            Rect::from_center_size(second.center(), vec2(29.0, 29.0)),
            Icon::Lock,
            MINT,
        );
        for (angle, radius) in [(0.1_f32, 4.0), (2.5, 5.0), (4.4, 3.0)] {
            let p = center + vec2(angle.cos() * 95.0, angle.sin() * 95.0);
            ui.painter()
                .circle_filled(p, radius, MINT.gamma_multiply(0.5));
        }
        ui.add_space(22.0);
        ui.horizontal_wrapped(|ui| {
            theme::badge(ui, Icon::Bluetooth, "Bluetooth", MUTED);
            theme::badge(ui, Icon::Shield, "Ende-zu-Ende verschlüsselt", MUTED);
        });
        ui.add_space(24.0);
        ui.label(
            RichText::new("Deine Daten gehören auf deinen PC.")
                .size(12.0)
                .color(MUTED),
        );
    }
    fn auth_card(&mut self, ui: &mut egui::Ui, width: f32) {
        if !self.view.profile_exists {
            self.onboarding(ui, width);
            self.auth_links(ui);
            return;
        }
        egui::Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(22)
            .inner_margin(28)
            .show(ui, |ui| {
                ui.set_width((width - 56.0).max(230.0));
                ui.heading("Schön, dass du da bist.");
                ui.label(
                    RichText::new("Entsperre deinen Tresor, um deine Gespräche zu öffnen.")
                        .color(MUTED),
                );
                ui.add_space(18.0);
                ui.add_enabled_ui(!self.busy && self.runtime.is_some(), |ui| {
                    ui.label("Dein Passwort");
                    let response = field(
                        ui,
                        &mut self.password,
                        "unlock_password",
                        "Passwort eingeben",
                        true,
                    );
                    if self.focus_auth {
                        response.request_focus();
                        self.focus_auth = false;
                    }
                    ui.add_space(12.0);
                    let enter = !self.rich.restore_open
                        && !self.settings_open
                        && ui.input_mut(take_plain_enter);
                    if (theme::primary(ui, "Tresor entsperren").clicked() || enter)
                        && self.command(Command::Unlock {
                            password: self.password.clone(),
                        })
                    {
                        self.busy = true;
                        self.password.zeroize();
                        clear_ui_memory(ui.ctx());
                    }
                    if cfg!(windows)
                        && ui.button("Mit Windows Hello öffnen").clicked()
                        && self.command(Command::UnlockHello)
                    {
                        self.password.zeroize();
                        self.busy = true;
                        clear_ui_memory(ui.ctx());
                    }
                });
                if self.busy {
                    ui.spinner();
                    ui.label("Dein Tresor wird geöffnet …");
                }
                self.show_error(ui);
            });
        self.auth_links(ui);
    }
    fn auth_links(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Speicherort & Einstellungen")
                            .size(12.0)
                            .color(MUTED),
                    )
                    .frame(false),
                )
                .clicked()
            {
                self.settings_open = true;
            }
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Verschlüsselte Sicherung wiederherstellen")
                            .size(11.0)
                            .color(MUTED),
                    )
                    .frame(false),
                )
                .clicked()
            {
                self.rich.restore_open = true;
            }
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(
                    RichText::new(&self.view.vault_path)
                        .size(9.0)
                        .color(MUTED.gamma_multiply(0.7)),
                )
                .truncate(),
            );
        });
    }
}
pub(super) fn field(
    ui: &mut egui::Ui,
    value: &mut String,
    id: &str,
    hint: &str,
    password: bool,
) -> egui::Response {
    egui::Frame::new()
        .fill(INPUT)
        .corner_radius(10)
        .inner_margin(Margin::symmetric(13, 12))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .id_salt(id)
                    .password(password)
                    .char_limit(if id == "profile_name" { 64 } else { usize::MAX })
                    .frame(false)
                    .hint_text(hint)
                    .desired_width(ui.available_width())
                    .font(FontId::proportional(14.0)),
            )
        })
        .inner
}
fn brand(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
        ui.painter().rect_filled(r, 11, MINT);
        theme::icon(
            ui.painter(),
            r.shrink(8.0),
            Icon::Chat,
            Color32::from_rgb(6, 45, 33),
        );
        ui.label(RichText::new("Hushlink").size(24.0).color(TEXT));
    });
}
