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
    native_docx_create::{render_docx,DocxBlock,DocxBlockKind},
    native_docx_tool::{register_docx_create,ExactDocxCreatePolicy,TOOL_ID,INPUT,OUTPUT},
    policy::{CapabilityGrant,PolicyEngine},
    registry::{NativeToolRegistry,DispatchError},
    wire::proto,
};
use sha2::{Sha256,Digest};
use prost::Message;
static SEQ:AtomicU64=AtomicU64::new(0);
struct SavedHuman;
impl TrustedUserTurnVerifier for SavedHuman{
    fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
        chat=="chat-docx-approved"&&turn=="g-user-docx-approved"
    }
}
#[tokio::test]
async fn native_word_creation_is_user_bound_exact_content_and_journaled(){
    let root=std::env::temp_dir().join(format!("lu-docx-authority-{}-{}",
        std::process::id(),SEQ.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&root).unwrap();
    let dir=Arc::new(Dir::open_ambient_dir(&root,ambient_authority()).unwrap());
    let blocks=vec![DocxBlock{
        kind:DocxBlockKind::Title,text:"Approved Proposal".into()
    }];
    let bytes=render_docx(&blocks).unwrap();
    let expected=format!("{:x}",Sha256::digest(&bytes));
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_docx_create(&registry,&mut catalog,Arc::clone(&dir)).unwrap();
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"chat-docx-approved".into(),
        user_turn_id:"g-user-docx-approved".into(),
        task_id:"docx-create-task".into(),
    };
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"docx-approval".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },&SavedHuman).unwrap();
    policy.grant(CapabilityGrant{
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:"docx-approval".into(),expires_at:None,
    }).unwrap();
    let fence=ExactDocxCreatePolicy{
        host_policy:&policy,file_name:"approved.docx",expected_sha256:&expected,
    };
    let request=|name:&str,call_id:&str|ToolInvocation{
        call_id:call_id.into(),tool_id:TOOL_ID.into(),
        workspace_id:Some("docx-approval".into()),
        input:ProtoPayload{
            type_url:INPUT.into(),
            bytes:proto::DocxCreateRequest{
                file_name:name.into(),
                blocks:vec![proto::DocxCreateBlock{
                    kind:"title".into(),text:"Approved Proposal".into(),
                }],
            }.encode_to_vec(),
        },
    };
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,request("elsewhere.docx","g-docx-deny-001"),
        origin.clone(),&fence
    ).await,Err(DispatchError::Denied(_))));
    assert!(!root.join("elsewhere.docx").exists());
    let approved=request("approved.docx","g-docx-allowed-001");
    let output=auth.dispatch_bound_durable(
        &registry,&journal,approved.clone(),origin.clone(),&fence
    ).await.unwrap();
    assert_eq!(output.payload.type_url,OUTPUT);
    let receipt=proto::DocxCreateResult::decode(output.payload.bytes.as_slice()).unwrap();
    assert_eq!(receipt.sha256,expected);
    assert_eq!(fs::read(root.join("approved.docx")).unwrap(),bytes);
    assert_eq!(journal.get(&approved.call_id).unwrap().unwrap().state,EffectState::Applied);
    assert_eq!(auth.dispatch_bound_durable(
        &registry,&journal,approved.clone(),origin.clone(),&fence
    ).await.unwrap(),output);
    auth.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,approved,origin,&fence
    ).await,Err(DispatchError::Denied(_))));
    drop(journal);drop(catalog);drop(registry);drop(dir);
    fs::remove_dir_all(root).unwrap();
}
