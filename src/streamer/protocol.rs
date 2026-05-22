/// Schwab streamer service identifier.
///
/// Open enum: any wire string Schwab adds later that does not match a known
/// variant decodes into [`Service::Unknown`] with the raw identifier
/// preserved. Consumers can still see the original name and the dispatcher
/// routes such messages to [`DataContent::Raw`].
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    strum::Display,
    strum::EnumString,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(into = "String", from = "String")]
pub enum Service {
    #[strum(serialize = "ADMIN")]
    Admin,
    #[strum(serialize = "LEVELONE_EQUITIES")]
    LevelOneEquities,
    #[strum(serialize = "LEVELONE_OPTIONS")]
    LevelOneOptions,
    #[strum(serialize = "LEVELONE_FUTURES")]
    LevelOneFutures,
    #[strum(serialize = "LEVELONE_FUTURES_OPTIONS")]
    LevelOneFuturesOptions,
    #[strum(serialize = "LEVELONE_FOREX")]
    LevelOneForex,
    #[strum(serialize = "NYSE_BOOK")]
    NyseBook,
    #[strum(serialize = "NASDAQ_BOOK")]
    NasdaqBook,
    #[strum(serialize = "OPTIONS_BOOK")]
    OptionsBook,
    #[strum(serialize = "CHART_EQUITY")]
    ChartEquity,
    #[strum(serialize = "CHART_FUTURES")]
    ChartFutures,
    #[strum(serialize = "SCREENER_EQUITY")]
    ScreenerEquity,
    #[strum(serialize = "SCREENER_OPTION")]
    ScreenerOption,
    #[strum(serialize = "ACCT_ACTIVITY")]
    AccountActivity,
    /// A service identifier Schwab sent that this crate does not recognize.
    /// The raw wire string is preserved so consumers can route on it.
    #[strum(default)]
    Unknown(String),
}

impl From<Service> for String {
    fn from(s: Service) -> Self {
        s.to_string()
    }
}

impl From<String> for Service {
    fn from(s: String) -> Self {
        // `EnumString` with `#[strum(default)]` makes `FromStr` infallible:
        // unrecognized strings land in `Service::Unknown(s)`.
        s.parse()
            .expect("Service FromStr is infallible (strum default)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Command {
    #[serde(rename = "LOGIN")]
    Login,
    #[serde(rename = "SUBS")]
    Subs,
    #[serde(rename = "ADD")]
    Add,
    #[serde(rename = "UNSUBS")]
    Unsubs,
    #[serde(rename = "VIEW")]
    View,
    #[serde(rename = "LOGOUT")]
    Logout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde_repr::Deserialize_repr)]
#[repr(u8)]
pub enum ResponseCode {
    Ok = 0,
    LoginDenied = 3,
    UnknownFailure = 9,
    ServiceNotAvailable = 11,
    CloseConnection = 12,
    ReachedSymbolLimit = 19,
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
}
