//! Serde adaptors for Schwab's timestamp encodings.
//!
//! Schwab sends point-in-time values two ways: ISO-8601 strings (handled
//! directly by `chrono`'s own serde support) and epoch-millisecond integers.
//! [`millis_opt`] decodes the integer form into `Option<DateTime<Utc>>` so the
//! public API exposes one timestamp type rather than a mix of `i64`/`u64` and
//! `DateTime<Utc>` for the same kind of value.

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
}
