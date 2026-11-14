//! Human-reviewed group extraction of 1–8 *separately OS-picked* archives.
//! Each item is pinned to a capability and its complete tree manifest. No
//! model path, WebView-chosen archive or unreviewed member is executable.
use std::{collections::HashMap,path::{Path,PathBuf},sync::{Arc,Mutex}};
use cap_std::fs::Dir;
use serde::Serialize;
use lu_core::native_archive_batch::{
    self,ArchiveBatchManifest,BatchArchiveSource,ArchiveBatchReceipt,
};
use lu_core::native_archive_tree::{ArchiveCancellation,TreeManifestEntry};
use crate::archive_desktop::{
    physical_file,physical_folder,base_for_folder,
};

const MAX_REVIEWS:usize=4;
#[derive(Clone)]
struct ArchiveSource{
    name:String,
    display:String,
    target:String,
    root:Arc<Dir>,
}
#[derive(Clone)]
pub(crate) struct PinnedBatchReview{
    chat:String,
    id:String,
    sources:Vec<ArchiveSource>,
    output:Arc<Dir>,
    output_display:PathBuf,
    plan:ArchiveBatchManifest,
    group_name:String,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ArchiveBatchItemView{
    pub source:String,
    pub output_folder:String,
    pub files:usize,
    pub expanded_bytes:u64,
    pub manifest_sha256:String,
    pub entries:Vec<TreeManifestEntry>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ArchiveBatchView{
    pub review_id:String,
    pub chat_id:String,
    pub group_display:String,
    pub group_name:String,
    pub archive_count:usize,
    pub total_files:usize,
    pub total_bytes:u64,
    pub manifest_sha256:String,
    pub items:Vec<ArchiveBatchItemView>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ArchiveBatchResultView{
    pub group_name:String,
    pub archive_count:usize,
    pub total_files:usize,
    pub total_bytes:u64,
    pub manifest_sha256:String,
    pub effect_call_id:String,
    pub source_archives_preserved:bool,
}
impl PinnedBatchReview{
    pub fn view(&self)->ArchiveBatchView{
        ArchiveBatchView{
            review_id:self.id.clone(),chat_id:self.chat.clone(),
            group_display:self.output_display.join(&self.group_name)
                .to_string_lossy().into_owned(),
            group_name:self.group_name.clone(),
            archive_count:self.plan.items.len(),
            total_files:self.plan.total_files,total_bytes:self.plan.total_bytes,
            manifest_sha256:self.plan.sha256.clone(),
            items:self.sources.iter().zip(self.plan.items.iter()).map(|(source,item)|
                ArchiveBatchItemView{
                    source:source.display.clone(),
                    output_folder:item.target_folder.clone(),
                    files:item.manifest.file_count,
                    expanded_bytes:item.manifest.total_bytes,
                    manifest_sha256:item.manifest.sha256.clone(),
                    entries:item.manifest.entries.clone(),
                }).collect(),
        }
    }
    pub fn matches(&self,other:&Self)->bool{
        self.chat==other.chat&&self.id==other.id
            &&self.group_name==other.group_name&&self.plan==other.plan
            &&self.sources.len()==other.sources.len()
            &&Arc::ptr_eq(&self.output,&other.output)
            &&self.sources.iter().zip(other.sources.iter()).all(|(a,b)|
                a.name==b.name&&a.target==b.target
                &&Arc::ptr_eq(&a.root,&b.root)
            )
    }
    pub fn plan(&self)->&ArchiveBatchManifest{&self.plan}
    pub fn group_name(&self)->&str{&self.group_name}
    pub fn destinations(&self)->(&Arc<Dir>,&Path){(&self.output,&self.output_display)}
    pub fn execute(&self,cancel:&ArchiveCancellation)
        ->Result<ArchiveBatchReceipt,String>{
        let inputs=self.sources.iter().map(|s|BatchArchiveSource{
            source_dir:s.root.as_ref(),source_name:&s.name,
            target_folder:&s.target,
        }).collect::<Vec<_>>();
        native_archive_batch::extract_archive_batch_new_cancellable(
            &inputs,&self.plan,&self.output,&self.group_name,cancel
        ).map_err(|e|format!("Archive batch failed or needs manual reconciliation: {e:?}"))
    }
}
#[derive(Default)]
pub(crate) struct ArchiveBatchQueue{
    pending:Mutex<HashMap<String,PinnedBatchReview>>,
}
impl ArchiveBatchQueue{
    /// Source paths are ONLY returned by native OS multi-selection; the
    /// destination is ONLY a native OS folder selection. No string paths from
    /// the WebView are accepted.
    pub fn stage(
        &self,chat:&str,id:&str,source_paths:&[PathBuf],
        destination:&Path,cancel:&ArchiveCancellation
    )->Result<ArchiveBatchView,String>{
        if !chat.starts_with("c-")||chat.len()>128
            ||!id.starts_with("ab-")||id.len()>128
            ||source_paths.is_empty()||source_paths.len()>8{
            return Err("Batch requires 1–8 native-picked archives".into());
        }
        let output=physical_folder(destination)?;
        let mut sources=Vec::new();
        for (index,path) in source_paths.iter().enumerate(){
            if cancel.is_cancelled(){return Err("Archive batch preview cancelled".into());}
            let (name,_,root)=physical_file(path)?;
            let target=format!("{:02}-{}",index+1,base_for_folder(&name));
            sources.push(ArchiveSource{
                name,display:path.to_string_lossy().into_owned(),
                target,root,
            });
        }
        let inputs=sources.iter().map(|s|BatchArchiveSource{
            source_dir:s.root.as_ref(),source_name:&s.name,
            target_folder:&s.target,
        }).collect::<Vec<_>>();
        let plan=native_archive_batch::preview_archive_batch(&inputs,cancel)
            .map_err(|e|format!("Archive batch preflight refused: {e:?}"))?;
        let group_name=format!("Lu-Batch-{}",id);
        if output.symlink_metadata(&group_name).is_ok(){
            return Err("The approved batch group output already exists".into());
        }
        let item=PinnedBatchReview{
            chat:chat.into(),id:id.into(),sources,
            output,output_display:destination.to_path_buf(),
            plan,group_name,
        };
        let view=item.view();
        let mut pending=self.pending.lock()
            .map_err(|_|"Archive batch review queue unavailable")?;
        if pending.len()>=MAX_REVIEWS||pending.contains_key(id){
            return Err("Archive batch review queue full or review already exists".into());
        }
        pending.insert(id.into(),item);
        Ok(view)
    }
    pub fn inspect(&self,chat:&str,id:&str)->Result<PinnedBatchReview,String>{
        let queue=self.pending.lock().map_err(|_|"Archive batch queue unavailable")?;
        let item=queue.get(id).ok_or("Archive batch review expired")?;
        if item.chat!=chat{return Err("Archive batch belongs to another chat".into());}
        Ok(item.clone())
    }
    pub fn consume(&self,chat:&str,id:&str)->Result<PinnedBatchReview,String>{
        let mut queue=self.pending.lock()
            .map_err(|_|"Archive batch queue unavailable")?;
        if queue.get(id).is_none_or(|r|r.chat!=chat){
            return Err("Archive batch approval expired".into());
        }
        queue.remove(id).ok_or("Batch approval already consumed".into())
    }
    pub fn deny(&self,chat:&str,id:&str)->Result<(),String>{
        self.consume(chat,id).map(|_|())
    }
}
