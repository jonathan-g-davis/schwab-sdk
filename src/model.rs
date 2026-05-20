//! Shared domain types.
//!
//! This module holds newtypes that flow across the public API. The pattern
//! for any value Schwab treats as sensitive (bearer tokens, customer
//! identifiers, account numbers) is:
//!
//! 1. A private `*Inner` type that derives `Zeroize` + `Serialize` +
//!    `Deserialize` and `impl`s `SerializableSecret` + `CloneableSecret`.
//! 2. A public newtype wrapping `SecretBox<*Inner>`. `SecretBox`'s `Debug`
//!    impl redacts the inner value (`SecretBox<TypeName>([REDACTED])`), so
//!    structs holding these types can derive `Debug` without leaking
//!    credentials into logs.

use secrecy::{CloneableSecret, ExposeSecret, SecretBox, SerializableSecret};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

// --- AuthToken ---------------------------------------------------------------

#[derive(Clone, Zeroize, Serialize, Deserialize)]
#[serde(transparent)]
struct AuthTokenInner(String);

impl CloneableSecret for AuthTokenInner {}
impl SerializableSecret for AuthTokenInner {}

/// OAuth bearer access token used in `Authorization: Bearer ...` headers and
/// in the streamer LOGIN frame's `Authorization` parameter.
/// 
/// Redacted in logs via `secrecy`.
pub struct AuthToken(SecretBox<AuthTokenInner>);

impl AuthToken {
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretBox::new(Box::new(AuthTokenInner(value.into()))))
    }

    /// Reveal the raw token. Use only at the point of constructing a wire
    /// header or frame; do not store, log, or pass into untyped contexts.
    pub fn expose_secret(&self) -> &str {
        &self.0.expose_secret().0
    }
}

impl Clone for AuthToken {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AuthToken").field(&self.0).finish()
    }
}

impl Serialize for AuthToken {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for AuthToken {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        SecretBox::<AuthTokenInner>::deserialize(d).map(Self)
    }
}

// --- CustomerId --------------------------------------------------------------

#[derive(Clone, Zeroize, Serialize, Deserialize)]
#[serde(transparent)]
struct CustomerIdInner(String);

impl CloneableSecret for CustomerIdInner {}
impl SerializableSecret for CustomerIdInner {}

/// `schwabClientCustomerId` from the user-preference endpoint. Echoed back
/// into every streamer request envelope. Treated as PII.
/// 
/// Redacted in logs via `secrecy`.
pub struct CustomerId(SecretBox<CustomerIdInner>);

impl CustomerId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretBox::new(Box::new(CustomerIdInner(value.into()))))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0.expose_secret().0
    }
}

impl Clone for CustomerId {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::fmt::Debug for CustomerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CustomerId").field(&self.0).finish()
    }
}

impl Serialize for CustomerId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for CustomerId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        SecretBox::<CustomerIdInner>::deserialize(d).map(Self)
    }
}

// --- AccountNumber -----------------------------------------------------------

#[derive(Clone, Zeroize, Serialize, Deserialize)]
#[serde(transparent)]
struct AccountNumberInner(String);

impl CloneableSecret for AccountNumberInner {}
impl SerializableSecret for AccountNumberInner {}

/// Schwab account number. PII-equivalent — appears in REST paths
/// (`/accounts/{accountNumber}/...`) and in account-activity events.
/// 
/// Redacted in logs via `secrecy`.
pub struct AccountNumber(SecretBox<AccountNumberInner>);

impl AccountNumber {
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretBox::new(Box::new(AccountNumberInner(value.into()))))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0.expose_secret().0
    }
}

impl Clone for AccountNumber {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::fmt::Debug for AccountNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AccountNumber").field(&self.0).finish()
    }
}

impl Serialize for AccountNumber {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for AccountNumber {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        SecretBox::<AccountNumberInner>::deserialize(d).map(Self)
    }
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
