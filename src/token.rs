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
/// # Examples
///
/// A swappable provider using `arc-swap` for wait-free reads. A refresh
/// loop calls [`rotate`](#method.rotate) when a new access token arrives
/// and the next [`access_token`](Self::access_token) call hands it out.
/// Wire it in with [`SchwabClient::with_token_provider`]. The same provider
/// is reused across every clone of the client.
///
/// ```no_run
/// use std::sync::Arc;
/// use arc_swap::ArcSwap;
/// use async_trait::async_trait;
/// use schwab_sdk::{AuthToken, Error, SchwabClient, TokenProvider};
/// 
/// struct SwappableProvider(ArcSwap<AuthToken>);
///
/// impl SwappableProvider {
///     fn new(initial: AuthToken) -> Self {
///         Self(ArcSwap::from_pointee(initial))
///     }
///
///     /// Called by your refresh loop when a fresh access token arrives.
///     fn rotate(&self, fresh: AuthToken) {
///         self.0.store(Arc::new(fresh));
///     }
/// }
///
/// #[async_trait]
/// impl TokenProvider for SwappableProvider {
///     async fn access_token(&self) -> Result<AuthToken, Error> {
///         Ok((*self.0.load_full()).clone())
///     }
/// }
///
/// async fn run() -> schwab_sdk::Result<()> {
///     let provider = Arc::new(SwappableProvider::new(AuthToken::new("initial-token")));
///     let client = SchwabClient::with_token_provider(provider.clone());
///
///     // The first REST call sees the initial token.
///     let _ = client.accounts().numbers().await?;
///
///     // Your refresh task obtains a new access token out of band, then
///     // hands it to the provider.
///     provider.rotate(AuthToken::new("rotated-token"));
/// 
///     // The next REST call sees the rotated token.
///     let _ = client.accounts().numbers().await?;
/// 
///     Ok(())
/// }
/// ```
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
