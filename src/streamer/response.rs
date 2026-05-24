use super::{account_activity, book, chart, level_one, screener};
use crate::error::{Error, Result};
use crate::streamer::protocol::{ResponseCode, Service, StreamerCommand};
use crate::streamer::subscription::Command as SubscriptionCommand;
use serde_with::{DisplayFromStr, PickFirst, serde_as};

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
#[non_exhaustive]
pub struct ResponsePayload {
    #[serde(rename = "requestid")]
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    pub request_id: u64,
    pub service: Service,
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    pub timestamp: u64,
    pub command: StreamerCommand,
    #[serde(rename = "SchwabClientCorrelId")]
    pub schwab_client_correlation_id: String,
    pub content: ResponseContent,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[non_exhaustive]
pub struct ResponseContent {
    pub code: ResponseCode,
    #[serde(rename = "msg")]
    pub message: String,
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
#[non_exhaustive]
pub struct Heartbeat {
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    pub heartbeat: u64,
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct RawDataPayload {
    service: Service,
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    timestamp: u64,
    command: StreamerCommand,
    content: serde_json::Value,
}

/// One element of a `data` array on a streamer frame, already decoded into a
/// service-specific typed shape.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DataPayload {
    pub service: Service,
    pub timestamp: u64,
    pub command: SubscriptionCommand,
    pub content: DataContent,
}

/// Typed content per streamer service.
///
/// Each variant corresponds to a service whose payload `schwab-rs` decodes
/// into typed fields. Services not yet typed land in [`DataContent::Raw`]
/// with Schwab's numeric-keyed JSON object preserved, so callers can still
/// destructure them by hand until a typed variant is added.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DataContent {
    LevelOneEquities(Vec<level_one::equities::Content>),
    LevelOneOptions(Vec<level_one::options::Content>),
    LevelOneFutures(Vec<level_one::futures::Content>),
    LevelOneFuturesOptions(Vec<level_one::futures_options::Content>),
    LevelOneForex(Vec<level_one::forex::Content>),
    NyseBook(Vec<book::Content>),
    NasdaqBook(Vec<book::Content>),
    OptionsBook(Vec<book::Content>),
    ChartEquity(Vec<chart::equity::Content>),
    ChartFutures(Vec<chart::futures::Content>),
    ScreenerEquity(Vec<screener::Content>),
    ScreenerOption(Vec<screener::Content>),
    AccountActivity(Vec<account_activity::Content>),
    /// Untyped fallback for services that don't have a typed variant yet.
    /// The inner value is the raw `content` array from Schwab with numeric
    /// field keys remapped to their snake_case names where the streamer
    /// knows the field set, and left numeric otherwise.
    Raw(serde_json::Value),
}

impl TryFrom<RawDataPayload> for DataPayload {
    type Error = Error;

    fn try_from(payload: RawDataPayload) -> Result<Self> {
        let command = SubscriptionCommand::try_from(payload.command).map_err(|e| Error::Codec {
            context: "data payload command".to_string(),
            reason: e,
        })?;
        let content = decode_service_content(&payload.service, payload.content)?;
        Ok(DataPayload {
            service: payload.service,
            timestamp: payload.timestamp,
            command,
            content,
        })
    }
}

fn decode_service_content(service: &Service, content: serde_json::Value) -> Result<DataContent> {
    match service {
        Service::LevelOneEquities => {
            let remapped = transform_keys::<level_one::equities::Field>(content)?;
            Ok(DataContent::LevelOneEquities(
                level_one::equities::Content::decode_batch(remapped)?,
            ))
        }
        Service::LevelOneOptions => {
            let remapped = transform_keys::<level_one::options::Field>(content)?;
            Ok(DataContent::LevelOneOptions(
                level_one::options::Content::decode_batch(remapped)?,
            ))
        }
        Service::LevelOneFutures => {
            let remapped = transform_keys::<level_one::futures::Field>(content)?;
            Ok(DataContent::LevelOneFutures(
                level_one::futures::Content::decode_batch(remapped)?,
            ))
        }
        Service::LevelOneFuturesOptions => {
            let remapped = transform_keys::<level_one::futures_options::Field>(content)?;
            Ok(DataContent::LevelOneFuturesOptions(
                level_one::futures_options::Content::decode_batch(remapped)?,
            ))
        }
        Service::LevelOneForex => {
            let remapped = transform_keys::<level_one::forex::Field>(content)?;
            Ok(DataContent::LevelOneForex(
                level_one::forex::Content::decode_batch(remapped)?,
            ))
        }
        Service::NyseBook => {
            let remapped = transform_keys::<book::nyse::Field>(content)?;
            Ok(DataContent::NyseBook(book::nyse::decode_batch(remapped)?))
        }
        Service::NasdaqBook => {
            let remapped = transform_keys::<book::nasdaq::Field>(content)?;
            Ok(DataContent::NasdaqBook(book::nasdaq::decode_batch(
                remapped,
            )?))
        }
        Service::OptionsBook => {
            let remapped = transform_keys::<book::options::Field>(content)?;
            Ok(DataContent::OptionsBook(book::options::decode_batch(
                remapped,
            )?))
        }
        Service::ChartEquity => {
            let remapped = transform_keys::<chart::equity::Field>(content)?;
            Ok(DataContent::ChartEquity(
                chart::equity::Content::decode_batch(remapped)?,
            ))
        }
        Service::ChartFutures => {
            let remapped = transform_keys::<chart::futures::Field>(content)?;
            Ok(DataContent::ChartFutures(
                chart::futures::Content::decode_batch(remapped)?,
            ))
        }
        Service::ScreenerEquity => {
            let remapped = transform_keys::<screener::equity::Field>(content)?;
            Ok(DataContent::ScreenerEquity(screener::equity::decode_batch(
                remapped,
            )?))
        }
        Service::ScreenerOption => {
            let remapped = transform_keys::<screener::option::Field>(content)?;
            Ok(DataContent::ScreenerOption(screener::option::decode_batch(
                remapped,
            )?))
        }
        Service::AccountActivity => {
            let remapped = transform_keys::<account_activity::Field>(content)?;
            Ok(DataContent::AccountActivity(
                account_activity::Content::decode_batch(remapped)?,
            ))
        }
        // ADMIN carries login/logout responses, not data; if one ever shows
        // up here, forward it as Raw rather than failing.
        Service::Admin => Ok(DataContent::Raw(content)),
        // Forward-compat: any service Schwab adds later is forwarded with
        // its raw content array. The numeric-keyed field map is preserved.
        Service::Unknown(_) => Ok(DataContent::Raw(content)),
    }
}

fn transform_keys<T: std::fmt::Display + TryFrom<u8>>(
    content: serde_json::Value,
) -> Result<serde_json::Value> {
    let array = content.as_array().ok_or_else(|| Error::Codec {
        context: "data payload content".to_string(),
        reason: "expected array".to_string(),
    })?;
    let mut out = Vec::with_capacity(array.len());
    for item in array {
        let object = item.as_object().ok_or_else(|| Error::Codec {
            context: "data payload item".to_string(),
            reason: "expected object".to_string(),
        })?;
        let mut map = serde_json::Map::with_capacity(object.len());
        for (k, v) in object {
            // Field-number keys get remapped to their name; everything else
            // (e.g. "key", "delayed", "assetMainType") passes through. An
            // unknown numeric discriminant is forward-compatibility: keep
            // the raw key so the consumer can still see the field.
            let mapped = match k.parse::<u8>() {
                Ok(n) => T::try_from(n)
                    .map(|field| field.to_string())
                    .unwrap_or_else(|_| k.clone()),
                Err(_) => k.clone(),
            };
            map.insert(mapped, v.clone());
        }
        out.push(serde_json::Value::Object(map));
    }
    Ok(serde_json::Value::Array(out))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(super) enum RawStreamerResponse {
    #[serde(rename = "response")]
    Response(Vec<ResponsePayload>),
    #[serde(rename = "notify")]
    Notify(Vec<Heartbeat>),
    #[serde(rename = "data")]
    Data(Vec<RawDataPayload>),
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum StreamerResponse {
    Response(Vec<ResponsePayload>),
    Notify(Vec<Heartbeat>),
    Data(Vec<DataPayload>),
}

impl TryFrom<RawStreamerResponse> for StreamerResponse {
    type Error = Error;

    fn try_from(response: RawStreamerResponse) -> Result<Self> {
        Ok(match response {
            RawStreamerResponse::Response(responses) => StreamerResponse::Response(responses),
            RawStreamerResponse::Notify(heartbeats) => StreamerResponse::Notify(heartbeats),
            RawStreamerResponse::Data(data) => {
                let converted = data
                    .into_iter()
                    .map(DataPayload::try_from)
                    .collect::<Result<Vec<DataPayload>>>()?;
                StreamerResponse::Data(converted)
            }
        })
    }
}

/// Parse a raw streamer frame into a typed [`StreamerResponse`]. Shared by the
/// per-service tests in the service modules.
#[cfg(test)]
pub(crate) fn parse(raw: &str) -> Result<StreamerResponse> {
    let raw_response: RawStreamerResponse =
        serde_json::from_slice(raw.as_bytes()).map_err(|e| Error::Codec {
            context: "test fixture".to_string(),
            reason: e.to_string(),
        })?;
    StreamerResponse::try_from(raw_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use rust_decimal_macros::dec;

    #[test]
    fn parses_heartbeat_notify() {
        let frame = r#"{"notify":[{"heartbeat":"1668715930582"}]}"#;
        let StreamerResponse::Notify(heartbeats) = parse(frame).unwrap() else {
            panic!("expected Notify");
        };
        assert_eq!(heartbeats.len(), 1);
        assert_eq!(heartbeats[0].heartbeat, 1668715930582);
    }

    #[test]
    fn unknown_numeric_field_does_not_fail_parse() {
        // Schwab adds a new field 99 we haven't typed yet. The remapper
        // should keep the raw "99" key (so it's accessible if anyone drops
        // down to Raw), and the typed struct ignores it via #[serde(default)]
        // and unknown-field tolerance (Deserialize is non-deny by default).
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_EQUITIES",
                "timestamp": 1,
                "command": "SUBS",
                "content": [{
                    "key": "X", "delayed": false,
                    "1": 1.0, "99": "future-field"
                }]
            }]
        }"#;
        let response = parse(frame).expect("forward-compat parse failed");
        let StreamerResponse::Data(data) = response else {
            panic!("expected Data");
        };
        let DataContent::LevelOneEquities(items) = &data[0].content else {
            panic!("expected LevelOneEquities");
        };
        assert_eq!(items[0].bid_price, Some(dec!(1.0)));
    }

    #[test]
    fn unknown_service_falls_back_to_raw() {
        // A service Schwab adds later that we have not yet typed decodes
        // into `Service::Unknown` and dispatches to `DataContent::Raw` with
        // the raw content array preserved.
        let frame = r#"{
            "data": [{
                "service": "BOND_BOOK",
                "timestamp": 1,
                "command": "SUBS",
                "content": [{"key":"AAA","1":1,"2":2}]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        assert_eq!(
            data[0].service,
            Service::Unknown("BOND_BOOK".to_string()),
            "expected Unknown(BOND_BOOK), got {:?}",
            data[0].service
        );
        match &data[0].content {
            DataContent::Raw(v) => {
                assert!(v.is_array(), "expected raw array, got {v:?}");
            }
            other => panic!("expected Raw fallback, got {other:?}"),
        }
    }

    #[test]
    fn unknown_service_round_trips_through_serde() {
        let svc = Service::Unknown("BOND_BOOK".to_string());
        let json = serde_json::to_string(&svc).unwrap();
        assert_eq!(json, r#""BOND_BOOK""#);
        let restored: Service = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, svc);
    }

    #[test]
    fn malformed_json_returns_decode_error() {
        let result = parse("not json at all");
        match result {
            Err(Error::Codec { .. }) => {}
            other => panic!("expected Decode error, got {other:?}"),
        }
    }

    #[test]
    fn malformed_data_content_returns_decode_error() {
        // `content` is supposed to be an array; passing a number triggers
        // the array-expected branch in `transform_keys`.
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_EQUITIES",
                "timestamp": 1,
                "command": "SUBS",
                "content": 42
            }]
        }"#;
        match parse(frame) {
            Err(Error::Codec { context, .. }) => {
                assert!(context.contains("content"), "context = {context}");
            }
            other => panic!("expected Decode error, got {other:?}"),
        }
    }
}
