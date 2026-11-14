use std::{fs,sync::{Arc,atomic::{AtomicU64,Ordering}}};
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use sha2::{Digest,Sha256};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        TrustedUserTurnVerifier,
    },
    journal::{EffectJournal,EffectState},
    model::ModelToolCatalog,
    native_docx_create::{render_docx,DocxBlock,DocxBlockKind},
    native_office_revision::{OfficeRevisionEdit,render_office_revision_batch},
    native_office_batch_tool::{
        register_office_batch_tool,ExactBatchPolicy,TOOL_ID,INPUT,OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::{DispatchError,NativeToolRegistry},wire::proto,
};
static NEXT:AtomicU64=AtomicU64::new(0);
struct User;
impl TrustedUserTurnVerifier for User{
    fn verify_user_turn(&self,c:&str,t:&str)->bool{
        c=="office-batch-chat"&&t=="g-batch-approved"
    }
}
#[tokio::test]
async fn exact_multiedit_package_requires_native_grant_and_journal(){
    let root=std::env::temp_dir().join(format!(
        "lu-office-batch-{}-{}",std::process::id(),
        NEXT.fetch_add(1,Ordering::SeqCst)
    ));
    fs::create_dir_all(&root).unwrap();
    let original=render_docx(&[
        DocxBlock{kind:DocxBlockKind::Title,text:"Before title".into()},
        DocxBlock{kind:DocxBlockKind::Paragraph,text:"Before body".into()},
    ]).unwrap();
    fs::write(root.join("original.docx"),&original).unwrap();
    let baseline=format!("{:x}",Sha256::digest(&original));
    let changes=vec![
        OfficeRevisionEdit::WordRun{
            text_index:0,expected_old:"Before title".into(),
            new_text:"After title".into(),
        },
        OfficeRevisionEdit::WordRun{
            text_index:1,expected_old:"Before body".into(),
            new_text:"After body".into(),
        },
    ];
    let preview=render_office_revision_batch(&original,&baseline,&changes).unwrap();
    let make=|source:&str,target:&str|proto::OfficeBatchRevisionRequest{
        source_name:source.into(),target_new_name:target.into(),
        original_sha256:baseline.clone(),
        approved_revised_sha256:preview.revised_sha256.clone(),
        patches:vec![
            proto::OfficeBatchPatch{change:Some(
                proto::office_batch_patch::Change::WordText(proto::OfficeTextPatch{
                    text_index:0,slide_number:0,expected_old:"Before title".into(),
                    new_text:"After title".into(),
                }))},
            proto::OfficeBatchPatch{change:Some(
                proto::office_batch_patch::Change::WordText(proto::OfficeTextPatch{
                    text_index:1,slide_number:0,expected_old:"Before body".into(),
                    new_text:"After body".into(),
                }))},
        ],
    };
    let approved=make("original.docx","approved-v2.docx");
    let source=Arc::new(Dir::open_ambient_dir(&root,ambient_authority()).unwrap());
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_office_batch_tool(&registry,&mut catalog,
        Arc::clone(&source),Arc::clone(&source)).unwrap();
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let auth=ConversationAuthority::new();let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"office-batch-chat".into(),
        user_turn_id:"g-batch-approved".into(),task_id:"batch-task".into(),
    };
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"batch-scope".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },&User).unwrap();
    policy.grant(CapabilityGrant{
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:"batch-scope".into(),expires_at:None,
    }).unwrap();
    let fence=ExactBatchPolicy{
        host_policy:&policy,trusted_source:&source,approved:&approved,
    };
    let call=|id:&str,req:proto::OfficeBatchRevisionRequest|ToolInvocation{
        call_id:id.into(),tool_id:TOOL_ID.into(),
        workspace_id:Some("batch-scope".into()),
        input:ProtoPayload{type_url:INPUT.into(),bytes:req.encode_to_vec()},
    };
    let invalid=call("batch-denied-012345",make("original.docx","wrong.docx"));
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,invalid,origin.clone(),&fence
    ).await,Err(DispatchError::Denied(_))));
    assert!(!root.join("wrong.docx").exists());
    let approved_call=call("batch-applied-012345",approved.clone());
    let output=auth.dispatch_bound_durable(
        &registry,&journal,approved_call.clone(),origin.clone(),&fence,
    ).await.unwrap();
    assert_eq!(output.payload.type_url,OUTPUT);
    let receipt=proto::OfficeRevisionResult::decode(output.payload.bytes.as_slice()).unwrap();
    assert_eq!(receipt.revised_sha256,preview.revised_sha256);
    assert_eq!(fs::read(root.join("approved-v2.docx")).unwrap(),preview.revised_bytes);
    assert_eq!(fs::read(root.join("original.docx")).unwrap(),original);
    assert_eq!(journal.get(&approved_call.call_id).unwrap().unwrap().state,EffectState::Applied);
    auth.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,approved_call,origin,&fence
    ).await,Err(DispatchError::Denied(_))));
    drop(journal);drop(catalog);drop(registry);drop(source);
    fs::remove_dir_all(root).unwrap();
}
