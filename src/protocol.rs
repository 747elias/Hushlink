//! Bounded Noise XX sessions. No networking or persistence is performed here.
use std::{
    collections::HashSet,
    io::{Read, Write},
};

use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use snow::{
    params::{CipherChoice, DHChoice, HashChoice},
    resolvers::{CryptoResolver, DefaultResolver},
    types::{Cipher, Dh, Hash, Random},
};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

pub const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
pub const MAX_FRAME: usize = 32_768;
pub const MAX_TEXT: usize = 16_384;
pub const MAX_NAME: usize = 80;
pub const MAX_SESSION_MESSAGES: usize = 4_096;
const CONTEXT: &[u8] = b"Hushlink/v1/RFCOMM/Noise_XX_25519_ChaChaPoly_SHA256";
const MAGIC: &[u8; 5] = b"HUSH\x01";
const VERSION: u8 = 1;
pub type MessageId = [u8; 16];
pub type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Die Verbindung wurde unterbrochen oder hat das Zeitlimit erreicht.")]
    Io(#[from] std::io::Error),
    #[error("Die sichere Verbindung konnte nicht hergestellt werden.")]
    Crypto,
    #[error("Ungültige oder manipulierte Nachricht. Die Verbindung wurde geschlossen.")]
    InvalidMessage,
    #[error("Die Verbindung ist noch nicht beidseitig bestätigt.")]
    NotReady,
    #[error("Die Identität dieses Kontakts hat sich geändert. Erneute Prüfung erforderlich.")]
    KeyChanged,
    #[error("Die Verbindung ist geschlossen. Bitte neu verbinden.")]
    Closed,
    #[error("Text ist leer oder zu lang (höchstens 16 KiB); Namen erlauben bis zu 80 UTF-8-Bytes.")]
    InvalidText,
    #[error("Die Nachrichtengrenze dieser Sitzung ist erreicht. Bitte neu verbinden.")]
    SessionLimit,
}

// snow 0.10's default primitives contain private key arrays without Drop wiping.
// These adapters delegate every cryptographic operation unchanged and overwrite
// their key slots using the library API before the boxed primitive is released.
// This is best effort: snow's internal handshake transcript/chaining buffers,
// compiler/OS copies and temporary primitive internals are outside our control.
struct ClearingResolver;
impl CryptoResolver for ClearingResolver {
    fn resolve_rng(&self) -> Option<Box<dyn Random>> {
        DefaultResolver.resolve_rng()
    }
    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<dyn Dh>> {
        DefaultResolver
            .resolve_dh(choice)
            .map(|inner| Box::new(ClearingDh(inner)) as Box<dyn Dh>)
    }
    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<dyn Hash>> {
        DefaultResolver.resolve_hash(choice)
    }
    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<dyn Cipher>> {
        DefaultResolver
            .resolve_cipher(choice)
            .map(|inner| Box::new(ClearingCipher(inner)) as Box<dyn Cipher>)
    }
}
struct ClearingDh(Box<dyn Dh>);
impl Dh for ClearingDh {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn pub_len(&self) -> usize {
        self.0.pub_len()
    }
    fn priv_len(&self) -> usize {
        self.0.priv_len()
    }
    fn set(&mut self, private: &[u8]) {
        self.0.set(private);
    }
    fn generate(&mut self, rng: &mut dyn Random) -> std::result::Result<(), snow::Error> {
        self.0.generate(rng)
    }
    fn pubkey(&self) -> &[u8] {
        self.0.pubkey()
    }
    fn privkey(&self) -> &[u8] {
        self.0.privkey()
    }
    fn dh(&self, public: &[u8], output: &mut [u8]) -> std::result::Result<(), snow::Error> {
        self.0.dh(public, output)
    }
}
impl Drop for ClearingDh {
    fn drop(&mut self) {
        self.0.set(&[0_u8; 32]);
        std::hint::black_box(&mut self.0);
    }
}
struct ClearingCipher(Box<dyn Cipher>);
impl Cipher for ClearingCipher {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn set(&mut self, key: &[u8; 32]) {
        self.0.set(key);
    }
    fn encrypt(&self, nonce: u64, authtext: &[u8], plaintext: &[u8], output: &mut [u8]) -> usize {
        self.0.encrypt(nonce, authtext, plaintext, output)
    }
    fn decrypt(
        &self,
        nonce: u64,
        authtext: &[u8],
        ciphertext: &[u8],
        output: &mut [u8],
    ) -> std::result::Result<usize, snow::Error> {
        self.0.decrypt(nonce, authtext, ciphertext, output)
    }
    fn rekey(&mut self) {
        self.0.rekey();
    }
}
impl Drop for ClearingCipher {
    fn drop(&mut self) {
        self.0.set(&[0_u8; 32]);
        std::hint::black_box(&mut self.0);
    }
}

/// A long-lived identity. Secret bytes are cleared on drop; no secret Debug/Clone.
pub struct IdentityKey {
    secret: Zeroizing<[u8; 32]>,
    public: [u8; 32],
}

impl IdentityKey {
    pub fn generate() -> Result<Self> {
        let mut secret = Zeroizing::new([0_u8; 32]);
        OsRng
            .try_fill_bytes(secret.as_mut())
            .map_err(|_| ProtocolError::Crypto)?;
        Self::from_private(&secret)
    }

    pub fn from_private(secret: &[u8; 32]) -> Result<Self> {
        let mut dh = ClearingResolver
            .resolve_dh(&DHChoice::Curve25519)
            .ok_or(ProtocolError::Crypto)?;
        dh.set(secret);
        let public = dh.pubkey().try_into().map_err(|_| ProtocolError::Crypto)?;
        Ok(Self {
            secret: Zeroizing::new(*secret),
            public,
        })
    }

    pub fn private(&self) -> &[u8; 32] {
        &self.secret
    }
    pub fn public(&self) -> [u8; 32] {
        self.public
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Initiator,
    Responder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trust {
    New,
    Known,
}

/// Contains application plaintext. Owners must clear it when locking.
#[derive(Zeroize)]
pub enum Event {
    Hello { name: String },
    TrustConfirmed,
    Chat { id: MessageId, text: String },
    Ack { id: MessageId },
    Closed,
}

/// This consumes fresh ephemeral keys on every call. The stream must impose
/// bounded handshake reads/writes and a total handshake deadline.
pub fn handshake<S: Read + Write>(
    stream: &mut S,
    identity: &IdentityKey,
    role: Role,
) -> Result<Session> {
    stream.write_all(MAGIC)?;
    stream.flush()?;
    let mut magic = [0_u8; 5];
    stream.read_exact(&mut magic)?;
    if magic != *MAGIC {
        return Err(ProtocolError::InvalidMessage);
    }
    let params = NOISE_PATTERN.parse().map_err(|_| ProtocolError::Crypto)?;
    let builder = snow::Builder::with_resolver(params, Box::new(ClearingResolver))
        .local_private_key(identity.private())
        .map_err(|_| ProtocolError::Crypto)?
        .prologue(CONTEXT)
        .map_err(|_| ProtocolError::Crypto)?;
    let mut noise = match role {
        Role::Initiator => builder.build_initiator(),
        Role::Responder => builder.build_responder(),
    }
    .map_err(|_| ProtocolError::Crypto)?;
    // XX has exactly three messages and no application handshake payloads.
    let mut scratch = Zeroizing::new(vec![0_u8; MAX_FRAME]);
    for index in 0..3 {
        let sending = match role {
            Role::Initiator => index % 2 == 0,
            Role::Responder => index % 2 != 0,
        };
        if sending {
            let length = noise
                .write_message(&[], &mut scratch)
                .map_err(|_| ProtocolError::Crypto)?;
            write_frame(stream, &scratch[..length])?;
        } else {
            let frame = read_frame(stream)?;
            let length = noise
                .read_message(&frame, &mut scratch)
                .map_err(|_| ProtocolError::Crypto)?;
            if length != 0 {
                return Err(ProtocolError::InvalidMessage);
            }
        }
    }
    let remote = noise
        .get_remote_static()
        .ok_or(ProtocolError::Crypto)?
        .try_into()
        .map_err(|_| ProtocolError::Crypto)?;
    if identity.public() == remote {
        return Err(ProtocolError::InvalidMessage);
    }
    let noise = noise
        .into_transport_mode()
        .map_err(|_| ProtocolError::Crypto)?;
    Ok(Session {
        noise: Some(noise),
        local_key: identity.public(),
        remote_key: remote,
        hello_sent: false,
        hello_received: false,
        contact_checked: false,
        local_confirmed: false,
        remote_confirmed: false,
        sent: HashSet::new(),
        received: HashSet::new(),
        pending_ack: HashSet::new(),
        awaiting_ack: HashSet::new(),
    })
}

pub fn read_frame<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    reader.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > MAX_FRAME {
        return Err(ProtocolError::InvalidMessage);
    }
    let mut frame = vec![0_u8; length];
    reader.read_exact(&mut frame)?;
    Ok(frame)
}

pub fn write_frame<W: Write>(writer: &mut W, frame: &[u8]) -> Result<()> {
    if frame.is_empty() || frame.len() > MAX_FRAME {
        return Err(ProtocolError::InvalidMessage);
    }
    writer.write_all(&(frame.len() as u32).to_be_bytes())?;
    writer.write_all(frame)?;
    writer.flush()?;
    Ok(())
}

/// Full 256-bit SHA-256 digest in conventional hexadecimal groups. Both peers
/// hash the same ordered identities and protocol context, never a short PIN.
pub fn security_fingerprint(first: &[u8; 32], second: &[u8; 32]) -> String {
    let (low, high) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    let mut hash = Sha256::new();
    hash.update(b"Hushlink identity fingerprint\0v1\0");
    hash.update(CONTEXT);
    hash.update(low);
    hash.update(high);
    let encoded = hex::encode_upper(hash.finalize());
    encoded
        .as_bytes()
        .chunks(4)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Session keys and nonce counters live exclusively inside snow. There is no
/// serialization, cloning, counter reset or reconnect/resume API.
pub struct Session {
    noise: Option<snow::TransportState>,
    local_key: [u8; 32],
    remote_key: [u8; 32],
    hello_sent: bool,
    hello_received: bool,
    contact_checked: bool,
    local_confirmed: bool,
    remote_confirmed: bool,
    sent: HashSet<MessageId>,
    received: HashSet<MessageId>,
    pending_ack: HashSet<MessageId>,
    awaiting_ack: HashSet<MessageId>,
}

impl Session {
    pub fn remote_key(&self) -> [u8; 32] {
        self.remote_key
    }
    pub fn fingerprint(&self) -> String {
        security_fingerprint(&self.local_key, &self.remote_key)
    }
    pub fn is_ready(&self) -> bool {
        self.noise.is_some()
            && self.hello_sent
            && self.hello_received
            && self.contact_checked
            && self.local_confirmed
            && self.remote_confirmed
    }
    pub fn is_closed(&self) -> bool {
        self.noise.is_none()
    }

    /// Compare the selected stored contact's identity before confirmation.
    /// A key change permanently kills this session. Forgetting/resetting a
    /// contact requires an explicit UI action and an entirely new handshake.
    pub fn check_contact(&mut self, expected: Option<[u8; 32]>) -> Result<Trust> {
        self.ensure_open()?;
        if self.contact_checked {
            return self.fail(ProtocolError::InvalidMessage);
        }
        if let Some(expected) = expected {
            if expected != self.remote_key {
                return self.fail(ProtocolError::KeyChanged);
            }
            self.contact_checked = true;
            Ok(Trust::Known)
        } else {
            self.contact_checked = true;
            Ok(Trust::New)
        }
    }

    pub fn hello(&mut self, name: &str) -> Result<Vec<u8>> {
        self.ensure_open()?;
        if self.hello_sent {
            return Err(ProtocolError::InvalidMessage);
        }
        validate_name(name)?;
        let mut plaintext = Zeroizing::new(vec![VERSION, 1]);
        plaintext.extend_from_slice(name.as_bytes());
        let result = self.encrypt(&plaintext)?;
        self.hello_sent = true;
        Ok(result)
    }

    pub fn confirm(&mut self) -> Result<Vec<u8>> {
        self.ensure_open()?;
        if !self.hello_sent || !self.hello_received || !self.contact_checked {
            return Err(ProtocolError::NotReady);
        }
        if self.local_confirmed {
            return Err(ProtocolError::InvalidMessage);
        }
        let result = self.encrypt(&[VERSION, 2])?;
        self.local_confirmed = true;
        Ok(result)
    }

    pub fn send_chat(&mut self, text: &str) -> Result<(MessageId, Vec<u8>)> {
        self.ensure_ready()?;
        validate_text(text)?;
        if self.sent.len() >= MAX_SESSION_MESSAGES {
            return Err(ProtocolError::SessionLimit);
        }
        let mut id = [0_u8; 16];
        OsRng
            .try_fill_bytes(&mut id)
            .map_err(|_| ProtocolError::Crypto)?;
        if self.sent.contains(&id) {
            return self.fail(ProtocolError::Crypto);
        }
        let mut plaintext = Zeroizing::new(vec![VERSION, 3]);
        plaintext.extend_from_slice(&id);
        plaintext.extend_from_slice(text.as_bytes());
        let result = self.encrypt(&plaintext)?;
        self.sent.insert(id);
        self.awaiting_ack.insert(id);
        Ok((id, result))
    }

    /// Call only after durable encrypted persistence of the received chat.
    pub fn acknowledge(&mut self, id: MessageId) -> Result<Vec<u8>> {
        self.ensure_ready()?;
        if !self.pending_ack.remove(&id) {
            return Err(ProtocolError::InvalidMessage);
        }
        let mut plaintext = Zeroizing::new(vec![VERSION, 4]);
        plaintext.extend_from_slice(&id);
        self.encrypt(&plaintext)
    }

    pub fn close(&mut self) -> Result<Vec<u8>> {
        self.ensure_open()?;
        let frame = self.encrypt(&[VERSION, 5])?;
        self.abort();
        Ok(frame)
    }

    /// Any malformed, duplicated, out-of-order, unauthenticated or unexpected
    /// incoming packet permanently aborts the session. No plaintext is returned.
    pub fn receive(&mut self, frame: &[u8]) -> Result<Event> {
        self.ensure_open()?;
        let result = self.receive_inner(frame);
        if result.is_err() {
            self.abort();
        }
        result
    }

    fn receive_inner(&mut self, frame: &[u8]) -> Result<Event> {
        if frame.len() < 18 || frame.len() > MAX_FRAME {
            return Err(ProtocolError::InvalidMessage);
        }
        let mut plaintext = Zeroizing::new(vec![0_u8; frame.len()]);
        let length = self
            .noise
            .as_mut()
            .ok_or(ProtocolError::Closed)?
            .read_message(frame, &mut plaintext)
            .map_err(|_| ProtocolError::Crypto)?;
        plaintext.truncate(length);
        if length < 2 || plaintext[0] != VERSION {
            return Err(ProtocolError::InvalidMessage);
        }
        let payload = &plaintext[2..];
        match plaintext[1] {
            1 => {
                if self.hello_received {
                    return Err(ProtocolError::InvalidMessage);
                }
                let name =
                    std::str::from_utf8(payload).map_err(|_| ProtocolError::InvalidMessage)?;
                validate_name(name).map_err(|_| ProtocolError::InvalidMessage)?;
                self.hello_received = true;
                Ok(Event::Hello {
                    name: name.to_owned(),
                })
            }
            2 => {
                if !payload.is_empty()
                    || !self.hello_sent
                    || !self.hello_received
                    || self.remote_confirmed
                {
                    return Err(ProtocolError::InvalidMessage);
                }
                self.remote_confirmed = true;
                Ok(Event::TrustConfirmed)
            }
            3 => {
                self.ensure_ready()?;
                if payload.len() < 17 || self.received.len() >= MAX_SESSION_MESSAGES {
                    return Err(ProtocolError::InvalidMessage);
                }
                let id: MessageId = payload[..16]
                    .try_into()
                    .map_err(|_| ProtocolError::InvalidMessage)?;
                if !self.received.insert(id) {
                    return Err(ProtocolError::InvalidMessage);
                }
                let text = std::str::from_utf8(&payload[16..])
                    .map_err(|_| ProtocolError::InvalidMessage)?;
                validate_text(text).map_err(|_| ProtocolError::InvalidMessage)?;
                self.pending_ack.insert(id);
                Ok(Event::Chat {
                    id,
                    text: text.to_owned(),
                })
            }
            4 => {
                self.ensure_ready()?;
                let id: MessageId = payload
                    .try_into()
                    .map_err(|_| ProtocolError::InvalidMessage)?;
                if !self.awaiting_ack.remove(&id) {
                    return Err(ProtocolError::InvalidMessage);
                }
                Ok(Event::Ack { id })
            }
            5 if payload.is_empty() => {
                self.abort();
                Ok(Event::Closed)
            }
            _ => Err(ProtocolError::InvalidMessage),
        }
    }

    pub fn abort(&mut self) {
        self.noise = None;
        self.local_confirmed = false;
        self.remote_confirmed = false;
        self.sent.clear();
        self.received.clear();
        self.pending_ack.clear();
        self.awaiting_ack.clear();
    }

    fn ensure_open(&self) -> Result<()> {
        if self.noise.is_some() {
            Ok(())
        } else {
            Err(ProtocolError::Closed)
        }
    }
    fn ensure_ready(&self) -> Result<()> {
        self.ensure_open()?;
        if self.is_ready() {
            Ok(())
        } else {
            Err(ProtocolError::NotReady)
        }
    }
    fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut frame = vec![0_u8; plaintext.len() + 16];
        let result = self
            .noise
            .as_mut()
            .ok_or(ProtocolError::Closed)?
            .write_message(plaintext, &mut frame);
        match result {
            Ok(length) => {
                frame.truncate(length);
                Ok(frame)
            }
            Err(_) => self.fail(ProtocolError::Crypto),
        }
    }
    fn fail<T>(&mut self, error: ProtocolError) -> Result<T> {
        self.abort();
        Err(error)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.abort();
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.len() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(ProtocolError::InvalidText);
    }
    Ok(())
}

fn validate_text(text: &str) -> Result<()> {
    if text.trim().is_empty() || text.len() > MAX_TEXT || text.contains('\0') {
        return Err(ProtocolError::InvalidText);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::{
        io::{self, Cursor},
        sync::mpsc::{self, Receiver, SyncSender},
        time::Duration,
    };

    /// A deliberately test-only in-memory byte transport. No socket/network
    /// fallback using this type is compiled into the desktop executable.
    struct MemoryPipe {
        sender: SyncSender<Vec<u8>>,
        receiver: Receiver<Vec<u8>>,
        buffered: Cursor<Vec<u8>>,
    }
    impl Read for MemoryPipe {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if output.is_empty() {
                return Ok(0);
            }
            if self.buffered.position() as usize == self.buffered.get_ref().len() {
                match self.receiver.recv_timeout(Duration::from_secs(3)) {
                    Ok(data) => self.buffered = Cursor::new(data),
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
                    Err(_) => return Err(io::ErrorKind::TimedOut.into()),
                }
            }
            self.buffered.read(output)
        }
    }
    impl Write for MemoryPipe {
        fn write(&mut self, input: &[u8]) -> io::Result<usize> {
            self.sender
                .send(input.to_vec())
                .map_err(|_| io::ErrorKind::BrokenPipe)?;
            Ok(input.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn memory_pair() -> (MemoryPipe, MemoryPipe) {
        let (a_tx, a_rx) = mpsc::sync_channel(64);
        let (b_tx, b_rx) = mpsc::sync_channel(64);
        (
            MemoryPipe {
                sender: a_tx,
                receiver: b_rx,
                buffered: Cursor::new(Vec::new()),
            },
            MemoryPipe {
                sender: b_tx,
                receiver: a_rx,
                buffered: Cursor::new(Vec::new()),
            },
        )
    }
    fn pair() -> (Session, Session) {
        let (mut a, mut b) = memory_pair();
        let first = IdentityKey::from_private(&[7_u8; 32]).unwrap();
        let second = IdentityKey::from_private(&[9_u8; 32]).unwrap();
        let thread =
            std::thread::spawn(move || handshake(&mut b, &second, Role::Responder).unwrap());
        let a = handshake(&mut a, &first, Role::Initiator).unwrap();
        (a, thread.join().unwrap())
    }
    fn introductions(a: &mut Session, b: &mut Session) {
        let hello = a.hello("Anna 🦊").unwrap();
        assert!(matches!(b.receive(&hello).unwrap(), Event::Hello { name } if name == "Anna 🦊"));
        let hello = b.hello("Bär").unwrap();
        assert!(matches!(a.receive(&hello).unwrap(), Event::Hello { name } if name == "Bär"));
        assert_eq!(a.check_contact(None).unwrap(), Trust::New);
        assert_eq!(b.check_contact(Some(a.local_key)).unwrap(), Trust::Known);
    }
    fn ready_pair() -> (Session, Session) {
        let (mut a, mut b) = pair();
        introductions(&mut a, &mut b);
        let confirm = a.confirm().unwrap();
        b.receive(&confirm).unwrap();
        let confirm = b.confirm().unwrap();
        a.receive(&confirm).unwrap();
        assert!(a.is_ready() && b.is_ready());
        (a, b)
    }

    #[test]
    fn handshake_and_bidirectional_unicode_chat_with_authenticated_delivery() {
        let (mut a, mut b) = ready_pair();
        assert_eq!(a.fingerprint(), b.fingerprint());
        let text = "Künstliche Testnachricht 🦊\nGrüezi, öäü!";
        let (id, ciphertext) = a.send_chat(text).unwrap();
        assert!(
            !ciphertext
                .windows(text.len())
                .any(|window| window == text.as_bytes())
        );
        assert!(
            matches!(b.receive(&ciphertext).unwrap(), Event::Chat { id: got, text: got_text } if got == id && got_text == text)
        );
        assert!(a.awaiting_ack.contains(&id));
        let ack = b.acknowledge(id).unwrap();
        assert!(matches!(a.receive(&ack).unwrap(), Event::Ack { id: got } if got == id));
        assert!(!a.awaiting_ack.contains(&id));
        let (id, ciphertext) = b.send_chat("Antwort mit Zeilen\numbruch 🥨").unwrap();
        assert!(matches!(
            a.receive(&ciphertext).unwrap(),
            Event::Chat { .. }
        ));
        let ack = a.acknowledge(id).unwrap();
        b.receive(&ack).unwrap();
    }

    #[test]
    fn both_confirmations_are_required_and_known_contacts_do_not_skip_gate() {
        let (mut a, mut b) = pair();
        introductions(&mut a, &mut b);
        assert!(matches!(
            a.send_chat("blocked"),
            Err(ProtocolError::NotReady)
        ));
        let frame = a.confirm().unwrap();
        b.receive(&frame).unwrap();
        assert!(!a.is_ready() && !b.is_ready());
        assert!(matches!(
            b.send_chat("blocked"),
            Err(ProtocolError::NotReady)
        ));
        let frame = b.confirm().unwrap();
        a.receive(&frame).unwrap();
        assert!(a.is_ready() && b.is_ready());
    }

    #[test]
    fn received_chat_before_mutual_trust_aborts_without_releasing_plaintext() {
        let (mut a, mut b) = pair();
        introductions(&mut a, &mut b);
        let mut text = vec![VERSION, 3];
        text.extend_from_slice(&[42; 16]);
        text.extend_from_slice(b"must never be exposed");
        let forged = a.encrypt(&text).unwrap();
        assert!(matches!(b.receive(&forged), Err(ProtocolError::NotReady)));
        assert!(b.is_closed());
    }

    #[test]
    fn changed_contact_key_blocks_connection_until_fresh_reverification() {
        let (mut a, mut b) = pair();
        a.hello("A").unwrap();
        let hello = b.hello("B").unwrap();
        a.receive(&hello).unwrap();
        assert!(matches!(
            a.check_contact(Some([0x55; 32])),
            Err(ProtocolError::KeyChanged)
        ));
        assert!(a.is_closed());
        assert!(matches!(a.check_contact(None), Err(ProtocolError::Closed)));
        assert!(a.confirm().is_err());
    }

    #[test]
    fn confirmation_requires_introduction_and_contact_lookup() {
        let (mut a, mut b) = pair();
        assert!(matches!(a.confirm(), Err(ProtocolError::NotReady)));
        let frame = a.hello("A").unwrap();
        b.receive(&frame).unwrap();
        let frame = b.hello("B").unwrap();
        a.receive(&frame).unwrap();
        assert!(matches!(a.confirm(), Err(ProtocolError::NotReady)));
        a.check_contact(None).unwrap();
        assert!(a.confirm().is_ok());
    }

    #[test]
    fn ciphertext_tampering_and_ciphertext_replay_abort() {
        let (mut a, mut b) = ready_pair();
        let (_, mut ciphertext) = a.send_chat("synthetic authenticated test").unwrap();
        ciphertext[3] ^= 0x80;
        assert!(matches!(b.receive(&ciphertext), Err(ProtocolError::Crypto)));
        assert!(b.is_closed());
        let (mut a, mut b) = ready_pair();
        let (_, ciphertext) = a.send_chat("synthetic replay test").unwrap();
        b.receive(&ciphertext).unwrap();
        assert!(matches!(b.receive(&ciphertext), Err(ProtocolError::Crypto)));
        assert!(b.is_closed());
    }

    #[test]
    fn duplicate_message_id_with_fresh_nonce_is_rejected() {
        let (mut a, mut b) = ready_pair();
        let (id, ciphertext) = a.send_chat("first synthetic payload").unwrap();
        b.receive(&ciphertext).unwrap();
        let mut payload = vec![VERSION, 3];
        payload.extend_from_slice(&id);
        payload.extend_from_slice(b"duplicate synthetic payload");
        let duplicate = a.encrypt(&payload).unwrap();
        assert!(matches!(
            b.receive(&duplicate),
            Err(ProtocolError::InvalidMessage)
        ));
        assert!(b.is_closed());
    }

    #[test]
    fn duplicate_and_unexpected_acknowledgements_are_rejected() {
        let (mut a, mut b) = ready_pair();
        let (id, ciphertext) = a.send_chat("synthetic delivery test").unwrap();
        b.receive(&ciphertext).unwrap();
        let ack = b.acknowledge(id).unwrap();
        a.receive(&ack).unwrap();
        assert!(b.acknowledge(id).is_err());
        let mut plaintext = vec![VERSION, 4];
        plaintext.extend_from_slice(&id);
        let duplicate = b.encrypt(&plaintext).unwrap();
        assert!(matches!(
            a.receive(&duplicate),
            Err(ProtocolError::InvalidMessage)
        ));
        assert!(a.is_closed());
        let (mut a, mut b) = ready_pair();
        plaintext[2..].fill(0x77);
        let unexpected = b.encrypt(&plaintext).unwrap();
        assert!(a.receive(&unexpected).is_err());
    }

    #[test]
    fn parser_checks_frame_lengths_versions_and_utf8() {
        for length in [0, MAX_FRAME as u32 + 1, u32::MAX] {
            assert!(matches!(
                read_frame(&mut Cursor::new(length.to_be_bytes())),
                Err(ProtocolError::InvalidMessage)
            ));
        }
        assert!(read_frame(&mut Cursor::new([0, 0, 0, 5, 1])).is_err());
        assert!(write_frame(&mut Vec::new(), &[]).is_err());
        assert!(write_frame(&mut Vec::new(), &vec![0; MAX_FRAME + 1]).is_err());
        let mut encoded = Vec::new();
        write_frame(&mut encoded, &vec![7; MAX_FRAME]).unwrap();
        assert_eq!(
            read_frame(&mut Cursor::new(encoded)).unwrap(),
            vec![7; MAX_FRAME]
        );
        let (mut a, mut b) = ready_pair();
        let unknown_version = a.encrypt(&[2, 5]).unwrap();
        assert!(b.receive(&unknown_version).is_err());
        let (mut a, mut b) = ready_pair();
        let mut invalid_utf8 = vec![VERSION, 3];
        invalid_utf8.extend_from_slice(&[0x33; 16]);
        invalid_utf8.push(0xff);
        assert!(b.receive(&a.encrypt(&invalid_utf8).unwrap()).is_err());
    }

    #[test]
    fn duplicate_controls_and_unknown_types_abort() {
        let (mut a, mut b) = pair();
        introductions(&mut a, &mut b);
        let duplicate_hello = a.encrypt(&[VERSION, 1, b'A']).unwrap();
        assert!(b.receive(&duplicate_hello).is_err());
        let (mut a, mut b) = ready_pair();
        let duplicate_confirm = a.encrypt(&[VERSION, 2]).unwrap();
        assert!(b.receive(&duplicate_confirm).is_err());
        let (mut a, mut b) = ready_pair();
        let unknown = a.encrypt(&[VERSION, 255]).unwrap();
        assert!(b.receive(&unknown).is_err());
    }

    #[test]
    fn text_boundaries_and_bounded_message_bookkeeping() {
        let (mut a, mut b) = ready_pair();
        assert!(a.send_chat("").is_err());
        assert!(a.send_chat(" \n").is_err());
        assert!(a.send_chat("\0").is_err());
        assert!(a.send_chat(&"a".repeat(MAX_TEXT + 1)).is_err());
        let (_, maximum) = a.send_chat(&"a".repeat(MAX_TEXT)).unwrap();
        b.receive(&maximum).unwrap();
        for number in 1..MAX_SESSION_MESSAGES {
            let mut id = [0_u8; 16];
            id[..8].copy_from_slice(&(number as u64).to_be_bytes());
            a.sent.insert(id);
        }
        assert!(matches!(
            a.send_chat("bounded"),
            Err(ProtocolError::SessionLimit)
        ));
        let (mut a, _) = pair();
        assert!(a.hello("A\nB").is_err());
        assert!(a.hello(&"a".repeat(MAX_NAME + 1)).is_err());
    }

    #[test]
    fn abort_and_close_prevent_all_further_chat_and_reconnect_uses_fresh_keys() {
        let (mut a, mut b) = ready_pair();
        let (_, old_frame) = a.send_chat("old session synthetic message").unwrap();
        b.receive(&old_frame).unwrap();
        let close = a.close().unwrap();
        assert!(matches!(b.receive(&close), Ok(Event::Closed)));
        assert!(!a.is_ready() && !b.is_ready());
        assert!(a.send_chat("blocked after lock").is_err());
        assert!(b.receive(&old_frame).is_err());
        let (mut fresh_a, mut fresh_b) = ready_pair();
        assert_eq!(a.fingerprint(), fresh_a.fingerprint());
        assert!(fresh_b.receive(&old_frame).is_err());
        fresh_a.abort();
        assert!(fresh_a.send_chat("blocked after lock").is_err());
    }

    #[test]
    fn fingerprint_canonical_and_identity_is_stable() {
        let a = IdentityKey::from_private(&[7_u8; 32]).unwrap();
        let same = IdentityKey::from_private(a.private()).unwrap();
        let b = IdentityKey::from_private(&[9_u8; 32]).unwrap();
        assert_eq!(a.public(), same.public());
        let fingerprint = security_fingerprint(&a.public(), &b.public());
        assert_eq!(fingerprint, security_fingerprint(&b.public(), &a.public()));
        assert_eq!(fingerprint.replace(' ', "").len(), 64);
        assert_ne!(fingerprint, security_fingerprint(&a.public(), &[3; 32]));
        assert_ne!(
            IdentityKey::generate().unwrap().public(),
            IdentityKey::generate().unwrap().public()
        );
    }

    #[test]
    fn truncated_handshake_and_wrong_protocol_magic_fail() {
        let mut incorrect = Cursor::new(vec![0_u8; 64]);
        let identity = IdentityKey::from_private(&[7; 32]).unwrap();
        assert!(handshake(&mut incorrect, &identity, Role::Initiator).is_err());
        let (mut a, mut b) = memory_pair();
        let thread = std::thread::spawn(move || {
            b.write_all(MAGIC).unwrap();
            b.write_all(&u32::MAX.to_be_bytes()).unwrap();
        });
        assert!(handshake(&mut a, &identity, Role::Initiator).is_err());
        thread.join().unwrap();
    }
}
