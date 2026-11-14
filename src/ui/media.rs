use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
struct ResultEvent {
    silent: bool,
    sequence: u64,
    generation: u64,
    peer: String,
    result: Result<Content, String>,
}
enum Content {
    Image(Zeroizing<Vec<u8>>),
    Qr(Vec<u8>),
}
pub(super) struct MediaUi {
    mailbox: Arc<Mutex<Option<ResultEvent>>>,
    flag: Option<Arc<AtomicBool>>,
    sequence: u64,
    target: Option<(u64, String, bool)>,
    pending: Option<Zeroizing<Vec<u8>>>,
    pub(super) qr_open: bool,
    camera: u32,
    status: String,
}
impl Default for MediaUi {
    fn default() -> Self {
        Self {
            mailbox: Arc::new(Mutex::new(None)),
            flag: None,
            sequence: 0,
            target: None,
            pending: None,
            qr_open: false,
            camera: 0,
            status: String::new(),
        }
    }
}
impl MediaUi {
    pub(super) fn clear(&mut self) {
        if let Some(flag) = self.flag.take() {
            flag.store(true, Ordering::Release);
        }
        self.sequence = self.sequence.wrapping_add(1);
        self.pending = None;
        self.status.zeroize();
        self.qr_open = false;
        self.target = None;
        if let Ok(mut value) = self.mailbox.lock() {
            *value = None;
        }
    }
    fn start(
        &mut self,
        ctx: &egui::Context,
        clipboard: bool,
        silent: bool,
        flag: Arc<AtomicBool>,
    ) -> bool {
        if self.flag.is_some() {
            return false;
        }
        let Some((generation, peer, _)) = self.target.clone() else {
            return false;
        };
        let sequence = self.sequence;
        if flag.load(Ordering::Acquire) {
            return false;
        }
        self.flag = Some(flag.clone());
        let mailbox = self.mailbox.clone();
        let ctx = ctx.clone();
        let camera = self.camera;
        if !silent {
            self.status=if clipboard{"Bild wird aus der Zwischenablage gelesen …"}else{"Kamera liest den Code, höchstens 30 Sekunden. Halte den Code des anderen PCs in die Kamera."}.into();
        }
        std::thread::spawn(move || {
            if flag.load(Ordering::Acquire) {
                return;
            }
            let result = if clipboard {
                crate::media::clipboard_image().map(Content::Image)
            } else {
                crate::media::scan_camera(camera, &flag).map(Content::Qr)
            }
            .map_err(|e| e.to_string());
            if let Ok(mut value) = mailbox.lock()
                && !flag.load(Ordering::Acquire)
            {
                *value = Some(ResultEvent {
                    silent,
                    sequence,
                    generation,
                    peer,
                    result,
                });
                ctx.request_repaint();
            }
        });
        true
    }
}
impl Drop for MediaUi {
    fn drop(&mut self) {
        self.clear();
    }
}
impl App {
    fn start_media(&mut self, ctx: &egui::Context, clipboard: bool, silent: bool) -> bool {
        let Some(runtime) = &self.runtime else {
            return false;
        };
        self.media.start(
            ctx,
            clipboard,
            silent,
            runtime.media_operation_flag(self.generation),
        )
    }

    pub(super) fn sync_media(&mut self) {
        let target = self
            .view
            .peer
            .as_ref()
            .filter(|p| self.view.unlocked && (!p.ready || self.selected.as_deref() == Some(&p.id)))
            .map(|p| (self.generation, p.id.clone(), p.ready));
        if target != self.media.target {
            self.media.clear();
            self.media.target = target;
        }
    }
    pub(super) fn poll_media(&mut self, ctx: &egui::Context) {
        self.sync_media();
        let event = self.media.mailbox.lock().ok().and_then(|mut v| v.take());
        if let Some(event) = event
            && event.sequence == self.media.sequence
            && self
                .media
                .target
                .as_ref()
                .is_some_and(|(g, p, _)| *g == event.generation && *p == event.peer)
        {
            self.media.flag = None;
            match event.result {
                Ok(Content::Image(bytes)) => {
                    if self.view.peer.as_ref().is_some_and(|p| p.ready) {
                        self.media.pending = Some(bytes);
                        self.media.status =
                            "Bild vorbereitet. Es wird erst nach deinem Klick angeboten.".into();
                    }
                }
                Ok(Content::Qr(bytes)) => {
                    if self
                        .view
                        .peer
                        .as_ref()
                        .is_some_and(|p| crate::media::matches_qr(&bytes, &p.fingerprint))
                    {
                        self.fingerprint_checked = true;
                        self.media.status="Der gesamte Sicherheitscode stimmt überein. Bestätige den Kontakt und lass die andere Person ebenfalls prüfen.".into();
                    } else {
                        self.fingerprint_checked = false;
                        self.media.status="Der gelesene Code passt nicht zu dieser Verbindung. Kontakt nicht bestätigen; Gegenstelle und Code unabhängig prüfen.".into();
                    }
                }
                Err(error) => {
                    if !event.silent {
                        self.media.status = error;
                    }
                }
            }
        }
        if self.media.qr_open && self.view.peer.as_ref().is_some_and(|p| !p.ready) {
            self.qr_window(ctx);
        }
    }
    pub(super) fn clipboard_button(&mut self, ui: &mut egui::Ui, ready: bool) {
        if ui
            .add_enabled(
                ready && self.media.flag.is_none(),
                egui::Button::new("Bild einfügen …"),
            )
            .clicked()
        {
            self.start_media(ui.ctx(), true, false);
        }
    }
    pub(super) fn clipboard_paste(&mut self, ctx: &egui::Context, focused: bool) {
        // egui-winit consumes key-down even when there is no clipboard text.
        // Its forwarded V key-up also covers bitmap-only Ctrl+V.
        let paste=focused && ctx.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::Key{key:egui::Key::V,pressed:false,modifiers,..} if modifiers.ctrl && !modifiers.alt && !modifiers.shift)));
        if paste
            && self.view.peer.as_ref().is_some_and(|p| p.ready)
            && self.media.flag.is_none()
            && self.media.pending.is_none()
        {
            self.start_media(ctx, true, true);
        }
    }
    pub(super) fn image_offer(&mut self, ui: &mut egui::Ui) {
        if let Some(bytes) = &self.media.pending {
            let size = bytes.len();
            ui.group(|ui| {
                ui.label(format!(
                    "Zwischenablage.png · {:.1} KiB · noch nicht gesendet",
                    size as f64 / 1024.0
                ));
                ui.horizontal(|ui| {
                    if ui.button("Bild ausdrücklich anbieten").clicked()
                        && let Some(data) = self.media.pending.take()
                    {
                        self.command(Command::SendImage { data });
                        self.integration.transfers_open = true;
                    }
                    if ui.button("Verwerfen").clicked() {
                        self.media.pending = None;
                    }
                });
            });
        }
        if !self.media.status.is_empty() {
            ui.label(RichText::new(&self.media.status).small().color(MUTED));
        }
    }
    fn qr_window(&mut self, ctx: &egui::Context) {
        let Some(peer) = self.view.peer.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Sicherheitscode per Kamera vergleichen").open(&mut open).collapsible(false).vscroll(true).max_height((ctx.content_rect().height()-60.0).max(240.0)).default_width(410.0).show(ctx,|ui|{
            ui.label("Zeige diesen QR-Code auf diesem PC und scanne den Code auf dem anderen PC. Er enthält den gesamten Sicherheitscode dieser Verbindung. Ein Vergleich verlangt weiterhin die Bestätigung beider Personen.");
            if let Ok(payload)=crate::media::qr_payload(&peer.fingerprint) && let Ok(qr)=qrcode::QrCode::with_error_correction_level(payload.as_bytes(),qrcode::EcLevel::Q){
                let width=(ui.available_width()-16.0).clamp(120.0,300.0);let (rect,_)=ui.allocate_exact_size(Vec2::splat(width),egui::Sense::hover());ui.painter().rect_filled(rect,0.0,Color32::WHITE);let cell=width/(qr.width()+8) as f32;for y in 0..qr.width(){for x in 0..qr.width(){if qr[(x,y)]==qrcode::Color::Dark{let min=rect.min+egui::vec2((x+4) as f32*cell,(y+4) as f32*cell);ui.painter().rect_filled(egui::Rect::from_min_size(min,Vec2::splat(cell+0.1)),0.0,Color32::BLACK);}}}
            }
            ui.add(egui::Label::new(RichText::new(&peer.fingerprint).monospace()).wrap());
            ui.horizontal(|ui|{ui.label("Kamera");let mut index=self.media.camera+1;if ui.add(egui::DragValue::new(&mut index).range(1..=16)).changed(){self.media.camera=index-1;}
if ui.add_enabled(self.media.flag.is_none(),egui::Button::new("Mit Kamera prüfen")).clicked(){self.start_media(ctx,false,false);}});
            ui.label(RichText::new("Windows muss den Kamerazugriff für Desktop-Apps erlauben. Kamera nur während dieser bewussten Prüfung; keine Bilder werden gespeichert.").small().color(MUTED));
            ui.label(&self.media.status);
        });
        if !open {
            self.media.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_bitmap_and_worker_results_are_dropped_on_lock_or_context_reset() {
        let mut state = MediaUi::default();
        state.target = Some((1, "synthetic-peer".into(), true));
        state.pending = Some(Zeroizing::new(b"SYNTHETIC_IMAGE_CONTENT".to_vec()));
        let flag = Arc::new(AtomicBool::new(false));
        state.flag = Some(flag.clone());
        let sequence = state.sequence;
        *state.mailbox.lock().unwrap_or_else(|p| p.into_inner()) = Some(ResultEvent {
            sequence,
            silent: false,
            generation: 1,
            peer: "synthetic-peer".into(),
            result: Ok(Content::Image(Zeroizing::new(
                b"SYNTHETIC_LATE_IMAGE".to_vec(),
            ))),
        });
        state.clear();
        assert!(flag.load(Ordering::Acquire));
        assert!(state.pending.is_none() && state.target.is_none());
        assert_ne!(state.sequence, sequence);
        assert!(
            state
                .mailbox
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_none()
        );
    }
}
