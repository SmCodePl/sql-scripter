//! Reversible encryption of stored passwords (AES-256-GCM).
//!
//! Passwords cannot be hashed: the app must send the original password to SQL Server.
//! The 256-bit key never touches the `.db3` file – the Tauri app keeps it in the OS
//! credential store (macOS Keychain / Windows Credential Manager).

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;

use std::sync::OnceLock;

use crate::error::{Error, Result};

/// Prefix identifying the storage format, so the scheme can evolve later.
const PREFIX: &str = "v1:";
const NONCE_LEN: usize = 12;

#[derive(Clone)]
pub struct Cipher {
    aead: Aes256Gcm,
}

impl Cipher {
    pub fn new(key: &[u8; 32]) -> Self {
        Self { aead: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key)) }
    }

    /// New random key, base64-encoded for storage in the OS keychain.
    pub fn generate_key_b64() -> String {
        B64.encode(Aes256Gcm::generate_key(&mut OsRng))
    }

    pub fn from_key_b64(key_b64: &str) -> Result<Self> {
        let bytes = B64.decode(key_b64.trim()).map_err(|e| Error::Crypto(e.to_string()))?;
        let key: [u8; 32] = bytes.try_into().map_err(|_| Error::Crypto("invalid key length".into()))?;
        Ok(Self::new(&key))
    }

    /// Returns `v1:<base64(nonce || ciphertext+tag)>`. A fresh random nonce is used every time.
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.aead.encrypt(&nonce, plaintext.as_bytes()).map_err(|e| Error::Crypto(e.to_string()))?;
        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        Ok(format!("{PREFIX}{}", B64.encode(blob)))
    }

    pub fn decrypt(&self, stored: &str) -> Result<String> {
        let encoded = stored.strip_prefix(PREFIX).ok_or_else(|| Error::Crypto("unknown format".into()))?;
        let blob = B64.decode(encoded).map_err(|e| Error::Crypto(e.to_string()))?;
        if blob.len() <= NONCE_LEN {
            return Err(Error::Crypto("ciphertext too short".into()));
        }
        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
        let plaintext = self
            .aead
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            // Wrong key (e.g. keychain entry was deleted) ends up here.
            .map_err(|_| Error::Crypto("decryption failed – key changed?".into()))?;
        String::from_utf8(plaintext).map_err(|e| Error::Crypto(e.to_string()))
    }
}

type KeyLoader = dyn Fn() -> Result<Cipher> + Send + Sync;

/// Cipher whose key is loaded on first use (e.g. from the OS keychain).
/// A failed load (user denied keychain access) is not cached, so the next use retries.
pub struct LazyCipher {
    loader: Box<KeyLoader>,
    cell: OnceLock<Cipher>,
}

impl LazyCipher {
    pub fn new(loader: impl Fn() -> Result<Cipher> + Send + Sync + 'static) -> Self {
        Self { loader: Box::new(loader), cell: OnceLock::new() }
    }

    /// Already-loaded key (tests, tools).
    pub fn ready(cipher: Cipher) -> Self {
        let cell = OnceLock::new();
        let _ = cell.set(cipher);
        Self { loader: Box::new(|| Err(Error::Crypto("no key loader".into()))), cell }
    }

    pub fn get(&self) -> Result<&Cipher> {
        if let Some(c) = self.cell.get() {
            return Ok(c);
        }
        let cipher = (self.loader)()?;
        // If another thread won the race, its (identical) key is kept.
        let _ = self.cell.set(cipher);
        Ok(self.cell.get().expect("cipher just set"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_random_nonce() {
        let c = Cipher::from_key_b64(&Cipher::generate_key_b64()).unwrap();
        let a = c.encrypt("Hasło;z=znakami\"").unwrap();
        let b = c.encrypt("Hasło;z=znakami\"").unwrap();
        assert_ne!(a, b, "same plaintext must give different ciphertexts");
        assert!(a.starts_with("v1:"));
        assert!(!a.contains("Hasło"));
        assert_eq!(c.decrypt(&a).unwrap(), "Hasło;z=znakami\"");
    }

    #[test]
    fn wrong_key_or_tampering_fails() {
        let c1 = Cipher::from_key_b64(&Cipher::generate_key_b64()).unwrap();
        let c2 = Cipher::from_key_b64(&Cipher::generate_key_b64()).unwrap();
        let enc = c1.encrypt("secret").unwrap();
        assert!(c2.decrypt(&enc).is_err());

        let mut tampered = enc.clone();
        tampered.pop();
        tampered.push(if enc.ends_with('A') { 'B' } else { 'A' });
        assert!(c1.decrypt(&tampered).is_err());
        assert!(Cipher::from_key_b64("short").is_err());
    }

    #[test]
    fn lazy_cipher_retries_after_failure() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let key = Cipher::generate_key_b64();
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let lazy = LazyCipher::new(move || {
            // First call fails (user cancelled the keychain prompt), second succeeds.
            if c.fetch_add(1, Ordering::SeqCst) == 0 { Err(Error::Crypto("cancelled".into())) } else { Cipher::from_key_b64(&key) }
        });
        assert!(lazy.get().is_err());
        assert!(lazy.get().is_ok());
        assert!(lazy.get().is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 2, "successful key is cached");
    }
}
