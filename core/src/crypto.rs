//! Cryptographic envelope.
//!
//! Design (see docs/01 §6, deviation D-1):
//! * master key  = Argon2id(passphrase, salt)  → 32 bytes, held only while the app is unlocked
//! * data key    = random 32 bytes per record, wrapped with AES-256-GCM under the master key
//! * field value = AES-256-GCM(data key, random nonce) ‖ tag
//! * documents   = XChaCha20-Poly1305 with a per-document key (24-byte nonce, no nonce reuse risk)
//! * search      = HMAC-SHA256 blind index over the normalised value (no plaintext at rest)
//!
//! Every ciphertext blob starts with a one-byte format version so keys and algorithms can be
//! rotated later without guessing what an old blob contained.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use unicode_normalization::UnicodeNormalization;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::{CoreError, Result};

type HmacSha256 = Hmac<Sha256>;

pub const FORMAT_V1: u8 = 1;
const GCM_NONCE: usize = 12;
const GCM_TAG: usize = 16;
const XCHACHA_NONCE: usize = 24;
const XCHACHA_TAG: usize = 16;
pub const SALT_LEN: usize = 16;

fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).map_err(|e| CoreError::Crypto(format!("rng: {e}")))?;
    Ok(buf)
}

pub fn random_salt() -> Result<[u8; SALT_LEN]> {
    random_bytes::<SALT_LEN>()
}

/// Argon2id parameters. Stored next to the key so a future parameter change stays decryptable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// Production parameters: 64 MiB, 3 iterations, single lane (modest hardware).
    pub fn standard() -> Self {
        KdfParams { m_cost_kib: 65_536, t_cost: 3, p_cost: 1 }
    }

    /// Test parameters. Never used for real data; kept explicit so tests stay fast.
    pub fn for_tests() -> Self {
        KdfParams { m_cost_kib: 8, t_cost: 1, p_cost: 1 }
    }
}

impl Default for KdfParams {
    fn default() -> Self {
        Self::standard()
    }
}

/// 32-byte symmetric key that zeroises on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey(pub [u8; 32]);

impl SecretKey {
    pub fn generate() -> Result<Self> {
        Ok(SecretKey(random_bytes::<32>()?))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        SecretKey(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

/// Derive the master key from a passphrase.
pub fn derive_master_key(
    passphrase: &str,
    salt: &[u8; SALT_LEN],
    params: KdfParams,
) -> Result<SecretKey> {
    if passphrase.chars().count() < 12 {
        return Err(CoreError::Validation(
            "passphrase must be at least 12 characters".into(),
        ));
    }
    let argon_params = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(32))
        .map_err(|e| CoreError::Crypto(format!("argon2 params: {e}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut out = [0u8; 32];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut out)
        .map_err(|e| CoreError::Crypto(format!("argon2: {e}")))?;
    Ok(SecretKey(out))
}

/// AES-256-GCM seal: `version ‖ nonce ‖ ciphertext ‖ tag`.
pub fn seal(key: &SecretKey, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let nonce_bytes = random_bytes::<GCM_NONCE>()?;
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
        .map_err(|e| CoreError::Crypto(format!("key: {e}")))?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), Payload { msg: plaintext, aad })
        .map_err(|_| CoreError::Crypto("seal failed".into()))?;
    let mut out = Vec::with_capacity(1 + GCM_NONCE + ciphertext.len());
    out.push(FORMAT_V1);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn open(key: &SecretKey, blob: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < 1 + GCM_NONCE + GCM_TAG {
        return Err(CoreError::Crypto("ciphertext too short".into()));
    }
    match blob[0] {
        FORMAT_V1 => {}
        v => return Err(CoreError::Crypto(format!("unknown format version {v}"))),
    }
    let nonce = &blob[1..1 + GCM_NONCE];
    let body = &blob[1 + GCM_NONCE..];
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
        .map_err(|e| CoreError::Crypto(format!("key: {e}")))?;
    cipher
        .decrypt(Nonce::from_slice(nonce), Payload { msg: body, aad })
        .map_err(|_| CoreError::Crypto("decryption failed (wrong key or tampered data)".into()))
}

/// Per-document encryption with XChaCha20-Poly1305.
pub fn seal_document(key: &SecretKey, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let nonce_bytes = random_bytes::<XCHACHA_NONCE>()?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|e| CoreError::Crypto(format!("key: {e}")))?;
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce_bytes), Payload { msg: plaintext, aad })
        .map_err(|_| CoreError::Crypto("document seal failed".into()))?;
    let mut out = Vec::with_capacity(1 + XCHACHA_NONCE + ciphertext.len());
    out.push(FORMAT_V1);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn open_document(key: &SecretKey, blob: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < 1 + XCHACHA_NONCE + XCHACHA_TAG {
        return Err(CoreError::Crypto("ciphertext too short".into()));
    }
    if blob[0] != FORMAT_V1 {
        return Err(CoreError::Crypto("unknown format version".into()));
    }
    let nonce = &blob[1..1 + XCHACHA_NONCE];
    let body = &blob[1 + XCHACHA_NONCE..];
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|e| CoreError::Crypto(format!("key: {e}")))?;
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: body, aad })
        .map_err(|_| CoreError::Crypto("document decryption failed".into()))
}

/// Wrap a record data key with the master key.
pub fn wrap_key(master: &SecretKey, data_key: &SecretKey) -> Result<Vec<u8>> {
    seal(master, data_key.as_bytes(), b"notaire:dek:v1")
}

pub fn unwrap_key(master: &SecretKey, wrapped: &[u8]) -> Result<SecretKey> {
    let bytes = open(master, wrapped, b"notaire:dek:v1")?;
    if bytes.len() != 32 {
        return Err(CoreError::Crypto("bad data key length".into()));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(SecretKey(key))
}

/// Arabic/Latin normalisation used before hashing into a blind index and before search.
///
/// Unifies alef variants, ya/alef-maqsura, ta-marbuta, removes diacritics and tatweel,
/// case-folds Latin text and collapses whitespace. Deliberately conservative: it must not
/// merge genuinely different names.
pub fn normalize_name(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.nfkc() {
        let mapped = match ch {
            '\u{064B}'..='\u{0652}' | '\u{0670}' | '\u{0640}' | '\u{0653}'..='\u{065F}' => continue,
            '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0671}' => '\u{0627}', // آ أ إ ٱ → ا
            '\u{0649}' => '\u{064A}',                                          // ى → ي
            '\u{0629}' => '\u{0647}',                                          // ة → ه
            '\u{0624}' => '\u{0648}',                                          // ؤ → و
            '\u{0626}' => '\u{064A}',                                          // ئ → ي
            // Punctuation inside names is not significant: "Ben-Ali" and "BEN ALI" are one person.
            '-' | '.' | '_' | '\u{2010}'..='\u{2015}' => ' ',
            c => c,
        };
        for lower in mapped.to_lowercase() {
            out.push(lower);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Deterministic blind index: same normalised value → same 32-byte tag, no recoverable plaintext.
pub fn blind_index(index_key: &SecretKey, value: &str) -> [u8; 32] {
    let normalised = normalize_name(value);
    let mut mac = <HmacSha256 as Mac>::new_from_slice(index_key.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(b"notaire:bidx:v1");
    mac.update(normalised.as_bytes());
    let out = mac.finalize().into_bytes();
    let mut tag = [0u8; 32];
    tag.copy_from_slice(&out);
    tag
}

pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Ed25519 keys for signing rule packages and update packages.
pub struct SigningIdentity {
    pub public: [u8; 32],
    secret: [u8; 32],
}

impl SigningIdentity {
    pub fn generate() -> Result<Self> {
        let secret = random_bytes::<32>()?;
        let sk = ed25519_dalek::SigningKey::from_bytes(&secret);
        Ok(SigningIdentity { public: sk.verifying_key().to_bytes(), secret })
    }

    pub fn from_secret(secret: [u8; 32]) -> Self {
        let sk = ed25519_dalek::SigningKey::from_bytes(&secret);
        SigningIdentity { public: sk.verifying_key().to_bytes(), secret }
    }

    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        let sk = ed25519_dalek::SigningKey::from_bytes(&self.secret);
        sk.sign(message).to_bytes()
    }

    pub fn verify(public: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
        use ed25519_dalek::Verifier;
        match ed25519_dalek::VerifyingKey::from_bytes(public) {
            Ok(vk) => vk
                .verify(message, &ed25519_dalek::Signature::from_bytes(signature))
                .is_ok(),
            Err(_) => false,
        }
    }
}

impl Drop for SigningIdentity {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-answer test: AES-256-GCM, NIST GCM test case 16.
    #[test]
    fn aes_gcm_nist_kat() {
        let key_hex = "feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308";
        let iv = hex::decode("cafebabefacedbaddecaf888").unwrap();
        let pt = hex::decode(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39",
        )
        .unwrap();
        let aad = hex::decode("feedfacedeadbeeffeedfacedeadbeefabaddad2").unwrap();
        let expected = hex::decode(
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f66276fc6ece\
             0f4e1768cddf8853bb2d551b",
        )
        .unwrap();

        let mut k = [0u8; 32];
        k.copy_from_slice(&hex::decode(key_hex).unwrap());
        let cipher = Aes256Gcm::new_from_slice(&k).unwrap();
        let ct = cipher
            .encrypt(Nonce::from_slice(&iv), Payload { msg: &pt, aad: &aad })
            .unwrap();
        assert_eq!(ct, expected);
    }

    #[test]
    fn field_roundtrip_and_fresh_nonces() {
        let key = SecretKey::generate().unwrap();
        let a = seal(&key, b"BENALI Mohamed", b"party:1:name").unwrap();
        let b = seal(&key, b"BENALI Mohamed", b"party:1:name").unwrap();
        assert_ne!(a, b, "nonce reuse would make equal ciphertexts");
        assert_eq!(open(&key, &a, b"party:1:name").unwrap(), b"BENALI Mohamed");
    }

    #[test]
    fn wrong_key_and_wrong_aad_fail() {
        let key = SecretKey::generate().unwrap();
        let other = SecretKey::generate().unwrap();
        let blob = seal(&key, b"secret", b"aad").unwrap();
        assert!(open(&other, &blob, b"aad").is_err());
        assert!(open(&key, &blob, b"other-aad").is_err());
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let key = SecretKey::generate().unwrap();
        let mut blob = seal(&key, b"amount=100", b"aad").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(open(&key, &blob, b"aad").is_err());
    }

    #[test]
    fn document_roundtrip() {
        let key = SecretKey::generate().unwrap();
        let blob = seal_document(&key, b"%PDF-1.7 demo", b"doc:abc").unwrap();
        assert_eq!(open_document(&key, &blob, b"doc:abc").unwrap(), b"%PDF-1.7 demo");
    }

    #[test]
    fn key_wrapping_roundtrip() {
        let master = SecretKey::generate().unwrap();
        let dek = SecretKey::generate().unwrap();
        let wrapped = wrap_key(&master, &dek).unwrap();
        let back = unwrap_key(&master, &wrapped).unwrap();
        assert_eq!(back.as_bytes(), dek.as_bytes());
        assert!(unwrap_key(&SecretKey::generate().unwrap(), &wrapped).is_err());
    }

    #[test]
    fn blind_index_normalises_arabic_variants() {
        let key = SecretKey::generate().unwrap();
        assert_eq!(blind_index(&key, "أحمد"), blind_index(&key, "احمد"));
        assert_eq!(blind_index(&key, "فاطمة"), blind_index(&key, "فاطمه"));
        assert_eq!(blind_index(&key, "  مصطفى "), blind_index(&key, "مصطفي"));
        assert_eq!(blind_index(&key, "Benali"), blind_index(&key, "BENALI"));
        assert_eq!(blind_index(&key, "Ben-Ali"), blind_index(&key, "BEN ALI"));
        // Diacritics must not change the index.
        assert_eq!(blind_index(&key, "مُحَمَّد"), blind_index(&key, "محمد"));
        // Different names must not collide.
        assert_ne!(blind_index(&key, "محمد"), blind_index(&key, "أحمد"));
    }

    #[test]
    fn master_key_derivation_is_deterministic_and_salted() {
        let salt = [7u8; SALT_LEN];
        let a = derive_master_key("correct horse battery", &salt, KdfParams::for_tests()).unwrap();
        let b = derive_master_key("correct horse battery", &salt, KdfParams::for_tests()).unwrap();
        let c = derive_master_key("correct horse battery", &[8u8; SALT_LEN], KdfParams::for_tests())
            .unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
        assert_ne!(a.as_bytes(), c.as_bytes());
    }

    #[test]
    fn short_passphrase_refused() {
        let salt = [1u8; SALT_LEN];
        assert!(derive_master_key("short", &salt, KdfParams::for_tests()).is_err());
    }

    #[test]
    fn signing_identity_verifies_and_rejects() {
        let id = SigningIdentity::generate().unwrap();
        let sig = id.sign(b"rules package v3");
        assert!(SigningIdentity::verify(&id.public, b"rules package v3", &sig));
        assert!(!SigningIdentity::verify(&id.public, b"rules package v4", &sig));
    }

    #[test]
    fn debug_does_not_leak_key_material() {
        let key = SecretKey::generate().unwrap();
        assert_eq!(format!("{key:?}"), "SecretKey(<redacted>)");
    }
}
