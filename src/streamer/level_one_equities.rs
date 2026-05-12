use strum::{Display, EnumString, FromRepr};

use crate::streamer::{
    Service, StreamerRequest,
    subscription::{Subscription, SubscriptionParameters},
};

impl From<Subscription<Field>> for StreamerRequest {
    fn from(subscription: Subscription<Field>) -> Self {
        StreamerRequest {
            service: Service::LevelOneEquities,
            command: subscription.command.into(),
            parameters: serde_json::to_value(SubscriptionParameters {
                keys: subscription.keys,
                fields: subscription.fields,
            })
            .unwrap(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde_repr::Serialize_repr,
    Display,
    EnumString,
    FromRepr,
)]
#[repr(u8)]
#[strum(serialize_all = "snake_case")]
pub enum Field {
    Symbol,
    BidPrice,
    AskPrice,
    LastPrice,
    BidSize,
    AskSize,
    AskId,
    BidId,
    TotalVolume,
    LastSize,
    HighPrice,
    LowPrice,
    ClosePrice,
    ExchangeId,
    Marginable,
    Description,
    LastId,
    OpenPrice,
    NetChange,
    High52WeekPrice,
    Low52WeekPrice,
    PeRatio,
    AnnualDividendAmount,
    DividendYield,
    Nav,
    ExchangeName,
    DividendDate,
    RegularMarketQuote,
    RegularMarketTrade,
    RegularMarketLastPrice,
    RegularMarketLastSize,
    RegularMarketNetChange,
    SecurityStatus,
    MarkPrice,
    QuoteTime,
    TradeTime,
    RegularMarketTradeTime,
    BidTime,
    AskTime,
    AskMicId,
    BidMicId,
    LastMicId,
    NetPercentageChange,
    RegularMarketPercentageChange,
    MarkPriceNetChange,
    MarkPricePercentageChange,
    HardToBorrowQuantity,
    HardToBorrowRate,
    HardToBorrow,
    Shortable,
    PostMarketNetChange,
    PostMarketPercentageChange,
}

impl From<Field> for u8 {
    fn from(field: Field) -> Self {
        field as u8
    }
}

impl TryFrom<u8> for Field {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Field::from_repr(value).ok_or_else(|| format!("Invalid field: {}", value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_parameters() {
        let parameters = SubscriptionParameters {
            keys: vec!["AAPL".to_string()],
            fields: vec![Field::Symbol, Field::BidPrice, Field::AskPrice],
        };
        let serialized = serde_json::to_string(&parameters).unwrap();
        assert_eq!(serialized, r#"{"keys":"AAPL","fields":"0,1,2"}"#);
    }
}
