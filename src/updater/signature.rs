//! Authenticode validation of the exact deny-write handle before process start.
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    mem,
    os::windows::{ffi::OsStrExt, io::AsRawHandle},
    path::Path,
    ptr,
};
use windows_sys::{Win32::Security::WinTrust::*, core::GUID};
struct Trust {
    data: WINTRUST_DATA,
    action: GUID,
}
impl Drop for Trust {
    fn drop(&mut self) {
        self.data.dwStateAction = WTD_STATEACTION_CLOSE;
        unsafe {
            WinVerifyTrust(
                ptr::null_mut(),
                &mut self.action,
                (&mut self.data as *mut WINTRUST_DATA).cast(),
            );
        }
    }
}
pub(super) fn verify(file: &File, path: &Path) -> Result<()> {
    let configured = option_env!("HUSHLINK_PUBLISHER_SHA256").unwrap_or("");
    ensure!(
        configured.len() == 64 && configured.bytes().all(|c| c.is_ascii_hexdigit()),
        "Dieser Entwicklungsbuild hat noch kein autorisiertes Herausgeberzertifikat für automatische Installationen. Bitte erst den signierten Release verwenden."
    );
    let expected = hex::decode(configured)?;
    verify_with_pin(file, path, &expected)
}
fn verify_with_pin(file: &File, path: &Path, expected: &[u8]) -> Result<()> {
    ensure!(expected.len() == 32, "Herausgeberkonfiguration ungültig.");
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut info = WINTRUST_FILE_INFO {
        cbStruct: mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: wide.as_ptr(),
        hFile: file.as_raw_handle(),
        pgKnownSubject: ptr::null_mut(),
    };
    let data = WINTRUST_DATA {
        cbStruct: mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_WHOLECHAIN,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut info },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT,
        ..Default::default()
    };
    let mut trust = Trust {
        data,
        action: WINTRUST_ACTION_GENERIC_VERIFY_V2,
    };
    let status = unsafe {
        WinVerifyTrust(
            ptr::null_mut(),
            &mut trust.action,
            (&mut trust.data as *mut WINTRUST_DATA).cast(),
        )
    };
    ensure!(
        status == 0,
        "Die Windows-Herausgebersignatur des Updates ist ungültig oder kann nicht geprüft werden."
    );
    let provider = unsafe { WTHelperProvDataFromStateData(trust.data.hWVTStateData) };
    ensure!(
        !provider.is_null(),
        "Herausgebersignatur konnte nicht gelesen werden."
    );
    let signer = unsafe { WTHelperGetProvSignerFromChain(provider, 0, 0, 0) };
    ensure!(!signer.is_null(), "Herausgebersignatur fehlt.");
    let signer = unsafe { &*signer };
    ensure!(
        signer.dwError == 0
            && signer.csCertChain > 0
            && !signer.pasCertChain.is_null()
            && signer.csCounterSigners > 0,
        "Herausgebersignatur oder bestätigter Zeitstempel fehlt."
    );
    let leaf = unsafe { &*signer.pasCertChain };
    ensure!(
        leaf.dwError == 0 && !leaf.pCert.is_null(),
        "Herausgeberzertifikat ungültig."
    );
    let certificate = unsafe { &*leaf.pCert };
    ensure!(
        !certificate.pbCertEncoded.is_null()
            && certificate.cbCertEncoded > 0
            && certificate.cbCertEncoded <= 65_536,
        "Herausgeberzertifikat ungültig."
    );
    let der = unsafe {
        std::slice::from_raw_parts(
            certificate.pbCertEncoded,
            certificate.cbCertEncoded as usize,
        )
    };
    ensure!(
        Sha256::digest(der).as_slice() == expected,
        "Das Update wurde von einem anderen Herausgeberzertifikat signiert. Zertifikatswechsel unabhängig prüfen und gegebenenfalls den neuen Installer manuell beziehen."
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsigned_test_binary_never_passes_publisher_validation() -> Result<()> {
        let path = std::env::current_exe()?;
        let file = File::open(&path)?;
        assert!(verify_with_pin(&file, &path, &[0; 32]).is_err());
        assert!(verify_with_pin(&file, &path, &[]).is_err());
        Ok(())
    }
}
