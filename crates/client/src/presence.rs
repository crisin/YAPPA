//! Presence as the UI needs it: everybody online with their state, me included, rebuilt
//! from the presence room's snapshots and events (ADR-005, S6). The meaning of the
//! attributes is `api_types::PresenceState`; the room itself is `transport::presence`.

use api_types::{PresenceEntry, PresenceState, VoiceToken};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use transport::presence::{PresenceEvent, PresenceLink, PresencePeer};
use transport::LinkState;

/// The folded view. Pure data, so it is tested without a server.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PresenceView {
    me: Option<String>,
    my_name: String,
    my_state: PresenceState,
    peers: BTreeMap<String, PresenceEntry>,
    pub link: Option<LinkState>,
}

fn entry(peer: &PresencePeer) -> PresenceEntry {
    PresenceEntry {
        user_id: peer.identity.clone(),
        display_name: if peer.name.is_empty() {
            peer.identity.clone()
        } else {
            peer.name.clone()
        },
        state: PresenceState::from_attributes(&peer.attributes),
    }
}

impl PresenceView {
    pub fn new(my_name: &str, my_state: PresenceState) -> Self {
        Self {
            my_name: my_name.into(),
            my_state,
            ..Self::default()
        }
    }

    /// Returns whether the visible list changed.
    pub fn apply(&mut self, event: &PresenceEvent) -> bool {
        match event {
            PresenceEvent::Snapshot { me, peers } => {
                self.me = Some(me.clone());
                self.peers = peers
                    .iter()
                    .map(|p| (p.identity.clone(), entry(p)))
                    .collect();
                true
            }
            PresenceEvent::Upsert(peer) => {
                let new = entry(peer);
                self.peers.insert(peer.identity.clone(), new.clone()) != Some(new)
            }
            PresenceEvent::Left { identity } => self.peers.remove(identity).is_some(),
            PresenceEvent::State(state) => {
                self.link = Some(*state);
                false
            }
            PresenceEvent::Failed { retryable, .. } => {
                if !retryable {
                    // Kicked or refused: nobody is visible any more, me included.
                    self.peers.clear();
                    self.me = None;
                }
                true
            }
        }
    }

    pub fn set_mine(&mut self, state: PresenceState) {
        self.my_state = state;
    }

    pub fn mine(&self) -> &PresenceState {
        &self.my_state
    }

    /// Me first (once connected), then the others by name.
    pub fn entries(&self) -> Vec<PresenceEntry> {
        let mut others: Vec<PresenceEntry> = self.peers.values().cloned().collect();
        others.sort_by_key(|e| e.display_name.to_lowercase());
        let me = self.me.as_ref().map(|id| PresenceEntry {
            user_id: id.clone(),
            display_name: self.my_name.clone(),
            state: self.my_state.clone(),
        });
        me.into_iter().chain(others).collect()
    }
}

type Notify = Arc<dyn Fn(Vec<PresenceEntry>) + Send + Sync>;

/// A live presence connection plus its view. Dropping it leaves the room.
pub struct Presence {
    view: Arc<Mutex<PresenceView>>,
    notify: Notify,
    link: PresenceLink,
}

impl Presence {
    pub fn start(
        token: &VoiceToken,
        initial: PresenceState,
        on_change: impl Fn(Vec<PresenceEntry>) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let name = name_from_token(&token.token).unwrap_or_default();
        let view = Arc::new(Mutex::new(PresenceView::new(&name, initial.clone())));
        let notify: Notify = Arc::new(on_change);
        let (for_link, notify_link) = (view.clone(), notify.clone());
        let link = PresenceLink::start(
            &token.ws_url,
            &token.token,
            Box::new(move |event| {
                let (changed, entries) = {
                    let mut view = for_link.lock().expect("lock");
                    let changed = view.apply(&event);
                    (changed, view.entries())
                };
                if changed {
                    notify_link(entries);
                }
            }),
        )?;
        link.set_attributes(initial.to_attributes());
        Ok(Self { view, notify, link })
    }

    /// Changes my state (channel, talk mode, away) — sent to everybody, shown at once here.
    pub fn update(&self, change: impl FnOnce(&mut PresenceState)) {
        let (attributes, entries) = {
            let mut view = self.view.lock().expect("lock");
            let mut state = view.mine().clone();
            change(&mut state);
            view.set_mine(state.clone());
            (state.to_attributes(), view.entries())
        };
        self.link.set_attributes(attributes);
        (self.notify)(entries);
    }

    pub fn entries(&self) -> Vec<PresenceEntry> {
        self.view.lock().expect("lock").entries()
    }

    pub fn mine(&self) -> PresenceState {
        self.view.lock().expect("lock").mine().clone()
    }
}

/// The display name is in the token (the control plane put it there); the presence room
/// tells it back only after connecting, and the UI wants it right away.
fn name_from_token(jwt: &str) -> Option<String> {
    use base64::Engine as _;
    let payload = jwt.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    json.get("name")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_types::TalkMode;
    use std::collections::HashMap;

    fn peer(id: &str, name: &str, channel: &str) -> PresencePeer {
        PresencePeer {
            identity: id.into(),
            name: name.into(),
            attributes: HashMap::from([
                ("channel".to_string(), channel.to_string()),
                ("talk".to_string(), "ptt".to_string()),
            ]),
        }
    }

    #[test]
    fn snapshots_replace_and_events_update_the_view() {
        let mut view = PresenceView::new("Krille", PresenceState::default());
        assert!(
            view.entries().is_empty(),
            "nothing before the first snapshot"
        );
        view.apply(&PresenceEvent::Snapshot {
            me: "k".into(),
            peers: vec![peer("b", "ben", "raid"), peer("s", "Sasha", "")],
        });
        let names: Vec<_> = view.entries().into_iter().map(|e| e.display_name).collect();
        assert_eq!(names, ["Krille", "ben", "Sasha"]);
        assert_eq!(view.entries()[1].state.channel_id.as_deref(), Some("raid"));
        assert_eq!(view.entries()[1].state.talk, TalkMode::Ptt);

        assert!(view.apply(&PresenceEvent::Upsert(peer("b", "ben", "lobby"))));
        assert!(
            !view.apply(&PresenceEvent::Upsert(peer("b", "ben", "lobby"))),
            "no change"
        );
        assert!(view.apply(&PresenceEvent::Left {
            identity: "s".into()
        }));
        assert_eq!(view.entries().len(), 2);

        // A reconnect brings the full picture; whoever left meanwhile is gone.
        view.apply(&PresenceEvent::Snapshot {
            me: "k".into(),
            peers: vec![],
        });
        assert_eq!(view.entries().len(), 1);
    }

    #[test]
    fn a_kick_empties_the_view_but_a_retry_keeps_it() {
        let mut view = PresenceView::new("Krille", PresenceState::default());
        view.apply(&PresenceEvent::Snapshot {
            me: "k".into(),
            peers: vec![peer("b", "Ben", "")],
        });
        view.apply(&PresenceEvent::Failed {
            message: "network".into(),
            retryable: true,
        });
        assert_eq!(view.entries().len(), 2);
        view.apply(&PresenceEvent::Failed {
            message: "kicked".into(),
            retryable: false,
        });
        assert!(view.entries().is_empty());
    }

    #[test]
    fn the_name_comes_from_the_token() {
        // {"name":"Krille","sub":"k"} as a JWT payload, header and signature irrelevant.
        let jwt = "x.eyJuYW1lIjoiS3JpbGxlIiwic3ViIjoiayJ9.y";
        assert_eq!(name_from_token(jwt).as_deref(), Some("Krille"));
        assert_eq!(name_from_token("garbage"), None);
    }
}
