//! Serde adaptors for Schwab's non-RFC3339 date and time encodings.
//!
//! Some Schwab fields encode a date or instant as a plain integer or a non-ISO
//! string that cannot be deserialized directly by `chrono`.
//!
//! Supported formats:
//! - Epoch milliseconds: [`millis_opt`]
//! - `YYYY-MM-DD HH:MM:SS.S` (where time is always midnight): [`naive_date_opt`]
//! - Days since epoch: [`epoch_days_opt`]

/// Serde adaptor for an optional epoch-millisecond timestamp carried on the
/// wire as a JSON integer. Use with `#[serde(default, with = "millis_opt")]`.
///
/// Re-export of [`chrono::serde::ts_milliseconds_option`].
pub(crate) use chrono::serde::ts_milliseconds_option as millis_opt;

/// Serde adaptor for an optional date carried on the wire as a
/// `YYYY-MM-DD HH:MM:SS.S` string (the format Schwab's `fundamental` block
/// uses, with the time always midnight). Use with
/// `#[serde(default, with = "naive_date_opt")]`.
///
/// Only the date portion is significant, so the leading date token (up to the
/// first space or `T`) is parsed as `%Y-%m-%d`; the time and fractional
/// seconds are ignored. A JSON `null` or an empty string decodes to `None`. A
/// non-empty value whose date portion does not parse is a deserialization error.
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

/// Serde adaptor for an optional date carried on the wire as a count of days
/// since the Unix epoch. Use with `#[serde(default, with = "epoch_days_opt")]`.
///
/// Used for the streamer `chart_day` field.
///
/// A missing field or JSON `null` decodes to `None`. A count that overflows the
/// representable date range is a deserialization error.
pub(crate) mod epoch_days_opt {
    use chrono::{DateTime, NaiveDate};
    use serde::{Deserialize, Deserializer};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Some(days) = Option::<i64>::deserialize(deserializer)? else {
            return Ok(None);
        };
        match days
            .checked_mul(86_400)
            .and_then(|secs| DateTime::from_timestamp(secs, 0))
        {
            Some(dt) => Ok(Some(dt.date_naive())),
            None => Err(serde::de::Error::custom(format!(
                "epoch day out of range: {days}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde::Deserialize;

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

    #[derive(Deserialize)]
    struct DayHolder {
        #[serde(default, with = "super::epoch_days_opt")]
        d: Option<NaiveDate>,
    }

    #[test]
    fn epoch_day_count_decodes_to_date() {
        // 20608 days after 1970-01-01 is 2026-06-04 (observed alongside a
        // chart_time on the same day).
        let h: DayHolder = serde_json::from_str(r#"{"d": 20608}"#).unwrap();
        assert_eq!(h.d, NaiveDate::from_ymd_opt(2026, 6, 4));
    }

    #[test]
    fn epoch_day_missing_and_null_decode_to_none() {
        let missing: DayHolder = serde_json::from_str(r#"{}"#).unwrap();
        assert!(missing.d.is_none());
        let null: DayHolder = serde_json::from_str(r#"{"d": null}"#).unwrap();
        assert!(null.d.is_none());
    }

    #[test]
    fn epoch_day_out_of_range_is_an_error() {
        assert!(serde_json::from_str::<DayHolder>(r#"{"d": 9223372036854775807}"#).is_err());
    }
}
