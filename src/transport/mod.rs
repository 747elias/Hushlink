//! Bluetooth Classic RFCOMM only. No IP fallback exists.
//!
//! All potentially blocking functions belong on a worker thread. Dropping a
//! listener unregisters its SDP service; discovery is an explicit operation.
use std::sync::atomic::{AtomicBool, Ordering};

pub const SERVICE_UUID: &str = "7d2ea28a-f7bd-4a6d-b1f4-2cf2f653cc0e";
pub const CONNECT_TIMEOUT_SECS: u64 = 45;
pub const IO_TIMEOUT_SECS: u64 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// OS-provided Bluetooth label, never an identity proof.
    pub name: String,
    /// Six-byte Bluetooth address, normalized as XX:XX:XX:XX:XX:XX.
    pub address: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Availability {
    pub radio_name: String,
    pub connectable: bool,
    pub discoverable: bool,
}

fn cancelled(cancel: &AtomicBool) -> anyhow::Result<()> {
    if cancel.load(Ordering::Acquire) {
        anyhow::bail!("Bluetooth-Vorgang abgebrochen.");
    }
    Ok(())
}

fn parse_address(value: &str) -> anyhow::Result<u64> {
    let compact = if value.len() == 17 {
        if !value.as_bytes().iter().enumerate().all(|(i, c)| {
            if i % 3 == 2 {
                *c == b':'
            } else {
                c.is_ascii_hexdigit()
            }
        }) {
            anyhow::bail!("Ungültige Bluetooth-Adresse.");
        }
        value.replace(':', "")
    } else if value.len() == 12 && value.bytes().all(|c| c.is_ascii_hexdigit()) {
        value.to_owned()
    } else {
        anyhow::bail!("Ungültige Bluetooth-Adresse.");
    };
    let address = u64::from_str_radix(&compact, 16)?;
    anyhow::ensure!(address != 0, "Ungültige Bluetooth-Adresse.");
    Ok(address)
}

#[cfg(any(windows, test))]
fn format_address(address: u64) -> String {
    (0..6)
        .rev()
        .map(|n| format!("{:02X}", (address >> (n * 8)) & 0xff))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
pub use unsupported::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_parser_is_bounded_and_strict() -> anyhow::Result<()> {
        let address = parse_address("01:23:45:67:89:aB")?;
        assert_eq!(format_address(address), "01:23:45:67:89:AB");
        assert_eq!(parse_address("0123456789AB")?, address);
        for invalid in [
            "",
            "00:00:00:00:00:00",
            "1:23:45:67:89:ab",
            "01-23-45-67-89-ab",
            "01:23:45:67:89:GG",
            "1".repeat(1000).as_str(),
        ] {
            assert!(parse_address(invalid).is_err());
        }
        Ok(())
    }

    #[test]
    fn cancellation_is_explicit() {
        let flag = AtomicBool::new(false);
        assert!(cancelled(&flag).is_ok());
        flag.store(true, Ordering::Release);
        assert!(cancelled(&flag).is_err());
    }
}
