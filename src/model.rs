//! Shared domain types.
//!
//! This module holds newtypes that flow across the public API. Sensitive
//! string values (bearer tokens, customer identifiers, account numbers) are
//! defined via the `sensitive_string_newtype!` macro, which produces a
//! `SecretBox`-backed newtype with:
//!
//! - `Clone` (via `CloneableSecret`).
//! - `Debug` that redacts via `secrecy`
//!   (`Name(SecretBox<NameInner>([REDACTED]))`).
//! - `Serialize` / `Deserialize` over the inner string (gated by
//!   `SerializableSecret`).
//! - `new(impl Into<String>)` and `expose_secret() -> &str`.

use secrecy::{CloneableSecret, ExposeSecret, SecretBox, SerializableSecret};
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

        impl Clone for $name {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.0).finish()
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.0.serialize(s)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                SecretBox::<$inner>::deserialize(d).map(Self)
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
    /// Schwab account number. PII-equivalent — appears in REST paths
    /// (`/accounts/{accountNumber}/...`) and in account-activity events.
    ///
    /// Redacted in logs via `secrecy`.
    pub AccountNumber, AccountNumberInner
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
}
