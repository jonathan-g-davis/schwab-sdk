//! Bearer-credential provider for [`SchwabClient`](crate::SchwabClient).
//!
//! A trivial implementation is provided:
//!
//! - [`StaticTokenProvider`] - returns the same [`AuthToken`] forever.
//!   This is what [`SchwabClient::new`](crate::SchwabClient::new) wraps
//!   internally; callers who never need to rotate a token need not
//!   interact with the trait at all.
//!
//! A consumer that wants on-demand refresh, lazy fetch from a secret
//! store, or any other policy implements [`TokenProvider`] directly.

use async_trait::async_trait;

use crate::error::Error;
use crate::secrets::AuthToken;

/// Source of the bearer token used on every Schwab REST request.
///
/// The SDK calls [`access_token`](Self::access_token) once per request,
/// just before sending. A provider that wants to cache should do so
/// internally; the SDK does not.
///
/// The trait itself carries no `Send`/`Sync` bound so `!Send`
/// implementations remain expressible (tests, future client variants).
/// The bound is enforced at the storage site: [`SchwabClient`] holds
/// `Arc<dyn TokenProvider + Send + Sync>`, so a provider handed to
/// [`SchwabClient::with_token_provider`] must satisfy both.
///
/// [`SchwabClient`]: crate::SchwabClient
/// [`SchwabClient::with_token_provider`]: crate::SchwabClient::with_token_provider
#[async_trait]
pub trait TokenProvider {
    /// Return the current bearer token. Called once per REST request.
    ///
    /// A failure here surfaces as [`Error::TokenProvider`] before any
    /// network I/O is attempted.
    async fn access_token(&self) -> Result<AuthToken, Error>;
}

/// [`TokenProvider`] that returns the same [`AuthToken`] for every call.
///
/// This is the default impl wrapping the token passed to
/// [`SchwabClient::new`](crate::SchwabClient::new); callers who hold a
/// short-lived token and tear the client down when it expires need no
/// other provider.
#[derive(Debug, Clone)]
pub struct StaticTokenProvider(AuthToken);

impl StaticTokenProvider {
    /// Wrap an [`AuthToken`] so it can be served as a [`TokenProvider`].
    pub fn new(token: AuthToken) -> Self {
        Self(token)
    }
}

#[async_trait]
impl TokenProvider for StaticTokenProvider {
    async fn access_token(&self) -> Result<AuthToken, Error> {
        Ok(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn static_provider_returns_the_same_token() {
        let provider = StaticTokenProvider::new(AuthToken::new("abc"));
        let a = provider.access_token().await.unwrap();
        let b = provider.access_token().await.unwrap();
        assert_eq!(a.expose_secret(), "abc");
        assert_eq!(b.expose_secret(), "abc");
    }

    #[test]
    fn static_provider_debug_does_not_leak_token() {
        let provider = StaticTokenProvider::new(AuthToken::new("super-secret"));
        let debug = format!("{provider:?}");
        assert!(
            !debug.contains("super-secret"),
            "Debug leaked token: {debug}"
        );
    }
}
