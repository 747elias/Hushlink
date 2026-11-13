//! Win32 desktop Bluetooth: Winsock AF_BTH/RFCOMM and simple SDP records.
//!
//! Every unsafe call below uses SDK-defined layouts, valid owned handles and
//! pointers whose storage remains alive until the synchronous call returns.
use super::*;
use anyhow::{Context, Result, bail, ensure};
use std::{
    io::{self, Read, Write},
    mem::size_of,
    ptr::null_mut,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Devices::Bluetooth::*,
        Foundation::{CloseHandle, ERROR_NO_MORE_ITEMS, GetLastError, HANDLE},
        Networking::WinSock::{self as ws, *},
    },
    core::GUID,
};

const SERVICE_ID: GUID = GUID::from_u128(0x7d2ea28a_f7bd_4a6d_b1f4_2cf2f653cc0e);
const BT_PORT_ANY: u32 = u32::MAX;
const POLL: Duration = Duration::from_millis(100);
const INQUIRY_MULTIPLIER: u8 = 5; // Windows units of 1.28 seconds: 6.4 seconds.
const MAX_DISCOVERED_DEVICES: usize = 64;

fn startup() -> Result<()> {
    static STARTED: OnceLock<i32> = OnceLock::new();
    let code = *STARTED.get_or_init(|| {
        let mut data = WSADATA::default();
        // SAFETY: data is a valid output buffer; Winsock remains initialized for
        // the process lifetime, including all shared socket handles.
        unsafe { WSAStartup(0x0202, &mut data) }
    });
    ensure!(
        code == 0,
        "Die Windows-Bluetooth-Schnittstelle konnte nicht gestartet werden (Windows-Fehler {code})."
    );
    Ok(())
}

fn last_error() -> i32 {
    // SAFETY: thread-local error query has no preconditions.
    unsafe { WSAGetLastError() }
}

fn explain(code: i32, action: &str) -> anyhow::Error {
    let hint = match code {
        WSAEACCES => {
            "Zugriff oder Bluetooth-Pairing abgelehnt. Koppeln Sie beide PCs in den Windows-Bluetooth-Einstellungen und bestätigen Sie die Windows-Abfrage."
        }
        WSAEAFNOSUPPORT | WSAEPROTONOSUPPORT | WSAEPFNOSUPPORT => {
            "Der Adapter oder Treiber unterstützt Bluetooth Classic/RFCOMM nicht. Ein reiner BLE-Adapter reicht nicht aus."
        }
        WSAENETDOWN | WSAENETUNREACH | WSAEHOSTDOWN => {
            "Bluetooth ist nicht erreichbar. Funk, Reichweite und Windows-Bluetooth-Dienst prüfen."
        }
        WSAETIMEDOUT => {
            "Zeitüberschreitung. Gegenstelle, Reichweite und Pairing prüfen; auf dem anderen PC muss Hushlink den Dienst anbieten."
        }
        WSAECONNREFUSED | WSASERVICE_NOT_FOUND | WSAEADDRNOTAVAIL => {
            "Kein erreichbarer Hushlink-Dienst. Auf der Gegenstelle „Verbindung anbieten“ aktivieren und gegebenenfalls beide PCs koppeln."
        }
        WSAEADDRINUSE => {
            "Der Bluetooth-Kanal ist belegt. Andere Hushlink-Instanzen schliessen und erneut versuchen."
        }
        WSAECONNRESET | WSAECONNABORTED | WSAEDISCON | WSAESHUTDOWN => {
            "Die Bluetooth-Verbindung wurde getrennt."
        }
        _ => "Windows-Bluetooth-Einstellungen, Adapter und Treiber prüfen.",
    };
    anyhow::anyhow!("{action}: {hint} (Windows-Fehler {code})")
}

fn check(result: i32, action: &str) -> Result<()> {
    if result == SOCKET_ERROR {
        Err(explain(last_error(), action))
    } else {
        Ok(())
    }
}

fn utf16_label(value: &[u16]) -> String {
    let end = value.iter().position(|v| *v == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
        .chars()
        .filter(|c| !c.is_control())
        .take(248)
        .collect()
}

struct Radio(HANDLE);
impl Drop for Radio {
    fn drop(&mut self) {
        // SAFETY: owned radio handle was returned by BluetoothFindFirstRadio.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn radio() -> Result<Radio> {
    let params = BLUETOOTH_FIND_RADIO_PARAMS {
        dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
    };
    let mut handle = null_mut();
    // SAFETY: correctly sized initialized parameters and output handle.
    let find = unsafe { BluetoothFindFirstRadio(&params, &mut handle) };
    if find.is_null() {
        // SAFETY: thread-local error query.
        let code = unsafe { GetLastError() };
        bail!(
            "Kein aktiver Bluetooth-Classic-Adapter gefunden. Bluetooth einschalten, Flugmodus ausschalten und den Adapter im Geräte-Manager prüfen (Windows-Fehler {code})."
        );
    }
    // SAFETY: enumeration handle is valid and no longer needed.
    unsafe {
        BluetoothFindRadioClose(find);
    }
    ensure!(
        !handle.is_null(),
        "Windows hat keinen Bluetooth-Adapter zurückgegeben."
    );
    Ok(Radio(handle))
}

pub fn available() -> Result<Availability> {
    startup()?;
    let radio = radio()?;
    let mut info = BLUETOOTH_RADIO_INFO {
        dwSize: size_of::<BLUETOOTH_RADIO_INFO>() as u32,
        ..Default::default()
    };
    // SAFETY: valid owned handle and correctly sized output structure.
    let code = unsafe { BluetoothGetRadioInfo(radio.0, &mut info) };
    ensure!(
        code == 0,
        "Bluetooth-Adapter konnte nicht gelesen werden (Windows-Fehler {code})."
    );
    // Actually test the RFCOMM provider; finding a radio alone is insufficient.
    let _probe = Socket::create()?;
    Ok(Availability {
        radio_name: utf16_label(&info.szName),
        // SAFETY: valid handle, read-only Bluetooth API calls.
        connectable: unsafe { BluetoothIsConnectable(radio.0) != 0 },
        discoverable: unsafe { BluetoothIsDiscoverable(radio.0) != 0 },
    })
}

pub fn discover(cancel: &AtomicBool) -> Result<Vec<Device>> {
    cancelled(cancel)?;
    available()?;
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: 1,
        fReturnRemembered: 1,
        fReturnUnknown: 1,
        fReturnConnected: 1,
        fIssueInquiry: 1,
        cTimeoutMultiplier: INQUIRY_MULTIPLIER,
        hRadio: null_mut(),
    };
    let mut info = BLUETOOTH_DEVICE_INFO {
        dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    // SAFETY: initialized structures; the OS inquiry is bounded to 6.4 seconds.
    // Run only on a worker. A cancelled result is discarded after the OS returns.
    let find = unsafe { BluetoothFindFirstDevice(&params, &mut info) };
    if find.is_null() {
        // SAFETY: error query after the failed call.
        let code = unsafe { GetLastError() };
        cancelled(cancel)?;
        if code == ERROR_NO_MORE_ITEMS {
            return Ok(Vec::new());
        }
        bail!(
            "Bluetooth-Suche fehlgeschlagen. Funk und Windows-Bluetooth-Dienst prüfen (Windows-Fehler {code})."
        );
    }
    struct DeviceFind(HBLUETOOTH_DEVICE_FIND);
    impl Drop for DeviceFind {
        fn drop(&mut self) {
            // SAFETY: this guard uniquely owns the enumeration handle.
            unsafe {
                BluetoothFindDeviceClose(self.0);
            }
        }
    }
    let find = DeviceFind(find);
    let mut devices = Vec::new();
    loop {
        cancelled(cancel)?;
        // SAFETY: Bluetooth API initialized this union as a device address.
        let address = format_address(unsafe { info.Address.Anonymous.ullLong });
        if !devices.iter().any(|d: &Device| d.address == address) {
            let name = utf16_label(&info.szName);
            devices.push(Device {
                name: if name.is_empty() {
                    "Bluetooth-Gerät".to_owned()
                } else {
                    name
                },
                address,
            });
        }
        if devices.len() >= MAX_DISCOVERED_DEVICES {
            break;
        }
        info.dwSize = size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
        // SAFETY: valid enumeration handle and output structure.
        if unsafe { BluetoothFindNextDevice(find.0, &mut info) } == 0 {
            // SAFETY: read the error immediately after the enumeration call.
            let code = unsafe { GetLastError() };
            if code != ERROR_NO_MORE_ITEMS {
                bail!("Bluetooth-Suche wurde unterbrochen (Windows-Fehler {code}).");
            }
            break;
        }
    }
    cancelled(cancel)?;
    devices.sort_by(|a, b| a.name.cmp(&b.name).then(a.address.cmp(&b.address)));
    Ok(devices)
}

struct Socket {
    raw: SOCKET,
    cancelled: AtomicBool,
}
impl Socket {
    fn create() -> Result<Self> {
        startup()?;
        // SAFETY: valid constant address family, socket kind and RFCOMM protocol.
        let raw = unsafe { socket(AF_BTH as i32, SOCK_STREAM, BTHPROTO_RFCOMM as i32) };
        ensure!(
            raw != INVALID_SOCKET,
            "{}",
            explain(last_error(), "Bluetooth-Socket öffnen")
        );
        Ok(Self {
            raw,
            cancelled: AtomicBool::new(false),
        })
    }
    fn nonblocking(&self, enabled: bool) -> Result<()> {
        let mut flag = u32::from(enabled);
        // SAFETY: owned socket and valid pointer to one u32.
        check(
            unsafe { ioctlsocket(self.raw, FIONBIO, &mut flag) },
            "Bluetooth-Betriebsart setzen",
        )
    }
    fn option_u32(&self, level: i32, option: i32, value: u32) -> Result<()> {
        // SAFETY: pointer/length match the Windows ULONG socket option ABI.
        check(
            unsafe {
                setsockopt(
                    self.raw,
                    level,
                    option,
                    (&value as *const u32).cast(),
                    size_of::<u32>() as i32,
                )
            },
            "Bluetooth-Option setzen",
        )
    }
    fn secure_link(&self) -> Result<()> {
        self.option_u32(SOL_RFCOMM as i32, SO_BTH_AUTHENTICATE as i32, 1)?;
        self.option_u32(SOL_RFCOMM as i32, SO_BTH_ENCRYPT as i32, 1)
    }
    fn configure_stream(&self) -> Result<()> {
        self.nonblocking(false)?;
        self.option_u32(SOL_SOCKET, SO_RCVTIMEO, (IO_TIMEOUT_SECS * 1000) as u32)?;
        self.option_u32(SOL_SOCKET, SO_SNDTIMEO, (IO_TIMEOUT_SECS * 1000) as u32)
    }
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        // SAFETY: handle stays open while any Arc (including this caller) exists.
        // shutdown wakes blocked recv/send without handle-reuse races.
        unsafe {
            shutdown(self.raw, SD_BOTH);
        }
    }
    fn check_active(&self) -> io::Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Bluetooth-Verbindung geschlossen.",
            ))
        } else {
            Ok(())
        }
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        // SAFETY: last owner closes the handle exactly once.
        unsafe {
            closesocket(self.raw);
        }
    }
}

#[derive(Clone)]
pub struct CancelHandle(Arc<Socket>);
impl CancelHandle {
    pub fn cancel(&self) {
        self.0.cancel();
    }
}

pub struct Stream(Arc<Socket>);
impl Stream {
    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle(Arc::clone(&self.0))
    }
    /// Clones share the same byte stream. Use at most one reader and one writer.
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.check_active()?;
        Ok(Self(Arc::clone(&self.0)))
    }
    pub fn set_read_timeout(&self, timeout: Duration) -> io::Result<()> {
        let millis = timeout.as_millis().clamp(1, u32::MAX as u128) as u32;
        self.0
            .option_u32(SOL_SOCKET, SO_RCVTIMEO, millis)
            .map_err(io::Error::other)
    }
}
impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.check_active()?;
        if buf.is_empty() {
            return Ok(0);
        }
        let len = buf.len().min(i32::MAX as usize) as i32;
        // SAFETY: exclusive output slice covers len bytes and socket stays owned.
        let result = unsafe { recv(self.0.raw, buf.as_mut_ptr(), len, 0) };
        if result == SOCKET_ERROR {
            Err(io::Error::from_raw_os_error(last_error()))
        } else {
            Ok(result as usize)
        }
    }
}
impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.check_active()?;
        let len = buf.len().min(i32::MAX as usize) as i32;
        // SAFETY: immutable input slice covers len bytes and socket stays owned.
        let result = unsafe { send(self.0.raw, buf.as_ptr(), len, 0) };
        if result == SOCKET_ERROR {
            Err(io::Error::from_raw_os_error(last_error()))
        } else {
            Ok(result as usize)
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.check_active()
    }
}

fn fd_set(socket: SOCKET) -> FD_SET {
    let mut set = FD_SET {
        fd_count: 1,
        ..Default::default()
    };
    set.fd_array[0] = socket;
    set
}

fn wait(socket: SOCKET, writable: bool, timeout: Duration) -> Result<bool> {
    let mut wanted = fd_set(socket);
    let mut errors = fd_set(socket);
    let timeout = TIMEVAL {
        tv_sec: timeout.as_secs() as i32,
        tv_usec: timeout.subsec_micros() as i32,
    };
    // SAFETY: all fd sets and timeval remain valid throughout select; the socket
    // remains owned by the caller. Windows ignores nfds.
    let result = unsafe {
        select(
            0,
            if writable { null_mut() } else { &mut wanted },
            if writable { &mut wanted } else { null_mut() },
            &mut errors,
            &timeout,
        )
    };
    check(result, "Auf Bluetooth warten")?;
    Ok(result > 0)
}

pub fn connect(address: &str, cancel: &AtomicBool) -> Result<Stream> {
    cancelled(cancel)?;
    let remote = SOCKADDR_BTH {
        addressFamily: AF_BTH,
        btAddr: parse_address(address)?,
        serviceClassId: SERVICE_ID,
        port: 0,
    };
    let socket = Socket::create()?;
    socket.secure_link()?;
    socket.nonblocking(true)?;
    // SAFETY: packed SDK address layout and its exact size are supplied.
    let result = unsafe {
        ws::connect(
            socket.raw,
            (&remote as *const SOCKADDR_BTH).cast(),
            size_of::<SOCKADDR_BTH>() as i32,
        )
    };
    if result == SOCKET_ERROR {
        let code = last_error();
        if !matches!(code, WSAEWOULDBLOCK | WSAEINPROGRESS | WSAEALREADY) {
            return Err(explain(code, "Mit Hushlink verbinden"));
        }
        let deadline = Instant::now() + Duration::from_secs(CONNECT_TIMEOUT_SECS);
        loop {
            cancelled(cancel)?;
            if Instant::now() >= deadline {
                bail!("{}", explain(WSAETIMEDOUT, "Mit Hushlink verbinden"));
            }
            if wait(socket.raw, true, POLL)? {
                let mut code = 0i32;
                let mut len = size_of::<i32>() as i32;
                // SAFETY: output i32 and size match SO_ERROR's ABI.
                check(
                    unsafe {
                        getsockopt(
                            socket.raw,
                            SOL_SOCKET,
                            SO_ERROR,
                            (&mut code as *mut i32).cast(),
                            &mut len,
                        )
                    },
                    "Bluetooth-Verbindung prüfen",
                )?;
                if code != 0 {
                    return Err(explain(code, "Mit Hushlink verbinden"));
                }
                break;
            }
        }
    }
    cancelled(cancel)?;
    socket.configure_stream()?;
    Ok(Stream(Arc::new(socket)))
}

pub struct Listener {
    socket: Socket,
    address: SOCKADDR_BTH,
}
impl Listener {
    pub fn bind() -> Result<Self> {
        let status = available()?;
        ensure!(
            status.connectable,
            "Windows erlaubt derzeit keine eingehenden Bluetooth-Verbindungen. In den Bluetooth-Einstellungen Verbindungen zulassen."
        );
        let socket = Socket::create()?;
        socket.secure_link()?;
        let mut address = SOCKADDR_BTH {
            addressFamily: AF_BTH,
            port: BT_PORT_ANY,
            ..Default::default()
        };
        // SAFETY: SDK sockaddr layout and exact byte count.
        check(
            unsafe {
                bind(
                    socket.raw,
                    (&address as *const SOCKADDR_BTH).cast(),
                    size_of::<SOCKADDR_BTH>() as i32,
                )
            },
            "Bluetooth-Dienst binden",
        )?;
        let mut len = size_of::<SOCKADDR_BTH>() as i32;
        // SAFETY: address is a valid output buffer for the selected family.
        check(
            unsafe {
                getsockname(
                    socket.raw,
                    (&mut address as *mut SOCKADDR_BTH).cast(),
                    &mut len,
                )
            },
            "Bluetooth-Kanal lesen",
        )?;
        socket.nonblocking(true)?;
        // Listen before publishing: clients never discover an unready socket.
        // SAFETY: bound, valid RFCOMM socket and bounded backlog.
        check(
            unsafe { listen(socket.raw, 1) },
            "Bluetooth-Verbindungen anbieten",
        )?;
        let listener = Self { socket, address };
        listener.register(RNRSERVICE_REGISTER)?;
        Ok(listener)
    }
    fn register(&self, operation: WSAESETSERVICEOP) -> Result<()> {
        let mut address = self.address;
        let mut service_id = SERVICE_ID;
        let mut name: Vec<u16> = "Hushlink\0".encode_utf16().collect();
        let mut comment: Vec<u16> = "Hushlink Bluetooth chat v1\0".encode_utf16().collect();
        let socket_address = SOCKET_ADDRESS {
            lpSockaddr: (&mut address as *mut SOCKADDR_BTH).cast(),
            iSockaddrLength: size_of::<SOCKADDR_BTH>() as i32,
        };
        let mut endpoint = CSADDR_INFO {
            LocalAddr: socket_address,
            RemoteAddr: socket_address,
            iSocketType: SOCK_STREAM,
            iProtocol: BTHPROTO_RFCOMM as i32,
        };
        let query = WSAQUERYSETW {
            dwSize: size_of::<WSAQUERYSETW>() as u32,
            lpszServiceInstanceName: name.as_mut_ptr(),
            lpServiceClassId: &mut service_id,
            lpszComment: comment.as_mut_ptr(),
            dwNameSpace: NS_BTH,
            dwNumberOfCsAddrs: 1,
            lpcsaBuffer: &mut endpoint,
            ..Default::default()
        };
        // SAFETY: all nested buffers live through this synchronous registration,
        // with SDK layouts matching Microsoft's WinsockBluetoothConnection sample.
        check(
            unsafe { WSASetServiceW(&query, operation, 0) },
            "Hushlink-Dienst veröffentlichen",
        )
    }
    pub fn accept(&self, cancel: &AtomicBool) -> Result<(Stream, Device)> {
        loop {
            cancelled(cancel)?;
            if !wait(self.socket.raw, false, POLL)? {
                continue;
            }
            let mut remote = SOCKADDR_BTH::default();
            let mut len = size_of::<SOCKADDR_BTH>() as i32;
            // SAFETY: ready listening socket, correctly sized output address.
            let raw = unsafe {
                accept(
                    self.socket.raw,
                    (&mut remote as *mut SOCKADDR_BTH).cast(),
                    &mut len,
                )
            };
            if raw == INVALID_SOCKET {
                let code = last_error();
                if code == WSAEWOULDBLOCK {
                    continue;
                }
                return Err(explain(code, "Bluetooth-Verbindung annehmen"));
            }
            let socket = Socket {
                raw,
                cancelled: AtomicBool::new(false),
            };
            cancelled(cancel)?;
            socket.configure_stream()?;
            let address = format_address(remote.btAddr);
            let device = Device {
                name: "Eingehender Bluetooth-PC".to_owned(),
                address,
            };
            return Ok((Stream(Arc::new(socket)), device));
        }
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        // Best effort; Windows also removes this process's SDP records on exit.
        let _ = self.register(RNRSERVICE_DELETE);
    }
}

pub fn open_bluetooth_settings() -> Result<()> {
    // Constant URI and executable, no untrusted text enters a command shell.
    std::process::Command::new("explorer.exe")
        .arg("ms-settings:bluetooth")
        .spawn()
        .context("Die Windows-Bluetooth-Einstellungen konnten nicht geöffnet werden.")?;
    Ok(())
}
