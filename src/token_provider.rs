//! Pluggable bearer-token source for streamer reconnect and REST refresh.
//!
//! Implementers own caching, refresh timing, and storage. `schwab-rs` does
//! not call this trait directly today; it is exposed as a standardized seam
//! so consumer reconnect loops can be written against a single shape:
//!
//! ```ignore
//! loop {
//!     let token = provider.access_token().await?;
//!     let streamer = client.streamer().await?;
//!     streamer.login(token).await?;
//!     // drive streamer; on disconnect, loop again.
//! }
//! ```

use crate::model::AuthToken;

/// A source of access tokens for streamer login or REST authentication.
///
/// The associated `Error` type lets implementers surface their own failure
/// modes (HTTP error talking to an OAuth server, missing token in store,
/// expired refresh token, etc.) without coupling to `schwab_rs::Error`.
pub trait TokenProvider {
    /// Implementation-defined error type for token retrieval failures.
    type Error: std::error::Error;

    /// Return a currently-valid access token. The implementation is
    /// responsible for caching and refreshing as needed; callers should
    /// treat every call as potentially performing I/O.
    fn access_token(
        &self,
    ) -> impl std::future::Future<Output = std::result::Result<AuthToken, Self::Error>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial in-memory provider.
    struct StaticProvider {
        token: String,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("static provider error")]
    struct StaticError;

    impl TokenProvider for StaticProvider {
        type Error = StaticError;

        async fn access_token(&self) -> Result<AuthToken, Self::Error> {
            Ok(AuthToken::new(self.token.clone()))
        }
    }

    #[tokio::test]
    async fn static_provider_yields_a_token() {
        let p = StaticProvider {
            token: "abc".to_string(),
        };
        let token = p.access_token().await.unwrap();
        assert_eq!(token.expose_secret(), "abc");
    }

    /// An async provider that yields the task.
    struct YieldingProvider;

    impl TokenProvider for YieldingProvider {
        type Error = std::convert::Infallible;

        async fn access_token(&self) -> Result<AuthToken, Self::Error> {
            tokio::task::yield_now().await;
            Ok(AuthToken::new("yielded"))
        }
    }

    #[tokio::test]
    async fn yielding_provider_awaits_inside_impl() {
        let token = YieldingProvider.access_token().await.unwrap();
        assert_eq!(token.expose_secret(), "yielded");
    }
}
