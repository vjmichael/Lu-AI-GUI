//! CORE-MERGE-C2: existing supervised model-to-Protobuf loop for two narrowly
//! human-requested, local read-only utilities. Registration is not a grant.
//! No second execution engine, external worker, PATH lookup, or file reader.
use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime},
};
use prost::Message;
use serde_json::Value;
use crate::{
    agent_tool_loop::{NativeToolTurn, ToolDispatchCheckpoint},
    contracts::{AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    inference_host::Cancellation,
    model::ModelToolCatalog,
    model_turn_journal::ModelTurnJournal,
    native_capability_inventory::{discover_turn_capabilities, NativeCapabilityView},
    native_environment_discovery::{register_environment_discovery_tool, HostProgramSpec},
    native_schema_validation::register_schema_validation_tool,
    registry::NativeToolRegistry,
    supervised_tool_generation::{
        run_supervised_native_turn_journaled_observed, ModelStepGenerator,
        NativeGenerationError, NativeTurnReport, NativeTurnServices,
    },
    wire::proto,
};

const GRANT_SECONDS:u64=300;
const SCHEMA_ID:&str="lu.validate.schema";
const ENV_ID:&str="lu.exec.check_env";

/// Host-selected from the CURRENT exact human prompt. No model, remembered
/// source, WebView tool name, directory, or executable supplies this scope.
#[derive(Debug,Clone)]
pub enum CoreUtilityRequest{
    Validate{schema:Value,data:Value},
    CheckProgram{program:String,spec:HostProgramSpec},
}
enum ExactInput{
    Schema{schema:Vec<u8>,data:Vec<u8>},
    Program(String),
}
struct OnlyOriginalInput{
    tool_id:String,
    scope:String,
    exact:ExactInput,
}
impl ToolAuthorizer for OnlyOriginalInput{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        if desc.effect!=ToolEffect::ReadWorkspace ||
            desc.id!=self.tool_id || call.tool_id!=self.tool_id ||
            call.workspace_id.as_deref()!=Some(self.scope.as_str()){
            // No workspace path is authorized. A host-selected opaque scope
            // is accepted below; model-selected paths are never considered.
            return Err(AuthorizationError::new("utility effect or identity not approved"));
        }
        if call.input.type_url!=desc.input_type_url{
            return Err(AuthorizationError::new("utility protobuf type mismatch"));
        }
        match &self.exact{
            ExactInput::Schema{schema,data}=>{
                let decoded=proto::SchemaValidateRequest::decode(call.input.bytes.as_slice())
                    .map_err(|_|AuthorizationError::new("malformed schema request"))?;
                if decoded.encode_to_vec()!=call.input.bytes ||
                    decoded.schema_json!=*schema || decoded.instance_json!=*data{
                    return Err(AuthorizationError::new(
                        "model arguments differ from original human schema/data"
                    ));
                }
            }
            ExactInput::Program(program)=>{
                let decoded=proto::EnvironmentCheckRequest::decode(call.input.bytes.as_slice())
                    .map_err(|_|AuthorizationError::new("malformed environment request"))?;
                if decoded.encode_to_vec()!=call.input.bytes || decoded.program_id!=*program{
                    return Err(AuthorizationError::new(
                        "model program differs from human-approved candidate"
                    ));
                }
            }
        }
        Ok(())
    }
}
pub struct CoreUtilityTurn{
    registry:NativeToolRegistry,
    catalog:ModelToolCatalog,
    authority:ConversationAuthority,
    turn:NativeToolTurn,
    policy:OnlyOriginalInput,
    origin:BoundCallOrigin,
}
impl CoreUtilityTurn{
    pub fn new(
        verifier:&dyn TrustedUserTurnVerifier,
        conversation_id:&str,user_turn_id:&str,task_id:&str,
        request:CoreUtilityRequest,
    )->Result<Self,String>{
        if [conversation_id,user_turn_id,task_id].iter().any(|s|
            s.is_empty() || s.len()>128 || !s.bytes()
                .all(|b|b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'))
        ) || !verifier.verify_user_turn(conversation_id,user_turn_id){
            return Err("Original authenticated utility turn missing".into());
        }
        let registry=NativeToolRegistry::new();
        let mut catalog=ModelToolCatalog::new();
        let (tool_id,name,exact)=match request{
            CoreUtilityRequest::Validate{schema,data}=>{
                let schema_bytes=serde_json::to_vec(&schema)
                    .map_err(|_|"Cannot encode original schema")?;
                let data_bytes=serde_json::to_vec(&data)
                    .map_err(|_|"Cannot encode original data")?;
                if schema_bytes.len()>16*1024 || data_bytes.len()>32*1024{
                    return Err("Original schema/data exceeds bounded tool input".into());
                }
                register_schema_validation_tool(&registry,&mut catalog)?;
                (SCHEMA_ID.to_owned(),"lu_validate_schema".to_owned(),
                    ExactInput::Schema{schema:schema_bytes,data:data_bytes})
            }
            CoreUtilityRequest::CheckProgram{program,spec}=>{
                if spec.id!=program{
                    return Err("Host candidate does not match selected original program".into());
                }
                register_environment_discovery_tool(&registry,&mut catalog,vec![spec])?;
                (ENV_ID.to_owned(),"lu_check_env".to_owned(),ExactInput::Program(program))
            }
        };
        let descriptor=registry.tools()
            .map_err(|_|"Core utility registry unavailable")?;
        if descriptor.len()!=1 || descriptor[0].id!=tool_id ||
            descriptor[0].effect!=ToolEffect::ReadWorkspace {
            return Err("Unexpected native tool in single-utility registry".into());
        }
        let allowed=BTreeSet::from([name]);
        let authority=ConversationAuthority::new();
        let origin=BoundCallOrigin{
            conversation_id:conversation_id.into(),
            user_turn_id:user_turn_id.into(),task_id:task_id.into(),
        };
        let expires_at=SystemTime::now().checked_add(Duration::from_secs(GRANT_SECONDS))
            .ok_or("Cannot bound utility grant")?;
        authority.open_user_task(TaskAuthority{
            conversation_id:conversation_id.into(),
            user_turn_id:user_turn_id.into(),
            task_id:task_id.into(),
            execution_scope_id:task_id.into(),
            capabilities:vec![AuthorizedCapability{
                tool_id:tool_id.clone(),effect:ToolEffect::ReadWorkspace,
            }],
            expires_at:Some(expires_at),
        },verifier).map_err(|_|"Original utility grant denied")?;
        let mut turn=NativeToolTurn::new_read_only(
            &catalog,&registry,&authority,origin.clone(),task_id.into(),&allowed
        ).map_err(|_|"Original utility tool admission failed")?;
        turn.require_observed_tool_result();
        Ok(Self{registry,catalog,authority,turn,
            policy:OnlyOriginalInput{tool_id,scope:task_id.into(),exact},origin})
    }
    pub fn capabilities(&self)->Result<Vec<NativeCapabilityView>,String>{
        discover_turn_capabilities(
            &self.registry,&self.catalog,&self.authority,&self.origin,true
        ).map_err(|_|"Utility capability projection refused".into())
    }
    pub async fn run_journaled_observed(
        &mut self,prompt:&str,model:&mut dyn ModelStepGenerator,
        cancel:&Cancellation,journal:&ModelTurnJournal,
        observe:&(dyn Fn(ToolDispatchCheckpoint)+Send+Sync),
    )->Result<NativeTurnReport,NativeGenerationError>{
        let services=NativeTurnServices{
            catalog:&self.catalog,registry:&self.registry,
            authority:&self.authority,host_policy:&self.policy,
        };
        let result=run_supervised_native_turn_journaled_observed(
            prompt,&mut self.turn,model,&services,cancel,journal,observe
        ).await;
        let _=self.authority.revoke_task(&self.origin.task_id);
        result
    }
}
impl Drop for CoreUtilityTurn{
    fn drop(&mut self){let _=self.authority.revoke_task(&self.origin.task_id);}
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::{collections::VecDeque,fs,path::PathBuf};
    use serde_json::json;
    struct Original;
    impl TrustedUserTurnVerifier for Original{
        fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
            chat=="c-original"&&turn=="g-original"
        }
    }
    struct Replies(VecDeque<String>);
    impl ModelStepGenerator for Replies{
        fn generate_step(&mut self,_prompt:&str,_max:u32,
            _cancel:&Cancellation)->Result<String,NativeGenerationError>{
            self.0.pop_front().ok_or(NativeGenerationError::MissingCompletion)
        }
    }
    fn audit()->(PathBuf,ModelTurnJournal){
        let path=std::env::temp_dir().join(format!(
            "lu-c2-utility-{}-{:?}.sqlite",std::process::id(),
            std::thread::current().id()
        ));
        let _=fs::remove_file(&path);
        let journal=ModelTurnJournal::open(&path).unwrap();
        (path,journal)
    }
    #[tokio::test]
    async fn original_schema_is_the_only_data_that_model_may_validate(){
        let mut turn=CoreUtilityTurn::new(&Original,"c-original","g-original",
            "g-original",CoreUtilityRequest::Validate{
                schema:json!({"type":"integer"}),data:json!(42),
            }).unwrap();
        let names=turn.capabilities().unwrap();
        assert_eq!(names.len(),1);
        assert_eq!(names[0].public_name,"lu_validate_schema");
        let (path,journal)=audit();
        let mut mismatch=Replies(VecDeque::from(vec![
            r#"{"kind":"tool","name":"lu_validate_schema","arguments":{"schema":{"type":"string"},"data":42}}"#.into()
        ]));
        assert!(turn.run_journaled_observed("Validate this exact user schema/data",
            &mut mismatch,&Cancellation::default(),&journal,&|_|{}).await.is_err());
        drop(turn);drop(journal);let _=fs::remove_file(path);
        let mut turn=CoreUtilityTurn::new(&Original,"c-original","g-original",
            "g-original",CoreUtilityRequest::Validate{
                schema:json!({"type":"integer"}),data:json!(42),
            }).unwrap();
        let (path,journal)=audit();
        let mut replies=Replies(VecDeque::from(vec![
            r#"{"kind":"tool","name":"lu_validate_schema","arguments":{"schema":{"type":"integer"},"data":42}}"#.into(),
            r#"{"kind":"final","text":"The exact supplied data matches the supplied schema."}"#.into()
        ]));
        let result=turn.run_journaled_observed("Validate this exact user schema/data",
            &mut replies,&Cancellation::default(),&journal,&|_|{}).await.unwrap();
        assert_eq!(result.tool_calls,1);
        assert_eq!(result.answer,"The exact supplied data matches the supplied schema.");
        assert!(turn.capabilities().is_err() ||
            turn.capabilities().unwrap().is_empty());
        drop(turn);drop(journal);let _=fs::remove_file(path);
    }
    #[tokio::test]
    async fn environment_scope_refuses_unrequested_program_and_fabricated_turn(){
        let spec=HostProgramSpec{id:"python".into(),candidate_files:vec![
            if cfg!(windows){PathBuf::from(r"C:\Program Files\Python313\python.exe")}
            else{PathBuf::from("/usr/bin/python3")}
        ]};
        assert!(CoreUtilityTurn::new(&Original,"c-original","g-fabricated",
            "g-fabricated",CoreUtilityRequest::CheckProgram{
                program:"python".into(),spec:spec.clone()
            }).is_err());
        assert!(CoreUtilityTurn::new(&Original,"c-original","g-original",
            "g-original",CoreUtilityRequest::CheckProgram{
                program:"git".into(),spec:spec.clone()
            }).is_err());
        let mut turn=CoreUtilityTurn::new(&Original,"c-original","g-original",
            "g-original",CoreUtilityRequest::CheckProgram{
                program:"python".into(),spec
            }).unwrap();
        let names=turn.capabilities().unwrap();
        assert_eq!(names.len(),1);
        assert_eq!(names[0].public_name,"lu_check_env");
        let (path,journal)=audit();
        let mut replies=Replies(VecDeque::from(vec![
            r#"{"kind":"tool","name":"lu_check_env","arguments":{"program":"git"}}"#.into()
        ]));
        assert!(turn.run_journaled_observed("Check Python on this computer",
            &mut replies,&Cancellation::default(),&journal,&|_|{}).await.is_err());
        drop(turn);drop(journal);let _=fs::remove_file(path);
        let spec=HostProgramSpec{id:"python".into(),candidate_files:vec![
            if cfg!(windows){PathBuf::from(r"C:\Program Files\Python313\python.exe")}
            else{PathBuf::from("/usr/bin/python3")}
        ]};
        let mut current=CoreUtilityTurn::new(&Original,"c-original","g-original",
            "g-original",CoreUtilityRequest::CheckProgram{
                program:"python".into(),spec,
            }).unwrap();
        let (path,journal)=audit();
        let mut chosen=Replies(VecDeque::from(vec![
            r#"{"kind":"tool","name":"lu_check_env","arguments":{"program":"python"}}"#.into(),
            r#"{"kind":"final","text":"The approved candidate presence was checked; this does not prove runtime execution."}"#.into()
        ]));
        let report=current.run_journaled_observed(
            "Check if Python is installed on my computer",
            &mut chosen,&Cancellation::default(),&journal,&|_|{}
        ).await.unwrap();
        assert_eq!(report.tool_calls,1);
        assert!(report.answer.contains("does not prove runtime execution"));
        assert!(current.capabilities().unwrap().is_empty());
        drop(current);drop(journal);let _=fs::remove_file(path);
    }
    #[tokio::test]
    async fn revoked_or_cancelled_original_turn_never_dispatches(){
        let request=||CoreUtilityRequest::Validate{
            schema:json!({"type":"integer"}),data:json!(42),
        };
        let mut revoked=CoreUtilityTurn::new(&Original,"c-original","g-original",
            "g-original",request()).unwrap();
        revoked.authority.revoke_task("g-original").unwrap();
        let (path,journal)=audit();
        let mut model=Replies(VecDeque::from(vec![
            r#"{"kind":"tool","name":"lu_validate_schema","arguments":{"schema":{"type":"integer"},"data":42}}"#.into()
        ]));
        assert!(revoked.run_journaled_observed("Validate original JSON",
            &mut model,&Cancellation::default(),&journal,&|_|{}).await.is_err());
        assert_eq!(model.0.len(),1,"revocation must refuse before model generation");
        drop(revoked);drop(journal);let _=fs::remove_file(path);
        let mut cancelled=CoreUtilityTurn::new(&Original,"c-original","g-original",
            "g-original",request()).unwrap();
        let cancel=Cancellation::default();cancel.cancel();
        let (path,journal)=audit();
        let mut model=Replies(VecDeque::from(vec![
            r#"{"kind":"tool","name":"lu_validate_schema","arguments":{"schema":{"type":"integer"},"data":42}}"#.into()
        ]));
        assert!(cancelled.run_journaled_observed("Validate original JSON",
            &mut model,&cancel,&journal,&|_|{}).await.is_err());
        assert_eq!(model.0.len(),1,"cancelled task may not call model or tool");
        drop(cancelled);drop(journal);let _=fs::remove_file(path);
    }
}
