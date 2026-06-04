//! Serde adaptors for Schwab's timestamp encodings.
//!
//! Schwab sends point-in-time values two ways: ISO-8601 strings (handled
//! directly by `chrono`'s own serde support) and epoch-millisecond integers.
//! [`millis_opt`] decodes the integer form into `Option<DateTime<Utc>>` so the
//! public API exposes one timestamp type rather than a mix of `i64`/`u64` and
//! `DateTime<Utc>` for the same kind of value.
//!
//! The instruments `fundamental` block is a third case: date-only values sent
//! as `YYYY-MM-DD HH:MM:SS.S` (space-separated, always midnight).
//! [`naive_date_opt`] decodes the date portion into `Option<NaiveDate>`.

/// Serde adaptor for an optional epoch-millisecond timestamp carried on the
/// wire as a JSON integer. Use with `#[serde(default, with = "millis_opt")]`.
///
/// The wire value is interpreted as milliseconds since the Unix epoch. A
/// missing field or JSON `null` decodes to `None`. An out-of-range value is a
/// deserialization error.
pub(crate) mod millis_opt {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<i64>::deserialize(deserializer)? {
            Some(millis) => DateTime::from_timestamp_millis(millis)
                .map(Some)
                .ok_or_else(|| {
                    serde::de::Error::custom(format!("epoch milliseconds out of range: {millis}"))
                }),
            None => Ok(None),
        }
    }
}

/// Serde adaptor for an optional date carried on the wire as a
/// `YYYY-MM-DD HH:MM:SS.S` string (the shape Schwab's `fundamental` block
/// uses, with the time always midnight). Use with
/// `#[serde(default, with = "naive_date_opt")]`.
///
/// Only the date portion is significant, so the leading date token (up to the
/// first space or `T`) is parsed as `%Y-%m-%d`; the time and fractional
/// seconds are ignored. A missing field, JSON `null`, or an empty string
/// decodes to `None`. A non-empty value whose date portion does not parse is a
/// deserialization error.
pub(crate) mod naive_date_opt {
    use chrono::NaiveDate;
    use serde::{Deserialize, Deserializer};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Some(raw) = Option::<String>::deserialize(deserializer)? else {
            return Ok(None);
        };
        let date = raw.trim().split([' ', 'T']).next().unwrap_or("");
        if date.is_empty() {
            return Ok(None);
        }
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map(Some)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Holder {
        #[serde(default, with = "super::millis_opt")]
        ts: Option<DateTime<Utc>>,
    }

    #[test]
    fn integer_decodes_to_utc_datetime() {
        let h: Holder = serde_json::from_str(r#"{"ts": 1621376892336}"#).unwrap();
        assert_eq!(h.ts.unwrap().timestamp_millis(), 1621376892336);
    }

    #[test]
    fn missing_and_null_decode_to_none() {
        let missing: Holder = serde_json::from_str(r#"{}"#).unwrap();
        assert!(missing.ts.is_none());
        let null: Holder = serde_json::from_str(r#"{"ts": null}"#).unwrap();
        assert!(null.ts.is_none());
    }

    #[test]
    fn out_of_range_is_an_error() {
        assert!(serde_json::from_str::<Holder>(r#"{"ts": 9223372036854775807}"#).is_err());
    }

    use chrono::NaiveDate;

    #[derive(Deserialize)]
    struct DateHolder {
        #[serde(default, with = "super::naive_date_opt")]
        d: Option<NaiveDate>,
    }

    #[test]
    fn fundamental_datetime_string_decodes_to_date() {
        // The exact shape observed on the live instruments endpoint.
        let h: DateHolder = serde_json::from_str(r#"{"d": "2026-05-11 00:00:00.0"}"#).unwrap();
        assert_eq!(h.d, NaiveDate::from_ymd_opt(2026, 5, 11));
    }

    #[test]
    fn bare_date_and_t_separator_decode() {
        let bare: DateHolder = serde_json::from_str(r#"{"d": "2026-05-11"}"#).unwrap();
        assert_eq!(bare.d, NaiveDate::from_ymd_opt(2026, 5, 11));
        let t_sep: DateHolder = serde_json::from_str(r#"{"d": "2026-05-11T09:30:00Z"}"#).unwrap();
        assert_eq!(t_sep.d, NaiveDate::from_ymd_opt(2026, 5, 11));
    }

    #[test]
    fn missing_null_and_empty_decode_to_none() {
        let missing: DateHolder = serde_json::from_str(r#"{}"#).unwrap();
        assert!(missing.d.is_none());
        let null: DateHolder = serde_json::from_str(r#"{"d": null}"#).unwrap();
        assert!(null.d.is_none());
        let empty: DateHolder = serde_json::from_str(r#"{"d": ""}"#).unwrap();
        assert!(empty.d.is_none());
    }

    #[test]
    fn malformed_date_is_an_error() {
        assert!(serde_json::from_str::<DateHolder>(r#"{"d": "not-a-date"}"#).is_err());
    }
}
