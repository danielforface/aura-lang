/// Ed25519 cryptographic signing for package authenticity
/// Enables secure package verification and Enterprise adoption

use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use hex::{encode as hex_encode, decode as hex_decode};
use std::fs;
use std::path::Path;
use miette::{IntoDiagnostic, Report};
use base64::Engine;
use rand::thread_rng;

pub type SigningError = Report;

fn signing_msg(message: impl Into<String>) -> SigningError {
    Report::msg(message.into())
}

/// Ed25519 signing key (secret)
#[derive(Clone)]
pub struct PackageSigningKey {
    inner: SigningKey,
}

/// Ed25519 verifying key (public)
#[derive(Clone, Debug)]
pub struct PackageVerifyingKey {
    inner: VerifyingKey,
}

impl PackageSigningKey {
    /// Generate a new random signing key
    /// Returns (signing_key, verifying_key, hex_string)
    pub fn generate() -> (Self, PackageVerifyingKey, String) {
        let mut rng = thread_rng();
        let mut secret_bytes = [0u8; 32];
        use rand::RngCore;
        rng.fill_bytes(&mut secret_bytes);
        
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();
        let hex = hex_encode(verifying_key.as_bytes());

        (
            PackageSigningKey { inner: signing_key },
            PackageVerifyingKey { inner: verifying_key },
            hex,
        )
    }

    /// Load signing key from hex-encoded bytes
    pub fn from_hex(hex: &str) -> Result<Self, SigningError> {
        let bytes = hex_decode(hex)
            .map_err(|e| signing_msg(format!("invalid hex encoding: {e}")))?;

        if bytes.len() != 32 {
            return Err(signing_msg(format!(
                "signing key must be 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);

        let signing_key = SigningKey::from_bytes(&key_bytes);
        Ok(PackageSigningKey { inner: signing_key })
    }

    /// Load signing key from file (32-byte binary)
    pub fn from_file(path: &Path) -> Result<Self, SigningError> {
        let bytes = fs::read(path)
            .into_diagnostic()
            .map_err(|e| signing_msg(format!("failed to read key file: {e}")))?;

        if bytes.len() != 32 {
            return Err(signing_msg(format!(
                "key file must contain exactly 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);

        let signing_key = SigningKey::from_bytes(&key_bytes);
        Ok(PackageSigningKey { inner: signing_key })
    }

    /// Save signing key to file (32-byte binary)
    /// ⚠️ WARNING: This is your secret key! Keep it safe!
    pub fn save_to_file(&self, path: &Path) -> Result<(), SigningError> {
        fs::write(path, self.inner.as_bytes())
            .into_diagnostic()
            .map_err(|e| signing_msg(format!("failed to write key file: {e}")))
    }

    /// Get the corresponding public verifying key
    pub fn verifying_key(&self) -> PackageVerifyingKey {
        PackageVerifyingKey {
            inner: self.inner.verifying_key(),
        }
    }

    /// Sign data and return base64-encoded signature
    pub fn sign_data(&self, data: &[u8]) -> String {
        let signature = self.inner.sign(data);
        base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
    }

    /// Sign a SHA256 hash (hex string) and return base64-encoded signature
    pub fn sign_sha256_hash(&self, hash_hex: &str) -> Result<String, SigningError> {
        if hash_hex.len() != 64 {
            return Err(signing_msg(format!(
                "SHA256 hash must be 64 hex chars, got {}",
                hash_hex.len()
            )));
        }

        let hash_bytes = hex_decode(hash_hex)
            .map_err(|e| signing_msg(format!("invalid hash hex: {e}")))?;

        let signature = self.inner.sign(&hash_bytes);
        Ok(base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()))
    }
}

impl PackageVerifyingKey {
    /// Load verifying key from hex-encoded bytes (public key)
    pub fn from_hex(hex: &str) -> Result<Self, SigningError> {
        let bytes = hex_decode(hex)
            .map_err(|e| signing_msg(format!("invalid hex encoding: {e}")))?;

        if bytes.len() != 32 {
            return Err(signing_msg(format!(
                "public key must be 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);

        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|e| signing_msg(format!("invalid public key: {e}")))?;

        Ok(PackageVerifyingKey { inner: verifying_key })
    }

    /// Load verifying key from file (32-byte binary)
    pub fn from_file(path: &Path) -> Result<Self, SigningError> {
        let bytes = fs::read(path)
            .into_diagnostic()
            .map_err(|e| signing_msg(format!("failed to read key file: {e}")))?;

        Self::from_hex(&hex_encode(&bytes))
    }

    /// Get hex-encoded representation of public key
    pub fn to_hex(&self) -> String {
        hex_encode(self.inner.as_bytes())
    }

    /// Verify a signature over raw data
    pub fn verify_data(&self, data: &[u8], signature_b64: &str) -> Result<(), SigningError> {
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(signature_b64)
            .map_err(|e| signing_msg(format!("invalid base64 signature: {e}")))?;

        if sig_bytes.len() != 64 {
            return Err(signing_msg(format!(
                "signature must be 64 bytes, got {}",
                sig_bytes.len()
            )));
        }

        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_array);

        self.inner
            .verify(data, &signature)
            .map_err(|e| signing_msg(format!("signature verification failed: {e}")))
    }

    /// Verify a signature over a SHA256 hash (hex string)
    pub fn verify_sha256_hash(&self, hash_hex: &str, signature_b64: &str) -> Result<(), SigningError> {
        if hash_hex.len() != 64 {
            return Err(signing_msg(format!(
                "SHA256 hash must be 64 hex chars, got {}",
                hash_hex.len()
            )));
        }

        let hash_bytes = hex_decode(hash_hex)
            .map_err(|e| signing_msg(format!("invalid hash hex: {e}")))?;

        self.verify_data(&hash_bytes, signature_b64)
    }
}

/// Complete package signature with metadata
#[derive(Clone, Debug)]
pub struct PackageSignature {
    /// Base64-encoded Ed25519 signature
    pub signature: String,
    /// Hex-encoded SHA256 hash of the package
    pub package_hash: String,
    /// ID of the key used (for key rotation)
    pub key_id: String,
    /// Timestamp of signature (ISO 8601)
    pub timestamp: String,
}

impl PackageSignature {
    /// Create a new package signature
    pub fn new(
        signature: String,
        package_hash: String,
        key_id: String,
    ) -> Self {
        let timestamp = chrono::Utc::now().to_rfc3339();
        PackageSignature {
            signature,
            package_hash,
            key_id,
            timestamp,
        }
    }

    /// Serialize signature to JSON
    pub fn to_json(&self) -> Result<String, SigningError> {
        serde_json::to_string_pretty(self)
            .into_diagnostic()
            .map_err(|e| signing_msg(format!("failed to serialize signature: {e}")))
    }

    /// Deserialize signature from JSON
    pub fn from_json(json: &str) -> Result<Self, SigningError> {
        serde_json::from_str(json)
            .into_diagnostic()
            .map_err(|e| signing_msg(format!("failed to parse signature JSON: {e}")))
    }
}

use serde::{Deserialize, Serialize};

impl Serialize for PackageSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("PackageSignature", 4)?;
        state.serialize_field("signature", &self.signature)?;
        state.serialize_field("package_hash", &self.package_hash)?;
        state.serialize_field("key_id", &self.key_id)?;
        state.serialize_field("timestamp", &self.timestamp)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for PackageSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct PackageSignatureVisitor;

        impl<'de> Visitor<'de> for PackageSignatureVisitor {
            type Value = PackageSignature;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct PackageSignature")
            }

            fn visit_map<V>(self, mut map: V) -> Result<PackageSignature, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut signature = None;
                let mut package_hash = None;
                let mut key_id = None;
                let mut timestamp = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "signature" => {
                            if signature.is_some() {
                                return Err(de::Error::duplicate_field("signature"));
                            }
                            signature = Some(map.next_value()?);
                        }
                        "package_hash" => {
                            if package_hash.is_some() {
                                return Err(de::Error::duplicate_field("package_hash"));
                            }
                            package_hash = Some(map.next_value()?);
                        }
                        "key_id" => {
                            if key_id.is_some() {
                                return Err(de::Error::duplicate_field("key_id"));
                            }
                            key_id = Some(map.next_value()?);
                        }
                        "timestamp" => {
                            if timestamp.is_some() {
                                return Err(de::Error::duplicate_field("timestamp"));
                            }
                            timestamp = Some(map.next_value()?);
                        }
                        _ => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                Ok(PackageSignature {
                    signature: signature.ok_or_else(|| de::Error::missing_field("signature"))?,
                    package_hash: package_hash.ok_or_else(|| de::Error::missing_field("package_hash"))?,
                    key_id: key_id.ok_or_else(|| de::Error::missing_field("key_id"))?,
                    timestamp: timestamp.unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
                })
            }
        }

        deserializer.deserialize_struct("PackageSignature", &["signature", "package_hash", "key_id", "timestamp"], PackageSignatureVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation_and_verification() {
        let (signing_key, verifying_key, _hex) = PackageSigningKey::generate();
        
        let data = b"test data";
        let signature = signing_key.sign_data(data);
        
        assert!(verifying_key.verify_data(data, &signature).is_ok());
    }

    #[test]
    fn test_sha256_hash_signing() {
        use sha2::Sha256;
        use sha2::Digest;
        
        let (signing_key, verifying_key, _hex) = PackageSigningKey::generate();
        
        let mut hasher = Sha256::new();
        hasher.update(b"package content");
        let hash = hex_encode(hasher.finalize());
        
        let signature = signing_key.sign_sha256_hash(&hash).expect("sign failed");
        assert!(verifying_key.verify_sha256_hash(&hash, &signature).is_ok());
    }

    #[test]
    fn test_invalid_signature_rejected() {
        let (_signing_key, verifying_key, _hex) = PackageSigningKey::generate();
        let (other_signing_key, _, _) = PackageSigningKey::generate();
        
        let data = b"test data";
        let wrong_signature = other_signing_key.sign_data(data);
        
        assert!(verifying_key.verify_data(data, &wrong_signature).is_err());
    }

    #[test]
    fn test_hex_serialization() {
        let (signing_key, _verifying_key, hex) = PackageSigningKey::generate();
        
        let loaded = PackageVerifyingKey::from_hex(&hex).expect("load failed");
        
        let data = b"test";
        let signature = signing_key.sign_data(data);
        assert!(loaded.verify_data(data, &signature).is_ok());
    }

    #[test]
    fn test_signature_tamper_detection() {
        let (signing_key, verifying_key, _hex) = PackageSigningKey::generate();
        
        let data = b"original data";
        let signature = signing_key.sign_data(data);
        
        // Try to verify with different data
        let tampered = b"tampered data";
        assert!(verifying_key.verify_data(tampered, &signature).is_err());
    }
}
