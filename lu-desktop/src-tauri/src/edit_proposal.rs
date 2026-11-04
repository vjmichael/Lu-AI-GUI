//! Inert completed-model proposals for editing an existing text file.
//! A model names its intended file and replacement bytes, never an actual OS
//! path, directory, user grant, file handle or approval. Only a native picker
//! may select and pin the real file. Full old/new bytes are shown to the user.
use cap_std::{ambient_authority, fs::Dir};
use lu_core::edit_safety::{bind_reviewed_edit_with_host_identity, plan_text_edit, ReviewedEdit,
    TextEditPlan};
#[cfg(windows)]
use lu_core::windows_file_guard::{inspect_open_file,WindowsFileIdentity};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, io::Read, path::Path, sync::{Arc,Mutex}};

const MAX_PROPOSALS: usize = 8;
pub const MAX_EDIT_PROPOSAL_BYTES: usize = 16 * 1024;
pub const MAX_PREVIEW_ORIGINAL_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProposedEdit {
    pub kind: String,
    pub file_name: String,
    pub purpose: String,
    pub content: String,
}
impl ProposedEdit {
    pub fn digest(&self) -> String {
        format!("{:x}",Sha256::digest(self.content.as_bytes()))
    }
    pub fn bytes(&self) -> usize { self.content.len() }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditProposalError {
    Invalid, Full, Duplicate, Missing, WrongConversation, NotSelected,
    Changed, Unavailable,
}
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct EditReviewView {
    pub exact_target: String,
    pub file_name: String,
    pub original: String,
    pub replacement: String,
    pub original_bytes: usize,
    pub replacement_bytes: usize,
    pub original_sha256: String,
    pub replacement_sha256: String,
    pub approval_sha256: String,
    pub risk: String,
}
#[derive(Clone)]
pub struct SelectedEdit {
    pub generation_id: String,
    pub proposal: ProposedEdit,
    pub directory: std::path::PathBuf,
    pub directory_cap: Arc<Dir>,
    pub original: Vec<u8>,
    pub plan: TextEditPlan,
    pub reviewed: ReviewedEdit,
    #[cfg(windows)]
    pub selected_file_identity: WindowsFileIdentity,
}
impl SelectedEdit {
    pub fn view(&self) -> EditReviewView {
        EditReviewView {
            exact_target: self.directory.join(&self.proposal.file_name)
                .to_string_lossy().into_owned(),
            file_name: self.proposal.file_name.clone(),
            original: String::from_utf8(self.original.clone())
                .expect("original verified UTF-8 during selection"),
            replacement: self.proposal.content.clone(),
            original_bytes: self.original.len(),
            replacement_bytes: self.proposal.bytes(),
            original_sha256: self.plan.expected_original_sha256.clone(),
            replacement_sha256: self.plan.replacement_sha256.clone(),
            approval_sha256: self.reviewed.approval_sha256.clone(),
            risk: format!("{:?}",self.plan.risk),
        }
    }
}
struct PendingEdit {
    chat_id: String,
    generation_id: String,
    proposal: ProposedEdit,
    selected: Option<SelectedEdit>,
}
#[derive(Default)]
pub struct EditProposalQueue { pending: Mutex<HashMap<String,PendingEdit>> }
fn valid_id(s:&str)->bool {
    !s.is_empty() && s.len()<=128 &&
        s.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_'||c==b'-')
}
fn valid_file(s:&str)->bool {
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."&&
        !s.starts_with(".lu-")&&!s.ends_with('.')&&!s.ends_with(' ')&&
        !s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
impl EditProposalQueue {
    pub fn stage(&self,id:&str,chat:&str,generation:&str,raw:&str)
        -> Result<ProposedEdit,EditProposalError> {
        if !valid_id(id)||!valid_id(chat)||!valid_id(generation)||
            raw.len()>MAX_EDIT_PROPOSAL_BYTES+4096 { return Err(EditProposalError::Invalid); }
        let proposal: ProposedEdit=serde_json::from_str(raw)
            .map_err(|_|EditProposalError::Invalid)?;
        if proposal.kind!="propose_edit_file"||!valid_file(&proposal.file_name)||
            proposal.content.len()>MAX_EDIT_PROPOSAL_BYTES||
            proposal.purpose.is_empty()||proposal.purpose.len()>256||
            proposal.purpose.chars().any(char::is_control) {
            return Err(EditProposalError::Invalid);
        }
        lu_core::write_format::identify_text_format(&proposal.file_name,&proposal.content)
            .map_err(|_|EditProposalError::Invalid)?;
        let mut pending=self.pending.lock().map_err(|_|EditProposalError::Unavailable)?;
        if pending.contains_key(id) {return Err(EditProposalError::Duplicate);}
        if pending.len()>=MAX_PROPOSALS {return Err(EditProposalError::Full);}
        pending.insert(id.into(),PendingEdit {
            chat_id:chat.into(),generation_id:generation.into(),
            proposal:proposal.clone(),selected:None
        });
        Ok(proposal)
    }
    /// Called with the path returned by a native OS file picker only.
    pub fn choose_file(&self,id:&str,chat:&str,picked:&Path)
        -> Result<SelectedEdit,EditProposalError> {
        // Reject wrong conversation/id before doing any path access.
        let (generation,proposal)={
            let pending=self.pending.lock().map_err(|_|EditProposalError::Unavailable)?;
            let entry=pending.get(id).ok_or(EditProposalError::Missing)?;
            if entry.chat_id!=chat {return Err(EditProposalError::WrongConversation);}
            (entry.generation_id.clone(),entry.proposal.clone())
        };
        if picked.file_name().and_then(|n|n.to_str())!=Some(proposal.file_name.as_str()) {
            return Err(EditProposalError::Invalid);
        }
        let parent=picked.parent().ok_or(EditProposalError::Invalid)?;
        let metadata=fs::symlink_metadata(parent).map_err(|_|EditProposalError::Invalid)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(EditProposalError::Invalid);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(EditProposalError::Invalid);
            }
        }
        let canonical=parent.canonicalize().map_err(|_|EditProposalError::Invalid)?;
        let exact=canonical.join(&proposal.file_name);
        // No-follow final-file identity check on Windows and Unix inode check.
        let mut file=crate::native_host_read::open_verified_selected_file(&exact)
            .map_err(|_|EditProposalError::Invalid)?;
        if file.metadata().map_err(|_|EditProposalError::Invalid)?.len() >
            MAX_PREVIEW_ORIGINAL_BYTES as u64 {return Err(EditProposalError::Invalid);}
        let mut original=Vec::new();
        (&mut file).take(MAX_PREVIEW_ORIGINAL_BYTES as u64+1)
            .read_to_end(&mut original).map_err(|_|EditProposalError::Invalid)?;
        if original.len()>MAX_PREVIEW_ORIGINAL_BYTES {
            return Err(EditProposalError::Invalid);
        }
        let original_text=std::str::from_utf8(&original)
            .map_err(|_|EditProposalError::Invalid)?;
        if original_text.contains('\0') {
            return Err(EditProposalError::Invalid);
        }
        let pinned=Arc::new(Dir::open_ambient_dir(&canonical,ambient_authority())
            .map_err(|_|EditProposalError::Invalid)?);
        #[cfg(windows)]
        let selected_file_identity={
            // The actual native-picked HANDLE is the authority source;
            // compare the entry opened relative to the pinned directory
            // before releasing the picker handle. Same bytes are not enough.
            let picked_identity=inspect_open_file(&file)
                .map_err(|_|EditProposalError::Invalid)?;
            let pinned_file=pinned.open(&proposal.file_name)
                .map_err(|_|EditProposalError::Changed)?.into_std();
            let current_identity=inspect_open_file(&pinned_file)
                .map_err(|_|EditProposalError::Changed)?;
            if picked_identity!=current_identity {
                return Err(EditProposalError::Changed);
            }
            picked_identity
        };
        let sha=format!("{:x}",Sha256::digest(&original));
        let plan=plan_text_edit(&proposal.file_name,&original,&sha,
            proposal.content.as_bytes()).map_err(|_|EditProposalError::Invalid)?;
        let reviewed=bind_reviewed_edit_with_host_identity(&plan,&proposal.file_name,
            &original,proposal.content.as_bytes(),
            {
                #[cfg(windows)]
                {Some(selected_file_identity.token())}
                #[cfg(not(windows))]
                {None}
            }.as_deref()
        ).map_err(|_|EditProposalError::Invalid)?;
        let selection=SelectedEdit {
            generation_id:generation,proposal, directory:canonical,
            directory_cap:pinned,original,plan,reviewed,
            #[cfg(windows)]
            selected_file_identity,
        };
        let mut pending=self.pending.lock().map_err(|_|EditProposalError::Unavailable)?;
        let item=pending.get_mut(id).ok_or(EditProposalError::Missing)?;
        if item.chat_id!=chat || item.generation_id!=selection.generation_id ||
            item.proposal!=selection.proposal {
            return Err(EditProposalError::Changed);
        }
        item.selected=Some(selection.clone());
        Ok(selection)
    }
    pub fn review_details(&self,id:&str,chat:&str)->Result<SelectedEdit,EditProposalError>{
        let pending=self.pending.lock().map_err(|_|EditProposalError::Unavailable)?;
        let item=pending.get(id).ok_or(EditProposalError::Missing)?;
        if item.chat_id!=chat {return Err(EditProposalError::WrongConversation);}
        item.selected.clone().ok_or(EditProposalError::NotSelected)
    }
    pub fn take_confirmed(&self,id:&str,chat:&str)->Result<SelectedEdit,EditProposalError>{
        let mut pending=self.pending.lock().map_err(|_|EditProposalError::Unavailable)?;
        let item=pending.get(id).ok_or(EditProposalError::Missing)?;
        if item.chat_id!=chat {return Err(EditProposalError::WrongConversation);}
        if item.selected.is_none() {return Err(EditProposalError::NotSelected);}
        pending.remove(id).and_then(|p|p.selected).ok_or(EditProposalError::NotSelected)
    }
    pub fn dismiss(&self,id:&str,chat:&str)->Result<(),EditProposalError>{
        let mut pending=self.pending.lock().map_err(|_|EditProposalError::Unavailable)?;
        let item=pending.get(id).ok_or(EditProposalError::Missing)?;
        if item.chat_id!=chat {return Err(EditProposalError::WrongConversation);}
        pending.remove(id);Ok(())
    }
}
