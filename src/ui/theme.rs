//! Shared visual language. Icons are painted locally; no network assets or icon font.
use super::*;
use egui::{
    Align2, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, WidgetInfo,
    WidgetType, pos2, vec2,
};

pub(super) const RAIL: Color32 = Color32::from_rgb(17, 26, 32);
pub(super) const SURFACE: Color32 = Color32::from_rgb(24, 35, 42);
pub(super) const INPUT: Color32 = Color32::from_rgb(31, 44, 52);
pub(super) const LINE: Color32 = Color32::from_rgb(40, 54, 62);
pub(super) const TEXT: Color32 = Color32::from_rgb(233, 239, 241);
pub(super) const ACTIVE: Color32 = Color32::from_rgb(31, 54, 56);
pub(super) const OUTGOING: Color32 = Color32::from_rgb(0, 82, 69);
pub(super) const SOFT_GREEN: Color32 = Color32::from_rgb(24, 59, 50);

pub(super) fn configure(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = INPUT;
    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_color = Some(MUTED);
    visuals.selection.bg_fill = Color32::from_rgb(25, 79, 66);
    visuals.selection.stroke = Stroke::new(1.0, MINT);
    visuals.hyperlink_color = MINT;
    visuals.window_corner_radius = CornerRadius::same(18);
    visuals.menu_corner_radius = CornerRadius::same(12);
    visuals.window_stroke = Stroke::new(1.0, LINE);
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 36,
        spread: 0,
        color: Color32::from_black_alpha(90),
    };
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(9);
        widget.fg_stroke = Stroke::new(1.2, TEXT);
        widget.bg_stroke = Stroke::NONE;
    }
    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    visuals.widgets.inactive.weak_bg_fill = INPUT;
    visuals.widgets.inactive.bg_fill = INPUT;
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(42, 59, 65);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(42, 59, 65);
    visuals.widgets.active.bg_fill = SOFT_GREEN;
    visuals.widgets.active.weak_bg_fill = SOFT_GREEN;
    visuals.widgets.open.bg_fill = ACTIVE;
    visuals.widgets.open.weak_bg_fill = ACTIVE;
    ctx.set_visuals(visuals);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(10.0, 8.0);
        s.spacing.button_padding = vec2(12.0, 8.0);
        s.spacing.interact_size = vec2(36.0, 34.0);
        s.spacing.window_margin = egui::Margin::same(20);
        s.spacing.menu_margin = egui::Margin::same(8);
        s.text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(22.0));
        s.text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        s.text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(11.0));
    });
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Inter".into(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/Inter.ttf")).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".into());
    #[cfg(windows)]
    if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\seguiemj.ttf") {
        fonts.font_data.insert(
            "Segoe Emoji".into(),
            egui::FontData::from_owned(data).into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("Segoe Emoji".into());
    }
    ctx.set_fonts(fonts);
}

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Chat,
    Search,
    Plus,
    Settings,
    Lock,
    Shield,
    Bluetooth,
    Attach,
    Send,
    File,
    Check,
    DoubleCheck,
    Clock,
    Close,
    Link,
    Image,
    Pin,
    Smile,
    Download,
    Back,
    More,
}

pub(super) fn icon(p: &egui::Painter, rect: Rect, kind: Icon, color: Color32) {
    let center = rect.center();
    let size = rect.width().min(rect.height()) * 0.5;
    let point = |x: f32, y: f32| center + vec2(x * size, y * size);
    let stroke = Stroke::new(1.6, color);
    let line = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|(x, y)| point(*x, *y)).collect(),
            stroke,
        ));
    };
    match kind {
        Icon::Chat => {
            p.rect_stroke(
                Rect::from_min_max(point(-0.72, -0.62), point(0.72, 0.48)),
                4,
                stroke,
                StrokeKind::Middle,
            );
            line(&[(-0.40, 0.49), (-0.45, 0.84), (0.03, 0.48)]);
        }
        Icon::Search => {
            p.circle_stroke(point(-0.15, -0.15), size * 0.51, stroke);
            line(&[(0.26, 0.26), (0.77, 0.77)]);
        }
        Icon::Plus => {
            line(&[(-0.66, 0.0), (0.66, 0.0)]);
            line(&[(0.0, -0.66), (0.0, 0.66)]);
        }
        Icon::Settings => {
            p.circle_stroke(center, size * 0.52, stroke);
            p.circle_stroke(center, size * 0.19, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                line(&[
                    (a.cos() * 0.52, a.sin() * 0.52),
                    (a.cos() * 0.77, a.sin() * 0.77),
                ]);
            }
        }
        Icon::Lock => {
            p.rect_stroke(
                Rect::from_min_max(point(-0.57, -0.05), point(0.57, 0.70)),
                3,
                stroke,
                StrokeKind::Middle,
            );
            line(&[
                (-0.37, -0.06),
                (-0.37, -0.49),
                (-0.22, -0.70),
                (0.22, -0.70),
                (0.37, -0.49),
                (0.37, -0.06),
            ]);
            p.circle_filled(point(0.0, 0.29), size * 0.08, color);
        }
        Icon::Shield => {
            line(&[
                (0.0, -0.79),
                (0.65, -0.54),
                (0.60, 0.18),
                (0.36, 0.55),
                (0.0, 0.81),
                (-0.36, 0.55),
                (-0.60, 0.18),
                (-0.65, -0.54),
                (0.0, -0.79),
            ]);
            line(&[(-0.28, 0.01), (-0.05, 0.25), (0.33, -0.24)]);
        }
        Icon::Bluetooth => {
            line(&[(0.0, -0.88), (0.0, 0.88), (0.57, 0.38), (-0.53, -0.44)]);
            line(&[(-0.53, 0.44), (0.57, -0.38), (0.0, -0.88)]);
        }
        Icon::Attach => {
            line(&[
                (0.56, -0.20),
                (-0.12, 0.52),
                (-0.37, 0.53),
                (-0.52, 0.31),
                (-0.44, 0.08),
                (0.21, -0.61),
                (0.50, -0.66),
                (0.74, -0.43),
                (0.76, -0.15),
                (0.04, 0.66),
                (-0.28, 0.80),
                (-0.63, 0.67),
                (-0.82, 0.35),
                (-0.77, 0.04),
                (-0.12, -0.69),
            ]);
        }
        Icon::Send => {
            line(&[
                (-0.76, -0.69),
                (0.83, 0.0),
                (-0.76, 0.69),
                (-0.42, 0.0),
                (-0.76, -0.69),
            ]);
            line(&[(-0.42, 0.0), (0.83, 0.0)]);
        }
        Icon::File => {
            line(&[
                (-0.52, -0.78),
                (0.17, -0.78),
                (0.56, -0.37),
                (0.56, 0.78),
                (-0.52, 0.78),
                (-0.52, -0.78),
            ]);
            line(&[(0.17, -0.78), (0.17, -0.37), (0.56, -0.37)]);
            line(&[(-0.27, 0.10), (0.29, 0.10)]);
            line(&[(-0.27, 0.37), (0.20, 0.37)]);
        }
        Icon::Check => line(&[(-0.64, 0.0), (-0.16, 0.45), (0.72, -0.50)]),
        Icon::DoubleCheck => {
            line(&[(-0.82, 0.0), (-0.40, 0.42), (0.35, -0.45)]);
            line(&[(-0.06, 0.19), (0.17, 0.42), (0.91, -0.45)]);
        }
        Icon::Clock => {
            p.circle_stroke(center, size * 0.72, stroke);
            line(&[(0.0, -0.40), (0.0, 0.0), (0.32, 0.20)]);
        }
        Icon::Close => {
            line(&[(-0.52, -0.52), (0.52, 0.52)]);
            line(&[(-0.52, 0.52), (0.52, -0.52)]);
        }
        Icon::Link => {
            line(&[
                (-0.10, -0.30),
                (0.33, -0.73),
                (0.64, -0.72),
                (0.80, -0.48),
                (0.74, -0.20),
                (0.30, 0.23),
            ]);
            line(&[
                (0.10, 0.30),
                (-0.33, 0.73),
                (-0.64, 0.72),
                (-0.80, 0.48),
                (-0.74, 0.20),
                (-0.30, -0.23),
            ]);
            line(&[(-0.28, 0.28), (0.28, -0.28)]);
        }
        Icon::Image => {
            p.rect_stroke(
                rect.shrink(rect.width() * 0.17),
                3,
                stroke,
                StrokeKind::Middle,
            );
            line(&[
                (-0.60, 0.50),
                (-0.14, -0.07),
                (0.17, 0.24),
                (0.39, -0.04),
                (0.63, 0.27),
            ]);
            p.circle_filled(point(0.29, -0.32), size * 0.11, color);
        }
        Icon::Pin => {
            line(&[
                (-0.36, -0.72),
                (0.36, -0.72),
                (0.27, -0.18),
                (0.53, 0.20),
                (-0.53, 0.20),
                (-0.27, -0.18),
                (-0.36, -0.72),
            ]);
            line(&[(0.0, 0.20), (0.0, 0.83)]);
        }
        Icon::Smile => {
            p.circle_stroke(center, size * 0.76, stroke);
            p.circle_filled(point(-0.25, -0.18), size * 0.07, color);
            p.circle_filled(point(0.25, -0.18), size * 0.07, color);
            line(&[(-0.35, 0.22), (-0.18, 0.40), (0.18, 0.40), (0.35, 0.22)]);
        }
        Icon::Download => {
            line(&[(0.0, -0.72), (0.0, 0.29)]);
            line(&[(-0.33, 0.01), (0.0, 0.34), (0.33, 0.01)]);
            line(&[(-0.64, 0.39), (-0.64, 0.74), (0.64, 0.74), (0.64, 0.39)]);
        }
        Icon::More => {
            for y in [-0.48, 0.0, 0.48] {
                p.circle_filled(point(0.0, y), size * 0.10, color);
            }
        }
        Icon::Back => {
            line(&[(0.22, -0.63), (-0.44, 0.0), (0.22, 0.63)]);
            line(&[(-0.44, 0.0), (0.73, 0.0)]);
        }
    }
}

pub(super) fn icon_button(ui: &mut egui::Ui, kind: Icon, label: &str, active: bool) -> Response {
    let fill = if active {
        SOFT_GREEN
    } else {
        Color32::TRANSPARENT
    };
    let response = ui.add_sized(
        [38.0, 38.0],
        egui::Button::new("")
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(12),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    let color = if !ui.is_enabled() {
        MUTED.gamma_multiply(0.38)
    } else if active {
        MINT
    } else if response.hovered() {
        TEXT
    } else {
        MUTED
    };
    icon(
        ui.painter(),
        Rect::from_center_size(response.rect.center(), vec2(24.0, 24.0)),
        kind,
        color,
    );
    response.on_hover_text(label)
}
pub(super) fn primary(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .color(Color32::from_rgb(7, 36, 28))
                .size(14.0),
        )
        .fill(MINT)
        .corner_radius(11)
        .min_size(vec2(0.0, 42.0)),
    )
}
pub(super) fn pill(ui: &mut egui::Ui, label: &str, selected: bool) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(if selected {
            MINT
        } else {
            MUTED
        }))
        .fill(if selected { SOFT_GREEN } else { INPUT })
        .corner_radius(18)
        .min_size(vec2(0.0, 30.0)),
    )
}
pub(super) fn avatar(p: &egui::Painter, center: Pos2, radius: f32, name: &str, id: &str) {
    let colors = [
        Color32::from_rgb(54, 85, 81),
        Color32::from_rgb(71, 75, 105),
        Color32::from_rgb(102, 78, 63),
        Color32::from_rgb(62, 84, 106),
        Color32::from_rgb(92, 67, 84),
    ];
    let color = colors[id.bytes().fold(0usize, |n, b| n.wrapping_add(b as usize)) % colors.len()];
    p.circle_filled(center, radius, color);
    let initials = Zeroizing::new(
        name.split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .flat_map(char::to_uppercase)
            .collect::<String>(),
    );
    p.text(
        center,
        Align2::CENTER_CENTER,
        initials.as_str(),
        FontId::proportional(radius * 0.66),
        Color32::from_rgb(219, 235, 230),
    );
}
pub(super) fn badge(ui: &mut egui::Ui, kind: Icon, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.10))
        .corner_radius(16)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                icon(ui.painter(), r, kind, color);
                ui.label(RichText::new(text).size(11.0).color(color));
            });
        });
}
pub(super) fn wallpaper(p: &egui::Painter, rect: Rect) {
    let p = p.with_clip_rect(rect);
    let color = Color32::from_rgb(20, 34, 39);
    for row in 0..(rect.height() / 56.0).ceil() as usize {
        for col in 0..(rect.width() / 56.0).ceil() as usize {
            let x = rect.left() + col as f32 * 56.0 + if row % 2 == 0 { 14.0 } else { 42.0 };
            let y = rect.top() + row as f32 * 56.0 + 18.0;
            p.circle_filled(pos2(x, y), 1.0, color);
        }
    }
}

pub(super) fn icon_menu(
    ui: &mut egui::Ui,
    kind: Icon,
    label: &str,
    contents: impl FnOnce(&mut egui::Ui),
) -> Response {
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().interact_size = vec2(34.0, 32.0);
            ui.spacing_mut().button_padding = vec2(7.0, 6.0);
            ui.menu_button(" ", contents).response
        })
        .inner;
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    icon(
        ui.painter(),
        Rect::from_center_size(response.rect.center(), vec2(21.0, 21.0)),
        kind,
        MUTED,
    );
    response.on_hover_text(label)
}
