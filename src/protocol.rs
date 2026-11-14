//! Bounded Noise XX sessions. No networking or persistence is performed here.
use std::{
    collections::{HashMap, HashSet},
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
pub const MAX_SESSION_FRAMES: usize = 65_536;
pub const MAX_FILE_SIZE: u64 = 20 * 1024 * 1024;
pub const MAX_FILE_CHUNK: usize = 8_192;
pub const MAX_FILE_NAME: usize = 180;
const CONTEXT: &[u8] = b"Hushlink/v2/RFCOMM/Noise_XX_25519_ChaChaPoly_SHA256";
const MAGIC: &[u8; 5] = b"HUSH\x02";
const VERSION: u8 = 2;
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
    #[error("Die Datei ist zu groß oder ihr Name ist ungültig (höchstens 20 MiB).")]
    InvalidFile,
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
    Hello {
        name: String,
    },
    TrustConfirmed,
    Chat {
        id: MessageId,
        text: String,
    },
    Ack {
        id: MessageId,
    },
    Closed,
    FileOffer {
        id: MessageId,
        name: String,
        size: u64,
        sha256: [u8; 32],
    },
    FileAccept {
        id: MessageId,
    },
    FileChunk {
        id: MessageId,
        offset: u64,
        data: Vec<u8>,
    },
    FileComplete {
        id: MessageId,
    },
    FileStored {
        id: MessageId,
    },
    FileCancel {
        id: MessageId,
    },
    FileCancelAck {
        id: MessageId,
    },
    /// Authenticated, state-valid traffic that crossed a local cancellation.
    /// No file contents are released; the runtime must ignore this event.
    FileDiscarded {
        id: MessageId,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FilePhase {
    Offered,
    Accepted,
    Complete,
    Stored,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FileDirection {
    Incoming,
    Outgoing,
}

struct CancelledFile {
    transfer: FileTransfer,
    direction: FileDirection,
    peer_cancelled: bool,
}

struct FileTransfer {
    id: MessageId,
    size: u64,
    next_offset: u64,
    phase: FilePhase,
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
        incoming_file: None,
        outgoing_file: None,
        cancelled_files: HashMap::new(),
        pending_cancel_ack: HashSet::new(),
        finished_files: HashSet::new(),
        sent_frames: 0,
        received_frames: 0,
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
    hash.update(b"Hushlink identity fingerprint\0v2\0");
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
    incoming_file: Option<FileTransfer>,
    outgoing_file: Option<FileTransfer>,
    cancelled_files: HashMap<MessageId, CancelledFile>,
    pending_cancel_ack: HashSet<MessageId>,
    finished_files: HashSet<MessageId>,
    sent_frames: usize,
    received_frames: usize,
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
        if self.sent.contains(&id) || self.received.contains(&id) {
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

    /// Announce metadata only. The receiver must consent before any file bytes
    /// may be sent. The caller must hash a stable snapshot of the source first.
    pub fn offer_file(
        &mut self,
        name: &str,
        size: u64,
        sha256: [u8; 32],
    ) -> Result<(MessageId, Vec<u8>)> {
        self.ensure_ready()?;
        validate_file_name(name)?;
        if size > MAX_FILE_SIZE {
            return Err(ProtocolError::InvalidFile);
        }
        if self.outgoing_file.is_some() {
            return Err(ProtocolError::InvalidMessage);
        }
        if self.sent.len() >= MAX_SESSION_MESSAGES {
            return Err(ProtocolError::SessionLimit);
        }
        let mut id = [0_u8; 16];
        OsRng
            .try_fill_bytes(&mut id)
            .map_err(|_| ProtocolError::Crypto)?;
        if self.sent.contains(&id) || self.received.contains(&id) {
            return self.fail(ProtocolError::Crypto);
        }
        let mut plaintext = Zeroizing::new(vec![VERSION, 6]);
        plaintext.extend_from_slice(&id);
        plaintext.extend_from_slice(&size.to_be_bytes());
        plaintext.extend_from_slice(&sha256);
        plaintext.extend_from_slice(name.as_bytes());
        let frame = self.encrypt(&plaintext)?;
        self.sent.insert(id);
        self.outgoing_file = Some(FileTransfer {
            id,
            size,
            next_offset: 0,
            phase: FilePhase::Offered,
        });
        Ok((id, frame))
    }

    pub fn accept_file(&mut self, id: MessageId) -> Result<Vec<u8>> {
        self.ensure_ready()?;
        self.require_incoming(id, FilePhase::Offered)?;
        let frame = self.file_control(7, id)?;
        if let Some(transfer) = self.incoming_file.as_mut() {
            transfer.phase = FilePhase::Accepted;
        }
        Ok(frame)
    }

    /// Supply one nonempty chunk at the exact next offset. The runtime must
    /// process inbound control/cancellation between bounded outgoing chunks.
    pub fn send_file_chunk(&mut self, id: MessageId, offset: u64, data: &[u8]) -> Result<Vec<u8>> {
        self.ensure_ready()?;
        self.require_outgoing(id, FilePhase::Accepted)?;
        let transfer = self
            .outgoing_file
            .as_ref()
            .ok_or(ProtocolError::InvalidMessage)?;
        let next = check_file_chunk(transfer, offset, data.len())?;
        let mut plaintext = Zeroizing::new(vec![VERSION, 8]);
        plaintext.extend_from_slice(&id);
        plaintext.extend_from_slice(&offset.to_be_bytes());
        plaintext.extend_from_slice(data);
        let frame = self.encrypt(&plaintext)?;
        if let Some(transfer) = self.outgoing_file.as_mut() {
            transfer.next_offset = next;
        }
        Ok(frame)
    }

    pub fn complete_file(&mut self, id: MessageId) -> Result<Vec<u8>> {
        self.ensure_ready()?;
        self.require_outgoing(id, FilePhase::Accepted)?;
        if self
            .outgoing_file
            .as_ref()
            .is_none_or(|transfer| transfer.next_offset != transfer.size)
        {
            return Err(ProtocolError::InvalidMessage);
        }
        let frame = self.file_control(9, id)?;
        if let Some(transfer) = self.outgoing_file.as_mut() {
            transfer.phase = FilePhase::Complete;
        }
        Ok(frame)
    }

    /// Call only after checking the exact advertised size and SHA-256 digest
    /// and durably storing the bytes inside the encrypted vault. Receiving
    /// FileComplete alone never proves storage or integrity of the whole file.
    pub fn file_stored(&mut self, id: MessageId) -> Result<Vec<u8>> {
        self.ensure_ready()?;
        self.require_incoming(id, FilePhase::Complete)?;
        let frame = self.file_control(10, id)?;
        self.incoming_file = None;
        self.finished_files.insert(id);
        Ok(frame)
    }

    /// Either person may cancel an active offer/transfer. Its ID remains spent.
    /// State-valid traffic already in flight is authenticated and discarded until
    /// the peer's cancellation acknowledgement; it never releases file bytes.
    pub fn cancel_file(&mut self, id: MessageId) -> Result<Vec<u8>> {
        self.ensure_ready()?;
        if !self.has_file(id) {
            return Err(ProtocolError::InvalidMessage);
        }
        let frame = self.file_control(11, id)?;
        let (transfer, direction) = if self
            .incoming_file
            .as_ref()
            .is_some_and(|transfer| transfer.id == id)
        {
            (self.incoming_file.take(), FileDirection::Incoming)
        } else {
            (self.outgoing_file.take(), FileDirection::Outgoing)
        };
        let transfer = transfer.ok_or(ProtocolError::InvalidMessage)?;
        self.cancelled_files.insert(
            id,
            CancelledFile {
                transfer,
                direction,
                peer_cancelled: false,
            },
        );
        Ok(frame)
    }

    /// Stop the cancelled transfer before sending this acknowledgement. Its
    /// position in the ordered stream bounds which earlier frames may be drained.
    pub fn acknowledge_file_cancel(&mut self, id: MessageId) -> Result<Vec<u8>> {
        self.ensure_ready()?;
        if !self.pending_cancel_ack.contains(&id) {
            return Err(ProtocolError::InvalidMessage);
        }
        let frame = self.file_control(12, id)?;
        self.pending_cancel_ack.remove(&id);
        Ok(frame)
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
        if self.received_frames >= MAX_SESSION_FRAMES {
            return Err(ProtocolError::SessionLimit);
        }
        let mut plaintext = Zeroizing::new(vec![0_u8; frame.len()]);
        let length = self
            .noise
            .as_mut()
            .ok_or(ProtocolError::Closed)?
            .read_message(frame, &mut plaintext)
            .map_err(|_| ProtocolError::Crypto)?;
        self.received_frames += 1;
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
                if self.sent.contains(&id) || !self.received.insert(id) {
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
            6 => {
                self.ensure_ready()?;
                if payload.len() < 57 || self.incoming_file.is_some() {
                    return Err(ProtocolError::InvalidMessage);
                }
                let id = parse_id(&payload[..16])?;
                let size = u64::from_be_bytes(
                    payload[16..24]
                        .try_into()
                        .map_err(|_| ProtocolError::InvalidMessage)?,
                );
                let sha256 = payload[24..56]
                    .try_into()
                    .map_err(|_| ProtocolError::InvalidMessage)?;
                let name = std::str::from_utf8(&payload[56..])
                    .map_err(|_| ProtocolError::InvalidMessage)?;
                validate_file_name(name).map_err(|_| ProtocolError::InvalidMessage)?;
                if size > MAX_FILE_SIZE
                    || self.received.len() >= MAX_SESSION_MESSAGES
                    || self.sent.contains(&id)
                    || !self.received.insert(id)
                {
                    return Err(ProtocolError::InvalidMessage);
                }
                self.incoming_file = Some(FileTransfer {
                    id,
                    size,
                    next_offset: 0,
                    phase: FilePhase::Offered,
                });
                Ok(Event::FileOffer {
                    id,
                    name: name.to_owned(),
                    size,
                    sha256,
                })
            }
            7 => {
                self.ensure_ready()?;
                let id = parse_id(payload)?;
                if let Some(cancelled) = self.cancelled_files.get_mut(&id) {
                    if cancelled.direction != FileDirection::Outgoing
                        || cancelled.transfer.phase != FilePhase::Offered
                    {
                        return Err(ProtocolError::InvalidMessage);
                    }
                    cancelled.transfer.phase = FilePhase::Accepted;
                    return Ok(Event::FileDiscarded { id });
                }
                self.require_outgoing(id, FilePhase::Offered)?;
                if let Some(transfer) = self.outgoing_file.as_mut() {
                    transfer.phase = FilePhase::Accepted;
                }
                Ok(Event::FileAccept { id })
            }
            8 => {
                self.ensure_ready()?;
                if payload.len() < 25 {
                    return Err(ProtocolError::InvalidMessage);
                }
                let id = parse_id(&payload[..16])?;
                let offset = u64::from_be_bytes(
                    payload[16..24]
                        .try_into()
                        .map_err(|_| ProtocolError::InvalidMessage)?,
                );
                if let Some(cancelled) = self.cancelled_files.get_mut(&id) {
                    if cancelled.direction != FileDirection::Incoming
                        || cancelled.transfer.phase != FilePhase::Accepted
                    {
                        return Err(ProtocolError::InvalidMessage);
                    }
                    cancelled.transfer.next_offset =
                        check_file_chunk(&cancelled.transfer, offset, payload.len() - 24)?;
                    return Ok(Event::FileDiscarded { id });
                }
                self.require_incoming(id, FilePhase::Accepted)?;
                let transfer = self
                    .incoming_file
                    .as_mut()
                    .ok_or(ProtocolError::InvalidMessage)?;
                transfer.next_offset = check_file_chunk(transfer, offset, payload.len() - 24)?;
                Ok(Event::FileChunk {
                    id,
                    offset,
                    data: payload[24..].to_vec(),
                })
            }
            9 => {
                self.ensure_ready()?;
                let id = parse_id(payload)?;
                if let Some(cancelled) = self.cancelled_files.get_mut(&id) {
                    if cancelled.direction != FileDirection::Incoming
                        || cancelled.transfer.phase != FilePhase::Accepted
                        || cancelled.transfer.next_offset != cancelled.transfer.size
                    {
                        return Err(ProtocolError::InvalidMessage);
                    }
                    cancelled.transfer.phase = FilePhase::Complete;
                    return Ok(Event::FileDiscarded { id });
                }
                self.require_incoming(id, FilePhase::Accepted)?;
                let transfer = self
                    .incoming_file
                    .as_mut()
                    .ok_or(ProtocolError::InvalidMessage)?;
                if transfer.next_offset != transfer.size {
                    return Err(ProtocolError::InvalidMessage);
                }
                transfer.phase = FilePhase::Complete;
                Ok(Event::FileComplete { id })
            }
            10 => {
                self.ensure_ready()?;
                let id = parse_id(payload)?;
                if let Some(cancelled) = self.cancelled_files.get_mut(&id) {
                    if cancelled.direction != FileDirection::Outgoing
                        || cancelled.transfer.phase != FilePhase::Complete
                    {
                        return Err(ProtocolError::InvalidMessage);
                    }
                    cancelled.transfer.phase = FilePhase::Stored;
                    return Ok(Event::FileDiscarded { id });
                }
                self.require_outgoing(id, FilePhase::Complete)?;
                self.outgoing_file = None;
                Ok(Event::FileStored { id })
            }
            11 => {
                self.ensure_ready()?;
                let id = parse_id(payload)?;
                if self.pending_cancel_ack.contains(&id) {
                    return Err(ProtocolError::InvalidMessage);
                }
                if let Some(cancelled) = self.cancelled_files.get_mut(&id) {
                    if cancelled.peer_cancelled {
                        return Err(ProtocolError::InvalidMessage);
                    }
                    cancelled.peer_cancelled = true;
                } else if self.has_file(id) {
                    self.remove_file(id);
                } else if !self.finished_files.remove(&id) {
                    return Err(ProtocolError::InvalidMessage);
                }
                self.pending_cancel_ack.insert(id);
                Ok(Event::FileCancel { id })
            }
            12 => {
                self.ensure_ready()?;
                let id = parse_id(payload)?;
                if self.cancelled_files.remove(&id).is_none() {
                    return Err(ProtocolError::InvalidMessage);
                }
                Ok(Event::FileCancelAck { id })
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
        self.incoming_file = None;
        self.outgoing_file = None;
        self.cancelled_files.clear();
        self.pending_cancel_ack.clear();
        self.finished_files.clear();
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
    fn require_incoming(&self, id: MessageId, phase: FilePhase) -> Result<()> {
        if self
            .incoming_file
            .as_ref()
            .is_some_and(|transfer| transfer.id == id && transfer.phase == phase)
        {
            Ok(())
        } else {
            Err(ProtocolError::InvalidMessage)
        }
    }
    fn require_outgoing(&self, id: MessageId, phase: FilePhase) -> Result<()> {
        if self
            .outgoing_file
            .as_ref()
            .is_some_and(|transfer| transfer.id == id && transfer.phase == phase)
        {
            Ok(())
        } else {
            Err(ProtocolError::InvalidMessage)
        }
    }
    fn has_file(&self, id: MessageId) -> bool {
        self.incoming_file
            .as_ref()
            .is_some_and(|file| file.id == id)
            || self
                .outgoing_file
                .as_ref()
                .is_some_and(|file| file.id == id)
    }
    fn remove_file(&mut self, id: MessageId) {
        if self
            .incoming_file
            .as_ref()
            .is_some_and(|file| file.id == id)
        {
            self.incoming_file = None;
        }
        if self
            .outgoing_file
            .as_ref()
            .is_some_and(|file| file.id == id)
        {
            self.outgoing_file = None;
        }
    }
    fn file_control(&mut self, kind: u8, id: MessageId) -> Result<Vec<u8>> {
        let mut plaintext = Zeroizing::new(vec![VERSION, kind]);
        plaintext.extend_from_slice(&id);
        self.encrypt(&plaintext)
    }
    fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        if self.sent_frames >= MAX_SESSION_FRAMES {
            return Err(ProtocolError::SessionLimit);
        }
        let mut frame = vec![0_u8; plaintext.len() + 16];
        let result = self
            .noise
            .as_mut()
            .ok_or(ProtocolError::Closed)?
            .write_message(plaintext, &mut frame);
        match result {
            Ok(length) => {
                self.sent_frames += 1;
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

fn parse_id(payload: &[u8]) -> Result<MessageId> {
    payload
        .try_into()
        .map_err(|_| ProtocolError::InvalidMessage)
}

fn check_file_chunk(transfer: &FileTransfer, offset: u64, length: usize) -> Result<u64> {
    let next = offset
        .checked_add(length as u64)
        .ok_or(ProtocolError::InvalidMessage)?;
    if length == 0
        || length > MAX_FILE_CHUNK
        || offset != transfer.next_offset
        || next > transfer.size
    {
        return Err(ProtocolError::InvalidMessage);
    }
    Ok(next)
}

/// A display basename, never a destination path. Restrict Windows-invalid and
/// reserved names even when validating on other platforms. Export still needs
/// a user-selected path; receiving a file must never create/open it implicitly.
pub fn validate_file_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > MAX_FILE_NAME
        || name.ends_with([' ', '.'])
        || name.chars().any(|character| {
            character.is_control()
                || "<>:\"/\\|?*".contains(character)
                || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
    {
        return Err(ProtocolError::InvalidFile);
    }
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    let numbered_device = base
        .strip_prefix("COM")
        .or_else(|| base.strip_prefix("LPT"))
        .is_some_and(|number| {
            matches!(
                number,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        });
    if name.trim().is_empty()
        || matches!(
            base.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        )
        || numbered_device
    {
        return Err(ProtocolError::InvalidFile);
    }
    Ok(())
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
        let unknown_version = a.encrypt(&[1, 5]).unwrap();
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

    fn accepted_file(a: &mut Session, b: &mut Session, data: &[u8]) -> MessageId {
        let digest: [u8; 32] = Sha256::digest(data).into();
        let (id, offer) = a
            .offer_file("Künstliche Datei 🦊.txt", data.len() as u64, digest)
            .unwrap();
        assert!(matches!(
            b.receive(&offer).unwrap(),
            Event::FileOffer { id: got, name, size, sha256 }
                if got == id && name == "Künstliche Datei 🦊.txt"
                    && size == data.len() as u64 && sha256 == digest
        ));
        let accept = b.accept_file(id).unwrap();
        assert!(matches!(a.receive(&accept).unwrap(), Event::FileAccept { id: got } if got == id));
        id
    }

    fn transfer_file(a: &mut Session, b: &mut Session, id: MessageId, data: &[u8]) {
        let mut received = Zeroizing::new(Vec::new());
        for (number, chunk) in data.chunks(MAX_FILE_CHUNK).enumerate() {
            let offset = (number * MAX_FILE_CHUNK) as u64;
            let ciphertext = a.send_file_chunk(id, offset, chunk).unwrap();
            if let Event::FileChunk {
                id: got,
                offset: got_offset,
                mut data,
            } = b.receive(&ciphertext).unwrap()
            {
                assert_eq!(got, id);
                assert_eq!(got_offset, offset);
                received.extend_from_slice(&data);
                data.zeroize();
            } else {
                panic!("expected bounded file chunk");
            }
        }
        assert_eq!(received.as_slice(), data);
        assert!(a.outgoing_file.is_some());
        let complete = a.complete_file(id).unwrap();
        assert!(
            matches!(b.receive(&complete).unwrap(), Event::FileComplete { id: got } if got == id)
        );
        // A completed socket transfer remains pending until explicit durable
        // storage in the runtime authorizes the authenticated receipt.
        assert!(a.outgoing_file.is_some());
        let stored = b.file_stored(id).unwrap();
        assert!(matches!(a.receive(&stored).unwrap(), Event::FileStored { id: got } if got == id));
        assert!(a.outgoing_file.is_none() && b.incoming_file.is_none());
    }

    fn raw_file(kind: u8, id: MessageId, payload: &[u8]) -> Vec<u8> {
        let mut packet = vec![VERSION, kind];
        packet.extend_from_slice(&id);
        packet.extend_from_slice(payload);
        packet
    }

    #[test]
    fn bidirectional_files_and_unicode_chat_share_the_authenticated_session() {
        let (mut a, mut b) = ready_pair();
        let data = "Synthetic attachment, äöü 🦊\n".repeat(1_000).into_bytes();
        let other = b"Synthetic reverse transfer";
        let id = accepted_file(&mut a, &mut b, &data);
        let reverse = accepted_file(&mut b, &mut a, other);
        let (chat_id, chat) = a
            .send_chat("Chat bleibt während Dateiversand benutzbar 🦊")
            .unwrap();
        assert!(matches!(b.receive(&chat).unwrap(), Event::Chat { .. }));
        let ack = b.acknowledge(chat_id).unwrap();
        assert!(matches!(a.receive(&ack).unwrap(), Event::Ack { .. }));
        transfer_file(&mut a, &mut b, id, &data);
        transfer_file(&mut b, &mut a, reverse, other);
        let empty = accepted_file(&mut a, &mut b, &[]);
        transfer_file(&mut a, &mut b, empty, &[]);
    }

    #[test]
    fn files_require_mutual_trust_and_explicit_recipient_consent() {
        let (mut a, mut b) = pair();
        introductions(&mut a, &mut b);
        assert!(matches!(
            a.offer_file("file.txt", 1, [0; 32]),
            Err(ProtocolError::NotReady)
        ));
        assert!(matches!(
            b.accept_file([1; 16]),
            Err(ProtocolError::NotReady)
        ));
        let mut payload = Vec::new();
        payload.extend_from_slice(&1_u64.to_be_bytes());
        payload.extend_from_slice(&[0; 32]);
        payload.extend_from_slice(b"file.txt");
        let forged = a.encrypt(&raw_file(6, [1; 16], &payload)).unwrap();
        assert!(matches!(b.receive(&forged), Err(ProtocolError::NotReady)));
        assert!(b.is_closed());
        let (mut a, mut b) = ready_pair();
        let (id, offer) = a.offer_file("file.txt", 1, [0; 32]).unwrap();
        b.receive(&offer).unwrap();
        assert!(a.send_file_chunk(id, 0, &[5]).is_err());
        assert!(a.complete_file(id).is_err());
        assert!(b.file_stored(id).is_err());
        let chunk = a.encrypt(&raw_file(8, id, &[0; 9])).unwrap();
        assert!(b.receive(&chunk).is_err());
        assert!(b.is_closed());
    }

    #[test]
    fn file_offer_names_sizes_and_active_transfer_counts_are_bounded() {
        for invalid in [
            "",
            " ",
            ".",
            "..",
            "../file",
            "C:\\file",
            "a/b",
            "a\\b",
            "AUX.txt",
            "con",
            "NUL",
            "COM1.txt",
            "Lpt9",
            "COM¹",
            "CON .txt",
            "a.",
            "a ",
            "a:b",
            "line\nbreak",
            "a\u{202e}exe.txt",
        ] {
            assert!(validate_file_name(invalid).is_err(), "{invalid:?}");
        }
        assert!(validate_file_name("Umlaute äöü 🦊.pdf").is_ok());
        assert!(validate_file_name(&"a".repeat(MAX_FILE_NAME + 1)).is_err());
        let (mut a, mut b) = ready_pair();
        assert!(
            a.offer_file("synthetic", MAX_FILE_SIZE + 1, [0; 32])
                .is_err()
        );
        let (id, offer) = a.offer_file("synthetic", MAX_FILE_SIZE, [0; 32]).unwrap();
        b.receive(&offer).unwrap();
        assert!(a.offer_file("second", 0, [0; 32]).is_err());
        let mut duplicate_offer = raw_file(6, [55; 16], &0_u64.to_be_bytes());
        duplicate_offer.extend_from_slice(&[0; 32]);
        duplicate_offer.extend_from_slice(b"second");
        assert!(b.receive(&a.encrypt(&duplicate_offer).unwrap()).is_err());
        assert!(b.is_closed());
        assert!(a.cancel_file(id).is_ok());
    }

    #[test]
    fn file_parser_rejects_offsets_truncation_oversize_and_early_completion() {
        for (offset, data) in [
            (1_u64, vec![1]),
            (0, vec![]),
            (0, vec![1; MAX_FILE_CHUNK + 1]),
            (u64::MAX, vec![1]),
        ] {
            let (mut a, mut b) = ready_pair();
            let id = accepted_file(&mut a, &mut b, &[0; 3]);
            let mut bytes = offset.to_be_bytes().to_vec();
            bytes.extend_from_slice(&data);
            assert!(a.send_file_chunk(id, offset, &data).is_err());
            let frame = a.encrypt(&raw_file(8, id, &bytes)).unwrap();
            assert!(b.receive(&frame).is_err());
            assert!(b.is_closed());
        }
        let (mut a, mut b) = ready_pair();
        let id = accepted_file(&mut a, &mut b, &[0; 3]);
        assert!(a.send_file_chunk(id, 0, &[0; 4]).is_err());
        assert!(a.complete_file(id).is_err());
        let early = a.encrypt(&raw_file(9, id, &[])).unwrap();
        assert!(b.receive(&early).is_err());
        for kind in 6..=12 {
            let (mut a, mut b) = ready_pair();
            let frame = a.encrypt(&[VERSION, kind]).unwrap();
            assert!(b.receive(&frame).is_err());
            assert!(b.is_closed());
        }
    }

    #[test]
    fn received_file_offers_enforce_bounds_and_utf8_before_releasing_metadata() {
        for (size, name) in [
            (MAX_FILE_SIZE + 1, b"synthetic".to_vec()),
            (0, vec![b'a'; MAX_FILE_NAME + 1]),
            (0, vec![0xff]),
            (0, b"../synthetic".to_vec()),
        ] {
            let (mut a, mut b) = ready_pair();
            let mut payload = size.to_be_bytes().to_vec();
            payload.extend_from_slice(&[0; 32]);
            payload.extend_from_slice(&name);
            let forged = a.encrypt(&raw_file(6, [42; 16], &payload)).unwrap();
            assert!(b.receive(&forged).is_err());
            assert!(b.is_closed());
        }
        let (mut a, mut b) = ready_pair();
        let id = accepted_file(&mut a, &mut b, b"synthetic");
        let mut chunk = a.send_file_chunk(id, 0, b"synthetic").unwrap();
        let last = chunk.len() - 1;
        chunk[last] ^= 1;
        assert!(matches!(b.receive(&chunk), Err(ProtocolError::Crypto)));
        assert!(b.is_closed());
    }

    #[test]
    fn duplicate_file_controls_chunks_and_receipts_abort() {
        for kind in [7, 8, 9, 10] {
            let (mut a, mut b) = ready_pair();
            let id = accepted_file(&mut a, &mut b, &[0; 1]);
            let frame = a.send_file_chunk(id, 0, &[0]).unwrap();
            b.receive(&frame).unwrap();
            if kind == 7 {
                let duplicate = b.encrypt(&raw_file(7, id, &[])).unwrap();
                assert!(a.receive(&duplicate).is_err());
            } else if kind == 8 {
                let duplicate = a.encrypt(&raw_file(8, id, &[0; 9])).unwrap();
                assert!(b.receive(&duplicate).is_err());
            } else {
                let frame = a.complete_file(id).unwrap();
                b.receive(&frame).unwrap();
                if kind == 9 {
                    let duplicate = a.encrypt(&raw_file(9, id, &[])).unwrap();
                    assert!(b.receive(&duplicate).is_err());
                } else {
                    let frame = b.file_stored(id).unwrap();
                    a.receive(&frame).unwrap();
                    let duplicate = b.encrypt(&raw_file(10, id, &[])).unwrap();
                    assert!(a.receive(&duplicate).is_err());
                }
            }
        }
        let (mut a, mut b) = ready_pair();
        let id = accepted_file(&mut a, &mut b, &[0; 1]);
        let early_stored = b.encrypt(&raw_file(10, id, &[])).unwrap();
        assert!(a.receive(&early_stored).is_err());
    }

    #[test]
    fn cancellation_spends_ids_and_rejects_data_after_acknowledgement() {
        for cancel_sender in [false, true] {
            let (mut a, mut b) = ready_pair();
            let id = accepted_file(&mut a, &mut b, &[0; 3]);
            let (canceller, peer) = if cancel_sender {
                (&mut a, &mut b)
            } else {
                (&mut b, &mut a)
            };
            let cancel = canceller.cancel_file(id).unwrap();
            assert!(
                matches!(peer.receive(&cancel).unwrap(), Event::FileCancel { id: got } if got == id)
            );
            let ack = peer.acknowledge_file_cancel(id).unwrap();
            assert!(peer.acknowledge_file_cancel(id).is_err());
            assert!(
                matches!(canceller.receive(&ack).unwrap(), Event::FileCancelAck { id: got } if got == id)
            );
            assert!(a.send_file_chunk(id, 0, &[0]).is_err());
            assert!(b.accept_file(id).is_err());
            assert!(a.cancel_file(id).is_err());
            let mut bytes = 0_u64.to_be_bytes().to_vec();
            bytes.push(1);
            let late = a.encrypt(&raw_file(8, id, &bytes)).unwrap();
            assert!(b.receive(&late).is_err());
            assert!(b.is_closed());
        }
        let (mut a, mut b) = ready_pair();
        let (id, offer) = a.offer_file("synthetic", 0, [0; 32]).unwrap();
        b.receive(&offer).unwrap();
        let cancel = b.cancel_file(id).unwrap();
        a.receive(&cancel).unwrap();
        let mut reused = raw_file(6, id, &0_u64.to_be_bytes());
        reused.extend_from_slice(&[0; 32]);
        reused.extend_from_slice(b"synthetic");
        assert!(b.receive(&a.encrypt(&reused).unwrap()).is_err());
    }

    #[test]
    fn cancellation_drains_in_flight_chunks_without_releasing_plaintext() {
        let (mut a, mut b) = ready_pair();
        let id = accepted_file(&mut a, &mut b, &[1, 2, 3]);
        let in_flight = a.send_file_chunk(id, 0, &[1, 2, 3]).unwrap();
        let complete = a.complete_file(id).unwrap();
        let cancel = b.cancel_file(id).unwrap();
        assert!(
            matches!(b.receive(&in_flight).unwrap(), Event::FileDiscarded { id: got } if got == id)
        );
        assert!(
            matches!(b.receive(&complete).unwrap(), Event::FileDiscarded { id: got } if got == id)
        );
        assert!(matches!(
            a.receive(&cancel).unwrap(),
            Event::FileCancel { .. }
        ));
        let ack = a.acknowledge_file_cancel(id).unwrap();
        assert!(matches!(
            b.receive(&ack).unwrap(),
            Event::FileCancelAck { .. }
        ));
        let (chat_id, chat) = a.send_chat("chat survives cancellation").unwrap();
        b.receive(&chat).unwrap();
        let ack = b.acknowledge(chat_id).unwrap();
        a.receive(&ack).unwrap();
        assert!(a.is_ready() && b.is_ready());
        assert!(a.cancelled_files.is_empty() && b.cancelled_files.is_empty());
    }

    #[test]
    fn cancellation_can_cross_accept_storage_and_a_simultaneous_cancel() {
        let (mut a, mut b) = ready_pair();
        let (id, offer) = a.offer_file("synthetic", 0, [0; 32]).unwrap();
        b.receive(&offer).unwrap();
        let accept = b.accept_file(id).unwrap();
        let cancel = a.cancel_file(id).unwrap();
        assert!(matches!(
            a.receive(&accept).unwrap(),
            Event::FileDiscarded { .. }
        ));
        b.receive(&cancel).unwrap();
        let ack = b.acknowledge_file_cancel(id).unwrap();
        a.receive(&ack).unwrap();
        assert!(a.is_ready() && b.is_ready());

        let id = accepted_file(&mut a, &mut b, &[]);
        let complete = a.complete_file(id).unwrap();
        b.receive(&complete).unwrap();
        let stored = b.file_stored(id).unwrap();
        let cancel = a.cancel_file(id).unwrap();
        assert!(matches!(
            a.receive(&stored).unwrap(),
            Event::FileDiscarded { .. }
        ));
        assert!(matches!(
            b.receive(&cancel).unwrap(),
            Event::FileCancel { .. }
        ));
        let ack = b.acknowledge_file_cancel(id).unwrap();
        a.receive(&ack).unwrap();
        assert!(a.is_ready() && b.is_ready());

        let id = accepted_file(&mut a, &mut b, &[1]);
        let cancel_a = a.cancel_file(id).unwrap();
        let cancel_b = b.cancel_file(id).unwrap();
        a.receive(&cancel_b).unwrap();
        b.receive(&cancel_a).unwrap();
        let ack_a = a.acknowledge_file_cancel(id).unwrap();
        let ack_b = b.acknowledge_file_cancel(id).unwrap();
        a.receive(&ack_b).unwrap();
        b.receive(&ack_a).unwrap();
        assert!(a.is_ready() && b.is_ready());
    }

    #[test]
    fn cancellation_still_rejects_duplicate_controls_and_malformed_drained_data() {
        for kind in [11, 12] {
            let (mut a, mut b) = ready_pair();
            let id = accepted_file(&mut a, &mut b, &[1]);
            let cancel = a.cancel_file(id).unwrap();
            b.receive(&cancel).unwrap();
            if kind == 11 {
                let duplicate = a.encrypt(&raw_file(11, id, &[])).unwrap();
                assert!(b.receive(&duplicate).is_err());
            } else {
                let ack = b.acknowledge_file_cancel(id).unwrap();
                a.receive(&ack).unwrap();
                let duplicate = b.encrypt(&raw_file(12, id, &[])).unwrap();
                assert!(a.receive(&duplicate).is_err());
            }
        }
        let (mut a, mut b) = ready_pair();
        let id = accepted_file(&mut a, &mut b, &[1]);
        b.cancel_file(id).unwrap();
        let malformed = a.encrypt(&raw_file(8, id, &[0; 10])).unwrap();
        assert!(b.receive(&malformed).is_err());
        assert!(b.is_closed());
    }

    #[test]
    fn message_and_file_ids_share_one_namespace_and_lock_discards_transfers() {
        let (mut a, mut b) = ready_pair();
        let (id, chat) = a.send_chat("synthetic namespace check").unwrap();
        b.receive(&chat).unwrap();
        let mut reused = raw_file(6, id, &0_u64.to_be_bytes());
        reused.extend_from_slice(&[0; 32]);
        reused.extend_from_slice(b"synthetic");
        assert!(b.receive(&a.encrypt(&reused).unwrap()).is_err());
        let (mut a, mut b) = ready_pair();
        let id = accepted_file(&mut a, &mut b, &[0; 1]);
        a.abort();
        b.abort();
        assert!(a.send_file_chunk(id, 0, &[0]).is_err());
        assert!(b.file_stored(id).is_err());
        assert!(a.incoming_file.is_none() && a.outgoing_file.is_none());
        let (mut cancelled_a, mut cancelled_b) = ready_pair();
        let cancelled_id = accepted_file(&mut cancelled_a, &mut cancelled_b, &[1]);
        let cancel = cancelled_a.cancel_file(cancelled_id).unwrap();
        cancelled_b.receive(&cancel).unwrap();
        cancelled_a.abort();
        cancelled_b.abort();
        assert!(cancelled_a.cancelled_files.is_empty());
        assert!(cancelled_b.pending_cancel_ack.is_empty());
        assert!(cancelled_b.acknowledge_file_cancel(cancelled_id).is_err());
        let (mut fresh_a, mut fresh_b) = ready_pair();
        let id = accepted_file(&mut fresh_a, &mut fresh_b, &[8]);
        transfer_file(&mut fresh_a, &mut fresh_b, id, &[8]);
    }

    #[test]
    fn authenticated_frame_budget_includes_chunks_and_controls() {
        let two_maximum_files = 2 * (MAX_FILE_SIZE.div_ceil(MAX_FILE_CHUNK as u64) + 3) + 2;
        assert!(MAX_SESSION_FRAMES as u64 > two_maximum_files);
        let (mut a, mut b) = ready_pair();
        let id = accepted_file(&mut a, &mut b, &[0; 1]);
        a.sent_frames = MAX_SESSION_FRAMES;
        assert!(matches!(
            a.send_file_chunk(id, 0, &[0]),
            Err(ProtocolError::SessionLimit)
        ));
        let (mut a, mut b) = ready_pair();
        let (_, frame) = a.send_chat("synthetic last frame").unwrap();
        b.received_frames = MAX_SESSION_FRAMES;
        assert!(matches!(
            b.receive(&frame),
            Err(ProtocolError::SessionLimit)
        ));
        assert!(b.is_closed());
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
    fn handshake_authentication_rejects_modified_responder_message() {
        struct Tampered(MemoryPipe);
        impl Read for Tampered {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                self.0.read(output)
            }
        }
        impl Write for Tampered {
            fn write(&mut self, input: &[u8]) -> io::Result<usize> {
                let mut changed = input.to_vec();
                if changed.len() > 32 {
                    let last = changed.len() - 1;
                    changed[last] ^= 0x80;
                }
                self.0.write(&changed)
            }
            fn flush(&mut self) -> io::Result<()> {
                self.0.flush()
            }
        }
        let (mut a, b) = memory_pair();
        let identity = IdentityKey::from_private(&[7; 32]).unwrap();
        let other = IdentityKey::from_private(&[9; 32]).unwrap();
        let responder =
            std::thread::spawn(move || handshake(&mut Tampered(b), &other, Role::Responder));
        assert!(matches!(
            handshake(&mut a, &identity, Role::Initiator),
            Err(ProtocolError::Crypto)
        ));
        drop(a);
        assert!(responder.join().unwrap().is_err());
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
