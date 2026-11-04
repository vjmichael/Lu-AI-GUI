//! Native-host half of the headed Chromium visibility protocol.
//! Browser requests are not authority: the host must first authorize an
//! expected visibility transition from its own supervisor state. A browser
//! process cannot select another HWND/PID or grant itself rights.
use serde::{Deserialize,Serialize};
use crate::browser_process_owner::{OwnedBrowserWindow,WindowOperationError};

const MAX_EVENT_BYTES:usize=1024;

#[derive(Debug,Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(rename="type")]
    kind:String,
    #[serde(rename="requestId")]
    request_id:String,
    visible:bool,
}
#[derive(Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct Response {
    #[serde(rename="type")]
    pub kind:String,
    #[serde(rename="requestId")]
    pub request_id:String,
    pub ok:bool,
}
#[derive(Debug,PartialEq,Eq)]
pub enum VisibilityProtocolError {
    InvalidRequest,
    Unauthorized,
    NativeFailure,
}
fn parse_request(line:&str)->Result<Request,VisibilityProtocolError>{
    if line.len()>MAX_EVENT_BYTES {return Err(VisibilityProtocolError::InvalidRequest);}
    let input:Request=serde_json::from_str(line)
        .map_err(|_|VisibilityProtocolError::InvalidRequest)?;
    if input.kind!="host_visibility_request"
        || !input.request_id.starts_with("visibility-")
        || input.request_id.len()>48
        || !input.request_id[11..].bytes().all(|b|b.is_ascii_digit())
        || input.request_id.len()==11 {
        return Err(VisibilityProtocolError::InvalidRequest);
    }
    Ok(input)
}
/// Enforces the next exact transition. Callers must arrange permission from
/// the trusted per-turn/browser state machine before processing a worker event.
pub struct VisibilityBridge {
    expected:Option<bool>,
    last_request_id:Option<String>,
}
impl VisibilityBridge {
    pub fn new()->Self{Self{expected:None,last_request_id:None}}
    pub fn is_transition_authorized(&self)->bool {self.expected.is_some()}
    pub fn authorize_next_transition(&mut self,visible:bool) {
        self.expected=Some(visible);
    }
    pub fn process_with<F>(&mut self,line:&str,mut native:F)
      ->Result<String,VisibilityProtocolError>
    where F:FnMut(bool)->Result<(),WindowOperationError> {
        let req=parse_request(line)?;
        if self.expected!=Some(req.visible)
            || self.last_request_id.as_ref()==Some(&req.request_id) {
            return Err(VisibilityProtocolError::Unauthorized);
        }
        // A failure must never be reported as native success or unlock an
        // unapproved transition; the host can explicitly retry/reconcile.
        native(req.visible).map_err(|_|VisibilityProtocolError::NativeFailure)?;
        self.expected=None;
        self.last_request_id=Some(req.request_id.clone());
        serde_json::to_string(&Response{
            kind:"host_visibility_response".into(),request_id:req.request_id,ok:true
        }).map_err(|_|VisibilityProtocolError::InvalidRequest)
    }
    pub fn process_owned_window(&mut self,line:&str,owner:&mut OwnedBrowserWindow)
      ->Result<String,VisibilityProtocolError> {
        self.process_with(line,|visible|owner.set_visible(visible))
    }
}
impl Default for VisibilityBridge {
    fn default()->Self{Self::new()}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(id:&str,visible:bool)->String{
        serde_json::json!({"type":"host_visibility_request",
            "requestId":id,"visible":visible}).to_string()
    }
    #[test]
    fn no_worker_initiated_window_operation_without_host_authorization() {
        let mut bridge=VisibilityBridge::new();
        assert_eq!(bridge.process_with(&request("visibility-1",false),|_|Ok(())),
            Err(VisibilityProtocolError::Unauthorized));
        bridge.authorize_next_transition(false);
        assert_eq!(bridge.process_with(&request("visibility-1",true),|_|Ok(())),
            Err(VisibilityProtocolError::Unauthorized));
        let response=bridge.process_with(&request("visibility-1",false),|_|Ok(())).unwrap();
        assert_eq!(serde_json::from_str::<Response>(&response).unwrap(),
            Response{kind:"host_visibility_response".into(),
                request_id:"visibility-1".into(),ok:true});
        bridge.authorize_next_transition(false);
        assert_eq!(bridge.process_with(&request("visibility-1",false),|_|Ok(())),
            Err(VisibilityProtocolError::Unauthorized));
    }
    #[test]
    fn failed_native_operation_does_not_acknowledge_or_consume_permission(){
        let mut bridge=VisibilityBridge::new();
        bridge.authorize_next_transition(true);
        assert_eq!(bridge.process_with(&request("visibility-2",true),|_|{
            Err(WindowOperationError::NativeFailure)
        }),Err(VisibilityProtocolError::NativeFailure));
        assert!(bridge.process_with(&request("visibility-2",true),|_|Ok(())).is_ok());
    }
    #[test]
    fn rejects_fake_protocol_messages_and_unbounded_lines() {
        let mut bridge=VisibilityBridge::new();
        bridge.authorize_next_transition(false);
        for invalid in [
            request("visibility-not-a-number",false),
            serde_json::json!({"type":"host_visibility_request",
                "requestId":"visibility-4","visible":false,"pid":123}).to_string(),
            serde_json::json!({"type":"host_visibility_response",
                "requestId":"visibility-4","ok":true}).to_string(),
            "a".repeat(1100),
        ] {
            assert_eq!(bridge.process_with(&invalid,|_|Ok(())),
                Err(VisibilityProtocolError::InvalidRequest));
        }
    }
}
