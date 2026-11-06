//! Wave 2 desktop structured-document creation. A user chooses an existing
//! destination FOLDER through a native picker; Lu creates one NEW DOCX/PPTX
//! filename with a host nonce. WebView supplies only bounded draft text.
//! Approval is an independent native modal after a complete preview. Neither
//! the draft nor this queue creates a tool grant. Exact package hash is
//! re-rendered and checked by the Lu Core host fence before publication.
use std::{
    collections::HashMap,path::Path,sync::{Arc,Mutex},
};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority},
    journal::EffectJournal,
    model::ModelToolCatalog,
    native_docx_create::{render_docx,DocxBlock,DocxBlockKind},
    native_docx_tool::{
        register_docx_create,ExactDocxCreatePolicy,TOOL_ID as DOCX_ID,
        INPUT as DOCX_INPUT,OUTPUT as DOCX_OUTPUT,
    },
    native_pptx_create::{render_pptx,PresentationSlide},
    native_pptx_tool::{
        register_pptx_create,ExactPptxCreatePolicy,TOOL_ID as PPTX_ID,
        INPUT as PPTX_INPUT,OUTPUT as PPTX_OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::NativeToolRegistry,
    wire::proto,
};
use prost::Message;
use serde::Serialize;
use sha2::{Digest,Sha256};
use crate::chat_store::ChatStore;

const MAX_PENDING:usize=8;
const MAX_BODY_BYTES:usize=16*1024;
fn safe_title(s:&str)->bool{
    !s.trim().is_empty()&&s.len()<=256
        &&!s.chars().any(|c|c.is_control())
}
fn safe_body(s:&str)->bool{
    !s.is_empty()&&s.len()<=MAX_BODY_BYTES
        &&s.chars().all(|c|c=='\n'||c=='\r'||c=='\t'||!c.is_control())
}
fn safe_review_id(s:&str)->bool{
    (16..=128).contains(&s.len())&&s.bytes().all(|c|
        c.is_ascii_alphanumeric()||c==b'-'||c==b'_')
}
fn safe_target(s:&str)->bool{
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
        &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn pinned_folder(path:&Path)->Result<Arc<Dir>,String>{
    let meta=std::fs::symlink_metadata(path).map_err(|_|"Document folder unavailable")?;
    if !meta.is_dir()||meta.file_type().is_symlink(){
        return Err("Document folder cannot be a link".into())
    }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0 {
            return Err("Windows reparse-point document folder refused".into())
        }
    }
    Ok(Arc::new(Dir::open_ambient_dir(path,ambient_authority())
        .map_err(|_|"Cannot open selected document folder")?))
}
fn document_name(title:&str,review_id:&str,extension:&str)->String{
    let cleaned=title.chars().filter(|c|c.is_ascii_alphanumeric()||*c=='-'||*c=='_')
        .take(44).collect::<String>();
    let prefix=if cleaned.is_empty(){"Lu-Document"}else{cleaned.as_str()};
    let id=review_id.chars().filter(|c|c.is_ascii_alphanumeric())
        .take(10).collect::<String>();
    format!("{prefix}-{id}.{extension}")
}
fn docx_blocks(title:&str,body:&str)->Vec<DocxBlock>{
    let mut blocks=vec![DocxBlock{kind:DocxBlockKind::Title,text:title.into()}];
    for paragraph in body.lines(){
        if paragraph.trim().is_empty(){continue}
        blocks.push(DocxBlock{
            kind:DocxBlockKind::Paragraph,text:paragraph.to_owned(),
        });
    }
    blocks
}
fn pptx_slides(title:&str,body:&str)->Result<Vec<PresentationSlide>,String>{
    // Blank lines divide slides; each non-empty block begins with a slide
    // title; following lines are bullet points. A one-block deck uses the
    // explicit title as its slide title and body lines as bullets.
    let segments=body.split("\n\n")
        .map(|part|part.lines().map(str::trim)
            .filter(|line|!line.is_empty()).collect::<Vec<_>>())
        .filter(|segment|!segment.is_empty()).collect::<Vec<_>>();
    if segments.is_empty()||segments.len()>64{
        return Err("Presentation must contain 1–64 nonempty slide blocks".into())
    }
    let mut slides=Vec::new();
    for (i,part) in segments.iter().enumerate(){
        let (slide_title,bullets)=if segments.len()==1{
            (title.to_owned(),part.iter().map(|s|(*s).to_owned()).collect::<Vec<_>>())
        }else{
            (format!("{} — {}",title,part[0]),
                part[1..].iter().map(|s|(*s).to_owned()).collect::<Vec<_>>())
        };
        if i>=64||bullets.len()>20{
            return Err("Presentation exceeds slide or bullet limits".into())
        }
        slides.push(PresentationSlide{title:slide_title,bullet_points:bullets});
    }
    Ok(slides)
}
#[derive(Clone)]
enum RenderedDocument{Docx(Vec<DocxBlock>),Pptx(Vec<PresentationSlide>)}
impl RenderedDocument{
    fn bytes(&self)->Result<Vec<u8>,String>{
        match self{
            Self::Docx(blocks)=>render_docx(blocks)
                .map_err(|_|"Word document validation failed".into()),
            Self::Pptx(slides)=>render_pptx(slides)
                .map_err(|_|"PowerPoint document validation failed".into()),
        }
    }
    fn kind(&self)->&'static str{
        match self{Self::Docx(_)=>"docx",Self::Pptx(_)=>"pptx"}
    }
    fn tool_id(&self)->&'static str{
        match self{Self::Docx(_)=>DOCX_ID,Self::Pptx(_)=>PPTX_ID}
    }
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct DocumentCreateView{
    pub review_id:String,
    pub chat_id:String,
    pub file_name:String,
    pub exact_destination:String,
    pub document_format:String,
    pub title:String,
    pub content:String,
    pub bytes:u64,
    pub sha256:String,
    pub content_units:usize,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct DocumentCreatedView{
    pub file_name:String,
    pub exact_destination:String,
    pub document_format:String,
    pub bytes_written:u64,
    pub sha256:String,
    pub effect_call_id:String,
}
#[derive(Clone)]
pub struct PinnedDocumentCreate {
    chat_id:String,review_id:String,
    file_name:String,exact_destination:String,
    title:String,body:String,
    expected_sha256:String,expected_bytes:u64,
    directory:Arc<Dir>,document:RenderedDocument,
}
impl PinnedDocumentCreate{
    fn view(&self)->DocumentCreateView{
        DocumentCreateView{
            review_id:self.review_id.clone(),chat_id:self.chat_id.clone(),
            file_name:self.file_name.clone(),
            exact_destination:self.exact_destination.clone(),
            document_format:self.document.kind().into(),
            title:self.title.clone(),content:self.body.clone(),
            bytes:self.expected_bytes,sha256:self.expected_sha256.clone(),
            content_units:match &self.document{
                RenderedDocument::Docx(b)=>b.len(),
                RenderedDocument::Pptx(s)=>s.len(),
            },
        }
    }
    pub fn for_modal(&self)->DocumentCreateView{self.view()}
    pub fn same_as(&self,other:&Self)->bool{
        self.chat_id==other.chat_id&&self.review_id==other.review_id
            &&self.file_name==other.file_name
            &&self.exact_destination==other.exact_destination
            &&self.title==other.title&&self.body==other.body
            &&self.expected_sha256==other.expected_sha256
            &&self.expected_bytes==other.expected_bytes
            &&self.document.kind()==other.document.kind()
            &&Arc::ptr_eq(&self.directory,&other.directory)
    }
}
#[derive(Default)]
pub struct DocumentCreateQueue{
    pending:Mutex<HashMap<String,PinnedDocumentCreate>>,
}
impl DocumentCreateQueue{
    /// This stages a preview only. No effect is committed or permission minted.
    pub fn stage(&self,chat:&str,id:&str,destination:&Path,
        format:&str,title:&str,body:&str
    )->Result<DocumentCreateView,String>{
        if chat.is_empty()||!safe_review_id(id)
            ||!safe_title(title)||!safe_body(body){
            return Err("Invalid bounded document proposal".into())
        }
        let root=pinned_folder(destination)?;
        let document=match format{
            "docx"=>RenderedDocument::Docx(docx_blocks(title,body)),
            "pptx"=>RenderedDocument::Pptx(pptx_slides(title,body)?),
            _=>return Err("Document format must be DOCX or PPTX".into()),
        };
        let file_name=document_name(title,id,format);
        if !safe_target(&file_name){return Err("Invalid generated document filename".into())}
        if root.symlink_metadata(&file_name).is_ok(){
            return Err("New document target already exists".into())
        }
        let bytes=document.bytes()?;
        let expected_sha256=format!("{:x}",Sha256::digest(&bytes));
        let preview=PinnedDocumentCreate{
            chat_id:chat.into(),review_id:id.into(),
            file_name:file_name.clone(),
            exact_destination:destination.join(&file_name).to_string_lossy().into_owned(),
            title:title.into(),body:body.into(),
            expected_sha256,expected_bytes:bytes.len() as u64,directory:root,
            document,
        };
        let mut queue=self.pending.lock().map_err(|_|"Document review unavailable")?;
        if queue.len()>=MAX_PENDING{return Err("Too many pending document reviews".into())}
        if queue.contains_key(id){return Err("Document review already exists".into())}
        let view=preview.view();
        queue.insert(id.into(),preview);
        Ok(view)
    }
    pub fn inspect(&self,chat:&str,id:&str)->Result<PinnedDocumentCreate,String>{
        let queue=self.pending.lock().map_err(|_|"Document review unavailable")?;
        let found=queue.get(id).ok_or("Document review expired")?;
        if found.chat_id!=chat{return Err("Document review belongs to another chat".into())}
        Ok(found.clone())
    }
    pub fn consume(&self,chat:&str,id:&str)->Result<PinnedDocumentCreate,String>{
        let mut queue=self.pending.lock().map_err(|_|"Document review unavailable")?;
        let found=queue.get(id).ok_or("Document review expired")?;
        if found.chat_id!=chat{return Err("Document review belongs to another chat".into())}
        queue.remove(id).ok_or("Document review already consumed".into())
    }
    pub fn deny(&self,chat:&str,id:&str)->Result<(),String>{
        let _=self.consume(chat,id)?; Ok(())
    }
}
pub async fn execute_approved_document_create(
    chats:&ChatStore,effects:&EffectJournal,
    chat:&str,user_turn:&str,effect_id:&str,
    preview:&PinnedDocumentCreate,
)->Result<DocumentCreatedView,String>{
    chats.append_user_with_turn_id(chat,user_turn,
        &format!("Approved creating NEW {} in {}. Exact package SHA-256 {}. {} bytes. One-use native authorization.",
            preview.file_name,preview.exact_destination,
            preview.expected_sha256,preview.expected_bytes))?;
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    match &preview.document{
        RenderedDocument::Docx(_)=>register_docx_create(
            &registry,&mut catalog,Arc::clone(&preview.directory)
        ).map_err(|_|"DOCX native registration failed")?,
        RenderedDocument::Pptx(_)=>register_pptx_create(
            &registry,&mut catalog,Arc::clone(&preview.directory)
        ).map_err(|_|"PPTX native registration failed")?,
    }
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task_id=format!("document-create-{user_turn}");
    let scope=format!("document-scope-{user_turn}");
    authority.open_user_task(TaskAuthority{
        conversation_id:chat.into(),user_turn_id:user_turn.into(),
        task_id:task_id.clone(),execution_scope_id:scope.clone(),
        capabilities:vec![AuthorizedCapability{
            tool_id:preview.document.tool_id().into(),
            effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },chats).map_err(|_|"Document write human-turn admission rejected")?;
    policy.grant(CapabilityGrant{
        tool_id:preview.document.tool_id().into(),
        effect:ToolEffect::WriteWorkspace,
        scope_id:scope.clone(),expires_at:None,
    }).map_err(|_|"Document write capability rejected")?;
    let payload=match &preview.document{
        RenderedDocument::Docx(blocks)=>{
            let pieces=blocks.iter().map(|b|proto::DocxCreateBlock{
                kind:match b.kind {
                    DocxBlockKind::Title=>"title",
                    DocxBlockKind::Heading1=>"heading1",
                    DocxBlockKind::Heading2=>"heading2",
                    DocxBlockKind::Paragraph=>"paragraph",
                }.into(),text:b.text.clone(),
            }).collect();
            ProtoPayload{
                type_url:DOCX_INPUT.into(),
                bytes:proto::DocxCreateRequest{
                    file_name:preview.file_name.clone(),blocks:pieces,
                }.encode_to_vec(),
            }
        },
        RenderedDocument::Pptx(slides)=>{
            ProtoPayload{
                type_url:PPTX_INPUT.into(),
                bytes:proto::PptxCreateRequest{
                    file_name:preview.file_name.clone(),
                    slides:slides.iter().map(|s|proto::PptxCreateSlide{
                        title:s.title.clone(),bullet_points:s.bullet_points.clone(),
                    }).collect(),
                }.encode_to_vec(),
            }
        },
    };
    let call=ToolInvocation{
        call_id:effect_id.into(),
        tool_id:preview.document.tool_id().into(),
        workspace_id:Some(scope),
        input:payload,
    };
    let origin=BoundCallOrigin{
        conversation_id:chat.into(),user_turn_id:user_turn.into(),task_id,
    };
    let output=match &preview.document{
        RenderedDocument::Docx(_)=>{
            authority.dispatch_bound_durable(
                &registry,effects,call,origin,
                &ExactDocxCreatePolicy{
                    host_policy:&policy,file_name:&preview.file_name,
                    expected_sha256:&preview.expected_sha256,
                },
            ).await
        },
        RenderedDocument::Pptx(_)=>{
            authority.dispatch_bound_durable(
                &registry,effects,call,origin,
                &ExactPptxCreatePolicy{
                    host_policy:&policy,approved_filename:&preview.file_name,
                    approved_sha256:&preview.expected_sha256,
                },
            ).await
        },
    }.map_err(|e|format!("Document creation rejected or requires reconciliation: {e:?}"))?;
    let (target,bytes,digest)=match &preview.document{
        RenderedDocument::Docx(_)=>{
            if output.payload.type_url!=DOCX_OUTPUT{
                return Err("Word creation receipt type unexpected".into())
            }
            let r=proto::DocxCreateResult::decode(output.payload.bytes.as_slice())
                .map_err(|_|"Invalid Word creation receipt")?;
            (r.file_name,r.bytes_written,r.sha256)
        },
        RenderedDocument::Pptx(_)=>{
            if output.payload.type_url!=PPTX_OUTPUT{
                return Err("PowerPoint creation receipt type unexpected".into())
            }
            let r=proto::PptxCreateResult::decode(output.payload.bytes.as_slice())
                .map_err(|_|"Invalid PowerPoint creation receipt")?;
            (r.file_name,r.bytes_written,r.sha256)
        },
    };
    if target!=preview.file_name||digest!=preview.expected_sha256
        ||bytes!=preview.expected_bytes{
        return Err("Document publication receipt does not match approved bytes".into())
    }
    // The trusted host re-reads the actual created bytes. A mismatched
    // result is UNCERTAIN and requires recovery instead of automatic retry.
    let mut file=preview.directory.open(&target)
        .map_err(|_|"Cannot reverify newly created document")?;
    let mut data=Vec::new();
    use std::io::Read;
    (&mut file).take(1024*1024+1).read_to_end(&mut data)
        .map_err(|_|"Cannot read created document for verification")?;
    if data.len() as u64!=bytes
        ||format!("{:x}",Sha256::digest(&data))!=digest{
        return Err("Created document bytes differ from approved package; review recovery".into())
    }
    Ok(DocumentCreatedView{
        file_name:target,exact_destination:preview.exact_destination.clone(),
        document_format:preview.document.kind().into(),
        bytes_written:bytes,sha256:digest,effect_call_id:effect_id.into(),
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn preview_keeps_filesystem_unmodified_until_native_consent(){
        let path=std::env::temp_dir().join(format!(
            "lu-document-queue-{}",crate::next_id("review-")
        ));
        std::fs::create_dir_all(&path).unwrap();
        let queue=DocumentCreateQueue::default();
        for (kind,body) in [
            ("docx","Paragraph one\nParagraph two"),
            ("pptx","Bullet one\nBullet two"),
        ]{
            let id=crate::next_id("proposal-");
            let view=queue.stage("chat-one",&id,&path,kind,"Approved draft",body)
                .unwrap();
            assert_eq!(view.document_format,kind);
            assert!(view.bytes>0);
            assert!(!path.join(&view.file_name).exists());
            assert!(queue.inspect("wrong-chat",&id).is_err());
            queue.deny("chat-one",&id).unwrap();
            assert!(queue.inspect("chat-one",&id).is_err());
        }
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn page_breaks_bound_presentation_structure(){
        assert_eq!(pptx_slides("Title","a\nb\n\nsecond\nc").unwrap().len(),2);
        assert!(pptx_slides("Title",&"a\n".repeat(30)).is_err());
        assert!(!safe_target("../escape.docx"));
    }
}
