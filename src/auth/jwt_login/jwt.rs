//! Minimal, signature-unverified JWT inspection used to schedule re-login.

use base64::Engine;
use serde_json::Value;

/// Read `exp` from a JWT payload without verifying the signature — we are
/// the client, not the validator. Opaque (non-JWT) tokens return `None` and
/// get no proactive refresh, only the 401-reactive path.
pub fn decode_jwt_exp(token: &str) -> Option<u64> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims.get("exp")?.as_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_exp_claim() {
        // header {"alg":"none"} . payload {"exp":1900000000,"sub":"u"} . empty sig
        let engine = &base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = engine.encode(br#"{"alg":"none"}"#);
        let payload = engine.encode(br#"{"exp":1900000000,"sub":"u"}"#);
        let token = format!("{header}.{payload}.");
        assert_eq!(decode_jwt_exp(&token), Some(1_900_000_000));
    }

    #[test]
    fn opaque_token_has_no_expiry() {
        assert_eq!(decode_jwt_exp("not-a-jwt"), None);
        assert_eq!(decode_jwt_exp("a.b.c"), None);
    }
}
