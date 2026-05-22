//! Shared domain types.
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

use secrecy::{CloneableSecret, ExposeSecret, SecretBox, SecretString, SerializableSecret};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

macro_rules! sensitive_string_newtype {
    ($(#[$meta:meta])* $vis:vis $name:ident, $inner:ident) => {
        #[derive(Clone, Zeroize, Serialize, Deserialize)]
        #[serde(transparent)]
        struct $inner(String);

        impl CloneableSecret for $inner {}
        impl SerializableSecret for $inner {}

        $(#[$meta])*
        #[derive(Debug, Clone, Zeroize, Serialize, Deserialize)]
        #[serde(transparent)]
        $vis struct $name(SecretBox<$inner>);

        impl $name {
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
    /// Redacted in logs via `secrecy`.
    pub AuthToken, AuthTokenInner
}

sensitive_string_newtype! {
    /// `schwabClientCustomerId` from the user-preference endpoint. Echoed
    /// back into every streamer request envelope. Treated as PII.
    ///
    /// Redacted in logs via `secrecy`.
    pub CustomerId, CustomerIdInner
}

sensitive_string_newtype! {
    /// Schwab account number. PII-equivalent - appears in account-activity
    /// events and in response bodies for account lookups.
    ///
    /// Redacted in logs via `secrecy`.
    pub AccountNumber, AccountNumberInner
}

sensitive_string_newtype! {
    /// Encrypted account-number hash returned by `GET /accounts/accountNumbers`.
    /// Schwab requires this value (not the plain account number) in the
    /// `{accountNumber}` path segment of subsequent REST calls.
    ///
    /// Still account-linked, so redacted in logs via `secrecy`.
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
