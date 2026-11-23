//! AES-256-GCM archive encryption with an Argon2id-derived key (archive format v2).
//!
//! Format v2 binds the complete header (version, KDF parameters, salt, nonce, device, platform,
//! mode, timestamp) to the ciphertext as AEAD associated data, so any header tampering fails
//! authentication. The previous unkeyed SHA-256 "integrity" field was removed: it could be
//! recomputed by an attacker and only duplicated the GCM tag.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const ARCHIVE_VERSION: u8 = 2;
/// Minimum passphrase length (Unicode scalar values), enforced by the backend.
pub const MIN_PASSPHRASE_CHARS: usize = 12;

const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;
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
    #[error("Unsupported archive version {0}")]
    UnsupportedVersion(u8),
    // Keep the literal in sync with MIN_PASSPHRASE_CHARS.
    #[error("Passphrase must be at least 12 characters")]
    WeakPassphrase,
}

/// Derived symmetric key, overwritten with zeros on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct DerivedKey {
    key: [u8; KEY_LEN],
}

/// Argon2 parameters stored in every archive so they can be raised later without breaking old files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub alg: String,
    /// Memory cost in KiB.
    pub m_kib: u32,
    /// Iterations.
    pub t: u32,
    /// Parallelism (lanes).
    pub p: u32,
}

impl KdfParams {
    /// OWASP-aligned Argon2id: 64 MiB, 3 iterations, 1 lane.
    pub fn current() -> Self {
        Self {
            alg: "argon2id".to_string(),
            m_kib: 65_536,
            t: 3,
            p: 1,
        }
    }

    /// Rejects parameters outside sane bounds before any expensive work is done
    /// (prevents memory/CPU exhaustion through a crafted archive header).
    fn validate(&self) -> Result<(), CryptoError> {
        let ok = self.alg == "argon2id"
            && (19_456..=1_048_576).contains(&self.m_kib)
            && (1..=10).contains(&self.t)
            && (1..=4).contains(&self.p);
        if ok {
            Ok(())
        } else {
            Err(CryptoError::InvalidPayload)
        }
    }
}

/// Authenticated (but not secret) metadata describing an archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveHeader {
    pub version: u8,
    pub kdf: KdfParams,
    pub salt_hex: String,
    pub nonce_hex: String,
    pub device_id: String,
    pub platform: String,
    pub mode: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedArchive {
    pub header: ArchiveHeader,
    pub ciphertext_hex: String,
}

/// Caller-provided metadata that gets bound to the archive.
pub struct ArchiveMeta<'a> {
    pub device_id: &'a str,
    pub platform: &'a str,
    pub mode: &'a str,
}

fn validate_passphrase(passphrase: &str) -> Result<(), CryptoError> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(CryptoError::WeakPassphrase);
    }
    Ok(())
}

/// Canonical AEAD associated data: the serialized header (field order is fixed by the struct).
fn header_aad(header: &ArchiveHeader) -> Result<Vec<u8>, CryptoError> {
    serde_json::to_vec(header).map_err(|_| CryptoError::InvalidPayload)
}

/// Derives a 256-bit key with Argon2id directly into the zeroizing key buffer.
pub fn derive_key_argon2id(
    passphrase: &str,
    salt: &[u8],
    kdf: &KdfParams,
) -> Result<DerivedKey, CryptoError> {
    let params = Params::new(kdf.m_kib, kdf.t, kdf.p, Some(KEY_LEN))
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut derived = DerivedKey {
        key: [0u8; KEY_LEN],
    };
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut derived.key)
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
    Ok(derived)
}

/// Encrypts raw backup bytes with AES-256-GCM. Fresh random salt and nonce per archive.
pub fn encrypt_payload(
    plaintext: &[u8],
    passphrase: &str,
    meta: &ArchiveMeta<'_>,
) -> Result<EncryptedArchive, CryptoError> {
    validate_passphrase(passphrase)?;

    let kdf = KdfParams::current();
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let derived = derive_key_argon2id(passphrase, &salt, &kdf)?;
    let cipher = Aes256Gcm::new_from_slice(&derived.key)
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;

    let header = ArchiveHeader {
        version: ARCHIVE_VERSION,
        kdf,
        salt_hex: hex::encode(salt),
        nonce_hex: hex::encode(nonce_bytes),
        device_id: meta.device_id.to_string(),
        platform: meta.platform.to_string(),
        mode: meta.mode.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let aad = header_aad(&header)?;

    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;

    Ok(EncryptedArchive {
        header,
        ciphertext_hex: hex::encode(ciphertext),
    })
}

/// Authenticates and decrypts an archive. The returned plaintext is zeroized on drop.
pub fn decrypt_payload(
    archive: &EncryptedArchive,
    passphrase: &str,
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let header = &archive.header;
    if header.version != ARCHIVE_VERSION {
        return Err(CryptoError::UnsupportedVersion(header.version));
    }
    header.kdf.validate()?;

    let salt = hex::decode(&header.salt_hex).map_err(|_| CryptoError::InvalidPayload)?;
    if salt.len() != SALT_LEN {
        return Err(CryptoError::InvalidPayload);
    }
    let nonce_bytes = hex::decode(&header.nonce_hex).map_err(|_| CryptoError::InvalidPayload)?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(CryptoError::InvalidPayload);
    }
    let ciphertext =
        hex::decode(&archive.ciphertext_hex).map_err(|_| CryptoError::InvalidPayload)?;

    let aad = header_aad(header)?;
    let derived = derive_key_argon2id(passphrase, &salt, &header.kdf)?;
    let cipher = Aes256Gcm::new_from_slice(&derived.key)
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;

    cipher
        .decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: &ciphertext[..],
                aad: &aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::DecryptionAuthFailed)
}

/// SHA-256 hex digest of a byte slice (used for whole-file archive hashes).
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
