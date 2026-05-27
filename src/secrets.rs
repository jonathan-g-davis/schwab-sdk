//! Domain types for sensitive strings.
//!
//! This module holds newtypes that flow across the public API. Sensitive
//! string values (bearer tokens, customer identifiers, account numbers) are
//! defined via the `sensitive_string_newtype!` macro, which produces a
//! `SecretBox`-backed newtype with:
//!
//! - `Clone` (via `CloneableSecret`).
//! - `Debug` that redacts via `secrecy`.
//! - `Serialize` / `Deserialize` over the inner string (gated by
//!   `SerializableSecret`).
//! - `new(impl Into<String>)` and `expose_secret() -> &str`.
//! - `From<&str>`, `From<String>`, and `From<SecretString>` for convenience.
//!
//! # Example
//!
//! Construct an [`AuthToken`] for [`SchwabClient::new`](crate::SchwabClient::new),
//! then reach the raw value only at the point of use:
//!
//! ```no_run
//! use schwab_sdk::{AuthToken, SchwabClient};
//!
//! # async fn run() -> schwab_sdk::Result<()> {
//! // Construction: the raw string is wrapped immediately. Prefer reading from
//! // a credential store over `std::env::var` in production; see
//! // "Token storage" below.
//! let token = AuthToken::new(std::env::var("SCHWAB_AUTH_TOKEN").unwrap());
//!
//! // `Debug` redacts; the bearer never appears in `{:?}` output.
//! println!("token = {token:?}"); // prints `token = AuthToken([REDACTED])`
//!
//! // The SDK reveals the secret internally only at the `Authorization`
//! // header and the streamer LOGIN frame. Callers do not need to.
//! let client = SchwabClient::new(token);
//! let accounts = client.accounts().numbers().await?;
//! # let _ = accounts;
//! # Ok(())
//! # }
//! ```
//!
//! When a caller does need the raw value (e.g. when implementing a
//! [`TokenProvider`](crate::TokenProvider) over an external store),
//! [`expose_secret`](secrecy::ExposeSecret::expose_secret) can be used to
//! retrieve it.
//!
//! ```
//! use schwab_sdk::AuthToken;
//!
//! let token = AuthToken::new("abc123");
//! assert_eq!(token.expose_secret(), "abc123");
//! ```
//!
//! # Threat model
//!
//! These newtypes reduce the chance of accidental credential or PII
//! leakage from code that uses them as intended. They are not a
//! security boundary on their own; an explicit
//! `.expose_secret().to_string()`, a misconfigured logger, or a
//! compromised process defeats them.
//!
//! **What they help with**
//!
//! - `{:?}` / `dbg!` / `Debug`-derived `Error` variants do not print
//!   the secret. The redacted form is `Secret([REDACTED ...])`.
//! - `Drop` zeroises the heap buffer that held the secret, narrowing
//!   the window during which a swap-out, post-free read, or stale-page
//!   capture could observe it.
//! - `Clone` copies the protected box rather than producing a plain
//!   `String`, so the secret does not silently widen when passed
//!   around.
//! - [`expose_secret`](secrecy::ExposeSecret::expose_secret) is the
//!   single, grep-able boundary that yields the raw value. Code review
//!   can enumerate every call site.
//!
//! **What they do not help with**
//!
//! - An explicit `.expose_secret().to_string()`, an assignment into a
//!   plain `String` field, or any other code path that copies the raw
//!   bytes out of the protected box. The `secrecy` machinery no longer
//!   applies to the copy.
//! - A `Debug` impl elsewhere that captures an already-exposed form of
//!   the secret (e.g. a `serde_json::Value` built from
//!   `expose_secret()` and then `Debug`-printed).
//! - A debugger, `ptrace` reader, or memory profiler attached to the
//!   live process.
//! - A core dump that snapshots heap pages before `Drop` runs, or heap
//!   pages swapped to disk before the buffer was zeroised.
//! - Logging frameworks, panic hooks, or backtrace machinery that
//!   capture values before this crate's redaction applies.
//!
//! These limits are listed so callers can make informed decisions about
//! what additional process- and host-level hardening to apply. The
//! crate is provided under MIT / Apache-2.0 with no warranty; see
//! `SECURITY.md`.
//!
//! # Caller responsibilities
//!
//! The newtypes cover what happens to a secret once it is inside the
//! SDK. Everything outside of that boundary (where the secret comes from,
//! how it is logged, what other process-level state can see it) is the
//! caller's responsibility.
//!
//! Below are recommendations for how to handle the secrets in your own code.
//!
//! ## Token storage
//! The SDK does not persist tokens. Put the refresh token in an OS-native
//! credential store (Keychain on macOS, Credential Manager on Windows,
//! `keyring`/`keyring-core` against kernel keyutils on Linux). Do not commit
//! tokens to `.env`, config files, or CI environment variables visible across
//! jobs. A refresh token carries trading authority on a real-money account;
//! treat it at that sensitivity.
//!
//! The [`keyring-core`](https://crates.io/crates/keyring-core) and its
//! platform-native implementations are a good starting point.
//!
//! ## Process exposure
//!
//! A token in a process's environment is readable by any process running as
//! the same user, and by `/proc/<pid>/environ` on Linux. Prefer reading from a
//! credential store at startup over `std::env::var` in production binaries.
//! Never use `env!` for a real token: that bakes the value into the binary at
//! compile time.
//!
//! ## Logging discipline
//!
//! If you wrap SDK calls in `tracing` or similar, redact request bodies and
//! headers. The streamer LOGIN frame serialises the auth token into JSON
//! before transmission, so logging a constructed frame body leaks a bearer
//! even though [`AuthToken`] redacts in its own `Debug`. Either keep
//! frame-level logging off, or scrub by field.
//!
//! Secrets are only redacted within the newtypes. Only call `.expose_secret()`
//! to get the raw value at the point of use instead of logging or storing it.
//!
//! ## Data at rest
//!
//! Zeroising on `Drop` does not protect against a debugger attached to the
//! live process, a core dump that captures heap pages, or pages swapped to
//! disk. If these are a concern, you should apply host-level hardening
//! (e.g., encrypted swap).

use secrecy::zeroize::Zeroize;
use secrecy::{CloneableSecret, ExposeSecret, SecretBox, SecretString, SerializableSecret};
use serde::{Deserialize, Serialize};

/// Deserialize a [`String`] from either a JSON string or a JSON integer.
///
/// Schwab returns the same logical field as different JSON types across
/// endpoints (e.g. `accountNumber` is a string on `securitiesAccount` and
/// an `int64` on `Order`). This function accepts either form to prevent a
/// parse error.
fn deserialize_string_or_int<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = String;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a string or integer")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<String, E> {
            Ok(v.to_owned())
        }

        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<String, E> {
            Ok(v)
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<String, E> {
            Ok(v.to_string())
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<String, E> {
            Ok(v.to_string())
        }
    }

    d.deserialize_any(V)
}

macro_rules! sensitive_string_newtype {
    // Default: deserialize from a JSON string only.
    ($(#[$meta:meta])* $vis:vis $name:ident, $inner:ident) => {
        #[derive(Clone, Serialize, Deserialize)]
        #[serde(transparent)]
        struct $inner(String);

        sensitive_string_newtype!(@common $(#[$meta])* $vis $name, $inner);
    };

    // Use a custom deserializer for the inner type.
    ($(#[$meta:meta])* $vis:vis $name:ident, $inner:ident, deserialize_with = $de:path) => {
        #[derive(Clone, Serialize)]
        #[serde(transparent)]
        struct $inner(String);

        impl<'de> Deserialize<'de> for $inner {
            fn deserialize<D>(d: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                $de(d).map($inner)
            }
        }

        sensitive_string_newtype!(@common $(#[$meta])* $vis $name, $inner);
    };

    // Common expansion: the inner type, its zeroization, cloning, and serialization.
    (@common $(#[$meta:meta])* $vis:vis $name:ident, $inner:ident) => {
        impl Zeroize for $inner {
            fn zeroize(&mut self) {
                self.0.zeroize();
            }
        }

        impl CloneableSecret for $inner {}
        impl SerializableSecret for $inner {}

        $(#[$meta])*
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(transparent)]
        $vis struct $name(SecretBox<$inner>);

        impl $name {
            /// Wrap a raw string in the redacting newtype.
            pub fn new(value: impl Into<String>) -> Self {
                Self(SecretBox::new(Box::new($inner(value.into()))))
            }

            /// Reveal the raw value. Use only at the point of constructing a
            /// wire header, frame, or URL path segment; do not store, log,
            /// or pass into untyped contexts.
            pub fn expose_secret(&self) -> &str {
                &self.0.expose_secret().0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl From<SecretString> for $name {
            fn from(value: SecretString) -> Self {
                Self::new(value.expose_secret())
            }
        }
    };
}

sensitive_string_newtype! {
    /// OAuth bearer access token used in `Authorization: Bearer ...` headers
    /// and in the streamer LOGIN frame's `Authorization` parameter.
    ///
    /// # Security
    ///
    /// Bearer credential with trading authority on a real-money
    /// account. Wrapped in `secrecy::SecretBox`: `Debug` redacts and
    /// `Drop` zeroises. Obtain the raw value via
    /// [`expose_secret`](secrecy::ExposeSecret::expose_secret) only at
    /// the point of use (header construction, LOGIN-frame
    /// construction); do not store it in a plain `String`, do not
    /// include it in error variants or log lines, and do not pass it
    /// to a serializer that prints its input on error. See the
    /// module-level threat model for what these properties do and do
    /// not defend against.
    pub AuthToken, AuthTokenInner
}

sensitive_string_newtype! {
    /// `schwabClientCustomerId` from the user-preference endpoint. Echoed
    /// back into every streamer request envelope.
    ///
    /// # Security
    ///
    /// PII linking a streamer session to a Schwab customer. Not itself
    /// a bearer credential, but identifying enough that it should be
    /// handled with the same care: do not log, do not surface in error
    /// strings, do not write to disk outside an OS-native credential
    /// store. `Debug` redacts and `Drop` zeroises; see the module-level
    /// threat model for the limits of those properties.
    pub CustomerId, CustomerIdInner
}

sensitive_string_newtype! {
    /// Schwab account number. Appears in account-activity events and in
    /// response bodies for account lookups.
    ///
    /// # Security
    ///
    /// PII at financial-account sensitivity. Not used in REST URL
    /// paths - per-account endpoints take the encrypted
    /// [`AccountHash`] instead - but does appear in response payloads
    /// and streamer account-activity frames. Do not log, do not embed
    /// in error strings, do not transmit to third-party services.
    /// `Debug` redacts and `Drop` zeroises; see the module-level
    /// threat model for the limits of those properties.
    pub AccountNumber, AccountNumberInner, deserialize_with = deserialize_string_or_int
}

// Add impls for PartialEq, Eq, and Hash to the AccountNumber type so response
// types that contain an `AccountNumber` can derive `PartialEq` / `Eq` / `Hash`.
//
// AccountNumber is sensitive enough that we don't want to accidentally log it,
// but not so sensitive that we couldn't use it as a key in a HashMap.
impl PartialEq for AccountNumber {
    fn eq(&self, other: &Self) -> bool {
        self.expose_secret() == other.expose_secret()
    }
}

impl Eq for AccountNumber {}

impl std::hash::Hash for AccountNumber {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.expose_secret().hash(state);
    }
}

sensitive_string_newtype! {
    /// Encrypted account-number hash returned by `GET /accounts/accountNumbers`.
    /// Schwab requires this value (not the plain account number) in the
    /// `{accountNumber}` path segment of subsequent REST calls.
    ///
    /// # Security
    ///
    /// Account-linked identifier. Schwab encrypts the account number
    /// before issuing this hash, so it is less directly sensitive than
    /// [`AccountNumber`], but it is still a stable account identifier
    /// that an attacker could use to correlate activity. Treat as PII:
    /// do not log, do not include in error variants, do not share
    /// outside the SDK boundary. `Debug` redacts and `Drop` zeroises;
    /// see the module-level threat model for the limits of those
    /// properties.
    pub AccountHash, AccountHashInner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_token_debug_is_redacted() {
        let token = AuthToken::new("super-secret-bearer");
        let debug = format!("{token:?}");
        assert!(
            !debug.contains("super-secret-bearer"),
            "Debug leaked secret: {debug}"
        );
        assert!(debug.contains("REDACTED"), "expected REDACTED in {debug}");
    }

    #[test]
    fn auth_token_serializes_to_inner_string() {
        let token = AuthToken::new("abc123");
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(json, r#""abc123""#);
    }

    #[test]
    fn auth_token_round_trips_through_serde() {
        let token = AuthToken::new("round-trip");
        let json = serde_json::to_string(&token).unwrap();
        let restored: AuthToken = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.expose_secret(), "round-trip");
    }

    #[test]
    fn customer_id_debug_is_redacted() {
        let id = CustomerId::new("CUST-001");
        let debug = format!("{id:?}");
        assert!(!debug.contains("CUST-001"));
        assert!(debug.contains("REDACTED"));
    }

    #[test]
    fn customer_id_round_trips() {
        let id = CustomerId::new("CUST-001");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""CUST-001""#);
        let restored: CustomerId = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.expose_secret(), "CUST-001");
    }

    #[test]
    fn account_number_debug_is_redacted() {
        let acct = AccountNumber::new("12345678");
        let debug = format!("{acct:?}");
        assert!(!debug.contains("12345678"));
        assert!(debug.contains("REDACTED"));
    }

    #[test]
    fn account_number_round_trips() {
        let acct = AccountNumber::new("12345678");
        let json = serde_json::to_string(&acct).unwrap();
        assert_eq!(json, r#""12345678""#);
        let restored: AccountNumber = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.expose_secret(), "12345678");
    }

    #[test]
    fn account_number_deserializes_from_string_or_int() {
        // Schwab's wire type varies across endpoints (string on
        // `securitiesAccount`, `int64` on `Order`). Both must decode.
        let from_str: AccountNumber = serde_json::from_str(r#""12345678""#).unwrap();
        let from_int: AccountNumber = serde_json::from_str("12345678").unwrap();
        assert_eq!(from_str.expose_secret(), "12345678");
        assert_eq!(from_int.expose_secret(), "12345678");
        assert_eq!(from_str, from_int);

        let debug = format!("{from_int:?}");
        assert!(!debug.contains("12345678"), "Debug leaked: {debug}");
        assert!(debug.contains("REDACTED"), "expected REDACTED in {debug}");
    }

    #[test]
    fn account_number_unexpected_type_produces_descriptive_error() {
        let err = serde_json::from_str::<AccountNumber>("true").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("string") && msg.contains("integer"),
            "missing expectation: {msg}",
        );
        assert!(msg.contains("bool"), "missing offending type: {msg}");
    }

    #[test]
    fn account_hash_debug_is_redacted() {
        let hash = AccountHash::new("ABCDEF0123456789");
        let debug = format!("{hash:?}");
        assert!(!debug.contains("ABCDEF0123456789"));
        assert!(debug.contains("REDACTED"));
    }

    #[test]
    fn account_hash_round_trips() {
        let hash = AccountHash::new("ABCDEF0123456789");
        let json = serde_json::to_string(&hash).unwrap();
        assert_eq!(json, r#""ABCDEF0123456789""#);
        let restored: AccountHash = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.expose_secret(), "ABCDEF0123456789");
    }
}
