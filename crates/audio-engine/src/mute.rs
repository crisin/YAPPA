//! Microphone and hearing as one small state machine (A3). Pure data: the engine applies
//! the result to the pump's mute flag, the transport and the mixer, and tells the UI with
//! `Event::MuteState`. Deafened implies muted — nobody wants to talk into a room they
//! cannot hear — and undeafening gives the microphone back the state it had before.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MuteState {
    /// Microphone off by its own switch (key or button).
    muted: bool,
    deafened: bool,
    /// What `muted` was when deafening started, so undeafening can restore it.
    muted_before_deafen: bool,
}

/// What the engine has to apply: send silence (and tell the transport) and mute the mix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effective {
    pub muted: bool,
    pub deafened: bool,
}

impl MuteState {
    /// The microphone is off — by its own switch or because of deafening.
    pub fn mic_muted(&self) -> bool {
        self.muted || self.deafened
    }

    pub fn deafened(&self) -> bool {
        self.deafened
    }

    pub fn effective(&self) -> Effective {
        Effective {
            muted: self.mic_muted(),
            deafened: self.deafened,
        }
    }

    /// Muting is plain. Unmuting while deafened undeafens too: the one switch the user
    /// reaches for should bring them back into the conversation.
    pub fn set_muted(&mut self, muted: bool) {
        if muted {
            self.muted = true;
        } else {
            self.muted = false;
            self.deafened = false;
        }
    }

    pub fn set_deafened(&mut self, deafened: bool) {
        if deafened && !self.deafened {
            self.muted_before_deafen = self.muted;
            self.deafened = true;
        } else if !deafened && self.deafened {
            self.deafened = false;
            self.muted = self.muted_before_deafen;
        }
    }

    /// The mute key: flips what is in effect, so one press while deafened brings sound and
    /// microphone back.
    pub fn toggle_muted(&mut self) {
        self.set_muted(!self.mic_muted());
    }

    pub fn toggle_deafened(&mut self) {
        self.set_deafened(!self.deafened);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eff(muted: bool, deafened: bool) -> Effective {
        Effective { muted, deafened }
    }

    #[test]
    fn the_mute_key_flips_the_microphone() {
        let mut s = MuteState::default();
        assert_eq!(s.effective(), eff(false, false));
        s.toggle_muted();
        assert_eq!(s.effective(), eff(true, false));
        s.toggle_muted();
        assert_eq!(s.effective(), eff(false, false));
    }

    #[test]
    fn deafen_implies_mute_and_undeafen_gives_the_microphone_back_as_it_was() {
        let mut s = MuteState::default();
        s.toggle_deafened();
        assert_eq!(s.effective(), eff(true, true));
        s.toggle_deafened();
        assert_eq!(s.effective(), eff(false, false), "was not muted before");

        s.set_muted(true);
        s.set_deafened(true);
        assert_eq!(s.effective(), eff(true, true));
        s.set_deafened(false);
        assert_eq!(s.effective(), eff(true, false), "stays muted as before");
    }

    #[test]
    fn unmuting_while_deafened_brings_everything_back() {
        let mut s = MuteState::default();
        s.set_deafened(true);
        s.toggle_muted();
        assert_eq!(s.effective(), eff(false, false));
        s.set_deafened(true);
        s.set_muted(false);
        assert_eq!(s.effective(), eff(false, false));
    }

    #[test]
    fn setting_the_same_value_twice_changes_nothing() {
        let mut s = MuteState::default();
        s.set_muted(true);
        s.set_deafened(true);
        s.set_deafened(true);
        s.set_deafened(false);
        assert_eq!(s.effective(), eff(true, false));
        s.set_muted(true);
        assert_eq!(s.effective(), eff(true, false));
    }
}
