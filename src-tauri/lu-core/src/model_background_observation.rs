//! Explicit current-conversation task observations. Never launches or cancels a worker.
use std::{collections::BTreeSet,sync::Arc,time::{Duration,SystemTime}};
use crate::{agent_tool_loop::{NativeToolTurn,ToolDispatchCheckpoint},contracts::{AuthorizationError,ProtoPayload,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,TrustedUserTurnVerifier},inference_host::Cancellation,model::ModelToolCatalog,model_turn_journal::ModelTurnJournal,native_background_result::{register_model_background_observation,RESULT_ID},native_background_status::TOOL_ID,registry::NativeToolRegistry,supervised_tool_generation::{run_supervised_native_turn_journaled_observed,ModelStepGenerator,NativeGenerationError,NativeTurnReport,NativeTurnServices},tasks::TaskStore};
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Request{original:String,id:String,result:bool}
impl Request{
 pub fn parse(original:&str)->Option<Self>{
  if original.len()>256||!original.is_ascii()||original.chars().any(char::is_control)||crate::capability_router::user_declines_tools(original){return None}
  let s=original.trim().trim_end_matches(['?','.','!']);let folded=s.to_ascii_lowercase();
  let (prefix,result)=[("show status of background task ",false),("check status of background task ",false),("show result of background task ",true),("get result of background task ",true)].into_iter().find(|(p,_)|folded.starts_with(p))?;
  let id=&s[prefix.len()..];if id.is_empty()||id.len()>128||!id.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_')){return None}Some(Self{original:original.into(),id:id.into(),result})
 }
 pub fn name(&self)->&'static str{if self.result{"lu_background_result"}else{"lu_background_status"}}
 fn tool(&self)->&'static str{if self.result{RESULT_ID}else{TOOL_ID}}
}
struct Fence<'a>{chat:&'a str,turn:&'a str,tool:&'a str,input:&'a ProtoPayload,verifier:&'a dyn TrustedUserTurnVerifier,live:&'a(dyn Fn()->Result<(),String>+Send+Sync)}
impl ToolAuthorizer for Fence<'_>{fn authorize(&self,call:&ToolInvocation,d:&ToolDescriptor)->Result<(),AuthorizationError>{
 if (self.live)().is_err()||!self.verifier.verify_user_turn(self.chat,self.turn)||call.workspace_id.as_deref()!=Some(self.chat)||d.effect!=ToolEffect::ReadWorkspace||d.id!=self.tool||call.tool_id!=self.tool||&call.input!=self.input{return Err(AuthorizationError::new("Exact task observation not authorized"))}Ok(())
}}
pub struct Observation{request:Request,registry:NativeToolRegistry,catalog:ModelToolCatalog,authority:ConversationAuthority,turn:NativeToolTurn,origin:BoundCallOrigin,input:ProtoPayload}
impl Observation{
 pub fn new(v:&dyn TrustedUserTurnVerifier,chat:&str,turn:&str,original:&str,store:Arc<TaskStore>)->Result<Self,String>{
  let request=Request::parse(original).ok_or("No explicit task observation")?;if !v.verify_user_turn(chat,turn){return Err("No authenticated original task request".into())}
  let registry=NativeToolRegistry::new();let mut catalog=register_model_background_observation(&registry,store)?;let allowed=BTreeSet::from([request.name().to_owned()]);let fixed=serde_json::json!({"task_id":request.id});let input=catalog.encode_call(&allowed,request.name(),"bound-observation",Some(chat),&fixed.to_string()).map_err(|_|"Invalid task observation")?.input;
  catalog.bind_current_input(&registry,request.name(),fixed.as_object().ok_or("Invalid task input")?.clone()).map_err(|_|"Task input unavailable")?;
  let authority=ConversationAuthority::new();let origin=BoundCallOrigin{conversation_id:chat.into(),user_turn_id:turn.into(),task_id:turn.into()};authority.open_user_task(TaskAuthority{conversation_id:chat.into(),user_turn_id:turn.into(),task_id:turn.into(),execution_scope_id:chat.into(),capabilities:vec![AuthorizedCapability{tool_id:request.tool().into(),effect:ToolEffect::ReadWorkspace}],expires_at:Some(SystemTime::now().checked_add(Duration::from_secs(300)).ok_or("Cannot bound inspection")?)},v).map_err(|_|"Task observation denied")?;
  let mut native=NativeToolTurn::new_read_only(&catalog,&registry,&authority,origin.clone(),chat.into(),&allowed).map_err(|_|"Task observation admission failed")?;native.require_single_observed_tool_result();Ok(Self{request,registry,catalog,authority,turn:native,origin,input})
 }
 pub async fn run(&mut self,model:&mut dyn ModelStepGenerator,v:&dyn TrustedUserTurnVerifier,cancel:&Cancellation,j:&ModelTurnJournal,live:&(dyn Fn()->Result<(),String>+Send+Sync),observe:&(dyn Fn(ToolDispatchCheckpoint)+Send+Sync))->Result<NativeTurnReport,NativeGenerationError>{
  if live().is_err()||cancel.is_cancelled()||!v.verify_user_turn(&self.origin.conversation_id,&self.origin.user_turn_id){return Err(NativeGenerationError::InvalidHostInput)}
  let fence=Fence{chat:&self.origin.conversation_id,turn:&self.origin.user_turn_id,tool:self.request.tool(),input:&self.input,verifier:v,live};let services=NativeTurnServices{catalog:&self.catalog,registry:&self.registry,authority:&self.authority,host_policy:&fence};
  let result=run_supervised_native_turn_journaled_observed(&self.request.original,&mut self.turn,model,&services,cancel,j,observe).await;let _=self.authority.revoke_task(&self.origin.task_id);
  if live().is_err()||cancel.is_cancelled()||!v.verify_user_turn(&self.origin.conversation_id,&self.origin.user_turn_id){return Err(NativeGenerationError::InvalidHostInput)}result
 }
}
impl Drop for Observation{fn drop(&mut self){let _=self.authority.revoke_task(&self.origin.task_id);}}
