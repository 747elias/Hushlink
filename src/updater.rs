//! Public-release updater. This module never reads a vault or starts an installer
//! without an explicit call to `launch_installer` from the desktop UI.

use anyhow::{Context, Result, bail, ensure};
use rand::{RngCore, rngs::OsRng};
use reqwest::{Url, blocking::Client, redirect::Policy};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

const API_URL: &str = "https://api.github.com/repos/747elias/Hushlink/releases?per_page=20";
const RELEASE_ROOT: &str = "https://github.com/747elias/Hushlink/releases";
const MAX_METADATA: u64 = 2 * 1024 * 1024;
const MAX_CHECKSUMS: u64 = 64 * 1024;
const MAX_INSTALLER: u64 = 100 * 1024 * 1024;

/// Describes a release from the fixed Hushlink repository. Download locations
/// stay private so a caller cannot replace them with an arbitrary executable.
#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    version: String,
    notes: String,
    release_url: String,
    installer_url: String,
    installer_name: String,
    installer_size: u64,
    checksums_url: String,
    checksums_size: u64,
}

impl ReleaseInfo {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn notes(&self) -> &str {
        &self.notes
    }

    pub fn release_url(&self) -> &str {
        &self.release_url
    }

    pub fn installer_size(&self) -> u64 {
        self.installer_size
    }
}

/// An installer verified against the release's SHA256SUMS.txt. Its path is not
/// publicly constructible. Dropping an uninstalled download removes its staging
/// files; a launched installer retains its bootstrap until cache cleanup.
#[derive(Debug)]
pub struct VerifiedUpdate {
    release: ReleaseInfo,
    digest: [u8; 32],
    staging: Staging,
}

impl VerifiedUpdate {
    pub fn version(&self) -> &str {
        &self.release.version
    }
}

#[derive(Debug)]
struct Staging {
    directory: PathBuf,
    part: PathBuf,
    executable: PathBuf,
    launched: AtomicBool,
}

impl Drop for Staging {
    fn drop(&mut self) {
        if self.launched.load(Ordering::Acquire) {
            // Inno Setup may still need its original bootstrap after spawn.
            return;
        }
        // Delete only the two paths this instance created, never recursively.
        let _ = fs::remove_file(&self.part);
        let _ = fs::remove_file(&self.executable);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    draft: bool,
    html_url: String,
    body: Option<String>,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

fn allowed_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.fragment().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "api.github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn client(timeout: Duration) -> Result<Client> {
    Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(timeout)
        .user_agent(concat!("Hushlink/", env!("CARGO_PKG_VERSION")))
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !allowed_url(attempt.url()) {
                attempt.error("Nicht erlaubte Update-Weiterleitung")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .context("Sichere Update-Verbindung konnte nicht vorbereitet werden.")
}

fn read_limited(mut reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "Update-Antwort ist zu groß.");
    Ok(bytes)
}

fn get_bounded(client: &Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    ensure!(
        allowed_url(&Url::parse(url)?),
        "Unzulässige Update-Adresse."
    );
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .context("GitHub ist für die Update-Prüfung nicht erreichbar.")?
        .error_for_status()
        .context("GitHub hat die Update-Anfrage abgelehnt.")?;
    if let Some(length) = response.content_length() {
        ensure!(length <= limit, "Update-Antwort ist zu groß.");
    }
    read_limited(response, limit).context("Update-Antwort konnte nicht gelesen werden.")
}

/// Checks the 20 most recent releases, including prereleases, and chooses the
/// greatest semantic version newer than the running app. No credentials, device
/// identifier, chat contents or vault data are sent.
pub fn check_for_update(current_version: &str) -> Result<Option<ReleaseInfo>> {
    let client = client(Duration::from_secs(30))?;
    let bytes = get_bounded(&client, API_URL, MAX_METADATA)?;
    select_release(&bytes, current_version)
}

fn select_release(bytes: &[u8], current_version: &str) -> Result<Option<ReleaseInfo>> {
    ensure!(
        bytes.len() as u64 <= MAX_METADATA,
        "Update-Antwort ist zu groß."
    );
    let current = Version::parse(current_version).context("Ungültige lokale App-Version.")?;
    let releases: Vec<ApiRelease> =
        serde_json::from_slice(bytes).context("GitHub lieferte ungültige Update-Daten.")?;
    ensure!(releases.len() <= 20, "Zu viele Update-Einträge.");
    let mut newest: Option<(Version, ApiRelease)> = None;
    for release in releases {
        if release.draft {
            continue;
        }
        let Some(tag) = release.tag_name.strip_prefix('v') else {
            continue;
        };
        if tag.len() > 128 {
            continue;
        }
        let Ok(version) = Version::parse(tag) else {
            continue;
        };
        // A unique canonical spelling is used in both filenames and URLs.
        if version.to_string() != tag || !version.cmp_precedence(&current).is_gt() {
            continue;
        }
        if let Some((previous, _)) = &newest {
            ensure!(
                !previous.cmp_precedence(&version).is_eq(),
                "Mehrdeutige Update-Version auf GitHub."
            );
            if version.cmp_precedence(previous).is_lt() {
                continue;
            }
        }
        newest = Some((version, release));
    }
    let Some((version, release)) = newest else {
        return Ok(None);
    };
    let version = version.to_string();
    let installer_name = format!("Hushlink-v{version}-windows-x64-setup.exe");
    let download_root = format!("{RELEASE_ROOT}/download/v{version}");
    let release_url = format!("{RELEASE_ROOT}/tag/v{version}");
    ensure!(
        release.html_url == release_url,
        "Unzulässiger Update-Herausgeber."
    );
    let installer = exact_asset(&release.assets, &installer_name)?;
    let checksums = exact_asset(&release.assets, "SHA256SUMS.txt")?;
    ensure!(
        installer.browser_download_url == format!("{download_root}/{installer_name}")
            && checksums.browser_download_url == format!("{download_root}/SHA256SUMS.txt"),
        "Unzulässige Update-Dateiadresse."
    );
    ensure!(
        installer.size > 0 && installer.size <= MAX_INSTALLER,
        "Update-Installationsdatei ist zu groß oder leer."
    );
    ensure!(
        checksums.size > 0 && checksums.size <= MAX_CHECKSUMS,
        "Update-Prüfsummendatei ist zu groß oder leer."
    );
    let notes = release.body.as_deref().unwrap_or_default();
    Ok(Some(ReleaseInfo {
        version,
        notes: notes.chars().take(16_384).collect(),
        release_url,
        installer_url: installer.browser_download_url.clone(),
        installer_name,
        installer_size: installer.size,
        checksums_url: checksums.browser_download_url.clone(),
        checksums_size: checksums.size,
    }))
}

fn exact_asset<'a>(assets: &'a [ApiAsset], name: &str) -> Result<&'a ApiAsset> {
    let mut matches = assets.iter().filter(|asset| asset.name == name);
    let asset = matches
        .next()
        .context("Die Update-Version enthält noch nicht alle erforderlichen Dateien.")?;
    ensure!(matches.next().is_none(), "Mehrdeutige Update-Datei.");
    Ok(asset)
}

fn checksum_for(bytes: &[u8], filename: &str) -> Result<[u8; 32]> {
    let text = std::str::from_utf8(bytes).context("Ungültige Update-Prüfsummen.")?;
    let mut digest = None;
    for line in text.lines().filter(|line| !line.is_empty()) {
        // GNU sha256sum text/binary format: exactly 64 hex digits, then two
        // spaces (or space + '*'), then a plain basename without directories.
        let raw = line.as_bytes();
        ensure!(
            raw.len() > 66 && raw[64] == b' ' && matches!(raw[65], b' ' | b'*'),
            "Ungültiges Update-Prüfsummenformat."
        );
        let hash = hex::decode(&raw[..64]).context("Ungültige Update-Prüfsumme.")?;
        let name = std::str::from_utf8(&raw[66..])?;
        ensure!(
            !name.contains(['/', '\\', ':'])
                && !name.chars().any(char::is_control)
                && !matches!(name, "." | ".."),
            "Unzulässiger Dateiname in Update-Prüfsummen."
        );
        if name == filename {
            ensure!(digest.is_none(), "Doppelte Update-Prüfsumme.");
            digest = Some(<[u8; 32]>::try_from(hash.as_slice())?);
        }
    }
    digest.context("Installationsdatei fehlt in den Update-Prüfsummen.")
}

fn staging(root: &Path, filename: &str) -> Result<Staging> {
    fs::create_dir_all(root).context("Update-Ordner konnte nicht angelegt werden.")?;
    for _ in 0..8 {
        let mut random = [0u8; 16];
        OsRng.fill_bytes(&mut random);
        let directory = root.join(format!("update-{}", hex::encode(random)));
        let mut builder = fs::DirBuilder::new();
        builder.recursive(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&directory) {
            Ok(()) => {
                return Ok(Staging {
                    part: directory.join("download.part"),
                    executable: directory.join(filename),
                    directory,
                    launched: AtomicBool::new(false),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("Update konnte nicht vorbereitet werden."),
        }
    }
    bail!("Kein freier temporärer Update-Ordner gefunden.")
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    ensure!(
        !cancel.load(Ordering::Acquire),
        "Update-Download abgebrochen."
    );
    Ok(())
}

fn copy_verified(
    mut reader: impl Read,
    output: &mut File,
    expected_size: u64,
    expected_digest: &[u8; 32],
    cancel: &AtomicBool,
    progress: impl Fn(u64, u64),
) -> Result<()> {
    ensure!(
        expected_size > 0 && expected_size <= MAX_INSTALLER,
        "Ungültige Update-Größe."
    );
    let mut received = 0u64;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 32 * 1024];
    progress(0, expected_size);
    loop {
        cancelled(cancel)?;
        let count = reader
            .read(&mut buffer)
            .context("Update-Download unterbrochen.")?;
        if count == 0 {
            break;
        }
        received += count as u64;
        ensure!(
            received <= expected_size,
            "Update-Datei ist größer als angekündigt."
        );
        digest.update(&buffer[..count]);
        output.write_all(&buffer[..count])?;
        progress(received, expected_size);
    }
    cancelled(cancel)?;
    ensure!(received == expected_size, "Update-Datei ist unvollständig.");
    ensure!(
        digest.finalize().as_slice() == expected_digest,
        "Update-Prüfsumme stimmt nicht überein."
    );
    output.sync_all()?;
    Ok(())
}

/// Downloads and verifies a release on a worker thread. Cancellation is checked
/// between network reads; all requests have finite timeouts. Installation is a
/// separate explicit action. The callback must not block the worker.
pub fn download_update(
    release: &ReleaseInfo,
    cancel: &AtomicBool,
    progress: impl Fn(u64, u64),
) -> Result<VerifiedUpdate> {
    cancelled(cancel)?;
    let client = client(Duration::from_secs(120))?;
    let checksums = get_bounded(&client, &release.checksums_url, MAX_CHECKSUMS)?;
    ensure!(
        checksums.len() as u64 == release.checksums_size,
        "Update-Prüfsummendatei hat eine unerwartete Größe."
    );
    let digest = checksum_for(&checksums, &release.installer_name)?;
    cancelled(cancel)?;
    let project = directories::ProjectDirs::from("org", "Hushlink", "Hushlink")
        .context("Lokaler Update-Ordner nicht verfügbar.")?;
    let staging = staging(
        &project.cache_dir().join("updates"),
        &release.installer_name,
    )?;
    let response = client
        .get(&release.installer_url)
        .send()
        .context("Update-Download konnte nicht gestartet werden.")?
        .error_for_status()
        .context("GitHub hat den Update-Download abgelehnt.")?;
    if let Some(size) = response.content_length() {
        ensure!(
            size == release.installer_size,
            "Unerwartete Update-Dateigröße."
        );
    }
    let mut output = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&staging.part)
        .context("Temporäre Update-Datei konnte nicht angelegt werden.")?;
    copy_verified(
        response,
        &mut output,
        release.installer_size,
        &digest,
        cancel,
        progress,
    )?;
    validate_pe(&mut output, release.installer_size)?;
    cancelled(cancel)?;
    drop(output);
    fs::rename(&staging.part, &staging.executable)?;
    Ok(VerifiedUpdate {
        release: release.clone(),
        digest,
        staging,
    })
}

fn validate_pe(file: &mut File, size: u64) -> Result<()> {
    ensure!(size >= 256, "Update ist keine Windows-Installationsdatei.");
    file.seek(SeekFrom::Start(0))?;
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)?;
    ensure!(
        &dos[..2] == b"MZ",
        "Update ist keine Windows-Installationsdatei."
    );
    let offset = u32::from_le_bytes(dos[60..64].try_into()?) as u64;
    ensure!(
        offset >= 64 && offset + 94 <= size,
        "Beschädigter Windows-Dateikopf."
    );
    file.seek(SeekFrom::Start(offset))?;
    let mut header = [0u8; 94];
    file.read_exact(&mut header)?;
    ensure!(&header[..4] == b"PE\0\0", "Beschädigter Windows-Dateikopf.");
    let machine = u16::from_le_bytes(header[4..6].try_into()?);
    let characteristics = u16::from_le_bytes(header[22..24].try_into()?);
    let optional_size = u16::from_le_bytes(header[20..22].try_into()?) as u64;
    let magic = u16::from_le_bytes(header[24..26].try_into()?);
    let subsystem = u16::from_le_bytes(header[92..94].try_into()?);
    ensure!(
        matches!((machine, magic), (0x14c, 0x10b) | (0x8664, 0x20b))
            && characteristics & 0x0002 != 0
            && characteristics & 0x2000 == 0
            && optional_size >= 70
            && offset + 24 + optional_size <= size
            && subsystem == 2,
        "Update ist keine unterstützte Windows-Installationsdatei."
    );
    Ok(())
}

fn revalidate(file: &mut File, expected_size: u64, expected_digest: &[u8; 32]) -> Result<()> {
    ensure!(
        file.metadata()?.len() == expected_size,
        "Update-Datei wurde verändert."
    );
    let mut digest = Sha256::new();
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = [0u8; 32 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    ensure!(
        digest.finalize().as_slice() == expected_digest,
        "Update-Datei wurde verändert."
    );
    validate_pe(file, expected_size)
}

/// Starts only the already verified installer after explicit UI confirmation.
/// The UI must lock its vault, close sessions and exit immediately after success.
/// The per-user installer never needs elevation and never replaces a vault.
pub fn launch_installer(update: &VerifiedUpdate) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ)
            .open(&update.staging.executable)?;
        // Keep this read-only handle open across CreateProcess. Windows refuses
        // replacement or writes between the final hash check and process start.
        revalidate(&mut file, update.release.installer_size, &update.digest)?;
        std::process::Command::new(&update.staging.executable)
            .args([
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/CLOSEAPPLICATIONS",
                "/RESTARTAPPLICATIONS",
            ])
            .spawn()
            .context("Update-Installation konnte nicht gestartet werden.")?;
        update.staging.launched.store(true, Ordering::Release);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let mut file = File::open(&update.staging.executable)?;
        revalidate(&mut file, update.release.installer_size, &update.digest)?;
        bail!("Die Update-Installation ist nur unter Windows verfügbar.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::Cursor;

    fn fixture(version: &str) -> Value {
        let root = format!("{RELEASE_ROOT}/download/v{version}");
        let name = format!("Hushlink-v{version}-windows-x64-setup.exe");
        json!({
            "tag_name": format!("v{version}"), "draft": false, "prerelease": true,
            "html_url": format!("{RELEASE_ROOT}/tag/v{version}"), "body": "Künstliche Release-Notiz.",
            "assets": [
                { "name": name, "browser_download_url": format!("{root}/{name}"), "size": 4096 },
                { "name": "SHA256SUMS.txt", "browser_download_url": format!("{root}/SHA256SUMS.txt"), "size": 128 }
            ]
        })
    }

    fn select(releases: &[Value], current: &str) -> Result<Option<ReleaseInfo>> {
        select_release(&serde_json::to_vec(releases)?, current)
    }

    fn pe(machine: u16, magic: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 512];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(&machine.to_le_bytes());
        bytes[84..86].copy_from_slice(&224u16.to_le_bytes());
        bytes[86..88].copy_from_slice(&2u16.to_le_bytes());
        bytes[88..90].copy_from_slice(&magic.to_le_bytes());
        bytes[156..158].copy_from_slice(&2u16.to_le_bytes());
        bytes
    }

    #[test]
    fn selects_semver_and_prerelease_without_downgrade() -> Result<()> {
        let release = select(
            &[fixture("0.3.0-rc.1"), fixture("0.2.9"), fixture("0.10.0")],
            "0.2.0",
        )?
        .context("release")?;
        assert_eq!(release.version(), "0.10.0");
        assert_eq!(release.installer_size(), 4096);
        assert!(select(&[fixture("0.2.0"), fixture("0.1.9")], "0.2.0")?.is_none());
        assert_eq!(
            select(&[fixture("0.3.0-rc.1")], "0.2.0")?
                .context("prerelease")?
                .version(),
            "0.3.0-rc.1"
        );
        assert!(select(&[fixture("0.2.0-rc.1")], "0.2.0")?.is_none());
        assert!(select(&[fixture("0.2.0+build.2")], "0.2.0+build.1")?.is_none());
        Ok(())
    }

    #[test]
    fn rejects_ambiguous_incomplete_or_untrusted_release() -> Result<()> {
        assert!(select(&[fixture("0.3.0"), fixture("0.3.0")], "0.2.0").is_err());
        assert!(select(&[fixture("0.3.0+one"), fixture("0.3.0+two")], "0.2.0").is_err());
        let mut missing = fixture("0.3.0");
        missing["assets"] = json!([]);
        assert!(select(&[missing], "0.2.0").is_err());
        let mut duplicate = fixture("0.3.0");
        let asset = duplicate["assets"][0].clone();
        duplicate["assets"]
            .as_array_mut()
            .context("assets")?
            .push(asset);
        assert!(select(&[duplicate], "0.2.0").is_err());
        for field in ["name", "browser_download_url", "size"] {
            let mut release = fixture("0.3.0");
            release["assets"][0][field] = if field == "size" {
                json!(MAX_INSTALLER + 1)
            } else {
                json!("../../evil.exe")
            };
            assert!(select(&[release], "0.2.0").is_err());
        }
        let mut release = fixture("0.3.0");
        release["html_url"] = json!("https://github.com/attacker/Hushlink/releases/tag/v0.3.0");
        assert!(select(&[release], "0.2.0").is_err());
        let mut release = fixture("0.3.0");
        release["draft"] = json!(true);
        assert!(select(&[release], "0.2.0")?.is_none());
        assert!(select_release(&vec![b' '; MAX_METADATA as usize + 1], "0.2.0").is_err());
        assert!(select(&vec![fixture("0.3.0"); 21], "0.2.0").is_err());
        assert!(select_release(b"broken", "0.2.0").is_err());
        Ok(())
    }

    #[test]
    fn redirects_are_https_and_exact_hosts_only() -> Result<()> {
        for url in [
            "http://github.com/a",
            "https://github.com.attacker.test/a",
            "https://localhost/a",
            "https://127.0.0.1/a",
            "https://github.com:8443/a",
            "https://user:pass@github.com/a",
            "https://github.com/a#fragment",
        ] {
            assert!(!allowed_url(&Url::parse(url)?), "{url}");
        }
        for host in [
            "github.com",
            "api.github.com",
            "release-assets.githubusercontent.com",
            "objects.githubusercontent.com",
        ] {
            assert!(allowed_url(&Url::parse(&format!(
                "https://{host}/a?signature=synthetic"
            ))?));
        }
        Ok(())
    }

    #[test]
    fn checksum_parser_rejects_duplicates_and_path_traversal() -> Result<()> {
        let hash = "12".repeat(32);
        let good = format!("{hash}  setup.exe\r\n{hash} *other.zip\r\n");
        assert_eq!(checksum_for(good.as_bytes(), "setup.exe")?, [0x12; 32]);
        for text in [
            format!("{hash}  setup.exe\n{hash}  setup.exe\n"),
            format!("{hash}  ../setup.exe\n"),
            format!("{hash}  C:\\setup.exe\n"),
            format!("{hash} setup.exe\n"),
            format!("{hash}  other.exe\n"),
            format!("{}  setup.exe\n", "zz".repeat(32)),
        ] {
            assert!(checksum_for(text.as_bytes(), "setup.exe").is_err());
        }
        assert!(read_limited(Cursor::new([0u8; 11]), 10).is_err());
        assert_eq!(read_limited(Cursor::new([1u8; 10]), 10)?, vec![1; 10]);
        Ok(())
    }

    #[test]
    fn validates_pe_and_rejects_tampering_truncation_and_non_gui() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("synthetic.exe");
        for (machine, magic) in [(0x14c, 0x10b), (0x8664, 0x20b)] {
            let bytes = pe(machine, magic);
            fs::write(&path, &bytes)?;
            let mut file = OpenOptions::new().read(true).write(true).open(&path)?;
            let digest: [u8; 32] = Sha256::digest(&bytes).into();
            revalidate(&mut file, bytes.len() as u64, &digest)?;
            file.seek(SeekFrom::Start(300))?;
            file.write_all(b"tampered")?;
            assert!(revalidate(&mut file, bytes.len() as u64, &digest).is_err());
        }
        for bytes in [
            vec![0u8; 512],
            pe(0x14c, 0x20b),
            pe(0xaa64, 0x20b),
            vec![0; 10],
        ] {
            fs::write(&path, &bytes)?;
            assert!(validate_pe(&mut File::open(&path)?, bytes.len() as u64).is_err());
        }
        let mut bytes = pe(0x8664, 0x20b);
        bytes[156..158].copy_from_slice(&3u16.to_le_bytes());
        fs::write(&path, &bytes)?;
        assert!(validate_pe(&mut File::open(&path)?, bytes.len() as u64).is_err());
        Ok(())
    }

    #[test]
    fn download_copy_checks_length_hash_cancel_and_cleans_staging() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let bytes = pe(0x8664, 0x20b);
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        let cancel = AtomicBool::new(false);
        let staged = staging(directory.path(), "setup.exe")?;
        let staged_path = staged.directory.clone();
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&staged.part)?;
        copy_verified(
            Cursor::new(&bytes),
            &mut file,
            bytes.len() as u64,
            &digest,
            &cancel,
            |_, _| {},
        )?;
        assert!(
            copy_verified(
                Cursor::new(&bytes),
                &mut file,
                bytes.len() as u64 - 1,
                &digest,
                &cancel,
                |_, _| {}
            )
            .is_err()
        );
        assert!(
            copy_verified(
                Cursor::new(&bytes),
                &mut file,
                bytes.len() as u64 + 1,
                &digest,
                &cancel,
                |_, _| {}
            )
            .is_err()
        );
        assert!(
            copy_verified(
                Cursor::new(&bytes),
                &mut file,
                bytes.len() as u64,
                &[0; 32],
                &cancel,
                |_, _| {}
            )
            .is_err()
        );
        cancel.store(true, Ordering::Release);
        assert!(
            copy_verified(
                Cursor::new(&bytes),
                &mut file,
                bytes.len() as u64,
                &digest,
                &cancel,
                |_, _| {}
            )
            .is_err()
        );
        cancel.store(false, Ordering::Release);
        assert!(
            copy_verified(
                Cursor::new(&bytes),
                &mut file,
                bytes.len() as u64,
                &digest,
                &cancel,
                |received, _| {
                    if received > 0 {
                        cancel.store(true, Ordering::Release);
                    }
                }
            )
            .is_err()
        );
        drop(file);
        drop(staged);
        assert!(!staged_path.exists());
        Ok(())
    }
}
