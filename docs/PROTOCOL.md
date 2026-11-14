# Hushlink wire protocol v4

This document describes `src/protocol.rs`; it is not an external security audit.
Application traffic travels only through Windows Bluetooth Classic RFCOMM. The
in-memory byte transport exists only in tests. Wire v4 requires Hushlink v0.4 or
later on **both** devices. It deliberately rejects older wire prefixes rather
than downgrading or silently omitting file-transfer consent. Vault migration is
independent of the wire version; existing pinned contact identities are retained.

## Framing and handshake

Each peer first writes the five bytes `48 55 53 48 04` (`HUSH`, version 4), then
reads the same prefix. Every subsequent handshake or transport message is a
4-byte unsigned big-endian length followed by exactly that many bytes. Valid
lengths are 1–32768 bytes, rejected before allocation; the underlying transport
must impose I/O and overall handshake deadlines. There are exactly three Noise
XX messages, with empty application handshake payloads. No profile names, chat
content, filenames, file digests or file bytes appear outside encrypted transport
messages.

The pinned snow implementation uses
`Noise_XX_25519_ChaChaPoly_SHA256`, prologue
`Hushlink/v4/RFCOMM/Noise_XX_25519_ChaChaPoly_SHA256`. The local static X25519
identity lives in the encrypted profile; ephemeral handshake keys are newly
generated on every connection. snow's `into_transport_mode` produces distinct
send/receive cipher states. The application never reads, resets, serializes or
clones their keys or counters. Disconnect, lock, protocol error and
reconnection discard the entire session. A reconnect always uses a fresh
handshake. There is no cryptographic session resumption or transport fallback. Application file bytes may resume inside a completely fresh authenticated session as described below.

The authenticated transport plaintext begins with version byte `03`, one type
byte, then the following payload. Integers below are unsigned big-endian.

| Type | Payload | State and validation |
| --- | --- | --- |
| `01` Hello | 1–80 UTF-8 bytes of display name | Exactly once per direction; nonblank, no control characters |
| `02` Confirm | Empty | Both introductions complete; local contact lookup required before sending; exactly once |
| `03` Chat | 16 random ID bytes, then 1–16384 UTF-8 bytes | Both confirmations present locally; nonblank, no NUL; unique ID |
| `04` Ack | Exactly 16 message ID bytes | Both confirmations present; one outstanding sent Chat, exactly once |
| `05` Close | Empty | Ends the session and clears cipher state |
| `06` FileOffer | 16-byte ID, 8-byte size, 32-byte SHA-256, 1–180 UTF-8 bytes of basename | Mutual trust; new ID; at most one active incoming transfer; size ≤20 MiB |
| `07` FileAccept | Exactly 16 file ID bytes | Explicit recipient consent; outgoing offer, exactly once |
| `08` FileChunk | 16-byte ID, 8-byte offset, 1–8192 data bytes | Accepted incoming file; exact next offset; no overflow or data beyond offered size |
| `09` FileComplete | Exactly 16 file ID bytes | Accepted incoming file; accumulated length equals offered size; exactly once |
| `0A` FileStored | Exactly 16 file ID bytes | Completed outgoing file; receiver verified digest and durably saved encrypted bytes; exactly once |
| `0B` FileCancel | Exactly 16 file ID bytes | Either party stops an active file; also handles a cancellation crossing completion; exactly once |
| `0C` FileCancelAck | Exactly 16 file ID bytes | Acknowledges one local cancellation after the peer stopped its transfer; exactly once |
| `0D` FileResume | 16-byte ID, 8-byte offset, 32-byte SHA-256 of stored prefix | Explicit renewed recipient consent; outgoing offer; 0 < offset ≤ offered size; exactly once |
| `0E` Action | 16-byte operation ID followed by the bounded action below | Both confirmations; unique shared-namespace ID |
| `0F` ActionAck | Exactly 16 operation-ID bytes | One pending action; durable encrypted persistence first |
| `10` Typing | 4-byte big-endian sequence, 1-byte boolean 0/1 | Both confirmations; exact next sequence starting at 1 |

Text limits count UTF-8 bytes, not characters. Newlines, umlauts and emoji are
supported in chat. IDs come from the OS CSPRNG. Chat and file offers share an ID
namespace across both directions: a received ID may not reuse a local ID or any
previous received ID. Each direction allows at most 4096 new chat/file IDs and
65536 authenticated frames, including Hello, confirmations, chunks and controls.
The separate frame budget permits multiple 20 MiB transfers without a chat-ID
limit accidentally counting every chunk. These limits bound duplicate,
cancellation and acknowledgement tracking. Reconnect starts a new session;
unacknowledged content is never silently retransmitted.

The transport actor adds bounded queues and deadlines. It must disconnect on
partial, truncated or timed-out frames, never resume parsing at a new header
after consuming an incomplete frame. Blocking file operations run outside the
UI thread. The sender processes inbound controls between bounded outgoing
chunks so cancellation and simultaneous chat remain responsive.

## File consent, integrity and storage

A file progresses through `Offered → Accepted → Complete → Stored`. Sending data
before FileAccept, completing before the exact size, repeating a control or
reusing an offset fails closed. Zero-byte files still require offer, consent,
completion and storage acknowledgement, but no chunks. One transfer in each
direction may run concurrently; text and encrypted control frames share the
same ordered session. FileOffer metadata alone never permits automatic saving,
opening or executing a file.

The sender first takes a bounded, stable in-memory snapshot and hashes those
exact bytes. The recipient explicitly accepts or rejects the advertised file.
The protocol enforces chunk ordering and total size; the runtime must additionally
verify the SHA-256 digest of the complete bytes against the authenticated offer,
then persist the attachment inside the encrypted vault **before** calling
`file_stored`. A digest mismatch or storage failure must never produce FileStored.
This digest binds the whole-file snapshot; confidentiality and authenticity still
come from Noise's authenticated encryption, not from an unauthenticated checksum.

The filename is a display basename, never an export path. The parser rejects
Windows-invalid characters, path separators, device names (including reserved
COM/LPT variants), control characters, directional override/isolate characters
and trailing spaces/dots. Unicode names remain supported within the byte limit.
Export requires a separately chosen local destination; receiving never writes a
peer-supplied path or opens an attachment. Stored attachments count toward the
vault's own storage bounds. Deleting or cancelling does not promise physical
erasure or deletion of a copy already stored by the other person.

Socket write completion is only a local send result. An authenticated Chat Ack
or FileStored means the recipient saved the respective content in its encrypted
vault. An unfinished file must not be labelled received merely because its last
chunk or FileComplete was written. A completed file may already have reached the
recipient when cancellation crosses FileStored; cancellation cannot revoke that
copy, and the cancelling side does not treat the crossing receipt as success.

## Cancellation races

Local cancellation stops new outgoing chunks and sends FileCancel. Its protocol
state is retained as a bounded tombstone until the peer's FileCancelAck arrives.
Already-sent FileAccept, FileChunk, FileComplete or FileStored can cross the
cancellation in the opposite direction. Such frames are still authenticated and
validated against the exact original direction, phase, offsets and size; they
produce only `FileDiscarded { id }`, with **no file plaintext or success event**.
Malformed, duplicate or out-of-order drained frames still abort the session.

On FileCancel, the peer stops the associated work and promptly calls
`acknowledge_file_cancel`. Ordered delivery means all its earlier transfer frames
precede that acknowledgement. After FileCancelAck, the tombstone is removed;
further frames for that cancelled ID fail closed. Simultaneous cancellations are
valid: each party stops, acknowledges the other's cancellation and drains until
its own acknowledgement. Repeated cancellation requests/acknowledgements remain
invalid. This avoids disconnecting ordinary chat merely because a chunk was
already in flight, without ever releasing data after the user cancelled.

Runtime commands resulting from earlier UI/storage work may arrive after a
transfer was cancelled. They must be discarded by matching active transfer ID
and session generation, rather than reviving the transfer. Reconnection always
starts a new offer with new consent; encrypted application checkpoints may be revalidated after lock, never the old cipher state.

## Human verification and contact binding

XX authenticates possession of identity keys, not the human behind them. Both
people compare the **complete** fingerprint over an independent trusted channel
before their first chat or file transfer. SHA-256 is computed over the exact
concatenation:

```
"Hushlink identity fingerprint\0v4\0"
|| "Hushlink/v4/RFCOMM/Noise_XX_25519_ChaChaPoly_SHA256"
|| min(local_static_public, remote_static_public)
|| max(local_static_public, remote_static_public)
```

Keys are ordered lexicographically as 32-byte arrays. The 256-bit digest is
shown as 64 uppercase hexadecimal digits in sixteen groups of four. This is
the conventional full digest representation, not a short PIN. Both devices
show the same result. Names and Bluetooth device addresses are not identity
proof. A person confirms on their own device, which sends the authenticated
Confirm message. Neither device releases or sends Chat, Ack or file traffic until
its local confirmation and the peer's authenticated confirmation are both present.

The session requires a contact lookup before local confirmation. For an
existing selected contact, the expected stored public key must match the
Noise remote static key. A mismatch latches the session closed. The UI must
require a deliberate contact reset and a fresh connection for reverification;
`check_contact(None)` cannot revive the blocked session. Only mutually
confirmed identities are saved as trusted contacts in the encrypted vault.
Known contacts still exchange local and remote confirmations on every new
session; the actor may issue the local confirmation using the previously stored
human trust after matching the pinned key. Both current-session authenticated
confirmations remain necessary. Bluetooth names cannot authorize a connection.

## Fail-closed handling

Any AEAD authentication error, unknown version/type, invalid UTF-8, invalid
length, duplicate ID/control, unexpected acknowledgement or application traffic
before mutual confirmation destroys the session and releases no plaintext from
the rejected frame. A replayed ciphertext fails because the directional nonce
state has advanced. A malicious authenticated peer re-encrypting an old ID is
also rejected. Control traffic and delivery acknowledgements use the same
authenticated encryption as chats and files.

The `Session` API validates protocol state; the actor additionally owns session
cancellation, acceptance/rejection, durable storage, digest checking, trust
persistence and lock generation checks. Lock must drop all sessions, queued
plaintext work and UI copies. Resolver adapters delegate all cryptography to
snow and overwrite its default DH/cipher key slots through the library API on
drop. snow internal handshake chaining/transcript buffers and temporary primitive
copies are not fully under application control. Zeroization is best-effort with
compiler, graphics, OS paging and crash-dump limitations; an unlocked or
compromised endpoint can expose all displayed content and file bytes.

## Observable metadata and limitations

Bluetooth discovery, addresses, presence, connection timing, sizes and traffic
patterns remain observable. Hushlink does not promise anonymity or conceal the
existence of a connection. A compromised endpoint or an attacker controlling
both fingerprint-comparison channels defeats the intended trust check. Forward
secrecy protects past connection traffic after later identity-key compromise;
it does not protect histories and attachments already stored in a vault that is
later unlocked. There is no padding, group chat, message expiry or cryptographic
physical erasure guarantee. Exported files are ordinary plaintext files at a
user-selected destination; users must trust their origin before opening them.

## Sources checked

- [Noise specification](https://noiseprotocol.org/noise.html), XX patterns,
  transport `Split()` states, framing size ceiling and security properties.
- [snow 0.10.0](https://docs.rs/snow/0.10.0/snow/), `Builder`,
  `HandshakeState`, `TransportState` and default X25519 resolver API.
- [snow 0.10.0 implementation](https://github.com/mcginty/snow/tree/v0.10.0),
  pinned dependency matching the API used by this application.

These references were accessed during the original implementation on 2026-11-13.
Wire v2 retains those cryptographic primitives and adds authenticated file states.

## Encrypted file checkpoints (v3)

The sender retains its immutable complete source snapshot in the encrypted vault.
A failed outgoing attachment can be explicitly offered again with the same
application ID **in a new session**. Each session still spends every ID only once;
no Noise key, counter or ciphertext is retained for resumption. Both identities
must pass the ordinary fresh-session trust gate. A changed contact key cannot
select the old identity's checkpoint.

The recipient binds a checkpoint to the contact identity, transfer ID, basename,
full expected size and full SHA-256. Any metadata mismatch rejects the offer.
Renewed explicit consent is always required. With no saved bytes it sends normal
FileAccept; otherwise FileResume includes the exact saved length and SHA-256 of
that prefix. The sender verifies this digest against its immutable snapshot
before sending bytes after that offset. Missing, malformed, oversized, repeated
or state-invalid resume controls abort. Cancellation may drain an in-flight
resume control using the existing authenticated cancellation acknowledgement.

A checkpoint is encrypted after each additional 1 MiB and on graceful disconnect
or lock. Abrupt process loss may require retransmitting the last uncheckpointed
section. No partial attachment can be exported. Only exact full size, full digest
verification and durable final encrypted storage permit FileStored. After a lost
receipt, the recipient can revalidate an already complete file and acknowledge
it again in the new session without duplicate history, unread counts or files.
Explicit cancel/reject clears received checkpoint bytes; history deletion or
contact removal deletes their checkpoint records as well.

## Conversation operations introduced in v4

The Action body starts with a one-byte kind and the referenced 16-byte message ID.
Reply (kind 1) appends UTF-8 text, 1–16384 bytes and nonempty after trimming.
Edit (kind 2) appends a 4-byte big-endian revision followed by bounded UTF-8 text.
React (kind 3) appends the same revision followed by exactly one of the fixed
UTF-8 values: empty (clear), 👍, ❤️, 😂, 🎉, 😮, 😢, ✅. NUL and invalid UTF-8 are
rejected. Edit/reaction revisions are 1–1000, exactly one above the stored value.
These operations share the message/file ID namespace and all frame budgets.

The runtime binds every operation to the authenticated contact. Remote edits may
only change that contact's own received text messages; attachments cannot be
edited. A reply links the original ID without transmitting an extra plaintext
quote. If the other party no longer has the original, it displays an explicit
missing-reference label. Pins and search are entirely local and encrypted at rest.

Outgoing operations are persisted before a worker may write them. Their changes
remain visibly unconfirmed until ActionAck; a reply has the normal message delivery
states. Up to 64 pending operations survive disconnect/lock/restart and can be
explicitly retried after a fresh mutually trusted session. There is no automatic
retransmission and no serialization of cipher counters. A session never sends the
same operation twice. Receivers keep up to 10000 encrypted receipt records binding
contact, operation ID and SHA-256 of the complete canonical operation. A matching
retry in a fresh session is acknowledged without a second mutation, unread count
or notification; a reused ID with different content or contact is rejected.
Receipt IDs/digests remain after local message/history deletion so late retries
cannot recreate removed replies. Forgetting the contact also removes its receipts.
At the receipt limit remove/reconfirm the contact or use a new profile.

Typing is optional, disabled by default, contains no draft text and is sent at most
once every three seconds while typing. A false state ends it; receiving nothing
for six seconds clears the UI indicator. Each direction enforces an exact increasing
sequence and the common encrypted-frame limit. Incoming typing, actions and receipts
before mutual trust or after closure are rejected. All malformed, manipulated,
replayed or state-invalid controls abort the session.

The camera QR payload is `HUSHLINK:VERIFY:4:` followed by all 64 uppercase hexadecimal
fingerprint digits without spaces. It binds the same canonical identities and v4
context as the text display. It is never opened as a URL or treated as an automatic
connection approval. Both users still confirm independently.
