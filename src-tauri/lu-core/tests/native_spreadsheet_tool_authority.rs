//! Genuine native spreadsheet tool with exact target/content host approval.
use std::{fs,path::PathBuf,sync::{Arc,atomic::{AtomicU64,Ordering}}};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,
        ConversationAuthority,TaskAuthority,TrustedUserTurnVerifier},
    journal::{EffectJournal,EffectState},
    model::ModelToolCatalog,
    native_spreadsheet_create::{render_xlsx,SpreadsheetCell,CellValue},
    native_spreadsheet_tool::{register_spreadsheet_create,ExactSpreadsheetCreatePolicy,
        TOOL_ID,INPUT,OUTPUT,MODEL_NAME},
    policy::{CapabilityGrant,PolicyEngine},
    registry::{NativeToolRegistry,DispatchError},
    wire::proto,
};
use prost::Message;
use sha2::{Sha256,Digest};
static COUNT:AtomicU64=AtomicU64::new(0);
fn fixture()->(PathBuf,Arc<Dir>){
    let path=std::env::temp_dir().join(format!(
        "lu-sheet-tool-{}-{}",std::process::id(),COUNT.fetch_add(1,Ordering::SeqCst)
    ));
    fs::create_dir(&path).unwrap();
    let dir=Dir::open_ambient_dir(&path,ambient_authority()).unwrap();
    (path,Arc::new(dir))
}
struct Verifier;
impl TrustedUserTurnVerifier for Verifier{
    fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
        chat=="chat-approved"&&turn=="g-user-approved"
    }
}
fn authorized()->(ConversationAuthority,PolicyEngine,BoundCallOrigin){
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let origin=BoundCallOrigin{
        conversation_id:"chat-approved".into(),user_turn_id:"g-user-approved".into(),
        task_id:"sheet-task-01".into(),
    };
    auth.open_user_task(TaskAuthority {
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"approved-file".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },&Verifier).unwrap();
    policy.grant(CapabilityGrant {
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:"approved-file".into(),expires_at:None,
    }).unwrap();
    (auth,policy,origin)
}
fn request(file_name:&str,call_id:&str)->ToolInvocation{
    ToolInvocation {
        call_id:call_id.into(),tool_id:TOOL_ID.into(),
        workspace_id:Some("approved-file".into()),
        input:ProtoPayload {
            type_url:INPUT.into(),
            bytes:proto::SpreadsheetCreateRequest{
                file_name:file_name.into(),sheet_name:"Expenses".into(),
                cells:vec![
                    proto::SpreadsheetCreateCell{
                        row:1,column:1,value:Some(
                            proto::spreadsheet_create_cell::Value::Text("Cost".into())
                        ),
                    },
                    proto::SpreadsheetCreateCell{
                        row:2,column:1,value:Some(
                            proto::spreadsheet_create_cell::Value::Number(27.5)
                        ),
                    },
                ],
            }.encode_to_vec(),
        },
    }
}
fn digest()->String{
    let bytes=render_xlsx("Expenses",&[
        SpreadsheetCell{row:1,column:1,value:CellValue::Text("Cost".into())},
        SpreadsheetCell{row:2,column:1,value:CellValue::Number(27.5)},
    ]).unwrap();
    format!("{:x}",Sha256::digest(&bytes))
}
#[tokio::test]
async fn real_approved_xlsx_write_is_journaled_and_no_clobber(){
    let (root,dir)=fixture();
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_spreadsheet_create(&registry,&mut catalog,Arc::clone(&dir)).unwrap();
    assert!(catalog.names_for_native_ids(&std::collections::BTreeSet::from([TOOL_ID.to_owned()]))
        .contains(MODEL_NAME));
    let effects=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let (auth,policy,origin)=authorized();
    let sha=digest();
    let fence=ExactSpreadsheetCreatePolicy{
        host_policy:&policy,approved_file_name:"budget.xlsx",approved_sha256:&sha,
    };
    let call=request("budget.xlsx","g-sheet-write-approved-001");
    let response=auth.dispatch_bound_durable(
        &registry,&effects,call.clone(),origin.clone(),&fence
    ).await.unwrap();
    assert_eq!(response.payload.type_url,OUTPUT);
    let receipt=proto::SpreadsheetCreateResult::decode(response.payload.bytes.as_slice()).unwrap();
    assert_eq!(receipt.file_name,"budget.xlsx");
    assert_eq!(receipt.sha256,sha);
    assert_eq!(effects.get(&call.call_id).unwrap().unwrap().state,EffectState::Applied);
    let original=fs::read(root.join("budget.xlsx")).unwrap();
    let mut zip=zip::ZipArchive::new(std::io::Cursor::new(&original)).unwrap();
    assert!(zip.by_name("xl/worksheets/sheet1.xml").is_ok());
    drop(zip);
    // Persisted result is served without rewriting. Approval is still
    // reevaluated on every attempt, and revocation prohibits cached replay.
    assert_eq!(auth.dispatch_bound_durable(
        &registry,&effects,call.clone(),origin.clone(),&fence
    ).await.unwrap(),response);
    assert_eq!(fs::read(root.join("budget.xlsx")).unwrap(),original);
    auth.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&effects,call,origin,&fence
    ).await,Err(DispatchError::Denied(_))));
    drop(effects);drop(catalog);drop(registry);drop(dir);
    fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn mismatched_target_or_bytes_are_denied_before_journal_or_effect(){
    let (root,dir)=fixture();
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_spreadsheet_create(&registry,&mut catalog,Arc::clone(&dir)).unwrap();
    let effects=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let (auth,policy,origin)=authorized();
    let sha=digest();
    let fence=ExactSpreadsheetCreatePolicy{
        host_policy:&policy,approved_file_name:"budget.xlsx",approved_sha256:&sha,
    };
    let bad_name=request("different.xlsx","g-sheet-invalid-target");
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&effects,bad_name,origin.clone(),&fence
    ).await,Err(DispatchError::Denied(_))));
    let wrong_sha="0".repeat(64);
    let fence_bad=ExactSpreadsheetCreatePolicy {
        host_policy:&policy,approved_file_name:"budget.xlsx",approved_sha256:&wrong_sha,
    };
    let bad_bytes=request("budget.xlsx","g-sheet-invalid-bytes");
    assert!(matches!(auth.dispatch_bound_durable(
        &registry,&effects,bad_bytes,origin,&fence_bad
    ).await,Err(DispatchError::Denied(_))));
    assert!(!root.join("budget.xlsx").exists());
    drop(effects);drop(catalog);drop(registry);drop(dir);
    fs::remove_dir_all(root).unwrap();
}
