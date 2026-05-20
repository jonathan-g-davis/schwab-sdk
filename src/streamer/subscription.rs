use derive_builder::Builder;
use serde_with::{SerializeAs, StringWithSeparator, formats::CommaSeparator, serde_as};

use crate::streamer::StreamerCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    #[serde(rename = "SUBS")]
    Subscribe,
    #[serde(rename = "ADD")]
    Add,
    #[serde(rename = "UNSUBS")]
    Unsubscribe,
    #[serde(rename = "VIEW")]
    View,
}

impl From<Command> for StreamerCommand {
    fn from(command: Command) -> Self {
        match command {
            Command::Subscribe => StreamerCommand::Subs,
            Command::Add => StreamerCommand::Add,
            Command::Unsubscribe => StreamerCommand::Unsubs,
            Command::View => StreamerCommand::View,
        }
    }
}

#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned")]
pub struct Subscription<T> {
    pub(super) command: Command,
    pub(super) keys: Vec<String>,
    pub(super) fields: Vec<T>,
}

#[serde_as]
#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct SubscriptionParameters<T: Into<u8> + Copy> {
    #[serde(rename = "keys")]
    #[serde_as(as = "StringWithSeparator<CommaSeparator, String>")]
    pub(super) keys: Vec<String>,
    #[serde(rename = "fields")]
    #[serde(serialize_with = "fields_serializer")]
    pub(super) fields: Vec<T>,
}

fn fields_serializer<S, T>(fields: &[T], serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    T: Into<u8> + Copy,
{
    let fields_iter = fields
        .iter()
        .map(|f| (*f).into().to_string())
        .collect::<Vec<String>>();
    StringWithSeparator::<CommaSeparator, String>::serialize_as(&fields_iter, serializer)
}
