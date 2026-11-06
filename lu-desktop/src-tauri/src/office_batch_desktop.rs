//! Human-first native UI review of an ordered 2..32-edit Office revision.
//! Only the OS picker selects source/new output folder. The host computes the
//! exact entire revised OOXML ZIP digest and unchanged-part proof. Model/WebView
//! supplies draft fields, not file authority. Final native consent is one-use.
use std::{collections::HashMap,path::Path,sync::{Arc,Mutex}};
use cap_std::fs::Dir;
use prost::Message;
use serde::Serialize;
use sha2::{Digest,Sha256};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority},
    journal::EffectJournal,model::ModelToolCatalog,
    native_office_revision::{OfficeRevisionEdit,render_office_revision_batch},
    native_office_batch_tool::{register_office_batch_tool,ExactBatchPolicy,
        TOOL_ID,INPUT,OUTPUT},
    policy::{CapabilityGrant,PolicyEngine},registry::NativeToolRegistry,wire::proto,
};
use crate::{
    chat_store::ChatStore,
    office_revision_desktop::{
        OfficePatchProposal,open_parent,open_folder,output_name,read_selected,
    },
};
const MAX_PENDING:usize=8;
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct OfficeBatchChangeView{pub kind:String,pub target:String,
    pub expected_old:String,pub new_value:String}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct OfficeBatchView{
    pub review_id:String,pub chat_id:String,
    pub source_display:String,pub destination_display:String,
    pub original_sha256:String,pub revised_sha256:String,
    pub revised_bytes:usize,pub changed_parts:String,
    pub changes:Vec<OfficeBatchChangeView>,
    pub unaffected_parts:Vec<(String,String)>,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct OfficeBatchReceipt{
    pub file_name:String,pub exact_destination:String,
    pub sha256:String,pub bytes_written:u64,pub effect_call_id:String,
}
#[derive(Clone)]
pub struct PinnedOfficeBatch{
    chat:String,id:String,source_name:String,target_name:String,
    source:Arc<Dir>,destination:Arc<Dir>,
    request:proto::OfficeBatchRevisionRequest,view:OfficeBatchView,
}
impl PinnedOfficeBatch{
    pub fn view(&self)->OfficeBatchView{self.view.clone()}
    pub fn same(&self,other:&Self)->bool{
        self.chat==other.chat&&self.id==other.id&&
        self.source_name==other.source_name&&self.target_name==other.target_name&&
        self.request.encode_to_vec()==other.request.encode_to_vec()&&
        Arc::ptr_eq(&self.source,&other.source)&&
        Arc::ptr_eq(&self.destination,&other.destination)
    }
}
#[derive(Default)]
pub struct OfficeBatchQueue{pending:Mutex<HashMap<String,PinnedOfficeBatch>>}
fn change(edit:&OfficeRevisionEdit)->proto::office_batch_patch::Change{
    match edit{
        OfficeRevisionEdit::WordRun{text_index,expected_old,new_text}=>
            proto::office_batch_patch::Change::WordText(proto::OfficeTextPatch{
                text_index:*text_index as u32,slide_number:0,
                expected_old:expected_old.clone(),new_text:new_text.clone(),
            }),
        OfficeRevisionEdit::SlideRun{slide_number,text_index,expected_old,new_text}=>
            proto::office_batch_patch::Change::SlideText(proto::OfficeTextPatch{
                slide_number:*slide_number as u32,text_index:*text_index as u32,
                expected_old:expected_old.clone(),new_text:new_text.clone(),
            }),
        OfficeRevisionEdit::CellNumber{sheet_number,address,expected_old,new_number}=>
            proto::office_batch_patch::Change::CellNumber(proto::OfficeNumberPatch{
                sheet_number:*sheet_number as u32,address:address.clone(),
                expected_old:*expected_old,new_number:*new_number,
            }),
        OfficeRevisionEdit::CellBoolean{sheet_number,address,expected_old,new_boolean}=>
            proto::office_batch_patch::Change::CellBoolean(proto::OfficeBooleanPatch{
                sheet_number:*sheet_number as u32,address:address.clone(),
                expected_old:*expected_old,new_boolean:*new_boolean,
            }),
    }
}
fn change_preview(edit:&OfficeRevisionEdit)->OfficeBatchChangeView{
    match edit{
        OfficeRevisionEdit::WordRun{text_index,expected_old,new_text}=>
            OfficeBatchChangeView{kind:"Word text".into(),
                target:format!("Text run {text_index}"),expected_old:expected_old.clone(),
                new_value:new_text.clone()},
        OfficeRevisionEdit::SlideRun{slide_number,text_index,expected_old,new_text}=>
            OfficeBatchChangeView{kind:"PowerPoint text".into(),
                target:format!("Slide {slide_number}, text run {text_index}"),
                expected_old:expected_old.clone(),new_value:new_text.clone()},
        OfficeRevisionEdit::CellNumber{sheet_number,address,expected_old,new_number}=>
            OfficeBatchChangeView{kind:"Excel number".into(),
                target:format!("Sheet {sheet_number}, cell {address}"),
                expected_old:expected_old.to_string(),new_value:new_number.to_string()},
        OfficeRevisionEdit::CellBoolean{sheet_number,address,expected_old,new_boolean}=>
            OfficeBatchChangeView{kind:"Excel boolean".into(),
                target:format!("Sheet {sheet_number}, cell {address}"),
                expected_old:expected_old.to_string(),new_value:new_boolean.to_string()},
    }
}
impl OfficeBatchQueue{
    pub fn stage(&self,chat:&str,id:&str,source_path:&Path,
        output_folder:&Path,proposals:&[OfficePatchProposal]
    )->Result<OfficeBatchView,String>{
        if chat.is_empty()||id.len()<16||id.len()>128||
            proposals.len()<2||proposals.len()>32{
            return Err("Batch needs 2–32 exact changes".into())
        }
        let (source_name,source)=open_parent(source_path)?;
        let destination=open_folder(output_folder)?;
        let target_name=output_name(&source_name,id)?;
        if destination.symlink_metadata(&target_name).is_ok(){
            return Err("Office output version already exists".into())
        }
        let original=read_selected(&source,&source_name)?;
        let baseline=format!("{:x}",Sha256::digest(&original));
        let mut edits=Vec::new();
        for proposed in proposals{edits.push(proposed.parse(&source_name)?)}
        let preview=render_office_revision_batch(&original,&baseline,&edits)
            .map_err(|e|format!("Unsafe or changed Office batch: {e:?}"))?;
        let request=proto::OfficeBatchRevisionRequest{
            source_name:source_name.clone(),target_new_name:target_name.clone(),
            original_sha256:baseline.clone(),
            approved_revised_sha256:preview.revised_sha256.clone(),
            patches:edits.iter().map(|e|
                proto::OfficeBatchPatch{change:Some(change(e))}).collect(),
        };
        let view=OfficeBatchView{
            review_id:id.into(),chat_id:chat.into(),
            source_display:source_path.to_string_lossy().into_owned(),
            destination_display:output_folder.join(&target_name)
                .to_string_lossy().into_owned(),
            original_sha256:baseline,
            revised_sha256:preview.revised_sha256,
            revised_bytes:preview.revised_bytes.len(),
            changed_parts:preview.target_part,
            changes:edits.iter().map(change_preview).collect(),
            unaffected_parts:preview.unaffected_parts,
        };
        let pending=PinnedOfficeBatch{chat:chat.into(),id:id.into(),
            source_name,target_name,source,destination,request,view:view.clone()};
        let mut queue=self.pending.lock().map_err(|_|"Batch queue unavailable")?;
        if queue.len()>=MAX_PENDING||queue.contains_key(id){
            return Err("Batch queue full or review duplicate".into())
        }
        queue.insert(id.into(),pending);
        Ok(view)
    }
    pub fn inspect(&self,chat:&str,id:&str)->Result<PinnedOfficeBatch,String>{
        let queue=self.pending.lock().map_err(|_|"Batch review unavailable")?;
        let item=queue.get(id).ok_or("Batch review expired")?;
        if item.chat!=chat{return Err("Batch review belongs to another chat".into())}
        Ok(item.clone())
    }
    pub fn take(&self,chat:&str,id:&str)->Result<PinnedOfficeBatch,String>{
        let mut queue=self.pending.lock().map_err(|_|"Batch queue unavailable")?;
        if queue.get(id).is_none_or(|b|b.chat!=chat){
            return Err("Batch review expired".into())
        }
        queue.remove(id).ok_or("Batch review already consumed".into())
    }
}
pub async fn apply(
    chats:&ChatStore,effects:&EffectJournal,chat:&str,
    turn:&str,effect_id:&str,review:&PinnedOfficeBatch,
)->Result<OfficeBatchReceipt,String>{
    chats.append_user_with_turn_id(chat,turn,&format!(
        "Approved {} exact ordered Office edits in a NEW version {}. Original SHA256 {}, revised package SHA256 {}. Original preserved.",
        review.view.changes.len(),review.view.destination_display,
        review.request.original_sha256,review.request.approved_revised_sha256,
    ))?;
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_office_batch_tool(&registry,&mut catalog,
        Arc::clone(&review.source),Arc::clone(&review.destination))
        .map_err(|_|"Native batch tool registration failed")?;
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task=format!("office-batch-{turn}");
    let scope=format!("office-batch-scope-{turn}");
    auth.open_user_task(TaskAuthority{
        conversation_id:chat.into(),user_turn_id:turn.into(),
        task_id:task.clone(),execution_scope_id:scope.clone(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },chats).map_err(|_|"Batch human user turn not admitted")?;
    policy.grant(CapabilityGrant{
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:scope.clone(),expires_at:None,
    }).map_err(|_|"Batch write grant refused")?;
    let reply=auth.dispatch_bound_durable(
        &registry,effects,ToolInvocation{
            call_id:effect_id.into(),tool_id:TOOL_ID.into(),
            workspace_id:Some(scope),input:ProtoPayload{
                type_url:INPUT.into(),bytes:review.request.encode_to_vec(),
            },
        },BoundCallOrigin{
            conversation_id:chat.into(),user_turn_id:turn.into(),task_id:task,
        },&ExactBatchPolicy{
            host_policy:&policy,trusted_source:&review.source,
            approved:&review.request,
        }
    ).await.map_err(|e|format!("Office batch failed or uncertain: {e:?}"))?;
    if reply.payload.type_url!=OUTPUT{
        return Err("Office batch receipt had unexpected Protobuf type".into())
    }
    let result=proto::OfficeRevisionResult::decode(reply.payload.bytes.as_slice())
        .map_err(|_|"Office batch receipt malformed")?;
    if result.target_new_name!=review.target_name||
        result.revised_sha256!=review.request.approved_revised_sha256||
        result.bytes_written as usize!=review.view.revised_bytes{
        return Err("Office batch receipt differs from approved package".into())
    }
    let written=read_selected(&review.destination,&review.target_name)?;
    if format!("{:x}",Sha256::digest(&written))!=result.revised_sha256{
        return Err("Created Office batch version differs from approved SHA".into())
    }
    let original=read_selected(&review.source,&review.source_name)?;
    if format!("{:x}",Sha256::digest(&original))!=review.request.original_sha256{
        return Err("Original Office document changed; manual recovery required".into())
    }
    Ok(OfficeBatchReceipt{file_name:result.target_new_name,
        exact_destination:review.view.destination_display.clone(),
        sha256:result.revised_sha256,bytes_written:result.bytes_written,
        effect_call_id:effect_id.into()})
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::fs;
    use lu_core::native_docx_create::{render_docx,DocxBlock,DocxBlockKind};
    #[test]
    fn native_review_renders_two_edits_but_cannot_write_without_consent(){
        let root=std::env::temp_dir().join(crate::next_id("lu-office-batch-ui-"));
        fs::create_dir_all(root.join("revisions")).unwrap();
        let original=render_docx(&[
            DocxBlock{kind:DocxBlockKind::Title,text:"Original title".into()},
            DocxBlock{kind:DocxBlockKind::Paragraph,text:"Original body".into()},
        ]).unwrap();
        fs::write(root.join("source.docx"),&original).unwrap();
        let changes=vec![
            OfficePatchProposal{
                kind:"word_text".into(),text_index:Some(0),
                slide_number:None,sheet_number:None,address:None,
                expected_old:"Original title".into(),new_value:"New title".into(),
            },
            OfficePatchProposal{
                kind:"word_text".into(),text_index:Some(1),
                slide_number:None,sheet_number:None,address:None,
                expected_old:"Original body".into(),new_value:"New body".into(),
            },
        ];
        let queue=OfficeBatchQueue::default();
        let id=crate::next_id("ob-");
        let view=queue.stage("chat-one",&id,
            &root.join("source.docx"),&root.join("revisions"),&changes).unwrap();
        assert_eq!(view.changes.len(),2);
        assert!(view.unaffected_parts.iter().any(|(part,_)|part=="word/styles.xml"));
        assert_ne!(view.original_sha256,view.revised_sha256);
        assert!(queue.inspect("not-chat",&id).is_err());
        let taken=queue.take("chat-one",&id).unwrap();
        assert!(queue.inspect("chat-one",&id).is_err());
        assert_eq!(taken.view().review_id,id);
        assert_eq!(fs::read(root.join("source.docx")).unwrap(),original);
        assert_eq!(fs::read_dir(root.join("revisions")).unwrap().count(),0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn batch_refuses_wrong_expected_original_before_creating_anything(){
        let root=std::env::temp_dir().join(crate::next_id("lu-office-batch-denied-"));
        fs::create_dir_all(root.join("revisions")).unwrap();
        let original=render_docx(&[
            DocxBlock{kind:DocxBlockKind::Title,text:"Original title".into()},
            DocxBlock{kind:DocxBlockKind::Paragraph,text:"Original body".into()},
        ]).unwrap();
        fs::write(root.join("source.docx"),&original).unwrap();
        let changes=vec![
            OfficePatchProposal{
                kind:"word_text".into(),text_index:Some(0),
                slide_number:None,sheet_number:None,address:None,
                expected_old:"Original title".into(),new_value:"New title".into(),
            },
            OfficePatchProposal{
                kind:"word_text".into(),text_index:Some(1),
                slide_number:None,sheet_number:None,address:None,
                expected_old:"Wrong body".into(),new_value:"New body".into(),
            },
        ];
        let queue=OfficeBatchQueue::default();
        assert!(queue.stage("chat-one",&crate::next_id("ob-"),
            &root.join("source.docx"),&root.join("revisions"),&changes).is_err());
        assert_eq!(fs::read_dir(root.join("revisions")).unwrap().count(),0);
        fs::remove_dir_all(root).unwrap();
    }
}
