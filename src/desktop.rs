//! Per-user Windows integration. The tray has its own native message loop, so a
//! hidden eframe viewport can still be restored without waiting for a UI frame.

use anyhow::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopEvent {
    Show,
    Lock,
    CheckUpdates,
    Exit,
}

#[cfg(windows)]
#[path = "desktop/windows.rs"]
mod windows;
#[cfg(windows)]
pub use windows::{
    DesktopIntegration, InstanceGuard, acquire_instance, set_startup, startup_enabled,
};

#[cfg(not(windows))]
pub struct InstanceGuard;

#[cfg(not(windows))]
pub fn acquire_instance() -> Result<Option<InstanceGuard>> {
    Ok(Some(InstanceGuard))
}

#[cfg(not(windows))]
pub struct DesktopIntegration;

#[cfg(not(windows))]
impl DesktopIntegration {
    pub fn new() -> Result<Self> {
        anyhow::bail!("Das Infobereich-Symbol ist nur unter Windows verfügbar.")
    }

    pub fn try_event(&self) -> Option<DesktopEvent> {
        None
    }

    pub fn set_waker(&self, _wake: impl Fn() + Send + Sync + 'static) {}

    pub fn is_available(&self) -> bool {
        false
    }
}

#[cfg(not(windows))]
pub fn startup_enabled() -> Result<bool> {
    Ok(false)
}

#[cfg(not(windows))]
pub fn set_startup(_enabled: bool) -> Result<()> {
    anyhow::bail!("Der Windows-Autostart ist auf diesem System nicht verfügbar.")
}

// Keep quoting independent of lossy UTF-8 conversion: Windows paths may contain
// UTF-16 that cannot be represented by Rust's String. Run values are limited to
// 260 characters by the documented Windows Run-key contract.
#[cfg(any(windows, test))]
fn startup_command_units(executable: &[u16]) -> Result<Vec<u16>> {
    anyhow::ensure!(
        !executable.is_empty() && !executable.iter().any(|c| matches!(c, 0 | 34)),
        "Der Programmpfad ist für den Windows-Autostart ungültig."
    );
    let mut command = Vec::with_capacity(executable.len() + 10);
    command.push(b'"' as u16);
    command.extend_from_slice(executable);
    command.extend("\" --tray".encode_utf16());
    anyhow::ensure!(
        command.len() <= 260,
        "Der Programmpfad ist für den Windows-Autostart zu lang. Installiere Hushlink in einem kürzeren Pfad."
    );
    command.push(0);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_quotes_the_executable_and_preserves_unicode() -> Result<()> {
        let path: Vec<u16> = r"C:\Benutzer\Jörg 😀\Hushlink.exe".encode_utf16().collect();
        let command = startup_command_units(&path)?;
        assert_eq!(
            String::from_utf16(&command[..command.len() - 1])?,
            "\"C:\\Benutzer\\Jörg 😀\\Hushlink.exe\" --tray"
        );
        assert_eq!(command.last(), Some(&0));
        // Unpaired UTF-16 must remain unchanged, rather than silently replacing
        // a character and registering a different executable path.
        assert_eq!(startup_command_units(&[0xd800])?[1], 0xd800);
        Ok(())
    }

    #[test]
    fn startup_rejects_embedded_delimiters_and_run_key_overflow() -> Result<()> {
        for path in [vec![], vec![0], vec![b'"' as u16]] {
            assert!(startup_command_units(&path).is_err());
        }
        assert_eq!(startup_command_units(&vec![b'a' as u16; 251])?.len(), 261);
        assert!(startup_command_units(&vec![b'a' as u16; 252]).is_err());
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_never_reports_a_working_tray_or_autostart() -> Result<()> {
        assert!(DesktopIntegration::new().is_err());
        assert!(!startup_enabled()?);
        assert!(set_startup(true).is_err());
        Ok(())
    }
}
