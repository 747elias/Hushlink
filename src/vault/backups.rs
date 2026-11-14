//! Backups are authenticated ciphertext copies, never plaintext snapshots.
use super::*;
impl Vault {
    pub fn backup_to_new(&self, destination: impl AsRef<Path>) -> Result<()> {
        if self.relocation_uncertain {
            return Err(VaultError::Conflict);
        }
        let bytes = bounded_read(&self.path)?;
        if digest(&bytes) != self.expected_digest {
            return Err(VaultError::Conflict);
        }
        let _destination_lock = lock_file(destination.as_ref())?;
        atomic_write(destination.as_ref(), &bytes, None)
    }
}
pub fn create(
    vault: &Vault,
    folder: &Path,
    id: &str,
    now: u64,
    retention: u8,
) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        id.len() == 32 && hex::decode(id).is_ok() && (1..=20).contains(&retention),
        "Ungültige Sicherungseinstellungen."
    );
    anyhow::ensure!(folder.is_dir(), "Der Sicherungsordner ist nicht verfügbar.");
    let mut random = [0; 8];
    OsRng.try_fill_bytes(&mut random)?;
    let destination = folder.join(format!(
        "HushlinkBackup-{id}-{now:020}-{}.vault",
        hex::encode(random)
    ));
    vault.backup_to_new(&destination)?;
    prune(folder, id, retention, &destination)?;
    Ok(destination)
}
fn owned(name: &str, id: &str) -> bool {
    let prefix = format!("HushlinkBackup-{id}-");
    let Some(suffix) = name
        .strip_prefix(&prefix)
        .and_then(|n| n.strip_suffix(".vault"))
    else {
        return false;
    };
    suffix.len() == 37
        && suffix.as_bytes()[20] == b'-'
        && suffix.as_bytes()[..20].iter().all(|c| c.is_ascii_digit())
        && suffix.as_bytes()[21..]
            .iter()
            .all(|c| c.is_ascii_hexdigit())
}
fn prune(folder: &Path, id: &str, retention: u8, keep: &Path) -> anyhow::Result<()> {
    let mut files = Vec::new();
    for (index, entry) in fs::read_dir(folder)?.enumerate() {
        anyhow::ensure!(
            index < 10_000,
            "Zu viele Dateien im Sicherungsordner. Bitte einen eigenen Ordner wählen."
        );
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if owned(name, id) && entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    let remove = files.len().saturating_sub(usize::from(retention));
    for file in files.into_iter().filter(|file| file != keep).take(remove) {
        // Only owned, regular ciphertext files. Never follow a symlink or recurse.
        if fs::symlink_metadata(&file)?.file_type().is_file() {
            fs::remove_file(file)?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_copies_restore_and_pruning_preserves_foreign_files() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let source = directory.path().join("profile.vault");
        let folder = directory.path().join("backups");
        fs::create_dir(&folder)?;
        let profile = Profile::new("SYNTHETIC_BACKUP_IDENTITY_04".into(), [9; 32], [8; 32]);
        let mut vault = Vault::create(&source, "synthetic-backup-password", profile)?;
        let id = hex::encode([6; 16]);
        let foreign = folder.join("personal-document.txt");
        fs::write(&foreign, b"synthetic foreign file")?;
        let a = create(&vault, &folder, &id, 1, 2)?;
        let b = create(&vault, &folder, &id, 2, 2)?;
        let c = create(&vault, &folder, &id, 3, 2)?;
        assert!(!a.exists() && b.exists() && c.exists() && foreign.exists());
        let backwards = create(&vault, &folder, &id, 0, 2)?;
        assert!(
            backwards.exists(),
            "a newly written backup must survive a clock rollback"
        );
        let bytes = fs::read(&c)?;
        assert!(
            !bytes
                .windows(28)
                .any(|w| w == b"SYNTHETIC_BACKUP_IDENTITY_04")
        );
        let destination = directory.path().join("restored.vault");
        let backup = Vault::open(&c, "synthetic-backup-password")?;
        backup.backup_to_new(&destination)?;
        assert!(backup.backup_to_new(&destination).is_err());
        drop(backup);
        let restored = Vault::open(&destination, "synthetic-backup-password")?;
        assert_eq!(
            restored.profile().display_name,
            "SYNTHETIC_BACKUP_IDENTITY_04"
        );
        drop(restored);
        assert!(Vault::open(&c, "wrong-password").is_err());
        vault.change_password("synthetic-backup-password", "synthetic-new-password")?;
        let d = create(&vault, &folder, &id, 4, 2)?;
        assert!(Vault::open(&d, "synthetic-backup-password").is_err());
        assert!(Vault::open(&c, "synthetic-backup-password").is_ok());
        assert!(!owned("HushlinkBackup-../../profile.vault", &id));
        Ok(())
    }
}
