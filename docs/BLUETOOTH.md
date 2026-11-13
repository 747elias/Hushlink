# Windows Bluetooth Classic implementation and technical spike

## Selected API and packaging

Hushlink uses the **Win32 desktop Winsock Bluetooth provider**, not WinRT
`RfcommServiceProvider`. It creates `AF_BTH / SOCK_STREAM / BTHPROTO_RFCOMM`
sockets. This supports both initiating a connection and accepting one in a
normal unpackaged Windows 11 desktop EXE. No Appx/MSIX identity, Bluetooth
manifest capability or service registration installer is required for this
Win32 path. The Windows Bluetooth stack, working Classic adapter/driver and
operating-system authorization/pairing remain necessary. A BLE-only adapter or
Bluetooth emulation inside a build runner does not establish RFCOMM support.

Stable application service UUID: **`7d2ea28a-f7bd-4a6d-b1f4-2cf2f653cc0e`**.
The UUID is not a credential or proof of identity.

The server uses an automatically allocated channel (`BT_PORT_ANY`), obtains it
with `getsockname`, starts listening, then publishes a simple SDP record with
`WSASetServiceW(RNRSERVICE_REGISTER)`. Both `CSADDR_INFO` addresses point to the
bound `SOCKADDR_BTH`, following Microsoft's WinsockBluetoothConnection sample.
The SDP record advertises only the generic name “Hushlink” and the application
UUID, never the encrypted local profile name. Dropping the listener removes the
record with `RNRSERVICE_DELETE`; Windows also removes records on process exit.

The client sets the target Bluetooth address and service UUID, with port zero.
Windows performs the matching SDP resolution for that UUID as part of `connect`.
There is no IP, WLAN, Internet or TCP fallback. The accepted socket is only a
transport; the UI must approve the incoming request and the Noise/trust state
machine must complete before user messages can be sent.

Both directions require Windows Bluetooth authentication and link encryption
(`SO_BTH_AUTHENTICATE`, `SO_BTH_ENCRYPT`) before connection establishment. Windows
owns any required system pairing prompt. This provides defense in depth and
**does not replace** Hushlink's independent identity/fingerprint confirmation.
If pairing fails, Hushlink explains how to pair in Settings and retry. The
“Windows-Bluetooth-Einstellungen” command opens the fixed URI
`ms-settings:bluetooth`; no device names enter a command shell.

## Discovery and visibility

Discovery is explicit: `BluetoothFindFirstDevice` runs one 6.4-second Classic
inquiry and includes remembered/paired devices, with at most 64 returned entries.
These are **candidate Bluetooth devices**, not authenticated Hushlink contacts.
A candidate may be stale, out of range or run a different application. Hushlink's
UUID service is checked when connecting; the UI must say this and report a
missing service truthfully. Names and addresses are never identity proofs.

The app does not silently enable discoverability or permanently change the
radio's visibility. A user may need to make a previously unpaired PC visible in
Windows Settings while the other PC searches. Already paired PCs can reconnect
without inquiry if their saved address remains valid. `available()` checks a
real local radio, its connectable/discoverable flags, and actual creation of an
RFCOMM socket. Disabled radio, unsupported provider and access/pairing failures
produce actionable errors rather than successful synthetic states.

## Worker, bounds and cancellation

All calls are worker-only. The inquiry has a 6.4-second OS budget; cancellation
is checked before/after the OS call and between results. The OS call itself
cannot be interrupted with this API. No decrypted profile state is needed by
it. Connection and listener waits use nonblocking sockets and 100 ms `select`
polls, with a 45-second connection deadline and explicit cancellation flag.
Connected sockets default to 30-second read/write timeouts. The session worker
may choose a shorter read timeout with an incremental frame parser or a dedicated
reader; restarting `read_exact` after a partial timeout would lose frame state.

`Stream::try_clone` shares one socket: use one reader and one writer, never two
concurrent writers. `CancelHandle::cancel` sets an atomic flag and calls
`shutdown(SD_BOTH)` to wake I/O. The last `Arc` closes the socket exactly once,
avoiding handle-reuse races. Lock must cancel the session and clear its state;
workers must discard stale results after cancellation. No transport code logs
message payloads or keys.

## Early technical verification

The separate `tools/rfcomm-probe` crate includes the **production** transport
module directly; it does not have an alternate transport. This keeps the early
Windows API build independent of UI, vault and application integration.

On the Linux development host, the following checks passed against the pinned
Rust 1.91.1 toolchain and `windows-sys` 0.61.2 Windows SDK bindings:

```text
cargo check --locked --manifest-path tools/rfcomm-probe/Cargo.toml --target x86_64-pc-windows-msvc
cargo clippy --locked --manifest-path tools/rfcomm-probe/Cargo.toml --target x86_64-pc-windows-msvc --all-targets -- -D warnings
cargo test --locked --manifest-path tools/rfcomm-probe/Cargo.toml
```

Cross-checking typechecks **listener creation, bind/listen, service registration,
accept, client connection, actual I/O and cancellation code**. It does not link a
Windows EXE, execute a Windows API or demonstrate Bluetooth access. The CI
Windows runner builds this probe with MSVC separately from the full app. A
successful runner build still does not demonstrate access to physical radios.

### Actual two-PC probe (still requires hardware)

Build on a Windows 11 PC from the repository:

```powershell
cargo build --locked --manifest-path tools/rfcomm-probe/Cargo.toml --release
$probe = '.\tools\rfcomm-probe\target\release\hushlink-rfcomm-probe.exe'
& $probe status
& $probe scan
```

Pair two Windows 11 PCs using Windows Settings and verify that each supports
Classic. On PC A: `& $probe listen`. On PC B:
`& $probe connect AA:BB:CC:DD:EE:FF`, using PC A's address returned by discovery.
The probe sends and echoes only the fixed synthetic bytes
`HUSHLINK-RFCOMM-PROBE-V1`. It is **not an encrypted chat** and must never be
modified to handle user data. Verify the client prints its two-way success and
the server prints the actual received address. Then swap roles to test each
PC's registration, incoming and outgoing capability. Also test missing service,
Bluetooth off, rejected pairing, loss of range and stopping the process; no
success should be reported for these cases. The GUI manual test separately
covers Noise, trust, persistence and locking.

No physical two-PC probe has been performed in this Linux cloud environment.
This is an explicit pre-release hardware validation gap.

## Official documentation consulted

Consulted 2026-11-13. These pages describe the API actually used, rather than
WinRT requirements for a different transport:

- [Bluetooth and socket](https://learn.microsoft.com/en-us/windows/win32/bluetooth/bluetooth-and-socket)
- [Bluetooth and bind](https://learn.microsoft.com/en-us/windows/win32/bluetooth/bluetooth-and-bind)
- [Bluetooth and connect](https://learn.microsoft.com/en-us/windows/win32/bluetooth/bluetooth-and-connect)
- [Bluetooth and WSASetService](https://learn.microsoft.com/en-us/windows/win32/bluetooth/bluetooth-and-wsasetservice)
- [Bluetooth and socket options](https://learn.microsoft.com/en-us/windows/win32/bluetooth/bluetooth-and-socket-options)
- [Bluetooth service discovery](https://learn.microsoft.com/en-us/windows/win32/bluetooth/bluetooth-and-wsalookupservicebegin-for-service-discovery)
- [BluetoothFindFirstDevice and inquiry multiplier](https://learn.microsoft.com/en-us/windows/win32/api/bluetoothapis/nf-bluetoothapis-bluetoothfindfirstdevice)
- [Microsoft Windows Classic Samples: WinsockBluetoothConnection](https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/WinsockBluetoothConnection/cpp)

The source and SDK API contracts establish that the chosen desktop API offers
server registration, incoming acceptance and outgoing initiation without MSIX.
Hardware tests establish whether particular adapters, drivers and Windows
configuration actually support that path; compilation cannot establish that.
