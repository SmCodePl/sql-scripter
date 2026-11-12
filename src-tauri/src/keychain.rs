//! Source of the AES key used to encrypt stored passwords. Loaded lazily, on first use.
//!
//! - Release builds: OS credential store (macOS Keychain / Windows Credential Manager).
//! - Debug builds: a key file in the app data directory. An unsigned debug binary changes its
//!   code signature on every rebuild, which would make macOS ask for Keychain access each time.
//!   Debug builds therefore also use a separate store file (see `lib.rs`).

#[cfg(debug_assertions)]
use std::path::Path;

#[cfg(not(debug_assertions))]
use scripter_core::Error;
use scripter_core::crypto::{Cipher, LazyCipher};
#[cfg(not(debug_assertions))]
use scripter_core::i18n::t;

#[cfg(not(debug_assertions))]
pub fn key_source(_app_data: &std::path::Path) -> LazyCipher {
    LazyCipher::new(load_from_keychain)
}

#[cfg(debug_assertions)]
pub fn key_source(app_data: &Path) -> LazyCipher {
    let path = app_data.join("dev-encryption.key");
    LazyCipher::new(move || load_from_file(&path))
}

#[cfg(not(debug_assertions))]
const SERVICE: &str = "com.sm-code.sqlscripter";
#[cfg(not(debug_assertions))]
const ACCOUNT: &str = "connections-db3-key";

#[cfg(not(debug_assertions))]
fn load_from_keychain() -> scripter_core::Result<Cipher> {
    let denied = |e: keyring::Error| {
        Error::Validation(format!(
            "{} ({e})",
            t(
                "Brak dostępu do klucza szyfrowania w pęku kluczy systemu. Spróbuj ponownie i wybierz „Zawsze zezwalaj”",
                "No access to the encryption key in the system keychain. Try again and choose “Always Allow”",
            )
        ))
    };

    let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(denied)?;
    let key = match entry.get_password() {
        Ok(key) => key,
        Err(keyring::Error::NoEntry) => {
            let key = Cipher::generate_key_b64();
            entry.set_password(&key).map_err(denied)?;
            tracing::info!("generated new password encryption key in the OS keychain");
            key
        }
        Err(e) => return Err(denied(e)),
    };
    Cipher::from_key_b64(&key)
}

#[cfg(debug_assertions)]
fn load_from_file(path: &Path) -> scripter_core::Result<Cipher> {
    if let Ok(key) = std::fs::read_to_string(path) {
        return Cipher::from_key_b64(&key);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let key = Cipher::generate_key_b64();
    std::fs::write(path, &key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    tracing::warn!(path = %path.display(), "DEBUG build: using a file-based encryption key");
    Cipher::from_key_b64(&key)
}
