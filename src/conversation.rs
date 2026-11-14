//! Bounded authenticated conversation operations; no networking or storage here.
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};
pub const REACTIONS: &[&str] = &["", "👍", "❤️", "😂", "🎉", "😮", "😢", "✅"];
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Action {
    Reply {
        target: String,
        text: String,
    },
    Edit {
        target: String,
        revision: u32,
        text: String,
    },
    React {
        target: String,
        revision: u32,
        reaction: String,
    },
}
impl Action {
    pub fn target(&self) -> &str {
        match self {
            Self::Reply { target, .. } | Self::Edit { target, .. } | Self::React { target, .. } => {
                target
            }
        }
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.target().len() == 32 && hex::decode(self.target()).is_ok(),
            "Ungültiger Nachrichtenverweis."
        );
        match self {
            Self::Reply { text, .. } | Self::Edit { text, .. } => anyhow::ensure!(
                !text.trim().is_empty()
                    && text.len() <= crate::protocol::MAX_TEXT
                    && !text.contains('\0'),
                "Nachricht ist leer oder zu lang."
            ),
            Self::React { reaction, .. } => anyhow::ensure!(
                REACTIONS.contains(&reaction.as_str()),
                "Unzulässige Reaktion."
            ),
        }
        if let Self::Edit { revision, .. } | Self::React { revision, .. } = self {
            anyhow::ensure!((1..=1000).contains(revision), "Änderungsgrenze erreicht.");
        }
        Ok(())
    }
    pub fn encode(&self) -> anyhow::Result<Zeroizing<Vec<u8>>> {
        self.validate()?;
        let mut bytes = Zeroizing::new(Vec::new());
        bytes.push(match self {
            Self::Reply { .. } => 1,
            Self::Edit { .. } => 2,
            Self::React { .. } => 3,
        });
        bytes.extend(hex::decode(self.target())?);
        match self {
            Self::Reply { text, .. } => bytes.extend(text.as_bytes()),
            Self::Edit { revision, text, .. } => {
                bytes.extend(revision.to_be_bytes());
                bytes.extend(text.as_bytes());
            }
            Self::React {
                revision, reaction, ..
            } => {
                bytes.extend(revision.to_be_bytes());
                bytes.extend(reaction.as_bytes());
            }
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            bytes.len() >= 17 && bytes.len() <= crate::protocol::MAX_TEXT + 21,
            "Ungültige Aktion."
        );
        let target = hex::encode(&bytes[1..17]);
        let action = match bytes[0] {
            1 => Self::Reply {
                target,
                text: std::str::from_utf8(&bytes[17..])?.into(),
            },
            2 | 3 => {
                anyhow::ensure!(bytes.len() >= 21, "Ungültige Aktion.");
                let revision = u32::from_be_bytes(bytes[17..21].try_into()?);
                let text = std::str::from_utf8(&bytes[21..])?.to_owned();
                if bytes[0] == 2 {
                    Self::Edit {
                        target,
                        revision,
                        text,
                    }
                } else {
                    Self::React {
                        target,
                        revision,
                        reaction: text,
                    }
                }
            }
            _ => anyhow::bail!("Unbekannte Aktion."),
        };
        action.validate()?;
        Ok(action)
    }
}
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct PendingAction {
    pub id: String,
    pub contact_id: String,
    pub action: Action,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_actions_roundtrip_unicode_and_reject_invalid_revisions() -> anyhow::Result<()> {
        let actions = [
            Action::Reply {
                target: hex::encode([3; 16]),
                text: "Grüezi 🦀\nAntwort".into(),
            },
            Action::Edit {
                target: hex::encode([3; 16]),
                revision: 2,
                text: "Änderung".into(),
            },
            Action::React {
                target: hex::encode([3; 16]),
                revision: 1,
                reaction: "❤️".into(),
            },
        ];
        for action in actions {
            let bytes = action.encode()?;
            let decoded = Action::decode(&bytes)?;
            assert_eq!(*decoded.encode()?, *bytes);
        }
        for bytes in [
            vec![],
            vec![2; 20],
            vec![9; 30],
            vec![1; crate::protocol::MAX_TEXT + 22],
        ] {
            assert!(Action::decode(&bytes).is_err());
        }
        assert!(
            Action::Edit {
                target: hex::encode([0; 16]),
                revision: 0,
                text: "synthetic".into()
            }
            .validate()
            .is_err()
        );
        Ok(())
    }
}

/// Receipts survive reconnects without retaining cipher state. The digest binds
/// the entire operation; a reused ID carrying different data is rejected.
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub id: String,
    pub contact_id: String,
    pub digest: [u8; 32],
}

pub fn apply(
    profile: &mut crate::vault::Profile,
    contact: &str,
    id: &str,
    action: &Action,
    outgoing: bool,
    now: u64,
) -> anyhow::Result<bool> {
    use crate::vault::{MAX_MESSAGES, MessageStatus, StoredMessage};
    use sha2::{Digest, Sha256};
    action.validate()?;
    anyhow::ensure!(
        id.len() == 32
            && hex::decode(id).is_ok()
            && profile.contacts.iter().any(|c| c.id == contact),
        "Ungültiger Kontakt oder Aktionsverweis."
    );
    let digest: [u8; 32] = Sha256::digest(&*action.encode()?).into();
    if !outgoing && let Some(receipt) = profile.action_receipts.iter().find(|r| r.id == id) {
        anyhow::ensure!(
            receipt.contact_id == contact && receipt.digest == digest,
            "Widersprüchlich wiederholte Aktion."
        );
        return Ok(false);
    }
    anyhow::ensure!(
        !profile.messages.iter().any(|m| m.id == id)
            && !profile.pending_actions.iter().any(|p| p.id == id)
            && !profile.action_receipts.iter().any(|r| r.id == id),
        "Aktionsverweis bereits verwendet."
    );
    if !outgoing {
        anyhow::ensure!(
            profile.action_receipts.len() < 10_000,
            "Bestätigungsgrenze erreicht. Kontakt nach unabhängiger Prüfung entfernen und neu bestätigen oder ein neues Profil verwenden."
        );
    }
    match action {
        Action::Reply { target, text } => {
            anyhow::ensure!(
                profile.messages.len() < MAX_MESSAGES,
                "Verlaufsgrenze erreicht."
            );
            if outgoing {
                anyhow::ensure!(
                    profile
                        .messages
                        .iter()
                        .any(|m| m.id == *target && m.contact_id == contact),
                    "Nachricht zum Antworten fehlt."
                );
            }
            profile.messages.push(StoredMessage {
                id: id.into(),
                contact_id: contact.into(),
                text: text.clone(),
                timestamp_ms: now,
                outgoing,
                status: if outgoing {
                    MessageStatus::Pending
                } else {
                    MessageStatus::Received
                },
                attachment_id: None,
                reply_to: Some(target.clone()),
                edit_revision: 0,
                reaction_local: String::new(),
                reaction_remote: String::new(),
                reaction_local_revision: 0,
                reaction_remote_revision: 0,
                pinned: false,
            });
        }
        Action::Edit {
            target,
            revision,
            text,
        } => {
            let m = profile
                .messages
                .iter_mut()
                .find(|m| m.id == *target && m.contact_id == contact)
                .ok_or_else(|| anyhow::anyhow!("Nachricht zum Bearbeiten fehlt."))?;
            anyhow::ensure!(
                m.outgoing == outgoing
                    && m.attachment_id.is_none()
                    && m.status == MessageStatus::Received
                    && *revision == m.edit_revision + 1,
                "Unzulässige Bearbeitung."
            );
            m.text.zeroize();
            m.text.clone_from(text);
            m.edit_revision = *revision;
        }
        Action::React {
            target,
            revision,
            reaction,
        } => {
            let m = profile
                .messages
                .iter_mut()
                .find(|m| m.id == *target && m.contact_id == contact)
                .ok_or_else(|| anyhow::anyhow!("Nachricht für Reaktion fehlt."))?;
            anyhow::ensure!(
                m.status == MessageStatus::Received,
                "Nachricht noch nicht bestätigt."
            );
            let (value, number) = if outgoing {
                (&mut m.reaction_local, &mut m.reaction_local_revision)
            } else {
                (&mut m.reaction_remote, &mut m.reaction_remote_revision)
            };
            anyhow::ensure!(*revision == *number + 1, "Unzulässige Reaktionsfolge.");
            value.zeroize();
            value.clone_from(reaction);
            *number = *revision;
        }
    }
    if !outgoing {
        profile.action_receipts.push(Receipt {
            id: id.into(),
            contact_id: contact.into(),
            digest,
        });
    }
    Ok(true)
}

#[cfg(test)]
mod state_tests {
    use super::*;
    fn profile() -> anyhow::Result<crate::vault::Profile> {
        let mut p = crate::vault::Profile::new("synthetic profile".into(), [1; 32], [2; 32]);
        p.contacts.push(crate::vault::Contact {
            id: hex::encode([3; 32]),
            display_name: "synthetic contact".into(),
            identity_key: [3; 32],
            bluetooth_address: "synthetic address".into(),
            alias: String::new(),
            favorite: false,
            unread: 0,
            muted_until_ms: 0,
        });
        p.messages.push(serde_json::from_value(serde_json::json!({"id":hex::encode([4;16]),"contact_id":hex::encode([3;32]),"text":"SYNTHETIC_ORIGINAL_04","timestamp_ms":1,"outgoing":true,"status":"Received"}))?);
        Ok(p)
    }
    #[test]
    fn edits_reactions_and_receipts_enforce_ownership_revision_and_full_action_binding()
    -> anyhow::Result<()> {
        let mut p = profile()?;
        let c = hex::encode([3; 32]);
        let target = hex::encode([4; 16]);
        assert!(
            apply(
                &mut p,
                &c,
                &hex::encode([5; 16]),
                &Action::Edit {
                    target: target.clone(),
                    revision: 1,
                    text: "FORGED_REMOTE_EDIT".into()
                },
                false,
                2
            )
            .is_err()
        );
        assert!(
            apply(
                &mut p,
                &c,
                &hex::encode([5; 16]),
                &Action::Edit {
                    target: target.clone(),
                    revision: 2,
                    text: "SKIPPED_REVISION".into()
                },
                true,
                2
            )
            .is_err()
        );
        assert_eq!(p.messages[0].text, "SYNTHETIC_ORIGINAL_04");
        apply(
            &mut p,
            &c,
            &hex::encode([5; 16]),
            &Action::Edit {
                target: target.clone(),
                revision: 1,
                text: "SYNTHETIC_EDIT_04".into(),
            },
            true,
            2,
        )?;
        assert_eq!(p.messages[0].edit_revision, 1);
        let id = hex::encode([6; 16]);
        let action = Action::React {
            target: target.clone(),
            revision: 1,
            reaction: "👍".into(),
        };
        assert!(apply(&mut p, &c, &id, &action, false, 3)?);
        assert!(!apply(&mut p, &c, &id, &action, false, 3)?);
        assert!(
            apply(
                &mut p,
                &c,
                &id,
                &Action::React {
                    target: target.clone(),
                    revision: 1,
                    reaction: "❤️".into()
                },
                false,
                3
            )
            .is_err()
        );
        assert_eq!(p.messages[0].reaction_remote, "👍");
        assert!(apply(&mut p, &c, &hex::encode([7; 16]), &action, false, 3).is_err());
        let reply = Action::Reply {
            target,
            text: "SYNTHETIC_REPLY_04".into(),
        };
        let id = hex::encode([8; 16]);
        apply(&mut p, &c, &id, &reply, false, 4)?;
        p.messages.retain(|m| m.id != id);
        assert!(!apply(&mut p, &c, &id, &reply, false, 4)?);
        assert_eq!(p.messages.len(), 1);
        Ok(())
    }
}
