//! TOOLS-10C: bind interactive browser operations to Lu's existing per-turn
//! conversation authority and typed native tool grants.
//!
//! The worker cannot mint origin scopes or capabilities. Navigation/read
//! requires an exact NetworkRead task grant; clicks, keystrokes and forms
//! require a distinct NetworkWrite grant because their consequences may be
//! irreversible. Human takeover, show/hide and the process lifecycle are
//! never model-callable browser operations.
use crate::{
    browser_worker_supervisor::{BrowserOperation,BrowserSupervisorError,OwnedBrowserWorker},
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    contracts::{AuthorizationError,ProtoPayload,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    wire::proto,
};
use serde_json::Value;
use prost::Message;
use std::collections::BTreeSet;

pub const BROWSER_READ_TOOL:&str="lu.web.browser.read";
pub const BROWSER_INTERACT_TOOL:&str="lu.web.browser.interact";
const BROWSER_INPUT:&str="type.googleapis.com/lu.v1.BrowserOperationRequest";
const BROWSER_OUTPUT:&str="type.googleapis.com/lu.v1.BrowserOperationResult";

#[derive(Debug)]
pub enum BrowserAuthorizationError{
    MissingGrant,
    UnapprovedOrigin,
    UnsupportedModelOperation,
    InvalidArguments,
    Worker(BrowserSupervisorError),
}
pub fn browser_descriptor(effect:ToolEffect)->Result<ToolDescriptor,BrowserAuthorizationError>{
    let (id,title)=match effect {
        ToolEffect::NetworkRead=>(BROWSER_READ_TOOL,"Read authorized browser sources"),
        ToolEffect::NetworkWrite=>(BROWSER_INTERACT_TOOL,"Interact with authorized browser page"),
        _=>return Err(BrowserAuthorizationError::MissingGrant),
    };
    Ok(ToolDescriptor{
        id:id.into(),display_name:title.into(),effect,
        input_type_url:BROWSER_INPUT.into(),output_type_url:BROWSER_OUTPUT.into(),
    })
}
fn required_effect(op:BrowserOperation)->Result<ToolEffect,BrowserAuthorizationError>{
    match op{
        BrowserOperation::Navigate|BrowserOperation::Read|BrowserOperation::Accessibility
          |BrowserOperation::Screenshot|BrowserOperation::Status
          =>Ok(ToolEffect::NetworkRead),
        BrowserOperation::Click|BrowserOperation::ClickSelector|BrowserOperation::Type
          |BrowserOperation::Press|BrowserOperation::Scroll|BrowserOperation::Back
          |BrowserOperation::Forward
          =>Ok(ToolEffect::NetworkWrite),
        BrowserOperation::HumanResolved|BrowserOperation::Show|BrowserOperation::Hide
          =>Err(BrowserAuthorizationError::UnsupportedModelOperation),
    }
}
fn valid_origin(raw:&str)->bool{
    let Some(host)=raw.strip_prefix("https://") else{return false;};
    !host.is_empty()&&host.len()<=245&&!host.contains(':')&&!host.contains('/')
      &&!host.contains('@')&&!host.contains('?')&&!host.contains('#')
      &&!host.contains('\\')&&host.bytes().all(|b|
          b.is_ascii_alphanumeric()||b==b'.'||b==b'-')
      &&!host.starts_with('.')&&!host.ends_with('.')
      &&host.contains('.')&&!host.ends_with(".local")
      &&!host.ends_with(".internal")&&!host.ends_with(".localhost")
      &&!host.bytes().any(|b|b.is_ascii_uppercase())
      &&host.split('.').all(|label|!label.is_empty()&&label.len()<=63
          &&label.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
          &&label.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric))
      &&host.rsplit('.').next().is_some_and(|tld|
          tld.len()>=2&&tld.bytes().all(|b|b.is_ascii_lowercase()))
}
/// A model input cannot add origins. Navigation is confined to a trusted
/// immutable origin list; browser redirects are separately checked by Node.
pub struct BrowserOriginScope {
    approved:BTreeSet<String>,
}
impl BrowserOriginScope {
    pub fn from_host_grant(origins:Vec<String>)->Result<Self,BrowserAuthorizationError>{
        if origins.is_empty()||origins.len()>32
           ||origins.iter().any(|x|!valid_origin(x)){
            return Err(BrowserAuthorizationError::UnapprovedOrigin);
        }
        let approved:BTreeSet<_>=origins.into_iter().collect();
        if approved.len()>32{return Err(BrowserAuthorizationError::UnapprovedOrigin);}
        Ok(Self{approved})
    }
    pub fn allows(&self,url:&str)->bool {
        if url.len()>2048||url.chars().any(char::is_control)||url.contains('\\'){
            return false;
        }
        let Some(remainder)=url.strip_prefix("https://") else{return false;};
        let host=remainder.split(&['/','?','#'][..]).next().unwrap_or("");
        // Restrict to ASCII hostnames under an exact host grant. This is
        // intentionally narrower than a full URL parser: no credentials,
        // ports, IDNA aliases, percent-escaped authority, or IP addresses.
        if !valid_origin(&format!("https://{host}")){return false;}
        self.approved.contains(&format!("https://{host}"))
    }
    pub fn origins(&self)->Vec<String>{self.approved.iter().cloned().collect()}
}
/// Convert the exact, externally validated operation into one canonical
/// typed request. This is encoding, NOT a permission grant; its output still
/// requires immutable conversation authority and host policy at dispatch.
pub fn encode_browser_request(
    operation:BrowserOperation,args:&Value
)->Result<ProtoPayload,BrowserAuthorizationError>{
    required_effect(operation)?;
    let object=args.as_object().ok_or(BrowserAuthorizationError::InvalidArguments)?;
    if object.contains_key("op")||object.contains_key("type"){
        return Err(BrowserAuthorizationError::InvalidArguments);
    }
    let json=serde_json::to_vec(args)
        .map_err(|_|BrowserAuthorizationError::InvalidArguments)?;
    if json.len()>8192{return Err(BrowserAuthorizationError::InvalidArguments);}
    Ok(ProtoPayload{
        type_url:BROWSER_INPUT.into(),
        bytes:proto::BrowserOperationRequest{
            operation:operation.operation_name().into(),
            arguments_json:json,
        }.encode_to_vec()
    })
}
fn operation_from_name(name:&str)->Option<BrowserOperation>{
    use BrowserOperation as O;
    Some(match name{
        "status"=>O::Status,"navigate"=>O::Navigate,
        "read"=>O::Read,"accessibility"=>O::Accessibility,
        "screenshot"=>O::Screenshot,"click"=>O::Click,
        "click_selector"=>O::ClickSelector,"type"=>O::Type,
        "press"=>O::Press,"scroll"=>O::Scroll,
        "back"=>O::Back,"forward"=>O::Forward,
        _=>return None,
    })
}
/// Fail closed unless the Protobuf wire format is canonical, the JSON is a
/// bounded canonical object, and the operation is a model-allowed capability.
pub fn decode_browser_request(
    invocation:&ToolInvocation
)->Result<(BrowserOperation,Value),BrowserAuthorizationError>{
    if invocation.input.type_url!=BROWSER_INPUT||invocation.input.bytes.len()>9000{
        return Err(BrowserAuthorizationError::InvalidArguments);
    }
    let raw=proto::BrowserOperationRequest::decode(invocation.input.bytes.as_slice())
        .map_err(|_|BrowserAuthorizationError::InvalidArguments)?;
    if raw.encode_to_vec()!=invocation.input.bytes{
        return Err(BrowserAuthorizationError::InvalidArguments);
    }
    let operation=operation_from_name(&raw.operation)
        .ok_or(BrowserAuthorizationError::UnsupportedModelOperation)?;
    required_effect(operation)?;
    if raw.arguments_json.len()>8192{
        return Err(BrowserAuthorizationError::InvalidArguments);
    }
    let args:Value=serde_json::from_slice(&raw.arguments_json)
        .map_err(|_|BrowserAuthorizationError::InvalidArguments)?;
    let obj=args.as_object().ok_or(BrowserAuthorizationError::InvalidArguments)?;
    if obj.contains_key("op")||obj.contains_key("type")
      ||serde_json::to_vec(&args).ok().as_deref()!=Some(raw.arguments_json.as_slice()){
        return Err(BrowserAuthorizationError::InvalidArguments);
    }
    Ok((operation,args))
}
/// Only the host scheduler supplies the immutable origin & invocation.
/// The browser command is not executable without a verified task capability
/// and the ordinary Lu host authorizer. Never infer approval from a webpage.
pub fn authorize_browser_operation(
    authority:&ConversationAuthority,
    origin:&BoundCallOrigin,
    invocation:&ToolInvocation,
    host_policy:&dyn ToolAuthorizer,
    scope:&BrowserOriginScope,
    operation:BrowserOperation,
    args:&Value,
)->Result<(),BrowserAuthorizationError>{
    let (decoded_operation,decoded_args)=decode_browser_request(invocation)?;
    if operation!=decoded_operation||*args!=decoded_args{
        return Err(BrowserAuthorizationError::InvalidArguments);
    }
    let effect=required_effect(operation)?;
    let descriptor=browser_descriptor(effect)?;
    if invocation.tool_id!=descriptor.id {
        return Err(BrowserAuthorizationError::InvalidArguments);
    }
    if matches!(operation,BrowserOperation::Navigate){
        let url=args.get("url").and_then(Value::as_str)
            .ok_or(BrowserAuthorizationError::InvalidArguments)?;
        if !scope.allows(url){return Err(BrowserAuthorizationError::UnapprovedOrigin);}
    }
    authority.authorize_worker_call(origin,invocation,&descriptor,host_policy)
      .map_err(|_:AuthorizationError|BrowserAuthorizationError::MissingGrant)
}
/// Host-only dispatch after typed authorization; effects cannot be
/// downgraded to NetworkRead by pretending a click is a read operation.
pub fn execute_authorized_browser_operation(
    authority:&ConversationAuthority,
    origin:&BoundCallOrigin,
    invocation:&ToolInvocation,
    host_policy:&dyn ToolAuthorizer,
    scope:&BrowserOriginScope,
    worker:&mut OwnedBrowserWorker,
    operation:BrowserOperation,
    args:Value,
)->Result<Value,BrowserAuthorizationError>{
    authorize_browser_operation(authority,origin,invocation,host_policy,scope,
        operation,&args)?;
    worker.execute(operation,args).map_err(BrowserAuthorizationError::Worker)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_scope_is_exact_origin_and_https(){
        let s=BrowserOriginScope::from_host_grant(vec!["https://papers.example".into()]).unwrap();
        assert!(s.allows("https://papers.example/article?q=a"));
        for unsafe_grant in ["https://127.0.0.1","https://localhost",
            "https://printer.local","https://server.internal","https://PAPERS.example"] {
            assert!(BrowserOriginScope::from_host_grant(
                vec![unsafe_grant.into()]).is_err(),"{unsafe_grant}");
        }

        for url in ["http://papers.example/a","https://papers.example.evil.test/",
            "https://papers.example:8443/","https://user@papers.example",
            "https://127.0.0.1/private","https://papers.example@evil.test/"] {
            assert!(!s.allows(url),"{url}");
        }
    }
    #[test]
    fn multi_origin_grant_is_exact_and_cannot_implicitly_expand(){
        let approved=BrowserOriginScope::from_host_grant(vec![
            "https://papers.example".into(),"https://catalog.example".into()
        ]).unwrap();
        assert!(approved.allows("https://papers.example/article"));
        assert!(approved.allows("https://catalog.example/record"));
        for url in ["https://other.example/","https://papers.example.evil.test/",
            "http://catalog.example/","https://user@catalog.example/",
            "https://catalog.example:8443/"]{
            assert!(!approved.allows(url),"{url}");
        }
        for origin in ["https://a..example","https://-bad.example",
            "https://bad-.example"]{
            assert!(BrowserOriginScope::from_host_grant(vec![origin.into()]).is_err());
        }
        let mut oversized=vec!["https://host.example".to_string();33];
        assert!(BrowserOriginScope::from_host_grant(std::mem::take(&mut oversized)).is_err());
    }
    #[test]
    fn interaction_is_not_downgraded_to_read_authority(){
        assert_eq!(required_effect(BrowserOperation::Click).unwrap(),ToolEffect::NetworkWrite);
        assert_eq!(required_effect(BrowserOperation::Type).unwrap(),ToolEffect::NetworkWrite);
        assert_eq!(required_effect(BrowserOperation::Read).unwrap(),ToolEffect::NetworkRead);
        assert!(required_effect(BrowserOperation::HumanResolved).is_err());
        assert!(required_effect(BrowserOperation::Show).is_err());
        assert!(required_effect(BrowserOperation::Hide).is_err());
    }
    #[test]
    fn typed_browser_operation_must_match_the_exact_authorized_payload(){
        let request=encode_browser_request(BrowserOperation::Navigate,
            &serde_json::json!({"url":"https://papers.example/first"})).unwrap();
        let invocation=ToolInvocation{
            call_id:"call1".into(),tool_id:BROWSER_READ_TOOL.into(),
            workspace_id:None,input:request,
        };
        assert_eq!(decode_browser_request(&invocation).unwrap().0,BrowserOperation::Navigate);
        let auth=ConversationAuthority::new();
        let origin=BoundCallOrigin{
            conversation_id:"chat".into(),user_turn_id:"user".into(),task_id:"task".into()
        };
        struct Allow;
        impl ToolAuthorizer for Allow {
            fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{
                Ok(())
            }
        }
        let scope=BrowserOriginScope::from_host_grant(vec!["https://papers.example".into()]).unwrap();
        for (op,args) in [
            (BrowserOperation::Read,serde_json::json!({"url":"https://papers.example/first"})),
            (BrowserOperation::Navigate,serde_json::json!({"url":"https://papers.example/second"}))
        ] {
            let result=authorize_browser_operation(&auth,&origin,&invocation,&Allow,
                &scope,op,&args);
            assert!(matches!(result,Err(BrowserAuthorizationError::InvalidArguments)));
        }
        // Even with an exact payload and permissive host policy, an absent
        // persisted USER task cannot authorize a native browser action.
        assert!(matches!(authorize_browser_operation(&auth,&origin,&invocation,&Allow,
            &scope,BrowserOperation::Navigate,
            &serde_json::json!({"url":"https://papers.example/first"})),
            Err(BrowserAuthorizationError::MissingGrant)));
        assert!(encode_browser_request(BrowserOperation::Show,
            &serde_json::json!({})).is_err());
        assert!(encode_browser_request(BrowserOperation::HumanResolved,
            &serde_json::json!({})).is_err());
    }
    #[test]
    fn verified_read_grant_never_authorizes_browser_interaction_or_other_task(){
        use crate::conversation_authority::{
            TaskAuthority,AuthorizedCapability,TrustedUserTurnVerifier,
        };
        use std::time::{SystemTime,Duration};
        struct Verifier;
        impl TrustedUserTurnVerifier for Verifier {
            fn verify_user_turn(&self,c:&str,u:&str)->bool{
                c=="verified-chat"&&u=="trusted-user-turn"
            }
        }
        struct HostAllowed;
        impl ToolAuthorizer for HostAllowed {
            fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{
                Ok(())
            }
        }
        let authority=ConversationAuthority::new();
        authority.open_user_task(TaskAuthority{
            conversation_id:"verified-chat".into(),
            user_turn_id:"trusted-user-turn".into(),
            task_id:"approved-browser".into(),
            execution_scope_id:"scope-read".into(),
            capabilities:vec![AuthorizedCapability{
                tool_id:BROWSER_READ_TOOL.into(),effect:ToolEffect::NetworkRead,
            }],
            expires_at:Some(SystemTime::now()+Duration::from_secs(120)),
        },&Verifier).unwrap();
        let origin=BoundCallOrigin{
            conversation_id:"verified-chat".into(),
            user_turn_id:"trusted-user-turn".into(),
            task_id:"approved-browser".into(),
        };
        let scope=BrowserOriginScope::from_host_grant(
            vec!["https://papers.example".into()]).unwrap();
        let read_payload=encode_browser_request(BrowserOperation::Navigate,
            &serde_json::json!({"url":"https://papers.example/a"})).unwrap();
        let read=ToolInvocation{
            call_id:"read1".into(),tool_id:BROWSER_READ_TOOL.into(),
            workspace_id:Some("scope-read".into()),input:read_payload,
        };
        assert!(authorize_browser_operation(
            &authority,&origin,&read,&HostAllowed,&scope,
            BrowserOperation::Navigate,&serde_json::json!({"url":"https://papers.example/a"})
        ).is_ok());
        let click=ToolInvocation{
            call_id:"click1".into(),tool_id:BROWSER_INTERACT_TOOL.into(),
            workspace_id:Some("scope-read".into()),
            input:encode_browser_request(BrowserOperation::Click,
                &serde_json::json!({"x":1,"y":2})).unwrap(),
        };
        assert!(matches!(authorize_browser_operation(
            &authority,&origin,&click,&HostAllowed,&scope,BrowserOperation::Click,
            &serde_json::json!({"x":1,"y":2})),
            Err(BrowserAuthorizationError::MissingGrant)));
        let wrong_scope=ToolInvocation{
            workspace_id:Some("other-scope".into()),..read.clone()
        };
        assert!(authorize_browser_operation(
            &authority,&origin,&wrong_scope,&HostAllowed,&scope,
            BrowserOperation::Navigate,&serde_json::json!({"url":"https://papers.example/a"})
        ).is_err());
        authority.revoke_task("approved-browser").unwrap();
        assert!(authorize_browser_operation(
            &authority,&origin,&read,&HostAllowed,&scope,
            BrowserOperation::Navigate,&serde_json::json!({"url":"https://papers.example/a"})
        ).is_err());
    }
    #[test]
    fn noncanonical_browser_wire_and_json_do_not_reach_worker(){
        let payload=encode_browser_request(BrowserOperation::Read,&serde_json::json!({})).unwrap();
        let mut invocation=ToolInvocation{
            call_id:"call".into(),tool_id:BROWSER_READ_TOOL.into(),
            workspace_id:None,input:payload,
        };
        invocation.input.bytes.push(0);
        assert!(decode_browser_request(&invocation).is_err());
        invocation.input=ProtoPayload{
            type_url:BROWSER_INPUT.into(),
            bytes:proto::BrowserOperationRequest{
                operation:"read".into(),arguments_json:b"{ \"x\": 1 }".to_vec(),
            }.encode_to_vec(),
        };
        assert!(decode_browser_request(&invocation).is_err());
    }
    #[test]
    fn untrusted_model_input_cannot_substitute_permission(){
        struct Denied;
        impl ToolAuthorizer for Denied {
            fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{
                Err(AuthorizationError::new("denied"))
            }
        }
        let auth=ConversationAuthority::new();
        let origin=BoundCallOrigin{
            conversation_id:"chat".into(),user_turn_id:"user".into(),task_id:"task".into()
        };
        let invocation=ToolInvocation{
            call_id:"test".into(),tool_id:BROWSER_READ_TOOL.into(),
            workspace_id:None,
            input:encode_browser_request(BrowserOperation::Navigate,
                &serde_json::json!({"url":"https://papers.example/a"})).unwrap()
        };
        let scope=BrowserOriginScope::from_host_grant(vec!["https://papers.example".into()]).unwrap();
        let denied=authorize_browser_operation(&auth,&origin,&invocation,&Denied,
            &scope,BrowserOperation::Navigate,
            &serde_json::json!({"url":"https://papers.example/a"}));
        assert!(matches!(denied,Err(BrowserAuthorizationError::MissingGrant)));
    }
}
