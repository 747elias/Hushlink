# Hushlink wire protocol v1

This document describes the implementation in `src/protocol.rs`; it is not an
external security audit. Application traffic travels only through Windows
Bluetooth Classic RFCOMM. The in-memory byte transport exists only in tests.

## Framing and handshake

Each peer first writes the five bytes `48 55 53 48 01` (`HUSH`, version 1), then
reads the same prefix. Every subsequent handshake or transport message is a
4-byte unsigned big-endian length followed by exactly that many bytes. Valid
lengths are 1–32768 bytes, rejected before allocation; the underlying transport
must impose I/O and overall handshake deadlines. There are exactly three Noise
XX messages, with empty application handshake payloads. No profile names or
chat content appear outside encrypted transport messages.

The pinned snow implementation uses
`Noise_XX_25519_ChaChaPoly_SHA256`, prologue
`Hushlink/v1/RFCOMM/Noise_XX_25519_ChaChaPoly_SHA256`. The local static X25519
identity lives in the encrypted profile; ephemeral handshake keys are newly
generated on every connection. snow's `into_transport_mode` produces distinct
send/receive cipher states. The application never reads, resets, serializes or
clones their keys or counters. Disconnect, lock, protocol error and
reconnection discard the entire session. A reconnect always uses a fresh
handshake. There is no session resumption or transport fallback.

The authenticated transport plaintext begins with one version byte `01`, one
type byte and the following payload:

| Type | Payload | State and validation |
| --- | --- | --- |
| `01` Hello | 1–80 UTF-8 bytes of display name | Exactly once per direction; nonblank, no control characters |
| `02` Confirm | Empty | Both introductions complete; local contact lookup required before sending; exactly once |
| `03` Chat | 16 random ID bytes, then 1–16384 UTF-8 bytes | Both confirmations present locally; nonblank, no NUL; unique ID per direction |
| `04` Ack | Exactly 16 message ID bytes | Both confirmations present; refers to one outstanding sent Chat, exactly once |
| `05` Close | Empty | Ends the session and clears cipher state |

Text length limits count UTF-8 bytes, not characters. Newlines, umlauts and
emoji are supported in chat. IDs come from the OS CSPRNG. Each direction allows
at most 4096 chats per connection, bounding duplicate and acknowledgement
tracking. The UI can reconnect to start a new session when this limit is reached.
The transport actor adds bounded queues and deadlines; it must disconnect on
partial, truncated or timed-out frames, never resume parsing at a new header
after consuming an incomplete frame.

An acknowledgement must be emitted only after the receiving application has
saved the message in its encrypted vault. Local socket write completion means
“Gesendet”, while authenticated Ack means “Empfangen”. Unacknowledged messages
must not be silently retransmitted after reconnect. Storage errors must not
produce a false delivery acknowledgement.

## Human verification and contact binding

XX authenticates possession of identity keys, not the human behind them. Both
people compare the **complete** fingerprint over an independent trusted channel
before their first chat. SHA-256 is computed over the exact concatenation:

```
"Hushlink identity fingerprint\0v1\0"
|| "Hushlink/v1/RFCOMM/Noise_XX_25519_ChaChaPoly_SHA256"
|| min(local_static_public, remote_static_public)
|| max(local_static_public, remote_static_public)
```

Keys are ordered lexicographically as 32-byte arrays. The 256-bit digest is
shown as 64 uppercase hexadecimal digits in sixteen groups of four. This is
the conventional full digest representation, not a short PIN. Both devices
show the same result. Names and Bluetooth device addresses are not identity
proof. A person confirms on their own device, which sends the authenticated
Confirm message. Neither device releases or sends Chat/Ack until its local
confirmation and the peer's authenticated confirmation are both present.

The session requires a contact lookup before local confirmation. For an
existing selected contact, the expected stored public key must match the
Noise remote static key. A mismatch latches the session closed. The UI must
require a deliberate contact reset and a fresh connection for reverification;
`check_contact(None)` cannot revive the blocked session. Only mutually
confirmed identities are saved as trusted contacts in the encrypted vault.
Known contacts still exchange local and remote confirmations on every new
session; this keeps the gate explicit and avoids relying on Bluetooth names.

## Fail-closed handling

Any AEAD authentication error, unknown version/type, invalid UTF-8, invalid
length, duplicate message ID, duplicated control, unexpected Ack or Chat
before mutual confirmation destroys the session and releases no plaintext
from the rejected frame. A replayed ciphertext fails because the directional
nonce state has advanced. A malicious authenticated peer re-encrypting an old
message ID is also rejected. Control traffic and delivery acknowledgements
use the same authenticated encryption as chats.

The `Session` API validates protocol state; the actor additionally owns session
cancellation, acceptance/rejection, durable storage, trust persistence and
lock generation checks. Lock must drop all sessions, queued plaintext work and
UI copies. Zeroization is best-effort in a process with compiler, graphics,
OS paging and crash-dump limitations; an unlocked or compromised endpoint can
expose all displayed content.

## Observable metadata and limitations

Bluetooth discovery, addresses, presence, connection timing, sizes and traffic
patterns remain observable. Hushlink does not promise anonymity or conceal the
existence of a connection. A compromised endpoint or an attacker controlling
both fingerprint-comparison channels defeats the intended trust check. Forward
secrecy protects past connection traffic after later identity-key compromise;
it does not protect histories already stored in a vault that is later unlocked.
There is no padding, group chat, file transfer, message expiry or cryptographic
physical erasure guarantee.

## Sources checked

- [Noise specification](https://noiseprotocol.org/noise.html), XX patterns,
  transport `Split()` states, framing size ceiling and security properties.
- [snow 0.10.0](https://docs.rs/snow/0.10.0/snow/), `Builder`,
  `HandshakeState`, `TransportState` and default X25519 resolver API.
- [snow 0.10.0 implementation](https://github.com/mcginty/snow/tree/v0.10.0),
  pinned dependency matching the API used by this application.

These references were accessed during implementation on 2026-11-13.
