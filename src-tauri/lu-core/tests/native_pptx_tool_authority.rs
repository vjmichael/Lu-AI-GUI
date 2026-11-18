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
    native_pptx_create::{render_pptx,PresentationSlide},
    native_pptx_tool::{
        register_pptx_create,ExactPptxCreatePolicy,TOOL_ID,INPUT,OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::{DispatchError,NativeToolRegistry},
    wire::proto,
};
static NEXT:AtomicU64=AtomicU64::new(0);
struct Human;
impl TrustedUserTurnVerifier for Human{
    fn verify_user_turn(&self,c:&str,t:&str)->bool{
        c=="pptx-chat"&&t=="g-human-pptx"
    }
}
#[tokio::test]
async fn pptx_creation_requires_exact_content_and_durable_human_turn(){
    let root=std::env::temp_dir().join(format!("lu-pptx-grant-{}-{}",
        std::process::id(),NEXT.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&root).unwrap();
    let dir=Arc::new(Dir::open_ambient_dir(&root,ambient_authority()).unwrap());
    let slides=vec![PresentationSlide{
        title:"Approved slide".into(),bullet_points:vec!["Real content".into()],
    }];
    let bytes=render_pptx(&slides).unwrap();
    let digest=format!("{:x}",Sha256::digest(&bytes));
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_pptx_create(&registry,&mut catalog,Arc::clone(&dir)).unwrap();
    let effects=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"pptx-chat".into(),user_turn_id:"g-human-pptx".into(),
        task_id:"pptx-task".into(),
    };
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"pptx-approved".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },&Human).unwrap();
    policy.grant(CapabilityGrant{
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:"pptx-approved".into(),expires_at:None,
    }).unwrap();
    let fence=ExactPptxCreatePolicy{
        host_policy:&policy,
        approved_filename:"deck.pptx",
        approved_sha256:&digest,
    };
    let call=|file:&str,id:&str|ToolInvocation{
        call_id:id.into(),tool_id:TOOL_ID.into(),
        workspace_id:Some("pptx-approved".into()),
        input:ProtoPayload{
            type_url:INPUT.into(),
            bytes:proto::PptxCreateRequest{
                file_name:file.into(),
                slides:vec![proto::PptxCreateSlide{
                    title:"Approved slide".into(),
                    bullet_points:vec!["Real content".into()],
                }],
            }.encode_to_vec(),
        },
    };
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&effects,call("not-approved.pptx","pptx-deny-001"),
        origin.clone(),&fence
    ).await,Err(DispatchError::Denied(_))));
    assert!(!root.join("not-approved.pptx").exists());
    let approved=call("deck.pptx","pptx-allowed-001");
    let out=auth.dispatch_bound_durable(
        &registry,&effects,approved.clone(),origin.clone(),&fence
    ).await.unwrap();
    assert_eq!(out.payload.type_url,OUTPUT);
    let receipt=proto::PptxCreateResult::decode(out.payload.bytes.as_slice()).unwrap();
    assert_eq!(receipt.sha256,digest);
    assert_eq!(fs::read(root.join("deck.pptx")).unwrap(),bytes);
    assert_eq!(effects.get(&approved.call_id).unwrap().unwrap().state,EffectState::Applied);
    assert_eq!(auth.dispatch_bound_durable(
        &registry,&effects,approved.clone(),origin.clone(),&fence
    ).await.unwrap(),out);
    auth.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&effects,approved,origin,&fence
    ).await,Err(DispatchError::Denied(_))));
    drop(effects);drop(catalog);drop(registry);drop(dir);
    fs::remove_dir_all(root).unwrap();
}
