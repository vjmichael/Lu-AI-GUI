use std::{fs,sync::{Arc,atomic::{AtomicU64,Ordering}}};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        TrustedUserTurnVerifier,
    },
    journal::{EffectJournal,EffectState},
    model::ModelToolCatalog,
    native_file_transfer::source_digest,
    native_file_transfer_tool::{
        register_file_transfer_tools,ExactFileTransferPolicy,COPY_ID,MOVE_ID,INPUT,OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::{NativeToolRegistry,DispatchError},
    wire::proto,
};
use prost::Message;
static SEQ:AtomicU64=AtomicU64::new(0);
struct User;
impl TrustedUserTurnVerifier for User{
    fn verify_user_turn(&self,c:&str,t:&str)->bool{
        c=="file-chat"&&t=="g-user-files"
    }
}
fn fixture()->(std::path::PathBuf,Arc<Dir>){
    let root=std::env::temp_dir().join(format!(
        "lu-file-transfer-tool-{}-{}",
        std::process::id(),SEQ.fetch_add(1,Ordering::SeqCst)
    ));
    fs::create_dir_all(&root).unwrap();
    let dir=Arc::new(Dir::open_ambient_dir(&root,ambient_authority()).unwrap());
    (root,dir)
}
async fn check_operation(tool_id:&str,target:&str,move_source:bool){
    let (root,dir)=fixture();
    fs::write(root.join("source.txt"),b"original bytes").unwrap();
    let (_,sha)=source_digest(&dir,"source.txt").unwrap();
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_file_transfer_tools(&registry,&mut catalog,Arc::clone(&dir)).unwrap();
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"file-chat".into(),user_turn_id:"g-user-files".into(),
        task_id:"file-transfer-task".into(),
    };
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"file-approval".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:tool_id.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },&User).unwrap();
    policy.grant(CapabilityGrant{
        tool_id:tool_id.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:"file-approval".into(),expires_at:None,
    }).unwrap();
    let fence=ExactFileTransferPolicy{
        host_policy:&policy,source_root:&dir,tool_id,
        source_name:"source.txt",target_name:target,
        source_sha256:&sha,
    };
    let request=|name:&str,id:&str|ToolInvocation{
        call_id:id.into(),tool_id:tool_id.into(),
        workspace_id:Some("file-approval".into()),
        input:ProtoPayload{
            type_url:INPUT.into(),
            bytes:proto::FileTransferRequest{
                source_name:"source.txt".into(),target_name:name.into(),
                expected_source_sha256:sha.clone(),
            }.encode_to_vec(),
        },
    };
    let denied=request("elsewhere.txt","files-denied-001");
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,denied,origin.clone(),&fence
    ).await,Err(DispatchError::Denied(_))));
    assert!(!root.join("elsewhere.txt").exists());
    let approved=request(target,"files-approved-001");
    let output=auth.dispatch_bound_durable(
        &registry,&journal,approved.clone(),origin.clone(),&fence
    ).await.unwrap();
    assert_eq!(output.payload.type_url,OUTPUT);
    let receipt=proto::FileTransferResult::decode(output.payload.bytes.as_slice()).unwrap();
    assert_eq!(receipt.sha256,sha);
    assert_eq!(receipt.source_removed,move_source);
    assert_eq!(fs::read(root.join(target)).unwrap(),b"original bytes");
    assert_eq!(root.join("source.txt").exists(),!move_source);
    assert_eq!(journal.get(&approved.call_id).unwrap().unwrap().state,EffectState::Applied);
    if !move_source{
        assert_eq!(auth.dispatch_bound_durable(
            &registry,&journal,approved.clone(),origin.clone(),&fence
        ).await.unwrap(),output);
    }
    auth.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,approved,origin,&fence
    ).await,Err(DispatchError::Denied(_))));
    drop(journal);drop(catalog);drop(registry);drop(dir);
    fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn exact_approved_copy_uses_no_clobber_and_journal(){
    check_operation(COPY_ID,"copy.txt",false).await
}
#[tokio::test]
async fn exact_approved_move_removes_original_and_journals(){
    check_operation(MOVE_ID,"moved.txt",true).await
}
#[tokio::test]
async fn a_changed_source_digest_denies_before_any_side_effect(){
    let (root,dir)=fixture();
    fs::write(root.join("source.txt"),b"first").unwrap();
    let (_,digest)=source_digest(&dir,"source.txt").unwrap();
    fs::write(root.join("source.txt"),b"changed").unwrap();
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_file_transfer_tools(&registry,&mut catalog,Arc::clone(&dir)).unwrap();
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"file-chat".into(),user_turn_id:"g-user-files".into(),
        task_id:"file-transfer-task".into(),
    };
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),execution_scope_id:"file-approval".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:COPY_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },&User).unwrap();
    policy.grant(CapabilityGrant{
        tool_id:COPY_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:"file-approval".into(),expires_at:None,
    }).unwrap();
    let fence=ExactFileTransferPolicy{
        host_policy:&policy,source_root:&dir,tool_id:COPY_ID,
        source_name:"source.txt",target_name:"copy.txt",
        source_sha256:&digest,
    };
    let call=ToolInvocation{
        call_id:"files-hash-mismatch-001".into(),tool_id:COPY_ID.into(),
        workspace_id:Some("file-approval".into()),
        input:ProtoPayload{
            type_url:INPUT.into(),
            bytes:proto::FileTransferRequest{
                source_name:"source.txt".into(),target_name:"copy.txt".into(),
                expected_source_sha256:digest,
            }.encode_to_vec(),
        },
    };
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,call.clone(),origin,&fence
    ).await,Err(DispatchError::Denied(_))));
    assert!(journal.get(&call.call_id).unwrap().is_none());
    assert!(!root.join("copy.txt").exists());
    drop(journal);drop(catalog);drop(registry);drop(dir);
    fs::remove_dir_all(root).unwrap();
}
