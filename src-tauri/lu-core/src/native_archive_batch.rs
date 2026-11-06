//! TOOLS-C-BATCH: multi-archive extraction within a NEW approved group folder.
//! Every archive tree is fully hash-reviewed BEFORE ANY filesystem mutation.
//! Group creation is deliberately non-atomic: a crash leaves an incomplete,
//! explicitly reconcilable folder, NEVER a false all-or-nothing success.
//! Original archives and existing destination folders are never modified.
use crate::native_archive_tree::{
    ArchiveCancellation,TreeExtractError,TreeManifest,TreeReceipt,
    preview_archive_tree,extract_archive_tree_new_cancellable,
    observe_tree_completion,verify_tree_manifest,
};
use cap_std::fs::{Dir,OpenOptions};
use sha2::{Digest,Sha256};
use std::{
    collections::BTreeSet,
    io::{Read,Write},
};
const MAX_ARCHIVES:usize=8;
const MAX_GROUP_BYTES:u64=512*1024*1024;
const MARKER:&str=".lu-batch-complete";

pub struct BatchArchiveSource<'a>{
    pub source_dir:&'a Dir,
    pub source_name:&'a str,
    pub target_folder:&'a str,
}
#[derive(Debug,Clone,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchArchiveEntry{
    pub source_name:String,
    pub target_folder:String,
    pub manifest:TreeManifest,
}
#[derive(Debug,Clone,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveBatchManifest{
    pub items:Vec<BatchArchiveEntry>,
    pub total_files:usize,
    pub total_bytes:u64,
    pub sha256:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ArchiveBatchReceipt{
    pub group_name:String,
    pub archive_count:usize,
    pub file_count:usize,
    pub total_bytes:u64,
    pub sha256:String,
}
fn flat(s:&str)->bool{
    !s.is_empty()&&s.len()<=120&&s!="."&&s!=".."
        &&!s.starts_with(".lu-")&&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn valid_digest(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn hash_plan(items:&[BatchArchiveEntry])->String{
    let mut hash=Sha256::new();
    hash.update(b"LU_ARCHIVE_BATCH_MANIFEST_V1");
    for item in items{
        for field in [
            item.source_name.as_bytes(),
            item.target_folder.as_bytes(),
            item.manifest.sha256.as_bytes()
        ]{
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field);
        }
        hash.update((item.manifest.file_count as u64).to_le_bytes());
        hash.update(item.manifest.total_bytes.to_le_bytes());
    }
    format!("{:x}",hash.finalize())
}
pub fn verify_archive_batch_plan(plan:&ArchiveBatchManifest)->bool{
    if plan.items.is_empty()||plan.items.len()>MAX_ARCHIVES||
        !valid_digest(&plan.sha256)||plan.total_bytes>MAX_GROUP_BYTES {
        return false;
    }
    let mut names=BTreeSet::new();
    let mut files=0usize;
    let mut bytes=0u64;
    for item in &plan.items{
        if !flat(&item.source_name)||!flat(&item.target_folder)
            ||!names.insert(item.target_folder.to_ascii_lowercase())
            ||!verify_tree_manifest(&item.manifest) {
            return false;
        }
        let Some(count)=files.checked_add(item.manifest.file_count)else{return false};
        let Some(total)=bytes.checked_add(item.manifest.total_bytes)else{return false};
        files=count;bytes=total;
    }
    files==plan.total_files&&bytes==plan.total_bytes
        &&hash_plan(&plan.items)==plan.sha256
}
pub fn preview_archive_batch(
    sources:&[BatchArchiveSource<'_>],cancel:&ArchiveCancellation
)->Result<ArchiveBatchManifest,TreeExtractError>{
    if sources.is_empty()||sources.len()>MAX_ARCHIVES{
        return Err(TreeExtractError::TooLarge);
    }
    let mut items=Vec::new();
    let mut names=BTreeSet::new();
    let mut total_files=0usize;
    let mut total_bytes=0u64;
    for source in sources {
        if cancel.is_cancelled(){return Err(TreeExtractError::Cancelled);}
        if !flat(source.source_name)||!flat(source.target_folder)
            ||!names.insert(source.target_folder.to_ascii_lowercase()){
            return Err(TreeExtractError::InvalidName);
        }
        let manifest=preview_archive_tree(source.source_dir,source.source_name)?;
        total_files=total_files.checked_add(manifest.file_count)
            .ok_or(TreeExtractError::TooLarge)?;
        total_bytes=total_bytes.checked_add(manifest.total_bytes)
            .ok_or(TreeExtractError::TooLarge)?;
        if total_bytes>MAX_GROUP_BYTES{return Err(TreeExtractError::TooLarge);}
        items.push(BatchArchiveEntry{
            source_name:source.source_name.into(),
            target_folder:source.target_folder.into(),
            manifest,
        });
    }
    let sha256=hash_plan(&items);
    let plan=ArchiveBatchManifest{items,total_files,total_bytes,sha256};
    if !verify_archive_batch_plan(&plan){return Err(TreeExtractError::InvalidArchive);}
    Ok(plan)
}
/// A successful GROUP receipt requires every archive tree to pass its own
/// exact hash observation and an exclusive group completion marker. Any
/// interruption after group creation becomes manual reconciliation, never an
/// automatic replay or deletion.
pub fn extract_archive_batch_new_cancellable(
    sources:&[BatchArchiveSource<'_>],plan:&ArchiveBatchManifest,
    destination:&Dir,new_group:&str,cancel:&ArchiveCancellation
)->Result<ArchiveBatchReceipt,TreeExtractError>{
    if !flat(new_group)||!verify_archive_batch_plan(plan)
        ||sources.len()!=plan.items.len(){
        return Err(TreeExtractError::InvalidName);
    }
    if cancel.is_cancelled(){return Err(TreeExtractError::Cancelled);}
    // Before the first mutation, independently recheck all source content.
    // Manifests remain bound to exact source basenames and ordered targets.
    for (source,approved) in sources.iter().zip(&plan.items){
        if source.source_name!=approved.source_name
            ||source.target_folder!=approved.target_folder{
            return Err(TreeExtractError::ManifestChanged);
        }
        let now=preview_archive_tree(source.source_dir,source.source_name)?;
        if now!=approved.manifest{return Err(TreeExtractError::ManifestChanged);}
    }
    if destination.symlink_metadata(new_group).is_ok(){
        return Err(TreeExtractError::TargetExists);
    }
    destination.create_dir(new_group).map_err(|e|
        if e.kind()==std::io::ErrorKind::AlreadyExists{
            TreeExtractError::TargetExists
        }else{TreeExtractError::Io(e.to_string())}
    )?;
    let operation=(||->Result<(),TreeExtractError>{
        let group=destination.open_dir(new_group)
            .map_err(|e|TreeExtractError::Io(e.to_string()))?;
        #[cfg(windows)]
        crate::windows_file_guard::reject_opened_directory_reparse(&group)
            .map_err(|_|TreeExtractError::UnsafeMember)?;
        for (source,approved) in sources.iter().zip(&plan.items){
            if cancel.is_cancelled(){return Err(TreeExtractError::Cancelled);}
            let TreeReceipt{manifest_sha256,..}=extract_archive_tree_new_cancellable(
                source.source_dir,source.source_name,&group,
                &approved.target_folder,&approved.manifest.sha256,cancel
            )?;
            if manifest_sha256!=approved.manifest.sha256
                ||!observe_tree_completion(
                    &group,&approved.target_folder,&approved.manifest
                )?{
                return Err(TreeExtractError::ManifestChanged);
            }
        }
        if cancel.is_cancelled(){return Err(TreeExtractError::Cancelled);}
        let mut opts=OpenOptions::new();
        opts.create_new(true).write(true);
        let mut marker=group.open_with(MARKER,&opts)
            .map_err(|e|TreeExtractError::Io(e.to_string()))?;
        marker.write_all(plan.sha256.as_bytes())
            .map_err(|e|TreeExtractError::Io(e.to_string()))?;
        marker.sync_all()
            .map_err(|e|TreeExtractError::Io(e.to_string()))?;
        Ok(())
    })();
    if operation.is_err(){return Err(TreeExtractError::PublishedNeedsReconciliation);}
    if !observe_archive_batch_completion(destination,new_group,plan)?{
        return Err(TreeExtractError::PublishedNeedsReconciliation);
    }
    Ok(ArchiveBatchReceipt{
        group_name:new_group.into(),archive_count:plan.items.len(),
        file_count:plan.total_files,total_bytes:plan.total_bytes,
        sha256:plan.sha256.clone()
    })
}
/// Independently recheck all published members. No marker-alone success.
pub fn observe_archive_batch_completion(
    destination:&Dir,group_name:&str,plan:&ArchiveBatchManifest
)->Result<bool,TreeExtractError>{
    if !flat(group_name)||!verify_archive_batch_plan(plan){
        return Err(TreeExtractError::InvalidName);
    }
    let group=match destination.open_dir(group_name){
        Ok(group)=>group,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(e)=>return Err(TreeExtractError::Io(e.to_string())),
    };
    #[cfg(windows)]
    if crate::windows_file_guard::reject_opened_directory_reparse(&group).is_err(){
        return Ok(false);
    }
    let mut marker=match group.open(MARKER){
        Ok(file)=>file,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(e)=>return Err(TreeExtractError::Io(e.to_string())),
    };
    let mut checksum=String::new();
    marker.take(65).read_to_string(&mut checksum)
        .map_err(|e|TreeExtractError::Io(e.to_string()))?;
    if checksum!=plan.sha256{return Ok(false);}
    let expected=plan.items.iter().map(|i|i.target_folder.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut found=BTreeSet::new();
    for node in group.read_dir(".").map_err(|e|TreeExtractError::Io(e.to_string()))?{
        let node=node.map_err(|e|TreeExtractError::Io(e.to_string()))?;
        let name=node.file_name().to_string_lossy().into_owned();
        if name==MARKER{continue;}
        if !expected.contains(&name.to_ascii_lowercase())
            ||!found.insert(name.to_ascii_lowercase()){
            return Ok(false);
        }
    }
    if found!=expected{return Ok(false);}
    for item in &plan.items{
        if !observe_tree_completion(&group,&item.target_folder,&item.manifest)?{
            return Ok(false);
        }
    }
    Ok(true)
}
