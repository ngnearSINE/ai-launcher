//! Chromium `os_crypt` as used by Electron `safeStorage`.
//! Windows: AES-256-GCM with a key wrapped by DPAPI in `Local State`.
//! Linux: AES-128-CBC with a PBKDF2 key from the keyring password (`v11`) or `peanuts` (`v10`).
//! Plaintext only lives in memory and is never logged.

use std::path::Path;

use base64::Engine;

use crate::model::UsageError;

fn b64(s: &str) -> Result<Vec<u8>, UsageError> {
    base64::engine::general_purpose::STANDARD.decode(s.trim()).map_err(|_| UsageError::Decrypt("base64".into()))
}

/// Decrypts a base64 `v10`/`v11` blob stored by the app whose data folder is `profile_dir`.
pub fn decrypt_os_crypt(profile_dir: &Path, encoded: &str) -> Result<Vec<u8>, UsageError> {
    let blob = b64(encoded)?;
    if blob.len() < 3 || !(blob.starts_with(b"v10") || blob.starts_with(b"v11")) {
        return Err(UsageError::Decrypt("prefix".into()));
    }
    imp::decrypt(profile_dir, &blob)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, UsageError> {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(&input, None, None, None, None, 0, &mut output)
                .map_err(|_| UsageError::Decrypt("dpapi".into()))?;
            let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(output.pbData as _)));
            Ok(out)
        }
    }

    pub fn decrypt(profile_dir: &Path, blob: &[u8]) -> Result<Vec<u8>, UsageError> {
        let state = std::fs::read(profile_dir.join("Local State")).map_err(|_| UsageError::Decrypt("local-state".into()))?;
        let state: serde_json::Value = serde_json::from_slice(&state).map_err(|_| UsageError::Decrypt("local-state".into()))?;
        let wrapped = state["os_crypt"]["encrypted_key"].as_str().ok_or(UsageError::Decrypt("key-missing".into()))?;
        let wrapped = b64(wrapped)?;
        let wrapped = wrapped.strip_prefix(b"DPAPI").ok_or(UsageError::Decrypt("key-prefix".into()))?;
        let mut key = dpapi_unprotect(wrapped)?;
        if key.len() != 32 || blob.len() < 3 + 12 + 16 {
            key.iter_mut().for_each(|b| *b = 0);
            return Err(UsageError::Decrypt("length".into()));
        }
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| UsageError::Decrypt("key".into()));
        key.iter_mut().for_each(|b| *b = 0);
        let nonce = Nonce::from_slice(&blob[3..15]);
        cipher?.decrypt(nonce, &blob[15..]).map_err(|_| UsageError::Decrypt("aead".into()))
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};

    type Dec = cbc::Decryptor<aes::Aes128>;

    /// Reads the Secret Service item Chromium stores for the app (`application=Claude`)
    /// through python3 (GObject or dbus), which ships with Ubuntu desktops.
    fn keyring_password(app: &str) -> Result<Vec<u8>, UsageError> {
        const SCRIPT: &str = r#"
import sys, base64
app = sys.argv[1]
def via_gi():
    import gi; gi.require_version('Secret', '1'); from gi.repository import Secret
    schema = Secret.Schema.new('chrome_libsecret_os_crypt_password_v2', Secret.SchemaFlags.DONT_MATCH_NAME, {'application': Secret.SchemaAttributeType.STRING})
    return Secret.password_lookup_sync(schema, {'application': app}, None)
def via_dbus():
    import dbus
    bus = dbus.SessionBus(); svc = bus.get_object('org.freedesktop.secrets', '/org/freedesktop/secrets')
    iface = dbus.Interface(svc, 'org.freedesktop.Secret.Service')
    _, session = iface.OpenSession('plain', dbus.String('', variant_level=1))
    unlocked, locked = iface.SearchItems({'xdg:schema': 'chrome_libsecret_os_crypt_password_v2', 'application': app})
    items = list(unlocked)
    if not items: return None
    item = dbus.Interface(bus.get_object('org.freedesktop.secrets', items[0]), 'org.freedesktop.Secret.Item')
    return bytes(item.GetSecret(session)[2]).decode('utf8')
pw = None
for f in (via_gi, via_dbus):
    try:
        pw = f()
        if pw: break
    except Exception: pass
if not pw: sys.exit(1)
sys.stdout.write(base64.b64encode(pw.encode('utf8')).decode())
"#;
        let out = launcher_platform::host_command("python3")
            .args(["-c", SCRIPT, app])
            .stderr(std::process::Stdio::null())
            .output()
            .map_err(|_| UsageError::Decrypt("keyring".into()))?;
        if !out.status.success() {
            return Err(UsageError::Decrypt("keyring".into()));
        }
        b64(&String::from_utf8_lossy(&out.stdout))
    }

    pub fn decrypt(_profile_dir: &Path, blob: &[u8]) -> Result<Vec<u8>, UsageError> {
        let password = if blob.starts_with(b"v10") { b"peanuts".to_vec() } else { keyring_password("Claude")? };
        let mut key = [0u8; 16];
        pbkdf2::pbkdf2_hmac::<sha1::Sha1>(&password, b"saltysalt", 1, &mut key);
        let mut buf = blob[3..].to_vec();
        let plain = Dec::new(&key.into(), &[0x20u8; 16].into())
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map_err(|_| UsageError::Decrypt("cbc".into()))?
            .to_vec();
        Ok(plain)
    }
}
