//! Worker actors connecting the encrypted vault, authenticated session and real RFCOMM.
//! All KDF, file, Bluetooth and protocol work runs outside the eframe thread.

use crate::{desktop, protocol, storage_location, transport, vault};
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded, select};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[cfg(windows)]
mod windows_lock;

const COMMAND_CAPACITY: usize = 64;
const FRAME_DEADLINE: Duration = Duration::from_secs(30);
const MAX_NETWORK_FRAME: usize = protocol::MAX_FRAME;

pub enum Command {
    Create {
        name: String,
        password: Zeroizing<String>,
    },
    Unlock {
        password: Zeroizing<String>,
    },
    Discover,
    StartListening,
    StopListening,
    Connect {
        address: String,
    },
    Respond {
        accept: bool,
    },
    ConfirmPeer,
    AllowKeyChange,
    Send {
        text: Zeroizing<String>,
    },
    SendFile {
        path: PathBuf,
    },
    RespondFile {
        id: String,
        accept: bool,
    },
    CancelFile {
        id: String,
    },
    ExportFile {
        attachment_id: String,
        destination: PathBuf,
    },
    ChooseVault {
        path: PathBuf,
    },
    RelocateVault {
        path: PathBuf,
    },
    SetDesktopSettings {
        start_with_windows: bool,
        minimize_to_tray: bool,
        automatic_updates: bool,
    },
    Disconnect,
    DeleteHistory {
        contact_id: String,
    },
    ChangePassword {
        old: Zeroizing<String>,
        new: Zeroizing<String>,
    },
    SetInactivity {
        minutes: u32,
    },
    Lock,
    Shutdown,
}

impl Drop for Command {
    fn drop(&mut self) {
        match self {
            Self::Create { name, .. } => name.zeroize(),
            Self::Connect { address } => address.zeroize(),
            Self::DeleteHistory { contact_id } => contact_id.zeroize(),
            Self::RespondFile { id, .. } | Self::CancelFile { id } => id.zeroize(),
            Self::ExportFile { attachment_id, .. } => attachment_id.zeroize(),
            _ => {}
        }
    }
}

#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct DeviceView {
    pub name: String,
    pub address: String,
}
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct ContactView {
    pub id: String,
    pub name: String,
    pub address: String,
}
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MessageView {
    pub id: String,
    pub contact_id: String,
    pub text: String,
    pub attachment_id: Option<String>,
    pub attachment_name: Option<String>,
    pub attachment_size: u64,
    pub timestamp_ms: u64,
    pub outgoing: bool,
    #[zeroize(skip)]
    pub status: vault::MessageStatus,
}
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct PeerView {
    pub name: String,
    pub address: String,
    pub id: String,
    pub fingerprint: String,
    pub known: bool,
    pub local_confirmed: bool,
    pub ready: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileTransferState {
    AwaitingConsent,
    Transferring,
    Stored,
    Cancelled,
    Failed,
}
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct FileTransferView {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub transferred: u64,
    pub outgoing: bool,
    #[zeroize(skip)]
    pub state: FileTransferState,
}
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct View {
    pub vault_path: String,
    pub profile_exists: bool,
    pub unlocked: bool,
    pub profile_name: String,
    pub contacts: Vec<ContactView>,
    pub messages: Vec<MessageView>,
    pub devices: Vec<DeviceView>,
    pub availability: String,
    pub status: String,
    pub listening: bool,
    pub discovering: bool,
    pub pending_incoming: Option<DeviceView>,
    pub peer: Option<PeerView>,
    pub key_change: Option<DeviceView>,
    pub inactivity_minutes: u32,
    pub start_with_windows: bool,
    pub minimize_to_tray: bool,
    pub automatic_updates: bool,
    pub transfers: Vec<FileTransferView>,
}

// State is delivered through a coalescing mailbox, so it does not inflate the
// bounded error queue. Keep the public UI snapshot API allocation-free.
#[allow(clippy::large_enum_variant)]
pub enum Event {
    State(View),
    Error(String),
    Busy(bool),
}
struct Envelope {
    generation: u64,
    command: Command,
}
struct Outbound {
    generation: u64,
    event: Event,
}
#[derive(Default)]
struct Mailbox {
    state: Option<(u64, View)>,
    busy: Option<(u64, bool)>,
}

#[derive(Default)]
struct CancellationHub {
    generation: AtomicU64,
    sensitive: AtomicBool,
    last_activity: Mutex<Option<Instant>>,
    #[cfg(any(windows, test))]
    os_locked: AtomicBool,
    #[cfg(windows)]
    watcher_ready: AtomicBool,
    flags: Mutex<Vec<Arc<AtomicBool>>>,
    sockets: Mutex<Vec<(u64, transport::CancelHandle)>>,
    socket_id: AtomicU64,
}
impl CancellationHub {
    fn touch(&self) {
        // UI activity never waits for the actor or an OS/file operation.
        if let Ok(mut activity) = self.last_activity.try_lock() {
            *activity = Some(Instant::now());
        }
    }
    fn idle_expired(&self, timeout: Duration) -> bool {
        self.last_activity.lock().map_or(true, |activity| {
            activity.is_none_or(|last| last.elapsed() >= timeout)
        })
    }
    fn invalidate(&self) -> u64 {
        self.sensitive.store(false, Ordering::Release);
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.cancel_all();
        generation
    }
    #[cfg(any(windows, test))]
    fn record_windows_lock(&self) {
        self.os_locked.store(true, Ordering::Release);
        self.invalidate();
    }
    #[cfg(any(windows, test))]
    fn observe_windows_state(&self, locked: bool) {
        self.os_locked.store(locked, Ordering::Release);
        if locked && self.sensitive.swap(false, Ordering::AcqRel) {
            self.invalidate();
        }
    }
    fn flag(&self, generation: u64) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut flags) = self.flags.lock() {
            // Keep active operations only; completed workers release their reference.
            flags.retain(|f| Arc::strong_count(f) > 1);
            flags.push(flag.clone());
            if generation != self.generation.load(Ordering::Acquire) {
                flag.store(true, Ordering::Release);
            }
        } else {
            flag.store(true, Ordering::Release);
        }
        flag
    }
    fn socket(self: &Arc<Self>, generation: u64, handle: transport::CancelHandle) -> SocketLease {
        let id = self.socket_id.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut sockets) = self.sockets.lock() {
            if generation == self.generation.load(Ordering::Acquire) {
                sockets.push((id, handle));
            } else {
                handle.cancel();
            }
        } else {
            handle.cancel();
        }
        SocketLease {
            id,
            hub: self.clone(),
        }
    }
    fn cancel_all(&self) {
        if let Ok(mut flags) = self.flags.lock() {
            for flag in flags.drain(..) {
                flag.store(true, Ordering::Release);
            }
        }
        if let Ok(mut sockets) = self.sockets.lock() {
            for (_, socket) in sockets.drain(..) {
                socket.cancel();
            }
        }
    }
}

struct SocketLease {
    id: u64,
    hub: Arc<CancellationHub>,
}
impl Drop for SocketLease {
    fn drop(&mut self) {
        if let Ok(mut sockets) = self.hub.sockets.lock() {
            sockets.retain(|(id, _)| *id != self.id);
        }
    }
}

pub struct Runtime {
    commands: Sender<Envelope>,
    events: Receiver<Outbound>,
    mailbox: Arc<Mutex<Mailbox>>,
    hub: Arc<CancellationHub>,
}
impl Runtime {
    pub fn new(path: PathBuf) -> Self {
        let (commands, rx) = bounded(COMMAND_CAPACITY);
        let (tx, events) = bounded(COMMAND_CAPACITY);
        let hub = Arc::new(CancellationHub::default());
        hub.generation.store(1, Ordering::Release);
        #[cfg(windows)]
        {
            hub.os_locked.store(true, Ordering::Release);
            windows_lock::start(Arc::downgrade(&hub));
        }
        let worker_hub = hub.clone();
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let worker_mailbox = mailbox.clone();
        thread::spawn(move || Actor::new(path, tx, worker_hub, worker_mailbox).run(rx));
        Self {
            commands,
            events,
            hub,
            mailbox,
        }
    }
    pub fn vault_path() -> anyhow::Result<PathBuf> {
        let project = directories::ProjectDirs::from("org", "Hushlink", "Hushlink")
            .ok_or_else(|| anyhow::anyhow!("Der lokale App-Datenordner ist nicht verfügbar."))?;
        Ok(project.data_local_dir().join("profile.vault"))
    }
    pub fn record_activity(&self) {
        self.hub.touch();
    }
    pub fn generation(&self) -> u64 {
        self.hub.generation.load(Ordering::Acquire)
    }
    pub fn send(&self, command: Command) -> Result<(), String> {
        if matches!(command, Command::Lock) {
            self.lock();
            return Ok(());
        }
        let generation = self.hub.generation.load(Ordering::Acquire);
        if matches!(command, Command::Create { .. } | Command::Unlock { .. }) {
            #[cfg(windows)]
            if !self.hub.watcher_ready.load(Ordering::Acquire)
                || self.hub.os_locked.load(Ordering::Acquire)
            {
                return Err("Die Windows-Sperrüberwachung ist noch nicht bereit oder die Sitzung ist gesperrt. Bitte kurz warten und erneut versuchen.".into());
            }
            // Covers queued unlocks as well as active KDF work. A lock event
            // invalidates the generation even before the actor receives this job.
            self.hub.sensitive.store(true, Ordering::Release);
        }
        self.commands
            .try_send(Envelope {
                generation,
                command,
            })
            .map_err(|e| match e {
                TrySendError::Full(_) => "Hushlink arbeitet noch. Bitte kurz warten.".to_owned(),
                TrySendError::Disconnected(_) => {
                    "Der Hintergrundprozess wurde beendet. Bitte Hushlink neu starten.".to_owned()
                }
            })
    }
    /// Immediately invalidates all old jobs/events and cancels open Bluetooth sockets.
    pub fn lock(&self) {
        let generation = self.hub.invalidate();
        while self.events.try_recv().is_ok() {}
        if let Ok(mut mailbox) = self.mailbox.lock() {
            *mailbox = Mailbox::default();
        }
        // A full queue consists of invalid old commands; the actor also observes generation.
        let _ = self.commands.try_send(Envelope {
            generation,
            command: Command::Lock,
        });
    }
    pub fn try_event(&self) -> Option<Event> {
        while let Ok(out) = self.events.try_recv() {
            if out.generation == self.hub.generation.load(Ordering::Acquire) {
                return Some(out.event);
            }
        }
        if let Ok(mut mailbox) = self.mailbox.lock() {
            if let Some((generation, busy)) = mailbox.busy.take()
                && generation == self.hub.generation.load(Ordering::Acquire)
            {
                return Some(Event::Busy(busy));
            }
            if let Some((generation, view)) = mailbox.state.take()
                && generation == self.hub.generation.load(Ordering::Acquire)
            {
                return Some(Event::State(view));
            }
        }
        None
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.hub.invalidate();
        let _ = self.commands.try_send(Envelope {
            generation: self.hub.generation.load(Ordering::Acquire),
            command: Command::Shutdown,
        });
    }
}

struct Background {
    generation: u64,
    event: BackgroundEvent,
}
impl Drop for Background {
    fn drop(&mut self) {
        match &mut self.event {
            BackgroundEvent::Devices(Ok(devices)) => {
                for device in devices {
                    device.name.zeroize();
                    device.address.zeroize();
                }
            }
            BackgroundEvent::Incoming(_, device, _)
            | BackgroundEvent::Connected(_, device, _, _) => {
                device.name.zeroize();
                device.address.zeroize();
            }
            BackgroundEvent::Availability(Ok(availability)) => availability.radio_name.zeroize(),
            BackgroundEvent::Peer(_, PeerEvent::Hello(name)) => name.zeroize(),
            BackgroundEvent::Peer(_, PeerEvent::FileOffer { name, .. }) => name.zeroize(),
            BackgroundEvent::Peer(_, PeerEvent::Identity { fingerprint, .. }) => {
                fingerprint.zeroize()
            }
            _ => {}
        }
    }
}
enum BackgroundEvent {
    Empty,
    Availability(Result<transport::Availability, String>),
    Devices(Result<Vec<transport::Device>, String>),
    Listening(Arc<AtomicBool>),
    Incoming(transport::Stream, transport::Device, Arc<AtomicBool>),
    Connected(
        transport::Stream,
        transport::Device,
        Arc<AtomicBool>,
        SocketLease,
    ),
    ConnectError(String, Arc<AtomicBool>),
    ListenerError(String, Arc<AtomicBool>),
    Peer(u64, PeerEvent),
    FileLoaded(u64, Result<PreparedFile, String>),
    Exported(Result<(), String>),
}
#[derive(Clone)]
struct PreparedFile {
    name: Zeroizing<String>,
    data: Arc<Zeroizing<Vec<u8>>>,
    sha256: [u8; 32],
}
struct IncomingFile {
    id: protocol::MessageId,
    name: Zeroizing<String>,
    size: u64,
    sha256: [u8; 32],
    accepted: bool,
    data: Zeroizing<Vec<u8>>,
    activity: Instant,
}
struct OutgoingFile {
    id: protocol::MessageId,
    file: PreparedFile,
    offset: usize,
    accepted: bool,
    completed: bool,
    activity: Instant,
}
enum PeerCommand {
    Confirm,
    Send(Zeroizing<String>),
    Ack(protocol::MessageId),
    OfferFile(PreparedFile),
    AcceptFile(protocol::MessageId),
    CancelFile(protocol::MessageId),
    FileStored(protocol::MessageId),
    Close,
}
enum PeerEvent {
    Identity {
        key: [u8; 32],
        fingerprint: String,
        known: bool,
    },
    Hello(String),
    LocalConfirmed,
    Ready,
    Outgoing {
        id: protocol::MessageId,
        text: Zeroizing<String>,
        persisted: Sender<bool>,
    },
    Sent(protocol::MessageId),
    Chat {
        id: protocol::MessageId,
        text: Zeroizing<String>,
    },
    Ack(protocol::MessageId),
    FileOffered {
        id: protocol::MessageId,
        file: PreparedFile,
        persisted: Sender<bool>,
    },
    FileOffer {
        id: protocol::MessageId,
        name: String,
        size: u64,
        sha256: [u8; 32],
    },
    FileAccepted(protocol::MessageId),
    FileChunk {
        id: protocol::MessageId,
        offset: u64,
        data: Zeroizing<Vec<u8>>,
    },
    FileProgress {
        id: protocol::MessageId,
        transferred: u64,
    },
    FileComplete(protocol::MessageId),
    FileSent(protocol::MessageId),
    FileStored(protocol::MessageId),
    FileCancelled(protocol::MessageId),
    Error(String),
    KeyChanged,
    Closed,
}
struct PeerWorker {
    id: u64,
    device: transport::Device,
    commands: Sender<PeerCommand>,
    cancel: Option<transport::CancelHandle>,
    flag: Arc<AtomicBool>,
    key: Option<[u8; 32]>,
    _lease: Option<SocketLease>,
}
impl Drop for PeerWorker {
    fn drop(&mut self) {
        self.flag.store(true, Ordering::Release);
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        self.device.name.zeroize();
        self.device.address.zeroize();
    }
}

struct Actor {
    path: PathBuf,
    default_path: PathBuf,
    path_error: Option<String>,
    incoming_file: Option<IncomingFile>,
    file_loading: Option<u64>,
    desktop_preferences: storage_location::DesktopPreferences,
    events: Sender<Outbound>,
    mailbox: Arc<Mutex<Mailbox>>,
    background: Receiver<Background>,
    background_tx: Sender<Background>,
    hub: Arc<CancellationHub>,
    generation: u64,
    view: View,
    vault: Option<vault::Vault>,
    saved_profile: Option<vault::Profile>,
    peer: Option<PeerWorker>,
    pending: Option<(transport::Stream, transport::Device, SocketLease)>,
    pending_since: Option<Instant>,
    connect_flag: Option<Arc<AtomicBool>>,
    listener_flag: Option<Arc<AtomicBool>>,
    discovery_flag: Option<Arc<AtomicBool>>,
    next_peer_id: u64,
    last_snapshot: Instant,
}
impl Actor {
    fn new(
        path: PathBuf,
        events: Sender<Outbound>,
        hub: Arc<CancellationHub>,
        mailbox: Arc<Mutex<Mailbox>>,
    ) -> Self {
        let (background_tx, background) = bounded(COMMAND_CAPACITY);
        let default_path = path;
        let (path, path_error) = match storage_location::load(&default_path) {
            Ok(path) => (path, None),
            Err(error) => (default_path.clone(), Some(error.to_string())),
        };
        let mut view = View::default();
        let desktop_preferences = storage_location::load_preferences(&default_path).unwrap_or(
            storage_location::DesktopPreferences {
                start_with_windows: false,
                minimize_to_tray: false,
                automatic_updates: false,
            },
        );
        view.vault_path = path.display().to_string();
        view.profile_exists = path.exists() || path_error.is_some();
        view.status = path_error
            .clone()
            .unwrap_or_else(|| "Tresor gesperrt".into());
        view.inactivity_minutes = 5;
        view.start_with_windows = desktop_preferences.start_with_windows;
        view.minimize_to_tray = desktop_preferences.minimize_to_tray;
        view.automatic_updates = desktop_preferences.automatic_updates;
        Self {
            path,
            default_path,
            path_error,
            incoming_file: None,
            file_loading: None,
            desktop_preferences,
            events,
            mailbox,
            background,
            background_tx,
            hub,
            generation: 1,
            view,
            vault: None,
            saved_profile: None,
            peer: None,
            pending: None,
            pending_since: None,
            connect_flag: None,
            listener_flag: None,
            discovery_flag: None,
            next_peer_id: 0,
            last_snapshot: Instant::now(),
        }
    }
    fn current(&self) -> bool {
        self.generation == self.hub.generation.load(Ordering::Acquire)
    }
    fn emit(&self, event: Event) {
        if !self.current() {
            return;
        }
        match event {
            Event::State(view) => {
                if let Ok(mut mailbox) = self.mailbox.lock() {
                    mailbox.state = Some((self.generation, view));
                }
            }
            Event::Busy(busy) => {
                if let Ok(mut mailbox) = self.mailbox.lock() {
                    mailbox.busy = Some((self.generation, busy));
                }
            }
            event @ Event::Error(_) => {
                let _ = self.events.try_send(Outbound {
                    generation: self.generation,
                    event,
                });
            }
        }
    }
    fn snapshot(&mut self) {
        self.last_snapshot = Instant::now();
        self.emit(Event::State(self.view.clone()));
    }
    fn error(&self, error: impl ToString) {
        self.emit(Event::Error(error.to_string()));
    }
    fn sync_profile(&mut self) {
        let Some(vault) = &self.vault else {
            return;
        };
        let profile = vault.profile();
        self.view.profile_exists = true;
        self.view.unlocked = true;
        self.view.profile_name.clone_from(&profile.display_name);
        self.view.inactivity_minutes = profile.settings.inactivity_minutes;
        self.view.start_with_windows = self.desktop_preferences.start_with_windows;
        self.view.minimize_to_tray = self.desktop_preferences.minimize_to_tray;
        self.view.automatic_updates = self.desktop_preferences.automatic_updates;
        self.view.contacts = profile
            .contacts
            .iter()
            .map(|c| ContactView {
                id: c.id.clone(),
                name: c.display_name.clone(),
                address: c.bluetooth_address.clone(),
            })
            .collect();
        self.view.messages = profile
            .messages
            .iter()
            .map(|m| MessageView {
                id: m.id.clone(),
                contact_id: m.contact_id.clone(),
                text: m.text.clone(),
                attachment_id: m.attachment_id.clone(),
                attachment_name: m
                    .attachment_id
                    .as_ref()
                    .and_then(|id| profile.attachments.iter().find(|a| a.id == *id))
                    .map(|a| a.name.clone()),
                attachment_size: m
                    .attachment_id
                    .as_ref()
                    .and_then(|id| profile.attachments.iter().find(|a| a.id == *id))
                    .map_or(0, |a| a.data.len() as u64),
                timestamp_ms: m.timestamp_ms,
                outgoing: m.outgoing,
                status: m.status,
            })
            .collect();
    }
    fn save(&mut self) -> bool {
        if let Some(vault) = &mut self.vault {
            let settings = &mut vault.profile_mut().settings;
            settings.start_with_windows = self.desktop_preferences.start_with_windows;
            settings.minimize_to_tray = self.desktop_preferences.minimize_to_tray;
            settings.automatic_updates = self.desktop_preferences.automatic_updates;
        }
        let result = self.vault.as_mut().map(|v| v.save());
        match result {
            None => false,
            Some(Err(error)) => {
                if let (Some(vault), Some(saved)) = (&mut self.vault, &self.saved_profile) {
                    *vault.profile_mut() = saved.clone();
                }
                self.sync_profile();
                self.error(error);
                false
            }
            _ => {
                self.saved_profile = self.vault.as_ref().map(|v| v.profile().clone());
                self.sync_profile();
                true
            }
        }
    }
    fn clear(&mut self) {
        self.hub.sensitive.store(false, Ordering::Release);
        self.peer = None;
        self.incoming_file = None;
        self.file_loading = None;
        if let Some((stream, mut device, _lease)) = self.pending.take() {
            stream.cancel_handle().cancel();
            device.name.zeroize();
            device.address.zeroize();
        }
        self.pending_since = None;
        self.connect_flag = None;
        self.vault = None;
        self.saved_profile = None;
        if let Some(flag) = self.listener_flag.take() {
            flag.store(true, Ordering::Release);
        }
        if let Some(flag) = self.discovery_flag.take() {
            flag.store(true, Ordering::Release);
        }
        self.view = View::default();
        self.view.vault_path = self.path.display().to_string();
        self.view.profile_exists = self.path.exists() || self.path_error.is_some();
        self.view.status = self
            .path_error
            .clone()
            .unwrap_or_else(|| "Tresor gesperrt".into());
        self.view.inactivity_minutes = 5;
        self.view.start_with_windows = self.desktop_preferences.start_with_windows;
        self.view.minimize_to_tray = self.desktop_preferences.minimize_to_tray;
        self.view.automatic_updates = self.desktop_preferences.automatic_updates;
        self.snapshot();
        self.emit(Event::Busy(false));
    }
    fn enforce_idle_lock(&mut self) {
        let expired = self.vault.as_ref().is_some_and(|vault| {
            self.hub.idle_expired(Duration::from_secs(
                u64::from(vault.profile().settings.inactivity_minutes) * 60,
            ))
        });
        if expired {
            self.generation = self.hub.invalidate();
            self.clear();
        }
    }
    fn run(mut self, commands: Receiver<Envelope>) {
        self.snapshot();
        loop {
            let generation = self.hub.generation.load(Ordering::Acquire);
            if generation != self.generation {
                self.generation = generation;
                self.clear();
            }
            self.enforce_idle_lock();
            if self.incoming_file.as_ref().is_some_and(|file| {
                file.activity.elapsed() > Duration::from_secs(if file.accepted { 90 } else { 180 })
            }) {
                self.disconnect(
                    "Dateiübertragung hat das Zeitlimit erreicht. Bitte neu verbinden.",
                );
            }
            if self
                .pending_since
                .is_some_and(|since| since.elapsed() > Duration::from_secs(30))
            {
                if let Some((stream, mut device, _lease)) = self.pending.take() {
                    stream.cancel_handle().cancel();
                    device.name.zeroize();
                    device.address.zeroize();
                }
                self.pending_since = None;
                self.view.pending_incoming = None;
                self.view.status = "Eingehende Verbindung ist abgelaufen".into();
                self.snapshot();
            }
            select! {
                recv(commands) -> item => {
                    let Ok(mut envelope) = item else { self.hub.cancel_all(); break; };
                    if matches!(envelope.command, Command::Shutdown) { self.hub.cancel_all(); break; }
                    if envelope.generation != self.generation { continue; }
                    if let Err(error) = self.command(&mut envelope.command) {
                        if self.vault.is_none() { self.hub.sensitive.store(false, Ordering::Release); }
                        self.error(error);
                    }
                    self.snapshot();
                }
                recv(self.background) -> item => {
                    if let Ok(mut bg) = item
                        && bg.generation == self.generation && self.current() {
                        let event = std::mem::replace(&mut bg.event, BackgroundEvent::Empty);
                        let progress_only = matches!(&event, BackgroundEvent::Peer(_, PeerEvent::FileChunk { .. } | PeerEvent::FileProgress { .. }));
                        self.background(event);
                        // A progress packet must not clone the entire text history
                        // 2,560 times for one file. State changes still repaint now.
                        if !progress_only || self.peer.is_none() || self.last_snapshot.elapsed() >= Duration::from_millis(100) {
                            self.snapshot();
                        }
                    }
                }
                default(Duration::from_millis(100)) => {}
            }
        }
    }
    fn command(&mut self, command: &mut Command) -> anyhow::Result<()> {
        match command {
            Command::SetDesktopSettings {
                start_with_windows,
                minimize_to_tray,
                automatic_updates,
            } => {
                let preferences = storage_location::DesktopPreferences {
                    start_with_windows: *start_with_windows,
                    minimize_to_tray: *minimize_to_tray,
                    automatic_updates: *automatic_updates,
                };
                let previous_startup = desktop::startup_enabled()?;
                let startup_changed = previous_startup != *start_with_windows;
                if startup_changed {
                    desktop::set_startup(*start_with_windows)?;
                }
                if let Err(error) =
                    storage_location::save_preferences(&self.default_path, preferences)
                {
                    if startup_changed
                        && let Err(rollback_error) = desktop::set_startup(previous_startup)
                    {
                        anyhow::bail!(
                            "Einstellungen nicht gespeichert: {error}. Auch der bisherige Windows-Autostart konnte nicht wiederhergestellt werden: {rollback_error}"
                        );
                    }
                    return Err(error.into());
                }
                self.desktop_preferences = preferences;
                let mut mirrored = true;
                if let Some(vault) = &mut self.vault {
                    let settings = &mut vault.profile_mut().settings;
                    settings.start_with_windows = *start_with_windows;
                    settings.minimize_to_tray = *minimize_to_tray;
                    settings.automatic_updates = *automatic_updates;
                    mirrored = self.save();
                }
                self.view.start_with_windows = *start_with_windows;
                self.view.minimize_to_tray = *minimize_to_tray;
                self.view.automatic_updates = *automatic_updates;
                anyhow::ensure!(
                    mirrored,
                    "Desktop-Einstellungen gespeichert. Der verschlüsselte Profilspiegel konnte nicht aktualisiert werden; die gewählten Desktop-Einstellungen bleiben gültig."
                );
            }
            Command::ChooseVault { path } => {
                anyhow::ensure!(
                    self.vault.is_none(),
                    "Bitte vor dem Wechsel den Tresor sperren."
                );
                storage_location::save(&self.default_path, path)?;
                self.desktop_preferences = storage_location::load_preferences(&self.default_path)?;
                self.path = path.clone();
                self.path_error = None;
                self.clear();
            }
            Command::Create { name, password } => {
                anyhow::ensure!(
                    self.path_error.is_none(),
                    "Der gespeicherte Tresorpfad ist beschädigt. Bitte einen Tresor auswählen."
                );
                anyhow::ensure!(self.vault.is_none(), "Der Tresor ist bereits entsperrt.");
                anyhow::ensure!(
                    !self.path.exists(),
                    "Ein Profil ist bereits vorhanden. Bitte entsperren."
                );
                anyhow::ensure!(
                    !name.trim().is_empty()
                        && name.trim().len() <= protocol::MAX_NAME
                        && !name.chars().any(char::is_control),
                    "Bitte einen Anzeigenamen mit 1 bis 80 UTF-8-Bytes eingeben."
                );
                self.hub.sensitive.store(true, Ordering::Release);
                self.emit(Event::Busy(true));
                let identity = protocol::IdentityKey::generate()?;
                let mut profile = vault::Profile::new(
                    name.trim().to_owned(),
                    *identity.private(),
                    identity.public(),
                );
                profile.settings.start_with_windows = self.desktop_preferences.start_with_windows;
                profile.settings.minimize_to_tray = self.desktop_preferences.minimize_to_tray;
                profile.settings.automatic_updates = self.desktop_preferences.automatic_updates;
                let result = vault::Vault::create(&self.path, password.as_str(), profile);
                self.emit(Event::Busy(false));
                if !self.current() {
                    return Ok(());
                }
                self.vault = Some(result?);
                self.after_unlock();
            }
            Command::Unlock { password } => {
                anyhow::ensure!(
                    self.path_error.is_none(),
                    "Der gespeicherte Tresorpfad ist beschädigt. Bitte einen Tresor auswählen."
                );
                anyhow::ensure!(self.vault.is_none(), "Der Tresor ist bereits entsperrt.");
                self.hub.sensitive.store(true, Ordering::Release);
                self.emit(Event::Busy(true));
                let result = vault::Vault::open(&self.path, password.as_str());
                self.emit(Event::Busy(false));
                if !self.current() {
                    return Ok(());
                }
                self.vault = Some(result?);
                self.after_unlock();
            }
            Command::Lock => self.clear(),
            Command::Shutdown => {}
            _ => {
                anyhow::ensure!(
                    self.vault.is_some() && self.current(),
                    "Bitte zuerst den Tresor entsperren."
                );
                self.unlocked_command(command)?;
            }
        }
        Ok(())
    }
    fn after_unlock(&mut self) {
        self.hub.touch();
        self.saved_profile = self.vault.as_ref().map(|v| v.profile().clone());
        // Pending/sent messages from a former connection cannot acquire an ACK in a new
        // Noise session. Keep their history and mark their delivery as uncertain/failed.
        if let Some(vault) = &mut self.vault {
            let settings = &mut vault.profile_mut().settings;
            settings.start_with_windows = self.desktop_preferences.start_with_windows;
            settings.minimize_to_tray = self.desktop_preferences.minimize_to_tray;
            settings.automatic_updates = self.desktop_preferences.automatic_updates;
            for message in &mut vault.profile_mut().messages {
                if matches!(
                    message.status,
                    vault::MessageStatus::Pending | vault::MessageStatus::Sent
                ) {
                    message.status = vault::MessageStatus::Failed;
                }
            }
        }
        self.save();
        self.view.status = "Bereit – Bluetooth-Verbindung bewusst starten".into();
        let tx = self.background_tx.clone();
        let generation = self.generation;
        thread::spawn(move || {
            let result = transport::available().map_err(|e| e.to_string());
            let _ = tx.send(Background {
                generation,
                event: BackgroundEvent::Availability(result),
            });
        });
    }
    fn unlocked_command(&mut self, command: &mut Command) -> anyhow::Result<()> {
        match command {
            Command::Discover => {
                anyhow::ensure!(!self.view.discovering, "Die Gerätesuche läuft bereits.");
                self.view.discovering = true;
                self.view.devices.clear();
                self.view.status = "Bluetooth-Geräte werden gesucht …".into();
                let flag = self.hub.flag(self.generation);
                self.discovery_flag = Some(flag.clone());
                let tx = self.background_tx.clone();
                let generation = self.generation;
                thread::spawn(move || {
                    let result = transport::discover(&flag).map_err(|e| e.to_string());
                    let _ = tx.send(Background {
                        generation,
                        event: BackgroundEvent::Devices(result),
                    });
                });
            }
            Command::StartListening => self.start_listening()?,
            Command::StopListening => {
                if let Some(flag) = self.listener_flag.take() {
                    flag.store(true, Ordering::Release);
                }
                self.view.listening = false;
                self.view.status = "Verbindungsangebot beendet".into();
            }
            Command::Connect { address } => self.connect(address.clone())?,
            Command::Respond { accept } => {
                let pending = self.pending.take();
                self.pending_since = None;
                self.view.pending_incoming = None;
                if let Some((stream, mut device, lease)) = pending {
                    if *accept {
                        self.start_peer(stream, device, protocol::Role::Responder, lease)?;
                    } else {
                        stream.cancel_handle().cancel();
                        device.name.zeroize();
                        device.address.zeroize();
                        self.view.status = "Verbindung abgelehnt".into();
                    }
                }
            }
            Command::ConfirmPeer => {
                let peer = self
                    .peer
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Keine Verbindung zur Prüfung vorhanden."))?;
                peer.commands
                    .try_send(PeerCommand::Confirm)
                    .map_err(|_| anyhow::anyhow!("Verbindung ist beschäftigt oder geschlossen."))?;
            }
            Command::AllowKeyChange => {
                let device = self.view.key_change.take().ok_or_else(|| {
                    anyhow::anyhow!("Kein Schlüsselwechsel zur Prüfung vorhanden.")
                })?;
                self.disconnect("Erneute Kontaktprüfung wird vorbereitet");
                if let Some(vault) = &mut self.vault {
                    for contact in &mut vault.profile_mut().contacts {
                        if contact.bluetooth_address == device.address {
                            contact.bluetooth_address.zeroize();
                        }
                    }
                }
                anyhow::ensure!(
                    self.save(),
                    "Die neue Prüfung konnte nicht sicher vorbereitet werden."
                );
                self.connect(device.address.clone())?;
            }
            Command::Send { text } => {
                anyhow::ensure!(!text.trim().is_empty(), "Bitte eine Nachricht eingeben.");
                anyhow::ensure!(
                    self.view.peer.as_ref().is_some_and(|p| p.ready),
                    "Beide Personen müssen den Kontakt zuerst bestätigen."
                );
                let peer = self
                    .peer
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Keine aktive Verbindung."))?;
                peer.commands
                    .try_send(PeerCommand::Send(Zeroizing::new(std::mem::take(
                        &mut **text,
                    ))))
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "Nachricht konnte nicht eingereiht werden. Bitte erneut versuchen."
                        )
                    })?;
            }
            Command::SendFile { path } => self.load_file(path.clone())?,
            Command::RespondFile { id, accept } => self.respond_file(id, *accept)?,
            Command::CancelFile { id } => {
                let message_id = parse_message_id(id)?;
                anyhow::ensure!(
                    self.view.transfers.iter().any(|t| t.id == *id
                        && matches!(
                            t.state,
                            FileTransferState::AwaitingConsent | FileTransferState::Transferring
                        )),
                    "Keine aktive Dateiübertragung."
                );
                self.peer_command(PeerCommand::CancelFile(message_id))?;
                self.cancel_file(message_id);
            }
            Command::ExportFile {
                attachment_id,
                destination,
            } => self.export_file(attachment_id, destination.clone())?,
            Command::RelocateVault { path } => {
                anyhow::ensure!(
                    self.peer.is_none() && self.file_loading.is_none(),
                    "Bitte vor dem Verschieben die Verbindung trennen."
                );
                self.emit(Event::Busy(true));
                let result = storage_location::relocate(
                    self.vault
                        .as_mut()
                        .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?,
                    &self.default_path,
                    path,
                );
                self.emit(Event::Busy(false));
                if let Err(error) = result {
                    // A durable locator update can have committed before an OS
                    // flush/cleanup error. Never keep writing a possibly old copy.
                    self.generation = self.hub.invalidate();
                    match storage_location::load(&self.default_path) {
                        Ok(selected) => {
                            self.path = selected;
                            self.path_error = None;
                        }
                        Err(load_error) => self.path_error = Some(load_error.to_string()),
                    }
                    self.clear();
                    self.view.status = "Verschieben nicht vollständig abgeschlossen. Tresor gesperrt; Speicherort vor dem Entsperren prüfen.".into();
                    return Err(error.into());
                }
                self.path = path.clone();
                self.view.vault_path = self.path.display().to_string();
                self.view.status = "Verschlüsselter Tresor wurde verschoben".into();
            }
            Command::Disconnect => self.disconnect("Verbindung getrennt"),
            Command::DeleteHistory { contact_id } => {
                anyhow::ensure!(
                    !self
                        .view
                        .peer
                        .as_ref()
                        .is_some_and(|p| p.id == *contact_id && p.ready),
                    "Bitte vor dem Löschen die aktive Verbindung trennen."
                );
                if let Some(vault) = &mut self.vault {
                    vault
                        .profile_mut()
                        .messages
                        .retain(|m| m.contact_id != *contact_id);
                    vault
                        .profile_mut()
                        .attachments
                        .retain(|a| a.contact_id != *contact_id);
                }
                anyhow::ensure!(self.save(), "Der Verlauf konnte nicht gespeichert werden.");
                self.view.status = "Lokaler Verlauf gelöscht".into();
            }
            Command::ChangePassword { old, new } => {
                self.emit(Event::Busy(true));
                let result = self
                    .vault
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?
                    .change_password(old.as_str(), new.as_str());
                self.emit(Event::Busy(false));
                result?;
                self.view.status = "Passwort geändert; der Verlauf bleibt erhalten".into();
            }
            Command::SetInactivity { minutes } => {
                anyhow::ensure!(
                    (1..=120).contains(minutes),
                    "Die Inaktivitätssperre muss zwischen 1 und 120 Minuten liegen."
                );
                if let Some(vault) = &mut self.vault {
                    vault.profile_mut().settings.inactivity_minutes = *minutes;
                }
                anyhow::ensure!(
                    self.save(),
                    "Die Einstellung konnte nicht gespeichert werden."
                );
            }
            _ => {}
        }
        Ok(())
    }
    fn peer_command(&self, command: PeerCommand) -> anyhow::Result<()> {
        self.peer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Keine aktive Verbindung."))?
            .commands
            .try_send(command)
            .map_err(|_| anyhow::anyhow!("Verbindung ist beschäftigt oder geschlossen."))
    }
    fn load_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.view.peer.as_ref().is_some_and(|p| p.ready),
            "Beide Personen müssen den Kontakt zuerst bestätigen."
        );
        anyhow::ensure!(
            self.file_loading.is_none()
                && !self.view.transfers.iter().any(|t| t.outgoing
                    && matches!(
                        t.state,
                        FileTransferState::AwaitingConsent | FileTransferState::Transferring
                    )),
            "Es wird bereits eine Datei gesendet."
        );
        let peer_id = self
            .peer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Keine aktive Verbindung."))?
            .id;
        self.file_loading = Some(peer_id);
        let flag = self.hub.flag(self.generation);
        let tx = self.background_tx.clone();
        let generation = self.generation;
        thread::spawn(move || {
            let result = prepare_file(&path, &flag).map_err(|e| e.to_string());
            let _ = tx.send(Background {
                generation,
                event: BackgroundEvent::FileLoaded(peer_id, result),
            });
        });
        Ok(())
    }
    fn respond_file(&mut self, id: &str, accept: bool) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.view.peer.as_ref().is_some_and(|p| p.ready),
            "Kontakt noch nicht bestätigt."
        );
        let message_id = parse_message_id(id)?;
        let incoming = self
            .incoming_file
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Kein Dateiangebot vorhanden."))?;
        anyhow::ensure!(
            incoming.id == message_id && !incoming.accepted,
            "Dieses Dateiangebot ist nicht mehr offen."
        );
        if accept {
            anyhow::ensure!(
                self.file_capacity(incoming.size),
                "Der Tresor hat seine Dateigrenze erreicht. Bitte zuerst alte Verläufe löschen."
            );
            let text = incoming.name.to_string();
            let contact_id = self
                .view
                .peer
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Keine aktive Verbindung."))?
                .id
                .clone();
            if let Some(vault) = &mut self.vault {
                vault.profile_mut().messages.push(vault::StoredMessage {
                    id: id.to_owned(),
                    contact_id,
                    text,
                    timestamp_ms: timestamp_ms(),
                    outgoing: false,
                    status: vault::MessageStatus::Pending,
                    attachment_id: None,
                });
            }
            if !self.save() {
                self.disconnect("Dateiannahme konnte nicht sicher gespeichert werden");
                anyhow::bail!("Dateiannahme konnte nicht gespeichert werden.");
            }
            self.peer_command(PeerCommand::AcceptFile(message_id))?;
            if let Some(incoming) = &mut self.incoming_file {
                incoming.accepted = true;
                incoming.activity = Instant::now();
            }
            self.transfer_state(message_id, FileTransferState::Transferring);
        } else {
            self.peer_command(PeerCommand::CancelFile(message_id))?;
            self.cancel_file(message_id);
        }
        Ok(())
    }
    fn export_file(&mut self, attachment_id: &str, destination: PathBuf) -> anyhow::Result<()> {
        let attachment = self
            .vault
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?
            .profile()
            .attachments
            .iter()
            .find(|a| a.id == attachment_id)
            .ok_or_else(|| anyhow::anyhow!("Die Datei ist nicht im Tresor vorhanden."))?;
        let data = Zeroizing::new(attachment.data.clone());
        let digest = attachment.sha256;
        let flag = self.hub.flag(self.generation);
        let tx = self.background_tx.clone();
        let generation = self.generation;
        thread::spawn(move || {
            let result =
                export_attachment(&destination, &data, digest, &flag).map_err(|e| e.to_string());
            let _ = tx.send(Background {
                generation,
                event: BackgroundEvent::Exported(result),
            });
        });
        self.view.status = "Datei wird an den gewählten Ort entschlüsselt …".into();
        Ok(())
    }
    fn transfer_state(&mut self, id: protocol::MessageId, state: FileTransferState) {
        let id = hex::encode(id);
        if let Some(transfer) = self.view.transfers.iter_mut().find(|t| t.id == id) {
            transfer.state = state;
        }
    }
    fn cancel_file(&mut self, id: protocol::MessageId) {
        if self
            .view
            .transfers
            .iter()
            .any(|file| file.id == hex::encode(id) && file.state == FileTransferState::Stored)
        {
            return;
        }
        if self.incoming_file.as_ref().is_some_and(|f| f.id == id) {
            self.incoming_file = None;
        }
        self.transfer_state(id, FileTransferState::Cancelled);
        self.fail_file_message(id);
    }
    fn fail_file_message(&mut self, id: protocol::MessageId) {
        let id = hex::encode(id);
        let contact_id = self.view.peer.as_ref().map(|peer| peer.id.clone());
        if let Some(vault) = &mut self.vault {
            for message in &mut vault.profile_mut().messages {
                if message.id == id
                    && contact_id.as_ref() == Some(&message.contact_id)
                    && matches!(
                        message.status,
                        vault::MessageStatus::Pending | vault::MessageStatus::Sent
                    )
                {
                    message.status = vault::MessageStatus::Failed;
                }
            }
        }
        self.save();
    }
    fn start_listening(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.listener_flag.is_none(),
            "Hushlink bietet bereits Verbindungen an."
        );
        let flag = self.hub.flag(self.generation);
        self.listener_flag = Some(flag.clone());
        let tx = self.background_tx.clone();
        let generation = self.generation;
        self.view.status = "Bluetooth-Dienst wird angeboten …".into();
        thread::spawn(move || {
            let result = (|| -> anyhow::Result<()> {
                let listener = transport::Listener::bind()?;
                if flag.load(Ordering::Acquire) {
                    return Ok(());
                }
                tx.send(Background {
                    generation,
                    event: BackgroundEvent::Listening(flag.clone()),
                })?;
                while !flag.load(Ordering::Acquire) {
                    let (stream, device) = listener.accept(&flag)?;
                    if flag.load(Ordering::Acquire) {
                        break;
                    }
                    tx.send(Background {
                        generation,
                        event: BackgroundEvent::Incoming(stream, device, flag.clone()),
                    })?;
                }
                Ok(())
            })();
            if let Err(error) = result
                && !flag.load(Ordering::Acquire)
            {
                let _ = tx.send(Background {
                    generation,
                    event: BackgroundEvent::ListenerError(error.to_string(), flag.clone()),
                });
            }
        });
        Ok(())
    }
    fn connect(&mut self, address: String) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.peer.is_none() && self.pending.is_none() && self.connect_flag.is_none(),
            "Bitte zuerst die bestehende Verbindung trennen oder ablehnen."
        );
        self.view.status = "Verbindung wird hergestellt …".into();
        self.view.key_change = None;
        let name = self
            .view
            .devices
            .iter()
            .find(|d| d.address == address)
            .map(|d| d.name.clone())
            .or_else(|| {
                self.view
                    .contacts
                    .iter()
                    .find(|d| d.address == address)
                    .map(|d| d.name.clone())
            })
            .unwrap_or_else(|| address.clone());
        let device = transport::Device { name, address };
        let flag = self.hub.flag(self.generation);
        self.connect_flag = Some(flag.clone());
        let tx = self.background_tx.clone();
        let generation = self.generation;
        let hub = self.hub.clone();
        thread::spawn(move || {
            let result = transport::connect(&device.address, &flag);
            let event = match result {
                Ok(stream) => {
                    let lease = hub.socket(generation, stream.cancel_handle());
                    BackgroundEvent::Connected(stream, device, flag.clone(), lease)
                }
                Err(error) => BackgroundEvent::ConnectError(error.to_string(), flag.clone()),
            };
            let _ = tx.send(Background { generation, event });
        });
        Ok(())
    }
    fn start_peer(
        &mut self,
        stream: transport::Stream,
        device: transport::Device,
        role: protocol::Role,
        lease: SocketLease,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.peer.is_none(),
            "Eine andere Verbindung ist bereits aktiv."
        );
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tresor gesperrt."))?;
        let private = Zeroizing::new(vault.profile().identity_private);
        let display_name = Zeroizing::new(vault.profile().display_name.clone());
        let expected = vault
            .profile()
            .contacts
            .iter()
            .find(|c| c.bluetooth_address == device.address)
            .map(|c| c.identity_key);
        let flag = self.hub.flag(self.generation);
        let cancel = stream.cancel_handle();
        self.next_peer_id += 1;
        let id = self.next_peer_id;
        let (commands, receiver) = bounded(COMMAND_CAPACITY);
        self.peer = Some(PeerWorker {
            id,
            device: device.clone(),
            commands,
            cancel: Some(cancel),
            flag: flag.clone(),
            key: None,
            _lease: Some(lease),
        });
        let mut peer_view = PeerView::default();
        peer_view.name.clone_from(&device.name);
        peer_view.address.clone_from(&device.address);
        self.view.peer = Some(peer_view);
        self.view.status = "Geschützte Verbindung wird aufgebaut …".into();
        let tx = self.background_tx.clone();
        let generation = self.generation;
        thread::spawn(move || {
            peer_loop(
                stream,
                private,
                display_name,
                role,
                expected,
                flag,
                receiver,
                tx,
                generation,
                id,
            )
        });
        Ok(())
    }
    fn disconnect(&mut self, status: &str) {
        self.incoming_file = None;
        self.file_loading = None;
        for transfer in &mut self.view.transfers {
            if matches!(
                transfer.state,
                FileTransferState::AwaitingConsent | FileTransferState::Transferring
            ) {
                transfer.state = FileTransferState::Failed;
            }
        }
        if let Some(peer) = self.peer.take() {
            let _ = peer.commands.try_send(PeerCommand::Close);
        }
        if let Some((stream, mut device, _lease)) = self.pending.take() {
            stream.cancel_handle().cancel();
            device.name.zeroize();
            device.address.zeroize();
        }
        self.pending_since = None;
        if let Some(flag) = self.connect_flag.take() {
            flag.store(true, Ordering::Release);
        }
        self.view.pending_incoming = None;
        if let Some(peer) = &self.view.peer
            && let Some(vault) = &mut self.vault
        {
            for message in &mut vault.profile_mut().messages {
                if message.contact_id == peer.id
                    && matches!(
                        message.status,
                        vault::MessageStatus::Pending | vault::MessageStatus::Sent
                    )
                {
                    message.status = vault::MessageStatus::Failed;
                }
            }
        }
        self.view.peer = None;
        if self.vault.is_some() {
            self.save();
        }
        self.view.status = status.to_owned();
    }
    fn background(&mut self, event: BackgroundEvent) {
        match event {
            BackgroundEvent::Empty => {}
            BackgroundEvent::FileLoaded(peer_id, result) => {
                if self.file_loading != Some(peer_id) {
                    return;
                }
                if self.peer.as_ref().is_none_or(|p| p.id != peer_id)
                    || !self.view.peer.as_ref().is_some_and(|p| p.ready)
                {
                    self.file_loading = None;
                    return;
                }
                match result {
                    Ok(file) => {
                        if let Err(error) = self.peer_command(PeerCommand::OfferFile(file)) {
                            self.file_loading = None;
                            self.error(error);
                        }
                    }
                    Err(error) => {
                        self.file_loading = None;
                        self.error(error);
                    }
                }
            }
            BackgroundEvent::Exported(result) => match result {
                Ok(()) => {
                    self.view.status =
                        "Datei wurde entschlüsselt gespeichert. Sie liegt dort unverschlüsselt vor."
                            .into()
                }
                Err(error) => self.error(error),
            },
            BackgroundEvent::Availability(result) => {
                self.view.availability = match result {
                    Ok(availability) => format!("Bluetooth: {}", availability.radio_name),
                    Err(error) => error,
                };
            }
            BackgroundEvent::Devices(result) => {
                self.view.discovering = false;
                self.discovery_flag = None;
                match result {
                    Ok(devices) => {
                        self.view.devices = devices
                            .into_iter()
                            .map(|d| DeviceView {
                                name: d.name,
                                address: d.address,
                            })
                            .collect();
                        self.view.status = if self.view.devices.is_empty() {
                            "Keine Bluetooth-Geräte gefunden. Auf dem anderen PC Windows-Bluetooth-Sichtbarkeit prüfen.".into()
                        } else {
                            "Geräte gefunden. Hushlink muss auf dem Zielgerät Verbindungen anbieten.".into()
                        };
                    }
                    Err(error) => {
                        self.view.status = "Gerätesuche fehlgeschlagen".into();
                        self.error(error);
                    }
                }
            }
            BackgroundEvent::Listening(flag) => {
                if flag.load(Ordering::Acquire) {
                    return;
                }
                self.view.listening = true;
                self.view.status = "Verbindungen werden angeboten. Sichtbarkeit bei Bedarf in Windows einschalten.".into();
            }
            BackgroundEvent::Incoming(stream, device, flag) => {
                if flag.load(Ordering::Acquire)
                    || self.peer.is_some()
                    || self.pending.is_some()
                    || !self.view.listening
                {
                    stream.cancel_handle().cancel();
                    return;
                }
                let lease = self.hub.socket(self.generation, stream.cancel_handle());
                self.view.pending_incoming = Some(DeviceView {
                    name: device.name.clone(),
                    address: device.address.clone(),
                });
                self.pending = Some((stream, device, lease));
                self.pending_since = Some(Instant::now());
                self.view.status =
                    "Eingehende Bluetooth-Verbindung – annehmen oder ablehnen".into();
            }
            BackgroundEvent::Connected(stream, device, flag, lease) => {
                if flag.load(Ordering::Acquire) {
                    stream.cancel_handle().cancel();
                    return;
                }
                self.connect_flag = None;
                if let Err(error) =
                    self.start_peer(stream, device, protocol::Role::Initiator, lease)
                {
                    self.error(error);
                }
            }
            BackgroundEvent::ConnectError(error, flag) => {
                if flag.load(Ordering::Acquire) {
                    return;
                }
                self.connect_flag = None;
                self.view.status = "Verbindung fehlgeschlagen".into();
                self.error(error);
            }
            BackgroundEvent::ListenerError(error, flag) => {
                if flag.load(Ordering::Acquire) {
                    return;
                }
                self.listener_flag = None;
                self.view.listening = false;
                self.view.status = "Dienst konnte nicht angeboten werden".into();
                self.error(error);
            }
            BackgroundEvent::Peer(id, event) => {
                if self.peer.as_ref().is_some_and(|p| p.id == id) {
                    self.peer_event(event);
                }
            }
        }
    }
    fn peer_event(&mut self, event: PeerEvent) {
        match event {
            PeerEvent::Identity {
                key,
                fingerprint,
                known,
            } => {
                if let Some(peer) = &mut self.peer {
                    peer.key = Some(key);
                }
                if let Some(peer) = &mut self.view.peer {
                    peer.id = hex::encode(key);
                    peer.fingerprint = fingerprint;
                    peer.known = known;
                }
                self.view.status = if known {
                    "Bekannter Kontakt – Bestätigung der Gegenstelle wird geprüft".into()
                } else {
                    "Sicherheitsfingerabdruck auf beiden Geräten unabhängig vergleichen".into()
                };
            }
            PeerEvent::Hello(name) => {
                if let Some(peer) = &mut self.view.peer {
                    peer.name = name;
                }
            }
            PeerEvent::LocalConfirmed => {
                if let Some(peer) = &mut self.view.peer {
                    peer.local_confirmed = true;
                }
            }
            PeerEvent::Ready => {
                let Some(peer) = &self.peer else {
                    return;
                };
                let Some(key) = peer.key else {
                    return;
                };
                let Some(view) = &self.view.peer else {
                    return;
                };
                if self.vault.as_ref().is_some_and(|v| {
                    v.profile().contacts.len() >= vault::MAX_CONTACTS
                        && !v.profile().contacts.iter().any(|c| c.identity_key == key)
                }) {
                    self.disconnect(
                        "Die Kontaktgrenze ist erreicht. Ein neues Profil ist erforderlich.",
                    );
                    return;
                }
                if let Some(vault) = &mut self.vault {
                    let profile = vault.profile_mut();
                    if let Some(contact) =
                        profile.contacts.iter_mut().find(|c| c.identity_key == key)
                    {
                        contact.display_name.clone_from(&view.name);
                        contact.bluetooth_address.clone_from(&peer.device.address);
                    } else {
                        profile.contacts.push(vault::Contact {
                            id: hex::encode(key),
                            display_name: view.name.clone(),
                            identity_key: key,
                            bluetooth_address: peer.device.address.clone(),
                        });
                    }
                }
                if !self.save() {
                    self.disconnect("Kontakt konnte nicht sicher gespeichert werden");
                    return;
                }
                if let Some(peer) = &mut self.view.peer {
                    peer.ready = true;
                }
                self.view.status = "Verbunden – Kontakt auf beiden Geräten bestätigt".into();
            }
            PeerEvent::Outgoing {
                id,
                text,
                persisted,
            } => {
                let Some(peer) = &self.view.peer else {
                    return;
                };
                if self
                    .vault
                    .as_ref()
                    .is_some_and(|v| v.profile().messages.len() >= vault::MAX_MESSAGES)
                {
                    let _ = persisted.try_send(false);
                    self.disconnect(
                        "Die Verlaufsgrenze ist erreicht. Bitte einen Verlauf löschen.",
                    );
                    return;
                }
                if let Some(vault) = &mut self.vault {
                    vault.profile_mut().messages.push(vault::StoredMessage {
                        id: hex::encode(id),
                        contact_id: peer.id.clone(),
                        text: text.to_string(),
                        attachment_id: None,
                        timestamp_ms: timestamp_ms(),
                        outgoing: true,
                        status: vault::MessageStatus::Pending,
                    });
                }
                let saved = self.save();
                let _ = persisted.try_send(saved && self.current());
                if !saved {
                    self.disconnect("Nachricht konnte nicht sicher gespeichert werden");
                }
            }
            PeerEvent::Sent(id) => {
                self.update_delivery(id, vault::MessageStatus::Sent);
            }
            PeerEvent::Ack(id) => {
                self.update_delivery(id, vault::MessageStatus::Received);
            }
            PeerEvent::Chat { id, text } => {
                let Some(peer) = &self.view.peer else {
                    return;
                };
                if !peer.ready {
                    self.disconnect("Unzulässige Nachricht vor Kontaktbestätigung");
                    return;
                }
                if self
                    .vault
                    .as_ref()
                    .is_some_and(|v| v.profile().messages.len() >= vault::MAX_MESSAGES)
                {
                    self.disconnect(
                        "Die Verlaufsgrenze ist erreicht. Bitte einen Verlauf löschen.",
                    );
                    return;
                }
                if self.vault.as_ref().is_some_and(|v| {
                    v.profile()
                        .messages
                        .iter()
                        .any(|m| m.id == hex::encode(id) && m.contact_id == peer.id)
                }) {
                    self.disconnect("Doppelte Nachricht verworfen");
                    return;
                }
                if let Some(vault) = &mut self.vault {
                    vault.profile_mut().messages.push(vault::StoredMessage {
                        id: hex::encode(id),
                        contact_id: peer.id.clone(),
                        text: text.to_string(),
                        attachment_id: None,
                        timestamp_ms: timestamp_ms(),
                        outgoing: false,
                        status: vault::MessageStatus::Received,
                    });
                }
                if self.save() {
                    if let Some(peer) = &self.peer
                        && peer.commands.try_send(PeerCommand::Ack(id)).is_err()
                    {
                        self.disconnect("Empfangsbestätigung konnte nicht gesendet werden");
                    }
                } else {
                    self.disconnect("Nachricht konnte nicht sicher gespeichert werden");
                }
            }
            PeerEvent::FileOffered {
                id,
                file,
                persisted,
            } => {
                self.file_loading = None;
                let saved = self.store_outgoing_file(id, &file);
                let _ = persisted.try_send(saved && self.current());
                if !saved {
                    self.disconnect("Datei konnte nicht sicher im Tresor gespeichert werden");
                }
            }
            PeerEvent::FileOffer {
                id,
                name,
                size,
                sha256,
            } => {
                if !self.view.peer.as_ref().is_some_and(|peer| peer.ready)
                    || self.incoming_file.is_some()
                    || size > protocol::MAX_FILE_SIZE
                    || !vault::valid_attachment_name(&name)
                    || self.has_stored_message(id)
                {
                    self.disconnect("Unzulässiges oder doppeltes Dateiangebot");
                    return;
                }
                self.push_transfer(id, &name, size, false);
                self.incoming_file = Some(IncomingFile {
                    id,
                    name: Zeroizing::new(name),
                    size,
                    sha256,
                    accepted: false,
                    data: Zeroizing::new(Vec::new()),
                    activity: Instant::now(),
                });
                self.view.status = "Dateiangebot – Annahme ist erforderlich".into();
            }
            PeerEvent::FileAccepted(id) => {
                if !self.file_cancelled(id) {
                    self.transfer_state(id, FileTransferState::Transferring);
                }
            }
            PeerEvent::FileChunk { id, offset, data } => {
                if self.file_cancelled(id) {
                    return;
                }
                let Some(file) = self.incoming_file.as_mut() else {
                    self.disconnect("Dateidaten ohne angenommenes Angebot");
                    return;
                };
                if file.id != id
                    || !file.accepted
                    || offset != file.data.len() as u64
                    || data.is_empty()
                    || data.len() > protocol::MAX_FILE_CHUNK
                    || offset.saturating_add(data.len() as u64) > file.size
                {
                    self.disconnect("Unzulässige Dateidaten verworfen");
                    return;
                }
                file.data.extend_from_slice(&data);
                file.activity = Instant::now();
                let transferred = file.data.len() as u64;
                self.transfer_progress(id, transferred);
            }
            PeerEvent::FileProgress { id, transferred } => self.transfer_progress(id, transferred),
            PeerEvent::FileComplete(id) => {
                if !self.file_cancelled(id) {
                    self.complete_incoming_file(id);
                }
            }
            PeerEvent::FileSent(id) => {
                if !self.file_cancelled(id) {
                    self.update_delivery(id, vault::MessageStatus::Sent);
                }
            }
            PeerEvent::FileStored(id) => {
                if self.file_cancelled(id) {
                    return;
                }
                self.transfer_state(id, FileTransferState::Stored);
                self.update_delivery(id, vault::MessageStatus::Received);
            }
            PeerEvent::FileCancelled(id) => self.cancel_file(id),
            PeerEvent::KeyChanged => {
                let device = self.peer.as_ref().map(|p| DeviceView {
                    name: p.device.name.clone(),
                    address: p.device.address.clone(),
                });
                self.disconnect(
                    "Verbindung blockiert: Der gespeicherte Identitätsschlüssel hat sich geändert",
                );
                self.view.key_change = device;
                self.error("Schlüsselwechsel: Die Gegenstelle könnte ein neues Profil verwenden oder manipuliert sein. Erst nach unabhängiger Prüfung bewusst neu verbinden.");
            }
            PeerEvent::Error(error) => {
                self.disconnect("Verbindung sicher abgebrochen");
                self.error(error);
            }
            PeerEvent::Closed => self.disconnect("Die Gegenstelle hat die Verbindung beendet"),
        }
    }
    fn has_stored_message(&self, id: protocol::MessageId) -> bool {
        let id = hex::encode(id);
        self.vault.as_ref().is_none_or(|vault| {
            vault
                .profile()
                .attachments
                .iter()
                .any(|attachment| attachment.id == id)
                || vault.profile().messages.iter().any(|message| {
                    message.id == id
                        && self
                            .view
                            .peer
                            .as_ref()
                            .is_some_and(|peer| peer.id == message.contact_id)
                })
        })
    }
    fn file_cancelled(&self, id: protocol::MessageId) -> bool {
        let id = hex::encode(id);
        self.view.transfers.iter().any(|file| {
            file.id == id
                && matches!(
                    file.state,
                    FileTransferState::Cancelled | FileTransferState::Failed
                )
        })
    }
    fn file_capacity(&self, size: u64) -> bool {
        self.vault.as_ref().is_some_and(|vault| {
            let profile = vault.profile();
            let reservation = self.incoming_file.as_ref().filter(|file| file.accepted);
            let reserved = reservation.map_or(0, |file| file.size);
            profile.messages.len() < vault::MAX_MESSAGES
                && profile.attachments.len() + usize::from(reservation.is_some())
                    < vault::MAX_ATTACHMENTS
                && size <= protocol::MAX_FILE_SIZE
                && profile
                    .attachments
                    .iter()
                    .map(|file| file.data.len() as u64)
                    .sum::<u64>()
                    .saturating_add(reserved)
                    .saturating_add(size)
                    <= vault::MAX_TOTAL_ATTACHMENT_BYTES as u64
        })
    }
    fn push_transfer(&mut self, id: protocol::MessageId, name: &str, size: u64, outgoing: bool) {
        // Completed transient progress belongs to history; keep UI snapshots bounded.
        self.view.transfers.retain(|transfer| {
            matches!(
                transfer.state,
                FileTransferState::AwaitingConsent | FileTransferState::Transferring
            )
        });
        self.view.transfers.push(FileTransferView {
            id: hex::encode(id),
            name: name.to_owned(),
            size,
            transferred: 0,
            outgoing,
            state: FileTransferState::AwaitingConsent,
        });
    }
    fn transfer_progress(&mut self, id: protocol::MessageId, transferred: u64) {
        let id = hex::encode(id);
        if let Some(transfer) = self
            .view
            .transfers
            .iter_mut()
            .find(|transfer| transfer.id == id)
        {
            transfer.transferred = transferred;
        }
    }
    fn store_outgoing_file(&mut self, id: protocol::MessageId, file: &PreparedFile) -> bool {
        if !self.current()
            || !self.view.peer.as_ref().is_some_and(|peer| peer.ready)
            || !self.file_capacity(file.data.len() as u64)
            || self.has_stored_message(id)
        {
            self.error(
                "Die Datei kann nicht im Tresor gespeichert werden. Bitte alte Verläufe löschen.",
            );
            return false;
        }
        let Some(contact_id) = self.view.peer.as_ref().map(|peer| peer.id.clone()) else {
            return false;
        };
        let message_id = hex::encode(id);
        let timestamp_ms = timestamp_ms();
        if let Some(vault) = &mut self.vault {
            vault.profile_mut().messages.push(vault::StoredMessage {
                id: message_id.clone(),
                contact_id: contact_id.clone(),
                text: file.name.to_string(),
                timestamp_ms,
                outgoing: true,
                status: vault::MessageStatus::Pending,
                attachment_id: Some(message_id.clone()),
            });
            vault.profile_mut().attachments.push(vault::Attachment {
                id: message_id,
                contact_id,
                name: file.name.to_string(),
                data: file.data.to_vec(),
                sha256: file.sha256,
                outgoing: true,
                timestamp_ms,
            });
        }
        if !self.save() {
            return false;
        }
        self.push_transfer(id, &file.name, file.data.len() as u64, true);
        true
    }
    fn complete_incoming_file(&mut self, id: protocol::MessageId) {
        let Some(mut file) = self.incoming_file.take() else {
            self.disconnect("Dateiabschluss ohne angenommenes Angebot");
            return;
        };
        if !self.current()
            || !self.view.peer.as_ref().is_some_and(|peer| peer.ready)
            || file.id != id
            || !file.accepted
            || file.data.len() as u64 != file.size
            || <[u8; 32]>::from(Sha256::digest(&file.data)) != file.sha256
        {
            self.disconnect("Datei ist unvollständig oder wurde verändert");
            return;
        }
        let message_id = hex::encode(id);
        let Some(contact_id) = self.view.peer.as_ref().map(|peer| peer.id.clone()) else {
            return;
        };
        if let Some(vault) = &mut self.vault {
            let profile = vault.profile_mut();
            let Some(message) = profile.messages.iter_mut().find(|message| {
                message.id == message_id && message.contact_id == contact_id && !message.outgoing
            }) else {
                self.disconnect("Datei wurde nicht zum Empfang vorgemerkt");
                return;
            };
            message.status = vault::MessageStatus::Received;
            message.attachment_id = Some(message_id.clone());
            profile.attachments.push(vault::Attachment {
                id: message_id,
                contact_id,
                name: file.name.to_string(),
                data: std::mem::take(&mut *file.data),
                sha256: file.sha256,
                outgoing: false,
                timestamp_ms: timestamp_ms(),
            });
        }
        if self.save() && self.current() {
            self.transfer_state(id, FileTransferState::Stored);
            if self.peer_command(PeerCommand::FileStored(id)).is_err() {
                self.disconnect(
                    "Datei gespeichert; Empfangsbestätigung konnte nicht gesendet werden",
                );
            }
        } else {
            self.disconnect("Datei konnte nicht sicher im Tresor gespeichert werden");
        }
    }
    fn update_delivery(&mut self, id: protocol::MessageId, status: vault::MessageStatus) {
        let id = hex::encode(id);
        let Some(contact_id) = self.view.peer.as_ref().map(|p| p.id.clone()) else {
            return;
        };
        if let Some(vault) = &mut self.vault
            && let Some(message) = vault
                .profile_mut()
                .messages
                .iter_mut()
                .find(|m| m.id == id && m.contact_id == contact_id && m.outgoing)
        {
            message.status = status;
        }
        if !self.save() {
            self.disconnect("Nachrichtenstatus konnte nicht gespeichert werden");
        }
    }
}

fn parse_message_id(id: &str) -> anyhow::Result<protocol::MessageId> {
    let mut bytes = [0; 16];
    hex::decode_to_slice(id, &mut bytes)
        .map_err(|_| anyhow::anyhow!("Ungültige Dateiübertragung."))?;
    Ok(bytes)
}

fn await_persistence(durable: &Receiver<bool>, cancelled: &AtomicBool) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        anyhow::ensure!(!cancelled.load(Ordering::Acquire), "Verbindung beendet.");
        anyhow::ensure!(
            Instant::now() < deadline,
            "Das sichere Speichern hat das Zeitlimit erreicht."
        );
        match durable.recv_timeout(Duration::from_millis(100)) {
            Ok(true) => {
                anyhow::ensure!(!cancelled.load(Ordering::Acquire), "Verbindung beendet.");
                return Ok(());
            }
            Ok(false) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("Inhalt konnte nicht sicher gespeichert werden.")
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn prepare_file(path: &std::path::Path, cancelled: &AtomicBool) -> anyhow::Result<PreparedFile> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("Der Dateiname kann nicht verarbeitet werden."))?;
    anyhow::ensure!(
        vault::valid_attachment_name(name) && name.len() <= protocol::MAX_FILE_NAME,
        "Bitte eine Datei mit einem einfachen Dateinamen bis 180 UTF-8-Bytes wählen."
    );
    let mut source = std::fs::File::open(path)?;
    let metadata = source.metadata()?;
    anyhow::ensure!(metadata.is_file(), "Bitte eine reguläre Datei wählen.");
    anyhow::ensure!(
        metadata.len() <= protocol::MAX_FILE_SIZE,
        "Dateien dürfen höchstens 20 MiB groß sein."
    );
    let mut data = Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
    let mut chunk = Zeroizing::new([0_u8; protocol::MAX_FILE_CHUNK]);
    loop {
        anyhow::ensure!(
            !cancelled.load(Ordering::Acquire),
            "Dateiauswahl verworfen; Tresor gesperrt."
        );
        let count = source.read(&mut *chunk)?;
        if count == 0 {
            break;
        }
        anyhow::ensure!(
            data.len().saturating_add(count) <= protocol::MAX_FILE_SIZE as usize,
            "Die Datei ist während des Lesens zu groß geworden."
        );
        data.extend_from_slice(&chunk[..count]);
    }
    anyhow::ensure!(
        !cancelled.load(Ordering::Acquire),
        "Dateiauswahl verworfen; Tresor gesperrt."
    );
    let sha256 = Sha256::digest(&data).into();
    Ok(PreparedFile {
        name: Zeroizing::new(name.to_owned()),
        data: Arc::new(data),
        sha256,
    })
}

/// Writes only the explicitly selected plaintext destination. An existing file
/// is never overwritten; incomplete output is removed on failure/cancellation.
fn export_attachment(
    path: &std::path::Path,
    data: &[u8],
    digest: [u8; 32],
    cancelled: &AtomicBool,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        <[u8; 32]>::from(Sha256::digest(data)) == digest,
        "Die gespeicherte Datei ist beschädigt."
    );
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("Ungültiger Zieldateiname."))?;
    anyhow::ensure!(
        vault::valid_attachment_name(name),
        "Ungültiger Zieldateiname."
    );
    anyhow::ensure!(
        !cancelled.load(Ordering::Acquire),
        "Export verworfen; Tresor gesperrt."
    );
    let mut destination = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let result = (|| -> anyhow::Result<()> {
        for chunk in data.chunks(protocol::MAX_FILE_CHUNK) {
            anyhow::ensure!(
                !cancelled.load(Ordering::Acquire),
                "Export verworfen; Tresor gesperrt."
            );
            destination.write_all(chunk)?;
        }
        anyhow::ensure!(
            !cancelled.load(Ordering::Acquire),
            "Export verworfen; Tresor gesperrt."
        );
        destination.sync_all()?;
        anyhow::ensure!(
            !cancelled.load(Ordering::Acquire),
            "Export verworfen; Tresor gesperrt."
        );
        Ok(())
    })();
    drop(destination);
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[allow(clippy::too_many_arguments)]
fn peer_loop(
    mut stream: transport::Stream,
    private: Zeroizing<[u8; 32]>,
    display_name: Zeroizing<String>,
    role: protocol::Role,
    expected: Option<[u8; 32]>,
    cancelled: Arc<AtomicBool>,
    commands: Receiver<PeerCommand>,
    background: Sender<Background>,
    generation: u64,
    id: u64,
) {
    let emit = |event: PeerEvent| -> anyhow::Result<()> {
        if cancelled.load(Ordering::Acquire) {
            anyhow::bail!("Verbindung beendet.");
        }
        background
            .send(Background {
                generation,
                event: BackgroundEvent::Peer(id, event),
            })
            .map_err(|_| anyhow::anyhow!("Hushlink wurde beendet."))
    };
    let result = (|| -> anyhow::Result<()> {
        let identity = protocol::IdentityKey::from_private(&private)?;
        let watchdog_socket = stream.cancel_handle();
        let (handshake_done, watchdog) = bounded::<()>(1);
        thread::spawn(move || {
            if watchdog.recv_timeout(Duration::from_secs(30)).is_err() {
                watchdog_socket.cancel();
            }
        });
        let handshake = protocol::handshake(&mut stream, &identity, role);
        let _ = handshake_done.try_send(());
        let mut session = handshake?;
        if cancelled.load(Ordering::Acquire) {
            return Ok(());
        }
        let known = match session.check_contact(expected) {
            Ok(protocol::Trust::Known) => true,
            Ok(protocol::Trust::New) => false,
            Err(_) => {
                emit(PeerEvent::KeyChanged)?;
                return Ok(());
            }
        };
        emit(PeerEvent::Identity {
            key: session.remote_key(),
            fingerprint: session.fingerprint(),
            known,
        })?;
        protocol::write_frame(&mut stream, &session.hello(&display_name)?)?;
        let mut reader = stream.try_clone()?;
        reader.set_read_timeout(Duration::from_millis(250))?;
        let (frames_tx, frames) = bounded::<anyhow::Result<Vec<u8>>>(COMMAND_CAPACITY);
        let read_cancel = cancelled.clone();
        thread::spawn(move || {
            loop {
                let frame = read_network_frame(&mut reader, &read_cancel);
                let failed = frame.is_err();
                if frames_tx.send(frame).is_err() || failed {
                    break;
                }
            }
        });
        let mut ready_announced = false;
        let mut hello_received = false;
        let mut local_confirmed = false;
        let mut confirmation_requested = false;
        let mut outgoing_file: Option<OutgoingFile> = None;
        let mut incoming_file_id = None;
        let trust_deadline = Instant::now() + Duration::from_secs(180);
        loop {
            if cancelled.load(Ordering::Acquire) {
                break;
            }
            if !session.is_ready() && Instant::now() > trust_deadline {
                anyhow::bail!("Die Kontaktprüfung ist abgelaufen. Bitte neu verbinden.");
            }
            if session.is_ready() && !ready_announced {
                emit(PeerEvent::Ready)?;
                ready_announced = true;
            }
            if outgoing_file.as_ref().is_some_and(|file| {
                file.activity.elapsed() > Duration::from_secs(if file.accepted { 90 } else { 180 })
            }) {
                anyhow::bail!("Dateiübertragung hat das Zeitlimit erreicht. Bitte neu verbinden.");
            }
            let poll = if outgoing_file
                .as_ref()
                .is_some_and(|file| file.accepted && !file.completed)
            {
                Duration::from_millis(1)
            } else {
                Duration::from_millis(100)
            };
            select! {
                recv(commands) -> command => {
                    let Ok(command) = command else { break; };
                    match command {
                        PeerCommand::Confirm => {
                            // Local clicks are idempotent; an early click waits for
                            // the authenticated hello instead of aborting the peer.
                            confirmation_requested = true;
                            if hello_received && !local_confirmed {
                                protocol::write_frame(&mut stream, &session.confirm()?)?;
                                local_confirmed = true;
                                emit(PeerEvent::LocalConfirmed)?;
                            }
                        }
                        PeerCommand::Send(text) => {
                            anyhow::ensure!(ready_announced && session.is_ready(), "Kontakt noch nicht beidseitig bestätigt.");
                            let (message_id, frame) = session.send_chat(&text)?;
                            let (persisted, durable) = bounded(1);
                            emit(PeerEvent::Outgoing { id: message_id, text, persisted })?;
                            await_persistence(&durable, &cancelled)?;
                            protocol::write_frame(&mut stream, &frame)?;
                            emit(PeerEvent::Sent(message_id))?;
                        }
                        PeerCommand::Ack(message_id) => { protocol::write_frame(&mut stream, &session.acknowledge(message_id)?)?; }
                        PeerCommand::OfferFile(file) => {
                            anyhow::ensure!(ready_announced && session.is_ready(), "Kontakt noch nicht beidseitig bestätigt.");
                            anyhow::ensure!(outgoing_file.is_none(), "Es wird bereits eine Datei gesendet.");
                            let (message_id, frame) = session.offer_file(&file.name, file.data.len() as u64, file.sha256)?;
                            let (persisted, durable) = bounded(1);
                            emit(PeerEvent::FileOffered { id: message_id, file: file.clone(), persisted })?;
                            await_persistence(&durable, &cancelled)?;
                            protocol::write_frame(&mut stream, &frame)?;
                            outgoing_file = Some(OutgoingFile { id: message_id, file, offset: 0, accepted: false, completed: false, activity: Instant::now() });
                        }
                        PeerCommand::AcceptFile(message_id) => {
                            if incoming_file_id == Some(message_id) {
                                protocol::write_frame(&mut stream, &session.accept_file(message_id)?)?;
                            }
                        }
                        PeerCommand::CancelFile(message_id) => {
                            // A peer cancellation may overtake a UI click; stale clicks
                            // must not terminate an otherwise healthy chat session.
                            if outgoing_file.as_ref().is_some_and(|file| file.id == message_id) || incoming_file_id == Some(message_id) {
                                if outgoing_file.as_ref().is_some_and(|file| file.id == message_id) { outgoing_file = None; }
                                if incoming_file_id == Some(message_id) { incoming_file_id = None; }
                                protocol::write_frame(&mut stream, &session.cancel_file(message_id)?)?;
                            }
                        }
                        PeerCommand::FileStored(message_id) => {
                            if incoming_file_id == Some(message_id) {
                                protocol::write_frame(&mut stream, &session.file_stored(message_id)?)?;
                                incoming_file_id = None;
                            }
                        }
                        PeerCommand::Close => { if let Ok(frame) = session.close() { let _ = protocol::write_frame(&mut stream, &frame); } break; }
                    }
                }
                recv(frames) -> frame => {
                    let frame = frame.map_err(|_| anyhow::anyhow!("Bluetooth-Verbindung geschlossen."))??;
                    match session.receive(&frame)? {
                        protocol::Event::Hello { name } => {
                            hello_received = true;
                            emit(PeerEvent::Hello(name))?;
                            if (known || confirmation_requested) && !local_confirmed {
                                protocol::write_frame(&mut stream, &session.confirm()?)?;
                                local_confirmed = true;
                                emit(PeerEvent::LocalConfirmed)?;
                            }
                        }
                        protocol::Event::TrustConfirmed => {}
                        protocol::Event::Chat { id, text } => { emit(PeerEvent::Chat { id, text: Zeroizing::new(text) })?; }
                        protocol::Event::Ack { id } => { emit(PeerEvent::Ack(id))?; }
                        protocol::Event::FileOffer { id, name, size, sha256 } => {
                            incoming_file_id = Some(id);
                            emit(PeerEvent::FileOffer { id, name, size, sha256 })?;
                        }
                        protocol::Event::FileAccept { id } => {
                            let file = outgoing_file.as_mut().ok_or_else(|| anyhow::anyhow!("Dateiannahme ohne Angebot."))?;
                            anyhow::ensure!(file.id == id, "Ungültige Dateiannahme.");
                            file.accepted = true;
                            file.activity = Instant::now();
                            emit(PeerEvent::FileAccepted(id))?;
                        }
                        protocol::Event::FileChunk { id, offset, data } => { emit(PeerEvent::FileChunk { id, offset, data: Zeroizing::new(data) })?; }
                        protocol::Event::FileComplete { id } => { emit(PeerEvent::FileComplete(id))?; }
                        protocol::Event::FileStored { id } => {
                            anyhow::ensure!(outgoing_file.as_ref().is_some_and(|file| file.id == id && file.completed), "Dateibestätigung ohne Abschluss.");
                            outgoing_file = None;
                            emit(PeerEvent::FileStored(id))?;
                        }
                        protocol::Event::FileCancel { id } => {
                            if outgoing_file.as_ref().is_some_and(|file| file.id == id) { outgoing_file = None; }
                            if incoming_file_id == Some(id) { incoming_file_id = None; }
                            protocol::write_frame(&mut stream, &session.acknowledge_file_cancel(id)?)?;
                            emit(PeerEvent::FileCancelled(id))?;
                        }
                        protocol::Event::FileCancelAck { .. } | protocol::Event::FileDiscarded { .. } => {}
                        protocol::Event::Closed => { emit(PeerEvent::Closed)?; break; }
                    }
                }
                default(poll) => {}
            }
            // At most one bounded chunk per iteration. Commands, inbound chat,
            // cancellation and receipts are polled between every two writes.
            if let Some(file) = outgoing_file
                .as_mut()
                .filter(|file| file.accepted && !file.completed)
            {
                anyhow::ensure!(!cancelled.load(Ordering::Acquire), "Verbindung beendet.");
                if file.offset < file.file.data.len() {
                    let end = file
                        .offset
                        .saturating_add(protocol::MAX_FILE_CHUNK)
                        .min(file.file.data.len());
                    let frame = session.send_file_chunk(
                        file.id,
                        file.offset as u64,
                        &file.file.data[file.offset..end],
                    )?;
                    protocol::write_frame(&mut stream, &frame)?;
                    file.offset = end;
                    file.activity = Instant::now();
                    emit(PeerEvent::FileProgress {
                        id: file.id,
                        transferred: end as u64,
                    })?;
                } else {
                    protocol::write_frame(&mut stream, &session.complete_file(file.id)?)?;
                    file.completed = true;
                    file.activity = Instant::now();
                    emit(PeerEvent::FileSent(file.id))?;
                }
            }
        }
        Ok(())
    })();
    if let Err(error) = result
        && !cancelled.load(Ordering::Acquire)
    {
        let _ = emit(PeerEvent::Error(error.to_string()));
    }
    stream.cancel_handle().cancel();
}

/// Incremental RFCOMM frame reader: idle connections may remain open, but a
/// partial frame must finish in thirty seconds. Socket timeouts never discard
/// an already-read header/body fragment, preventing parser desynchronization.
fn read_network_frame(reader: &mut impl Read, cancelled: &AtomicBool) -> anyhow::Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    read_part(reader, &mut header, cancelled, None).and_then(|started| {
        let length = u32::from_be_bytes(header) as usize;
        anyhow::ensure!(
            (1..=MAX_NETWORK_FRAME).contains(&length),
            "Unzulässige Nachrichtengröße."
        );
        let mut body = vec![0; length];
        if let Err(error) = read_part(reader, &mut body, cancelled, started) {
            body.zeroize();
            return Err(error);
        }
        Ok(body)
    })
}
fn read_part(
    reader: &mut impl Read,
    out: &mut [u8],
    cancelled: &AtomicBool,
    mut started: Option<Instant>,
) -> anyhow::Result<Option<Instant>> {
    let mut used = 0;
    while used < out.len() {
        anyhow::ensure!(!cancelled.load(Ordering::Acquire), "Verbindung beendet.");
        if started.is_some_and(|t| t.elapsed() > FRAME_DEADLINE) {
            anyhow::bail!("Zeitlimit beim Empfang einer Nachricht überschritten.");
        }
        match reader.read(&mut out[used..]) {
            Ok(0) => anyhow::bail!("Bluetooth-Verbindung geschlossen."),
            Ok(count) => {
                if started.is_none() {
                    started = Some(Instant::now());
                }
                used += count;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(started)
}

/// Fail closed when Windows cannot establish the current interactive lock state.
/// This polls WTS session flags; no window subclass or package identity is needed.
#[cfg(windows)]
pub fn windows_session_locked() -> bool {
    use windows_sys::Win32::System::RemoteDesktop::{
        WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_UNLOCK, WTSFreeMemory,
        WTSINFOEXW, WTSQuerySessionInformationW, WTSSessionInfoEx,
    };
    let mut data = std::ptr::null_mut();
    let mut bytes = 0_u32;
    // SAFETY: WTS allocates the output; size is validated before reading, and its
    // buffer is always released with the matching WTSFreeMemory API.
    unsafe {
        if WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut data,
            &mut bytes,
        ) == 0
        {
            return true;
        }
        let locked = if !data.is_null() && bytes as usize >= std::mem::size_of::<WTSINFOEXW>() {
            let info = &*data.cast::<WTSINFOEXW>();
            info.Level != 1
                || info.Data.WTSInfoExLevel1.SessionFlags != WTS_SESSIONSTATE_UNLOCK as i32
        } else {
            true
        };
        WTSFreeMemory(data.cast());
        locked
    }
}
#[cfg(not(windows))]
pub fn windows_session_locked() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn native_lock_event_invalidates_queued_unlock_even_after_windows_unlocks() {
        let hub = CancellationHub::default();
        hub.generation.store(1, Ordering::Release);
        let queued_generation = hub.generation.load(Ordering::Acquire);
        let cancel = hub.flag(queued_generation);
        // The WTS event is retained even when no sensitive worker has started.
        hub.record_windows_lock();
        hub.observe_windows_state(false);
        assert_eq!(hub.generation.load(Ordering::Acquire), 2);
        assert_ne!(queued_generation, hub.generation.load(Ordering::Acquire));
        assert!(cancel.load(Ordering::Acquire));
        assert!(!hub.os_locked.load(Ordering::Acquire));
        assert!(!hub.sensitive.load(Ordering::Acquire));
    }

    #[test]
    fn lock_state_fallback_cancels_active_work_once_without_ui_frames() {
        let hub = CancellationHub::default();
        hub.generation.store(7, Ordering::Release);
        hub.sensitive.store(true, Ordering::Release);
        let cancel = hub.flag(7);
        hub.observe_windows_state(true);
        assert_eq!(hub.generation.load(Ordering::Acquire), 8);
        assert!(cancel.load(Ordering::Acquire));
        hub.observe_windows_state(true);
        assert_eq!(hub.generation.load(Ordering::Acquire), 8);
        hub.observe_windows_state(false);
        assert!(
            !hub.sensitive.load(Ordering::Acquire),
            "unlock must not restore a vault"
        );
    }

    #[test]
    fn partial_reads_survive_timeouts_and_preserve_unicode() {
        struct Fragmented {
            bytes: Cursor<Vec<u8>>,
            timeout: bool,
        }
        impl Read for Fragmented {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                self.timeout = !self.timeout;
                if self.timeout {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                let len = out.len().min(1);
                self.bytes.read(&mut out[..len])
            }
        }
        let content = "Künstliche Nachricht 🦀\nGrüsse".as_bytes();
        let mut wire = (content.len() as u32).to_be_bytes().to_vec();
        wire.extend_from_slice(content);
        let mut reader = Fragmented {
            bytes: Cursor::new(wire),
            timeout: false,
        };
        let frame = read_network_frame(&mut reader, &AtomicBool::new(false));
        assert!(matches!(frame, Ok(ref body) if body == content));
    }
    #[test]
    fn network_reader_rejects_bounds_and_cancelled_jobs() {
        for size in [0, MAX_NETWORK_FRAME + 1, u32::MAX as usize] {
            let mut reader = Cursor::new((size as u32).to_be_bytes());
            assert!(read_network_frame(&mut reader, &AtomicBool::new(false)).is_err());
        }
        let mut reader = Cursor::new(vec![0, 0, 0, 1, 42]);
        assert!(read_network_frame(&mut reader, &AtomicBool::new(true)).is_err());
    }
    #[test]
    fn locking_invalidates_queued_sensitive_events_and_commands() {
        let (commands, _) = bounded(2);
        let (tx, events) = bounded(2);
        let hub = Arc::new(CancellationHub::default());
        hub.generation.store(1, Ordering::Release);
        let runtime = Runtime {
            commands,
            events,
            hub: hub.clone(),
            mailbox: Arc::new(Mutex::new(Mailbox::default())),
        };
        let flag = hub.flag(1);
        let mut stale = View::default();
        stale.unlocked = true;
        stale.profile_name = "Synthetic secret profile".into();
        assert!(
            tx.send(Outbound {
                generation: 1,
                event: Event::State(stale)
            })
            .is_ok()
        );
        runtime.lock();
        assert!(runtime.try_event().is_none());
        assert!(flag.load(Ordering::Acquire));
        assert!(hub.flag(1).load(Ordering::Acquire));
        let mut stale = View::default();
        stale.unlocked = true;
        assert!(
            tx.send(Outbound {
                generation: 1,
                event: Event::State(stale)
            })
            .is_ok()
        );
        assert!(runtime.try_event().is_none());
    }
    fn actor_with_profile(
        directory: &tempfile::TempDir,
    ) -> anyhow::Result<(Actor, Receiver<Outbound>)> {
        let hub = Arc::new(CancellationHub::default());
        hub.generation.store(1, Ordering::Release);
        let (events, receiver) = bounded(COMMAND_CAPACITY);
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let mut actor = Actor::new(
            directory.path().join("synthetic.vault"),
            events,
            hub,
            mailbox,
        );
        actor.command(&mut Command::Create {
            name: "Synthetisches Profil".into(),
            password: Zeroizing::new("synthetic-password-123".into()),
        })?;
        Ok((actor, receiver))
    }

    #[test]
    fn actor_create_unlock_and_lock_reject_untrusted_or_locked_sends() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let (mut actor, _events) = actor_with_profile(&directory)?;
        assert!(actor.view.unlocked);
        assert!(
            actor
                .command(&mut Command::Send {
                    text: Zeroizing::new("NEVER_SEND_BEFORE_TRUST".into())
                })
                .is_err()
        );
        assert!(
            actor
                .vault
                .as_ref()
                .is_some_and(|v| v.profile().messages.is_empty())
        );
        actor.clear();
        assert!(actor.vault.is_none());
        assert!(actor.saved_profile.is_none());
        assert!(
            !actor.view.unlocked
                && actor.view.messages.is_empty()
                && actor.view.profile_name.is_empty()
        );
        assert!(
            actor
                .command(&mut Command::Send {
                    text: Zeroizing::new("NEVER_SEND_AFTER_LOCK".into())
                })
                .is_err()
        );
        assert!(
            actor
                .command(&mut Command::Unlock {
                    password: Zeroizing::new("incorrect-password".into())
                })
                .is_err()
        );
        assert!(!actor.view.unlocked);
        actor.command(&mut Command::Unlock {
            password: Zeroizing::new("synthetic-password-123".into()),
        })?;
        assert!(actor.view.unlocked && actor.view.profile_name == "Synthetisches Profil");
        Ok(())
    }

    fn attach_synthetic_peer(
        actor: &mut Actor,
        ready: bool,
    ) -> anyhow::Result<Receiver<PeerCommand>> {
        let key = [13_u8; 32];
        if let Some(vault) = &mut actor.vault {
            vault
                .profile_mut()
                .contacts
                .retain(|c| c.identity_key != key);
            vault.profile_mut().contacts.push(vault::Contact {
                id: hex::encode(key),
                display_name: "Künstlicher Testkontakt".into(),
                identity_key: key,
                bluetooth_address: "01:23:45:67:89:AB".into(),
            });
        }
        anyhow::ensure!(actor.save(), "synthetic profile could not be saved");
        let mut view = PeerView::default();
        view.id = hex::encode(key);
        view.name = "Künstlicher Testkontakt".into();
        view.ready = ready;
        actor.view.peer = Some(view);
        let (commands, receiver) = bounded(COMMAND_CAPACITY);
        actor.peer = Some(PeerWorker {
            id: 42,
            device: transport::Device {
                name: "Synthetic device".into(),
                address: "01:23:45:67:89:AB".into(),
            },
            commands,
            cancel: None,
            flag: Arc::new(AtomicBool::new(false)),
            key: Some(key),
            _lease: None,
        });
        Ok(receiver)
    }

    #[test]
    fn actor_inactivity_drops_vault_without_ui_frames() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let (mut actor, _events) = actor_with_profile(&directory)?;
        let generation = actor.generation;
        if let Some(vault) = &mut actor.vault {
            vault.profile_mut().settings.inactivity_minutes = 1;
        }
        if let Ok(mut activity) = actor.hub.last_activity.lock() {
            *activity = Some(Instant::now() - Duration::from_secs(61));
        }
        actor.enforce_idle_lock();
        assert!(actor.vault.is_none());
        assert!(!actor.view.unlocked);
        assert!(actor.view.messages.is_empty());
        assert!(actor.hub.generation.load(Ordering::Acquire) > generation);
        actor.hub.touch();
        assert!(
            actor.vault.is_none(),
            "activity cannot reopen a locked vault"
        );
        Ok(())
    }

    #[test]
    fn actor_never_releases_chat_before_trust_and_acks_only_durable_messages() -> anyhow::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let (mut actor, _events) = actor_with_profile(&directory)?;
        let commands = attach_synthetic_peer(&mut actor, false)?;
        actor.peer_event(PeerEvent::Chat {
            id: [1; 16],
            text: Zeroizing::new("UNTRUSTED_MUST_NOT_PERSIST".into()),
        });
        assert!(actor.view.messages.is_empty());
        assert!(
            actor
                .vault
                .as_ref()
                .is_some_and(|v| v.profile().messages.is_empty())
        );
        assert!(
            !commands
                .try_iter()
                .any(|c| matches!(c, PeerCommand::Ack(_)))
        );
        let commands = attach_synthetic_peer(&mut actor, true)?;
        actor.peer_event(PeerEvent::Chat {
            id: [2; 16],
            text: Zeroizing::new("SYNTHETIC_DURABLE_MESSAGE_🦀".into()),
        });
        assert!(matches!(commands.try_recv(), Ok(PeerCommand::Ack(id)) if id == [2;16]));
        assert_eq!(actor.view.messages.len(), 1);
        let bytes = std::fs::read(&actor.path)?;
        assert!(
            !bytes
                .windows(b"SYNTHETIC_DURABLE_MESSAGE".len())
                .any(|w| w == b"SYNTHETIC_DURABLE_MESSAGE")
        );
        // A repeated id from a fresh session must not be persisted a second time.
        actor.peer_event(PeerEvent::Chat {
            id: [2; 16],
            text: Zeroizing::new("DUPLICATE".into()),
        });
        assert_eq!(actor.view.messages.len(), 1);
        assert!(actor.peer.is_none());
        Ok(())
    }

    #[test]
    fn actor_save_failure_rolls_back_and_never_sends_receipt_or_outgoing_frame()
    -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let (mut actor, _events) = actor_with_profile(&directory)?;
        let commands = attach_synthetic_peer(&mut actor, true)?;
        let corrupt = b"SYNTHETIC_CORRUPT_VAULT";
        std::fs::write(&actor.path, corrupt)?;
        actor.peer_event(PeerEvent::Chat {
            id: [3; 16],
            text: Zeroizing::new("NEVER_ACK_UNSAVED".into()),
        });
        assert!(actor.view.messages.is_empty());
        assert!(
            !commands
                .try_iter()
                .any(|c| matches!(c, PeerCommand::Ack(_)))
        );
        assert_eq!(std::fs::read(&actor.path)?, corrupt);
        // The outgoing write permission is also denied when persistence fails.
        let mut peer = PeerView::default();
        peer.id = hex::encode([13; 32]);
        peer.ready = true;
        actor.view.peer = Some(peer);
        let (persisted, durable) = bounded(1);
        actor.peer_event(PeerEvent::Outgoing {
            id: [4; 16],
            text: Zeroizing::new("NEVER_WRITE_UNSAVED".into()),
            persisted,
        });
        assert!(matches!(durable.try_recv(), Ok(false)));
        assert!(actor.view.messages.is_empty());
        assert_eq!(std::fs::read(&actor.path)?, corrupt);
        Ok(())
    }

    #[test]
    fn files_require_consent_and_durable_storage_before_receipt() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let (mut actor, _events) = actor_with_profile(&directory)?;
        let commands = attach_synthetic_peer(&mut actor, true)?;
        let data = b"SYNTHETIC_ATTACHMENT_UNIQUE_836194_UNICODE_FILE";
        let id = [81; 16];
        actor.peer_event(PeerEvent::FileOffer {
            id,
            name: "Grüsse 🦀.txt".into(),
            size: data.len() as u64,
            sha256: Sha256::digest(data).into(),
        });
        assert!(commands.try_recv().is_err(), "offers never auto-accept");
        assert!(actor.view.messages.is_empty());
        actor.command(&mut Command::RespondFile {
            id: hex::encode(id),
            accept: true,
        })?;
        assert!(matches!(commands.try_recv(), Ok(PeerCommand::AcceptFile(got)) if got == id));
        assert_eq!(actor.view.messages[0].status, vault::MessageStatus::Pending);
        actor.peer_event(PeerEvent::FileChunk {
            id,
            offset: 0,
            data: Zeroizing::new(data.to_vec()),
        });
        assert!(
            commands.try_recv().is_err(),
            "a chunk does not prove complete storage"
        );
        actor.peer_event(PeerEvent::FileComplete(id));
        assert!(matches!(commands.try_recv(), Ok(PeerCommand::FileStored(got)) if got == id));
        assert_eq!(
            actor.view.messages[0].status,
            vault::MessageStatus::Received
        );
        assert!(
            actor
                .vault
                .as_ref()
                .is_some_and(|vault| vault.profile().attachments[0].data == data)
        );
        let encrypted = std::fs::read(&actor.path)?;
        assert!(!encrypted.windows(data.len()).any(|window| window == data));
        actor.clear();
        assert!(actor.incoming_file.is_none() && actor.view.transfers.is_empty());
        assert!(
            actor
                .command(&mut Command::SendFile {
                    path: directory.path().join("never-read.txt")
                })
                .is_err()
        );
        actor.command(&mut Command::Unlock {
            password: Zeroizing::new("synthetic-password-123".into()),
        })?;
        assert!(
            actor
                .vault
                .as_ref()
                .is_some_and(|vault| vault.profile().attachments[0].data == data)
        );
        Ok(())
    }

    #[test]
    fn file_corruption_or_failed_save_never_produces_storage_receipt() -> anyhow::Result<()> {
        for fail_save in [false, true] {
            let directory = tempfile::tempdir()?;
            let (mut actor, _events) = actor_with_profile(&directory)?;
            let commands = attach_synthetic_peer(&mut actor, true)?;
            let data = b"SYNTHETIC_BINARY_FAILURE_CASE";
            let id = [82; 16];
            actor.peer_event(PeerEvent::FileOffer {
                id,
                name: "synthetic.bin".into(),
                size: data.len() as u64,
                sha256: if fail_save {
                    Sha256::digest(data).into()
                } else {
                    [0; 32]
                },
            });
            actor.command(&mut Command::RespondFile {
                id: hex::encode(id),
                accept: true,
            })?;
            assert!(matches!(
                commands.try_recv(),
                Ok(PeerCommand::AcceptFile(_))
            ));
            actor.peer_event(PeerEvent::FileChunk {
                id,
                offset: 0,
                data: Zeroizing::new(data.to_vec()),
            });
            if fail_save {
                std::fs::write(&actor.path, b"SYNTHETIC_EXTERNAL_CHANGE")?;
            }
            actor.peer_event(PeerEvent::FileComplete(id));
            assert!(
                !commands
                    .try_iter()
                    .any(|command| matches!(command, PeerCommand::FileStored(_)))
            );
            assert!(
                actor
                    .vault
                    .as_ref()
                    .is_some_and(|vault| vault.profile().attachments.is_empty())
            );
            assert!(actor.peer.is_none());
            if fail_save {
                assert_eq!(std::fs::read(&actor.path)?, b"SYNTHETIC_EXTERNAL_CHANGE");
            }
        }
        Ok(())
    }

    #[test]
    fn files_reject_untrusted_bytes_and_cancelled_inflight_bytes_are_discarded()
    -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let (mut actor, _events) = actor_with_profile(&directory)?;
        let commands = attach_synthetic_peer(&mut actor, false)?;
        assert!(
            actor
                .command(&mut Command::SendFile {
                    path: directory.path().join("never-open.txt")
                })
                .is_err()
        );
        actor.peer_event(PeerEvent::FileOffer {
            id: [83; 16],
            name: "untrusted.txt".into(),
            size: 1,
            sha256: [0; 32],
        });
        assert!(actor.peer.is_none() && actor.view.transfers.is_empty());
        assert!(
            commands
                .try_iter()
                .all(|command| !matches!(command, PeerCommand::AcceptFile(_)))
        );
        let commands = attach_synthetic_peer(&mut actor, true)?;
        let id = [84; 16];
        actor.peer_event(PeerEvent::FileOffer {
            id,
            name: "cancel.txt".into(),
            size: 1,
            sha256: Sha256::digest([42]).into(),
        });
        actor.command(&mut Command::RespondFile {
            id: hex::encode(id),
            accept: true,
        })?;
        actor.command(&mut Command::CancelFile {
            id: hex::encode(id),
        })?;
        actor.peer_event(PeerEvent::FileChunk {
            id,
            offset: 0,
            data: Zeroizing::new(vec![42]),
        });
        actor.peer_event(PeerEvent::FileComplete(id));
        assert!(
            actor.peer.is_some(),
            "in-flight data crossing a cancellation must not kill chat"
        );
        assert!(actor.incoming_file.is_none());
        assert_eq!(actor.view.messages[0].status, vault::MessageStatus::Failed);
        assert!(
            !commands
                .try_iter()
                .any(|command| matches!(command, PeerCommand::FileStored(_)))
        );
        actor.clear();
        assert!(
            actor.saved_profile.is_none()
                && actor.view.messages.is_empty()
                && actor.view.transfers.is_empty()
        );
        Ok(())
    }

    #[test]
    fn outgoing_file_failure_never_authorizes_an_offer() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let (mut actor, _events) = actor_with_profile(&directory)?;
        let _commands = attach_synthetic_peer(&mut actor, true)?;
        let data = b"SYNTHETIC_UNSAVED_OUTGOING_FILE";
        let file = PreparedFile {
            name: Zeroizing::new("unsaved.bin".into()),
            data: Arc::new(Zeroizing::new(data.to_vec())),
            sha256: Sha256::digest(data).into(),
        };
        std::fs::write(&actor.path, b"SYNTHETIC_CHANGED_VAULT")?;
        let (persisted, durable) = bounded(1);
        actor.peer_event(PeerEvent::FileOffered {
            id: [86; 16],
            file,
            persisted,
        });
        assert!(matches!(durable.try_recv(), Ok(false)));
        assert!(actor.peer.is_none());
        assert!(
            actor
                .vault
                .as_ref()
                .is_some_and(|vault| vault.profile().attachments.is_empty())
        );
        assert_eq!(std::fs::read(&actor.path)?, b"SYNTHETIC_CHANGED_VAULT");
        let (saved, receipt) = bounded(1);
        saved.send(true)?;
        assert!(
            await_persistence(&receipt, &AtomicBool::new(true)).is_err(),
            "lock wins even when persistence was already reported successful"
        );
        Ok(())
    }

    #[test]
    fn outgoing_file_receipt_and_exports_preserve_original_ciphertext() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let (mut actor, _events) = actor_with_profile(&directory)?;
        let _commands = attach_synthetic_peer(&mut actor, true)?;
        let source = directory.path().join("Quelle 🦀.txt");
        let data = b"SYNTHETIC_OUTGOING_FILE_BYTES_19385";
        std::fs::write(&source, data)?;
        assert!(prepare_file(&source, &AtomicBool::new(true)).is_err());
        let file = prepare_file(&source, &AtomicBool::new(false))?;
        let id = [85; 16];
        let (persisted, durable) = bounded(1);
        actor.peer_event(PeerEvent::FileOffered {
            id,
            file,
            persisted,
        });
        assert!(matches!(durable.try_recv(), Ok(true)));
        assert_eq!(actor.view.messages[0].status, vault::MessageStatus::Pending);
        actor.peer_event(PeerEvent::FileSent(id));
        assert_eq!(actor.view.messages[0].status, vault::MessageStatus::Sent);
        actor.peer_event(PeerEvent::FileStored(id));
        assert_eq!(
            actor.view.messages[0].status,
            vault::MessageStatus::Received
        );
        let destination = directory.path().join("Export.txt");
        let digest = Sha256::digest(data).into();
        export_attachment(&destination, data, digest, &AtomicBool::new(false))?;
        assert_eq!(std::fs::read(&destination)?, data);
        assert!(
            export_attachment(
                &destination,
                b"overwrite",
                Sha256::digest(b"overwrite").into(),
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&destination)?, data);
        let cancelled = directory.path().join("cancelled.txt");
        assert!(export_attachment(&cancelled, data, digest, &AtomicBool::new(true)).is_err());
        assert!(!cancelled.exists());
        let corrupt = directory.path().join("corrupt.txt");
        assert!(export_attachment(&corrupt, data, [0; 32], &AtomicBool::new(false)).is_err());
        assert!(!corrupt.exists());
        actor.disconnect("synthetic test disconnect");
        actor.command(&mut Command::DeleteHistory {
            contact_id: hex::encode([13; 32]),
        })?;
        assert!(
            actor
                .vault
                .as_ref()
                .is_some_and(|vault| vault.profile().attachments.is_empty()
                    && vault.profile().messages.is_empty())
        );
        Ok(())
    }

    #[test]
    fn chosen_existing_vault_is_never_overwritten_and_preferences_survive_lock()
    -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let (mut actor, _events) = actor_with_profile(&directory)?;
        actor.command(&mut Command::SetDesktopSettings {
            start_with_windows: false,
            minimize_to_tray: false,
            automatic_updates: false,
        })?;
        // A legacy profile's defaults must not undo a global update opt-out.
        if let Some(vault) = &mut actor.vault {
            vault.profile_mut().settings.automatic_updates = true;
            vault.save()?;
        }
        actor.clear();
        actor.command(&mut Command::Unlock {
            password: Zeroizing::new("synthetic-password-123".into()),
        })?;
        assert!(!actor.view.automatic_updates);
        actor.clear();
        assert!(
            !actor.view.automatic_updates
                && !actor.view.start_with_windows
                && !actor.view.minimize_to_tray
        );
        let selected = directory.path().join("existing.vault");
        std::fs::write(&selected, b"SYNTHETIC_DAMAGED_EXISTING_FILE")?;
        actor.command(&mut Command::ChooseVault {
            path: selected.clone(),
        })?;
        assert_eq!(actor.path, selected);
        assert!(
            actor
                .command(&mut Command::Create {
                    name: "Synthetic".into(),
                    password: Zeroizing::new("synthetic-password-123".into())
                })
                .is_err()
        );
        assert_eq!(
            std::fs::read(&selected)?,
            b"SYNTHETIC_DAMAGED_EXISTING_FILE"
        );
        assert!(
            actor
                .command(&mut Command::Unlock {
                    password: Zeroizing::new("synthetic-password-123".into())
                })
                .is_err()
        );
        assert_eq!(
            std::fs::read(&selected)?,
            b"SYNTHETIC_DAMAGED_EXISTING_FILE"
        );
        assert_eq!(storage_location::load(&actor.default_path)?, selected);
        assert!(!storage_location::load_preferences(&actor.default_path)?.automatic_updates);
        Ok(())
    }

    #[test]
    fn coalesced_state_and_busy_completion_survive_full_error_queue() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let hub = Arc::new(CancellationHub::default());
        hub.generation.store(1, Ordering::Release);
        let (events, _errors) = bounded(1);
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let mut actor = Actor::new(
            directory.path().join("unused.vault"),
            events,
            hub,
            mailbox.clone(),
        );
        actor.error("Synthetic error fills queue");
        actor.emit(Event::Busy(true));
        actor.emit(Event::Busy(false));
        actor.view.status = "Final synthetic state".into();
        actor.snapshot();
        let mut guard = mailbox
            .lock()
            .map_err(|_| anyhow::anyhow!("test mailbox poisoned"))?;
        assert!(matches!(guard.busy.take(), Some((1, false))));
        assert!(
            matches!(guard.state.take(), Some((1, ref view)) if view.status == "Final synthetic state")
        );
        Ok(())
    }
}
