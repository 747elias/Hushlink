use super::*;
fn member(n: char) -> String {
    n.to_string().repeat(64)
}
fn deliver(from: &mut Store, to: &mut Store, sender: &str, recipient: &str) -> Result<usize> {
    let pending: Vec<_> = from
        .outbox
        .iter()
        .filter(|d| d.recipient == recipient && from.eligible(d))
        .cloned()
        .collect();
    for d in &pending {
        to.receive(recipient, sender, &d.id, &d.packet, 100)?;
        from.acknowledge(recipient, &d.id)?;
    }
    Ok(pending.len())
}
fn trio() -> Result<(Store, Store, Store, String)> {
    let (a, b, c) = (member('a'), member('b'), member('c'));
    let (mut x, mut y, mut z) = (Store::default(), Store::default(), Store::default());
    let id = x.create(
        &a,
        "Synthetische Gruppe 🦀".into(),
        vec![b.clone(), c.clone()],
    )?;
    deliver(&mut x, &mut y, &a, &b)?;
    deliver(&mut x, &mut z, &a, &c)?;
    y.consent(&b, &id, true)?;
    z.consent(&c, &id, true)?;
    deliver(&mut y, &mut x, &b, &a)?;
    deliver(&mut y, &mut z, &b, &c)?;
    deliver(&mut z, &mut x, &c, &a)?;
    deliver(&mut z, &mut y, &c, &b)?;
    Ok((x, y, z, id))
}
#[test]
fn three_members_chat_with_independent_durable_receipts_and_idempotent_reconnect() -> Result<()> {
    let (a, b, c) = (member('a'), member('b'), member('c'));
    let (mut x, mut y, mut z, g) = trio()?;
    x.send(&a, &g, "Grüezi 🦀\nGruppen-Nachricht".into(), 200)?;
    let retry = x
        .outbox
        .iter()
        .find(|d| d.recipient == b)
        .ok_or_else(|| anyhow::anyhow!("test delivery"))?
        .clone();
    assert_eq!(deliver(&mut x, &mut y, &a, &b)?, 1);
    assert_eq!(x.messages[0].delivered, vec![b.clone()]);
    assert!(!y.receive(&b, &a, &retry.id, &retry.packet, 201)?);
    assert_eq!(y.messages.len(), 1);
    let mut restored: Store = serde_json::from_slice(&serde_json::to_vec(&y)?)?;
    assert!(!restored.receive(&b, &a, &retry.id, &retry.packet, 202)?);
    deliver(&mut x, &mut z, &a, &c)?;
    assert_eq!(x.messages[0].delivered.len(), 2);
    y.send(&b, &g, "Antwort von B".into(), 300)?;
    deliver(&mut y, &mut x, &b, &a)?;
    deliver(&mut y, &mut z, &b, &c)?;
    assert_eq!(z.messages[1].sender, b);
    assert_eq!(x.messages[1].text, "Antwort von B");
    x.validate(&a)?;
    y.validate(&b)?;
    z.validate(&c)?;
    Ok(())
}
#[test]
fn membership_consent_unknown_contacts_sender_spoof_and_key_replacement_are_blocked() -> Result<()>
{
    let (a, b, c, d) = (member('a'), member('b'), member('c'), member('d'));
    let (mut x, mut y) = (Store::default(), Store::default());
    let g = x.create(&a, "Test".into(), vec![b.clone(), c.clone()])?;
    deliver(&mut x, &mut y, &a, &b)?;
    assert!(!y.can_join(&g, &b, std::slice::from_ref(&a)));
    assert!(y.can_join(&g, &b, &[a.clone(), c]));
    let packet = Packet::Text {
        group: g.clone(),
        revision: 1,
        message: "1".repeat(32),
        text: "NICHT_FREIGEBEN".into(),
    };
    assert!(y.receive(&b, &a, &"2".repeat(32), &packet, 0).is_err());
    assert!(y.messages.is_empty());
    y.consent(&b, &g, true)?;
    assert!(y.receive(&b, &d, &"3".repeat(32), &packet, 0).is_err());
    let forged = Packet::Roster {
        roster: Roster {
            id: g,
            owner: d.clone(),
            revision: 2,
            name: "Falsch".into(),
            members: vec![d.clone(), b.clone()],
        },
    };
    assert!(y.receive(&b, &d, &"4".repeat(32), &forged, 0).is_err());
    assert_eq!(y.groups[0].roster.owner, a);
    Ok(())
}
#[test]
fn removing_member_and_leaving_blocks_new_delivery_and_old_epochs() -> Result<()> {
    let (a, b, c) = (member('a'), member('b'), member('c'));
    let (mut x, mut y, mut z, g) = trio()?;
    x.send(&a, &g, "Vor Änderung".into(), 0)?;
    let old = x
        .outbox
        .iter()
        .find(|d| d.recipient == b)
        .ok_or_else(|| anyhow::anyhow!("test delivery"))?
        .clone();
    x.update(&a, &g, "Neue Liste".into(), vec![b.clone()])?;
    deliver(&mut x, &mut y, &a, &b)?;
    deliver(&mut x, &mut z, &a, &c)?;
    assert!(z.groups[0].closed);
    assert!(!y.groups[0].active);
    assert!(y.receive(&b, &a, &old.id, &old.packet, 0).is_err());
    y.consent(&b, &g, true)?;
    deliver(&mut y, &mut x, &b, &a)?;
    x.send(&a, &g, "Neue Version".into(), 1)?;
    assert!(
        !x.outbox
            .iter()
            .any(|d| d.recipient == c && matches!(d.packet, Packet::Text { .. }))
    );
    y.consent(&b, &g, false)?;
    deliver(&mut y, &mut x, &b, &a)?;
    assert!(
        !x.outbox
            .iter()
            .any(|d| d.recipient == b && matches!(d.packet, Packet::Text { .. }))
    );
    assert!(
        y.receive(
            &b,
            &a,
            &"5".repeat(32),
            &Packet::Text {
                group: g,
                revision: 2,
                message: "6".repeat(32),
                text: "Nach Austritt".into()
            },
            2
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn tampered_retries_history_deletion_and_parser_limits() -> Result<()> {
    let (a, b) = (member('a'), member('b'));
    let (mut x, mut y, _, g) = trio()?;
    x.send(&a, &g, "Eindeutiger Text".into(), 0)?;
    let packet = x
        .outbox
        .iter()
        .find(|d| d.recipient == b)
        .ok_or_else(|| anyhow::anyhow!("test delivery"))?
        .clone();
    y.receive(&b, &a, &packet.id, &packet.packet, 0)?;
    y.messages.clear();
    assert!(!y.receive(&b, &a, &packet.id, &packet.packet, 1)?);
    assert!(y.messages.is_empty());
    let other = Packet::Text {
        group: g,
        revision: 1,
        message: "7".repeat(32),
        text: "Manipuliert".into(),
    };
    assert!(y.receive(&b, &a, &packet.id, &other, 1).is_err());
    assert!(Packet::decode(&vec![b'x'; MAX_PACKET + 1]).is_err());
    assert!(Packet::decode(br#"{"kind":"Close","group":"bad","revision":1}"#).is_err());
    let mut roster = x.groups[0].roster.clone();
    roster.members.push(b);
    assert!(roster.validate().is_err());
    let escaped = Packet::Text {
        group: x.groups[0].roster.id.clone(),
        revision: 1,
        message: "9".repeat(32),
        text: "\u{1}".repeat(crate::protocol::MAX_TEXT),
    };
    assert!(escaped.encode().is_err());
    Ok(())
}

#[test]
fn offline_members_can_skip_roster_snapshots_but_not_roll_back_or_resurrect_cancelled_deliveries()
-> Result<()> {
    let (a, b) = (member('a'), member('b'));
    let (mut x, mut y, _, g) = trio()?;
    x.send(&a, &g, "Nicht mehr übertragen".into(), 0)?;
    x.update(&a, &g, "Liste 2".into(), vec![b.clone()])?;
    assert_eq!(x.messages[0].cancelled.len(), 2);
    x.update(&a, &g, "Liste 3".into(), vec![b.clone()])?;
    deliver(&mut x, &mut y, &a, &b)?;
    assert_eq!(y.groups[0].roster.revision, 3);
    assert!(!y.groups[0].active);
    let mut old = y.groups[0].roster.clone();
    old.revision = 2;
    assert!(
        y.receive(&b, &a, &"a".repeat(32), &Packet::Roster { roster: old }, 0)
            .is_err()
    );
    x.validate(&a)?;
    y.validate(&b)?;
    Ok(())
}
