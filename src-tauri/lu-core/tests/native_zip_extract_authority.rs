use std::{fs,io::Write,sync::{Arc,atomic::{AtomicU64,Ordering}}};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        TrustedUserTurnVerifier,
    },
    journal::{EffectJournal,EffectState},
    model::ModelToolCatalog,
    native_zip_extract::zip_member_sha256,
    native_zip_extract_tool::{
        register_zip_extractor,ExactZipExtractPolicy,TOOL_ID,INPUT,OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::{NativeToolRegistry,DispatchError},
    wire::proto,
};
use prost::Message;
use zip::{ZipWriter,write::SimpleFileOptions,CompressionMethod};
static N:AtomicU64=AtomicU64::new(0);
struct Persisted;
impl TrustedUserTurnVerifier for Persisted{
    fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
        chat=="approved-chat"&&turn=="g-user-zip-extract"
    }
}
#[tokio::test]
async fn zip_extract_bound_to_user_turn_exact_file_and_durable_effect(){
    let root=std::env::temp_dir().join(format!("lu-zip-policy-{}-{}",
        std::process::id(),N.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&root).unwrap();
    let mut z=ZipWriter::new(fs::File::create(root.join("bundle.zip")).unwrap());
    z.start_file("dir/payload.txt",SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)).unwrap();
    z.write_all(b"hello world").unwrap();
    z.finish().unwrap();
    let directory=Arc::new(Dir::open_ambient_dir(&root,ambient_authority()).unwrap());
    let sha=zip_member_sha256(&directory,"bundle.zip","dir/payload.txt").unwrap();
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_zip_extractor(&registry,&mut catalog,Arc::clone(&directory),
        Arc::clone(&directory)).unwrap();
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"approved-chat".into(),user_turn_id:"g-user-zip-extract".into(),
        task_id:"zip-task-001".into(),
    };
    authority.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"exact-zip-output".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },&Persisted).unwrap();
    policy.grant(CapabilityGrant{
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:"exact-zip-output".into(),expires_at:None,
    }).unwrap();
    let fence=ExactZipExtractPolicy{
        host_policy:&policy,trusted_source:&directory,
        archive_name:"bundle.zip",member_path:"dir/payload.txt",
        target_name:"approved.txt",expected_sha256:&sha,
    };
    let request=|target:&str,id:&str|ToolInvocation{
        call_id:id.into(),tool_id:TOOL_ID.into(),
        workspace_id:Some("exact-zip-output".into()),
        input:ProtoPayload{
            type_url:INPUT.into(),
            bytes:proto::ZipExtractRequest{
                archive_name:"bundle.zip".into(),member_path:"dir/payload.txt".into(),
                target_name:target.into(),expected_sha256:sha.clone(),
            }.encode_to_vec(),
        },
    };
    let wrong=request("unauthorized.txt","zip-unauthorized-001");
    assert!(matches!(authority.dispatch_bound_durable(
        &registry,&journal,wrong,origin.clone(),&fence
    ).await,Err(DispatchError::Denied(_))));
    assert!(!root.join("unauthorized.txt").exists());
    let approved=request("approved.txt","zip-approved-001");
    let output=authority.dispatch_bound_durable(
        &registry,&journal,approved.clone(),origin.clone(),&fence
    ).await.unwrap();
    assert_eq!(output.payload.type_url,OUTPUT);
    let receipt=proto::ZipExtractResult::decode(output.payload.bytes.as_slice()).unwrap();
    assert_eq!(receipt.bytes_written,11);
    assert_eq!(receipt.sha256,sha);
    assert_eq!(fs::read(root.join("approved.txt")).unwrap(),b"hello world");
    assert_eq!(journal.get(&approved.call_id).unwrap().unwrap().state,EffectState::Applied);
    let replay=authority.dispatch_bound_durable(
        &registry,&journal,approved.clone(),origin.clone(),&fence
    ).await.unwrap();
    assert_eq!(replay,output);
    authority.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(authority.dispatch_bound_durable(
        &registry,&journal,approved,origin,&fence
    ).await,Err(DispatchError::Denied(_))));
    drop(journal);drop(catalog);drop(registry);drop(directory);
    fs::remove_dir_all(root).unwrap();
}
