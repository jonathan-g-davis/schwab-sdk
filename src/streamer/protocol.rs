use crate::macros::string_enum;

string_enum! {
    /// Schwab streamer service identifier.
    ///
    /// Open enum: any wire string Schwab adds later that does not match a known
    /// variant decodes into [`Service::Unknown`] with the raw identifier
    /// preserved. Consumers can still see the original name and the dispatcher
    /// routes such messages to [`DataContent::Raw`].
    Service {
        Admin = "ADMIN",
        LevelOneEquities = "LEVELONE_EQUITIES",
        LevelOneOptions = "LEVELONE_OPTIONS",
        LevelOneFutures = "LEVELONE_FUTURES",
        LevelOneFuturesOptions = "LEVELONE_FUTURES_OPTIONS",
        LevelOneForex = "LEVELONE_FOREX",
        NyseBook = "NYSE_BOOK",
        NasdaqBook = "NASDAQ_BOOK",
        OptionsBook = "OPTIONS_BOOK",
        ChartEquity = "CHART_EQUITY",
        ChartFutures = "CHART_FUTURES",
        ScreenerEquity = "SCREENER_EQUITY",
        ScreenerOption = "SCREENER_OPTION",
        AccountActivity = "ACCT_ACTIVITY",
    }
}

string_enum! {
    /// A command string Schwab sends on a streamer frame.
    /// 
    /// Open enum: a command string Schwab adds later that does not match a
    /// known variant decodes into [`StreamerCommand::Unknown`] with the raw
    /// wire value preserved, so an unrecognized command never fails the whole
    /// frame.
    StreamerCommand {
        Login = "LOGIN",
        Subs = "SUBS",
        Add = "ADD",
        Unsubs = "UNSUBS",
        View = "VIEW",
        Logout = "LOGOUT",
    }
}

/// Status code on a streamer `response` frame, reporting the outcome of the
/// command the frame acknowledges.
///
/// Open enum: a numeric code Schwab adds later that does not match a known
/// variant decodes into [`ResponseCode::Unknown`] with the raw value
/// preserved, so an unrecognized code never fails the whole frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(from = "u8")]
#[non_exhaustive]
pub enum ResponseCode {
    Ok,
    LoginDenied,
    UnknownFailure,
    ServiceNotAvailable,
    CloseConnection,
    ReachedSymbolLimit,
    StreamConnNotFound,
    BadCommandFormat,
    FailedCommandSubs,
    FailedCommandUnsubs,
    FailedCommandAdd,
    FailedCommandView,
    SucceededCommandSubs,
    SucceededCommandUnsubs,
    SucceededCommandAdd,
    SucceededCommandView,
    StopStreaming,
    /// A status code Schwab sent that this crate does not recognize. The
    /// raw wire value is preserved so callers can still route on it.
    Unknown(u8),
}

impl From<u8> for ResponseCode {
    fn from(code: u8) -> Self {
        match code {
            0 => ResponseCode::Ok,
            3 => ResponseCode::LoginDenied,
            9 => ResponseCode::UnknownFailure,
            11 => ResponseCode::ServiceNotAvailable,
            12 => ResponseCode::CloseConnection,
            19 => ResponseCode::ReachedSymbolLimit,
            20 => ResponseCode::StreamConnNotFound,
            21 => ResponseCode::BadCommandFormat,
            22 => ResponseCode::FailedCommandSubs,
            23 => ResponseCode::FailedCommandUnsubs,
            24 => ResponseCode::FailedCommandAdd,
            25 => ResponseCode::FailedCommandView,
            26 => ResponseCode::SucceededCommandSubs,
            27 => ResponseCode::SucceededCommandUnsubs,
            28 => ResponseCode::SucceededCommandAdd,
            29 => ResponseCode::SucceededCommandView,
            30 => ResponseCode::StopStreaming,
            other => ResponseCode::Unknown(other),
        }
    }
}

impl ResponseCode {
    /// `true` if the code reports that the acknowledged command succeeded.
    /// Every other code, including [`ResponseCode::Unknown`], is a failure
    /// or a connection-lifecycle signal the caller must handle.
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            ResponseCode::Ok
                | ResponseCode::SucceededCommandSubs
                | ResponseCode::SucceededCommandUnsubs
                | ResponseCode::SucceededCommandAdd
                | ResponseCode::SucceededCommandView
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_response_codes_deserialize() {
        assert_eq!(
            serde_json::from_str::<ResponseCode>("0").unwrap(),
            ResponseCode::Ok
        );
        assert_eq!(
            serde_json::from_str::<ResponseCode>("3").unwrap(),
            ResponseCode::LoginDenied
        );
        assert_eq!(
            serde_json::from_str::<ResponseCode>("30").unwrap(),
            ResponseCode::StopStreaming
        );
    }

    #[test]
    fn unknown_response_code_falls_back() {
        // A code Schwab assigns after this crate was published must not
        // fail the response frame.
        assert_eq!(
            serde_json::from_str::<ResponseCode>("99").unwrap(),
            ResponseCode::Unknown(99)
        );
    }

    #[test]
    fn unassigned_codes_in_range_fall_back_to_unknown() {
        // Values with no documented meaning (1, 2, 4-8, 10, ...) decode as
        // Unknown rather than failing.
        assert_eq!(
            serde_json::from_str::<ResponseCode>("1").unwrap(),
            ResponseCode::Unknown(1)
        );
        assert_eq!(
            serde_json::from_str::<ResponseCode>("10").unwrap(),
            ResponseCode::Unknown(10)
        );
    }

    #[test]
    fn out_of_range_code_is_a_decode_error() {
        // The wire value is a u8; a larger number is a genuine decode
        // failure, not an Unknown code.
        assert!(serde_json::from_str::<ResponseCode>("256").is_err());
    }

    #[test]
    fn is_success_only_for_ok_and_succeeded_codes() {
        assert!(ResponseCode::Ok.is_success());
        assert!(ResponseCode::SucceededCommandSubs.is_success());
        assert!(ResponseCode::SucceededCommandUnsubs.is_success());
        assert!(ResponseCode::SucceededCommandAdd.is_success());
        assert!(ResponseCode::SucceededCommandView.is_success());

        assert!(!ResponseCode::LoginDenied.is_success());
        assert!(!ResponseCode::FailedCommandSubs.is_success());
        assert!(!ResponseCode::ServiceNotAvailable.is_success());
        assert!(!ResponseCode::StopStreaming.is_success());
        assert!(!ResponseCode::Unknown(99).is_success());
    }

    #[test]
    fn known_streamer_commands_round_trip() {
        for cmd in [
            StreamerCommand::Login,
            StreamerCommand::Subs,
            StreamerCommand::Add,
            StreamerCommand::Unsubs,
            StreamerCommand::View,
            StreamerCommand::Logout,
        ] {
            let json = serde_json::to_string(&cmd).unwrap();
            let back: StreamerCommand = serde_json::from_str(&json).unwrap();
            assert_eq!(cmd, back);
        }
    }

    #[test]
    fn unknown_streamer_command_falls_back() {
        // A command string Schwab adds after this crate was published must
        // not fail the response frame; the raw value is preserved so
        // callers can route on it.
        let parsed: StreamerCommand = serde_json::from_str(r#""SOMETHING_NEW""#).unwrap();
        assert_eq!(parsed, StreamerCommand::Unknown("SOMETHING_NEW".into()));
        // Serializes back to the same raw value.
        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            r#""SOMETHING_NEW""#
        );
    }
}
