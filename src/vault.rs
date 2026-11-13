//! Versioned, authenticated local profile storage. No plaintext is written to disk.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const MAGIC: &[u8; 8] = b"HUSHVLT\0";
const FORMAT_VERSION: u16 = 1;
const WRAP_AAD_LEN: usize = 64;
const BASE_HEADER_LEN: usize = 92;
const WRAPPED_KEY_LEN: usize = 48;
const HEADER_LEN: usize = BASE_HEADER_LEN + WRAPPED_KEY_LEN;
const TAG_LEN: usize = 16;
pub const MAX_VAULT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;
pub const MAX_MESSAGES: usize = 10_000;
pub const MAX_CONTACTS: usize = 500;

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub display_name: String,
    pub identity_private: [u8; 32],
    pub identity_public: [u8; 32],
    pub contacts: Vec<Contact>,
    pub messages: Vec<StoredMessage>,
    pub settings: Settings,
}

impl Profile {
    pub fn new(
        display_name: String,
        identity_private: [u8; 32],
        identity_public: [u8; 32],
    ) -> Self {
        Self {
            display_name,
            identity_private,
            identity_public,
            contacts: Vec::new(),
            messages: Vec::new(),
            settings: Settings::default(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    pub id: String,
    pub display_name: String,
    pub identity_key: [u8; 32],
    pub bluetooth_address: String,
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct StoredMessage {
    pub id: String,
    pub contact_id: String,
    pub text: String,
    pub timestamp_ms: u64,
    pub outgoing: bool,
    pub status: MessageStatus,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum MessageStatus {
    Pending,
    Sent,
    Received,
    Failed,
}

impl Zeroize for MessageStatus {
    fn zeroize(&mut self) {
        *self = Self::Failed;
    }
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Minutes until inactivity locking, in the inclusive range 1..=120.
    pub inactivity_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            inactivity_minutes: 5,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Dateizugriff fehlgeschlagen: {0}")]
    Io(#[from] std::io::Error),
    #[error("Passwort falsch oder Tresor wurde verändert. Die Datei bleibt unverändert.")]
    Authentication,
    #[error("Tresorformat ist beschädigt oder wird nicht unterstützt: {0}")]
    Format(&'static str),
    #[error("Ein Tresor ist bereits vorhanden. Er wird nicht überschrieben.")]
    AlreadyExists,
    #[error("Der Tresor ist bereits in einer anderen Hushlink-Instanz geöffnet.")]
    InUse,
    #[error(
        "Die Tresordatei wurde ausserhalb dieser Sitzung geändert. Bitte sperren und erneut öffnen."
    )]
    Conflict,
    #[error("Bitte mindestens 12 Zeichen als Passwort verwenden.")]
    PasswordTooShort,
    #[error("Das Passwort darf höchstens 1’024 UTF-8-Byte lang sein.")]
    PasswordTooLong,
    #[error(
        "Profil enthält ungültige oder zu viele Daten. Bitte Namen, Kontakte und Nachrichten prüfen."
    )]
    InvalidProfile,
    #[error("Der Tresor hat seine Grössenbegrenzung erreicht. Bitte alte Verläufe löschen.")]
    Limit,
    #[error("Kryptografische Verarbeitung ist fehlgeschlagen.")]
    Crypto,
}

pub type Result<T> = std::result::Result<T, VaultError>;

#[derive(Clone, Copy)]
struct KdfParameters {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl Default for KdfParameters {
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }
}

impl KdfParameters {
    fn params(self) -> Result<Params> {
        if !(64 * 1024..=256 * 1024).contains(&self.memory_kib)
            || !(3..=6).contains(&self.iterations)
            || !(1..=4).contains(&self.parallelism)
        {
            return Err(VaultError::Format("unzulässige Passwortparameter"));
        }
        Params::new(self.memory_kib, self.iterations, self.parallelism, Some(32))
            .map_err(|_| VaultError::Format("ungültige Passwortparameter"))
    }
}

struct Envelope {
    header: [u8; HEADER_LEN],
    body: Vec<u8>,
}

impl Envelope {
    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN + TAG_LEN || bytes.len() > MAX_VAULT_BYTES {
            return Err(VaultError::Format("ungültige Dateigrösse"));
        }
        let mut header = [0; HEADER_LEN];
        header.copy_from_slice(&bytes[..HEADER_LEN]);
        if &header[..8] != MAGIC
            || u16::from_le_bytes([header[8], header[9]]) != FORMAT_VERSION
            || header[10] != 1
            || header[11] != 1
        {
            return Err(VaultError::Format("unbekannte Version oder Kennung"));
        }
        let envelope = Self {
            header,
            body: bytes[HEADER_LEN..].to_vec(),
        };
        envelope.kdf()?.params()?;
        let length = read_u32(&envelope.header, 88)? as usize;
        if !(TAG_LEN..=MAX_VAULT_BYTES - HEADER_LEN).contains(&length)
            || length != envelope.body.len()
        {
            return Err(VaultError::Format("ungültige Inhaltslänge"));
        }
        Ok(envelope)
    }

    fn kdf(&self) -> Result<KdfParameters> {
        Ok(KdfParameters {
            memory_kib: read_u32(&self.header, 12)?,
            iterations: read_u32(&self.header, 16)?,
            parallelism: read_u32(&self.header, 20)?,
        })
    }

    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN + self.body.len());
        bytes.extend_from_slice(&self.header);
        bytes.extend_from_slice(&self.body);
        bytes
    }

    fn unwrap_key(&self, password: &str) -> Result<Zeroizing<[u8; 32]>> {
        let password_key = derive_key(password, &self.header[24..40], self.kdf()?)?;
        let cipher = XChaCha20Poly1305::new_from_slice(password_key.as_ref())
            .map_err(|_| VaultError::Crypto)?;
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    XNonce::from_slice(&self.header[40..64]),
                    Payload {
                        msg: &self.header[BASE_HEADER_LEN..HEADER_LEN],
                        aad: &self.header[..WRAP_AAD_LEN],
                    },
                )
                .map_err(|_| VaultError::Authentication)?,
        );
        if plain.len() != 32 {
            return Err(VaultError::Authentication);
        }
        let mut key = Zeroizing::new([0; 32]);
        key.copy_from_slice(&plain);
        Ok(key)
    }

    fn decrypt_profile(&self, key: &[u8; 32]) -> Result<Profile> {
        let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| VaultError::Crypto)?;
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    XNonce::from_slice(&self.header[64..88]),
                    Payload {
                        msg: &self.body,
                        aad: &self.header,
                    },
                )
                .map_err(|_| VaultError::Authentication)?,
        );
        let profile = serde_json::from_slice::<Profile>(&plain)
            .map_err(|_| VaultError::Format("ungültiger entschlüsselter Inhalt"))?;
        validate_profile(&profile)?;
        Ok(profile)
    }
}

/// The unlocked vault owns the independent data key and all decrypted persisted data.
/// Dropping it zeroizes its key and profile. It never stores a password.
pub struct Vault {
    path: PathBuf,
    profile: Profile,
    key: Zeroizing<[u8; 32]>,
    envelope: Envelope,
    expected_digest: [u8; 32],
    // Persistent lock-file inode: do not unlink it while other processes can hold it.
    _lock_file: File,
}

impl Vault {
    pub fn create(path: impl AsRef<Path>, password: &str, profile: Profile) -> Result<Self> {
        validate_new_password(password)?;
        validate_profile(&profile)?;
        let path = path.as_ref().to_path_buf();
        let parent = path.parent().ok_or(VaultError::Format("ungültiger Pfad"))?;
        fs::create_dir_all(parent)?;
        let lock_file = lock_file(&path)?;
        if path.try_exists()? {
            return Err(VaultError::AlreadyExists);
        }
        let mut key = Zeroizing::new([0; 32]);
        OsRng.fill_bytes(key.as_mut());
        let envelope = new_envelope(password, &key, &profile)?;
        let bytes = envelope.encode();
        atomic_write(&path, &bytes, None)?;
        Ok(Self {
            path,
            profile,
            key,
            envelope,
            expected_digest: digest(&bytes),
            _lock_file: lock_file,
        })
    }

    pub fn open(path: impl AsRef<Path>, password: &str) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let lock_file = lock_file(&path)?;
        let bytes = bounded_read(&path)?;
        let envelope = Envelope::decode(&bytes)?;
        let key = envelope.unwrap_key(password)?;
        let profile = envelope.decrypt_profile(&key)?;
        Ok(Self {
            path,
            profile,
            key,
            envelope,
            expected_digest: digest(&bytes),
            _lock_file: lock_file,
        })
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        &mut self.profile
    }

    /// Encrypt and atomically persist changes. Changes made outside this instance
    /// are detected and left untouched. A failed save leaves the last file intact.
    pub fn save(&mut self) -> Result<()> {
        validate_profile(&self.profile)?;
        let envelope = update_body(&self.envelope, &self.key, &self.profile)?;
        self.persist(envelope)
    }

    /// Verify the existing password and rewrap the unchanged random data key.
    /// A fresh salt, wrapping nonce and body nonce are generated for every change.
    pub fn change_password(&mut self, old_password: &str, new_password: &str) -> Result<()> {
        validate_new_password(new_password)?;
        // Authentication of the old key wrap is necessary even while unlocked.
        let _verified_key = self.envelope.unwrap_key(old_password)?;
        let envelope = new_envelope(new_password, &self.key, &self.profile)?;
        self.persist(envelope)
    }

    fn persist(&mut self, envelope: Envelope) -> Result<()> {
        let bytes = envelope.encode();
        atomic_write(&self.path, &bytes, Some(self.expected_digest))?;
        self.expected_digest = digest(&bytes);
        self.envelope = envelope;
        Ok(())
    }
}

fn validate_new_password(password: &str) -> Result<()> {
    if password.len() > 1024 {
        return Err(VaultError::PasswordTooLong);
    }
    if password.chars().count() < 12 {
        return Err(VaultError::PasswordTooShort);
    }
    Ok(())
}

fn validate_profile(profile: &Profile) -> Result<()> {
    fn valid_name(name: &str) -> bool {
        !name.trim().is_empty() && name.len() <= 80 && !name.chars().any(char::is_control)
    }
    if !valid_name(&profile.display_name)
        || profile.contacts.len() > MAX_CONTACTS
        || profile.messages.len() > MAX_MESSAGES
        || !(1..=120).contains(&profile.settings.inactivity_minutes)
    {
        return Err(VaultError::InvalidProfile);
    }
    for (index, contact) in profile.contacts.iter().enumerate() {
        if contact.id.len() != 64
            || contact.id != hex::encode(contact.identity_key)
            || !valid_name(&contact.display_name)
            || contact.bluetooth_address.len() > 128
            || profile.contacts[..index].iter().any(|c| c.id == contact.id)
        {
            return Err(VaultError::InvalidProfile);
        }
    }
    for message in &profile.messages {
        if message.id.is_empty()
            || message.id.len() > 128
            || message.text.is_empty()
            || message.text.len() > MAX_MESSAGE_BYTES
            || !profile.contacts.iter().any(|c| c.id == message.contact_id)
        {
            return Err(VaultError::InvalidProfile);
        }
    }
    Ok(())
}

fn derive_key(
    password: &str,
    salt: &[u8],
    parameters: KdfParameters,
) -> Result<Zeroizing<[u8; 32]>> {
    if password.len() > 1024 {
        return Err(VaultError::Authentication);
    }
    let params = parameters.params()?;
    let mut memory = Zeroizing::new(vec![argon2::Block::default(); params.block_count()]);
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0; 32]);
    argon2
        .hash_password_into_with_memory(
            password.as_bytes(),
            salt,
            key.as_mut(),
            memory.as_mut_slice(),
        )
        .map_err(|_| VaultError::Crypto)?;
    Ok(key)
}

fn new_envelope(password: &str, key: &[u8; 32], profile: &Profile) -> Result<Envelope> {
    validate_profile(profile)?;
    let plain =
        Zeroizing::new(serde_json::to_vec(profile).map_err(|_| VaultError::InvalidProfile)?);
    let length = plain.len().checked_add(TAG_LEN).ok_or(VaultError::Limit)?;
    if length > MAX_VAULT_BYTES - HEADER_LEN {
        return Err(VaultError::Limit);
    }
    let parameters = KdfParameters::default();
    let mut header = [0; HEADER_LEN];
    header[..8].copy_from_slice(MAGIC);
    header[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    header[10] = 1; // Argon2id v19
    header[11] = 1; // XChaCha20-Poly1305
    header[12..16].copy_from_slice(&parameters.memory_kib.to_le_bytes());
    header[16..20].copy_from_slice(&parameters.iterations.to_le_bytes());
    header[20..24].copy_from_slice(&parameters.parallelism.to_le_bytes());
    OsRng.fill_bytes(&mut header[24..88]);
    header[88..92].copy_from_slice(&(length as u32).to_le_bytes());
    let password_key = derive_key(password, &header[24..40], parameters)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(password_key.as_ref()).map_err(|_| VaultError::Crypto)?;
    let wrapped = cipher
        .encrypt(
            XNonce::from_slice(&header[40..64]),
            Payload {
                msg: key,
                aad: &header[..WRAP_AAD_LEN],
            },
        )
        .map_err(|_| VaultError::Crypto)?;
    header[BASE_HEADER_LEN..HEADER_LEN].copy_from_slice(&wrapped);
    encrypt_body(header, key, &plain)
}

fn update_body(envelope: &Envelope, key: &[u8; 32], profile: &Profile) -> Result<Envelope> {
    let plain =
        Zeroizing::new(serde_json::to_vec(profile).map_err(|_| VaultError::InvalidProfile)?);
    let length = plain.len().checked_add(TAG_LEN).ok_or(VaultError::Limit)?;
    if length > MAX_VAULT_BYTES - HEADER_LEN {
        return Err(VaultError::Limit);
    }
    // The wrapping AAD covers only the stable KDF/wrap prefix. The full header,
    // including the fresh body nonce and changed length, is authenticated by
    // the body AEAD. No password-derived key needs to remain in memory.
    let mut header = envelope.header;
    OsRng.fill_bytes(&mut header[64..88]);
    header[88..92].copy_from_slice(&(length as u32).to_le_bytes());
    encrypt_body(header, key, &plain)
}

fn encrypt_body(header: [u8; HEADER_LEN], key: &[u8; 32], plain: &[u8]) -> Result<Envelope> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| VaultError::Crypto)?;
    let body = cipher
        .encrypt(
            XNonce::from_slice(&header[64..88]),
            Payload {
                msg: plain,
                aad: &header,
            },
        )
        .map_err(|_| VaultError::Crypto)?;
    Ok(Envelope { header, body })
}

fn read_u32(bytes: &[u8], start: usize) -> Result<u32> {
    let raw = bytes
        .get(start..start + 4)
        .ok_or(VaultError::Format("unvollständiger Header"))?;
    let raw: [u8; 4] = raw
        .try_into()
        .map_err(|_| VaultError::Format("ungültiger Header"))?;
    Ok(u32::from_le_bytes(raw))
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn bounded_read(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_VAULT_BYTES as u64 {
        return Err(VaultError::Format("Datei ist zu gross"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_VAULT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_VAULT_BYTES {
        return Err(VaultError::Format("Datei ist zu gross"));
    }
    Ok(bytes)
}

fn lock_file(path: &Path) -> Result<File> {
    let mut lock_name = path.as_os_str().to_os_string();
    lock_name.push(".lock");
    let file = private_open_options()
        .create(true)
        .truncate(false)
        .open(PathBuf::from(lock_name))?;
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => VaultError::InUse,
        std::fs::TryLockError::Error(error) => VaultError::Io(error),
    })?;
    Ok(file)
}

fn private_open_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

fn atomic_write(path: &Path, bytes: &[u8], expected: Option<[u8; 32]>) -> Result<()> {
    verify_current(path, expected)?;
    let parent = path.parent().ok_or(VaultError::Format("ungültiger Pfad"))?;
    let mut random = [0; 16];
    OsRng.fill_bytes(&mut random);
    let temp_path = parent.join(format!(".hushlink-{}.tmp", hex::encode(random)));
    let operation = (|| {
        let mut file = private_open_options().create_new(true).open(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        verify_current(path, expected)?;
        atomic_replace(&temp_path, path, expected.is_some())?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    // Temporary files contain ciphertext only, and are never used as recovery input.
    if operation.is_ok() && temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }
    operation
}

fn verify_current(path: &Path, expected: Option<[u8; 32]>) -> Result<()> {
    match expected {
        Some(expected) => {
            let current = bounded_read(path).map_err(|_| VaultError::Conflict)?;
            if digest(&current) != expected {
                return Err(VaultError::Conflict);
            }
        }
        None if path.try_exists()? => return Err(VaultError::AlreadyExists),
        None => {}
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(temp: &Path, path: &Path, replacing: bool) -> Result<()> {
    if replacing {
        fs::rename(temp, path)?;
    } else {
        fs::hard_link(temp, path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                VaultError::AlreadyExists
            } else {
                VaultError::Io(error)
            }
        })?;
        fs::remove_file(temp)?;
    }
    Ok(())
}

#[cfg(windows)]
fn atomic_replace(temp: &Path, path: &Path, replacing: bool) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let source: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let flags = if replacing {
        MOVEFILE_WRITE_THROUGH | MOVEFILE_REPLACE_EXISTING
    } else {
        MOVEFILE_WRITE_THROUGH
    };
    // SAFETY: both UTF-16 buffers are NUL-terminated and remain alive during the
    // synchronous call. All temporary-file handles are closed. Both paths are
    // in the same directory/volume; COPY_ALLOWED is deliberately absent.
    let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) };
    if result == 0 {
        return Err(VaultError::Io(std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::sync::{Mutex, MutexGuard};

    type TestResult = std::result::Result<(), Box<dyn Error>>;
    const PASSWORD: &str = "Künstliches Passwort 2026 🗝";
    const OTHER_PASSWORD: &str = "Ein neues künstliches Passwort 2026";
    const MARKER: &str = "HUSHLINK_TEST_KLARTEXT_94721_Grüsse_🦊\nZweite Zeile";
    static KDF_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn serial() -> MutexGuard<'static, ()> {
        KDF_TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn profile() -> Profile {
        let mut profile = Profile::new("Künstliche Änne 🦊".to_owned(), [0x31; 32], [0x52; 32]);
        let contact = Contact {
            id: hex::encode([0x73; 32]),
            display_name: "Testkontakt Örn".to_owned(),
            identity_key: [0x73; 32],
            bluetooth_address: "AA:BB:CC:DD:EE:FF".to_owned(),
        };
        profile.messages.push(StoredMessage {
            id: "synthetic-message-1".to_owned(),
            contact_id: contact.id.clone(),
            text: MARKER.to_owned(),
            timestamp_ms: 1_700_000_000_000,
            outgoing: true,
            status: MessageStatus::Received,
        });
        profile.contacts.push(contact);
        profile
    }

    #[test]
    fn roundtrip_save_unicode_and_persisted_files_have_no_plaintext() -> TestResult {
        let _serial = serial();
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("profile.vault");
        let mut vault = Vault::create(&path, PASSWORD, profile())?;
        let initial = fs::read(&path)?;
        vault.profile_mut().messages[0]
            .text
            .push_str("\nNachtrag 🎉");
        vault.profile_mut().settings.inactivity_minutes = 10;
        vault.save()?;
        let saved = fs::read(&path)?;
        assert_ne!(&initial[64..88], &saved[64..88]);
        assert_eq!(&initial[24..64], &saved[24..64]);
        assert_eq!(
            &initial[BASE_HEADER_LEN..HEADER_LEN],
            &saved[BASE_HEADER_LEN..HEADER_LEN]
        );
        for entry in fs::read_dir(directory.path())? {
            let contents = fs::read(entry?.path())?;
            for secret in [
                MARKER.as_bytes(),
                PASSWORD.as_bytes(),
                "Testkontakt Örn".as_bytes(),
                &[0x31; 32],
            ] {
                assert!(
                    !contents
                        .windows(secret.len())
                        .any(|window| window == secret)
                );
            }
        }
        drop(vault);
        let reopened = Vault::open(&path, PASSWORD)?;
        assert_eq!(reopened.profile().display_name, "Künstliche Änne 🦊");
        assert_eq!(reopened.profile().identity_private, [0x31; 32]);
        assert_eq!(reopened.profile().contacts[0].identity_key, [0x73; 32]);
        assert_eq!(
            reopened.profile().messages[0].text,
            format!("{MARKER}\nNachtrag 🎉")
        );
        assert_eq!(
            reopened.profile().messages[0].status,
            MessageStatus::Received
        );
        assert_eq!(reopened.profile().settings.inactivity_minutes, 10);
        Ok(())
    }

    #[test]
    fn wrong_password_and_ciphertext_tampering_are_rejected_without_writes() -> TestResult {
        let _serial = serial();
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("profile.vault");
        drop(Vault::create(&path, PASSWORD, profile())?);
        let original = fs::read(&path)?;
        assert!(matches!(
            Vault::open(&path, "Falsches Passwort 2026"),
            Err(VaultError::Authentication)
        ));
        assert_eq!(fs::read(&path)?, original);
        let mut tampered = original.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x80;
        fs::write(&path, &tampered)?;
        assert!(matches!(
            Vault::open(&path, PASSWORD),
            Err(VaultError::Authentication)
        ));
        assert_eq!(fs::read(&path)?, tampered);
        let mut tampered_header = original;
        tampered_header[64] ^= 1;
        fs::write(&path, &tampered_header)?;
        assert!(matches!(
            Vault::open(&path, PASSWORD),
            Err(VaultError::Authentication)
        ));
        assert_eq!(fs::read(&path)?, tampered_header);
        Ok(())
    }

    #[test]
    fn password_change_rewraps_data_key_and_preserves_history() -> TestResult {
        let _serial = serial();
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("profile.vault");
        let mut vault = Vault::create(&path, PASSWORD, profile())?;
        let old_bytes = fs::read(&path)?;
        assert!(matches!(
            vault.change_password("Wrong password 123", OTHER_PASSWORD),
            Err(VaultError::Authentication)
        ));
        assert_eq!(fs::read(&path)?, old_bytes);
        vault.change_password(PASSWORD, OTHER_PASSWORD)?;
        let new_bytes = fs::read(&path)?;
        assert_ne!(&old_bytes[24..40], &new_bytes[24..40]);
        assert_ne!(&old_bytes[40..64], &new_bytes[40..64]);
        assert_ne!(&old_bytes[64..88], &new_bytes[64..88]);
        assert_ne!(
            &old_bytes[BASE_HEADER_LEN..HEADER_LEN],
            &new_bytes[BASE_HEADER_LEN..HEADER_LEN]
        );
        drop(vault);
        assert!(matches!(
            Vault::open(&path, PASSWORD),
            Err(VaultError::Authentication)
        ));
        let reopened = Vault::open(&path, OTHER_PASSWORD)?;
        assert_eq!(reopened.profile().messages[0].text, MARKER);
        assert_eq!(reopened.profile().identity_private, [0x31; 32]);
        Ok(())
    }

    #[test]
    fn corruption_or_external_changes_are_never_overwritten() -> TestResult {
        let _serial = serial();
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("profile.vault");
        let mut vault = Vault::create(&path, PASSWORD, profile())?;
        let damaged = b"synthetic broken file";
        fs::write(&path, damaged)?;
        vault.profile_mut().messages.clear();
        assert!(matches!(vault.save(), Err(VaultError::Conflict)));
        assert_eq!(fs::read(&path)?, damaged);
        drop(vault);
        assert!(matches!(
            Vault::open(&path, PASSWORD),
            Err(VaultError::Format(_))
        ));
        assert!(matches!(
            Vault::create(&path, PASSWORD, profile()),
            Err(VaultError::AlreadyExists)
        ));
        assert_eq!(fs::read(&path)?, damaged);
        Ok(())
    }

    #[test]
    fn concurrent_instances_cannot_write_the_same_vault() -> TestResult {
        let _serial = serial();
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("profile.vault");
        let vault = Vault::create(&path, PASSWORD, profile())?;
        assert!(matches!(
            Vault::open(&path, PASSWORD),
            Err(VaultError::InUse)
        ));
        assert!(matches!(
            Vault::create(&path, PASSWORD, profile()),
            Err(VaultError::InUse)
        ));
        drop(vault);
        let _reopened = Vault::open(&path, PASSWORD)?;
        Ok(())
    }

    #[test]
    fn malformed_headers_lengths_and_kdf_limits_are_bounded() -> TestResult {
        let mut bytes = vec![0; HEADER_LEN + TAG_LEN];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        bytes[10] = 1;
        bytes[11] = 1;
        bytes[12..16].copy_from_slice(&(64_u32 * 1024).to_le_bytes());
        bytes[16..20].copy_from_slice(&3_u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
        bytes[88..92].copy_from_slice(&(TAG_LEN as u32).to_le_bytes());
        assert!(Envelope::decode(&bytes).is_ok());
        for length in 0..HEADER_LEN + TAG_LEN {
            assert!(Envelope::decode(&bytes[..length]).is_err());
        }
        let mut oversized = bytes.clone();
        oversized[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Envelope::decode(&oversized),
            Err(VaultError::Format(_))
        ));
        let mut excessive_iterations = bytes.clone();
        excessive_iterations[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Envelope::decode(&excessive_iterations),
            Err(VaultError::Format(_))
        ));
        let mut invalid_length = bytes.clone();
        invalid_length[88..92].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Envelope::decode(&invalid_length).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(Envelope::decode(&trailing).is_err());
        Ok(())
    }

    #[test]
    fn oversize_file_is_rejected_before_deserialization() -> TestResult {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("profile.vault");
        let file = File::create(&path)?;
        file.set_len(MAX_VAULT_BYTES as u64 + 1)?;
        drop(file);
        assert!(matches!(
            Vault::open(&path, PASSWORD),
            Err(VaultError::Format(_))
        ));
        assert_eq!(fs::metadata(&path)?.len(), MAX_VAULT_BYTES as u64 + 1);
        Ok(())
    }

    #[test]
    fn profile_bounds_and_password_policy_are_enforced() -> TestResult {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("profile.vault");
        assert!(matches!(
            Vault::create(&path, "short", profile()),
            Err(VaultError::PasswordTooShort)
        ));
        let mut invalid = profile();
        invalid.contacts[0].id = "unbound-key".to_owned();
        assert!(matches!(
            validate_profile(&invalid),
            Err(VaultError::InvalidProfile)
        ));
        invalid = profile();
        invalid.messages[0].text = "x".repeat(MAX_MESSAGE_BYTES + 1);
        assert!(matches!(
            validate_profile(&invalid),
            Err(VaultError::InvalidProfile)
        ));
        invalid = profile();
        for minutes in [0, 121] {
            invalid.settings.inactivity_minutes = minutes;
            assert!(matches!(
                validate_profile(&invalid),
                Err(VaultError::InvalidProfile)
            ));
        }
        for minutes in [1, 120] {
            invalid.settings.inactivity_minutes = minutes;
            assert!(validate_profile(&invalid).is_ok());
        }
        for name in [
            "Nul\0Name".to_owned(),
            "Tab\tName".to_owned(),
            "x".repeat(81),
            "漢".repeat(30),
        ] {
            invalid = profile();
            invalid.display_name = name.clone();
            assert!(matches!(
                validate_profile(&invalid),
                Err(VaultError::InvalidProfile)
            ));
            invalid = profile();
            invalid.contacts[0].display_name = name;
            assert!(matches!(
                validate_profile(&invalid),
                Err(VaultError::InvalidProfile)
            ));
        }
        for name in ["x".repeat(80), "漢".repeat(26)] {
            invalid = profile();
            invalid.display_name = name.clone();
            invalid.contacts[0].display_name = name;
            assert!(validate_profile(&invalid).is_ok());
        }
        assert!(!path.exists());
        Ok(())
    }
}
