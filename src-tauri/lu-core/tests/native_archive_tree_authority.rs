use std::{fs,io::Write,sync::{Arc,atomic::{AtomicU64,Ordering}}};
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use zip::{ZipWriter,write::SimpleFileOptions,CompressionMethod};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        TrustedUserTurnVerifier,
    },
    journal::{EffectJournal,EffectState},
    model::ModelToolCatalog,
    native_archive_tree::{preview_archive_tree,observe_tree_completion},
    native_archive_tree_tool::{
        register_archive_tree_extractor,ExactArchiveTreePolicy,TOOL_ID,INPUT,OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::{DispatchError,NativeToolRegistry},
    wire::proto,
};
static COUNTER:AtomicU64=AtomicU64::new(0);
struct SavedHuman;
impl TrustedUserTurnVerifier for SavedHuman{
    fn verify_user_turn(&self,c:&str,t:&str)->bool{
        c=="archive-chat"&&t=="g-user-archive"
    }
}
#[tokio::test]
async fn full_tree_extraction_requires_exact_approved_manifest_and_reconciles(){
    let root=std::env::temp_dir().join(format!(
        "lu-tree-policy-{}-{}",std::process::id(),COUNTER.fetch_add(1,Ordering::SeqCst)
    ));
    fs::create_dir_all(&root).unwrap();
    let mut zip=ZipWriter::new(fs::File::create(root.join("source.zip")).unwrap());
    zip.start_file("nested/a.txt",SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)).unwrap();
    zip.write_all(b"alpha").unwrap();
    zip.finish().unwrap();
    let dir=Arc::new(Dir::open_ambient_dir(&root,ambient_authority()).unwrap());
    let preview=preview_archive_tree(&dir,"source.zip").unwrap();
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_archive_tree_extractor(
        &registry,&mut catalog,Arc::clone(&dir),Arc::clone(&dir)
    ).unwrap();
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"archive-chat".into(),
        user_turn_id:"g-user-archive".into(),
        task_id:"archive-tree-task".into(),
    };
    authority.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),execution_scope_id:"archive-approved".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },&SavedHuman).unwrap();
    policy.grant(CapabilityGrant{
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:"archive-approved".into(),expires_at:None,
    }).unwrap();
    let fence=ExactArchiveTreePolicy{
        host_policy:&policy,trusted_source:&dir,
        source_name:"source.zip",destination_folder:"approved-folder",
        manifest_sha256:&preview.sha256,
    };
    let request=|folder:&str,id:&str|ToolInvocation{
        call_id:id.into(),tool_id:TOOL_ID.into(),
        workspace_id:Some("archive-approved".into()),
        input:ProtoPayload{
            type_url:INPUT.into(),
            bytes:proto::ArchiveTreeExtractRequest{
                source_name:"source.zip".into(),target_folder:folder.into(),
                approved_manifest_sha256:preview.sha256.clone(),
            }.encode_to_vec(),
        },
    };
    assert!(matches!(authority.dispatch_bound_durable(
        &registry,&journal,request("unapproved-folder","tree-denied-001"),
        origin.clone(),&fence
    ).await,Err(DispatchError::Denied(_))));
    assert!(!root.join("unapproved-folder").exists());
    let approved=request("approved-folder","tree-approved-001");
    let out=authority.dispatch_bound_durable(
        &registry,&journal,approved.clone(),origin.clone(),&fence
    ).await.unwrap();
    assert_eq!(out.payload.type_url,OUTPUT);
    let receipt=proto::ArchiveTreeExtractResult::decode(out.payload.bytes.as_slice()).unwrap();
    assert_eq!(receipt.files_extracted,1);
    assert_eq!(receipt.manifest_sha256,preview.sha256);
    assert_eq!(fs::read(root.join("approved-folder/nested/a.txt")).unwrap(),b"alpha");
    assert!(observe_tree_completion(&dir,"approved-folder",&preview).unwrap());
    assert_eq!(journal.get(&approved.call_id).unwrap().unwrap().state,EffectState::Applied);
    assert_eq!(authority.dispatch_bound_durable(
        &registry,&journal,approved.clone(),origin.clone(),&fence
    ).await.unwrap(),out);
    authority.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(authority.dispatch_bound_durable(
        &registry,&journal,approved,origin,&fence
    ).await,Err(DispatchError::Denied(_))));
    drop(journal);drop(catalog);drop(registry);drop(dir);
    fs::remove_dir_all(root).unwrap();
}
