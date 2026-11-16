use super::*;
use crate::groups::Packet;
#[derive(Zeroize, ZeroizeOnDrop)]
pub enum GroupCommand {
    Create {
        name: String,
        members: Vec<String>,
    },
    Update {
        id: String,
        name: String,
        members: Vec<String>,
    },
    Consent {
        id: String,
        accept: bool,
    },
    Send {
        id: String,
        text: String,
    },
    Close {
        id: String,
    },
    Read {
        id: String,
    },
    ClearHistory {
        id: String,
    },
    Remove {
        id: String,
    },
    Sync(bool),
}
#[derive(Default)]
pub(super) struct GroupState {
    pub(super) sync: bool,
    listener_owned: bool,
    pub(super) attempts: std::collections::HashSet<String>,
    last_try: std::collections::HashMap<String, Instant>,
    pub(super) inflight: Option<String>,
    pub(super) activity: Option<Instant>,
}
impl Actor {
    pub(super) fn group_command(&mut self, command: &GroupCommand) -> anyhow::Result<()> {
        if let GroupCommand::Sync(enabled) = command {
            if *enabled {
                anyhow::ensure!(
                    self.peer.is_none() && self.connect_flag.is_none() && self.pending.is_none(),
                    "Zuerst die Einzelverbindung trennen."
                );
                if self.listener_flag.is_none() {
                    self.start_listening()?;
                    self.group_state.listener_owned = true;
                }
                self.group_state.sync = true;
                self.group_state.last_try.clear();
                self.view.status="Gruppensynchronisierung aktiv – bestätigte Mitglieder werden nacheinander verbunden".into();
            } else {
                self.group_state.sync = false;
                self.disconnect("Gruppensynchronisierung pausiert");
                if self.group_state.listener_owned {
                    if let Some(f) = self.listener_flag.take() {
                        f.store(true, Ordering::Release);
                    }
                    self.view.listening = false;
                    self.group_state.listener_owned = false;
                }
            }
            self.view.group_sync = *enabled;
            return Ok(());
        }
        let local = hex::encode(
            self.vault
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?
                .profile()
                .identity_public,
        );
        let known: Vec<_> = self
            .vault
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?
            .profile()
            .contacts
            .iter()
            .map(|c| c.id.clone())
            .collect();
        let no_connection = self.peer.is_none();
        let p = self
            .vault
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?
            .profile_mut();
        let prior = p.groups.clone();
        let result = (|| -> anyhow::Result<()> {
            match command {
                GroupCommand::Create { name, members } => {
                    anyhow::ensure!(
                        members.iter().all(|m| known.contains(m)),
                        "Alle Gruppenmitglieder zuerst als Kontakte prüfen."
                    );
                    p.groups.create(&local, name.clone(), members.clone())?;
                }
                GroupCommand::Update { id, name, members } => {
                    anyhow::ensure!(
                        members.iter().all(|m| known.contains(m)),
                        "Alle Gruppenmitglieder zuerst als Kontakte prüfen."
                    );
                    p.groups.update(&local, id, name.clone(), members.clone())?;
                }
                GroupCommand::Consent { id, accept } => {
                    anyhow::ensure!(
                        !accept || p.groups.can_join(id, &local, &known),
                        "Vor dem Beitritt alle Mitglieder unabhängig als Kontakte prüfen. Nach Austritt ist eine neue Liste erforderlich."
                    );
                    p.groups.consent(&local, id, *accept)?;
                }
                GroupCommand::Send { id, text } => {
                    anyhow::ensure!(
                        p.groups.can_join(id, &local, &known),
                        "Alle aktuellen Mitglieder müssen bestätigte Kontakte sein."
                    );
                    p.groups.send(&local, id, text.clone(), timestamp_ms())?;
                }
                GroupCommand::Close { id } => p.groups.close(&local, id)?,
                GroupCommand::Read { id } => {
                    if let Some(g) = p.groups.groups.iter_mut().find(|g| g.roster.id == *id) {
                        g.unread = 0;
                    }
                }
                GroupCommand::ClearHistory { id } => {
                    anyhow::ensure!(
                        no_connection,
                        "Zum Löschen zuerst synchronisieren pausieren und trennen."
                    );
                    p.groups.messages.retain(|m| m.group != *id);
                    p.groups.outbox.retain(|d| {
                        !(d.packet.group() == id && matches!(d.packet, Packet::Text { .. }))
                    });
                    if let Some(g) = p.groups.groups.iter_mut().find(|g| g.roster.id == *id) {
                        g.unread = 0;
                    }
                }
                GroupCommand::Remove { id } => {
                    anyhow::ensure!(
                        no_connection
                            && p.groups
                                .groups
                                .iter()
                                .any(|g| g.roster.id == *id && !g.active),
                        "Gruppe zuerst verlassen/schliessen und trennen."
                    );
                    p.groups.groups.retain(|g| g.roster.id != *id);
                    p.groups.messages.retain(|m| m.group != *id);
                    p.groups.outbox.retain(|d| d.packet.group() != id);
                }
                GroupCommand::Sync(_) => {}
            }
            p.groups.validate(&local)?;
            Ok(())
        })();
        if let Err(error) = result {
            p.groups = prior;
            return Err(error);
        }
        anyhow::ensure!(
            self.save(),
            "Gruppenänderung konnte nicht sicher gespeichert werden."
        );
        self.flush_group();
        Ok(())
    }
    pub(super) fn receive_group(&mut self, id: protocol::MessageId, packet: &Packet) {
        let Some(peer) = self
            .view
            .peer
            .as_ref()
            .filter(|p| p.ready)
            .map(|p| p.id.clone())
        else {
            self.disconnect("Gruppenpaket vor Kontaktprüfung verworfen");
            return;
        };
        let Some(v) = &mut self.vault else {
            return;
        };
        let local = hex::encode(v.profile().identity_public);
        let prior = v.profile().groups.clone();
        let result =
            v.profile_mut()
                .groups
                .receive(&local, &peer, &hex::encode(id), packet, timestamp_ms());
        let Ok(changed) = result else {
            v.profile_mut().groups = prior;
            self.disconnect(
                "Ungültige Gruppenliste, fehlender Beitritt oder widersprüchliche Wiederholung",
            );
            return;
        };
        if !self.save() {
            self.disconnect("Gruppenpaket konnte nicht verschlüsselt gespeichert werden");
            return;
        }
        self.group_state.activity = Some(Instant::now());
        if changed && matches!(packet, Packet::Text { .. } | Packet::Roster { .. }) {
            self.notify_peer();
        }
        if self.current() && self.peer_command(PeerCommand::GroupAck(id)).is_err() {
            self.disconnect("Gruppenbestätigung konnte nicht gesendet werden");
        }
    }
    pub(super) fn group_ack(&mut self, id: protocol::MessageId) {
        let id = hex::encode(id);
        let Some(peer) = self
            .view
            .peer
            .as_ref()
            .filter(|p| p.ready)
            .map(|p| p.id.clone())
        else {
            self.disconnect("Gruppenbestätigung ohne Kontaktprüfung");
            return;
        };
        if self.group_state.inflight.as_deref() != Some(&id) {
            self.disconnect("Unerwartete Gruppenbestätigung");
            return;
        }
        self.group_state.inflight = None;
        self.group_state.activity = Some(Instant::now());
        let Some(v) = &mut self.vault else {
            return;
        };
        // Consent/roster changes may deliberately cancel an in-flight delivery.
        // The session already authenticates ACK type and its outstanding ID.
        if v.profile().groups.outbox.iter().any(|d| d.id == id)
            && v.profile_mut().groups.acknowledge(&peer, &id).is_err()
        {
            self.disconnect("Ungültige Gruppenbestätigung");
            return;
        }
        if !self.save() {
            self.disconnect("Gruppenbestätigung konnte nicht gespeichert werden");
        }
    }
    fn flush_group(&mut self) {
        if self.group_state.inflight.is_some() || !self.current() {
            return;
        }
        let Some(peer) = self.view.peer.as_ref().filter(|p| p.ready) else {
            return;
        };
        let Some(v) = &self.vault else {
            return;
        };
        let Some(d) = v
            .profile()
            .groups
            .outbox
            .iter()
            .find(|d| {
                d.recipient == peer.id
                    && v.profile().groups.eligible(d)
                    && !self.group_state.attempts.contains(&d.id)
            })
            .cloned()
        else {
            return;
        };
        let Ok(id) = parse_message_id(&d.id) else {
            return;
        };
        if self
            .peer_command(PeerCommand::Group(id, d.packet.clone()))
            .is_ok()
        {
            self.group_state.attempts.insert(d.id.clone());
            self.group_state.inflight = Some(d.id.clone());
            self.group_state.activity = Some(Instant::now());
        }
    }
    pub(super) fn group_tick(&mut self) {
        if self.vault.is_none() || !self.current() {
            return;
        }
        self.flush_group();
        if !self.group_state.sync {
            return;
        }
        if self.peer.is_some() {
            let started = self.group_state.activity.get_or_insert_with(Instant::now);
            let file_active = self.file_loading.is_some()
                || self.view.transfers.iter().any(|t| {
                    matches!(
                        t.state,
                        FileTransferState::AwaitingConsent | FileTransferState::Transferring
                    )
                });
            let direct_pending = self.vault.as_ref().is_some_and(|v| {
                self.view.peer.as_ref().is_some_and(|peer| {
                    v.profile().messages.iter().any(|m| {
                        m.contact_id == peer.id
                            && m.outgoing
                            && matches!(
                                m.status,
                                vault::MessageStatus::Pending | vault::MessageStatus::Sent
                            )
                    })
                })
            });
            if !file_active
                && !direct_pending
                && started.elapsed()
                    > Duration::from_secs(if self.group_state.inflight.is_some() {
                        30
                    } else {
                        8
                    })
                && self.view.peer.as_ref().is_some_and(|p| p.ready)
            {
                self.disconnect("Gruppensynchronisierung – nächstes Mitglied");
            }
            return;
        }
        if self.pending.is_some() || self.connect_flag.is_some() {
            return;
        }
        let Some(v) = &self.vault else {
            return;
        };
        let mut contacts: Vec<_> = v
            .profile()
            .contacts
            .iter()
            .filter(|c| {
                !c.bluetooth_address.is_empty()
                    && v.profile()
                        .groups
                        .outbox
                        .iter()
                        .any(|d| d.recipient == c.id && v.profile().groups.eligible(d))
            })
            .collect();
        contacts.sort_by_key(|c| self.group_state.last_try.get(&c.id).copied());
        if let Some(c) = contacts.first() {
            if self
                .group_state
                .last_try
                .get(&c.id)
                .is_some_and(|t| t.elapsed() < Duration::from_secs(20))
            {
                return;
            }
            let address = c.bluetooth_address.clone();
            self.group_state
                .last_try
                .insert(c.id.clone(), Instant::now());
            if let Err(error) = self.connect(address) {
                self.error(error);
            }
        }
    }
    pub(super) fn group_auto_accept(&self, address: &str) -> bool {
        self.group_state.sync
            && self.vault.as_ref().is_some_and(|v| {
                v.profile().contacts.iter().any(|c| {
                    c.bluetooth_address == address
                        && v.profile()
                            .groups
                            .groups
                            .iter()
                            .any(|g| !g.closed && g.roster.members.contains(&c.id))
                })
            })
    }
}
