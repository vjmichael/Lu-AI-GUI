//! Native-picked, transient rollback review. A model or WebView supplies only
//! a review ID; neither can select the target nor replace the backup bytes.
use cap_std::fs::Dir;
use lu_core::{
    native_edit_existing::VerifiedRollbackPreview,
    rollback_approval_ledger::{RollbackDecision,RollbackRecord},
};
use std::{collections::HashMap,path::PathBuf,sync::{Arc,Mutex}};

pub const MAX_PENDING_ROLLBACKS:usize=8;
#[derive(Clone)]
pub struct SelectedRollback {
    pub record:RollbackRecord,
    pub directory:PathBuf,
    pub directory_cap:Arc<Dir>,
    pub preview:VerifiedRollbackPreview,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum RollbackReviewError { Invalid, Full, Duplicate, Missing, WrongChat, Poisoned }
#[derive(Default)]
pub struct RollbackReviewQueue { pending:Mutex<HashMap<String,SelectedRollback>> }
impl RollbackReviewQueue {
    pub fn stage(&self,selected:SelectedRollback)->Result<(),RollbackReviewError> {
        let r=&selected.record;
        if r.decision!=RollbackDecision::Reviewed||r.rollback_effect_id.is_some()||
            r.filename!=selected.preview.file_name||
            r.original_edit_id!=selected.preview.original_edit_id||
            r.expected_current_sha256!=selected.preview.replacement_sha256||
            r.restored_sha256!=selected.preview.original_sha256||
            r.restore_bytes!=selected.preview.restore_text.len() as u64||
            selected.directory.join(&r.filename).to_string_lossy()!=r.target||
            r.review_id.is_empty() {
            return Err(RollbackReviewError::Invalid);
        }
        let mut entries=self.pending.lock().map_err(|_|RollbackReviewError::Poisoned)?;
        if entries.contains_key(&r.review_id) {return Err(RollbackReviewError::Duplicate);}
        if entries.len()>=MAX_PENDING_ROLLBACKS {return Err(RollbackReviewError::Full);}
        entries.insert(r.review_id.clone(),selected);
        Ok(())
    }
    pub fn inspect(&self,review_id:&str,chat_id:&str)->Result<SelectedRollback,RollbackReviewError>{
        let entries=self.pending.lock().map_err(|_|RollbackReviewError::Poisoned)?;
        let item=entries.get(review_id).ok_or(RollbackReviewError::Missing)?;
        if item.record.chat_id!=chat_id {return Err(RollbackReviewError::WrongChat);}
        Ok(item.clone())
    }
    /// Caller must have recorded the independent native confirmation and
    /// durable exact approval. This consumes the selected capability once.
    pub fn take(&self,review_id:&str,chat_id:&str)->Result<SelectedRollback,RollbackReviewError>{
        let mut entries=self.pending.lock().map_err(|_|RollbackReviewError::Poisoned)?;
        let item=entries.get(review_id).ok_or(RollbackReviewError::Missing)?;
        if item.record.chat_id!=chat_id {return Err(RollbackReviewError::WrongChat);}
        entries.remove(review_id).ok_or(RollbackReviewError::Missing)
    }
    /// Reinspection invalidates earlier unspent previews of this edit.
    /// Caller must also deny their corresponding durable REVIEWED entries.
    pub fn clear_for(&self,chat_id:&str,original_proposal_id:&str)
        ->Result<Vec<String>,RollbackReviewError> {
        let mut entries=self.pending.lock().map_err(|_|RollbackReviewError::Poisoned)?;
        let old:Vec<String>=entries.iter()
            .filter(|(_,s)|s.record.chat_id==chat_id &&
                s.record.original_proposal_id==original_proposal_id)
            .map(|(key,_)|key.clone()).collect();
        for key in &old {entries.remove(key);}
        Ok(old)
    }
    pub fn dismiss(&self,review_id:&str,chat_id:&str)->Result<(),RollbackReviewError>{
        self.take(review_id,chat_id).map(|_|())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_review_never_grants_capability(){
        let q=RollbackReviewQueue::default();
        assert!(matches!(q.take("rb-not-selected","chat-one"),Err(RollbackReviewError::Missing)));
    }
}
