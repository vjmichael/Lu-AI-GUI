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
};
use serde_json::Value;
use std::collections::BTreeSet;

pub const BROWSER_READ_TOOL:&str="lu.web.browser.read";
pub const BROWSER_INTERACT_TOOL:&str="lu.web.browser.interact";
const BROWSER_INPUT:&str="type.googleapis.com/lu.v1.BrowserOperationRequest";
const BROWSER_OUTPUT:&str="type.googleapis.com/lu.v1.BrowserOperationResult";

#[derive(Debug)]
pub enum BrowserAuthorizationError{
    MissingGrant,
    WrongTask,
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
    let effect=required_effect(operation)?;
    let descriptor=browser_descriptor(effect)?;
    if invocation.tool_id!=descriptor.id || !args.is_object()
       ||invocation.input.bytes.len()>8192 {
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
    fn interaction_is_not_downgraded_to_read_authority(){
        assert_eq!(required_effect(BrowserOperation::Click).unwrap(),ToolEffect::NetworkWrite);
        assert_eq!(required_effect(BrowserOperation::Type).unwrap(),ToolEffect::NetworkWrite);
        assert_eq!(required_effect(BrowserOperation::Read).unwrap(),ToolEffect::NetworkRead);
        assert!(required_effect(BrowserOperation::HumanResolved).is_err());
        assert!(required_effect(BrowserOperation::Show).is_err());
        assert!(required_effect(BrowserOperation::Hide).is_err());
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
            input:ProtoPayload{type_url:BROWSER_INPUT.into(),bytes:vec![]}
        };
        let scope=BrowserOriginScope::from_host_grant(vec!["https://papers.example".into()]).unwrap();
        let denied=authorize_browser_operation(&auth,&origin,&invocation,&Denied,
            &scope,BrowserOperation::Navigate,
            &serde_json::json!({"url":"https://papers.example/a"}));
        assert!(matches!(denied,Err(BrowserAuthorizationError::MissingGrant)));
    }
}
