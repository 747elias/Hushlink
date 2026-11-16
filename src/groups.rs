//! Direct pairwise group fan-out. Sender identity comes exclusively from Noise.
//! No group cipher, relay, shared key or sender assertion is introduced here.
use anyhow::{Result, ensure};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const MAX_GROUPS: usize = 32;
pub const MAX_MEMBERS: usize = 8;
pub const MAX_OUTBOX: usize = 2048;
pub const MAX_RECEIPTS: usize = 20_000;
pub const MAX_PACKET: usize = crate::protocol::MAX_FRAME - 64;

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Roster {
    pub id: String,
    pub owner: String,
    pub revision: u32,
    pub name: String,
    pub members: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Packet {
    Roster {
        roster: Roster,
    },
    Consent {
        group: String,
        revision: u32,
        accepted: bool,
    },
    Text {
        group: String,
        revision: u32,
        message: String,
        text: String,
    },
    Close {
        group: String,
        revision: u32,
    },
}
impl Packet {
    pub fn group(&self) -> &str {
        match self {
            Self::Roster { roster } => &roster.id,
            Self::Consent { group, .. } | Self::Text { group, .. } | Self::Close { group, .. } => {
                group
            }
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(id(self.group(), 32), "Ungültiger Gruppenverweis.");
        match self {
            Self::Roster { roster } => roster.validate()?,
            Self::Text {
                revision,
                message,
                text,
                ..
            } => {
                ensure!(
                    *revision > 0
                        && id(message, 32)
                        && !text.trim().is_empty()
                        && text.len() <= crate::protocol::MAX_TEXT
                        && !text.contains('\0'),
                    "Ungültige Gruppennachricht."
                );
            }
            Self::Consent { revision, .. } | Self::Close { revision, .. } => {
                ensure!(*revision > 0, "Ungültige Gruppenversion.")
            }
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        self.validate()?;
        let mut bytes = Zeroizing::new(Vec::new());
        serde_json::to_writer(&mut *bytes, self)?;
        ensure!(bytes.len() <= MAX_PACKET, "Gruppenpaket zu lang.");
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= MAX_PACKET, "Gruppenpaket zu lang.");
        let p: Self = serde_json::from_slice(bytes)?;
        p.validate()?;
        Ok(p)
    }
}
impl Roster {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            id(&self.id, 32)
                && id(&self.owner, 64)
                && self.revision > 0
                && self.revision <= 10_000
                && !self.name.trim().is_empty()
                && self.name.len() <= 80
                && !self.name.chars().any(char::is_control),
            "Ungültige Gruppenliste."
        );
        ensure!(
            (2..=MAX_MEMBERS).contains(&self.members.len()) && self.members.contains(&self.owner),
            "Eine Gruppe benötigt 2 bis 8 Mitglieder einschliesslich Besitzer."
        );
        for (i, m) in self.members.iter().enumerate() {
            ensure!(
                id(m, 64) && !self.members[..i].contains(m),
                "Ungültiges oder doppeltes Mitglied."
            );
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub roster: Roster,
    pub active: bool,
    pub closed: bool,
    pub accepted: Vec<String>,
    pub departed: Vec<String>,
    pub unread: u32,
}
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub id: String,
    pub group: String,
    pub sender: String,
    pub text: String,
    pub timestamp_ms: u64,
    pub recipients: Vec<String>,
    pub delivered: Vec<String>,
    #[serde(default)]
    pub cancelled: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub id: String,
    pub recipient: String,
    pub packet: Packet,
}
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub id: String,
    pub sender: String,
    pub digest: [u8; 32],
}
#[derive(Clone, Default, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Store {
    pub groups: Vec<Group>,
    pub messages: Vec<Message>,
    pub outbox: Vec<Delivery>,
    pub receipts: Vec<Receipt>,
}
pub fn id(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn fresh() -> Result<String> {
    let mut b = [0; 16];
    OsRng.try_fill_bytes(&mut b)?;
    Ok(hex::encode(b))
}
fn others(r: &Roster, local: &str) -> Vec<String> {
    r.members
        .iter()
        .filter(|m| m.as_str() != local)
        .cloned()
        .collect()
}
impl Store {
    pub fn validate(&self, local: &str) -> Result<()> {
        ensure!(
            self.groups.len() <= MAX_GROUPS
                && self.messages.len() <= crate::vault::MAX_MESSAGES
                && self.outbox.len() <= MAX_OUTBOX
                && self.receipts.len() <= MAX_RECEIPTS,
            "Gruppenspeicher ist voll."
        );
        for (i, g) in self.groups.iter().enumerate() {
            g.roster.validate()?;
            ensure!(
                g.roster.members.iter().any(|m| m == local)
                    && !self.groups[..i].iter().any(|p| p.roster.id == g.roster.id)
                    && !(g.active && g.closed)
                    && g.unread <= crate::vault::MAX_MESSAGES as u32,
                "Ungültiger Gruppenstatus."
            );
            for list in [&g.accepted, &g.departed] {
                for (j, m) in list.iter().enumerate() {
                    ensure!(
                        g.roster.members.contains(m) && !list[..j].contains(m),
                        "Ungültiger Beitrittsstatus."
                    );
                }
            }
            ensure!(
                !g.active || g.accepted.iter().any(|m| m == local),
                "Fehlender eigener Beitritt."
            );
        }
        for (i, m) in self.messages.iter().enumerate() {
            ensure!(
                id(&m.id, 32)
                    && id(&m.sender, 64)
                    && self.groups.iter().any(|g| g.roster.id == m.group)
                    && !m.text.trim().is_empty()
                    && m.text.len() <= crate::protocol::MAX_TEXT
                    && !m.text.contains('\0')
                    && !self.messages[..i]
                        .iter()
                        .any(|p| p.group == m.group && p.sender == m.sender && p.id == m.id),
                "Ungültiger Gruppenverlauf."
            );
            ensure!(
                m.recipients.len() < MAX_MEMBERS && m.delivered.len() <= m.recipients.len(),
                "Ungültiger Zustellstatus."
            );
            for (j, p) in m.recipients.iter().enumerate() {
                ensure!(
                    id(p, 64) && !m.recipients[..j].contains(p),
                    "Ungültiger Empfänger."
                );
            }
            for (j, p) in m.delivered.iter().enumerate() {
                ensure!(
                    m.recipients.contains(p) && !m.delivered[..j].contains(p),
                    "Ungültige Empfangsbestätigung."
                );
            }
        }
        for m in &self.messages {
            ensure!(
                m.cancelled.len() + m.delivered.len() <= m.recipients.len(),
                "Ungültige abgebrochene Zustellung."
            );
            for (i, peer) in m.cancelled.iter().enumerate() {
                ensure!(
                    m.recipients.contains(peer)
                        && !m.delivered.contains(peer)
                        && !m.cancelled[..i].contains(peer),
                    "Ungültiger Zustellabbruch."
                );
            }
        }
        for (i, d) in self.outbox.iter().enumerate() {
            d.packet.encode()?;
            ensure!(
                id(&d.id, 32)
                    && id(&d.recipient, 64)
                    && self.groups.iter().any(|g| g.roster.id == d.packet.group())
                    && !self.outbox[..i].iter().any(|p| p.id == d.id),
                "Ungültige Gruppenwarteschlange."
            );
        }
        for (i, r) in self.receipts.iter().enumerate() {
            ensure!(
                id(&r.id, 32)
                    && id(&r.sender, 64)
                    && !self.receipts[..i]
                        .iter()
                        .any(|p| p.id == r.id && p.sender == r.sender),
                "Ungültiger Gruppennachweis."
            );
        }
        Ok(())
    }
    fn cancel_where(&mut self, keep: impl Fn(&Delivery) -> bool) {
        let messages = &mut self.messages;
        self.outbox.retain(|d| {
            if keep(d) {
                return true;
            }
            if let Packet::Text { group, message, .. } = &d.packet
                && let Some(m) = messages
                    .iter_mut()
                    .find(|m| m.group == *group && m.id == *message)
                && !m.delivered.contains(&d.recipient)
                && !m.cancelled.contains(&d.recipient)
            {
                m.cancelled.push(d.recipient.clone());
            }
            false
        });
    }
    fn enqueue(&mut self, recipient: String, packet: Packet) -> Result<()> {
        packet.encode()?;
        ensure!(
            self.outbox.len() < MAX_OUTBOX,
            "Gruppenwarteschlange voll. Zuerst synchronisieren."
        );
        self.outbox.push(Delivery {
            id: fresh()?,
            recipient,
            packet,
        });
        Ok(())
    }
    pub fn create(
        &mut self,
        local: &str,
        name: String,
        mut members: Vec<String>,
    ) -> Result<String> {
        ensure!(self.groups.len() < MAX_GROUPS, "Zu viele Gruppen.");
        members.push(local.into());
        members.sort();
        let roster = Roster {
            id: fresh()?,
            owner: local.into(),
            revision: 1,
            name,
            members,
        };
        roster.validate()?;
        ensure!(
            self.outbox.len() + roster.members.len() - 1 <= MAX_OUTBOX,
            "Gruppenwarteschlange voll."
        );
        for member in others(&roster, local) {
            self.enqueue(
                member,
                Packet::Roster {
                    roster: roster.clone(),
                },
            )?;
        }
        let group = roster.id.clone();
        self.groups.push(Group {
            roster,
            active: true,
            closed: false,
            accepted: vec![local.into()],
            departed: vec![],
            unread: 0,
        });
        Ok(group)
    }
    pub fn update(
        &mut self,
        local: &str,
        group: &str,
        name: String,
        mut members: Vec<String>,
    ) -> Result<()> {
        let g = self
            .groups
            .iter()
            .find(|g| g.roster.id == group)
            .ok_or_else(|| anyhow::anyhow!("Gruppe fehlt."))?;
        ensure!(
            g.active && !g.closed && g.roster.owner == local,
            "Nur der aktive Besitzer darf Mitglieder ändern."
        );
        members.push(local.into());
        members.sort();
        let old = g.roster.clone();
        let roster = Roster {
            id: group.into(),
            owner: local.into(),
            revision: old.revision + 1,
            name,
            members,
        };
        roster.validate()?;
        ensure!(
            self.outbox.len() + old.members.len() + roster.members.len() <= MAX_OUTBOX,
            "Gruppenwarteschlange voll."
        );
        self.cancel_where(|d| d.packet.group() != group);
        for removed in old.members.iter().filter(|m| !roster.members.contains(m)) {
            self.enqueue(
                removed.clone(),
                Packet::Close {
                    group: group.into(),
                    revision: old.revision,
                },
            )?;
        }
        for m in others(&roster, local) {
            self.enqueue(
                m,
                Packet::Roster {
                    roster: roster.clone(),
                },
            )?;
        }
        let g = self
            .groups
            .iter_mut()
            .find(|g| g.roster.id == group)
            .ok_or_else(|| anyhow::anyhow!("Gruppe fehlt."))?;
        g.roster = roster;
        g.accepted = vec![local.into()];
        g.departed.clear();
        Ok(())
    }
    pub fn consent(&mut self, local: &str, group: &str, accept: bool) -> Result<()> {
        let g = self
            .groups
            .iter()
            .find(|g| g.roster.id == group)
            .ok_or_else(|| anyhow::anyhow!("Gruppe fehlt."))?;
        ensure!(
            !g.closed && g.roster.owner != local,
            "Besitzer schliesst die Gruppe stattdessen."
        );
        let roster = g.roster.clone();
        ensure!(
            self.outbox.len() + roster.members.len() <= MAX_OUTBOX,
            "Gruppenwarteschlange voll."
        );
        self.cancel_where(|d| {
            !(d.packet.group() == group
                && matches!(d.packet, Packet::Text { .. } | Packet::Consent { .. }))
        });
        for m in others(&roster, local) {
            self.enqueue(
                m,
                Packet::Consent {
                    group: group.into(),
                    revision: roster.revision,
                    accepted: accept,
                },
            )?;
        }
        let g = self
            .groups
            .iter_mut()
            .find(|g| g.roster.id == group)
            .ok_or_else(|| anyhow::anyhow!("Gruppe fehlt."))?;
        g.active = accept;
        g.accepted.retain(|m| m != local);
        g.departed.retain(|m| m != local);
        if accept {
            g.accepted.push(local.into());
        } else {
            g.departed.push(local.into());
        }
        Ok(())
    }
    pub fn close(&mut self, local: &str, group: &str) -> Result<()> {
        let g = self
            .groups
            .iter()
            .find(|g| g.roster.id == group)
            .ok_or_else(|| anyhow::anyhow!("Gruppe fehlt."))?;
        ensure!(
            g.roster.owner == local && !g.closed,
            "Nur der Besitzer kann schliessen."
        );
        let roster = g.roster.clone();
        ensure!(
            self.outbox.len() + roster.members.len() <= MAX_OUTBOX,
            "Gruppenwarteschlange voll."
        );
        self.cancel_where(|d| {
            !(d.packet.group() == group && matches!(d.packet, Packet::Text { .. }))
        });
        for m in others(&roster, local) {
            self.enqueue(
                m,
                Packet::Close {
                    group: group.into(),
                    revision: roster.revision,
                },
            )?;
        }
        if let Some(g) = self.groups.iter_mut().find(|g| g.roster.id == group) {
            g.active = false;
            g.closed = true;
        }
        Ok(())
    }
    pub fn send(&mut self, local: &str, group: &str, text: String, now: u64) -> Result<()> {
        let g = self
            .groups
            .iter()
            .find(|g| g.roster.id == group)
            .ok_or_else(|| anyhow::anyhow!("Gruppe fehlt."))?;
        ensure!(g.active && !g.closed, "Zuerst der Gruppe beitreten.");
        let message = fresh()?;
        let packet = Packet::Text {
            group: group.into(),
            revision: g.roster.revision,
            message: message.clone(),
            text: text.clone(),
        };
        packet.validate()?;
        let recipients: Vec<_> = others(&g.roster, local)
            .into_iter()
            .filter(|m| !g.departed.contains(m))
            .collect();
        ensure!(
            !recipients.is_empty()
                && self.outbox.len() + recipients.len() <= MAX_OUTBOX
                && self.messages.len() < crate::vault::MAX_MESSAGES,
            "Keine Empfänger oder Gruppenspeicher voll."
        );
        for m in &recipients {
            self.enqueue(m.clone(), packet.clone())?;
        }
        self.messages.push(Message {
            id: message,
            group: group.into(),
            sender: local.into(),
            text,
            timestamp_ms: now,
            recipients,
            delivered: vec![],
            cancelled: vec![],
        });
        Ok(())
    }
    pub fn eligible(&self, d: &Delivery) -> bool {
        match &d.packet {
            Packet::Text {
                group, revision, ..
            } => self.groups.iter().any(|g| {
                g.roster.id == *group
                    && g.roster.revision == *revision
                    && g.active
                    && !g.closed
                    && g.accepted.contains(&d.recipient)
                    && !g.departed.contains(&d.recipient)
            }),
            _ => true,
        }
    }
    pub fn acknowledge(&mut self, peer: &str, id: &str) -> Result<()> {
        let i = self
            .outbox
            .iter()
            .position(|d| d.id == id && d.recipient == peer)
            .ok_or_else(|| anyhow::anyhow!("Unerwartete Gruppenbestätigung."))?;
        let d = self.outbox.remove(i);
        if let Packet::Text { group, message, .. } = &d.packet
            && let Some(m) = self
                .messages
                .iter_mut()
                .find(|m| m.group == *group && m.id == *message)
            && !m.delivered.iter().any(|p| p == peer)
        {
            m.delivered.push(peer.into());
        }

        Ok(())
    }
    /// Call only for a mutually verified Noise peer, then persist before ACK.
    pub fn receive(
        &mut self,
        local: &str,
        peer: &str,
        delivery: &str,
        packet: &Packet,
        now: u64,
    ) -> Result<bool> {
        packet.validate()?;
        ensure!(
            id(peer, 64) && id(delivery, 32) && peer != local,
            "Ungültiger Absender."
        );
        let digest: [u8; 32] = Sha256::digest(&*packet.encode()?).into();
        if let Some(r) = self
            .receipts
            .iter()
            .find(|r| r.sender == peer && r.id == delivery)
        {
            ensure!(r.digest == digest, "Widersprüchliche Wiederholung.");
            return Ok(false);
        }
        ensure!(
            self.receipts.len() < MAX_RECEIPTS,
            "Gruppenbestätigungen voll."
        );
        match packet {
            Packet::Roster { roster } => {
                ensure!(
                    roster.owner == peer && roster.members.iter().any(|m| m == local),
                    "Gruppenliste nicht vom Besitzer oder nicht für dich."
                );
                if let Some(g) = self.groups.iter_mut().find(|g| g.roster.id == roster.id) {
                    ensure!(
                        g.roster.owner == peer && !g.closed && roster.revision > g.roster.revision,
                        "Unzulässiger Besitzer- oder Versionswechsel."
                    );
                    g.roster = roster.clone();
                    g.active = false;
                    g.accepted = vec![peer.into()];
                    g.departed.clear();
                    g.unread = g
                        .unread
                        .saturating_add(1)
                        .min(crate::vault::MAX_MESSAGES as u32);
                    self.cancel_where(|d| d.packet.group() != roster.id);
                } else {
                    ensure!(self.groups.len() < MAX_GROUPS, "Zu viele Gruppen.");
                    self.groups.push(Group {
                        roster: roster.clone(),
                        active: false,
                        closed: false,
                        accepted: vec![peer.into()],
                        departed: vec![],
                        unread: 1,
                    });
                }
            }
            Packet::Consent {
                group,
                revision,
                accepted,
            } => {
                let g = self
                    .groups
                    .iter_mut()
                    .find(|g| g.roster.id == *group)
                    .ok_or_else(|| anyhow::anyhow!("Unbekannte Gruppe."))?;
                ensure!(
                    !g.closed
                        && g.roster.revision == *revision
                        && g.roster.members.iter().any(|m| m == peer)
                        && g.roster.owner != peer,
                    "Ungültiger Beitritt."
                );
                ensure!(
                    !(*accepted && g.departed.iter().any(|m| m == peer)),
                    "Ausgetretenes Mitglied benötigt neue Einladung."
                );
                g.accepted.retain(|m| m != peer);
                g.departed.retain(|m| m != peer);
                if *accepted {
                    g.accepted.push(peer.into());
                } else {
                    g.departed.push(peer.into());
                }
                if !accepted {
                    self.cancel_where(|d| {
                        !(d.recipient == peer
                            && d.packet.group() == *group
                            && matches!(d.packet, Packet::Text { .. }))
                    });
                }
            }
            Packet::Text {
                group,
                revision,
                message,
                text,
            } => {
                let g = self
                    .groups
                    .iter_mut()
                    .find(|g| g.roster.id == *group)
                    .ok_or_else(|| anyhow::anyhow!("Unbekannte Gruppe."))?;
                ensure!(
                    g.active
                        && !g.closed
                        && g.roster.revision == *revision
                        && g.roster.members.iter().any(|m| m == peer)
                        && g.accepted.iter().any(|m| m == peer)
                        && !g.departed.iter().any(|m| m == peer),
                    "Nachricht ohne gültigen Beitritt."
                );
                ensure!(
                    !self
                        .messages
                        .iter()
                        .any(|m| m.group == *group && m.sender == peer && m.id == *message)
                        && self.messages.len() < crate::vault::MAX_MESSAGES,
                    "Doppelter oder zu grosser Verlauf."
                );
                g.unread = g
                    .unread
                    .saturating_add(1)
                    .min(crate::vault::MAX_MESSAGES as u32);
                self.messages.push(Message {
                    id: message.clone(),
                    group: group.clone(),
                    sender: peer.into(),
                    text: text.clone(),
                    timestamp_ms: now,
                    recipients: vec![],
                    delivered: vec![],
                    cancelled: vec![],
                });
            }
            Packet::Close { group, revision } => {
                if let Some(g) = self.groups.iter_mut().find(|g| g.roster.id == *group) {
                    ensure!(
                        g.roster.owner == peer && *revision >= g.roster.revision,
                        "Ungültiges Schliessen."
                    );
                    g.active = false;
                    g.closed = true;
                    self.cancel_where(|d| d.packet.group() != *group);
                }
            }
        }
        self.receipts.push(Receipt {
            id: delivery.into(),
            sender: peer.into(),
            digest,
        });
        Ok(true)
    }
    pub fn can_join(&self, group: &str, local: &str, known: &[String]) -> bool {
        self.groups
            .iter()
            .find(|g| g.roster.id == group)
            .is_some_and(|g| {
                !g.closed
                    && !g.departed.iter().any(|m| m == local)
                    && g.roster
                        .members
                        .iter()
                        .all(|m| m == local || known.contains(m))
            })
    }
}

#[cfg(test)]
mod tests;
