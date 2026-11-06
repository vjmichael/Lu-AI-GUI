//! Native desktop preview/consent for NON-DESTRUCTIVE existing DOCX/PPTX/XLSX
//! revisions. Source and destination are native-picked directory capabilities.
//! A WebView patch is untrusted input; exact originals and revised OOXML part
//! checks are computed by the host, then approved in a separate native modal.
//! Original files are never changed or overwritten. No automatic retry.
use std::{
    collections::HashMap,fs,io::Read,path::Path,
    sync::{Arc,Mutex},
};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority},
    journal::EffectJournal,model::ModelToolCatalog,
    native_office_revision::{
        render_office_revision,OfficeRevisionEdit,OfficeRevisionPreview,
    },
    native_office_revision_tool::{
        register_office_revision_tool,ExactOfficeRevisionPolicy,TOOL_ID,INPUT,OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::NativeToolRegistry,wire::proto,
};
use prost::Message;
use serde::{Deserialize,Serialize};
use sha2::{Sha256,Digest};
use crate::chat_store::ChatStore;
const MAX_PENDING:usize=8;
const MAX_PACKAGE:usize=1024*1024;
fn safe_base(s:&str)->bool{
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
        &&!s.starts_with(".lu-stage-")
        &&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn safe_id(s:&str)->bool{
    (16..=128).contains(&s.len())&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn open_parent(path:&Path)->Result<(String,Arc<Dir>),String>{
    let name=path.file_name().and_then(|s|s.to_str())
        .ok_or("Selected Office file has an invalid filename")?;
    if !safe_base(name)||![".docx",".pptx",".xlsx"].iter()
        .any(|e|name.to_ascii_lowercase().ends_with(e)){
        return Err("Choose a supported DOCX/PPTX/XLSX document".into())
    }
    let meta=fs::symlink_metadata(path).map_err(|_|"Document unavailable")?;
    if !meta.is_file()||meta.file_type().is_symlink()
        ||meta.len()>MAX_PACKAGE as u64{
        return Err("Document must be a regular file under 1 MiB".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err("Reparse-point Office documents are not supported".into())
        }
    }
    let parent=path.parent().ok_or("Document folder unavailable")?;
    let cap=Arc::new(Dir::open_ambient_dir(parent,ambient_authority())
        .map_err(|_|"Cannot pin selected Office folder")?);
    Ok((name.into(),cap))
}
fn open_folder(path:&Path)->Result<Arc<Dir>,String>{
    let meta=fs::symlink_metadata(path).map_err(|_|"Output folder unavailable")?;
    if !meta.is_dir()||meta.file_type().is_symlink(){
        return Err("Office output folder cannot be a link".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err("Reparse-point output folders are not supported".into())
        }
    }
    Ok(Arc::new(Dir::open_ambient_dir(path,ambient_authority())
        .map_err(|_|"Cannot pin output folder")?))
}
fn output_name(source:&str,id:&str)->Result<String,String>{
    let (stem,ext)=source.rsplit_once('.').ok_or("Document has no extension")?;
    let suffix=id.bytes().filter(|b|b.is_ascii_alphanumeric())
        .take(12).map(char::from).collect::<String>();
    let prefix=stem.chars().filter(|c|c.is_ascii_alphanumeric()||*c=='-'||*c=='_')
        .take(48).collect::<String>();
    let prefix=if prefix.is_empty(){"Lu-Office"}else{prefix.as_str()};
    let name=format!("{prefix}-revision-{suffix}.{}",ext.to_ascii_lowercase());
    if !safe_base(&name){return Err("Invalid generated output filename".into())}
    Ok(name)
}
#[derive(Clone,Debug,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct OfficePatchProposal{
    pub kind:String,
    pub text_index:Option<usize>,
    pub slide_number:Option<usize>,
    pub sheet_number:Option<usize>,
    pub address:Option<String>,
    pub expected_old:String,
    pub new_value:String,
}
impl OfficePatchProposal{
    fn parse(&self,source_name:&str)->Result<OfficeRevisionEdit,String>{
        if self.expected_old.len()>32768 ||self.new_value.len()>32768{
            return Err("Office patch text exceeds limit".into())
        }
        let ext=source_name.to_ascii_lowercase();
        match self.kind.as_str(){
            "word_text" if ext.ends_with(".docx")=>Ok(OfficeRevisionEdit::WordRun{
                text_index:self.text_index.ok_or("Word run index required")?,
                expected_old:self.expected_old.clone(),
                new_text:self.new_value.clone(),
            }),
            "slide_text" if ext.ends_with(".pptx")=>Ok(OfficeRevisionEdit::SlideRun{
                slide_number:self.slide_number.ok_or("Slide number required")?,
                text_index:self.text_index.ok_or("Slide text index required")?,
                expected_old:self.expected_old.clone(),
                new_text:self.new_value.clone(),
            }),
            "cell_number" if ext.ends_with(".xlsx")=>Ok(OfficeRevisionEdit::CellNumber{
                sheet_number:self.sheet_number.ok_or("Worksheet number required")?,
                address:self.address.clone().ok_or("Cell address required")?,
                expected_old:self.expected_old.parse::<f64>()
                    .map_err(|_|"Expected cell value must be a number")?,
                new_number:self.new_value.parse::<f64>()
                    .map_err(|_|"New cell value must be a number")?,
            }),
            "cell_boolean" if ext.ends_with(".xlsx")=>Ok(OfficeRevisionEdit::CellBoolean{
                sheet_number:self.sheet_number.ok_or("Worksheet number required")?,
                address:self.address.clone().ok_or("Cell address required")?,
                expected_old:self.expected_old.parse::<bool>()
                    .map_err(|_|"Expected cell value must be true or false")?,
                new_boolean:self.new_value.parse::<bool>()
                    .map_err(|_|"New cell value must be true or false")?,
            }),
            _=>Err("Selected document type and patch kind do not match".into()),
        }
    }
}
#[derive(Clone,Serialize,Debug)]
#[serde(rename_all="camelCase")]
pub struct OfficePartView{pub part_name:String,pub sha256:String}
#[derive(Clone,Serialize,Debug)]
#[serde(rename_all="camelCase")]
pub struct OfficeRevisionView{
    pub review_id:String,
    pub chat_id:String,
    pub source_display:String,
    pub destination_display:String,
    pub source_name:String,
    pub target_new_name:String,
    pub original_sha256:String,
    pub revised_sha256:String,
    pub revised_bytes:usize,
    pub target_part:String,
    pub edit_kind:String,
    pub original_value:String,
    pub new_value:String,
    pub unaffected_parts:Vec<OfficePartView>,
}
#[derive(Clone,Serialize,Debug)]
#[serde(rename_all="camelCase")]
pub struct OfficeRevisionReceiptView{
    pub target_new_name:String,pub exact_destination:String,
    pub bytes_written:u64,pub sha256:String,pub effect_call_id:String,
}
#[derive(Clone)]
pub struct PinnedOfficeRevision{
    chat_id:String,review_id:String,
    source_display:String,destination_display:String,
    source_name:String,target_name:String,
    source:Arc<Dir>,destination:Arc<Dir>,
    edit:OfficeRevisionEdit,
    view:OfficeRevisionView,
    request:proto::OfficeRevisionRequest,
}
impl PinnedOfficeRevision{
    pub fn view(&self)->OfficeRevisionView{self.view.clone()}
    pub fn same_as(&self,other:&Self)->bool{
        self.chat_id==other.chat_id&&self.review_id==other.review_id
            &&self.source_display==other.source_display
            &&self.destination_display==other.destination_display
            &&self.source_name==other.source_name&&self.target_name==other.target_name
            &&self.edit==other.edit
            &&self.request.encode_to_vec()==other.request.encode_to_vec()
            &&Arc::ptr_eq(&self.source,&other.source)
            &&Arc::ptr_eq(&self.destination,&other.destination)
    }
}
#[derive(Default)]
pub struct OfficeRevisionQueue{
    pending:Mutex<HashMap<String,PinnedOfficeRevision>>,
}
fn read_selected(root:&Dir,name:&str)->Result<Vec<u8>,String>{
    let meta=root.symlink_metadata(name).map_err(|_|"Office source unavailable")?;
    if !meta.is_file()||meta.file_type().is_symlink()
        ||meta.len()>MAX_PACKAGE as u64 {
        return Err("Office source is not a supported regular file".into())
    }
    let mut file=root.open(name).map_err(|_|"Office source cannot be opened")?;
    let mut data=Vec::new();
    (&mut file).take(MAX_PACKAGE as u64+1).read_to_end(&mut data)
        .map_err(|_|"Cannot read Office document")?;
    if data.len()>MAX_PACKAGE{return Err("Office document exceeds limit".into())}
    Ok(data)
}
fn build_change(edit:&OfficeRevisionEdit)->proto::office_revision_request::Change{
    match edit{
        OfficeRevisionEdit::WordRun{text_index,expected_old,new_text}=>
            proto::office_revision_request::Change::WordText(proto::OfficeTextPatch{
                text_index:*text_index as u32,slide_number:0,
                expected_old:expected_old.clone(),new_text:new_text.clone(),
            }),
        OfficeRevisionEdit::SlideRun{slide_number,text_index,expected_old,new_text}=>
            proto::office_revision_request::Change::SlideText(proto::OfficeTextPatch{
                slide_number:*slide_number as u32,text_index:*text_index as u32,
                expected_old:expected_old.clone(),new_text:new_text.clone(),
            }),
        OfficeRevisionEdit::CellNumber{sheet_number,address,expected_old,new_number}=>
            proto::office_revision_request::Change::CellNumber(proto::OfficeNumberPatch{
                sheet_number:*sheet_number as u32,address:address.clone(),
                expected_old:*expected_old,new_number:*new_number,
            }),
        OfficeRevisionEdit::CellBoolean{sheet_number,address,expected_old,new_boolean}=>
            proto::office_revision_request::Change::CellBoolean(proto::OfficeBooleanPatch{
                sheet_number:*sheet_number as u32,address:address.clone(),
                expected_old:*expected_old,new_boolean:*new_boolean,
            }),
    }
}
impl OfficeRevisionQueue{
    /// The OS supplies both paths; the WebView supplies ONLY a bounded edit
    /// request and cannot select any actual file or write destination.
    pub fn stage(&self,chat:&str,id:&str,source_path:&Path,output_folder:&Path,
        proposed:&OfficePatchProposal
    )->Result<OfficeRevisionView,String>{
        if chat.is_empty()||!safe_id(id){return Err("Invalid Office review".into())}
        let (source_name,source)=open_parent(source_path)?;
        let destination=open_folder(output_folder)?;
        let target=output_name(&source_name,id)?;
        if destination.symlink_metadata(&target).is_ok(){
            return Err("Output Office revision already exists".into())
        }
        let original=read_selected(&source,&source_name)?;
        let original_sha256=format!("{:x}",Sha256::digest(&original));
        let edit=proposed.parse(&source_name)?;
        let OfficeRevisionPreview{
            revised_sha256,revised_bytes,target_part,unaffected_parts,..
        }=render_office_revision(&original,&original_sha256,&edit)
            .map_err(|e|format!("This Office revision cannot safely be rendered: {e:?}"))?;
        let request=proto::OfficeRevisionRequest{
            source_name:source_name.clone(),target_new_name:target.clone(),
            original_sha256:original_sha256.clone(),
            approved_revised_sha256:revised_sha256.clone(),
            change:Some(build_change(&edit)),
        };
        let view=OfficeRevisionView{
            review_id:id.into(),chat_id:chat.into(),
            source_display:source_path.to_string_lossy().into_owned(),
            destination_display:output_folder.join(&target).to_string_lossy().into_owned(),
            source_name:source_name.clone(),target_new_name:target.clone(),
            original_sha256,revised_sha256,
            revised_bytes:revised_bytes.len(),target_part,
            edit_kind:proposed.kind.clone(),
            original_value:proposed.expected_old.clone(),
            new_value:proposed.new_value.clone(),
            unaffected_parts:unaffected_parts.into_iter().map(|(part_name,sha256)|
                OfficePartView{part_name,sha256}
            ).collect(),
        };
        let pending=PinnedOfficeRevision{
            chat_id:chat.into(),review_id:id.into(),
            source_display:view.source_display.clone(),
            destination_display:view.destination_display.clone(),
            source_name,target_name:target,source,destination,
            edit,view:view.clone(),request,
        };
        let mut queue=self.pending.lock().map_err(|_|"Office review unavailable")?;
        if queue.len()>=MAX_PENDING{return Err("Too many Office reviews".into())}
        if queue.contains_key(id){return Err("Office review already exists".into())}
        queue.insert(id.into(),pending);
        Ok(view)
    }
    pub fn inspect(&self,chat:&str,id:&str)->Result<PinnedOfficeRevision,String>{
        let queue=self.pending.lock().map_err(|_|"Office review unavailable")?;
        let review=queue.get(id).ok_or("Office review expired")?;
        if review.chat_id!=chat{return Err("Office review belongs to another chat".into())}
        Ok(review.clone())
    }
    pub fn consume(&self,chat:&str,id:&str)->Result<PinnedOfficeRevision,String>{
        let mut queue=self.pending.lock().map_err(|_|"Office review unavailable")?;
        let review=queue.get(id).ok_or("Office review expired")?;
        if review.chat_id!=chat{return Err("Office review belongs to another chat".into())}
        queue.remove(id).ok_or("Office review was already consumed".into())
    }
    pub fn deny(&self,chat:&str,id:&str)->Result<(),String>{
        let _=self.consume(chat,id)?;Ok(())
    }
}
pub async fn execute_office_revision(
    chats:&ChatStore,effects:&EffectJournal,
    chat:&str,user_turn:&str,effect_id:&str,
    preview:&PinnedOfficeRevision,
)->Result<OfficeRevisionReceiptView,String>{
    chats.append_user_with_turn_id(chat,user_turn,
        &format!("Approved a NEW Office document revision {} from {}. Original SHA-256 {} and revised SHA-256 {}. Full package part preservation reviewed.",
            preview.destination_display,preview.source_display,
            preview.request.original_sha256,preview.request.approved_revised_sha256))?;
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_office_revision_tool(
        &registry,&mut catalog,Arc::clone(&preview.source),
        Arc::clone(&preview.destination)
    ).map_err(|_|"Native Office tool registration failed")?;
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task=format!("office-revision-{user_turn}");
    let scope=format!("office-revision-scope-{user_turn}");
    authority.open_user_task(TaskAuthority{
        conversation_id:chat.into(),user_turn_id:user_turn.into(),
        task_id:task.clone(),execution_scope_id:scope.clone(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },chats).map_err(|_|"Office user-turn grant failed")?;
    policy.grant(CapabilityGrant{
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:scope.clone(),expires_at:None,
    }).map_err(|_|"Office effect policy rejected")?;
    let output=authority.dispatch_bound_durable(
        &registry,effects,ToolInvocation{
            call_id:effect_id.into(),tool_id:TOOL_ID.into(),
            workspace_id:Some(scope),
            input:ProtoPayload{
                type_url:INPUT.into(),bytes:preview.request.encode_to_vec(),
            },
        },BoundCallOrigin{
            conversation_id:chat.into(),user_turn_id:user_turn.into(),task_id:task,
        },&ExactOfficeRevisionPolicy{
            host_policy:&policy,trusted_source:&preview.source,
            approved_request:&preview.request,
        },
    ).await.map_err(|e|format!("Office revision denied or needs recovery: {e:?}"))?;
    if output.payload.type_url!=OUTPUT{
        return Err("Office revision receipt had wrong message type".into())
    }
    let receipt=proto::OfficeRevisionResult::decode(output.payload.bytes.as_slice())
        .map_err(|_|"Office revision receipt is invalid")?;
    if receipt.target_new_name!=preview.target_name ||
        receipt.revised_sha256!=preview.request.approved_revised_sha256 ||
        receipt.bytes_written as usize!=preview.view.revised_bytes {
        return Err("Published Office revision differs from approved output".into())
    }
    let stored=read_selected(&preview.destination,&receipt.target_new_name)?;
    let actual=format!("{:x}",Sha256::digest(&stored));
    if actual!=receipt.revised_sha256{
        return Err("Office revision on disk differs from approved package; manual recovery needed".into())
    }
    // The original is verified still byte-identical after publishing the copy.
    let original=read_selected(&preview.source,&preview.source_name)?;
    if format!("{:x}",Sha256::digest(&original))!=preview.request.original_sha256{
        return Err("Original Office document changed; investigate external writes".into())
    }
    Ok(OfficeRevisionReceiptView{
        target_new_name:receipt.target_new_name,
        exact_destination:preview.destination_display.clone(),
        bytes_written:receipt.bytes_written,
        sha256:receipt.revised_sha256,effect_call_id:effect_id.into(),
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    use lu_core::native_docx_create::{render_docx,DocxBlock,DocxBlockKind};
    #[test]
    fn native_picked_word_edit_creates_only_inert_new_revision_preview(){
        let root=std::env::temp_dir().join(format!(
            "lu-office-desktop-{}",crate::next_id("test-")
        ));
        fs::create_dir_all(root.join("output")).unwrap();
        let original=render_docx(&[
            DocxBlock{kind:DocxBlockKind::Title,text:"Before".into()},
        ]).unwrap();
        fs::write(root.join("original.docx"),original).unwrap();
        let queue=OfficeRevisionQueue::default();
        let id="office-review-0123456789";
        let view=queue.stage("chat",id,&root.join("original.docx"),&root.join("output"),
            &OfficePatchProposal{
                kind:"word_text".into(),text_index:Some(0),
                slide_number:None,sheet_number:None,address:None,
                expected_old:"Before".into(),new_value:"After".into(),
            },
        ).unwrap();
        assert_eq!(view.target_part,"word/document.xml");
        assert_ne!(view.original_sha256,view.revised_sha256);
        assert!(view.unaffected_parts.iter().any(|p|p.part_name=="word/styles.xml"));
        assert!(!root.join("output").join(view.target_new_name).exists());
        assert!(queue.inspect("other-chat",id).is_err());
        queue.deny("chat",id).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
