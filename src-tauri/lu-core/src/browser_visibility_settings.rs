//! TOOLS-10 browser preference and visibility state contract.
//! The headed browser is mandatory; visibility is controlled by native OS
//! windows, not by launching a different headless Chromium instance.
use serde::{Deserialize,Serialize};

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum BrowserVisibilityPreference {
    AutomaticHidden,
    AlwaysVisible,
}
impl Default for BrowserVisibilityPreference {
    fn default()->Self {Self::AutomaticHidden}
}
impl BrowserVisibilityPreference {
    pub fn worker_mode(self)->&'static str {
        match self {
            Self::AutomaticHidden=>"automatic",
            Self::AlwaysVisible=>"always-visible",
        }
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum BrowserVisibilityState {
    NotStarted,
    Hidden,
    Visible,
    WaitingForHuman,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum VisibilityEvent {
    Launched,
    UserWantsVisible,
    HumanChallenge,
    HumanVerificationSucceeded,
    HumanVerificationStillRequired,
    NativeControllerFailed,
    Stopped,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum VisibilityDecision {
    Noop,
    RequestNativeHide,
    RequestNativeShow,
    PreserveHumanTakeover,
}
pub fn transition(
    current:BrowserVisibilityState,
    preference:BrowserVisibilityPreference,
    event:VisibilityEvent
)->(BrowserVisibilityState,VisibilityDecision) {
    use BrowserVisibilityState as S;
    use VisibilityDecision as D;
    use VisibilityEvent as E;
    match event {
        E::Stopped => (S::NotStarted,D::Noop),
        E::Launched => match preference {
            BrowserVisibilityPreference::AutomaticHidden => (S::Hidden,D::RequestNativeHide),
            BrowserVisibilityPreference::AlwaysVisible => (S::Visible,D::Noop),
        },
        E::UserWantsVisible | E::HumanChallenge =>
            (if matches!(event,E::HumanChallenge) {S::WaitingForHuman} else {S::Visible},
             D::RequestNativeShow),
        E::HumanVerificationStillRequired | E::NativeControllerFailed =>
            (S::WaitingForHuman,D::PreserveHumanTakeover),
        E::HumanVerificationSucceeded => match preference {
            BrowserVisibilityPreference::AutomaticHidden =>
                (S::Hidden,D::RequestNativeHide),
            BrowserVisibilityPreference::AlwaysVisible => (S::Visible,D::Noop),
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_mode_is_headed_hidden_and_explicitly_serializable() {
        let preference=BrowserVisibilityPreference::default();
        assert_eq!(preference.worker_mode(),"automatic");
        let json=serde_json::to_string(&preference).unwrap();
        assert_eq!(json,"\"automatic_hidden\"");
        assert_eq!(serde_json::from_str::<BrowserVisibilityPreference>(&json).unwrap(),preference);
    }
    #[test]
    fn challenge_reveals_same_browser_and_resumes_hidden() {
        let pref=BrowserVisibilityPreference::default();
        assert_eq!(transition(BrowserVisibilityState::NotStarted,pref,
            VisibilityEvent::Launched),(BrowserVisibilityState::Hidden,
            VisibilityDecision::RequestNativeHide));
        assert_eq!(transition(BrowserVisibilityState::Hidden,pref,
            VisibilityEvent::HumanChallenge),(BrowserVisibilityState::WaitingForHuman,
            VisibilityDecision::RequestNativeShow));
        assert_eq!(transition(BrowserVisibilityState::WaitingForHuman,pref,
            VisibilityEvent::HumanVerificationSucceeded),(BrowserVisibilityState::Hidden,
            VisibilityDecision::RequestNativeHide));
    }
    #[test]
    fn failed_visibility_does_not_claim_recovery() {
        let pref=BrowserVisibilityPreference::default();
        assert_eq!(transition(BrowserVisibilityState::WaitingForHuman,pref,
            VisibilityEvent::NativeControllerFailed),(BrowserVisibilityState::WaitingForHuman,
            VisibilityDecision::PreserveHumanTakeover));
    }
    #[test]
    fn always_visible_avoids_automatic_rehide() {
        let pref=BrowserVisibilityPreference::AlwaysVisible;
        assert_eq!(transition(BrowserVisibilityState::WaitingForHuman,pref,
            VisibilityEvent::HumanVerificationSucceeded),(BrowserVisibilityState::Visible,
            VisibilityDecision::Noop));
    }
}
