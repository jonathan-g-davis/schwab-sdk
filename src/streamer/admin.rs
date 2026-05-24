use crate::secrets::AuthToken;
use crate::streamer::{Service, StreamerCommand, StreamerRequest};

#[derive(Debug, Clone)]
pub(crate) struct Login {
    pub authorization: AuthToken,
    pub schwab_client_channel: String,
    pub schwab_client_function_id: String,
}

impl From<Login> for StreamerRequest {
    fn from(login: Login) -> Self {
        StreamerRequest {
            service: Service::Admin,
            command: StreamerCommand::Login,
            parameters: serde_json::json!({
                "Authorization": login.authorization.expose_secret(),
                "SchwabClientChannel": login.schwab_client_channel,
                "SchwabClientFunctionId": login.schwab_client_function_id,
            }),
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
    fn login_frame_parameters_encode_fields() {
        let login = Login {
            authorization: AuthToken::new("1234567890"),
            schwab_client_channel: "channel".to_string(),
            schwab_client_function_id: "fn-id".to_string(),
        };
        let request: StreamerRequest = login.into();
        assert_eq!(request.parameters["Authorization"], "1234567890");
        assert_eq!(request.parameters["SchwabClientChannel"], "channel");
        assert_eq!(request.parameters["SchwabClientFunctionId"], "fn-id");
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
