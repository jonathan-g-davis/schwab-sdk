//! `/expirationchain` - option expiration series for a symbol.
//!
//! Returns the option expiration (series) information for an optionable
//! symbol. Unlike `/chains`, the response does not include the individual
//! contracts at each expiration.
//!
//! Reached through
//! [`MarketData::expiration_chain`](super::MarketData::expiration_chain).

use serde::Deserialize;

use super::chains::{ExpirationType, SettlementType};
use crate::client::SchwabClient;
use crate::error::Result;

/// Accessor for `/expirationchain`. Construct via
/// [`MarketData::expiration_chain`](super::MarketData::expiration_chain).
pub struct ExpirationChain<'a> {
    client: &'a SchwabClient,
}

impl<'a> ExpirationChain<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// Fetch the option expiration series for an optionable `symbol`.
    pub async fn get(&self, symbol: impl AsRef<str>) -> Result<ExpirationChainResponse> {
        let md = self.client.market_data_http();
        let request = md
            .get("/expirationchain")
            .query(&[("symbol", symbol.as_ref())]);
        md.execute_json(request).await
    }
}

// --- Response shape ---

/// `/expirationchain` response body.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ExpirationChainResponse {
    /// Schwab response status string (typically `"SUCCESS"`).
    #[serde(default)]
    pub status: Option<String>,
    /// One entry per expiration date in the series.
    #[serde(rename = "expirationList", default)]
    pub expiration_list: Vec<Expiration>,
}

/// One expiration in the series.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct Expiration {
    /// Calendar days until expiration.
    #[serde(rename = "daysToExpiration", default)]
    pub days_to_expiration: Option<i32>,
    /// `yyyy-MM-dd` expiration date. The live API sends this as
    /// `expirationDate`; `expiration` is accepted as an alias.
    #[serde(rename = "expirationDate", alias = "expiration", default)]
    pub expiration_date: Option<String>,
    /// Expiration classification (standard/weekly/quarterly/...).
    #[serde(rename = "expirationType", default)]
    pub expiration_type: Option<ExpirationType>,
    /// `true` for standard (monthly) expirations.
    #[serde(default)]
    pub standard: Option<bool>,
    /// AM/PM settlement.
    #[serde(rename = "settlementType", default)]
    pub settlement_type: Option<SettlementType>,
    /// Comma-separated option root symbols for this expiration.
    #[serde(rename = "optionRoots", default)]
    pub option_roots: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiration_chain_parses() {
        // Shape modeled on Schwab's documented response: a list of
        // expirations with no per-expiration contracts.
        let json = r#"{
            "status": "SUCCESS",
            "expirationList": [
                {
                    "expirationDate": "2022-01-07",
                    "daysToExpiration": 2,
                    "expirationType": "W",
                    "standard": true,
                    "settlementType": "P"
                },
                {
                    "expirationDate": "2022-01-21",
                    "daysToExpiration": 16,
                    "expirationType": "S",
                    "standard": true
                }
            ]
        }"#;
        let resp: ExpirationChainResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.expiration_list.len(), 2);

        let first = &resp.expiration_list[0];
        assert_eq!(first.expiration_date.as_deref(), Some("2022-01-07"));
        assert_eq!(first.days_to_expiration, Some(2));
        assert_eq!(first.expiration_type, Some(ExpirationType::Weekly));
        assert_eq!(first.standard, Some(true));
        assert_eq!(first.settlement_type, Some(SettlementType::Pm));

        let second = &resp.expiration_list[1];
        assert_eq!(second.expiration_type, Some(ExpirationType::Standard));
        assert_eq!(second.settlement_type, None);
    }

    #[test]
    fn expiration_field_alias_is_accepted() {
        // Schwab's published schema names the date field `expiration`;
        // the alias keeps that wire form decoding cleanly.
        let json = r#"{ "expirationList": [ { "expiration": "2022-01-07" } ] }"#;
        let resp: ExpirationChainResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.expiration_list[0].expiration_date.as_deref(),
            Some("2022-01-07")
        );
    }

    #[test]
    fn empty_expiration_chain_parses() {
        let resp: ExpirationChainResponse = serde_json::from_str("{}").unwrap();
        assert!(resp.expiration_list.is_empty());
        assert!(resp.status.is_none());
    }
}
