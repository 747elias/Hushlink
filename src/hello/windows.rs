use super::*;
use rand::{RngCore, rngs::OsRng};
use std::{mem, ptr};
use windows_sys::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, LocalFree},
        Security::{
            Authorization::ConvertSidToStringSidW, Cryptography::*, GetTokenInformation,
            TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    },
    core::PCWSTR,
};
struct Cng(usize);
impl Drop for Cng {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                NCryptFreeObject(self.0);
            }
        }
    }
}
fn checked(status: i32) -> anyhow::Result<()> {
    anyhow::ensure!(
        status == 0,
        "Windows Hello ist nicht verfügbar, wurde abgebrochen oder unterstützt den benötigten Schlüsselschutz nicht. Bitte das Passwort verwenden."
    );
    Ok(())
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn provider() -> anyhow::Result<Cng> {
    let mut handle = 0;
    checked(unsafe { NCryptOpenStorageProvider(&mut handle, MS_NGC_KEY_STORAGE_PROVIDER, 0) })?;
    Ok(Cng(handle))
}
fn key_name(id: &str) -> anyhow::Result<Vec<u16>> {
    anyhow::ensure!(
        id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()),
        "Ungültiger Schlüsselverweis."
    );
    struct Token(HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let mut token = ptr::null_mut();
    anyhow::ensure!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } != 0,
        "Windows-Benutzer konnte nicht geprüft werden."
    );
    let token = Token(token);
    let mut user = [0usize; 128];
    let mut length = 0;
    anyhow::ensure!(
        unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                user.as_mut_ptr().cast(),
                mem::size_of_val(&user) as u32,
                &mut length,
            )
        } != 0,
        "Windows-Benutzer konnte nicht geprüft werden."
    );
    let user = unsafe { &*user.as_ptr().cast::<TOKEN_USER>() };
    let mut sid = ptr::null_mut();
    anyhow::ensure!(
        unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) } != 0,
        "Windows-Benutzer konnte nicht geprüft werden."
    );
    let mut count = 0;
    while count < 256 && unsafe { *sid.add(count) } != 0 {
        count += 1;
    }
    let result = if count < 256 {
        String::from_utf16(unsafe { std::slice::from_raw_parts(sid, count) }).ok()
    } else {
        None
    };
    unsafe {
        LocalFree(sid.cast());
    }
    let sid =
        result.ok_or_else(|| anyhow::anyhow!("Windows-Benutzer konnte nicht geprüft werden."))?;
    Ok(wide(&format!("{sid}//Hushlink//{id}")))
}
fn set(key: &Cng, name: PCWSTR, value: u32) -> anyhow::Result<()> {
    checked(unsafe { NCryptSetProperty(key.0, name, value.to_le_bytes().as_ptr(), 4, 0) })
}
fn property(key: &Cng, name: PCWSTR) -> anyhow::Result<u32> {
    let mut value = 0u32;
    let mut size = 0;
    checked(unsafe {
        NCryptGetProperty(
            key.0,
            name,
            (&mut value as *mut u32).cast(),
            4,
            &mut size,
            0,
        )
    })?;
    anyhow::ensure!(
        size == 4,
        "Windows Hello bestätigt den benötigten Schlüsselschutz nicht."
    );
    Ok(value)
}
fn validate_key(key: &Cng) -> anyhow::Result<()> {
    anyhow::ensure!(
        property(key, NCRYPT_LENGTH_PROPERTY)? == 2048
            && property(key, NCRYPT_EXPORT_POLICY_PROPERTY)? == 0
            && property(key, NCRYPT_KEY_USAGE_PROPERTY)? == NCRYPT_ALLOW_DECRYPT_FLAG
            && property(key, windows_sys::core::w!("NgcCacheType"))? == 1,
        "Windows Hello bestätigt den benötigten Schlüsselschutz nicht."
    );
    Ok(())
}
fn open(slot: &Slot) -> anyhow::Result<(Cng, Cng)> {
    slot.validate()?;
    let provider = provider()?;
    let name = key_name(&slot.key_id)?;
    let mut key = 0;
    checked(unsafe { NCryptOpenKey(provider.0, &mut key, name.as_ptr(), 0, 0) })?;
    let key = Cng(key);
    validate_key(&key)?;
    Ok((provider, key))
}
fn padding() -> BCRYPT_OAEP_PADDING_INFO {
    BCRYPT_OAEP_PADDING_INFO {
        pszAlgId: BCRYPT_SHA256_ALGORITHM,
        pbLabel: ptr::null_mut(),
        cbLabel: 0,
    }
}
fn decrypt(key: &Cng, wrapped: &[u8], silent: bool) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    let context = wide("Hushlink-Tresor entsperren");
    checked(unsafe {
        NCryptSetProperty(
            key.0,
            NCRYPT_USE_CONTEXT_PROPERTY,
            context.as_ptr().cast(),
            (context.len() * 2) as u32,
            0,
        )
    })?;
    set(key, NCRYPT_PIN_CACHE_IS_GESTURE_REQUIRED_PROPERTY, 1)?;
    let mut output = Zeroizing::new([0u8; 256]);
    let mut size = 0;
    let pad = padding();
    checked(unsafe {
        NCryptDecrypt(
            key.0,
            wrapped.as_ptr(),
            wrapped.len() as u32,
            (&pad as *const BCRYPT_OAEP_PADDING_INFO).cast(),
            output.as_mut_ptr(),
            output.len() as u32,
            &mut size,
            NCRYPT_PAD_OAEP_FLAG | if silent { NCRYPT_SILENT_FLAG } else { 0 },
        )
    })?;
    anyhow::ensure!(
        size == 32,
        "Windows Hello hat einen ungültigen Tresorschlüssel geliefert."
    );
    let mut value = Zeroizing::new([0; 32]);
    value.copy_from_slice(&output[..32]);
    Ok(value)
}
pub fn enroll(secret: &[u8; 32]) -> anyhow::Result<Slot> {
    let provider = provider()?;
    let mut id = [0; 16];
    OsRng.try_fill_bytes(&mut id)?;
    let id = hex::encode(id);
    let name = key_name(&id)?;
    let mut handle = 0;
    checked(unsafe {
        NCryptCreatePersistedKey(
            provider.0,
            &mut handle,
            NCRYPT_RSA_ALGORITHM,
            name.as_ptr(),
            0,
            0,
        )
    })?;
    let mut key = Cng(handle);
    let result = (|| {
        set(&key, NCRYPT_LENGTH_PROPERTY, 2048)?;
        set(&key, NCRYPT_EXPORT_POLICY_PROPERTY, 0)?;
        set(&key, NCRYPT_KEY_USAGE_PROPERTY, NCRYPT_ALLOW_DECRYPT_FLAG)?;
        // AUTH_MANDATORY. Unsupported providers fail closed; never fall back to
        // a cached/optional user-presence policy or an exportable software key.
        set(&key, windows_sys::core::w!("NgcCacheType"), 1)?;
        checked(unsafe { NCryptFinalizeKey(key.0, 0) })?;
        validate_key(&key)?;
        let mut wrapped = vec![0; 256];
        let mut size = 0;
        let pad = padding();
        checked(unsafe {
            NCryptEncrypt(
                key.0,
                secret.as_ptr(),
                32,
                (&pad as *const BCRYPT_OAEP_PADDING_INFO).cast(),
                wrapped.as_mut_ptr(),
                256,
                &mut size,
                NCRYPT_PAD_OAEP_FLAG,
            )
        })?;
        anyhow::ensure!(size == 256, "Windows-Hello-Schlüsselgrösse ungültig.");
        anyhow::ensure!(
            decrypt(&key, &wrapped, true).is_err(),
            "Windows Hello erlaubt Schlüsselzugriff ohne Bestätigung. Einrichtung wurde abgebrochen."
        );
        let roundtrip = decrypt(&key, &wrapped, false)?;
        anyhow::ensure!(
            roundtrip.as_ref() == secret,
            "Windows-Hello-Schlüsselprüfung fehlgeschlagen."
        );
        Ok(Slot {
            version: 1,
            key_id: id,
            wrapped_key: wrapped,
        })
    })();
    if result.is_err() && unsafe { NCryptDeleteKey(key.0, 0) } == 0 {
        key.0 = 0;
    }
    result
}
pub fn unwrap(slot: &Slot) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    let (_provider, key) = open(slot)?;
    anyhow::ensure!(
        decrypt(&key, &slot.wrapped_key, true).is_err(),
        "Windows Hello verlangt keine erneute Bestätigung. Bitte mit Passwort entsperren und Hello neu einrichten."
    );
    decrypt(&key, &slot.wrapped_key, false)
}
pub fn remove(slot: &Slot) -> anyhow::Result<()> {
    let (_provider, mut key) = open(slot)?;
    checked(unsafe { NCryptDeleteKey(key.0, 0) })?;
    key.0 = 0;
    Ok(())
}
