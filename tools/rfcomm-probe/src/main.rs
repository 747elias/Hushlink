//! Fixed synthetic bytes only. This probe is not an encrypted chat application.
use anyhow::{Context, Result, bail, ensure};
use hushlink_rfcomm_probe::transport;
use std::{
    io::{Read, Write},
    sync::atomic::AtomicBool,
};

const TEST_BYTES: &[u8] = b"HUSHLINK-RFCOMM-PROBE-V1";

fn main() -> Result<()> {
    let cancel = AtomicBool::new(false);
    match std::env::args().nth(1).as_deref() {
        Some("status") => {
            let state = transport::available()?;
            println!(
                "Adapter: {}; incoming: {}; discoverable: {}",
                state.radio_name, state.connectable, state.discoverable
            );
            println!("Service UUID: {}", transport::SERVICE_UUID);
        }
        Some("scan") => {
            for device in transport::discover(&cancel)? {
                println!(
                    "{} {} (Hushlink service checked upon connection)",
                    device.address, device.name
                );
            }
        }
        Some("listen") => {
            let listener = transport::Listener::bind()?;
            println!("Service registered. Waiting for one connection; stop with Ctrl+C.");
            let (mut stream, peer) = listener.accept(&cancel)?;
            let mut received = vec![0; TEST_BYTES.len()];
            stream
                .read_exact(&mut received)
                .context("Receive fixed probe bytes")?;
            ensure!(received == TEST_BYTES, "Unexpected probe bytes");
            stream.write_all(TEST_BYTES)?;
            println!(
                "Received and echoed synthetic probe bytes from {}",
                peer.address
            );
        }
        Some("connect") => {
            let address = std::env::args()
                .nth(2)
                .context("Missing Bluetooth address")?;
            let mut stream = transport::connect(&address, &cancel)?;
            stream.write_all(TEST_BYTES)?;
            let mut received = vec![0; TEST_BYTES.len()];
            stream
                .read_exact(&mut received)
                .context("Receive probe echo")?;
            ensure!(received == TEST_BYTES, "Unexpected probe echo");
            println!("Two-way RFCOMM probe passed with {address}");
        }
        Some("settings") => transport::open_bluetooth_settings()?,
        _ => bail!(
            "Usage: hushlink-rfcomm-probe status | scan | listen | connect XX:XX:XX:XX:XX:XX | settings"
        ),
    }
    Ok(())
}
