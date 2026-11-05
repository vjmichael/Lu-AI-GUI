//! Host-scoped, read-only Reader model turn.
//!
//! This is the trusted admission bridge between a user-selected immutable
//! document and Lu's *existing* supervised JSON -> Protobuf tool loop.
//! A model cannot choose a file path outside its pinned snapshot, request a
//! write effect, create its own user-turn grant, or extend grant lifetime.
//! The desktop must verify the actually persisted user turn when calling new.
//! This module is not automatically enabled for ordinary chat generation.
use std::{
    collections::BTreeSet,
    time::{Duration,SystemTime},
};
use prost::Message;
use crate::{
    agent_tool_loop::{NativeToolTurn,ToolTurnError},
    contracts::{AuthorizationError,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        TrustedUserTurnVerifier,
    },
    inference_host::Cancellation,
    model::ModelToolCatalog,
    model_turn_journal::ModelTurnJournal,
    native_reader_tool::{register_reader_pinned,ReaderRequest,TOOL_ID,INPUT_TYPE},
    registry::NativeToolRegistry,
    supervised_tool_generation::{
        run_supervised_native_turn_journaled,ModelStepGenerator,
        NativeGenerationError,NativeTurnReport,NativeTurnServices,
    },
};

pub const READER_MODEL_TOOL_NAME:&str="lu_reader_read";
const READER_GRANT_SECONDS:u64=300;
#[derive(Debug,PartialEq,Eq)]
pub enum ReaderTurnAdmissionError{
    InvalidUserScope,
    InvalidFileGrant,
    AuthorityUnavailable,
    ToolRegistration,
    ModelCatalog,
}
struct ReaderOnlyHostPolicy{
    filename:String,
}
impl ToolAuthorizer for ReaderOnlyHostPolicy{
    fn authorize(&self,call:&ToolInvocation,descriptor:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        if call.tool_id!=TOOL_ID||descriptor.id!=TOOL_ID||
            descriptor.effect!=ToolEffect::ReadWorkspace||
            call.input.type_url!=INPUT_TYPE{
            return Err(AuthorizationError::new("Reader grant does not permit that effect"));
        }
        let request=ReaderRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|AuthorizationError::new("Malformed Reader request"))?;
        if request.path!=self.filename{
            return Err(AuthorizationError::new(
                "Reader can access only the operator-selected document"));
        }
        Ok(())
    }
}
/// One approved, bounded document; no ambient filesystem access is exposed
/// to the model. Source bytes are pinned in-memory for this single task.
/// Host validation MUST pass an actual persisted-user-turn verifier, not an
/// untrusted WebView predicate or model-supplied approval string.
pub struct ReaderTurnSession{
    registry:NativeToolRegistry,
    catalog:ModelToolCatalog,
    authority:ConversationAuthority,
    turn:NativeToolTurn,
    policy:ReaderOnlyHostPolicy,
    task_id:String,
}
impl ReaderTurnSession{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        verifier:&dyn TrustedUserTurnVerifier,
        conversation_id:&str,user_turn_id:&str,task_id:&str,
        filename:&str,authorized_bytes:Vec<u8>,
    )->Result<Self,ReaderTurnAdmissionError>{
        if conversation_id.is_empty()||user_turn_id.is_empty()||task_id.is_empty()||
           [conversation_id,user_turn_id,task_id].iter().any(|s|s.len()>128||
                s.chars().any(|c|c=='\0')) {
            return Err(ReaderTurnAdmissionError::InvalidUserScope);
        }
        if !verifier.verify_user_turn(conversation_id,user_turn_id){
            return Err(ReaderTurnAdmissionError::InvalidUserScope);
        }
        let registry=NativeToolRegistry::new();
        let catalog=register_reader_pinned(&registry,filename,authorized_bytes)
            .map_err(|_|ReaderTurnAdmissionError::InvalidFileGrant)?;
        let authority=ConversationAuthority::new();
        let origin=BoundCallOrigin{
            conversation_id:conversation_id.into(),
            user_turn_id:user_turn_id.into(),
            task_id:task_id.into(),
        };
        let expiry=SystemTime::now().checked_add(Duration::from_secs(READER_GRANT_SECONDS))
            .ok_or(ReaderTurnAdmissionError::AuthorityUnavailable)?;
        authority.open_user_task(TaskAuthority{
            conversation_id:conversation_id.into(),
            user_turn_id:user_turn_id.into(),
            task_id:task_id.into(),
            execution_scope_id:task_id.into(),
            capabilities:vec![AuthorizedCapability{
                tool_id:TOOL_ID.into(),effect:ToolEffect::ReadWorkspace
            }],
            expires_at:Some(expiry),
        },verifier).map_err(|_|ReaderTurnAdmissionError::AuthorityUnavailable)?;
        let allowed=BTreeSet::from([READER_MODEL_TOOL_NAME.to_owned()]);
        let turn=NativeToolTurn::new_read_only(&catalog,&registry,&authority,
            origin,task_id.into(),&allowed).map_err(|e|match e{
                ToolTurnError::Unauthorized(_)=>ReaderTurnAdmissionError::AuthorityUnavailable,
                _=>ReaderTurnAdmissionError::ModelCatalog,
            })?;
        Ok(Self{
            registry,catalog,authority,turn,
            policy:ReaderOnlyHostPolicy{filename:filename.to_owned()},
            task_id:task_id.into(),
        })
    }
    /// The caller owns the current model session and cancellation resource.
    /// Only the final, verified answer may be appended to chat history.
    /// This method does not silently send output to any conversation.
    pub async fn run_journaled(
        &mut self,user_text:&str,
        model:&mut dyn ModelStepGenerator,
        cancel:&Cancellation,
        journal:&ModelTurnJournal,
    )->Result<NativeTurnReport,NativeGenerationError>{
        let services=NativeTurnServices{
            catalog:&self.catalog,
            registry:&self.registry,
            authority:&self.authority,
            host_policy:&self.policy,
        };
        let result=run_supervised_native_turn_journaled(
            user_text,&mut self.turn,model,&services,cancel,journal,
        ).await;
        // A successful model final answer does not preserve the capability
        // for later messages. A failed/cancelled call also loses authority.
        let _=self.authority.revoke_task(&self.task_id);
        result
    }
}
impl Drop for ReaderTurnSession{
    fn drop(&mut self){
        let _=self.authority.revoke_task(&self.task_id);
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::{collections::VecDeque,path::PathBuf};
    struct Verify(bool);
    impl TrustedUserTurnVerifier for Verify{
        fn verify_user_turn(&self,conversation_id:&str,user_turn_id:&str)->bool{
            self.0&&conversation_id=="chat-a"&&user_turn_id=="human-1"
        }
    }
    struct Scripted{ replies:VecDeque<String> }
    impl ModelStepGenerator for Scripted{
        fn generate_step(&mut self,_prompt:&str,_max_tokens:u32,
            _cancel:&Cancellation)->Result<String,NativeGenerationError>{
            self.replies.pop_front().ok_or(NativeGenerationError::MissingCompletion)
        }
    }
    fn audit_path()->PathBuf{
        std::env::temp_dir().join(format!(
            "lu-reader-bound-turn-{}-{:?}.sqlite",
            std::process::id(),std::thread::current().id()))
    }
    #[test]
    fn cannot_invent_permission_or_arbitrary_document_path(){
        assert!(matches!(ReaderTurnSession::new(
            &Verify(false),"chat-a","human-1","task-1","memo.md",
            b"approved text".to_vec()),Err(ReaderTurnAdmissionError::InvalidUserScope)));
        assert!(matches!(ReaderTurnSession::new(
            &Verify(true),"chat-b","human-1","task-1","memo.md",
            b"approved text".to_vec()),Err(ReaderTurnAdmissionError::InvalidUserScope)));
        assert!(matches!(ReaderTurnSession::new(
            &Verify(true),"chat-a","human-1","task-1","../memo.md",
            b"approved text".to_vec()),Err(ReaderTurnAdmissionError::InvalidFileGrant)));
    }
    #[tokio::test]
    async fn real_json_model_tool_loop_reads_pinned_document_under_bound_authority(){
        let path=audit_path();let _=std::fs::remove_file(&path);
        let journal=ModelTurnJournal::open(&path).unwrap();
        let mut session=ReaderTurnSession::new(
            &Verify(true),"chat-a","human-1","task-1","memo.md",
            b"Only the approved snapshot".to_vec()).unwrap();
        let mut model=Scripted{replies:VecDeque::from(vec![
            r#"{"kind":"tool","name":"lu_reader_read","arguments":{"path":"memo.md","max_chars":500}}"#.into(),
            r#"{"kind":"final","text":"The approved document was read."}"#.into(),
        ])};
        let report=session.run_journaled(
            "Read the approved document",&mut model,&Cancellation::default(),&journal)
            .await.unwrap();
        assert_eq!(report.answer,"The approved document was read.");
        assert_eq!(report.tool_calls,1);
        assert_eq!(report.model_generations,2);
        let scope=BoundCallOrigin{
            conversation_id:"chat-a".into(),user_turn_id:"human-1".into(),
            task_id:"task-1".into()
        };
        assert!(session.authority.permitted_tool_ids(&scope).unwrap().is_empty());
        drop(session);drop(journal);
        let _=std::fs::remove_file(path);
    }
}
