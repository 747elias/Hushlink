use super::*;
use crate::conversation::{Action, PendingAction};
use rand::{RngCore, rngs::OsRng};
impl Actor {
    pub(super) fn allow_connection_request(&mut self, address: &str) -> bool {
        let known = self.vault.as_ref().is_some_and(|v| {
            v.profile()
                .contacts
                .iter()
                .any(|c| c.bluetooth_address == address)
        });
        if known {
            return true;
        }
        if self.view.unknown_block_until_ms > timestamp_ms()
            || self
                .last_unknown_request
                .is_some_and(|t| t.elapsed() < Duration::from_secs(10))
        {
            return false;
        }
        self.last_unknown_request = Some(Instant::now());
        true
    }

    pub(super) fn queue_action(&mut self, action: Action) -> anyhow::Result<()> {
        let contact = self
            .view
            .peer
            .as_ref()
            .filter(|p| p.ready)
            .ok_or_else(|| anyhow::anyhow!("Beide Personen müssen den Kontakt bestätigen."))?
            .id
            .clone();
        let p = self
            .vault
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?
            .profile_mut();
        anyhow::ensure!(
            p.pending_actions.len() < 64,
            "Zu viele unbestätigte Änderungen. Zuerst erneut übertragen."
        );
        anyhow::ensure!(
            !p.pending_actions
                .iter()
                .any(|a| a.contact_id == contact && a.action.target() == action.target()),
            "Für diese Nachricht steht noch eine Bestätigung aus."
        );
        let mut bytes = [0; 16];
        OsRng.try_fill_bytes(&mut bytes)?;
        let id = hex::encode(bytes);
        crate::conversation::apply(p, &contact, &id, &action, true, timestamp_ms())?;
        p.pending_actions.push(PendingAction {
            id: id.clone(),
            contact_id: contact,
            action,
        });
        anyhow::ensure!(self.save(), "Änderung konnte nicht gespeichert werden.");
        self.retry_action(&id)
    }
    pub(super) fn retry_action(&mut self, id: &str) -> anyhow::Result<()> {
        let peer = self
            .view
            .peer
            .as_ref()
            .filter(|p| p.ready)
            .ok_or_else(|| anyhow::anyhow!("Zuerst erneut mit dem Kontakt verbinden."))?;
        anyhow::ensure!(
            !self.action_attempts.contains(id),
            "Diese Änderung wurde in dieser Verbindung bereits übertragen. Bei fehlender Bestätigung neu verbinden."
        );
        let pending = self
            .vault
            .as_ref()
            .and_then(|v| {
                v.profile()
                    .pending_actions
                    .iter()
                    .find(|a| a.id == id && a.contact_id == peer.id)
            })
            .ok_or_else(|| anyhow::anyhow!("Keine ausstehende Änderung für diesen Kontakt."))?;
        self.peer_command(PeerCommand::Action(
            parse_message_id(id)?,
            pending.action.clone(),
        ))?;
        self.action_attempts.insert(id.into());
        Ok(())
    }
    pub(super) fn receive_action(&mut self, id: protocol::MessageId, action: &Action) {
        let Some(contact) = self
            .view
            .peer
            .as_ref()
            .filter(|p| p.ready)
            .map(|p| p.id.clone())
        else {
            self.disconnect("Änderung vor Kontaktbestätigung verworfen");
            return;
        };
        let Some(v) = &mut self.vault else {
            return;
        };
        let result = crate::conversation::apply(
            v.profile_mut(),
            &contact,
            &hex::encode(id),
            action,
            false,
            timestamp_ms(),
        );
        let Ok(changed) = result else {
            self.disconnect("Unzulässige oder widersprüchlich wiederholte Änderung verworfen");
            return;
        };
        let reply = matches!(action, Action::Reply { .. });
        if changed
            && reply
            && let Some(c) = v
                .profile_mut()
                .contacts
                .iter_mut()
                .find(|c| c.id == contact)
        {
            c.unread = c.unread.saturating_add(1).min(vault::MAX_MESSAGES as u32);
        }
        if !self.save() {
            self.disconnect("Änderung konnte nicht sicher gespeichert werden");
            return;
        }
        if changed && reply {
            self.notify_peer();
        }
        if self.current() && self.peer_command(PeerCommand::ActionAck(id)).is_err() {
            self.disconnect("Änderungsbestätigung konnte nicht gesendet werden");
        }
    }
    pub(super) fn action_ack(&mut self, id: protocol::MessageId) {
        let id = hex::encode(id);
        let Some(contact) = self
            .view
            .peer
            .as_ref()
            .filter(|p| p.ready)
            .map(|p| p.id.clone())
        else {
            self.disconnect("Bestätigung vor Kontaktprüfung verworfen");
            return;
        };
        let Some(v) = &mut self.vault else {
            return;
        };
        let p = v.profile_mut();
        let Some(index) = p
            .pending_actions
            .iter()
            .position(|a| a.id == id && a.contact_id == contact)
        else {
            self.disconnect("Unerwartete Änderungsbestätigung verworfen");
            return;
        };
        let action = p.pending_actions.remove(index);
        if matches!(action.action, Action::Reply { .. })
            && let Some(m) = p
                .messages
                .iter_mut()
                .find(|m| m.id == id && m.contact_id == contact && m.outgoing)
        {
            m.status = vault::MessageStatus::Received;
        }
        if !self.save() {
            self.disconnect("Bestätigung konnte nicht gespeichert werden");
        }
    }
    pub(super) fn typing(&mut self, active: bool) -> anyhow::Result<()> {
        if !self.view.share_typing || !self.view.peer.as_ref().is_some_and(|p| p.ready) {
            return Ok(());
        }
        if active
            && self
                .local_typing_since
                .is_some_and(|t| t.elapsed() < Duration::from_secs(3))
        {
            return Ok(());
        }
        self.peer_command(PeerCommand::Typing(active))?;
        self.local_typing_since = if active { Some(Instant::now()) } else { None };
        Ok(())
    }
    pub(super) fn notify_peer(&self) {
        let muted = self
            .view
            .peer
            .as_ref()
            .and_then(|p| {
                self.vault
                    .as_ref()?
                    .profile()
                    .contacts
                    .iter()
                    .find(|c| c.id == p.id)
            })
            .is_some_and(|c| c.muted_until_ms > timestamp_ms());
        if self.current() && !muted {
            self.hub.notices.received(self.generation);
        }
    }
}
