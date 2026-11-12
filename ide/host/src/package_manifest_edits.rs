//! FIN-05B: exact saved-source vcpkg.json declaration edits.
//! No resolver, tool launch, source file write or package installation here.
//! Mutation must use a separately confirmed native Tauri source revision.
use crate::{IdeWorkspaceRegistry,package_preview::{PackageOperation,PackagePreviewRequest}};
use serde::Serialize;
use serde_json::{Value,json};
use sha2::{Sha256,Digest};

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct VcpkgManifestEdit {
    pub schema_version:u32,
    pub project_id:String,
    pub relative_path:&'static str,
    pub expected_revision:String,
    pub proposed_revision:String,
    pub proposed_text:String,
    pub preview_sha256:String,
    pub evidence_fingerprint:String,
    pub operation:&'static str,
    pub dependency_name:String,
    pub constraint_semantics:&'static str,
    pub review:crate::review::ChangeReview,
    pub source_rechecked:bool,
    pub process_started:bool,
    pub network_accessed:bool,
    pub files_modified:bool,
    pub execution_authorized:bool,
    pub lockfile_verified:bool,
    pub packages_installed:bool,
}
fn valid_name(name:&str)->bool {
    !name.is_empty() && name.len()<=64 &&
    name.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'-') &&
    !name.starts_with('-') && !name.ends_with('-')
}
fn make_proposed_manifest(source:&str,request:&PackagePreviewRequest)
    ->Result<String,&'static str>{
    let mut doc:Value=serde_json::from_str(source)
        .map_err(|_|"FIN05_VCPKG_JSON_INVALID")?;
    let object=doc.as_object_mut().ok_or("FIN05_VCPKG_ROOT_NOT_OBJECT")?;
    // A declaration edit is not a resolver: malformed special fields are
    // never silently normalized into an apparently valid package plan.
    if object.get("overrides").is_some_and(|v|!v.is_array()) ||
       object.get("builtin-baseline").is_some_and(|v|!v.is_string()) {
        return Err("FIN05_VCPKG_UNSUPPORTED_ROOT_SHAPE");
    }
    let name=request.package_name.as_deref()
        .filter(|s|valid_name(s)).ok_or("FIN05_VCPKG_DEPENDENCY_INVALID")?;
    // Compute before borrowing dependencies mutably: a concurrent manifest
    // edit still fails the later source-revision and review rechecks.
    let baseline_valid=object.get("builtin-baseline").is_some_and(|v|
        v.as_str().is_some_and(|s|s.len()==40 &&
            s.bytes().all(|b|b.is_ascii_hexdigit())));
    if !object.contains_key("dependencies"){
        object.insert("dependencies".into(),json!([]));
    }
    let deps=object.get_mut("dependencies").and_then(Value::as_array_mut)
        .ok_or("FIN05_VCPKG_DEPENDENCIES_UNSUPPORTED")?;
    if deps.len()>256 {return Err("FIN05_VCPKG_DEPENDENCY_BUDGET");}
    let mut match_index=None;
    for (i,entry) in deps.iter().enumerate(){
        let current=entry.as_str().or_else(||
            entry.as_object().and_then(|o|o.get("name")).and_then(Value::as_str))
            .ok_or("FIN05_VCPKG_DEPENDENCY_SHAPE_UNKNOWN")?;
        if current==name{
            if match_index.is_some(){return Err("FIN05_VCPKG_DUPLICATE_DEPENDENCY");}
            match_index=Some(i);
        }
    }
    match request.operation {
        PackageOperation::Add=>{
            if match_index.is_some(){return Err("FIN05_VCPKG_ALREADY_DECLARED");}
            let version=request.exact_version.as_deref()
                .filter(|s| !s.is_empty() && s.len()<=64 &&
                    s.bytes().all(|b|b.is_ascii_alphanumeric() ||
                        matches!(b,b'.'|b'-'|b'_')))
                .ok_or("FIN05_VCPKG_VERSION_UNSUPPORTED")?;
            // vcpkg's version>= is a LOWER BOUND, not an exact version pin.
            // It requires an existing baseline. No lock resolution implied.
            if !baseline_valid {
                return Err("FIN05_VCPKG_BASELINE_REQUIRED_FOR_VERSION");
            }
            deps.push(json!({"name":name,"version>=":version}));
        },
        PackageOperation::Remove=>{
            let pos=match_index.ok_or("FIN05_VCPKG_DEPENDENCY_NOT_FOUND")?;
            deps.remove(pos);
        },
        PackageOperation::Sync=>return Err("FIN05_VCPKG_SYNC_NOT_MANIFEST_EDIT"),
    }
    let mut proposed=serde_json::to_string_pretty(&doc)
        .map_err(|_|"FIN05_VCPKG_JSON_WRITE_DENIED")?;
    proposed.push('\n');
    if proposed.len()>256*1024{return Err("FIN05_VCPKG_EDIT_TOO_LARGE");}
    Ok(proposed)
}
impl IdeWorkspaceRegistry {
    pub fn prepare_exact_vcpkg_edit(
        &self,project:&str,request:PackagePreviewRequest,
        sha:&str,fingerprint:&str,
    )->Result<VcpkgManifestEdit,&'static str>{
        if request.manager_id!="vcpkg" ||
            !matches!(request.operation,PackageOperation::Add|PackageOperation::Remove) {
            return Err("FIN05_ONLY_VCPKG_DIRECT_DECLARATION_SUPPORTED");
        }
        let matched=self.revalidate_package_operation(
            project,request.clone(),sha,fingerprint)?;
        if !matched.matched_fresh_native_evidence ||
            matched.authorization_issued || matched.process_started ||
            matched.files_modified {
            return Err("FIN05_NATIVE_SOURCE_REVALIDATION_DENIED");
        }
        let source=self.preview_source_file(project,"vcpkg.json")
            .map_err(|_|"FIN05_VCPKG_SOURCE_UNAVAILABLE")?;
        if source.project_id!=project || source.relative_path!="vcpkg.json" ||
            source.byte_length>128*1024 {
            return Err("FIN05_VCPKG_SOURCE_SCOPE_DENIED");
        }
        let proposed=make_proposed_manifest(&source.text,&request)?;
        let review=self.review_source_change(
            project,"vcpkg.json",&source.revision,&proposed)
            .map_err(|_|"FIN05_VCPKG_DIFF_REVIEW_DENIED")?;
        if review.truncated || review.unchanged ||
            review.expected_revision!=source.revision ||
            review.hunks.is_empty() {
            return Err("FIN05_VCPKG_REVIEW_INCOMPLETE");
        }
        let proposed_revision=format!("{:x}",Sha256::digest(proposed.as_bytes()));
        if review.proposed_revision!=proposed_revision{
            return Err("FIN05_VCPKG_EDIT_SHA_MISMATCH");
        }
        Ok(VcpkgManifestEdit{
            schema_version:1,project_id:project.to_owned(),
            relative_path:"vcpkg.json",
            expected_revision:source.revision,
            proposed_revision,proposed_text:proposed,
            preview_sha256:matched.preview_sha256,
            evidence_fingerprint:matched.evidence_fingerprint,
            operation:match request.operation {
                PackageOperation::Add=>"add",
                PackageOperation::Remove=>"remove",
                _=>unreachable!(),
            },
            dependency_name:request.package_name.unwrap_or_default(),
            constraint_semantics:"vcpkg_version_gte_is_minimum_not_exact",
            review,source_rechecked:true,process_started:false,
            network_accessed:false,files_modified:false,
            execution_authorized:false,lockfile_verified:false,
            packages_installed:false,
        })
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn remove_exact_direct_dependency_without_touching_other_entries(){
        let request=PackagePreviewRequest{
            manager_id:"vcpkg".into(),operation:PackageOperation::Remove,
            package_name:Some("fmt".into()),exact_version:None,
        };
        let src=r#"{"name":"demo","dependencies":["fmt","zlib"]}"#;
        let v:Value=serde_json::from_str(
            &make_proposed_manifest(src,&request).unwrap()).unwrap();
        assert_eq!(v["dependencies"],json!(["zlib"]));
    }
    #[test]fn invalid_version_or_unbaselined_add_is_denied(){
        let request=PackagePreviewRequest{
            manager_id:"vcpkg".into(),operation:PackageOperation::Add,
            package_name:Some("fmt".into()),exact_version:Some("1.2.3".into()),
        };
        assert!(make_proposed_manifest(r#"{"dependencies":[]}"#,&request).is_err());
        assert!(make_proposed_manifest(r#"{"dependencies":[],"builtin-baseline":"0000000000000000000000000000000000000000"}"#,
            &request).is_ok());
    }
}
