//! Explicit non-Windows stubs for testing the portable cryptographic core.
use super::*;
use std::{
    io::{self, Read, Write},
    time::Duration,
};

fn unavailable() -> anyhow::Error {
    anyhow::anyhow!(
        "Hushlink benötigt Windows 11 mit einem Bluetooth-Classic/RFCOMM-Adapter. Dieser Build bietet keinen Ersatztransport."
    )
}

pub fn available() -> anyhow::Result<Availability> {
    Err(unavailable())
}
pub fn discover(cancel: &AtomicBool) -> anyhow::Result<Vec<Device>> {
    cancelled(cancel)?;
    Err(unavailable())
}
pub fn connect(address: &str, cancel: &AtomicBool) -> anyhow::Result<Stream> {
    cancelled(cancel)?;
    parse_address(address)?;
    Err(unavailable())
}
pub fn open_bluetooth_settings() -> anyhow::Result<()> {
    Err(unavailable())
}

pub struct Listener;
impl Listener {
    pub fn bind() -> anyhow::Result<Self> {
        Err(unavailable())
    }
    pub fn accept(&self, cancel: &AtomicBool) -> anyhow::Result<(Stream, Device)> {
        cancelled(cancel)?;
        Err(unavailable())
    }
}

pub struct Stream;
#[derive(Clone)]
pub struct CancelHandle;
impl CancelHandle {
    pub fn cancel(&self) {}
}
impl Stream {
    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle
    }
    pub fn try_clone(&self) -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::Unsupported, unavailable()))
    }
    pub fn set_read_timeout(&self, _timeout: Duration) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, unavailable()))
    }
}
impl Read for Stream {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Unsupported, unavailable()))
    }
}
impl Write for Stream {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Unsupported, unavailable()))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn non_windows_never_silently_uses_network_transport() {
        assert!(available().is_err());
        assert!(Listener::bind().is_err());
        assert!(discover(&AtomicBool::new(false)).is_err());
        assert!(connect("01:23:45:67:89:AB", &AtomicBool::new(false)).is_err());
        assert_eq!(format_address(0x0123456789AB), "01:23:45:67:89:AB");
    }
}
