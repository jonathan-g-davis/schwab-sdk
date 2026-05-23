use derive_builder::Builder;
use serde_with::{SerializeAs, StringWithSeparator, formats::CommaSeparator, serde_as};

use crate::error::Result;
use crate::streamer::WriteHalf;
use crate::streamer::protocol::StreamerCommand;
use crate::streamer::request::StreamerRequest;

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

impl TryFrom<StreamerCommand> for Command {
    type Error = String;

    fn try_from(command: StreamerCommand) -> std::result::Result<Self, Self::Error> {
        match command {
            StreamerCommand::Subs => Ok(Command::Subscribe),
            StreamerCommand::Add => Ok(Command::Add),
            StreamerCommand::Unsubs => Ok(Command::Unsubscribe),
            StreamerCommand::View => Ok(Command::View),
            StreamerCommand::Login | StreamerCommand::Logout => {
                Err(format!("Invalid subscription command: {command:?}"))
            }
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

// --- Typestate builder for subscribe/add/unsubscribe/view requests ---

/// Builder state: no verb has been picked yet. Only the verb methods
/// (`subscribe` / `add` / `unsubscribe` / `view`) are callable; `fields()`
/// and `send()` are not. Transition by calling one of the verbs.
pub struct NeedsVerb;

/// Builder state: a verb has been picked. `fields()` is callable and
/// `send()` writes the frame. Verb methods are not callable on this state
/// (commit to one verb per request).
pub struct Ready {
    command: Command,
}

/// Fluent subscribe/add/unsubscribe/view request bound to a [`WriteHalf`].
///
/// Constructed via the per-service accessors on [`WriteHalf`] (e.g.
/// [`WriteHalf::equities`]). The verb method (`subscribe` / `add` /
/// `unsubscribe` / `view`) transitions the builder from
/// [`NeedsVerb`] to [`Ready`]; the type system then makes `fields(...)` and
/// `send()` reachable. Calling `send()` without first picking a verb is a
/// compile-time error, not a runtime one.
#[must_use = "call .send() to write the streamer frame"]
pub struct SubscribeRequest<'a, F, S = NeedsVerb> {
    write_half: &'a WriteHalf,
    state: S,
    keys: Vec<String>,
    fields: Vec<F>,
}

impl<'a, F> SubscribeRequest<'a, F, NeedsVerb> {
    pub(crate) fn new(write_half: &'a WriteHalf) -> Self {
        Self {
            write_half,
            state: NeedsVerb,
            keys: Vec::new(),
            fields: Vec::new(),
        }
    }

    fn with_command<I, T>(self, command: Command, keys: I) -> SubscribeRequest<'a, F, Ready>
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        SubscribeRequest {
            write_half: self.write_half,
            state: Ready { command },
            keys: keys.into_iter().map(Into::into).collect(),
            fields: self.fields,
        }
    }

    /// SUBS: subscribe to `keys`, replacing any prior subscription on this
    /// service for the session.
    pub fn subscribe<I, T>(self, keys: I) -> SubscribeRequest<'a, F, Ready>
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.with_command(Command::Subscribe, keys)
    }

    /// ADD: add `keys` to the existing subscription on this service.
    #[allow(clippy::should_implement_trait)]
    pub fn add<I, T>(self, keys: I) -> SubscribeRequest<'a, F, Ready>
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.with_command(Command::Add, keys)
    }

    /// UNSUBS: remove `keys` from the existing subscription on this
    /// service. Fields are not used by Schwab for this command.
    pub fn unsubscribe<I, T>(self, keys: I) -> SubscribeRequest<'a, F, Ready>
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.with_command(Command::Unsubscribe, keys)
    }

    /// VIEW: change the field selection for `keys` without re-subscribing.
    pub fn view<I, T>(self, keys: I) -> SubscribeRequest<'a, F, Ready>
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.with_command(Command::View, keys)
    }
}

impl<F> SubscribeRequest<'_, F, Ready> {
    /// Set the field selection for the request. Required by Schwab for
    /// SUBS, ADD, and VIEW; ignored for UNSUBS.
    pub fn fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = F>,
    {
        self.fields = fields.into_iter().collect();
        self
    }
}

// The bound mentions the crate-internal IR `StreamerRequest`; it is
// satisfied by the per-service `From<Subscription<F>>` impls inside this
// crate, never by external code, so the lint warning is expected.
#[allow(private_bounds)]
impl<F> SubscribeRequest<'_, F, Ready>
where
    Subscription<F>: Into<StreamerRequest>,
{
    /// Serialize the request and write it as a single streamer frame.
    /// Returns when the frame has been handed to the socket; the matching
    /// `response` frame arrives later on the read half.
    pub async fn send(self) -> Result<()> {
        let subscription = Subscription {
            command: self.state.command,
            keys: self.keys,
            fields: self.fields,
        };
        self.write_half.send(subscription.into()).await
    }
}
