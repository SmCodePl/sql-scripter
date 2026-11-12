//! Source of the AES key used to encrypt stored passwords. Loaded lazily, on first use.
//!
//! Strategy (release builds):
//! 1. If a local key file already exists, use it (stable across rebuilds).
//! 2. Otherwise try the OS credential store (macOS Keychain / Windows Credential Manager).
//! 3. If the credential store is unavailable (user denied, unsigned build), create a local key file.
//!
//! Debug builds always use a key file to avoid Keychain prompts on every recompile.

use std::path::Path;

use scripter_core::crypto::{Cipher, LazyCipher};

#[cfg(not(debug_assertions))]
const SERVICE: &str = "com.sm-code.sqlscripter";
#[cfg(not(debug_assertions))]
const ACCOUNT: &str = "connections-db3-key";

pub fn key_source(app_data: &Path) -> LazyCipher {
    let key_path = app_data.join(KEY_FILE);

    #[cfg(debug_assertions)]
    {
        LazyCipher::new(move || load_from_file(&key_path))
    }

    #[cfg(not(debug_assertions))]
    {
        LazyCipher::new(move || load_release(&key_path))
    }
}

#[cfg(debug_assertions)]
const KEY_FILE: &str = "dev-encryption.key";

#[cfg(not(debug_assertions))]
const KEY_FILE: &str = "encryption.key";

/// Release key loading: file first, then Keychain, then create file.
#[cfg(not(debug_assertions))]
fn load_release(key_path: &Path) -> scripter_core::Result<Cipher> {
    // 1. Local key file takes priority (once created, always used).
    if key_path.exists() {
        if let Ok(cipher) = load_from_file(key_path) {
            return Ok(cipher);
        }
    }

    // 2. Try the OS credential store.
    match load_from_keychain() {
        Ok((cipher, key)) => {
            // Persist to file so future launches don't depend on Keychain.
            persist_key_to_file(key_path, &key);
            Ok(cipher)
        }
        Err(e) => {
            tracing::warn!(error = %e, "OS credential store unavailable, creating a local key file");
            // 3. Neither Keychain nor file existed — generate a fresh file-based key.
            create_new_key_file(key_path)
        }
    }
}

#[cfg(not(debug_assertions))]
fn load_from_keychain() -> scripter_core::Result<(Cipher, String)> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| scripter_core::Error::Validation(format!("keyring: {e}")))?;

    let key = match entry.get_password() {
        Ok(key) => key,
        Err(keyring::Error::NoEntry) => {
            let key = Cipher::generate_key_b64();
            entry.set_password(&key)
                .map_err(|e| scripter_core::Error::Validation(format!("keyring set: {e}")))?;
            tracing::info!("generated new encryption key in the OS credential store");
            key
        }
        Err(e) => return Err(scripter_core::Error::Validation(format!("keyring get: {e}"))),
    };
    let cipher = Cipher::from_key_b64(&key)?;
    Ok((cipher, key))
}

#[cfg(not(debug_assertions))]
fn persist_key_to_file(path: &Path, key: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(path, key) {
        tracing::warn!(error = %e, "could not persist encryption key to file");
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    tracing::info!(path = %path.display(), "persisted encryption key from credential store to file");
}

fn load_from_file(path: &Path) -> scripter_core::Result<Cipher> {
    let key = std::fs::read_to_string(path)?;
    Cipher::from_key_b64(key.trim())
}

#[cfg(not(debug_assertions))]
fn create_new_key_file(path: &Path) -> scripter_core::Result<Cipher> {
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
    tracing::info!(path = %path.display(), "created new file-based encryption key");
    Cipher::from_key_b64(&key)
}
