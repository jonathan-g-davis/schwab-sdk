use derive_builder::Builder;

use crate::streamer::{Service, StreamerCommand, StreamerRequest};

#[derive(Debug, Clone, serde::Serialize, Builder)]
#[builder(pattern = "owned")]
pub struct Login {
    #[serde(rename = "Authorization")]
    authorization: String,
    #[serde(rename = "SchwabClientChannel")]
    schwab_client_channel: String,
    #[serde(rename = "SchwabClientFunctionId")]
    schwab_client_function_id: String,
}

impl From<Login> for StreamerRequest {
    fn from(login: Login) -> Self {
        StreamerRequest {
            service: Service::Admin,
            command: StreamerCommand::Login,
            parameters: serde_json::to_value(login).unwrap(),
        }
    }
}

pub struct Logout;

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
        let login = LoginBuilder::default()
            .authorization("1234567890".to_string())
            .schwab_client_channel("test".to_string())
            .schwab_client_function_id("test".to_string())
            .build()
            .unwrap();

        let serialized = serde_json::to_string(&login).unwrap();
        assert_eq!(
            serialized,
            r#"{"Authorization":"1234567890","SchwabClientChannel":"test","SchwabClientFunctionId":"test"}"#
        );
    }
}
