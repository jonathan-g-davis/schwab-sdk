//! `/orders` and `/accounts/{accountNumber}/orders*`
//!
//! Endpoint coverage:
//!
//! - `GET /accounts/{accountNumber}/orders` - per-account list with required
//!   `fromEnteredTime` and `toEnteredTime`, optional `maxResults` and
//!   `status`. Schwab caps the date range at 1 year.
//! - `GET /accounts/{accountNumber}/orders/{orderId}` - single fetch.
//! - `GET /orders` - same shape, across every linked account. Date range
//!   is capped at 60 days.
//! - `POST /accounts/{accountNumber}/orders` - place an order.
//! - `PUT /accounts/{accountNumber}/orders/{orderId}` - replace an order.
//!   Schwab cancels the existing order and creates a new one; the new
//!   `orderId` is returned via the `Location` header.
//! - `DELETE /accounts/{accountNumber}/orders/{orderId}` - cancel an order.
//! - `POST /accounts/{accountNumber}/previewOrder` - preview an order
//!   without placing it. Returns Schwab's projected commissions, fees,
//!   buying-power impact, and validation alerts / rejects.
//!
//! `{accountNumber}` is the encrypted [`AccountHash`], not the plain
//! account number. `orderId` is the Schwab-assigned `int64` returned in
//! the `Location` header of a successful place / replace.
//!
//! ## Idempotency
//!
//! Schwab's Trader API exposes **no client-controllable idempotency key**.
//! [`Orders::place`] takes only the order body; if a network or 5xx
//! failure interrupts the response, the order may still have been
//! accepted. Callers that need retry-safe submission must dedupe at
//! their own layer, typically by listing orders after a transient
//! failure and matching by entered-time window, symbol, side, and
//! quantity.

mod enums;
mod preview;
mod request;
mod response;

pub use enums::*;
pub use preview::{
    AdvancedOrderType, AmountIndicator, ApiRuleAction, Commission, CommissionAndFee, CommissionLeg,
    CommissionValue, FeeLeg, FeeValue, Fees, OrderBalance, OrderLeg, OrderStrategy,
    OrderValidationDetail, OrderValidationResult, PreviewOrder, SettlementInstruction,
};
pub use request::{OrderRequest, SingleOrderBuilder};
pub use response::{ExecutionLeg, Order, OrderActivity, OrderLegCollection};

use chrono::{DateTime, SecondsFormat, Utc};

use crate::client::SchwabClient;
use crate::error::{Error, Result};
use crate::secrets::AccountHash;

// --- Namespaces ---

/// Accessor for `/accounts/{accountNumber}/orders*`. Construct via
/// [`SchwabClient::orders`](crate::SchwabClient::orders).
///
/// `account_hash` throughout is the encrypted [`AccountHash`] from
/// [`SchwabClient::accounts`](crate::SchwabClient::accounts) ->
/// [`numbers`](crate::accounts::Accounts::numbers), never the plain account
/// number.
///
/// # Examples
///
/// Place an equity market buy. [`Orders::place`] accepts any
/// `impl Into<OrderRequest>`, so a shortcut builder flows in without an
/// explicit `.build()`. On success Schwab returns the new order id parsed
/// from the `Location` header; fetch the order back with [`Orders::get`] to
/// see its fill status.
///
/// ```no_run
/// use rust_decimal_macros::dec;
/// use schwab_sdk::{AuthToken, SchwabClient};
/// use schwab_sdk::orders::OrderRequest;
///
/// # async fn run() -> schwab_sdk::Result<()> {
/// let client = SchwabClient::new(AuthToken::new("token"));
///
/// let accounts = client.accounts().numbers().await?;
/// let account_hash = &accounts.first().expect("a linked account").hash_value;
///
/// let order_id = client
///     .orders(account_hash)
///     .place(OrderRequest::buy_market("AAPL", dec!(10)))
///     .await?;
///
/// let order = client.orders(account_hash).get(order_id).await?;
/// println!("order {order_id} status: {:?}", order.status);
/// # Ok(())
/// # }
/// ```
///
/// List the working orders from the last week, then reprice and cancel one.
/// A replace cancels the original and returns a **new** id; the old id is
/// dead afterward.
///
/// ```no_run
/// use chrono::{Duration as ChronoDuration, Utc};
/// use rust_decimal_macros::dec;
/// use schwab_sdk::{AuthToken, SchwabClient};
/// use schwab_sdk::orders::{ApiOrderStatus, OrderRequest};
///
/// # async fn run() -> schwab_sdk::Result<()> {
/// let client = SchwabClient::new(AuthToken::new("token"));
/// let accounts = client.accounts().numbers().await?;
/// let account_hash = &accounts.first().expect("a linked account").hash_value;
/// let orders = client.orders(account_hash);
///
/// let working = orders
///     .list(Utc::now() - ChronoDuration::days(7), Utc::now())
///     .status(ApiOrderStatus::Working)
///     .send()
///     .await?;
///
/// // Each `Order` carries its id and the symbol on its first leg.
/// for order in &working {
///     println!("{:?}: {:?}", order.order_id, order.status);
/// }
///
/// if let Some(open_id) = working.first().and_then(|o| o.order_id) {
///     let new_id = orders
///         .replace(open_id, OrderRequest::buy_limit("AAPL", dec!(10), dec!(141.00)))
///         .await?;
///     orders.cancel(new_id).await?;
/// }
/// # Ok(())
/// # }
/// ```
pub struct Orders<'a, 'b> {
    client: &'a SchwabClient,
    account_hash: &'b AccountHash,
}

impl<'a, 'b> Orders<'a, 'b> {
    pub(crate) fn new(client: &'a SchwabClient, account_hash: &'b AccountHash) -> Self {
        Self {
            client,
            account_hash,
        }
    }

    /// `GET /accounts/{accountNumber}/orders/{orderId}` - fetch a single
    /// order. `order_id` is the Schwab-assigned `int64` (from the
    /// `Location` header on a place / replace, or from a list call).
    pub async fn get(&self, order_id: i64) -> Result<Order> {
        let hash = self.account_hash.expose_secret();
        let path = format!("/accounts/{hash}/orders/{order_id}");
        self.client.trader_http().get_json(&path).await
    }

    /// `POST /accounts/{accountNumber}/orders` - place an order.
    ///
    /// On success Schwab returns 201 with an empty body and a `Location`
    /// header pointing at the new order's resource. This method parses the
    /// trailing `{orderId}` segment from that header and returns it.
    /// Callers may then use [`Self::get`] to fetch the placed order's
    /// status and execution detail.
    ///
    /// Accepts any `impl Into<OrderRequest>` - the shortcuts (e.g.
    /// [`OrderRequest::buy_market`]) and the typestate builder both
    /// satisfy this, so callers can pass them in without an explicit
    /// `.build()`. A pre-built `OrderRequest` works too.
    ///
    /// Schwab has no client-controllable idempotency key, so a transient
    /// failure here may have placed the order anyway. Query orders placed
    /// since the time the original order was submitted and match by symbol,
    /// side, and quantity to determine whether the order was placed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use chrono::{Duration, Utc};
    /// use rust_decimal_macros::dec;
    /// use schwab_sdk::{AuthToken, SchwabClient};
    /// use schwab_sdk::orders::OrderRequest;
    ///
    /// # async fn run() -> schwab_sdk::Result<()> {
    /// let client = SchwabClient::new(AuthToken::new("token"));
    /// let accounts = client.accounts().numbers().await?;
    /// let account_hash = &accounts.first().expect("a linked account").hash_value;
    /// let orders = client.orders(account_hash);
    ///
    /// // Record the window *before* submitting, so a failed place is
    /// // reconcilable.
    /// let submitted_at = Utc::now();
    ///
    /// let order_id = match orders.place(OrderRequest::buy_limit("AAPL", dec!(10), dec!(140))).await {
    ///     Ok(id) => id,
    ///     Err(err) if err.is_retryable() => {
    ///         // The order may have been accepted before the failure. List
    ///         // orders entered since `submitted_at` and match by symbol,
    ///         // side, and quantity rather than resubmitting blindly.
    ///         let candidates = orders
    ///             .list(submitted_at - Duration::seconds(5), Utc::now())
    ///             .send()
    ///             .await?;
    ///         println!("place failed; {} order(s) to reconcile", candidates.len());
    ///         return Ok(());
    ///     }
    ///     Err(err) => return Err(err),
    /// };
    /// println!("placed {order_id}");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn place(&self, order: impl Into<OrderRequest>) -> Result<i64> {
        let order = order.into();
        let hash = self.account_hash.expose_secret();
        let response = self
            .client
            .trader_http()
            .post(&format!("/accounts/{hash}/orders"))
            .json(&order)
            .send()
            .await?;
        parse_order_id_from_location(&response)
    }

    /// `PUT /accounts/{accountNumber}/orders/{orderId}` - replace an order.
    ///
    /// Schwab cancels `order_id` and creates a brand-new order from the
    /// supplied order body; the returned `i64` is the **new** order's
    /// id, parsed from the response `Location` header. The original
    /// `order_id` is no longer valid after a successful replace.
    pub async fn replace(&self, order_id: i64, order: impl Into<OrderRequest>) -> Result<i64> {
        let order = order.into();
        let hash = self.account_hash.expose_secret();
        let response = self
            .client
            .trader_http()
            .put(&format!("/accounts/{hash}/orders/{order_id}"))
            .json(&order)
            .send()
            .await?;
        parse_order_id_from_location(&response)
    }

    /// `DELETE /accounts/{accountNumber}/orders/{orderId}` - cancel an
    /// order. Schwab returns 200 with an empty body on success; this
    /// method discards the response and returns `Ok(())`. Inspecting the
    /// order's terminal state after cancel is the caller's responsibility
    /// (typically by calling [`Self::get`]).
    pub async fn cancel(&self, order_id: i64) -> Result<()> {
        let hash = self.account_hash.expose_secret();
        self.client
            .trader_http()
            .delete(&format!("/accounts/{hash}/orders/{order_id}"))
            .send()
            .await?;
        Ok(())
    }

    /// `POST /accounts/{accountNumber}/previewOrder` - preview an order
    /// without submitting it. Returns Schwab's projected commissions,
    /// fees, buying-power impact, and validation result (which may
    /// include `rejects` even though the response status is 200; callers
    /// should inspect [`PreviewOrder::order_validation_result`] before
    /// treating the preview as approval).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_decimal_macros::dec;
    /// use schwab_sdk::{AuthToken, SchwabClient};
    /// use schwab_sdk::orders::OrderRequest;
    ///
    /// # async fn run() -> schwab_sdk::Result<()> {
    /// let client = SchwabClient::new(AuthToken::new("token"));
    /// let accounts = client.accounts().numbers().await?;
    /// let account_hash = &accounts.first().expect("a linked account").hash_value;
    ///
    /// let preview = client
    ///     .orders(account_hash)
    ///     .preview(OrderRequest::buy_limit("AAPL", dec!(10), dec!(140.00)))
    ///     .await?;
    ///
    /// // A 200 can still carry rejects; check before treating it as approval.
    /// if let Some(result) = &preview.order_validation_result {
    ///     if !result.rejects.is_empty() {
    ///         println!("rejected: {:?}", result.rejects);
    ///         return Ok(());
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn preview(&self, order: impl Into<OrderRequest>) -> Result<PreviewOrder> {
        let order = order.into();
        let hash = self.account_hash.expose_secret();
        self.client
            .trader_http()
            .post(&format!("/accounts/{hash}/previewOrder"))
            .json(&order)
            .send_json()
            .await
    }

    /// Begin a `GET /accounts/{accountNumber}/orders` request.
    ///
    /// `from_entered_time` and `to_entered_time` bound the result window.
    /// Schwab caps the window at one year; this builder does not enforce
    /// that. Optional filters chain before [`ListOrdersBuilder::send`].
    pub fn list(
        &self,
        from_entered_time: DateTime<Utc>,
        to_entered_time: DateTime<Utc>,
    ) -> ListOrdersBuilder<'a, 'b> {
        ListOrdersBuilder {
            client: self.client,
            account_hash: self.account_hash,
            from_entered_time,
            to_entered_time,
            max_results: None,
            status: None,
        }
    }
}

/// Accessor for `/orders` (across every linked account). Construct via
/// [`SchwabClient::orders_all`](crate::SchwabClient::orders_all).
pub struct AllOrders<'a> {
    client: &'a SchwabClient,
}

impl<'a> AllOrders<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// Begin a `GET /orders` request.
    ///
    /// `from_entered_time` and `to_entered_time` bound the result window.
    /// Schwab caps the window at 60 days for the cross-account endpoint;
    /// this builder does not enforce that.
    pub fn list(
        &self,
        from_entered_time: DateTime<Utc>,
        to_entered_time: DateTime<Utc>,
    ) -> ListAllOrdersBuilder<'a> {
        ListAllOrdersBuilder {
            client: self.client,
            from_entered_time,
            to_entered_time,
            max_results: None,
            status: None,
        }
    }
}

// --- List builders ---

/// In-flight request for `GET /accounts/{accountNumber}/orders`. Built via
/// [`Orders::list`].
#[must_use = "call .send() to execute the request"]
pub struct ListOrdersBuilder<'a, 'b> {
    client: &'a SchwabClient,
    account_hash: &'b AccountHash,
    from_entered_time: DateTime<Utc>,
    to_entered_time: DateTime<Utc>,
    max_results: Option<i64>,
    status: Option<ApiOrderStatus>,
}

impl<'a, 'b> ListOrdersBuilder<'a, 'b> {
    /// Cap the response size. Schwab's default is 3000.
    pub fn max_results(mut self, n: i64) -> Self {
        self.max_results = Some(n);
        self
    }

    /// Restrict the response to orders in a specific status.
    pub fn status(mut self, status: ApiOrderStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Order>> {
        let hash = self.account_hash.expose_secret();
        let from = self
            .from_entered_time
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let to = self
            .to_entered_time
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let mut request = self
            .client
            .trader_http()
            .get(&format!("/accounts/{hash}/orders"))
            .query(&[
                ("fromEnteredTime", from.as_str()),
                ("toEnteredTime", to.as_str()),
            ]);
        if let Some(n) = self.max_results {
            let n_str = n.to_string();
            request = request.query(&[("maxResults", n_str.as_str())]);
        }
        if let Some(s) = self.status {
            let s_str = s.to_string();
            request = request.query(&[("status", s_str.as_str())]);
        }
        request.send_json().await
    }
}

/// In-flight request for `GET /orders` across every linked account. Built
/// via [`AllOrders::list`].
#[must_use = "call .send() to execute the request"]
pub struct ListAllOrdersBuilder<'a> {
    client: &'a SchwabClient,
    from_entered_time: DateTime<Utc>,
    to_entered_time: DateTime<Utc>,
    max_results: Option<i64>,
    status: Option<ApiOrderStatus>,
}

impl<'a> ListAllOrdersBuilder<'a> {
    /// Cap the response size. Schwab's default is 3000.
    pub fn max_results(mut self, n: i64) -> Self {
        self.max_results = Some(n);
        self
    }

    /// Restrict the response to orders in a specific status.
    pub fn status(mut self, status: ApiOrderStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Order>> {
        let from = self
            .from_entered_time
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let to = self
            .to_entered_time
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let mut request = self.client.trader_http().get("/orders").query(&[
            ("fromEnteredTime", from.as_str()),
            ("toEnteredTime", to.as_str()),
        ]);
        if let Some(n) = self.max_results {
            let n_str = n.to_string();
            request = request.query(&[("maxResults", n_str.as_str())]);
        }
        if let Some(s) = self.status {
            let s_str = s.to_string();
            request = request.query(&[("status", s_str.as_str())]);
        }
        request.send_json().await
    }
}

// --- Location header parsing ---

/// Parse Schwab's `Location` header after a successful place / replace and
/// extract the trailing `{orderId}` segment. Accepts both absolute URLs
/// (`https://api.schwabapi.com/.../orders/123`) and bare paths.
fn parse_order_id_from_location(response: &reqwest::Response) -> Result<i64> {
    let value = response
        .headers()
        .get(reqwest::header::LOCATION)
        .ok_or_else(|| Error::OrderIdUnrecoverable("missing Location header".to_string()))?
        .to_str()
        .map_err(|e| Error::OrderIdUnrecoverable(format!("Location header not ASCII: {e}")))?;
    parse_order_id_from_location_str(value)
}

fn parse_order_id_from_location_str(location: &str) -> Result<i64> {
    let trimmed = location.trim_end_matches('/');
    let id_segment = trimmed
        .rsplit('/')
        .next()
        .ok_or_else(|| Error::OrderIdUnrecoverable(location.to_string()))?;
    let id_segment = id_segment.split(['?', '#']).next().unwrap_or(id_segment);
    id_segment
        .parse::<i64>()
        .map_err(|_| Error::OrderIdUnrecoverable(location.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_order_id_from_absolute_url() {
        let id = parse_order_id_from_location_str(
            "https://api.schwabapi.com/trader/v1/accounts/ABCDEF/orders/100000001",
        )
        .unwrap();
        assert_eq!(id, 100000001);
    }

    #[test]
    fn parse_order_id_from_relative_path() {
        let id = parse_order_id_from_location_str("/trader/v1/accounts/ABCDEF/orders/42").unwrap();
        assert_eq!(id, 42);
    }

    #[test]
    fn parse_order_id_strips_trailing_slash() {
        let id = parse_order_id_from_location_str("/accounts/ABCDEF/orders/99/").unwrap();
        assert_eq!(id, 99);
    }

    #[test]
    fn parse_order_id_strips_query_string() {
        let id = parse_order_id_from_location_str("/accounts/ABCDEF/orders/77?v=1").unwrap();
        assert_eq!(id, 77);
    }

    #[test]
    fn parse_order_id_rejects_non_numeric() {
        let err = parse_order_id_from_location_str("/accounts/ABCDEF/orders/oops").unwrap_err();
        match err {
            Error::OrderIdUnrecoverable(s) => assert!(s.contains("oops")),
            other => panic!("expected OrderIdUnrecoverable, got {other:?}"),
        }
    }
}
