use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2, Params,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),
    #[error("Encryption failed: {0}")]
    Encryption(String),
    #[error("Decryption or authentication tag verification failed")]
    DecryptionAuthFailed,
    #[error("Invalid encrypted payload format")]
    InvalidPayload,
}

/// Derived symmetric key zeroized automatically on drop to prevent RAM leaks.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct DerivedKey {
    #[zeroize(skip)]
    salt_b64: String,
    key: [u8; KEY_LEN],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptedArchive {
    pub salt_b64: String,
    pub nonce_hex: String,
    pub ciphertext_hex: String,
    pub sha256_hex: String,
}

/// Derives a 256-bit key using Argon2id with hardened memory/time cost parameters.
pub fn derive_key_argon2id(passphrase: &str, existing_salt_b64: Option<&str>) -> Result<DerivedKey, CryptoError> {
    let salt = match existing_salt_b64 {
        Some(s) => SaltString::from_b64(s).map_err(|e| CryptoError::KeyDerivation(e.to_string()))?,
        None => SaltString::generate(&mut OsRng),
    };

    // OWASP-aligned Argon2id parameters: 64 MiB memory, 3 iterations, 1 lane.
    let params = Params::new(65_536, 3, 1, Some(KEY_LEN))
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    let hash = argon2
        .hash_password(passphrase.as_bytes(), &salt)
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

    let raw_output = hash
        .hash
        .ok_or_else(|| CryptoError::KeyDerivation("Missing raw hash output".to_string()))?;

    let mut key = [0u8; KEY_LEN];
    let bytes = raw_output.as_bytes();
    if bytes.len() != KEY_LEN {
        return Err(CryptoError::KeyDerivation("Unexpected key length".to_string()));
    }
    key.copy_from_slice(bytes);

    Ok(DerivedKey {
        salt_b64: salt.to_string(),
        key,
    })
}

/// Encrypts raw backup bytes with AES-256-GCM and computes SHA-256 over the serialized archive.
pub fn encrypt_payload(plaintext: &[u8], passphrase: &str) -> Result<EncryptedArchive, CryptoError> {
    let derived = derive_key_argon2id(passphrase, None)?;
    let cipher = Aes256Gcm::new_from_slice(&derived.key)
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;

    let ciphertext_hex = hex::encode(ciphertext);
    let nonce_hex = hex::encode(nonce_bytes);

    let mut hasher = Sha256::new();
    hasher.update(derived.salt_b64.as_bytes());
    hasher.update(nonce_hex.as_bytes());
    hasher.update(ciphertext_hex.as_bytes());
    let sha256_hex = hex::encode(hasher.finalize());

    Ok(EncryptedArchive {
        salt_b64: derived.salt_b64.clone(),
        nonce_hex,
        ciphertext_hex,
        sha256_hex,
    })
}

/// Verifies integrity & decrypts an AES-256-GCM archive, returning zeroizable plaintext bytes.
pub fn decrypt_payload(archive: &EncryptedArchive, passphrase: &str) -> Result<Vec<u8>, CryptoError> {
    let mut hasher = Sha256::new();
    hasher.update(archive.salt_b64.as_bytes());
    hasher.update(archive.nonce_hex.as_bytes());
    hasher.update(archive.ciphertext_hex.as_bytes());
    let computed_sha = hex::encode(hasher.finalize());

    if computed_sha != archive.sha256_hex {
        return Err(CryptoError::InvalidPayload);
    }

    let derived = derive_key_argon2id(passphrase, Some(&archive.salt_b64))?;
    let cipher = Aes256Gcm::new_from_slice(&derived.key)
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;

    let nonce_bytes = hex::decode(&archive.nonce_hex).map_err(|_| CryptoError::InvalidPayload)?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(CryptoError::InvalidPayload);
    }
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = hex::decode(&archive.ciphertext_hex).map_err(|_| CryptoError::InvalidPayload)?;
    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| CryptoError::DecryptionAuthFailed)
}

/// Computes SHA-256 hex digest of a byte slice for pre-flight restore validation.
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}