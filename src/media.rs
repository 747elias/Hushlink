//! Bounded in-memory image attachments and full security-code QR comparison.
#[cfg(windows)]
use std::{
    io::Read,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use std::{
    io::{self, Write},
    sync::atomic::AtomicBool,
};
#[cfg(windows)]
use zeroize::Zeroize;
use zeroize::Zeroizing;
const MAX_PIXELS: usize = 16_000_000;
struct BoundedPng(Zeroizing<Vec<u8>>);
impl Write for BoundedPng {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > crate::vault::MAX_ATTACHMENT_BYTES {
            return Err(io::Error::other("Bild ist zu gross"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub fn encode_rgba(width: usize, height: usize, rgba: &[u8]) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    anyhow::ensure!(
        width > 0
            && height > 0
            && width <= 8192
            && height <= 8192
            && width.saturating_mul(height) <= MAX_PIXELS
            && rgba.len() == width.saturating_mul(height).saturating_mul(4),
        "Das Bild ist zu gross oder beschädigt (höchstens 16 Megapixel)."
    );
    let mut output = BoundedPng(Zeroizing::new(Vec::new()));
    {
        let mut encoder = png::Encoder::new(&mut output, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgba)?;
        writer.finish()?;
    }
    Ok(output.0)
}
#[cfg(windows)]
pub fn clipboard_image() -> anyhow::Result<Zeroizing<Vec<u8>>> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|_| anyhow::anyhow!("Zwischenablage ist nicht verfügbar."))?;
    let image=clipboard.get_image().map_err(|_|anyhow::anyhow!("Kein Bild in der Zwischenablage. Kopiere einen Screenshot oder ein Bild und versuche es erneut."))?;
    let width = image.width;
    let height = image.height;
    let rgba = Zeroizing::new(image.bytes.into_owned());
    encode_rgba(width, height, &rgba)
}
#[cfg(not(windows))]
pub fn clipboard_image() -> anyhow::Result<Zeroizing<Vec<u8>>> {
    anyhow::bail!(
        "Zwischenablage-Bilder sind in diesem Entwicklungsbuild nur unter Windows verfügbar."
    )
}

pub fn qr_payload(fingerprint: &str) -> anyhow::Result<String> {
    let code: String = fingerprint.chars().filter(|c| !c.is_whitespace()).collect();
    anyhow::ensure!(
        code.len() == 64 && code.bytes().all(|b| b.is_ascii_hexdigit()),
        "Ungültiger Sicherheitscode."
    );
    Ok(format!("HUSHLINK:VERIFY:4:{}", code.to_ascii_uppercase()))
}
pub fn matches_qr(payload: &[u8], fingerprint: &str) -> bool {
    qr_payload(fingerprint).is_ok_and(|expected| expected.as_bytes() == payload)
}

/// Run a disposable instance of this same executable. A stalled camera driver
/// cannot keep capturing after lock: the parent kills/waits the owned helper.
#[cfg(windows)]
pub fn scan_camera(index: u32, cancelled: &AtomicBool) -> anyhow::Result<Vec<u8>> {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::*,
    };
    struct Job(HANDLE);
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    anyhow::ensure!(
        !cancelled.load(Ordering::Acquire),
        "Kameraprüfung wurde abgebrochen."
    );
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    anyhow::ensure!(
        !job.is_null(),
        "Kamera konnte nicht sicher gestartet werden."
    );
    let job = Job(job);
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    anyhow::ensure!(
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        } != 0,
        "Kamera konnte nicht sicher gestartet werden."
    );

    anyhow::ensure!(index < 16, "Ungültige Kameraauswahl.");
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .arg("--hushlink-scan-qr")
        .arg(index.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000)
        .spawn()?;
    if unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle()) } == 0 {
        let _ = child.kill();
        let _ = child.wait();
        anyhow::bail!("Kamera konnte nicht sicher gestartet werden.");
    }
    let start = Instant::now();
    loop {
        if cancelled.load(Ordering::Acquire) || start.elapsed() > Duration::from_secs(35) {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Kameraprüfung abgebrochen oder abgelaufen.");
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                anyhow::ensure!(
                    status.success(),
                    "Die Kamera konnte keinen Sicherheitscode lesen. Windows-Kameraberechtigung für Desktop-Apps prüfen, gegebenenfalls Media Feature Pack installieren oder eine andere Kamera wählen."
                );
                let mut bytes = Vec::new();
                child
                    .stdout
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("Kameraprüfung fehlgeschlagen."))?
                    .take(257)
                    .read_to_end(&mut bytes)?;
                anyhow::ensure!(bytes.len() <= 256, "Ungültiger QR-Inhalt.");
                return Ok(bytes);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
    }
}
#[cfg(not(windows))]
pub fn scan_camera(_index: u32, _cancelled: &AtomicBool) -> anyhow::Result<Vec<u8>> {
    anyhow::bail!("Kameraprüfung ist nur unter Windows verfügbar.")
}
#[cfg(windows)]
fn camera_helper(index: u32) -> anyhow::Result<Vec<u8>> {
    use nokhwa::{
        Camera,
        pixel_format::LumaFormat,
        utils::{
            ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat,
            RequestedFormatType,
        },
    };
    anyhow::ensure!(index < 16, "Kameraauswahl ungültig");
    // Prefer uncompressed formats: no JPEG decoder/runtime dependency is needed.
    let mut camera = None;
    for format in [
        FrameFormat::YUYV,
        FrameFormat::NV12,
        FrameFormat::RAWRGB,
        FrameFormat::GRAY,
    ] {
        let request = RequestedFormat::new::<LumaFormat>(RequestedFormatType::Closest(
            CameraFormat::new_from(640, 480, format, 15),
        ));
        if let Ok(candidate) = Camera::with_backend(
            CameraIndex::Index(index),
            request,
            ApiBackend::MediaFoundation,
        ) {
            let size = candidate.resolution();
            if size.width() > 0
                && size.height() > 0
                && size.width() <= 1920
                && size.height() <= 1080
            {
                camera = Some(candidate);
                break;
            }
        }
    }
    let mut camera = camera.ok_or_else(|| anyhow::anyhow!("Kamera nicht verfügbar"))?;
    camera.open_stream()?;
    let start = Instant::now();
    let mut decoder = quircs::Quirc::default();
    while start.elapsed() < Duration::from_secs(30) {
        let frame = camera.frame()?;
        let image = frame.decode_image::<LumaFormat>()?;
        let width = image.width() as usize;
        let height = image.height() as usize;
        let gray = Zeroizing::new(image.into_raw());
        if width > 1920 || height > 1080 || width.saturating_mul(height) != gray.len() {
            break;
        }
        for code in decoder.identify(width, height, &gray).flatten() {
            if let Ok(decoded) = code.decode()
                && decoded.payload.len() == b"HUSHLINK:VERIFY:4:".len() + 64
                && decoded.payload.starts_with(b"HUSHLINK:VERIFY:4:")
            {
                camera.stop_stream()?;
                return Ok(decoded.payload);
            }
        }
    }
    camera.stop_stream()?;
    anyhow::bail!("Kein Sicherheitscode gefunden")
}
/// Called before GUI/tray initialization. No files or diagnostic output.
pub fn run_camera_helper_if_requested() -> bool {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--hushlink-scan-qr") {
        return false;
    }
    #[cfg(windows)]
    {
        let result = args
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|i| *i < 16)
            .ok_or_else(|| anyhow::anyhow!("Ungültige Kamera"))
            .and_then(camera_helper);
        if let Ok(mut bytes) = result {
            let okay = std::io::stdout().write_all(&bytes).is_ok();
            bytes.zeroize();
            std::process::exit(if okay { 0 } else { 1 });
        }
    }
    std::process::exit(1)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipboard_encoding_is_bounded_lossless_png_and_never_a_temp_file() -> anyhow::Result<()> {
        let pixels = [255, 0, 0, 255, 0, 255, 0, 255];
        let bytes = encode_rgba(2, 1, &pixels)?;
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let decoder = png::Decoder::new(std::io::Cursor::new(&*bytes));
        let mut reader = decoder.read_info()?;
        let mut decoded = vec![
            0;
            reader
                .output_buffer_size()
                .ok_or_else(|| anyhow::anyhow!("size"))?
        ];
        let info = reader.next_frame(&mut decoded)?;
        assert_eq!(&decoded[..info.buffer_size()], &pixels);
        for (w, h, data) in [
            (0, 1, &pixels[..]),
            (8193, 1, &pixels[..]),
            (4001, 4000, &pixels[..]),
            (2, 2, &pixels[..]),
        ] {
            assert!(encode_rgba(w, h, data).is_err());
        }
        Ok(())
    }
    #[test]
    fn qr_binds_the_full_canonical_code_and_rejects_other_contexts() -> anyhow::Result<()> {
        let code = crate::protocol::security_fingerprint(&[7; 32], &[9; 32]);
        let payload = qr_payload(&code)?;
        assert!(matches_qr(payload.as_bytes(), &code));
        assert!(!matches_qr(
            payload.as_bytes(),
            &crate::protocol::security_fingerprint(&[7; 32], &[8; 32])
        ));
        assert!(!matches_qr(payload.replace(":4:", ":3:").as_bytes(), &code));
        assert!(!matches_qr(b"https://example.org", &code));
        let qr = qrcode::QrCode::new(payload.as_bytes())?;
        let scale = 5;
        let width = (qr.width() + 8) * scale;
        let mut gray = vec![255; width * width];
        for y in 0..qr.width() {
            for x in 0..qr.width() {
                if qr[(x, y)] == qrcode::Color::Dark {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            gray[((y + 4) * scale + dy) * width + (x + 4) * scale + dx] = 0;
                        }
                    }
                }
            }
        }
        let mut decoder = quircs::Quirc::default();
        let decoded = decoder
            .identify(width, width, &gray)
            .next()
            .ok_or_else(|| anyhow::anyhow!("QR missing"))??
            .decode()?;
        assert!(matches_qr(&decoded.payload, &code));
        Ok(())
    }
}
