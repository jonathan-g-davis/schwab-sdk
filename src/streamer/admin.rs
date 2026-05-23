use crate::secrets::AuthToken;
use crate::streamer::{Service, StreamerCommand, StreamerRequest};

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Login {
    #[serde(rename = "Authorization")]
    pub authorization: AuthToken,
    #[serde(rename = "SchwabClientChannel")]
    pub schwab_client_channel: String,
    #[serde(rename = "SchwabClientFunctionId")]
    pub schwab_client_function_id: String,
}

impl From<Login> for StreamerRequest {
    fn from(login: Login) -> Self {
        let parameters = serde_json::to_value(login).expect("Login serialization is infallible");
        StreamerRequest {
            service: Service::Admin,
            command: StreamerCommand::Login,
            parameters,
        }
    }
}

pub(crate) struct Logout;

impl From<Logout> for StreamerRequest {
    fn from(_: Logout) -> Self {
        StreamerRequest {
            service: Service::Admin,
            command: StreamerCommand::Logout,
            parameters: serde_json::json!({}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_login() {
        let login = Login {
            authorization: AuthToken::new("1234567890"),
            schwab_client_channel: "test".to_string(),
            schwab_client_function_id: "test".to_string(),
        };

        let serialized = serde_json::to_string(&login).unwrap();
        assert_eq!(
            serialized,
            r#"{"Authorization":"1234567890","SchwabClientChannel":"test","SchwabClientFunctionId":"test"}"#
        );
    }

    #[test]
    fn login_debug_does_not_leak_auth_token() {
        let login = Login {
            authorization: AuthToken::new("super-secret-bearer"),
            schwab_client_channel: "ch".to_string(),
            schwab_client_function_id: "fn".to_string(),
        };
        let debug = format!("{login:?}");
        assert!(
            !debug.contains("super-secret-bearer"),
            "Debug leaked auth token: {debug}"
        );
    }

    #[test]
    fn from_login_never_panics() {
        let login = Login {
            authorization: AuthToken::new(""),
            schwab_client_channel: String::new(),
            schwab_client_function_id: String::new(),
        };
        let _request: StreamerRequest = login.into();

        let login = Login {
            authorization: AuthToken::new("\u{0}\"\\\n"),
            schwab_client_channel: "ch".to_string(),
            schwab_client_function_id: "fn".to_string(),
        };
        let _request: StreamerRequest = login.into();
    }
}
