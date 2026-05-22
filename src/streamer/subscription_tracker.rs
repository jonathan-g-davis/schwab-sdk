//! Subscription state tracking for reconnect-and-resubscribe.
//!
//! Schwab's streamer is stateful: a fresh WebSocket connection starts with
//! no active subscriptions, so after reconnect the consumer must replay
//! every SUBS the application is still interested in. This helper records
//! the wire effect of each command the consumer sends and produces the
//! minimum set of SUBS frames needed to restore the same state.
//!
//! Usage shape (consumer-owned; this crate does not invoke the tracker):
//!
//! ```ignore
//! let mut tracker = SubscriptionTracker::new();
//!
//! tracker.observe(&request);     // before or after every send
//! writer.send(request).await?;
//!
//! // After a reconnect:
//! for req in tracker.active_requests() {
//!     writer.send(req).await?;
//! }
//! ```
//!
//! Semantics per Schwab command:
//!
//! - `SUBS` replaces both keys and fields for the service.
//! - `ADD` adds keys, replaces fields if a fields parameter is present.
//! - `UNSUBS` removes keys; if the service has no keys left it is dropped.
//! - `VIEW` replaces fields, keeps keys.
//! - `LOGIN`/`LOGOUT` and ADMIN-service frames are ignored.
//!
//! Field sets are tracked as raw `u8` indices so the tracker is agnostic
//! to which service-specific `Field` enum was used; replay rebuilds a
//! generic SUBS frame straight onto the wire.

use std::collections::{BTreeSet, HashMap};

use crate::streamer::{Command, Service, StreamerRequest};

#[derive(Debug, Clone, Default)]
pub struct SubscriptionTracker {
    state: HashMap<Service, ServiceSubscription>,
}

#[derive(Debug, Clone, Default)]
struct ServiceSubscription {
    keys: BTreeSet<String>,
    fields: BTreeSet<u8>,
}

impl SubscriptionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the tracked state to reflect the given request. ADMIN frames
    /// and login/logout commands are no-ops.
    pub fn observe(&mut self, request: &StreamerRequest) {
        if matches!(request.service, Service::Admin) {
            return;
        }
        let (keys, fields) = parse_keys_fields(&request.parameters);

        match request.command {
            Command::Subs => {
                self.state.insert(
                    request.service.clone(),
                    ServiceSubscription { keys, fields },
                );
            }
            Command::Add => {
                let entry = self.state.entry(request.service.clone()).or_default();
                entry.keys.extend(keys);
                if !fields.is_empty() {
                    entry.fields = fields;
                }
            }
            Command::Unsubs => {
                if let Some(entry) = self.state.get_mut(&request.service) {
                    for k in &keys {
                        entry.keys.remove(k);
                    }
                    if entry.keys.is_empty() {
                        self.state.remove(&request.service);
                    }
                }
            }
            Command::View => {
                if let Some(entry) = self.state.get_mut(&request.service) {
                    entry.fields = fields;
                }
            }
            Command::Login | Command::Logout => {}
        }
    }

    /// Build the set of SUBS frames that would restore the currently-tracked
    /// subscriptions. One frame per service with at least one active key.
    /// Order is deterministic across the service set but not specified.
    pub fn active_requests(&self) -> Vec<StreamerRequest> {
        let mut out = Vec::with_capacity(self.state.len());
        for (service, sub) in &self.state {
            if sub.keys.is_empty() {
                continue;
            }
            let keys_csv = sub.keys.iter().cloned().collect::<Vec<_>>().join(",");
            let fields_csv = sub
                .fields
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let parameters = serde_json::json!({
                "keys": keys_csv,
                "fields": fields_csv,
            });
            out.push(StreamerRequest {
                service: service.clone(),
                command: Command::Subs,
                parameters,
            });
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    pub fn clear(&mut self) {
        self.state.clear();
    }
}

fn parse_keys_fields(params: &serde_json::Value) -> (BTreeSet<String>, BTreeSet<u8>) {
    let keys = params
        .get("keys")
        .and_then(|v| v.as_str())
        .map(|s| {
            s.split(',')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let fields = params
        .get("fields")
        .and_then(|v| v.as_str())
        .map(|s| {
            s.split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    (keys, fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streamer::level_one::equities::Field as EquitiesField;
    use crate::streamer::protocol::Command as StreamerCommand;
    use crate::streamer::subscription::{Command, Subscription};

    fn equities_subs(keys: &[&str], fields: &[EquitiesField]) -> StreamerRequest {
        Subscription {
            command: Command::Subscribe,
            keys: keys.iter().map(|s| s.to_string()).collect(),
            fields: fields.to_vec(),
        }
        .into()
    }

    fn equities_add(keys: &[&str], fields: &[EquitiesField]) -> StreamerRequest {
        Subscription {
            command: Command::Add,
            keys: keys.iter().map(|s| s.to_string()).collect(),
            fields: fields.to_vec(),
        }
        .into()
    }

    fn equities_unsubs(keys: &[&str]) -> StreamerRequest {
        Subscription::<EquitiesField> {
            command: Command::Unsubscribe,
            keys: keys.iter().map(|s| s.to_string()).collect(),
            fields: vec![],
        }
        .into()
    }

    fn equities_view(fields: &[EquitiesField]) -> StreamerRequest {
        Subscription::<EquitiesField> {
            command: Command::View,
            keys: vec![],
            fields: fields.to_vec(),
        }
        .into()
    }

    fn keys_csv(req: &StreamerRequest) -> String {
        req.parameters
            .get("keys")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn fields_csv(req: &StreamerRequest) -> String {
        req.parameters
            .get("fields")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    #[test]
    fn empty_tracker_emits_no_requests() {
        let t = SubscriptionTracker::new();
        assert!(t.is_empty());
        assert!(t.active_requests().is_empty());
    }

    #[test]
    fn subs_records_keys_and_fields() {
        let mut t = SubscriptionTracker::new();
        t.observe(&equities_subs(
            &["AAPL", "MSFT"],
            &[EquitiesField::Symbol, EquitiesField::BidPrice],
        ));
        let active = t.active_requests();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].service, Service::LevelOneEquities);
        assert_eq!(active[0].command, StreamerCommand::Subs);
        // BTreeSet sorts lexicographically.
        assert_eq!(keys_csv(&active[0]), "AAPL,MSFT");
        assert_eq!(fields_csv(&active[0]), "0,1");
    }

    #[test]
    fn second_subs_replaces_prior_state_for_service() {
        let mut t = SubscriptionTracker::new();
        t.observe(&equities_subs(&["AAPL"], &[EquitiesField::Symbol]));
        t.observe(&equities_subs(
            &["MSFT", "GOOG"],
            &[EquitiesField::BidPrice, EquitiesField::AskPrice],
        ));
        let active = t.active_requests();
        assert_eq!(keys_csv(&active[0]), "GOOG,MSFT");
        assert_eq!(fields_csv(&active[0]), "1,2");
    }

    #[test]
    fn add_merges_keys_into_existing_subscription() {
        let mut t = SubscriptionTracker::new();
        t.observe(&equities_subs(&["AAPL"], &[EquitiesField::Symbol]));
        t.observe(&equities_add(&["MSFT"], &[]));
        let active = t.active_requests();
        assert_eq!(keys_csv(&active[0]), "AAPL,MSFT");
        assert_eq!(fields_csv(&active[0]), "0");
    }

    #[test]
    fn unsubs_removes_keys_and_drops_empty_services() {
        let mut t = SubscriptionTracker::new();
        t.observe(&equities_subs(&["AAPL", "MSFT"], &[EquitiesField::Symbol]));
        t.observe(&equities_unsubs(&["AAPL"]));
        assert_eq!(keys_csv(&t.active_requests()[0]), "MSFT");

        t.observe(&equities_unsubs(&["MSFT"]));
        assert!(t.is_empty());
    }

    #[test]
    fn view_updates_fields_only() {
        let mut t = SubscriptionTracker::new();
        t.observe(&equities_subs(
            &["AAPL"],
            &[EquitiesField::Symbol, EquitiesField::BidPrice],
        ));
        t.observe(&equities_view(&[
            EquitiesField::LastPrice,
            EquitiesField::TotalVolume,
        ]));
        let active = t.active_requests();
        assert_eq!(keys_csv(&active[0]), "AAPL");
        // LastPrice=3, TotalVolume=8.
        assert_eq!(fields_csv(&active[0]), "3,8");
    }

    #[test]
    fn admin_and_login_logout_are_ignored() {
        let mut t = SubscriptionTracker::new();
        let login: StreamerRequest = crate::streamer::admin::LoginBuilder::default()
            .authorization(crate::model::AuthToken::new("tok"))
            .schwab_client_channel("ch".to_string())
            .schwab_client_function_id("fn".to_string())
            .build()
            .unwrap()
            .into();
        t.observe(&login);
        t.observe(&crate::streamer::StreamerRequest::logout().into());
        assert!(t.is_empty());
    }

    #[test]
    fn clear_drops_all_state() {
        let mut t = SubscriptionTracker::new();
        t.observe(&equities_subs(&["AAPL"], &[EquitiesField::Symbol]));
        t.clear();
        assert!(t.is_empty());
    }

    #[test]
    fn round_trips_through_observe_replay_observe() {
        // After SUBS + ADD, the active_requests output, when re-observed,
        // produces the same end state. This is the property reconnect
        // depends on.
        let mut t = SubscriptionTracker::new();
        t.observe(&equities_subs(&["AAPL"], &[EquitiesField::Symbol]));
        t.observe(&equities_add(&["MSFT"], &[]));
        let replay = t.active_requests();

        let mut t2 = SubscriptionTracker::new();
        for req in &replay {
            t2.observe(req);
        }
        assert_eq!(t2.active_requests().len(), 1);
        assert_eq!(keys_csv(&t2.active_requests()[0]), "AAPL,MSFT");
        assert_eq!(fields_csv(&t2.active_requests()[0]), "0");
    }

    #[test]
    fn tracks_multiple_services_independently() {
        let mut t = SubscriptionTracker::new();
        t.observe(&equities_subs(&["AAPL"], &[EquitiesField::Symbol]));

        use crate::streamer::level_one::options::Field as OptionsField;
        let options_sub: StreamerRequest = Subscription {
            command: Command::Subscribe,
            keys: vec!["AAPL  240315C00200000".to_string()],
            fields: vec![OptionsField::Symbol, OptionsField::Delta],
        }
        .into();
        t.observe(&options_sub);

        let active = t.active_requests();
        assert_eq!(active.len(), 2);
        let services: std::collections::HashSet<_> =
            active.iter().map(|r| r.service.clone()).collect();
        assert!(services.contains(&Service::LevelOneEquities));
        assert!(services.contains(&Service::LevelOneOptions));
    }
}
