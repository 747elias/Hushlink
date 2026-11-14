//! Optional vault-key wrapping by the Windows Hello (Passport) KSP.
//! There is deliberately no UserConsentVerifier/DPAPI or software-KSP fallback.
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub version: u16,
    pub key_id: String,
    pub wrapped_key: Vec<u8>,
}
impl Slot {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.version == 1
                && self.key_id.len() == 32
                && self.key_id.bytes().all(|c| c.is_ascii_hexdigit())
                && self.wrapped_key.len() == 256,
            "Ungültiger Windows-Hello-Schlüsselplatz."
        );
        Ok(())
    }
}
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{enroll, remove, unwrap};
#[cfg(not(windows))]
pub fn enroll(_key: &[u8; 32]) -> anyhow::Result<Slot> {
    anyhow::bail!("Windows Hello ist nur unter Windows verfügbar.")
}
#[cfg(not(windows))]
pub fn unwrap(_slot: &Slot) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    anyhow::bail!(
        "Windows Hello ist auf diesem Gerät nicht verfügbar. Bitte das Passwort verwenden."
    )
}
#[cfg(not(windows))]
pub fn remove(_slot: &Slot) -> anyhow::Result<()> {
    anyhow::bail!(
        "Der Windows-Hello-Schlüssel kann nur auf dem ursprünglichen Windows-Benutzerkonto entfernt werden."
    )
}
