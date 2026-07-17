//! AES-256-GCM encryption for eBird tokens held in the state file.
//!
//! The key comes from `EBIRD_ALERT_ENC_KEY` (64 hex chars = 32 bytes). Ciphertext is
//! serialized as `base64(nonce):base64(ciphertext)`.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Context as _, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

/// Parse the hex `EBIRD_ALERT_ENC_KEY` into 32 raw bytes.
pub fn parse_key(hex_key: &str) -> anyhow::Result<[u8; 32]> {
    let bytes = hex::decode(hex_key.trim()).context("EBIRD_ALERT_ENC_KEY must be hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("EBIRD_ALERT_ENC_KEY must be 32 bytes (64 hex chars)"))
}

/// Encrypt `plaintext`, returning `base64(nonce):base64(ciphertext)`.
pub fn encrypt(key: &[u8; 32], plaintext: &str) -> anyhow::Result<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("encrypt failed: {e}"))?;
    Ok(format!(
        "{}:{}",
        STANDARD.encode(nonce),
        STANDARD.encode(ciphertext)
    ))
}

/// Decrypt a `base64(nonce):base64(ciphertext)` string produced by [`encrypt`].
pub fn decrypt(key: &[u8; 32], enc: &str) -> anyhow::Result<String> {
    let (nonce_b64, ct_b64) = enc
        .split_once(':')
        .ok_or_else(|| anyhow!("malformed ciphertext (expected nonce:ciphertext)"))?;
    let nonce_bytes = STANDARD.decode(nonce_b64).context("bad nonce base64")?;
    let ciphertext = STANDARD.decode(ct_b64).context("bad ciphertext base64")?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| anyhow!("decrypt failed: {e}"))?;
    String::from_utf8(plaintext).context("decrypted token is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let key = [7u8; 32];
        let token = "abc123-ebird-token";
        let enc = encrypt(&key, token).unwrap();
        assert_ne!(enc, token);
        assert_eq!(decrypt(&key, &enc).unwrap(), token);
    }

    #[test]
    fn wrong_key_fails() {
        let enc = encrypt(&[1u8; 32], "secret").unwrap();
        assert!(decrypt(&[2u8; 32], &enc).is_err());
    }
}
