use super::*;
use rand::{RngCore, rngs::OsRng};
impl Actor {
    pub(super) fn configure_backups(
        &mut self,
        folder: &str,
        retention: u8,
        interval: u32,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            (1..=20).contains(&retention)
                && (5..=10080).contains(&interval)
                && folder.len() <= 16_384,
            "Ungültige Sicherungseinstellungen."
        );
        anyhow::ensure!(
            folder.is_empty() || std::path::Path::new(folder).is_dir(),
            "Sicherungsordner nicht verfügbar."
        );
        let p = self
            .vault
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?
            .profile_mut();
        if p.settings.backup_id.is_empty() {
            let mut id = [0; 16];
            OsRng.try_fill_bytes(&mut id)?;
            p.settings.backup_id = hex::encode(id);
        }
        p.settings.backup_folder = folder.into();
        p.settings.backup_retention = retention;
        p.settings.backup_interval_minutes = interval;
        anyhow::ensure!(
            self.save(),
            "Sicherungseinstellungen konnten nicht gespeichert werden."
        );
        self.backup_checked = None;
        Ok(())
    }
    pub(super) fn make_backup(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(self.current(), "Tresor wurde gesperrt.");
        let v = self
            .vault
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?;
        let settings = &v.profile().settings;
        anyhow::ensure!(
            !settings.backup_folder.is_empty(),
            "Zuerst einen Sicherungsordner wählen."
        );
        let now = timestamp_ms();
        vault::backups::create(
            v,
            std::path::Path::new(&settings.backup_folder),
            &settings.backup_id,
            now,
            settings.backup_retention,
        )?;
        if let Some(v) = &mut self.vault {
            v.profile_mut().settings.last_backup_ms = now;
        }
        anyhow::ensure!(
            self.save(),
            "Sicherung gespeichert, ihr Zeitstempel konnte im aktiven Tresor nicht gespeichert werden."
        );
        self.view.status = "Verschlüsselte Sicherung erstellt".into();
        self.snapshot();
        Ok(())
    }
    pub(super) fn backup_if_due(&mut self) {
        if !self.current()
            || self
                .backup_checked
                .is_some_and(|t| t.elapsed() < Duration::from_secs(60))
        {
            return;
        }
        let due = self.vault.as_ref().is_some_and(|v| {
            let s = &v.profile().settings;
            !s.backup_folder.is_empty()
                && timestamp_ms().saturating_sub(s.last_backup_ms)
                    >= u64::from(s.backup_interval_minutes) * 60_000
        });
        self.backup_checked = Some(Instant::now());
        if due && let Err(error) = self.make_backup() {
            self.error(error);
        }
    }
    pub(super) fn delete_item(&mut self, id: &str, attachment_only: bool) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.peer.is_none()
                && self.connect_flag.is_none()
                && self.incoming_file.is_none()
                && self.file_loading.is_none(),
            "Vor dem gezielten Löschen Verbindungen und Übertragungen beenden."
        );
        let p = self
            .vault
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?
            .profile_mut();
        let m = p
            .messages
            .iter()
            .find(|m| m.id == id || m.attachment_id.as_deref() == Some(id))
            .ok_or_else(|| anyhow::anyhow!("Eintrag nicht vorhanden."))?;
        let message = m.id.clone();
        anyhow::ensure!(
            !p.pending_actions
                .iter()
                .any(|a| a.id == message || a.action.target() == message),
            "Zuerst ausstehende Änderungen übertragen oder den gesamten Verlauf löschen."
        );
        anyhow::ensure!(
            !attachment_only || m.attachment_id.is_some(),
            "Kein gespeicherter Dateianhang vorhanden."
        );
        p.attachments.retain(|a| a.id != id && a.id != message);
        p.partials.retain(|a| a.id != id && a.id != message);
        p.messages.retain(|m| m.id != message);
        // Keep incoming receipts: a late retry must never recreate a deleted message.
        anyhow::ensure!(self.save(), "Eintrag konnte nicht sicher gelöscht werden.");
        self.view.status =
            "Eintrag lokal gelöscht. Ältere Sicherungen können ihn weiterhin enthalten.".into();
        Ok(())
    }
}
