use std::{fs,sync::{Arc,atomic::{AtomicU64,Ordering}}};
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        TrustedUserTurnVerifier,
    },
    journal::{EffectJournal,EffectState},
    model::ModelToolCatalog,
    native_file_transfer::{source_digest,TrashReceipt},
    native_trash_custody::{TrashCustodyLedger,CustodyRecord,CustodyState},
    native_file_trash_tool::{
        register_trash_restore_tools,ExactTrashPolicy,ExactRestorePolicy,
        TRASH_ID,RESTORE_ID,TRASH_INPUT,TRASH_OUTPUT,RESTORE_INPUT,RESTORE_OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::{DispatchError,NativeToolRegistry},
    wire::proto,
};
static NEXT:AtomicU64=AtomicU64::new(0);
struct Human;
impl TrustedUserTurnVerifier for Human{
    fn verify_user_turn(&self,c:&str,t:&str)->bool{
        c=="trash-chat"&&t=="g-user-trash"
    }
}
#[tokio::test]
async fn reversible_trash_and_restore_require_independent_exact_grants(){
    let root=std::env::temp_dir().join(format!("lu-trash-policy-{}-{}",
        std::process::id(),NEXT.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(root.join("trash")).unwrap();
    fs::write(root.join("document.txt"),b"retain me").unwrap();
    let source=Arc::new(Dir::open_ambient_dir(&root,ambient_authority()).unwrap());
    let trash=Arc::new(Dir::open_ambient_dir(root.join("trash"),ambient_authority()).unwrap());
    let (_,digest)=source_digest(&source,"document.txt").unwrap();
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_trash_restore_tools(
        &registry,&mut catalog,Arc::clone(&source),Arc::clone(&trash),None
    ).unwrap();
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let ledger=TrashCustodyLedger::open(&root.join("custody.sqlite")).unwrap();
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"trash-chat".into(),
        user_turn_id:"g-user-trash".into(),task_id:"trash-task".into(),
    };
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"trash-approved".into(),
        capabilities:vec![
            AuthorizedCapability{tool_id:TRASH_ID.into(),effect:ToolEffect::WriteWorkspace},
            AuthorizedCapability{tool_id:RESTORE_ID.into(),effect:ToolEffect::WriteWorkspace},
        ],expires_at:None,
    },&Human).unwrap();
    for id in [TRASH_ID,RESTORE_ID]{
        policy.grant(CapabilityGrant{
            tool_id:id.into(),effect:ToolEffect::WriteWorkspace,
            scope_id:"trash-approved".into(),expires_at:None,
        }).unwrap();
    }
    let fence=ExactTrashPolicy{
        host_policy:&policy,trusted_source:&source,
        source_name:"document.txt",source_sha256:&digest,
    };
    let bad=ToolInvocation{
        call_id:"trash-deny-123456".into(),tool_id:TRASH_ID.into(),
        workspace_id:Some("trash-approved".into()),
        input:ProtoPayload{
            type_url:TRASH_INPUT.into(),
            bytes:proto::FileTrashRequest{
                source_name:"other.txt".into(),expected_sha256:digest.clone(),
            }.encode_to_vec(),
        },
    };
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,bad,origin.clone(),&fence
    ).await,Err(DispatchError::Denied(_))));
    let request=ToolInvocation{
        call_id:"trash-transaction-1234567".into(),tool_id:TRASH_ID.into(),
        workspace_id:Some("trash-approved".into()),
        input:ProtoPayload{
            type_url:TRASH_INPUT.into(),
            bytes:proto::FileTrashRequest{
                source_name:"document.txt".into(),expected_sha256:digest.clone(),
            }.encode_to_vec(),
        },
    };
    let intended=CustodyRecord{
        id:"ct-real-record-001".into(),chat_id:"trash-chat".into(),
        parent:root.to_string_lossy().into_owned(),original:"document.txt".into(),
        trash_entry:format!("lu-trash-{}",request.call_id),bytes:9,
        sha256:digest.clone(),trash_effect_id:request.call_id.clone(),
        restore_effect_id:None,state:CustodyState::Pending,
    };
    ledger.stage(&intended).unwrap();
    let output=auth.dispatch_bound_durable(
        &registry,&journal,request.clone(),origin.clone(),&fence
    ).await.unwrap();
    assert_eq!(output.payload.type_url,TRASH_OUTPUT);
    assert!(!root.join("document.txt").exists());
    assert_eq!(journal.get(&request.call_id).unwrap().unwrap().state,EffectState::Applied);
    let custody=proto::FileTrashResult::decode(output.payload.bytes.as_slice()).unwrap();
    assert_eq!(custody.sha256,digest);
    let verified_receipt=TrashReceipt{
        original:custody.original_name,trash_entry:custody.trash_entry,
        bytes:custody.bytes,sha256:custody.sha256,
    };
    assert!(root.join("trash").join(&verified_receipt.trash_entry).is_file());
    let archived=ledger.confirm_trash(&intended.id,&intended.chat_id,&journal).unwrap();
    assert_eq!(archived.state,CustodyState::Trashed);
    let (verified,receipt)=ledger.validated_trash_receipt(
        &intended.id,&intended.chat_id,&journal
    ).unwrap();
    assert_eq!(verified.sha256,digest);
    assert_eq!(receipt,verified_receipt);
    let restore_registry=NativeToolRegistry::new();
    let mut restore_catalog=ModelToolCatalog::new();
    register_trash_restore_tools(
        &restore_registry,&mut restore_catalog,
        Arc::clone(&source),Arc::clone(&trash),Some(verified_receipt.clone())
    ).unwrap();
    let restore_fence=ExactRestorePolicy{
        host_policy:&policy,trusted_receipt_id:&verified_receipt.trash_entry,
        receipt:&verified_receipt,
    };
    let restore=ToolInvocation{
        call_id:"restore-transaction-1234567".into(),tool_id:RESTORE_ID.into(),
        workspace_id:Some("trash-approved".into()),
        input:ProtoPayload{
            type_url:RESTORE_INPUT.into(),
            bytes:proto::FileRestoreRequest{
                trusted_receipt_id:verified_receipt.trash_entry.clone(),
            }.encode_to_vec(),
        },
    };
    let pending=ledger.begin_restore(&intended.id,&intended.chat_id,&restore.call_id).unwrap();
    assert_eq!(pending.state,CustodyState::Restoring);
    let restored=auth.dispatch_bound_durable(
        &restore_registry,&journal,restore.clone(),origin.clone(),&restore_fence
    ).await.unwrap();
    assert_eq!(restored.payload.type_url,RESTORE_OUTPUT);
    let result=proto::FileRestoreResult::decode(restored.payload.bytes.as_slice()).unwrap();
    assert_eq!(result.sha256,digest);
    assert_eq!(fs::read(root.join("document.txt")).unwrap(),b"retain me");
    assert!(!root.join("trash").join(&verified_receipt.trash_entry).exists());
    assert_eq!(journal.get(&restore.call_id).unwrap().unwrap().state,EffectState::Applied);
    assert_eq!(ledger.confirm_restore(&intended.id,&intended.chat_id,&journal)
        .unwrap().state,CustodyState::Restored);
    assert!(ledger.validated_trash_receipt(&intended.id,&intended.chat_id,&journal).is_err());
    auth.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(auth.dispatch_bound_durable(
        &restore_registry,&journal,restore,origin,&restore_fence
    ).await,Err(DispatchError::Denied(_))));
    drop(ledger);drop(journal);drop(restore_catalog);drop(restore_registry);
    let persisted=TrashCustodyLedger::open(&root.join("custody.sqlite")).unwrap();
    assert_eq!(persisted.list_chat("trash-chat").unwrap()[0].state,CustodyState::Restored);
    drop(persisted);
    drop(catalog);drop(registry);drop(trash);drop(source);
    fs::remove_dir_all(root).unwrap();
}
