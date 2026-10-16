//! Offline license (Ed25519 signature) — foundation of the monetization.
//!
//! Model: the binary carries only the vendor's PUBLIC KEY. A license is
//! a token `DF1.<payload-b64url>.<assinatura-b64url>`, where the signature
//! covers the EXACT bytes of the payload. Verification is 100% local — no
//! network calls (the "offline-first CLI" promise survives intact).
//!
//! Honesty about the deal: this is an **anti-honest** gate, not an
//! anti-attacker one — whoever wants to patch the binary can. It is the same deal
//! as old TablePlus/Transmit/JetBrains: we monetize honesty +
//! convenience + updates, not cryptography. The real guardian (when
//! there is revenue) is the server; here we are in the $0-cost phase.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Vendor's public key embedded in the binary (base64url, 32 bytes).
/// The private one NEVER enters the repo nor the binary — it stays with the vendor.
const PUBLIC_KEY_B64: &str = "7hOXAKsC-3Jw9_HyJuWD6TSaiZqthHpIekrwqFsc96Q";

/// Token prefix — versions the format for future migration without breaking it.
const TOKEN_PREFIX: &str = "DF1";

/// Semantic content of the license (this is what the JSON payload carries).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LicensePayload {
    pub plan: String,
    pub holder: String,
    pub issued: String,
    pub expires: String,
}

#[derive(Debug, PartialEq)]
pub enum LicenseError {
    /// Token outside the `DF1.<payload>.<assinatura>` format.
    Format,
    /// Invalid base64.
    Encoding,
    /// Payload is not valid JSON.
    Json,
    /// Signature does not check out (tampered payload or wrong key).
    Signature,
    /// Valid token, but expired (`YYYY-MM-DD`).
    Expired(String),
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenseError::Format => write!(f, "formato inválido (esperado DF1.<payload>.<assinatura>)"),
            LicenseError::Encoding => write!(f, "base64 inválido"),
            LicenseError::Json => write!(f, "payload não é JSON válido"),
            LicenseError::Signature => write!(f, "assinatura não confere"),
            LicenseError::Expired(date) => write!(f, "licença vencida em {date}"),
        }
    }
}

fn b64u_decode(text: &str) -> Result<Vec<u8>, LicenseError> {
    URL_SAFE_NO_PAD.decode(text).map_err(|_| LicenseError::Encoding)
}

/// Verifies a token against the public key embedded in the binary.
pub fn verify_token(token: &str) -> Result<LicensePayload, LicenseError> {
    let pk = embedded_public_key()?;
    verify_token_with(token, &pk)
}

/// Embedded public key, already decoded.
pub fn embedded_public_key() -> Result<[u8; 32], LicenseError> {
    let bytes = b64u_decode(PUBLIC_KEY_B64)?;
    bytes
        .try_into()
        .map_err(|_| LicenseError::Encoding)
}

/// Core of the verification (split out so tests can use another key).
fn verify_token_with(token: &str, public_key: &[u8; 32]) -> Result<LicensePayload, LicenseError> {
    let parts: Vec<&str> = token.trim().split('.').collect();
    if parts.len() != 3 || parts[0] != TOKEN_PREFIX {
        return Err(LicenseError::Format);
    }

    let payload_bytes = b64u_decode(parts[1])?;
    let signature_bytes = b64u_decode(parts[2])?;
    let signature: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| LicenseError::Format)?;

    // 1) Signature FIRST, over the raw bytes of the payload.
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| LicenseError::Signature)?;
    key.verify(&payload_bytes, &Signature::from_bytes(&signature))
        .map_err(|_| LicenseError::Signature)?;

    // 2) Only then interpret the content.
    let payload: LicensePayload =
        serde_json::from_slice(&payload_bytes).map_err(|_| LicenseError::Json)?;

    // 3) Validity by date (lexicographic comparison of ISO works).
    if payload.expires < crate::today_iso() {
        return Err(LicenseError::Expired(payload.expires));
    }

    Ok(payload)
}

/// Path of the installed license file (XDG on Linux).
pub fn license_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("devforge").join("license.key")
}

/// Reads the installed license (if any), without verifying.
pub fn load_installed() -> Option<String> {
    std::fs::read_to_string(license_file())
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// Installs (writes) an already verified license.
pub fn install(token: &str) -> Result<PathBuf, std::io::Error> {
    let path = license_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, format!("{token}\n"))?;
    Ok(path)
}

/// Removes the installed license. Returns `true` if it existed.
pub fn remove() -> bool {
    std::fs::remove_file(license_file()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::{Signer, SigningKey};

    /// Mints a token with the test key (same format as the vendor's).
    fn mint(signing: &SigningKey, payload: &LicensePayload) -> String {
        let bytes = serde_json::to_vec(payload).unwrap();
        let signature = signing.sign(&bytes);
        format!(
            "DF1.{}.{}",
            URL_SAFE_NO_PAD.encode(&bytes),
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        )
    }

    fn payload(expires: &str) -> LicensePayload {
        LicensePayload {
            plan: "team".into(),
            holder: "acme".into(),
            issued: "2026-01-01".into(),
            expires: expires.into(),
        }
    }

    #[test]
    fn valid_token_is_accepted() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let token = mint(&sk, &payload("2099-01-01"));
        let pk = sk.verifying_key().to_bytes();
        let got = verify_token_with(&token, &pk).expect("deveria validar");
        assert_eq!(got.plan, "team");
    }

    #[test]
    fn tampered_payload_is_rejected() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let token = mint(&sk, &payload("2099-01-01"));
        let pk = sk.verifying_key().to_bytes();

        // swaps the payload for a "lifetime enterprise" one WITHOUT re-signing
        let forged = serde_json::to_vec(&payload("2999-12-31")).unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        let tampered = format!(
            "DF1.{}.{}",
            URL_SAFE_NO_PAD.encode(&forged),
            parts[2]
        );
        assert_eq!(verify_token_with(&tampered, &pk), Err(LicenseError::Signature));
    }

    #[test]
    fn expired_license_is_rejected() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let token = mint(&sk, &payload("2020-01-01"));
        let pk = sk.verifying_key().to_bytes();
        assert_eq!(
            verify_token_with(&token, &pk),
            Err(LicenseError::Expired("2020-01-01".into()))
        );
    }

    #[test]
    fn wrong_key_is_rejected() {
        let issuer = SigningKey::from_bytes(&[7u8; 32]);
        let other = SigningKey::from_bytes(&[9u8; 32]);
        let token = mint(&issuer, &payload("2099-01-01"));
        let pk = other.verifying_key().to_bytes();
        assert_eq!(verify_token_with(&token, &pk), Err(LicenseError::Signature));
    }

    #[test]
    fn garbage_and_wrong_prefix_are_rejected() {
        let pk = [0u8; 32];
        assert_eq!(verify_token_with("qualquer coisa", &pk), Err(LicenseError::Format));
        assert_eq!(verify_token_with("XX1.a.b", &pk), Err(LicenseError::Format));
        assert_eq!(verify_token_with("DF1.a.b", &pk), Err(LicenseError::Encoding));
    }
}
