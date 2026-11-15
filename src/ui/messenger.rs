use super::*;
use crate::runtime::{ContactView, MessageView};
use egui::{Align, Align2, FontId, Margin, Rect, Sense, Stroke, pos2, vec2};
use theme::{INPUT, Icon, SURFACE, TEXT};

#[derive(Default, Zeroize, zeroize::ZeroizeOnDrop)]
pub(super) struct Presentation {
    pub contact_query: String,
    pub unread_only: bool,
    pub favorites_only: bool,
    pub connections_open: bool,
    pub chat_search: bool,
    pub emoji_open: bool,
    pub settings_section: usize,
}

impl App {
    pub(super) fn navigation(&mut self, ui: &mut egui::Ui) {
        ui.add_space(14.0);
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            ui.painter().rect_filled(r, 13, MINT);
            theme::icon(
                ui.painter(),
                r.shrink(9.0),
                Icon::Chat,
                Color32::from_rgb(6, 45, 33),
            );
            ui.add_space(24.0);
            if theme::icon_button(ui, Icon::Chat, "Chats", !self.design.connections_open).clicked()
            {
                self.design.connections_open = false;
            }
            ui.add_space(8.0);
            if theme::icon_button(
                ui,
                Icon::Bluetooth,
                "Neue Verbindung",
                self.design.connections_open,
            )
            .clicked()
            {
                self.design.connections_open = !self.design.connections_open;
            }
            ui.add_space(8.0);
            if theme::icon_button(
                ui,
                Icon::File,
                "Dateiübertragungen",
                self.integration.transfers_open,
            )
            .clicked()
            {
                self.integration.transfers_open = true;
            }
        });
        ui.with_layout(egui::Layout::bottom_up(Align::Center), |ui| {
            ui.add_space(14.0);
            let (r, response) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::click());
            theme::avatar(
                ui.painter(),
                r.center(),
                18.0,
                &self.view.profile_name,
                &self.view.profile_name,
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    "Dein Profil und Einstellungen",
                )
            });
            if response
                .on_hover_text("Dein Profil und Einstellungen")
                .clicked()
            {
                self.settings_open = true;
            }
            ui.add_space(8.0);
            if theme::icon_button(ui, Icon::Lock, "Sperren · Strg+L", false).clicked() {
                self.lock(ui.ctx());
            }
            ui.add_space(8.0);
            if theme::icon_button(ui, Icon::Settings, "Einstellungen", self.settings_open).clicked()
            {
                self.settings_open = true;
            }
        });
    }
    pub(super) fn conversation_list(&mut self, ui: &mut egui::Ui) {
        if self.design.connections_open {
            self.connection_drawer(ui);
            return;
        }
        egui::TopBottomPanel::bottom("contacts_footer")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(20, 16)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
                    theme::icon(ui.painter(), r, Icon::Lock, MUTED);
                    ui.label(
                        RichText::new("Deine Gespräche bleiben bei dir.")
                            .size(11.0)
                            .color(MUTED),
                    );
                });
            });
        egui::TopBottomPanel::top("contacts_header")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(20, 16)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Chats").size(27.0).color(TEXT));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if theme::icon_button(ui, Icon::Plus, "Neue Verbindung", false).clicked() {
                            self.design.connections_open = true;
                        }
                    });
                });
                ui.add_space(12.0);
                egui::Frame::new()
                    .fill(INPUT)
                    .corner_radius(12)
                    .inner_margin(Margin::symmetric(12, 9))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(17.0, 17.0), Sense::hover());
                            theme::icon(ui.painter(), r, Icon::Search, MUTED);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.design.contact_query)
                                    .id_salt("contact-search")
                                    .frame(false)
                                    .hint_text("Suchen")
                                    .desired_width(ui.available_width())
                                    .char_limit(128),
                            );
                        });
                    });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if theme::pill(
                        ui,
                        "Alle",
                        !self.design.unread_only && !self.design.favorites_only,
                    )
                    .clicked()
                    {
                        self.design.unread_only = false;
                        self.design.favorites_only = false;
                    }
                    if theme::pill(ui, "Ungelesen", self.design.unread_only).clicked() {
                        self.design.unread_only = !self.design.unread_only;
                        self.design.favorites_only = false;
                    }
                    if theme::pill(ui, "Favoriten", self.design.favorites_only).clicked() {
                        self.design.favorites_only = !self.design.favorites_only;
                        self.design.unread_only = false;
                    }
                });
            });
        let mut selected = None;
        let mut manage = None;
        let mut connect = None;
        let query = Zeroizing::new(self.design.contact_query.to_lowercase());
        let latest: std::collections::HashMap<&str, &MessageView> = self
            .view
            .messages
            .iter()
            .map(|m| (m.contact_id.as_str(), m))
            .collect();
        let mut shown = 0;
        egui::ScrollArea::vertical().id_salt("messenger_contacts").auto_shrink([false,false]).show(ui,|ui|{
            ui.spacing_mut().item_spacing.y=2.0;
            for contact in &self.view.contacts{
                if (self.design.unread_only && contact.unread==0) || (self.design.favorites_only && !contact.favorite) || (!query.is_empty() && !Zeroizing::new(contact.name.to_lowercase()).contains(query.as_str())){continue;}
                shown+=1;
                let last=latest.get(contact.id.as_str()).copied();
                let is_selected=self.selected.as_deref()==Some(&contact.id);
                let response=contact_row(ui,contact,last,is_selected,self.view.peer.as_ref().is_some_and(|p|p.ready && p.id==contact.id));
                if response.clicked(){selected=Some(contact.id.clone());}
                response.context_menu(|ui|{
                    if ui.button("Kontakt verwalten").clicked(){manage=Some(contact.id.clone());ui.close();}
                    if ui.add_enabled(self.view.peer.is_none() && !contact.address.is_empty(),egui::Button::new("Erneut verbinden")).clicked(){connect=Some(contact.address.clone());ui.close();}
                });
            }
            if shown==0{
                egui::Frame::new().inner_margin(24).show(ui,|ui|{
                    ui.add_space(32.0);ui.label(RichText::new(if self.view.contacts.is_empty(){"Euer nächstes Gespräch beginnt hier."}else{"Keine passenden Chats"}).size(17.0).color(TEXT));
                    ui.add_space(8.0);ui.label(RichText::new(if self.view.contacts.is_empty(){"Verbinde dich mit einem PC in deiner Nähe. Bestätigte Kontakte erscheinen hier."}else{"Versuche einen anderen Namen oder Filter."}).color(MUTED));
                    if self.view.contacts.is_empty(){ui.add_space(12.0);if theme::primary(ui,"Neue Verbindung").clicked(){self.design.connections_open=true;}}
                });
            }
        });
        if let Some(id) = selected {
            self.selected = Some(id);
            self.delete_requested = false;
        }
        if let Some(id) = manage {
            self.open_contact(&id);
        }
        if let Some(address) = connect {
            self.command(Command::Connect { address });
        }
    }
    fn connection_drawer(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new().inner_margin(Margin::symmetric(20,18)).show(ui,|ui|{
            ui.horizontal(|ui|{if theme::icon_button(ui,Icon::Back,"Zurück zu Chats",false).clicked(){self.design.connections_open=false;}ui.label(RichText::new("Neue Verbindung").size(20.0));});
            ui.add_space(12.0);ui.label(RichText::new("Ein Gespräch.\nDirekt zwischen euch.").size(25.0));ui.add_space(8.0);
            ui.label(RichText::new("Hushlink verbindet zwei PCs in Bluetooth-Reichweite.").color(MUTED));ui.add_space(20.0);
            egui::ScrollArea::vertical().id_salt("connection_drawer_scroll").show(ui,|ui|{
                egui::Frame::new().fill(SURFACE).corner_radius(14).inner_margin(16).show(ui,|ui|{
                    ui.horizontal(|ui|{let(r,_)=ui.allocate_exact_size(vec2(26.0,26.0),Sense::hover());theme::icon(ui.painter(),r,Icon::Bluetooth,if self.view.bluetooth_available{MINT}else{MUTED});ui.label(RichText::new(if !self.view.bluetooth_checked{"Bluetooth wird geprüft"}else if self.view.bluetooth_available{"Bluetooth verfügbar"}else{"Bluetooth nicht bereit"}).size(14.0));});
                    ui.add_space(6.0);ui.label(RichText::new(&self.view.availability).size(12.0).color(MUTED));
                    if ui.small_button("Windows-Einstellungen öffnen").clicked() && crate::transport::open_bluetooth_settings().is_err(){self.error="Öffne Windows → Einstellungen → Bluetooth und Geräte.".into();}
                });
                ui.add_space(14.0);
                if theme::primary(ui,if self.view.listening{"Empfang beenden"}else{"Verbindung empfangen"}).clicked(){self.command(if self.view.listening{Command::StopListening}else{Command::StartListening});}
                if self.view.listening{ui.label(RichText::new("Empfang aktiv · Andere PCs können anfragen").size(12.0).color(MINT));}
                ui.add_space(4.0);
                if ui.add_enabled(!self.view.discovering,egui::Button::new(if self.view.discovering{"Geräte werden gesucht …"}else{"Geräte in der Nähe suchen"}).min_size(vec2(ui.available_width().min(260.0),40.0))).clicked(){self.command(Command::Discover);}
                if self.view.discovering{ui.spinner();}
                ui.add_space(10.0);
                for device in self.view.devices.clone(){egui::Frame::new().fill(INPUT).corner_radius(12).inner_margin(12).show(ui,|ui|{
                    ui.label(&device.name);ui.label(RichText::new(&device.address).size(11.0).color(MUTED));
                    if ui.add_enabled(self.view.peer.is_none() && !self.view.connecting,egui::Button::new("Verbinden")).clicked(){self.command(Command::Connect{address:device.address.clone()});}
                });}
                ui.add_space(12.0);ui.label(RichText::new(&self.view.status).size(12.0).color(MUTED));self.show_error(ui);
                ui.add_space(12.0);if ui.button("Verbindungsassistent öffnen").clicked(){self.features.wizard=true;}
                if (self.view.discovering || self.view.listening || self.view.connecting) && ui.button("Vorgänge beenden").clicked(){self.command(Command::CancelBluetooth);}
                ui.add_space(16.0);ui.label(RichText::new("Auf dem anderen PC Hushlink öffnen. Bei Bedarf beide Geräte in Windows koppeln. Ein Gerätename ersetzt keine Identitätsprüfung.").size(11.0).color(MUTED));
            });
        });
    }
    pub(super) fn messenger(&mut self, ui: &mut egui::Ui) {
        self.rich.sync(self.selected.as_deref());
        self.sync_media();
        if self.connection_safety(ui) {
            return;
        }
        let ready = self
            .view
            .peer
            .as_ref()
            .is_some_and(|p| p.ready && self.selected.as_deref() == Some(&p.id));
        egui::TopBottomPanel::top("chat_header")
            .show_separator_line(false)
            .exact_height(76.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(Margin::symmetric(22, 12)),
            )
            .show_inside(ui, |ui| self.conversation_header(ui, ready));
        if self.selected.is_none() && !self.rich.all_contacts {
            theme::wallpaper(ui.painter(), ui.available_rect_before_wrap());
            self.empty_conversation(ui, false);
            return;
        }
        egui::TopBottomPanel::bottom("message_composer")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(Margin::symmetric(18, 12)),
            )
            .show_inside(ui, |ui| self.message_composer(ui, ready));
        if self.design.chat_search || !self.rich.query.is_empty() || self.rich.pinned_only {
            egui::TopBottomPanel::top("conversation_search")
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(PANEL)
                        .inner_margin(Margin::symmetric(20, 10)),
                )
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        theme::icon(ui.painter(), r, Icon::Search, MUTED);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.rich.query)
                                .id_salt("history-search")
                                .hint_text("In Nachrichten suchen")
                                .desired_width((ui.available_width() - 42.0).max(80.0))
                                .frame(false)
                                .char_limit(256),
                        );
                        if theme::icon_button(ui, Icon::Close, "Suche schliessen", false).clicked()
                        {
                            self.rich.query.zeroize();
                            self.design.chat_search = false;
                            self.rich.pinned_only = false;
                            self.rich.all_contacts = false;
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        if theme::pill(ui, "Alle Chats", self.rich.all_contacts).clicked() {
                            self.rich.all_contacts = !self.rich.all_contacts;
                        }
                        if theme::pill(ui, "Gemerkte", self.rich.pinned_only).clicked() {
                            self.rich.pinned_only = !self.rich.pinned_only;
                        }
                    });
                });
        }
        theme::wallpaper(ui.painter(), ui.available_rect_before_wrap());
        egui::Frame::new()
            .inner_margin(Margin::symmetric(24, 16))
            .show(ui, |ui| {
                self.show_error(ui);
                if self.delete_requested {
                    self.delete_history_prompt(ui);
                }
                self.pending_changes(ui, ready);
                self.message_history(ui, ready);
            });
    }
    fn conversation_header(&mut self, ui: &mut egui::Ui, ready: bool) {
        let contact = self
            .view
            .contacts
            .iter()
            .find(|c| self.selected.as_deref() == Some(&c.id))
            .cloned();
        let name = Zeroizing::new(
            contact
                .as_ref()
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Hushlink".into()),
        );
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
            if let Some(c) = &contact {
                theme::avatar(ui.painter(), r.center(), 21.0, &c.name, &c.id);
            } else {
                ui.painter()
                    .circle_filled(r.center(), 21.0, theme::SOFT_GREEN);
                theme::icon(ui.painter(), r.shrink(10.0), Icon::Chat, MINT);
            }
            ui.allocate_ui_with_layout(
                vec2((ui.available_width() - 124.0).max(40.0), 44.0),
                egui::Layout::top_down(Align::LEFT),
                |ui| {
                    ui.add(egui::Label::new(RichText::new(name.as_str()).size(17.0)).truncate());
                    ui.horizontal(|ui| {
                        if ready {
                            let (r, _) = ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 3.0, MINT);
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new(if self.view.peer_typing && ready {
                                    "schreibt …"
                                } else if ready {
                                    "Sicher verbunden"
                                } else if contact.is_some() {
                                    "Nicht verbunden · Verlauf auf diesem PC"
                                } else {
                                    "Privat. Lokal. Direkt."
                                })
                                .size(11.0)
                                .color(if ready {
                                    MINT
                                } else {
                                    MUTED
                                }),
                            )
                            .truncate(),
                        );
                    });
                },
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                theme::icon_menu(ui, Icon::More, "Chat-Menü", |ui| {
                    if let Some(c) = &contact {
                        if ui.button("Kontakt verwalten").clicked() {
                            self.open_contact(&c.id);
                            ui.close();
                        }
                        if self.view.peer.is_none() && ui.button("Erneut verbinden").clicked() {
                            self.command(Command::Connect {
                                address: c.address.clone(),
                            });
                            ui.close();
                        }
                        if ui.button("Verlauf lokal löschen …").clicked() {
                            self.delete_requested = true;
                            ui.close();
                        }
                    }
                    if ui.button("Dateiübertragungen").clicked() {
                        self.integration.transfers_open = true;
                        ui.close();
                    }
                    if self.view.peer.is_some() && ui.button("Verbindung trennen").clicked() {
                        self.command(Command::Disconnect);
                        ui.close();
                    }
                });
                if theme::icon_button(
                    ui,
                    Icon::Search,
                    "Nachrichten durchsuchen",
                    self.design.chat_search,
                )
                .clicked()
                {
                    self.design.chat_search = !self.design.chat_search;
                }
                if !ready
                    && let Some(c) = &contact
                    && theme::icon_button(ui, Icon::Link, "Erneut verbinden", false).clicked()
                {
                    self.command(Command::Connect {
                        address: c.address.clone(),
                    });
                }
            });
        });
    }
    fn message_history(&mut self, ui: &mut egui::Ui, ready: bool) {
        let mut commands = Vec::new();
        let mut export = None;
        let mut retry = None;
        let mut selected = None;
        let mut found = false;
        let mut day = None;
        egui::ScrollArea::vertical()
            .id_salt(("messages", self.selected.as_deref()))
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .max_height(ui.available_height())
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 5.0;
                for message in &self.view.messages {
                    if (!self.rich.all_contacts
                        && self.selected.as_deref() != Some(&message.contact_id))
                        || !self.rich.matches(message)
                    {
                        continue;
                    }
                    found = true;
                    let message_day = message.timestamp_ms / 86_400_000;
                    if day != Some(message_day) {
                        day = Some(message_day);
                        ui.add_space(12.0);
                        ui.vertical_centered(|ui| {
                            egui::Frame::new()
                                .fill(SURFACE)
                                .corner_radius(8)
                                .inner_margin(Margin::symmetric(13, 6))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(day_label(message.timestamp_ms))
                                            .size(10.0)
                                            .color(MUTED),
                                    );
                                });
                        });
                        ui.add_space(10.0);
                    }
                    let max_width = (ui.available_width() * 0.78).clamp(130.0, 560.0);
                    let galley = ui.painter().layout(
                        message.text.clone(),
                        FontId::proportional(14.0),
                        TEXT,
                        max_width - 26.0,
                    );
                    let content_width = (galley.size().x + 4.0)
                        .max(if message.attachment_id.is_some() {
                            210.0
                        } else if message.reply_to.is_some() {
                            190.0
                        } else {
                            112.0
                        })
                        .min(max_width - 26.0);
                    let layout = if message.outgoing {
                        egui::Layout::right_to_left(Align::TOP)
                    } else {
                        egui::Layout::left_to_right(Align::TOP)
                    };
                    ui.with_layout(layout, |ui| {
                        let radius = egui::CornerRadius {
                            nw: if message.outgoing { 12 } else { 3 },
                            ne: if message.outgoing { 3 } else { 12 },
                            sw: 12,
                            se: 12,
                        };
                        egui::Frame::new()
                            .fill(if message.outgoing {
                                theme::OUTGOING
                            } else {
                                SURFACE
                            })
                            .corner_radius(radius)
                            .inner_margin(Margin::symmetric(12, 9))
                            .show(ui, |ui| {
                                ui.set_width(content_width);
                                ui.with_layout(egui::Layout::top_down(Align::LEFT), |ui| {
                                    if self.rich.all_contacts
                                        && let Some(c) = self
                                            .view
                                            .contacts
                                            .iter()
                                            .find(|c| c.id == message.contact_id)
                                        && ui
                                            .small_button(RichText::new(&c.name).color(MINT))
                                            .clicked()
                                    {
                                        selected = Some(c.id.clone());
                                    }
                                    if let Some(id) = &message.reply_to {
                                        let quoted = self.view.messages.iter().find(|m| {
                                            m.id == *id && m.contact_id == message.contact_id
                                        });
                                        egui::Frame::new()
                                            .fill(Color32::from_black_alpha(23))
                                            .corner_radius(6)
                                            .inner_margin(Margin::symmetric(10, 7))
                                            .show(ui, |ui| {
                                                ui.set_width((content_width - 20.0).max(70.0));
                                                ui.label(
                                                    RichText::new("Antwort").size(11.0).color(MINT),
                                                );
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(quoted.map_or(
                                                            "Nicht gespeicherte Nachricht",
                                                            |m| m.text.as_str(),
                                                        ))
                                                        .size(12.0)
                                                        .color(MUTED),
                                                    )
                                                    .truncate(),
                                                );
                                                let r = ui.min_rect();
                                                ui.painter().line_segment(
                                                    [r.left_top(), r.left_bottom()],
                                                    Stroke::new(2.5, MINT),
                                                );
                                            });
                                    }
                                    if message.attachment_id.is_some() {
                                        ui.horizontal_top(|ui| {
                                            let (r, _) = ui.allocate_exact_size(
                                                vec2(34.0, 40.0),
                                                Sense::hover(),
                                            );
                                            theme::icon(
                                                ui.painter(),
                                                r.shrink(6.0),
                                                if message
                                                    .attachment_name
                                                    .as_deref()
                                                    .is_some_and(|n| n.ends_with(".png"))
                                                {
                                                    Icon::Image
                                                } else {
                                                    Icon::File
                                                },
                                                MINT,
                                            );
                                            ui.vertical(|ui| {
                                                ui.add(egui::Label::new(&message.text).wrap());
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{:.1} KiB · Anhang",
                                                        message.attachment_size as f64 / 1024.0
                                                    ))
                                                    .size(10.0)
                                                    .color(MUTED),
                                                );
                                            });
                                        });
                                        if message.status == MessageStatus::Received
                                            && theme::icon_button(
                                                ui,
                                                Icon::Download,
                                                "Anhang speichern unter …",
                                                false,
                                            )
                                            .clicked()
                                        {
                                            export = message.attachment_id.clone();
                                        }
                                    } else {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&message.text).size(14.0),
                                            )
                                            .wrap(),
                                        );
                                    }
                                    if message.can_resume
                                        && ui
                                            .add_enabled(
                                                ready,
                                                egui::Button::new("Datei fortsetzen"),
                                            )
                                            .clicked()
                                    {
                                        retry = Some(message.id.clone());
                                    }
                                    if let Some(command) = self.rich.controls(
                                        ui,
                                        message,
                                        ready
                                            && self.selected.as_deref()
                                                == Some(&message.contact_id),
                                    ) {
                                        commands.push(command);
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(Align::Center),
                                        |ui| {
                                            if message.outgoing {
                                                let (r, response) = ui.allocate_exact_size(
                                                    vec2(16.0, 14.0),
                                                    Sense::hover(),
                                                );
                                                let (kind, color, label) = delivery(message.status);
                                                theme::icon(ui.painter(), r, kind, color);
                                                response.on_hover_text(label);
                                            }
                                            ui.label(
                                                RichText::new(bubble_time(message.timestamp_ms))
                                                    .size(10.0)
                                                    .color(MUTED),
                                            )
                                            .on_hover_text("Zeit in UTC");
                                            if message.edited {
                                                ui.label(
                                                    RichText::new("bearbeitet")
                                                        .size(10.0)
                                                        .color(MUTED),
                                                );
                                            }
                                            if message.pinned {
                                                let (r, _) = ui.allocate_exact_size(
                                                    vec2(11.0, 13.0),
                                                    Sense::hover(),
                                                );
                                                theme::icon(ui.painter(), r, Icon::Pin, MUTED);
                                            }
                                        },
                                    );
                                });
                            });
                    });
                }
                if !found {
                    self.empty_history(ui);
                }
                ui.add_space(12.0);
            });
        for command in commands {
            self.command(command);
        }
        if let Some(id) = selected {
            self.selected = Some(id);
            self.rich.all_contacts = false;
        }
        if let Some(id) = export {
            self.pick(Picker::Export { id }, ui.ctx());
        }
        if let Some(id) = retry {
            self.command(Command::RetryFile { id });
            self.integration.transfers_open = true;
        }
    }
    fn message_composer(&mut self, ui: &mut egui::Ui, ready: bool) {
        self.image_offer(ui);
        if let Some(target) = self.rich.reply.clone() {
            if let Some(message) = self
                .view
                .messages
                .iter()
                .find(|m| m.id == target && self.selected.as_deref() == Some(&m.contact_id))
            {
                egui::Frame::new()
                    .fill(INPUT)
                    .corner_radius(10)
                    .inner_margin(10)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Antwort").color(MINT).size(11.0));
                                ui.add(egui::Label::new(&message.text).truncate());
                            });
                            if theme::icon_button(ui, Icon::Close, "Antwort abbrechen", false)
                                .clicked()
                            {
                                self.rich.reply = None;
                            }
                        });
                    });
            } else {
                self.rich.reply = None;
            }
        }
        if self.design.emoji_open && ready {
            ui.horizontal_wrapped(|ui| {
                for emoji in ["🙂", "👍", "❤️", "😂", "🎉", "🦀", "🙌", "✨"] {
                    if ui.button(RichText::new(emoji).size(20.0)).clicked() {
                        self.composer.push_str(emoji);
                        self.design.emoji_open = false;
                    }
                }
            });
        }
        let mut send = false;
        let mut editor = None;
        ui.add_enabled_ui(ready, |ui| {
            ui.horizontal(|ui| {
                if theme::icon_button(ui, Icon::Smile, "Emoji einfügen", self.design.emoji_open)
                    .clicked()
                {
                    self.design.emoji_open = !self.design.emoji_open;
                }
                theme::icon_menu(ui, Icon::Attach, "Anhängen", |ui| {
                    if ui.button("Datei vom PC …").clicked() {
                        self.pick(Picker::Send, ui.ctx());
                        ui.close();
                    }
                    self.clipboard_button(ui, ready);
                });
                let width = (ui.available_width() - 54.0).max(80.0);
                let response = ui
                    .allocate_ui_with_layout(
                        vec2(width, 42.0),
                        egui::Layout::top_down(Align::LEFT),
                        |ui| {
                            egui::Frame::new()
                                .fill(INPUT)
                                .corner_radius(14)
                                .inner_margin(Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.set_width((width - 28.0).max(50.0));
                                    egui::ScrollArea::vertical()
                                        .id_salt("composer-scroll")
                                        .max_height(94.0)
                                        .auto_shrink([false, true])
                                        .show(ui, |ui| {
                                            ui.add(
                                                egui::TextEdit::multiline(&mut *self.composer)
                                                    .id_salt("composer")
                                                    .frame(false)
                                                    .desired_width(ui.available_width())
                                                    .desired_rows(1)
                                                    .hint_text(if ready {
                                                        "Nachricht schreiben"
                                                    } else {
                                                        "Zum Schreiben verbinden"
                                                    })
                                                    .return_key(Some(egui::KeyboardShortcut::new(
                                                        egui::Modifiers::SHIFT,
                                                        egui::Key::Enter,
                                                    ))),
                                            )
                                        })
                                        .inner
                                })
                                .inner
                        },
                    )
                    .inner;
                if response.changed() && self.view.share_typing {
                    self.command(Command::Typing {
                        active: !self.composer.is_empty(),
                    });
                }
                self.clipboard_paste(ui.ctx(), response.has_focus());
                send = response.has_focus() && ui.input_mut(take_plain_enter);
                let valid = !self.composer.trim().is_empty() && self.composer.len() <= MAX_TEXT;
                ui.add_enabled_ui(valid, |ui| {
                    let r = ui.add_sized(
                        [42.0, 42.0],
                        egui::Button::new("").fill(MINT).corner_radius(21),
                    );
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            valid,
                            "Nachricht senden",
                        )
                    });
                    theme::icon(
                        ui.painter(),
                        r.rect.shrink(11.0),
                        Icon::Send,
                        Color32::from_rgb(6, 45, 33),
                    );
                    if r.on_hover_text("Senden · Enter").clicked() {
                        send = true;
                    }
                });
                editor = Some(response);
            });
        });
        if ready
            && send
            && !self.composer.trim().is_empty()
            && self.composer.len() <= MAX_TEXT
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
            if let Some(editor) = editor {
                editor.request_focus();
            }
        }
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            if self.composer.len() > MAX_TEXT {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    "Die Nachricht ist zu lang. Bitte teile sie auf.",
                );
            } else {
                ui.label(
                    RichText::new(if ready {
                        "Enter sendet · Shift+Enter neue Zeile"
                    } else {
                        "Verbinde dich mit diesem Kontakt, um zu schreiben."
                    })
                    .size(10.0)
                    .color(MUTED),
                );
            }
        });
    }
    fn empty_history(&self, ui: &mut egui::Ui) {
        ui.add_space(36.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(if !self.rich.query.is_empty() || self.rich.pinned_only {
                    "Keine passenden Nachrichten"
                } else {
                    "Das erste Wort gehört dir."
                })
                .size(20.0),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(if !self.rich.query.is_empty() || self.rich.pinned_only {
                    "Ändere deine Suche oder die Filter."
                } else {
                    "Nachrichten in diesem Gespräch erscheinen hier."
                })
                .color(MUTED),
            );
        });
    }
    fn empty_conversation(&mut self, ui: &mut egui::Ui, compact: bool) {
        ui.add_space((ui.available_height() * 0.20).max(24.0));
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(120.0, 100.0), Sense::hover());
            ui.painter()
                .circle_filled(r.center(), 48.0, theme::SOFT_GREEN.gamma_multiply(0.50));
            theme::icon(
                ui.painter(),
                Rect::from_center_size(r.center(), vec2(50.0, 50.0)),
                Icon::Chat,
                MINT,
            );
            ui.add_space(22.0);
            ui.label(
                RichText::new("Nah dran. Ganz privat.")
                    .size(if compact { 25.0 } else { 31.0 })
                    .color(TEXT),
            );
            ui.add_space(12.0);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Wähle einen Chat oder verbinde dich mit einem PC in deiner Nähe.",
                    )
                    .color(MUTED),
                )
                .wrap(),
            );
            ui.add_space(24.0);
            if theme::primary(ui, "Neue Verbindung").clicked() {
                self.design.connections_open = true;
            }
            ui.add_space(36.0);
            ui.horizontal_wrapped(|ui| {
                theme::badge(ui, Icon::Bluetooth, "Direkt per Bluetooth", MUTED);
                theme::badge(ui, Icon::Lock, "Ende-zu-Ende verschlüsselt", MUTED);
            });
        });
    }
    fn pending_changes(&mut self, ui: &mut egui::Ui, ready: bool) {
        let pending: Vec<String> = self
            .view
            .pending_actions
            .iter()
            .filter(|(_, c)| self.selected.as_deref() == Some(c))
            .map(|(id, _)| id.clone())
            .collect();
        if !pending.is_empty() {
            ui.collapsing(
                format!("{} Änderungen noch unbestätigt", pending.len()),
                |ui| {
                    ui.label(
                        "Nach einem Abbruch erneut verbinden und bewusst noch einmal übertragen.",
                    );
                    for id in pending {
                        if ui
                            .add_enabled(
                                ready,
                                egui::Button::new(format!(
                                    "Änderung {} erneut übertragen",
                                    &id[..8]
                                )),
                            )
                            .clicked()
                        {
                            self.command(Command::RetryAction { id });
                        }
                    }
                },
            );
        }
    }
    fn delete_history_prompt(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new().fill(Color32::from_rgb(62,37,35)).corner_radius(12).inner_margin(16).show(ui,|ui|{
            ui.label("Diesen Verlauf samt Anhängen auf diesem PC löschen? Gegenstelle und Sicherungen behalten ihre Kopien. Sichere physische Löschung wird nicht zugesichert.");
            ui.horizontal(|ui|{if ui.button("Lokal löschen").clicked(){if let Some(id)=&self.selected{self.command(Command::DeleteHistory{contact_id:id.clone()});}self.delete_requested=false;}
if ui.button("Behalten").clicked(){self.delete_requested=false;}});
        });
    }
    fn connection_safety(&mut self, ui: &mut egui::Ui) -> bool {
        let pending = self.view.pending_incoming.clone();
        let changed = self.view.key_change.clone();
        let peer = self.view.peer.clone().filter(|p| !p.ready);
        if pending.is_none() && changed.is_none() && peer.is_none() {
            return false;
        }
        theme::wallpaper(ui.painter(), ui.available_rect_before_wrap());
        egui::ScrollArea::vertical().id_salt("trust-view").show(ui,|ui|{egui::Frame::new().inner_margin(28).show(ui,|ui|{
            ui.add_space(18.0);let(r,_)=ui.allocate_exact_size(vec2(52.0,52.0),Sense::hover());ui.painter().circle_filled(r.center(),26.0,theme::SOFT_GREEN);theme::icon(ui.painter(),r.shrink(13.0),Icon::Shield,MINT);ui.add_space(16.0);
            if let Some(device)=pending{ui.label(RichText::new("Ein neues Gespräch?").size(28.0));ui.add_space(10.0);ui.label(format!("{} möchte sich verbinden.",device.name));ui.label(RichText::new("Ein Gerätename ist noch kein Identitätsnachweis. Nach dem Annehmen vergleicht ihr euren Sicherheitscode.").color(MUTED));ui.label(RichText::new(&device.address).small().color(MUTED));ui.add_space(16.0);ui.horizontal(|ui|{if theme::primary(ui,"Annehmen").clicked(){self.command(Command::Respond{accept:true});}
if ui.button("Ablehnen").clicked(){self.command(Command::Respond{accept:false});}});}
            if let Some(device)=changed{ui.label(RichText::new("Identität hat sich geändert").size(25.0).color(Color32::from_rgb(245,178,144)));ui.label(format!("Die Verbindung zu {} ist blockiert. Kläre unabhängig, ob das Profil bewusst neu erstellt wurde. Bei Zweifel abbrechen.",device.name));ui.add_space(14.0);if ui.button("Neue Identität bewusst erneut prüfen").clicked(){self.command(Command::AllowKeyChange);}
if ui.button("Abbrechen").clicked(){self.command(Command::Disconnect);}}
            if let Some(peer)=peer{
                ui.label(RichText::new("Einmal vergleichen.\nSicher miteinander schreiben.").size(28.0));ui.add_space(12.0);ui.label(format!("Gegenstelle: {}",peer.name));ui.label(RichText::new("Vergleicht alle Gruppen persönlich oder über einen unabhängigen Weg. Erst nach Bestätigung auf beiden PCs wird der Chat freigegeben.").color(MUTED));ui.add_space(18.0);
                if peer.fingerprint.is_empty(){ui.spinner();ui.label("Die sichere Verbindung wird aufgebaut …");}
                else{
                    egui::Frame::new().fill(INPUT).corner_radius(14).inner_margin(20).show(ui,|ui|{ui.add(egui::Label::new(RichText::new(&peer.fingerprint).monospace().size(17.0).color(MINT)).wrap());});
                    ui.add_space(12.0);if ui.button("QR-Code & Kameravergleich").clicked(){self.media.qr_open=true;}
                    if peer.known{ui.label(RichText::new("Die Identität stimmt mit deinem gespeicherten Kontakt überein.").color(MINT));}
                    if peer.local_confirmed{ui.label("Von dir bestätigt. Warte auf die andere Person …");}else{ui.add_space(12.0);ui.checkbox(&mut self.fingerprint_checked,"Alle Gruppen stimmen auf beiden PCs überein");if ui.add_enabled(self.fingerprint_checked,egui::Button::new("Kontakt bestätigen").fill(theme::SOFT_GREEN)).clicked(){self.command(Command::ConfirmPeer);self.fingerprint_checked=false;}}
                }
                ui.add_space(12.0);if ui.button("Verbindung trennen").clicked(){self.command(Command::Disconnect);}
            }
            self.show_error(ui);
        });});
        true
    }
}

fn contact_row(
    ui: &mut egui::Ui,
    c: &ContactView,
    last: Option<&MessageView>,
    selected: bool,
    connected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 81.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, &c.name)
    });
    if selected || response.hovered() || response.has_focus() {
        ui.painter().rect_filled(
            rect.shrink2(vec2(8.0, 2.0)),
            10,
            if selected { theme::ACTIVE } else { SURFACE },
        );
    }
    theme::avatar(
        ui.painter(),
        pos2(rect.left() + 38.0, rect.center().y),
        23.0,
        &c.name,
        &c.id,
    );
    if connected {
        ui.painter()
            .circle_filled(pos2(rect.left() + 55.0, rect.center().y + 17.0), 5.0, MINT);
    }
    let x = rect.left() + 76.0;
    let right = rect.right() - 20.0;
    let max_width = (right - x - 42.0).max(40.0);
    let name = ui
        .painter()
        .layout(c.name.clone(), FontId::proportional(14.0), TEXT, max_width);
    ui.painter()
        .with_clip_rect(Rect::from_min_max(
            pos2(x, rect.top()),
            pos2(right - 28.0, rect.bottom()),
        ))
        .galley(pos2(x, rect.top() + 18.0), name, TEXT);
    let preview = Zeroizing::new(
        last.map(|m| {
            if m.attachment_id.is_some() {
                format!("{}Datei: {}", if m.outgoing { "Du: " } else { "" }, m.text)
            } else {
                format!(
                    "{}{}",
                    if m.outgoing { "Du: " } else { "" },
                    m.text.replace(['\n', '\r'], " ")
                )
            }
        })
        .unwrap_or_else(|| "Noch keine Nachrichten".into()),
    );
    let preview = Zeroizing::new(preview.chars().take(100).collect::<String>());
    let galley =
        ui.painter()
            .layout_no_wrap(preview.to_string(), FontId::proportional(12.0), MUTED);
    ui.painter()
        .with_clip_rect(Rect::from_min_max(
            pos2(x, rect.top()),
            pos2(right - 22.0, rect.bottom()),
        ))
        .galley(pos2(x, rect.top() + 44.0), galley, MUTED);
    if let Some(last) = last {
        ui.painter().text(
            pos2(right, rect.top() + 19.0),
            Align2::RIGHT_TOP,
            bubble_time(last.timestamp_ms),
            FontId::proportional(10.0),
            if c.unread > 0 { MINT } else { MUTED },
        );
    }
    if c.unread > 0 {
        let center = pos2(right - 9.0, rect.top() + 53.0);
        ui.painter().circle_filled(center, 9.0, MINT);
        ui.painter().text(
            center,
            Align2::CENTER_CENTER,
            c.unread.min(99).to_string(),
            FontId::proportional(9.0),
            BACKGROUND,
        );
    } else if c.favorite {
        theme::icon(
            ui.painter(),
            Rect::from_center_size(pos2(right - 9.0, rect.top() + 52.0), vec2(12.0, 12.0)),
            Icon::Pin,
            MUTED,
        );
    }
    response
}
fn bubble_time(timestamp: u64) -> String {
    let minutes = timestamp / 60_000;
    format!("{:02}:{:02}", (minutes / 60) % 24, minutes % 60)
}
fn day_label(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400);
    let day = timestamp / 86_400_000;
    if day == now {
        "HEUTE".into()
    } else if day + 1 == now {
        "GESTERN".into()
    } else {
        "FRÜHERES GESPRÄCH".into()
    }
}
fn delivery(status: MessageStatus) -> (Icon, Color32, &'static str) {
    match status {
        MessageStatus::Pending => (Icon::Clock, MUTED, "Ausstehend"),
        MessageStatus::Sent => (
            Icon::Check,
            MUTED,
            "Gesendet · noch keine Empfangsbestätigung",
        ),
        MessageStatus::Received => (
            Icon::DoubleCheck,
            MINT,
            "Empfangen und verschlüsselt gespeichert · keine Lesebestätigung",
        ),
        MessageStatus::Failed => (
            Icon::Close,
            Color32::from_rgb(245, 157, 142),
            "Fehlgeschlagen · Empfang nicht bestätigt",
        ),
    }
}
