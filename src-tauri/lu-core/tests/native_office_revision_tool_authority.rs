use std::{fs,sync::{Arc,atomic::{AtomicU64,Ordering}}};
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use sha2::{Sha256,Digest};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        TrustedUserTurnVerifier,
    },
    journal::{EffectJournal,EffectState},
    model::ModelToolCatalog,
    native_docx_create::{render_docx,DocxBlock,DocxBlockKind},
    native_office_revision::{render_office_revision,OfficeRevisionEdit},
    native_office_revision_tool::{
        register_office_revision_tool,ExactOfficeRevisionPolicy,TOOL_ID,INPUT,OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::{DispatchError,NativeToolRegistry},
    wire::proto,
};
static NEXT:AtomicU64=AtomicU64::new(0);
struct Human;
impl TrustedUserTurnVerifier for Human{
    fn verify_user_turn(&self,c:&str,t:&str)->bool{
        c=="office-chat"&&t=="g-office-approved"
    }
}
#[tokio::test]
async fn existing_word_document_gets_exact_approved_non_destructive_revision(){
    let root=std::env::temp_dir().join(format!(
        "lu-office-revision-tool-{}-{}",
        std::process::id(),NEXT.fetch_add(1,Ordering::SeqCst)
    ));
    fs::create_dir_all(&root).unwrap();
    let original=render_docx(&[
        DocxBlock{kind:DocxBlockKind::Title,text:"Original heading".into()},
        DocxBlock{kind:DocxBlockKind::Paragraph,text:"Body unchanged".into()},
    ]).unwrap();
    fs::write(root.join("original.docx"),&original).unwrap();
    let baseline=format!("{:x}",Sha256::digest(&original));
    let edit=OfficeRevisionEdit::WordRun{
        text_index:0,expected_old:"Original heading".into(),
        new_text:"Approved heading".into(),
    };
    let new=render_office_revision(&original,&baseline,&edit).unwrap();
    let approved=proto::OfficeRevisionRequest{
        source_name:"original.docx".into(),
        target_new_name:"revision.docx".into(),
        original_sha256:baseline,
        approved_revised_sha256:new.revised_sha256.clone(),
        change:Some(proto::office_revision_request::Change::WordText(
            proto::OfficeTextPatch{
                text_index:0,slide_number:0,
                expected_old:"Original heading".into(),
                new_text:"Approved heading".into(),
            }
        )),
    };
    let source=Arc::new(Dir::open_ambient_dir(&root,ambient_authority()).unwrap());
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_office_revision_tool(
        &registry,&mut catalog,Arc::clone(&source),Arc::clone(&source)
    ).unwrap();
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"office-chat".into(),
        user_turn_id:"g-office-approved".into(),
        task_id:"office-revision-task".into(),
    };
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),execution_scope_id:"office-scope".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },&Human).unwrap();
    policy.grant(CapabilityGrant{
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:"office-scope".into(),expires_at:None,
    }).unwrap();
    let fence=ExactOfficeRevisionPolicy{
        host_policy:&policy,trusted_source:&source,approved_request:&approved,
    };
    let request=|r:proto::OfficeRevisionRequest,id:&str|ToolInvocation{
        call_id:id.into(),tool_id:TOOL_ID.into(),
        workspace_id:Some("office-scope".into()),
        input:ProtoPayload{type_url:INPUT.into(),bytes:r.encode_to_vec()},
    };
    let mut wrong=approved.clone();
    wrong.target_new_name="unapproved.docx".into();
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,request(wrong,"office-denied-001"),origin.clone(),&fence
    ).await,Err(DispatchError::Denied(_))));
    assert!(!root.join("unapproved.docx").exists());
    let exact=request(approved.clone(),"office-approved-001");
    let output=auth.dispatch_bound_durable(
        &registry,&journal,exact.clone(),origin.clone(),&fence
    ).await.unwrap();
    assert_eq!(output.payload.type_url,OUTPUT);
    let receipt=proto::OfficeRevisionResult::decode(output.payload.bytes.as_slice()).unwrap();
    assert_eq!(receipt.revised_sha256,new.revised_sha256);
    assert_eq!(fs::read(root.join("original.docx")).unwrap(),original);
    assert_eq!(fs::read(root.join("revision.docx")).unwrap(),new.revised_bytes);
    assert_eq!(journal.get(&exact.call_id).unwrap().unwrap().state,EffectState::Applied);
    assert_eq!(auth.dispatch_bound_durable(
        &registry,&journal,exact.clone(),origin.clone(),&fence
    ).await.unwrap(),output);
    auth.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&journal,exact,origin,&fence
    ).await,Err(DispatchError::Denied(_))));
    drop(journal);drop(catalog);drop(registry);drop(source);
    fs::remove_dir_all(root).unwrap();
}
