//! OBSERVE-EGRESS-01 — explicit origin and destination policy.
//! A model instruction is NOT a network permission check. Native consumers
//! must preserve this taint and fail closed at a host-owned egress boundary.
//! Existing local loopback inference is separate from internet research.
use serde::{Deserialize,Serialize};
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum DataOrigin { ObserveLearn, OrdinaryUser }
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Destination { LocalUi, LocalMemory, LocalInference, NetworkSocket, NetworkWorker, RemoteModel, BrowserNavigation }
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataProvenance {
    pub origin:DataOrigin,
    pub network_egress_allowed:bool,
}
impl DataProvenance{
    pub const OBSERVE_LEARN:Self=Self{origin:DataOrigin::ObserveLearn,network_egress_allowed:false};
    pub fn gate(self, destination:Destination)->Result<(),&'static str>{
        if !self.network_egress_allowed
            &&matches!(destination,Destination::NetworkSocket|Destination::NetworkWorker|Destination::RemoteModel|Destination::BrowserNavigation){
            return Err("Observe & Learn content is local-only: network egress denied");
        }
        // A forged allow=true NEVER declassifies Observe & Learn.
        if self.origin==DataOrigin::ObserveLearn && self.network_egress_allowed{
            return Err("Observe & Learn provenance cannot be declassified");
        }
        Ok(())
    }
    pub fn joined(self, other:Self)->Self{
        if self.origin==DataOrigin::ObserveLearn||other.origin==DataOrigin::ObserveLearn{
            Self::OBSERVE_LEARN
        }else{Self{origin:DataOrigin::OrdinaryUser,network_egress_allowed:self.network_egress_allowed&&other.network_egress_allowed}}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn observation_is_tainted_and_denied_for_every_external_egress(){
        let tag=DataProvenance::OBSERVE_LEARN;
        for destination in [Destination::NetworkSocket,Destination::NetworkWorker,
            Destination::RemoteModel,Destination::BrowserNavigation]{
            assert!(tag.gate(destination).is_err());
        }
        for local in [Destination::LocalUi,Destination::LocalMemory,Destination::LocalInference]{
            assert!(tag.gate(local).is_ok());
        }
        assert!(DataProvenance{network_egress_allowed:true,..tag}
            .gate(Destination::LocalUi).is_err());
    }
    #[test]
    fn transformed_output_keeps_observation_taint_after_join(){
        let ordinary=DataProvenance{origin:DataOrigin::OrdinaryUser,network_egress_allowed:true};
        assert_eq!(ordinary.joined(DataProvenance::OBSERVE_LEARN),DataProvenance::OBSERVE_LEARN);
        assert_eq!(DataProvenance::OBSERVE_LEARN.joined(ordinary),DataProvenance::OBSERVE_LEARN);
        assert_eq!(ordinary.joined(ordinary),ordinary);
        // The network-deny bit is an independent policy boundary, even if
        // the origin is ordinary or an upstream adapter loses a taint label.
        let restricted=DataProvenance{origin:DataOrigin::OrdinaryUser,network_egress_allowed:false};
        for destination in [Destination::NetworkSocket,Destination::NetworkWorker,
            Destination::RemoteModel,Destination::BrowserNavigation]{
            assert!(restricted.gate(destination).is_err());
        }
        assert!(restricted.gate(Destination::LocalInference).is_ok());
        assert_eq!(ordinary.joined(restricted),restricted);
        assert_eq!(restricted.joined(ordinary),restricted);
    }
}
