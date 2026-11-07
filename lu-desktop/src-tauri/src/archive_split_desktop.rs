//! P08 user-initiated multipart archive commands. No model-visible tool route,
//! no arbitrary WebView path, no overwrite and no implied format conversion.
use std::{path::PathBuf,sync::Arc};
use tauri::{AppHandle,State};
use tauri_plugin_dialog::DialogExt;
use serde::Serialize;
use lu_core::native_archive_split::{
    self,Plan,Receipt,MANIFEST,
};
use crate::{DesktopState,next_id};
use crate::archive_desktop::{physical_file,physical_folder};

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct P08ReviewView{
    source:String,output:String,bytes:u64,sha256:String,parts:usize,
    part_bytes:u64,manifest_sha256:String,
    warning:&'static str,
}
fn view(plan:&Plan)->Result<P08ReviewView,String>{
    let manifest=serde_json::to_vec_pretty(plan).map_err(|_|"Invalid multipart manifest")?;
    use sha2::{Digest,Sha256};
    Ok(P08ReviewView{
        source:plan.source.clone(),
        output:native_archive_split::folder(&plan.source)
            .map_err(|_|"Untrusted archive filename")?,
        bytes:plan.bytes,sha256:plan.sha256.clone(),
        parts:plan.chunks.len(),part_bytes:plan.chunk_size,
        manifest_sha256:format!("{:x}",Sha256::digest(&[manifest.as_slice(),b"\n"].concat())),
        warning:"Lu raw-byte part format, NOT native split ZIP/RAR/7z. Original remains unchanged; interrupted output needs manual reconciliation.",
    })
}
async fn approve(app:AppHandle,title:&'static str,preview:String)
    ->Result<bool,String>{
    tauri::async_runtime::spawn_blocking(move||
        app.dialog().message(preview).title(title)
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "Approve new-file operation".into(),"Cancel".into()
            )).blocking_show()
    ).await.map_err(|_|"Native archive approval dialog unavailable".into())
}
fn source_name(path:&std::path::Path)->Result<String,String>{
    path.file_name().and_then(|s|s.to_str())
        .map(str::to_owned).ok_or("Archive name cannot be decoded".into())
}
fn stage_token()->String{next_id("p08-")}
/// User picks one existing archive, host hashes the exact bytes and displays
/// a native approval. The selected archive's directory owns the new part folder.
#[tauri::command]
pub(crate) async fn lu_p08_split_archive(
    app:AppHandle,state:State<'_,Arc<DesktopState>>,
    chat_id:String,part_mib:u32
)->Result<Option<Receipt>,String>{
    state.chats.history(&chat_id)?;
    if !matches!(part_mib,4|8|16|32){
        return Err("Choose 4, 8, 16 or 32 MiB per part".into());
    }
    let picked=tauri::async_runtime::spawn_blocking({
        let app=app.clone();
        move||app.dialog().file().blocking_pick_file()
            .map(|p|p.into_path().map_err(|_|"Archive path is not local".to_string()))
            .transpose()
    }).await.map_err(|_|"Native archive file picker unavailable")??;
    let Some(path)=picked else{return Ok(None)};
    let (_,_,root)=physical_file(&path)?;
    let source=source_name(&path)?;
    let size=part_mib as u64*1024*1024;
    let plan=tauri::async_runtime::spawn_blocking({
        let root=Arc::clone(&root);let source=source.clone();
        move||native_archive_split::plan_split(&root,&source,size)
            .map_err(|e|format!("Multipart source rejected: {e:?}"))
    }).await.map_err(|_|"Multipart planning failed")??;
    let reviewed=view(&plan)?;
    if !approve(app,"Lu — Confirm exact archive-byte split",format!(
        "Split this EXACT source into a NEW Lu multipart folder?\n\n\
         Source: {}\nOutput: {}\nOriginal bytes: {}\nOriginal SHA-256: {}\n\
         Parts: {} of at most {} bytes\nManifest SHA-256: {}\n\n\
         {}\n\nOn crash, staging is NOT automatically replayed.",
        reviewed.source,reviewed.output,reviewed.bytes,reviewed.sha256,
        reviewed.parts,reviewed.part_bytes,reviewed.manifest_sha256,
        reviewed.warning
    )).await?{return Ok(None);}
    let owner=Arc::clone(state.inner());
    let task=next_id("archive-split-");
    owner.background_tasks.create(&task,Some(&chat_id),None)
        .map_err(|_|"Multipart task registration refused")?;
    owner.background_tasks.start(&task).map_err(|_|"Multipart task start refused")?;
    let token=stage_token();
    let result=tauri::async_runtime::spawn_blocking(move||
        native_archive_split::split(&root,&plan,&token)
            .map_err(|e|format!("Archive split failed or requires reconciliation: {e:?}"))
    ).await.map_err(|_|"Multipart worker unavailable".to_owned()).and_then(|v|v);
    match result{
        Ok(receipt)=>{
            owner.background_tasks.complete(&task)
                .map_err(|_|"Multipart completion journal failed")?;
            Ok(Some(receipt))
        },
        Err(e)=>{
            let _=owner.background_tasks.fail(&task);
            Err(e)
        }
    }
}
/// Explicit restart recovery of a previous split stage: the human picks the
/// stage FOLDER and reviews the old plan again. The source bytes and all
/// staged parts are rehashed by the native core before any publication.
#[tauri::command]
pub(crate) async fn lu_p08_resume_split(
    app:AppHandle,state:State<'_,Arc<DesktopState>>,chat_id:String
)->Result<Option<Receipt>,String>{
    state.chats.history(&chat_id)?;
    let picked=tauri::async_runtime::spawn_blocking({
        let app=app.clone();
        move||app.dialog().file().blocking_pick_folder()
            .map(|p|p.into_path().map_err(|_|"Stage folder is not local".to_string()))
            .transpose()
    }).await.map_err(|_|"Native stage folder picker unavailable")??;
    let Some(path)=picked else{return Ok(None)};
    let stage=source_name(&path)?;
    let token=stage.strip_prefix(".lu-p08-split-")
        .ok_or("Not a Lu P08 split staging folder")?.to_owned();
    let parent=path.parent().ok_or("Stage folder has no parent")?;
    let root=physical_folder(parent)?;
    let metadata=root.symlink_metadata(&stage)
        .map_err(|_|"Staging folder unavailable")?;
    if !metadata.is_dir()||metadata.file_type().is_symlink(){
        return Err("Stage folder must be a regular directory".into());
    }
    let plan=native_archive_split::read_plan(&root.open_dir(&stage)
        .map_err(|_|"Stage plan cannot be read")?)
        .map_err(|_|"Stage manifest cannot be independently validated")?;
    let reviewed=view(&plan)?;
    if !approve(app,"Lu — Explicit resume of reviewed multipart staging",format!(
        "RESUME only if the original source and existing staged parts still match.\n\n\
         Source: {}\nOriginal SHA-256: {}\nParts: {}\n\
         Stage: {}\nDestination: {}\n\n\
         A different source or corrupt intermediate will be refused. \
         An already published/incomplete output must be reconciled manually.",
        reviewed.source,reviewed.sha256,reviewed.parts,stage,reviewed.output
    )).await?{return Ok(None);}
    let owner=Arc::clone(state.inner());
    let task=next_id("archive-resume-");
    owner.background_tasks.create(&task,Some(&chat_id),None)
        .map_err(|_|"Archive resume task could not be registered")?;
    owner.background_tasks.start(&task)
        .map_err(|_|"Archive resume task could not start")?;
    let result=tauri::async_runtime::spawn_blocking(move||
        native_archive_split::split(&root,&plan,&token)
            .map_err(|e|format!("Multipart resume denied or uncertain: {e:?}"))
    ).await.map_err(|_|"Native archive resume worker unavailable".to_owned()).and_then(|v|v);
    match result{
        Ok(value)=>{
            owner.background_tasks.complete(&task)
                .map_err(|_|"Archive resume completion not durable")?;
            Ok(Some(value))
        },
        Err(e)=>{let _=owner.background_tasks.fail(&task);Err(e)}
    }
}
/// Manifest is picked through the actual OS picker; output folder separately
/// picked through OS UI. No path from the WebView or model is accepted.
#[tauri::command]
pub(crate) async fn lu_p08_reassemble_archive(
    app:AppHandle,state:State<'_,Arc<DesktopState>>,chat_id:String
)->Result<Option<Receipt>,String>{
    state.chats.history(&chat_id)?;
    let picked=tauri::async_runtime::spawn_blocking({
        let app=app.clone();
        move||{
            let p=app.dialog().file().blocking_pick_file()
                .map(|p|p.into_path().map_err(|_|"Manifest is not a local file".to_string()))
                .transpose()?;
            let Some(manifest)=p else{return Ok(None)};
            let dest=app.dialog().file().blocking_pick_folder()
                .map(|p|p.into_path().map_err(|_|"Output folder is not local".to_string()))
                .transpose()?;
            Ok::<_,String>(dest.map(|folder|(manifest,folder)))
        }
    }).await.map_err(|_|"Native multipart manifest/folder picker unavailable")??;
    let Some((manifest,target))=picked else{return Ok(None)};
    if source_name(&manifest)?!=MANIFEST{
        return Err("Pick an actual Lu multipart manifest (lu-parts.json)".into());
    }
    let group=manifest.parent().ok_or("Multipart folder unavailable")?;
    let folder=source_name(group)?;
    let source_parent=group.parent().ok_or("Multipart source parent unavailable")?;
    let source_root=physical_folder(source_parent)?;
    let destination_root=physical_folder(&target)?;
    let plan=tauri::async_runtime::spawn_blocking({
        let source_root=Arc::clone(&source_root);
        let folder=folder.clone();
        move||native_archive_split::inspect(&source_root,&folder)
            .map_err(|e|format!("Multipart manifest/parts refused: {e:?}"))
    }).await.map_err(|_|"Multipart preflight worker unavailable")??;
    let reviewed=view(&plan)?;
    if !approve(app,"Lu — Confirm reassembly of exact archive bytes",format!(
        "Reconstruct the EXACT original archive into a NEW file?\n\n\
         Source group: {}\nDestination folder: {}\nNew filename: {}\n\
         Original bytes: {}\nOriginal SHA-256: {}\nPart count: {}\n\
         Manifest SHA-256: {}\n\n\
         Every part is verified twice. Existing files are NEVER overwritten.",
        folder,target.display(),reviewed.source,reviewed.bytes,
        reviewed.sha256,reviewed.parts,reviewed.manifest_sha256
    )).await?{return Ok(None);}
    let owner=Arc::clone(state.inner());
    let task=next_id("archive-join-");
    owner.background_tasks.create(&task,Some(&chat_id),None)
        .map_err(|_|"Archive join task registration refused")?;
    owner.background_tasks.start(&task)
        .map_err(|_|"Archive join task start refused")?;
    let token=stage_token();
    let result=tauri::async_runtime::spawn_blocking(move||
        native_archive_split::reassemble(
            &source_root,&destination_root,&folder,&token
        ).map_err(|e|format!("Archive join failed or needs manual reconciliation: {e:?}"))
    ).await.map_err(|_|"Archive join worker unavailable".to_owned()).and_then(|v|v);
    match result{
        Ok(v)=>{
            owner.background_tasks.complete(&task)
                .map_err(|_|"Archive join completion not journaled")?;
            Ok(Some(v))
        },
        Err(e)=>{let _=owner.background_tasks.fail(&task);Err(e)}
    }
}
