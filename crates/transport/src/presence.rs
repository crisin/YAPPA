//! The presence room (ADR-005): every client sits in the LiveKit room `presence` for as long
//! as the app runs, publishes no media and carries its state as participant attributes.
//! This module knows LiveKit and attributes as plain strings — what the keys mean is
//! `api_types::PresenceState`'s business (in `crates/client`).
//!
//! Spike S6: build the view from `remote_participants()` at every (re)connect plus events,
//! never from events alone — a join and a leave in one server update produce no event.

use crate::LinkState;
use livekit::prelude::*;
use livekit::DisconnectReason;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

/// One person in the presence room.
#[derive(Debug, Clone, PartialEq)]
pub struct PresencePeer {
    pub identity: String,
    pub name: String,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PresenceEvent {
    State(LinkState),
    /// The full room after a (re)connect: who I am and everybody else.
    Snapshot {
        me: String,
        peers: Vec<PresencePeer>,
    },
    /// Somebody came, or their name or attributes changed. Always the whole peer.
    Upsert(PresencePeer),
    Left {
        identity: String,
    },
    /// The link is gone. `retryable == false`: kicked, duplicate login, token refused — the
    /// link stops trying; otherwise it is already trying again.
    Failed {
        message: String,
        retryable: bool,
    },
}

pub type PresenceFn = Box<dyn Fn(PresenceEvent) + Send + Sync>;

enum Control {
    SetAttributes(HashMap<String, String>),
    Close,
}

/// A running presence connection. Dropping it leaves the room.
pub struct PresenceLink {
    control: mpsc::UnboundedSender<Control>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// Waits between attempts after a lost link: 2 s, 4 s, … capped at 30 s.
fn backoff(attempt: u32) -> Duration {
    Duration::from_secs(u64::from(2 * attempt.max(1)).min(30))
}

impl PresenceLink {
    /// Connects in the background; outcomes arrive as events.
    pub fn start(url: &str, token: &str, events: PresenceFn) -> std::io::Result<Self> {
        let (control, mut control_rx) = mpsc::unbounded_channel();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("yappa-presence")
            .enable_all()
            .build()?;
        let (url, token) = (url.to_string(), token.to_string());
        let thread = std::thread::Builder::new()
            .name("yappa-presence".into())
            .spawn(move || runtime.block_on(run(&url, &token, &events, &mut control_rx)))?;
        Ok(Self {
            control,
            thread: Some(thread),
        })
    }

    /// Replaces my attributes (send every key; an empty value deletes it on the server).
    /// Remembered and sent again after a reconnect.
    pub fn set_attributes(&self, attributes: HashMap<String, String>) {
        let _ = self.control.send(Control::SetAttributes(attributes));
    }
}

impl Drop for PresenceLink {
    fn drop(&mut self) {
        let _ = self.control.send(Control::Close);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn peer(p: &RemoteParticipant) -> PresencePeer {
    PresencePeer {
        identity: p.identity().to_string(),
        name: p.name(),
        attributes: p.attributes(),
    }
}

enum Ended {
    /// We asked for it.
    Closed,
    Lost {
        message: String,
        retryable: bool,
    },
}

async fn run(
    url: &str,
    token: &str,
    events: &PresenceFn,
    control: &mut mpsc::UnboundedReceiver<Control>,
) {
    let mut mine: Option<HashMap<String, String>> = None;
    let mut attempt = 0;
    loop {
        events(PresenceEvent::State(if attempt == 0 {
            LinkState::Connecting
        } else {
            LinkState::Reconnecting
        }));
        match session(url, token, events, control, &mut mine, &mut attempt).await {
            Ended::Closed => break,
            Ended::Lost { message, retryable } => {
                tracing::warn!(%message, retryable, "presence link lost");
                events(PresenceEvent::Failed { message, retryable });
                if !retryable {
                    break;
                }
            }
        }
        attempt += 1;
        // Wait, but stay responsive: a close ends the wait, new attributes are kept.
        let wait = tokio::time::sleep(backoff(attempt));
        tokio::pin!(wait);
        loop {
            tokio::select! {
                () = &mut wait => break,
                message = control.recv() => match message {
                    Some(Control::SetAttributes(attributes)) => mine = Some(attributes),
                    Some(Control::Close) | None => {
                        events(PresenceEvent::State(LinkState::Disconnected));
                        return;
                    }
                },
            }
        }
    }
    events(PresenceEvent::State(LinkState::Disconnected));
}

async fn session(
    url: &str,
    token: &str,
    events: &PresenceFn,
    control: &mut mpsc::UnboundedReceiver<Control>,
    mine: &mut Option<HashMap<String, String>>,
    attempt: &mut u32,
) -> Ended {
    let mut options = RoomOptions::default();
    options.auto_subscribe = false;
    // A close must not wait for a slow or hanging connect (realtime review C1): race it
    // against the control channel; new attributes meanwhile are only remembered.
    let connect = Room::connect(url, token, options);
    tokio::pin!(connect);
    let connected = loop {
        tokio::select! {
            result = &mut connect => break result,
            message = control.recv() => match message {
                Some(Control::SetAttributes(attributes)) => *mine = Some(attributes),
                Some(Control::Close) | None => return Ended::Closed,
            },
        }
    };
    let (room, mut room_events) = match connected {
        Ok(connected) => connected,
        Err(e) => {
            let message = format!("could not connect to {url}: {e}");
            let refused = ["401", "403", "Unauthorized", "Forbidden"];
            return Ended::Lost {
                retryable: !refused.iter().any(|sign| message.contains(sign)),
                message,
            };
        }
    };
    *attempt = 0;
    let me = room.local_participant().identity().to_string();
    tracing::info!(%me, "presence connected");
    events(PresenceEvent::Snapshot {
        me,
        peers: room.remote_participants().values().map(peer).collect(),
    });
    events(PresenceEvent::State(LinkState::Connected));
    if let Some(attributes) = mine.clone() {
        apply(&room, attributes);
    }

    let ended = loop {
        tokio::select! {
            message = control.recv() => match message {
                Some(Control::SetAttributes(attributes)) => {
                    *mine = Some(attributes.clone());
                    apply(&room, attributes);
                }
                Some(Control::Close) | None => break Ended::Closed,
            },
            event = room_events.recv() => match event {
                Some(RoomEvent::ParticipantConnected(p)) => events(PresenceEvent::Upsert(peer(&p))),
                Some(RoomEvent::ParticipantDisconnected(p)) => events(PresenceEvent::Left {
                    identity: p.identity().to_string(),
                }),
                Some(
                    RoomEvent::ParticipantAttributesChanged { participant, .. }
                    | RoomEvent::ParticipantNameChanged { participant, .. },
                ) => {
                    if let Participant::Remote(p) = participant {
                        events(PresenceEvent::Upsert(peer(&p)));
                    }
                }
                Some(RoomEvent::Reconnecting) => events(PresenceEvent::State(LinkState::Reconnecting)),
                Some(RoomEvent::Reconnected) => {
                    // Whatever happened meanwhile: the full picture again (S6).
                    events(PresenceEvent::Snapshot {
                        me: room.local_participant().identity().to_string(),
                        peers: room.remote_participants().values().map(peer).collect(),
                    });
                    events(PresenceEvent::State(LinkState::Connected));
                }
                Some(RoomEvent::Disconnected { reason }) => {
                    break match reason {
                        DisconnectReason::ClientInitiated => Ended::Closed,
                        reason => Ended::Lost {
                            message: format!("presence disconnected: {reason:?}"),
                            retryable: !matches!(
                                reason,
                                DisconnectReason::DuplicateIdentity
                                    | DisconnectReason::ParticipantRemoved
                                    | DisconnectReason::RoomDeleted
                            ),
                        },
                    };
                }
                Some(_) => {}
                None => break Ended::Lost {
                    message: "the presence room's event stream ended".into(),
                    retryable: true,
                },
            },
        }
    };
    if matches!(ended, Ended::Closed) {
        let _ = tokio::time::timeout(Duration::from_secs(2), room.close()).await;
    }
    ended
}

/// Sends my attributes in a task of its own: waiting for the server's answer must not keep
/// the loop from hearing a close (realtime review C1). Changes are seconds apart, so two
/// overlapping sends do not happen in practice; if they did, the server keeps the last.
fn apply(room: &Room, attributes: HashMap<String, String>) {
    let participant = room.local_participant();
    tokio::spawn(async move {
        if let Err(error) = participant.set_attributes(attributes).await {
            tracing::warn!(%error, "could not set presence attributes");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::backoff;
    use std::time::Duration;

    #[test]
    fn retries_slow_down_and_cap_at_thirty_seconds() {
        assert_eq!(backoff(1), Duration::from_secs(2));
        assert_eq!(backoff(3), Duration::from_secs(6));
        assert_eq!(backoff(100), Duration::from_secs(30));
    }
}
