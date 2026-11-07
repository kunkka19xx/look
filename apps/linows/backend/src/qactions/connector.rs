//! A control a declaration supplied, rather than one this app implements.
//! The native adapters in `controls` talk to a system API; this one talks to
//! whichever connector produced the row. Peer of `ConnectorControl.swift`.

use super::{ActionIntent, ActionOutcome, ActionState, SystemControl};

pub struct ConnectorControl {
    /// `declared:<block>#<control>`, so the press reaches whoever produced
    /// the row whatever is selected when it lands.
    action_id: String,
}

impl ConnectorControl {
    pub fn new(action_id: &str) -> Self {
        Self {
            action_id: action_id.to_string(),
        }
    }

    /// What the connector reached, never what was asked for: a connector may
    /// refuse, and a toggle that lies is worse than a slow one.
    fn read(&self, press: bool) -> ActionState {
        let answer = crate::sources::control(&self.action_id, press);
        match answer.error {
            Some(reason) => ActionState::Unavailable { reason },
            None if answer.on => ActionState::On,
            None => ActionState::Off,
        }
    }
}

impl SystemControl for ConnectorControl {
    fn state(&self) -> ActionState {
        self.read(false)
    }

    /// The intent is the connector's to decide, so a press is a press.
    fn apply(&self, _intent: ActionIntent) -> ActionOutcome {
        match self.read(true) {
            ActionState::Unavailable { reason } => ActionOutcome::Failed { message: reason },
            _ => ActionOutcome::Ok { banner: None },
        }
    }
}
