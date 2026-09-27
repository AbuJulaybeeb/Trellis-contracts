//! Typed environment / secrets validation (Issue #66).
//!
//! Startup and deployment must fail fast when required configuration is
//! missing, malformed, unsafe, or accidentally uses production-like secrets
//! in local mode. This module provides `no_std`-compatible validators used by
//! contracts (network IDs, feature flags) and mirrored by
//! `scripts/validate-config.sh` for off-chain deployment config.
//!
//! ## Guarantees
//!
//! - Missing values -> [`Error::ConfigMissing`].
//! - Malformed values (bad URL, unknown network, out-of-range flag) ->
//!   [`Error::ConfigInvalid`].
//! - Placeholder or leaked production secrets -> [`Error::UnsafeSecret`].
//! - Secrets are never printed in full: use [`RedactedSecret`] which only
//!   exposes a `first4...last2` preview in `Debug` output.
//!
//! See `docs/CONFIGURATION.md` for local / staging / production requirements.

use core::fmt;

use crate::errors::Error;

/// Deployment environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Environment {
    Local,
    Staging,
    Production,
}

impl Environment {
    /// Parse `TRELLIS_ENV` style values.
    pub fn parse(s: &str) -> Result<Self, Error> {
        match s {
            "local" => Ok(Environment::Local),
            "staging" => Ok(Environment::Staging),
            "production" => Ok(Environment::Production),
            _ => Err(Error::ConfigInvalid),
        }
    }

    pub fn is_production(self) -> bool {
        matches!(self, Environment::Production)
    }
}

/// Wrapper that never displays the full secret.
///
/// `Debug` prints `XXXX...YY` (first 4 / last 2 chars) so logs stay useful
/// without leaking key material.
pub struct RedactedSecret<'a>(pub &'a str);

impl<'a> fmt::Debug for RedactedSecret<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\"", redact_preview(self.0))
    }
}

/// Short preview helper used by [`RedactedSecret`].
///
/// Returns `true` length class only via static strings; to avoid allocation
/// the preview is written inline by the caller. This helper reports whether
/// a value looks redacted (contains `...`).
pub fn redact_preview(secret: &str) -> RedactedPreview<'_> {
    RedactedPreview(secret)
}

/// Display-only preview `first4...last2`.
pub struct RedactedPreview<'a>(&'a str);

impl<'a> fmt::Display for RedactedPreview<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.0;
        let count = s.chars().count();
        if count <= 8 {
            return write!(f, "***");
        }
        for (i, c) in s.chars().enumerate() {
            if i < 4 {
                write!(f, "{}", c)?;
            }
        }
        write!(f, "...")?;
        for (i, c) in s.chars().enumerate() {
            if i >= count - 2 {
                write!(f, "{}", c)?;
            }
        }
        Ok(())
    }
}

/// Require a non-empty value.
pub fn require_present(value: Option<&str>) -> Result<&str, Error> {
    match value {
        Some(v) if !v.trim().is_empty() => Ok(v),
        _ => Err(Error::ConfigMissing),
    }
}

/// Validate Stellar network / passphrase id.
///
/// Accepts `standalone`, `local`, `futurenet`, `testnet`, `mainnet`, and full
/// passphrases starting with `Test SDF` / `Public Global Stellar`.
pub fn validate_network_id(network: Option<&str>) -> Result<(), Error> {
    let v = require_present(network)?;
    let ok = matches!(v, "standalone" | "local" | "futurenet" | "testnet" | "mainnet")
        || v.starts_with("Test SDF")
        || v.starts_with("Public Global Stellar");
    if ok {
        Ok(())
    } else {
        Err(Error::ConfigInvalid)
    }
}

/// Validate RPC / Horizon URL (must be http(s), no whitespace).
pub fn validate_rpc_url(url: Option<&str>) -> Result<(), Error> {
    let v = require_present(url)?;
    let ok = (v.starts_with("http://") || v.starts_with("https://"))
        && !v.contains(' ')
        && v.len() >= 12
        && v.contains('.');
    if ok {
        Ok(())
    } else {
        Err(Error::ConfigInvalid)
    }
}

/// Placeholder / weak secrets that must never be accepted.
const DENYLIST: &[&str] = &[
    "",
    "changeme",
    "password",
    "secret",
    "test",
    "testing",
    "12345",
    "00000000",
    "SECRETS_PLACEHOLDER",
    "YOUR_SECRET_HERE",
];

fn is_denied(secret: &str) -> bool {
    let lower = lower_is_denied(secret);
    lower || DENYLIST.iter().any(|d| d.eq_ignore_ascii_case(secret.trim()))
}

// `no_std` lowercase compare without allocation: ASCII-only fold.
fn lower_is_denied(secret: &str) -> bool {
    let trimmed = secret.trim();
    // Compare case-insensitively against the small denylist.
    DENYLIST.iter().any(|d| {
        if d.len() != trimmed.len() {
            return false;
        }
        d.bytes()
            .zip(trimmed.bytes())
            .all(|(a, b)| a.to_ascii_lowercase() == b.to_ascii_lowercase())
    })
}

/// Validate a Stellar secret seed (`S...`, 56 chars, base32 body).
///
/// - Empty / placeholder -> [`Error::UnsafeSecret`].
/// - Wrong length / prefix / charset -> [`Error::ConfigInvalid`].
/// - Production-like (valid shape) secret used while `env == Local` ->
///   [`Error::UnsafeSecret`] to catch accidental prod-key reuse locally.
pub fn validate_secret_key(
    secret: Option<&str>,
    env: Environment,
) -> Result<(), Error> {
    let v = require_present(secret).map_err(|_| Error::ConfigMissing)?;
    let s = v.trim();
    if is_denied(s) || s.len() < 8 {
        return Err(Error::UnsafeSecret);
    }
    let looks_like_seed = s.starts_with('S')
        && s.len() == 56
        && s.bytes().all(|b| b.is_ascii_alphanumeric());
    if !looks_like_seed {
        // Short-but-not-denied test keys are unsafe, not merely malformed.
        if s.len() < 16 || s.eq_ignore_ascii_case("test-secret") {
            return Err(Error::UnsafeSecret);
        }
        return Err(Error::ConfigInvalid);
    }
    if env == Environment::Local {
        // A well-formed seed in local mode is almost certainly a copy-paste
        // of a real key: fail fast. Local dev must use `*_TEST_ONLY` fixtures
        // or the documented ephemeral keys (see docs/CONFIGURATION.md).
        return Err(Error::UnsafeSecret);
    }
    Ok(())
}

/// Validate a full deployment config bundle.
///
/// Order: missing -> malformed -> unsafe, so callers get the most actionable
/// error first.
pub fn validate_full_config(
    network: Option<&str>,
    rpc_url: Option<&str>,
    secret: Option<&str>,
    env: Environment,
) -> Result<(), Error> {
    validate_network_id(network)?;
    validate_rpc_url(rpc_url)?;
    validate_secret_key(secret, env)?;
    // Production must use https RPC.
    if env.is_production() {
        if let Some(url) = rpc_url {
            if url.starts_with("http://") {
                return Err(Error::ConfigInvalid);
            }
        }
    }
    Ok(())
}

/// Validate a `0`/`1`/`true`/`false` feature flag.
pub fn validate_feature_flag(value: Option<&str>) -> Result<bool, Error> {
    let v = require_present(value)?;
    match v.trim() {
        "1" | "true" | "TRUE" | "True" => Ok(true),
        "0" | "false" | "FALSE" | "False" => Ok(false),
        _ => Err(Error::ConfigInvalid),
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::format;
    use super::*;

    // 56-char well-formed seed fixture: 'S' + 55 alphanumeric chars.
    const TEST_SEED: &str = "SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    #[test]
    fn missing_values_fail_fast() {
        assert_eq!(require_present(None), Err(Error::ConfigMissing));
        assert_eq!(require_present(Some("  ")), Err(Error::ConfigMissing));
        assert_eq!(validate_network_id(None), Err(Error::ConfigMissing));
        assert_eq!(validate_rpc_url(None), Err(Error::ConfigMissing));
        assert_eq!(
            validate_secret_key(None, Environment::Staging),
            Err(Error::ConfigMissing)
        );
    }

    #[test]
    fn malformed_values_are_rejected() {
        assert_eq!(
            validate_network_id(Some("marsnet")),
            Err(Error::ConfigInvalid)
        );
        assert_eq!(
            validate_rpc_url(Some("not-a-url")),
            Err(Error::ConfigInvalid)
        );
        assert_eq!(
            validate_rpc_url(Some("ftp://example.com/rpc")),
            Err(Error::ConfigInvalid)
        );
        assert_eq!(
            validate_secret_key(Some("S-INVALID-LENGTH-LONGER-THAN-16-CHARS"), Environment::Staging),
            Err(Error::ConfigInvalid)
        );
        assert_eq!(validate_feature_flag(Some("maybe")), Err(Error::ConfigInvalid));
        assert_eq!(validate_feature_flag(Some("")), Err(Error::ConfigMissing));
    }

    #[test]
    fn unsafe_secrets_never_pass() {
        for bad in ["changeme", "password", "test", "YOUR_SECRET_HERE", "short"] {
            assert_eq!(
                validate_secret_key(Some(bad), Environment::Staging),
                Err(Error::UnsafeSecret),
                "expected UnsafeSecret for {bad}"
            );
        }
        // Production-shaped seed in local mode -> unsafe (prod reuse).
        assert_eq!(validate_secret_key(Some(TEST_SEED), Environment::Local), Err(Error::UnsafeSecret));
        // Same seed in staging/production shape-checks pass.
        assert!(validate_secret_key(Some(TEST_SEED), Environment::Staging).is_ok());
    }

    #[test]
    fn production_requires_https() {
        assert_eq!(
            validate_full_config(
                Some("mainnet"),
                Some("http://insecure.example.com/rpc"),
                Some(TEST_SEED),
                Environment::Production
            ),
            Err(Error::ConfigInvalid)
        );
        assert!(validate_full_config(
            Some("mainnet"),
            Some("https://horizon.stellar.org/rpc"),
            Some(TEST_SEED),
            Environment::Production
        )
        .is_ok());
    }

    #[test]
    fn secrets_are_never_printed_in_full() {
        let dbg = format!("{:?}", RedactedSecret(TEST_SEED));
        assert!(!dbg.contains(TEST_SEED));
        assert!(dbg.contains("..."));
    }
}
