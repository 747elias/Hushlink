//! Worker actors connecting the encrypted vault, authenticated session and real RFCOMM.
//! All KDF, file, Bluetooth and protocol work runs outside the eframe thread.

use crate::{protocol, transport, vault};
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded, select};
use std::{
    io::{self, Read},
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
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct View {
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
    #[cfg(any(windows, test))]
    os_locked: AtomicBool,
    #[cfg(windows)]
    watcher_ready: AtomicBool,
    flags: Mutex<Vec<Arc<AtomicBool>>>,
    sockets: Mutex<Vec<(u64, transport::CancelHandle)>>,
    socket_id: AtomicU64,
}
impl CancellationHub {
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
}
enum PeerCommand {
    Confirm,
    Send(Zeroizing<String>),
    Ack(protocol::MessageId),
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
}
impl Actor {
    fn new(
        path: PathBuf,
        events: Sender<Outbound>,
        hub: Arc<CancellationHub>,
        mailbox: Arc<Mutex<Mailbox>>,
    ) -> Self {
        let (background_tx, background) = bounded(COMMAND_CAPACITY);
        let mut view = View::default();
        view.profile_exists = path.exists();
        view.status = "Tresor gesperrt".into();
        view.inactivity_minutes = 5;
        Self {
            path,
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
    fn snapshot(&self) {
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
                timestamp_ms: m.timestamp_ms,
                outgoing: m.outgoing,
                status: m.status,
            })
            .collect();
    }
    fn save(&mut self) -> bool {
        let result = self.vault.as_mut().map(|v| v.save());
        match result {
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
        self.view.profile_exists = self.path.exists();
        self.view.status = "Tresor gesperrt".into();
        self.view.inactivity_minutes = 5;
        self.snapshot();
        self.emit(Event::Busy(false));
    }
    fn run(mut self, commands: Receiver<Envelope>) {
        self.snapshot();
        loop {
            let generation = self.hub.generation.load(Ordering::Acquire);
            if generation != self.generation {
                self.generation = generation;
                self.clear();
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
                        self.background(event); self.snapshot();
                    }
                }
                default(Duration::from_millis(100)) => {}
            }
        }
    }
    fn command(&mut self, command: &mut Command) -> anyhow::Result<()> {
        match command {
            Command::Create { name, password } => {
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
                let profile = vault::Profile::new(
                    name.trim().to_owned(),
                    *identity.private(),
                    identity.public(),
                );
                let result = vault::Vault::create(&self.path, password.as_str(), profile);
                self.emit(Event::Busy(false));
                if !self.current() {
                    return Ok(());
                }
                self.vault = Some(result?);
                self.after_unlock();
            }
            Command::Unlock { password } => {
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
        self.saved_profile = self.vault.as_ref().map(|v| v.profile().clone());
        // Pending/sent messages from a former connection cannot acquire an ACK in a new
        // Noise session. Keep their history and mark their delivery as uncertain/failed.
        if let Some(vault) = &mut self.vault {
            for message in &mut vault.profile_mut().messages {
                if message.outgoing
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
                    && message.outgoing
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
                            let deadline = Instant::now() + Duration::from_secs(30);
                            loop {
                                anyhow::ensure!(!cancelled.load(Ordering::Acquire), "Verbindung beendet.");
                                anyhow::ensure!(Instant::now() < deadline, "Das sichere Speichern hat das Zeitlimit erreicht.");
                                match durable.recv_timeout(Duration::from_millis(100)) {
                                    Ok(true) => break,
                                    Ok(false) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => anyhow::bail!("Nachricht konnte nicht sicher gespeichert werden."),
                                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                                }
                            }
                            protocol::write_frame(&mut stream, &frame)?;
                            emit(PeerEvent::Sent(message_id))?;
                        }
                        PeerCommand::Ack(message_id) => { protocol::write_frame(&mut stream, &session.acknowledge(message_id)?)?; }
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
                        protocol::Event::Closed => { emit(PeerEvent::Closed)?; break; }
                    }
                }
                default(Duration::from_millis(100)) => {}
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
