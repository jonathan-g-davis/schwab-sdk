use crate::error::Result;
use crate::streamer::Service;
use crate::streamer::WriteHalf;
use crate::streamer::protocol::StreamerCommand;
use crate::streamer::request::StreamerRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
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
            StreamerCommand::Login | StreamerCommand::Logout | StreamerCommand::Unknown(_) => {
                Err(format!("Invalid subscription command: {command:?}"))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Subscription<T> {
    pub(super) command: Command,
    pub(super) keys: Vec<String>,
    pub(super) fields: Vec<T>,
}

/// Build the `parameters` payload for a SUBS / ADD / UNSUBS / VIEW frame.
///
/// Returns the JSON object Schwab expects on the wire (`{"keys": "<csv>",
/// "fields": "<csv>"}`).
pub(super) fn subscribe_parameters<F, I>(keys: Vec<String>, fields: I) -> serde_json::Value
where
    F: Into<u8> + Copy,
    I: IntoIterator<Item = F>,
{
    let keys = keys.join(",");
    let fields = fields
        .into_iter()
        .map(|f| f.into().to_string())
        .collect::<Vec<_>>()
        .join(",");
    serde_json::json!({ "keys": keys, "fields": fields })
}

/// Binds a field enum to its streamer service, enabling the generic
/// `From<Subscription<F>> for StreamerRequest` impl below.
pub(crate) trait SubscriptionField: Into<u8> + Copy {
    const SERVICE: Service;
}

impl<F: SubscriptionField> From<Subscription<F>> for StreamerRequest {
    fn from(s: Subscription<F>) -> Self {
        StreamerRequest {
            service: F::SERVICE,
            command: s.command.into(),
            parameters: subscribe_parameters(s.keys, s.fields),
        }
    }
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

// The bound mentions the crate-internal `StreamerRequest`; it is satisfied
// by the generic `impl<F: SubscriptionField>` above, never by external code.
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
