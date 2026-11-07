//! F proof: real model-facing artifact inspection is bounded, read-only,
//! workspace scoped, and not equivalent to a user file-write or app action.
use lu_core::{
    contracts::{AuthorizationError,ToolAuthorizer,ToolDescriptor,ToolInvocation},
    model::ModelToolCatalog,
    native_artifact_inspect::register_artifact_inspection,
    registry::{DispatchError,NativeToolRegistry},
};
use std::{collections::BTreeSet,fs,path::PathBuf,
    sync::atomic::{AtomicU64,Ordering}};
static NEXT:AtomicU64=AtomicU64::new(0);
struct Deny;
impl ToolAuthorizer for Deny{
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
    ->Result<(),AuthorizationError>{Err(AuthorizationError::new("human denial"))}
}
struct Allow;
impl ToolAuthorizer for Allow{
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
    ->Result<(),AuthorizationError>{Ok(())}
}
fn fixture()->PathBuf{
    let dir=std::env::temp_dir().join(format!("lu-artifact-i-{}-{}",
        std::process::id(),NEXT.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir(&dir).unwrap();dir
}
#[tokio::test]
async fn model_cannot_approve_or_fabricate_an_artifact(){
    let dir=fixture();
    fs::write(dir.join("empty.xlsx"),b"").unwrap();
    fs::write(dir.join("record.json"),b"{\"written\":true}").unwrap();
    let reg=NativeToolRegistry::new();
    let mut cat=ModelToolCatalog::new();
    register_artifact_inspection(&reg,&mut cat,&dir).unwrap();
    let allowed=BTreeSet::from(["lu_artifact_inspect".to_owned()]);
    assert_eq!(cat.schemas(&allowed).unwrap().len(),1);
    for bad in [
        r#"{"path":"../secret.json","format":"json"}"#,
        r#"{"path":"record.json","format":"json","command":"delete"}"#,
        r#"{"path":"record.json","format":"json","expected_sha256":"abc"}"#,
        r#"{"path":"record.json","format":"unknown"}"#,
    ]{
        assert!(cat.encode_call(&allowed,"lu_artifact_inspect","bad",None,bad).is_err(),
            "bad artifact call accepted: {bad}");
    }
    let call=cat.encode_call(&allowed,"lu_artifact_inspect","artifact-1",None,
        r#"{"path":"record.json","format":"json"}"#).unwrap();
    assert!(matches!(reg.dispatch(call.clone(),&Deny).await,Err(DispatchError::Denied(_))));
    let result=reg.dispatch(call,&Allow).await.unwrap();
    let value=cat.decode_result(&allowed,"lu_artifact_inspect",&Ok(result)).unwrap();
    assert_eq!(value["ok"],true);
    assert_eq!(value["result"]["validation_level"],"json_syntax");
    assert_eq!(value["result"]["semantic_verified"],false);
    let empty=cat.encode_call(&allowed,"lu_artifact_inspect","artifact-2",None,
        r#"{"path":"empty.xlsx","format":"xlsx"}"#).unwrap();
    assert!(reg.dispatch(empty,&Allow).await.is_err());
    assert_eq!(fs::read(dir.join("record.json")).unwrap(),b"{\"written\":true}");
    fs::remove_dir_all(dir).unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn symlink_to_outside_workspace_is_refused(){
    use std::os::unix::fs::symlink;
    let dir=fixture();let other=fixture();
    fs::write(other.join("private.json"),b"{\"private\":true}").unwrap();
    symlink(other.join("private.json"),dir.join("alias.json")).unwrap();
    let reg=NativeToolRegistry::new();let mut cat=ModelToolCatalog::new();
    register_artifact_inspection(&reg,&mut cat,&dir).unwrap();
    let names=BTreeSet::from(["lu_artifact_inspect".to_owned()]);
    let call=cat.encode_call(&names,"lu_artifact_inspect","symlink-deny",None,
        r#"{"path":"alias.json","format":"json"}"#).unwrap();
    assert!(reg.dispatch(call,&Allow).await.is_err());
    fs::remove_dir_all(dir).unwrap();fs::remove_dir_all(other).unwrap();
}
