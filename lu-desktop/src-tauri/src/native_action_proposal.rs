//! Wave 3: completed-model intent -> inert, chat-bound human review handoff.
//! Model output can request a review card, never an effect or a grant.
//! The trusted host independently classifies the authenticated current USER
//! prompt before staging the exact model proposal. No paths are accepted.
use serde::{Deserialize,Serialize};
use std::{collections::HashMap,sync::Mutex};
const MAX_BYTES:usize=12*1024;
const MAX_PENDING:usize=16;
#[derive(Clone,Copy,Debug,Deserialize,Serialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub enum NativeAction{
    CopyFile,MoveFile,TrashFile,ExtractArchive,ExtractArchiveBatch,SplitArchive,
    CreateDocument,ReviseOffice,BatchOffice,
}
impl NativeAction{
    pub fn as_str(self)->&'static str{
        match self{
            Self::CopyFile=>"copy_file",Self::MoveFile=>"move_file",
            Self::TrashFile=>"trash_file",Self::ExtractArchive=>"extract_archive",
            Self::ExtractArchiveBatch=>"extract_archive_batch",
            Self::SplitArchive=>"split_archive",
            Self::CreateDocument=>"create_document",
            Self::ReviseOffice=>"revise_office",Self::BatchOffice=>"batch_office",
        }
    }
}
/// A safe, conservative host hint. It neither creates a grant nor directs a
/// filesystem operation. Ambiguous/negated/explanatory requests are not routed.
pub fn classify_current_user_action(input:&str)->Option<NativeAction>{
    let lower=input.trim().to_ascii_lowercase();
    if lower.is_empty()||lower.len()>8192||
        lower.contains("[attached text file:")||
        lower.contains("[/attached text file]")||
        ["do not ","don't ","never ","without ","how do i ","how to ",
         "what is ","what does ","explain ","can you explain "]
        .iter().any(|s|lower.starts_with(s))||
        ["don't copy","do not copy","don't move","do not move",
         "don't delete","do not delete","don't trash","do not trash",
         "don't extract","do not extract","don't split","do not split","don't revise","do not revise",
         "don't create","do not create"]
        .iter().any(|s|lower.contains(s)){
        return None
    }
    let mentions_file=lower.contains("file")||lower.contains(".txt")||
        lower.contains(".md")||lower.contains("document");
    let mut candidates=Vec::new();
    if mentions_file && ["copy ","duplicate "].iter().any(|v|lower.contains(v)){
        candidates.push(NativeAction::CopyFile)
    }
    if mentions_file && ["move ","rename "].iter().any(|v|lower.contains(v)){
        candidates.push(NativeAction::MoveFile)
    }
    if mentions_file && ["trash ","remove ","delete "].iter().any(|v|lower.contains(v)){
        candidates.push(NativeAction::TrashFile)
    }
    let archive_request=(lower.contains("extract ")||lower.contains("unpack ")) &&
        ["archive","zip",".7z",".rar",".tar",".rpm",".deb"].iter().any(|v|lower.contains(v));
    if archive_request{
        candidates.push(if lower.contains("batch")||lower.contains("multiple archives")||
            lower.contains("several archives"){
            NativeAction::ExtractArchiveBatch
        }else{NativeAction::ExtractArchive})
    }
    if ["split ","divide "].iter().any(|v|lower.contains(v)) &&
        ["archive","zip","large file","multipart"].iter().any(|v|lower.contains(v)){
        candidates.push(NativeAction::SplitArchive)
    }
    if ["create ","make ","draft "].iter().any(|v|lower.contains(v)) &&
        ["word document","powerpoint","docx","pptx"].iter().any(|v|lower.contains(v)){
        candidates.push(NativeAction::CreateDocument)
    }
    if ["revise ","edit ","modify "].iter().any(|v|lower.contains(v)) &&
        ["office","word document","powerpoint","spreadsheet","excel","docx","pptx","xlsx"]
        .iter().any(|v|lower.contains(v)){
        candidates.push(NativeAction::ReviseOffice)
    }
    if lower.contains("batch") && lower.contains("office") &&
        (lower.contains("edit")||lower.contains("revise")){
        candidates.retain(|c|*c!=NativeAction::ReviseOffice);
        candidates.push(NativeAction::BatchOffice)
    }
    if candidates.len()==1{Some(candidates[0])}else{None}
}
// Structured model-authored content is preview data only. The host will
// independently reopen the native-picked resource and compare original bytes
// before any user-approved, nonce-bound publication. No paths/targets allowed.
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeDocumentDraft{
    pub format:String,pub title:String,pub content:String,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeOfficePatch{
    pub kind:String,
    pub text_index:Option<usize>,
    pub slide_number:Option<usize>,
    pub sheet_number:Option<usize>,
    pub address:Option<String>,
    pub expected_old:String,
    pub new_value:String,
}
fn valid_patch(p:&NativeOfficePatch)->bool{
    ["word_text","slide_text","cell_number","cell_boolean","cell_text"]
        .contains(&p.kind.as_str())
        && p.expected_old.len()<=1024 && p.new_value.len()<=1024
        && !p.new_value.chars().any(|ch|ch.is_control()
            &&ch!=char::from(10)&&ch!=char::from(9))
        && p.address.as_deref().is_none_or(|v|
            v.len()<=16 && !v.is_empty()
            && v.bytes().all(|b|b.is_ascii_alphanumeric()||b==36))
        && p.text_index.is_none_or(|v|v<=100_000)
        && p.slide_number.is_none_or(|v|(1..=100_000).contains(&v))
        && p.sheet_number.is_none_or(|v|(1..=100_000).contains(&v))
}
#[derive(Clone,Debug,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ModelActionProposal{
    kind:String,action:NativeAction,purpose:String,
    #[serde(default)]document_draft:Option<NativeDocumentDraft>,
    #[serde(default)]office_patch:Option<NativeOfficePatch>,
    #[serde(default)]office_batch:Option<Vec<NativeOfficePatch>>,
}
#[derive(Clone,Debug,Serialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct ActionProposalView{
    pub chat_id:String,pub proposal_id:String,
    pub action:String,pub purpose:String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub document_draft:Option<NativeDocumentDraft>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub office_patch:Option<NativeOfficePatch>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub office_batch:Option<Vec<NativeOfficePatch>>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ProposalError{
    Invalid,ActionMismatch,Full,Duplicate,Missing,WrongConversation,Unavailable,
}
struct Pending{
    chat_id:String,origin_generation_id:String,view:ActionProposalView,
}
#[derive(Default)]
pub struct ActionProposalQueue{pending:Mutex<HashMap<String,Pending>>}
fn valid_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|c|
        c.is_ascii_alphanumeric()||matches!(c,b'-'|b'_'))
}
impl ActionProposalQueue{
    /// May be called only by the trusted host immediately after completion
    /// of its own model generation and persisted assistant journal checkpoint.
    pub fn stage(&self,id:&str,chat:&str,generation:&str,
        current_user_text:&str,model_output:&str)
        ->Result<ActionProposalView,ProposalError>{
        if !valid_id(id)||!valid_id(chat)||!valid_id(generation)||
            model_output.len()>MAX_BYTES{return Err(ProposalError::Invalid)}
        let expected=classify_current_user_action(current_user_text)
            .ok_or(ProposalError::ActionMismatch)?;
        let object=lu_core::model_json_boundary::parse_exact_boundary_object(
            model_output,MAX_BYTES).map_err(|_|ProposalError::Invalid)?;
        let p:ModelActionProposal=serde_json::from_value(
            serde_json::Value::Object(object)).map_err(|_|ProposalError::Invalid)?;
        if p.kind!="propose_native_action"||p.action!=expected||
            p.purpose.trim().is_empty()||p.purpose.len()>240||
            p.purpose.chars().any(char::is_control){
            return Err(ProposalError::Invalid)
        }
        if let Some(ref doc)=p.document_draft{
            if p.action!=NativeAction::CreateDocument||
                !matches!(doc.format.as_str(),"docx"|"pptx")||
                doc.title.trim().is_empty()||doc.title.len()>160||
                doc.content.trim().is_empty()||doc.content.len()>8192||
                doc.title.chars().any(char::is_control)||
                doc.content.chars().any(|ch|ch.is_control()&&ch!='\n'&&ch!='\t'&&ch!='\r'){
                return Err(ProposalError::Invalid)
            }
        }
        if let Some(ref patch)=p.office_patch{
            if p.action!=NativeAction::ReviseOffice||!valid_patch(patch){
                return Err(ProposalError::Invalid)
            }
        }
        if let Some(ref batch)=p.office_batch{
            if p.action!=NativeAction::BatchOffice||
                batch.len()<2||batch.len()>8||batch.iter().any(|p|!valid_patch(p)){
                return Err(ProposalError::Invalid)
            }
        }
        let view=ActionProposalView{
            chat_id:chat.into(),proposal_id:id.into(),
            action:p.action.as_str().into(),purpose:p.purpose,
            document_draft:p.document_draft,
            office_patch:p.office_patch,
            office_batch:p.office_batch,
        };
        let mut pending=self.pending.lock().map_err(|_|ProposalError::Unavailable)?;
        if pending.contains_key(id){return Err(ProposalError::Duplicate)}
        if pending.len()>=MAX_PENDING{return Err(ProposalError::Full)}
        pending.insert(id.into(),Pending{
            chat_id:chat.into(),origin_generation_id:generation.into(),
            view:view.clone(),
        });
        Ok(view)
    }
    /// Consuming a proposal merely opens an existing human-driven review
    /// workflow. It does not open a tool grant, select a path or dispatch.
    pub fn consume(&self,id:&str,chat:&str)->Result<ActionProposalView,ProposalError>{
        let mut pending=self.pending.lock().map_err(|_|ProposalError::Unavailable)?;
        let entry=pending.get(id).ok_or(ProposalError::Missing)?;
        if entry.chat_id!=chat{return Err(ProposalError::WrongConversation)}
        if !valid_id(&entry.origin_generation_id){return Err(ProposalError::Invalid)}
        Ok(pending.remove(id).ok_or(ProposalError::Missing)?.view)
    }
    pub fn deny(&self,id:&str,chat:&str)->Result<(),ProposalError>{
        self.consume(id,chat).map(|_|())
    }
    pub fn clear_chat(&self,chat:&str)->Result<usize,ProposalError>{
        let mut pending=self.pending.lock().map_err(|_|ProposalError::Unavailable)?;
        let before=pending.len();
        pending.retain(|_,p|p.chat_id!=chat);
        Ok(before-pending.len())
    }
}
mod tests{
    use super::*;
    #[test]
    fn explicit_human_intent_is_required_and_has_one_action(){
        assert_eq!(classify_current_user_action("Copy file notes.txt"),Some(NativeAction::CopyFile));
        assert_eq!(classify_current_user_action("Please move the file"),Some(NativeAction::MoveFile));
        assert_eq!(classify_current_user_action("Extract this zip archive"),Some(NativeAction::ExtractArchive));
        assert_eq!(classify_current_user_action("Extract multiple archives as batch"),Some(NativeAction::ExtractArchiveBatch));
        assert_eq!(classify_current_user_action("Split this archive"),Some(NativeAction::SplitArchive));
        assert_eq!(classify_current_user_action("Create a Word document"),Some(NativeAction::CreateDocument));
        assert_eq!(classify_current_user_action("Revise an Excel spreadsheet"),Some(NativeAction::ReviseOffice));
        for input in ["How do I copy a file?", "Do not delete a file",
            "Copy and move the file", "Tell me about archives",
            "Do not split this archive",
            "Explain how to modify a Word document",
            "Copy the file\n\n[Attached text file: note.txt]\nsecret"]{
            assert!(classify_current_user_action(input).is_none(),"{input}")
        }
    }
    #[test]
    fn model_drafts_are_bounded_inert_and_action_specific(){
        let q=ActionProposalQueue::default();
        let doc=r#"{"kind":"propose_native_action","action":"create_document","purpose":"Review my draft","documentDraft":{"format":"docx","title":"Report","content":"Original text"}} "#;
        let approved=q.stage("d-1","c-1","g-1","Create a Word document",doc).unwrap();
        assert_eq!(approved.document_draft.as_ref().unwrap().format,"docx");
        assert_eq!(q.consume("d-1","c-1").unwrap().document_draft.unwrap().content,"Original text");
        for (i,bad) in [
            r#"{"kind":"propose_native_action","action":"copy_file","purpose":"ok","documentDraft":{"format":"docx","title":"x","content":"y"}}"#,
            r#"{"kind":"propose_native_action","action":"create_document","purpose":"ok","documentDraft":{"format":"exe","title":"x","content":"y"}}"#,
            r#"{"kind":"propose_native_action","action":"revise_office","purpose":"ok","officePatch":{"kind":"shell","textIndex":0,"expectedOld":"x","newValue":"y"}}"#,
            r#"{"kind":"propose_native_action","action":"batch_office","purpose":"ok","officeBatch":[]}"#,
            r#"{"kind":"propose_native_action","action":"create_document","purpose":"ok","documentDraft":{"format":"docx","title":"x","content":"y","destination":"C:\\\\secret"}}"#,
        ].iter().enumerate(){
            let user=if bad.contains("batch_office"){"Batch revise an Office spreadsheet"}
                else if bad.contains("revise_office"){"Revise an Office spreadsheet"}
                else if bad.contains("copy_file"){"Copy file"}else{"Create a Word document"};
            assert!(q.stage(&format!("bad-{i}"),"c-1","g-1",user,bad).is_err());
        }
    }
    #[test]
    fn model_proposals_never_upgrade_human_action_or_chat(){
        let q=ActionProposalQueue::default();
        let good=r#"{"kind":"propose_native_action","action":"copy_file","purpose":"Select the source file for an approved copy review"}"#;
        let view=q.stage("p-1","c-1","g-1","Copy the file",good).unwrap();
        assert_eq!(view.action,"copy_file");
        assert_eq!(q.consume("p-1","c-2"),Err(ProposalError::WrongConversation));
        assert_eq!(q.consume("p-1","c-1").unwrap(),view);
        assert_eq!(q.consume("p-1","c-1"),Err(ProposalError::Missing));
        for (i,bad) in [
            r#"{"kind":"propose_native_action","action":"move_file","purpose":"wrong scope"}"#,
            r#"{"kind":"propose_native_action","action":"copy_file","purpose":"ok","purpose":"overwrite"}"#,
            r#"{"kind":"propose_native_action","action":"copy_file","purpose":"ok","path":"C:\\secret"}"#,
            r#"{"kind":"propose_native_action","action":"copy_file","purpose":"ok","permission":true}"#,
        ].iter().enumerate(){
            assert!(q.stage(&format!("p-{i}"),"c-1","g-1","Copy the file",bad).is_err())
        }
        assert_eq!(q.clear_chat("c-1").unwrap(),0);
    }
}
