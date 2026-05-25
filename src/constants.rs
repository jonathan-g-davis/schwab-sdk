use std::time::Duration;

/// Lifetime of a freshly issued Schwab OAuth access token
/// ([`AuthToken`](crate::AuthToken)).
/// Schwab issues access tokens valid for 30 minutes. A token may be
/// revoked or invalidated earlier, so treat this as the issued upper
/// bound rather than a guarantee.
pub const DEFAULT_AUTH_TOKEN_EXPIRY: Duration = Duration::from_secs(30 * 60); // 30 minutes

/// Lifetime of a freshly issued Schwab OAuth refresh token. Schwab issues
/// refresh tokens valid for 7 days; once it expires the full
/// authorization flow must be re-run to obtain a new one.
pub const DEFAULT_REFRESH_TOKEN_EXPIRY: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7 days

/// Production base URL for Schwab's Trader API.
pub const TRADER_BASE_URL: &str = "https://api.schwabapi.com/trader/v1";

/// Production base URL for Schwab's Market Data API.
pub const MARKET_DATA_BASE_URL: &str = "https://api.schwabapi.com/marketdata/v1";
