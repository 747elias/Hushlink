//! Non-secret bootstrap data needed before a vault can be unlocked.
//!
//! The selected path and desktop preferences deliberately live outside the
//! encrypted vault. This file never contains identities, contacts or messages.

use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::vault::{self, Result, Vault, VaultError};

const CONFIG_VERSION: u16 = 1;
const MAX_CONFIG_BYTES: u64 = 32 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopPreferences {
    pub start_with_windows: bool,
    pub minimize_to_tray: bool,
    pub automatic_updates: bool,
}

impl Default for DesktopPreferences {
    fn default() -> Self {
        Self {
            start_with_windows: true,
            minimize_to_tray: true,
            automatic_updates: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfiguration {
    version: u16,
    pub vault_path: PathBuf,
    pub preferences: DesktopPreferences,
}

impl StorageConfiguration {
    fn new(default_path: &Path) -> Result<Self> {
        validate_path(default_path)?;
        Ok(Self {
            version: CONFIG_VERSION,
            vault_path: default_path.to_path_buf(),
            preferences: DesktopPreferences::default(),
        })
    }
}

/// Load the selected location. A missing bootstrap retains the v0.1 default.
/// A damaged bootstrap is an error, never an excuse to create another vault.
pub fn load_config(default_path: &Path) -> Result<StorageConfiguration> {
    read(default_path)
}

pub fn load(default_path: &Path) -> Result<PathBuf> {
    Ok(read(default_path)?.vault_path)
}

/// Preferences must be available while locked, especially an update opt-out.
pub fn load_preferences(default_path: &Path) -> Result<DesktopPreferences> {
    Ok(read(default_path)?.preferences)
}

/// Explicitly choose a vault, without opening, changing or creating that vault.
/// This also provides deliberate recovery from a broken location configuration.
pub fn save(default_path: &Path, selected_path: &Path) -> Result<()> {
    validate_path(selected_path)?;
    update(default_path, true, |bootstrap| {
        bootstrap.vault_path = selected_path.to_path_buf();
    })
}

pub fn save_preferences(
    default_path: &Path,
    preferences: impl std::borrow::Borrow<DesktopPreferences>,
) -> Result<()> {
    let preferences = *preferences.borrow();
    // Do not silently repair corruption here: that could lose an existing
    // location or turn updates back on without an explicit user decision.
    update(default_path, false, |bootstrap| {
        bootstrap.preferences = preferences;
    })
}

/// Copy authenticated ciphertext, commit the locator, then remove the old file.
/// An uncertain locator commit preserves both copies and blocks further writes
/// until reopening. Cleanup failure uses the already committed new location.
pub fn relocate(vault: &mut Vault, default_path: &Path, destination: &Path) -> Result<()> {
    validate_path(destination)?;
    vault.relocate_with(destination, |path| save(default_path, path))
}

fn config_path(default_path: &Path) -> Result<PathBuf> {
    validate_path(default_path)?;
    let parent = default_path
        .parent()
        .ok_or(VaultError::Configuration("ungültiger Standardpfad"))?;
    Ok(parent.join("hushlink-settings.json"))
}

fn validate_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path.file_name().is_none()
        || path.as_os_str().len() > 16_384
        || path.to_string_lossy().contains('\0')
    {
        return Err(VaultError::Configuration("ungültiger Tresorpfad"));
    }
    Ok(())
}

fn read(default_path: &Path) -> Result<StorageConfiguration> {
    let path = config_path(default_path)?;
    match read_bytes(&path)? {
        Some(bytes) => decode(&bytes),
        None => StorageConfiguration::new(default_path),
    }
}

fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if file.metadata()?.len() > MAX_CONFIG_BYTES {
        return Err(VaultError::Configuration("Konfiguration ist zu gross"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(VaultError::Configuration("Konfiguration ist zu gross"));
    }
    Ok(Some(bytes))
}

fn decode(bytes: &[u8]) -> Result<StorageConfiguration> {
    let bootstrap: StorageConfiguration = serde_json::from_slice(bytes)
        .map_err(|_| VaultError::Configuration("ungültiger Inhalt"))?;
    if bootstrap.version != CONFIG_VERSION {
        return Err(VaultError::Configuration("unbekannte Version"));
    }
    validate_path(&bootstrap.vault_path)?;
    Ok(bootstrap)
}

fn update(
    default_path: &Path,
    repair: bool,
    change: impl FnOnce(&mut StorageConfiguration),
) -> Result<()> {
    let path = config_path(default_path)?;
    let parent = path
        .parent()
        .ok_or(VaultError::Configuration("ungültiger Konfigurationspfad"))?;
    fs::create_dir_all(parent)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // Serialize modifications so preference writes preserve the selected vault.
    let _lock = options.open(parent.join("hushlink-settings.lock"))?;
    _lock.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => VaultError::Configuration(
            "App-Konfiguration wird gerade von einer anderen Instanz geändert",
        ),
        std::fs::TryLockError::Error(error) => VaultError::Io(error),
    })?;
    let previous = read_bytes(&path)?;
    let mut bootstrap = match previous.as_deref().map(decode).transpose() {
        Ok(Some(bootstrap)) => bootstrap,
        Ok(None) => StorageConfiguration::new(default_path)?,
        Err(_) if repair => {
            let mut recovered = StorageConfiguration::new(default_path)?;
            // An unreadable opt-out must not turn on network access or autostart.
            recovered.preferences = DesktopPreferences {
                start_with_windows: false,
                minimize_to_tray: false,
                automatic_updates: false,
            };
            recovered
        }
        Err(error) => return Err(error),
    };
    change(&mut bootstrap);
    let bytes = serde_json::to_vec(&bootstrap)
        .map_err(|_| VaultError::Configuration("Pfad kann nicht gespeichert werden"))?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(VaultError::Configuration("Konfiguration ist zu gross"));
    }
    let expected = previous
        .as_deref()
        .map(|bytes| Sha256::digest(bytes).into());
    vault::atomic_write(&path, &bytes, expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_bootstrap_preserves_legacy_default_without_writing() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let default = directory.path().join("profile.vault");
        assert_eq!(load(&default)?, default);
        assert_eq!(load_preferences(&default)?, DesktopPreferences::default());
        assert_eq!(fs::read_dir(directory.path())?.count(), 0);
        Ok(())
    }

    #[test]
    fn path_and_opt_out_survive_separate_atomic_updates() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let default = directory.path().join("app/profile.vault");
        let selected = directory.path().join("eigene Daten/Änne 🔒.vault");
        let opted_out = DesktopPreferences {
            start_with_windows: false,
            minimize_to_tray: true,
            automatic_updates: false,
        };
        save_preferences(&default, opted_out)?;
        save(&default, &selected)?;
        assert_eq!(load(&default)?, selected);
        assert_eq!(load_preferences(&default)?, opted_out);
        assert!(!selected.exists());
        let bytes = fs::read(config_path(&default)?)?;
        let json: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| VaultError::Configuration("synthetischer Test"))?;
        assert_eq!(json.as_object().map(|object| object.len()), Some(3));
        assert!(json.get("version").is_some());
        assert!(json.get("vault_path").is_some());
        assert!(json.get("preferences").is_some());
        assert!(!bytes.windows(8).any(|window| window == b"password"));
        Ok(())
    }

    #[test]
    fn damaged_unknown_or_oversized_config_fails_without_losing_vault() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let default = directory.path().join("profile.vault");
        let selected = directory.path().join("other.vault");
        fs::write(&default, b"synthetic existing encrypted vault")?;
        for bytes in [
            b"broken config".to_vec(),
            br#"{"version":999,"vault_path":"/abs.vault","preferences":{"start_with_windows":false,"minimize_to_tray":true,"automatic_updates":false}}"#.to_vec(),
            vec![b' '; MAX_CONFIG_BYTES as usize + 1],
        ] {
            fs::write(config_path(&default)?, &bytes)?;
            assert!(load(&default).is_err());
            assert!(save_preferences(&default, DesktopPreferences::default()).is_err());
            assert_eq!(fs::read(config_path(&default)?)?, bytes);
            assert_eq!(fs::read(&default)?, b"synthetic existing encrypted vault");
        }
        // Oversized files require deliberate removal rather than unbounded reads.
        fs::write(config_path(&default)?, b"broken config")?;
        save(&default, &selected)?;
        assert_eq!(load(&default)?, selected);
        assert!(!load_preferences(&default)?.automatic_updates);
        assert!(!load_preferences(&default)?.start_with_windows);
        assert_eq!(fs::read(&default)?, b"synthetic existing encrypted vault");
        assert!(save(&default, Path::new("relative.vault")).is_err());
        assert_eq!(load(&default)?, selected);
        Ok(())
    }

    #[test]
    fn relocation_updates_locator_and_preserves_update_opt_out() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let default = directory.path().join("app/profile.vault");
        let destination = directory.path().join("selected/alles.vault");
        let preferences = DesktopPreferences {
            start_with_windows: false,
            minimize_to_tray: true,
            automatic_updates: false,
        };
        save_preferences(&default, preferences)?;
        let profile = vault::Profile::new("Synthetisches Profil".into(), [0x14; 32], [0x47; 32]);
        let mut vault = Vault::create(&default, "Künstliches Testpasswort 2026", profile)?;
        relocate(&mut vault, &default, &destination)?;
        assert_eq!(load(&default)?, destination);
        assert_eq!(load_preferences(&default)?, preferences);
        assert_eq!(vault.path(), destination);
        assert!(!default.exists());
        let config = fs::read(config_path(&default)?)?;
        let secret = b"Synthetisches Profil";
        assert!(!config.windows(secret.len()).any(|bytes| bytes == secret));
        drop(vault);
        assert_eq!(
            Vault::open(&destination, "Künstliches Testpasswort 2026")?
                .profile()
                .identity_private,
            [0x14; 32]
        );
        Ok(())
    }
}
