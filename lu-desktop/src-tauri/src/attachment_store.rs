//! CP-01B: host-profile attachment custody. ANSF stores only metadata.
//! Ordinary user image objects are immutable, create-new, bounded, and
//! verified before use. Observe pixels never enter this external byte store.
use cap_std::{ambient_authority,fs::{Dir,OpenOptions}};
use lu_core::{
    ansf_image_attachments::{ImageAttachment,valid_attachment_id},
    multimodal_saved_turn::{admit_data_url,PinnedImage,MAX_IMAGE_BYTES},
};
use base64::{engine::general_purpose::STANDARD,Engine as _};
use sha2::{Sha256,Digest};
use std::{collections::HashMap,fs,io::{Read,Write},path::Path,sync::Mutex};

const MAX_OBSERVED_IMAGES:usize=16;
const MAX_OBSERVED_BYTES:usize=64*1024*1024;

pub(crate) struct AttachmentStore{
    root:Dir,
    // This gate serializes draft mutation, verified snapshots and native
    // queue publication. It is never held during model/worker execution.
    pub(crate) gate:Mutex<()>,
    observed:Mutex<HashMap<String,(String,PinnedImage)>>,
}
fn chat_ok(chat:&str)->bool{
    chat.len()==34&&chat.starts_with("c-")&&
        chat[2..].bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn name(chat:&str,id:&str)->Result<String,String>{
    if !chat_ok(chat)||!valid_attachment_id(id){
        return Err("Invalid attachment ownership identity".into())
    }
    Ok(format!("{chat}-{id}.blob"))
}
fn hex(bytes:&[u8])->String{bytes.iter().map(|b|format!("{b:02x}")).collect()}
fn matches(a:&ImageAttachment,p:&PinnedImage)->bool{
    a.bytes==p.bytes.len() as u64&&a.media_type==p.mime_type&&
        a.sha256==hex(&p.sha256)&&a.sha256==format!("{:x}",Sha256::digest(&p.bytes))
}
fn real_directory(path:&Path)->Result<(),String>{
    let m=fs::symlink_metadata(path).map_err(|_|"Attachment storage unavailable")?;
    if !m.is_dir()||m.file_type().is_symlink(){return Err("Unsafe attachment storage directory".into())}
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes()&0x400!=0{return Err("Unsafe attachment storage junction".into())}
    }
    Ok(())
}
impl AttachmentStore{
    pub(crate) fn open(profile:&Path)->Result<Self,String>{
        real_directory(profile)?;
        let ansf=profile.join("ansf");real_directory(&ansf)?;
        let path=ansf.join("attachment-objects");
        let mut builder=fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path){
            Ok(())=>{
                #[cfg(unix)]
                fs::File::open(&ansf).and_then(|f|f.sync_all())
                    .map_err(|_|"Attachment storage parent sync failed")?;
            },
            Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>(),
            Err(_)=>return Err("Attachment storage creation failed".into()),
        }
        real_directory(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::symlink_metadata(&path).map_err(|_|"Attachment storage unavailable")?
                .permissions().mode()&0o077!=0{
                return Err("Attachment storage must be private".into())
            }
        }
        let root=Dir::open_ambient_dir(&path,ambient_authority())
            .map_err(|_|"Attachment storage open failed")?;
        Ok(Self{root,gate:Mutex::new(()),observed:Mutex::new(HashMap::new())})
    }
    pub(crate) fn metadata(id:String,display_name:&str,p:&PinnedImage,observed:bool)->ImageAttachment{
        let mut safe=crate::ansf_source_evidence::safe_reader_basename(display_name);
        if safe.trim().is_empty(){safe=if p.mime_type=="image/png"{"Image.png".into()}else{"Image.jpg".into()}}
        ImageAttachment{attachment_id:id,display_name:safe,media_type:p.mime_type.into(),
            bytes:p.bytes.len() as u64,sha256:hex(&p.sha256),observed}
    }
    fn sync_directory(&self)->Result<(),String>{
        // Linux directory entries require a separate fsync. Windows uses
        // file FlushFileBuffers through sync_all; NTFS crash behavior remains
        // a physical acceptance gate, not a claim of proven power-loss safety.
        #[cfg(unix)]
        self.root.open(".").and_then(|d|d.sync_all())
            .map_err(|_|"Attachment directory sync failed")?;
        Ok(())
    }
    pub(crate) fn write_new(&self,chat:&str,a:&ImageAttachment,p:&PinnedImage)->Result<(),String>{
        if a.observed||!matches(a,p)||p.bytes.len()>MAX_IMAGE_BYTES{
            return Err("Attachment source identity refused".into())
        }
        let filename=name(chat,&a.attachment_id)?;
        let mut options=OpenOptions::new();options.create_new(true).write(true);
        let mut file=self.root.open_with(&filename,&options)
            .map_err(|_|"Attachment object already exists or cannot be created")?;
        // Object remains unpublished to the manifest until full write, flush
        // and independent reopen/decode. On error no draft authority exists.
        let result=(||{
            file.write_all(&p.bytes).map_err(|_|"Attachment write failed")?;
            file.sync_all().map_err(|_|"Attachment sync failed")?;
            drop(file);
            self.sync_directory()?;
            self.read(chat,a).map(|_|())
        })();
        if result.is_err(){let _=self.remove_unreferenced(chat,&a.attachment_id);}
        result
    }
    pub(crate) fn read(&self,chat:&str,a:&ImageAttachment)->Result<PinnedImage,String>{
        if a.observed{
            lu_core::native_observe_egress::DataProvenance::OBSERVE_LEARN
                .gate(lu_core::native_observe_egress::Destination::LocalInference)
                .map_err(str::to_owned)?;
            let observed=self.observed.lock().map_err(|_|"Local observation unavailable")?;
            let (owner,p)=observed.get(&a.attachment_id)
                .ok_or("Local observation expired; capture it again or remove it")?;
            if owner!=chat||!matches(a,p){return Err("Local observation ownership changed".into())}
            return Ok(p.clone())
        }
        let filename=name(chat,&a.attachment_id)?;
        let meta=self.root.symlink_metadata(&filename).map_err(|_|"Saved image unavailable")?;
        if !meta.is_file()||meta.file_type().is_symlink()||meta.len()!=a.bytes||
            a.bytes==0||a.bytes>MAX_IMAGE_BYTES as u64{
            return Err("Saved image size or storage identity changed".into())
        }
        let file=self.root.open(&filename).map_err(|_|"Saved image unavailable")?;
        let opened=file.metadata().map_err(|_|"Saved image unavailable")?;
        if !opened.is_file()||opened.len()!=a.bytes{return Err("Saved image identity changed".into())}
        let mut bytes=Vec::with_capacity(a.bytes as usize);
        file.take(a.bytes+1).read_to_end(&mut bytes).map_err(|_|"Saved image read failed")?;
        if bytes.len() as u64!=a.bytes||format!("{:x}",Sha256::digest(&bytes))!=a.sha256{
            return Err("Saved image content changed".into())
        }
        let p=admit_data_url(&format!("data:{};base64,{}",a.media_type,STANDARD.encode(bytes)))
            .map_err(|_|"Saved image failed native validation")?;
        if !matches(a,&p){return Err("Saved image failed identity validation".into())}
        Ok(p)
    }
    /// Shared ordinary-Send dispatch readback: retrieve the exact original
    /// manifest and independently reopen every byte object in saved order.
    /// No text-only downgrade, resorting or lookup by current draft.
    pub(crate) fn verify_saved_exact(&self,chats:&crate::chat_store::ChatStore,
        chat:&str,turn:&str,expected:&[ImageAttachment],pinned:&[PinnedImage]
    )->Result<(),String>{
        let original=chats.require(chat)?.saved_image_manifest(chat,turn)
            .map_err(|_|"Saved attachment manifest unavailable")?;
        if original.as_slice()!=expected{return Err("Saved attachment manifest changed".into())}
        if original.len()!=pinned.len(){return Err("Saved image membership changed".into())}
        for (a,p) in original.iter().zip(pinned){
            if &self.read(chat,a)?!=p{return Err("Saved image changed while queued".into())}
        }
        Ok(())
    }
    pub(crate) fn reject_live_observed_digest(&self,p:&PinnedImage)->Result<(),String>{
        let observed=self.observed.lock().map_err(|_|"Local observation unavailable")?;
        if observed.values().any(|(_,source)|source.sha256==p.sha256){
            return Err("A local observation cannot be reclassified as a regular attachment".into())
        }
        Ok(())
    }
    pub(crate) fn register_observed(&self,chat:&str,a:&ImageAttachment,p:PinnedImage)->Result<(),String>{
        if !chat_ok(chat)||!a.observed||!matches(a,&p){return Err("Local observation identity refused".into())}
        let mut observed=self.observed.lock().map_err(|_|"Local observation unavailable")?;
        if observed.get(&a.attachment_id).is_some_and(|(owner,old)|owner!=chat||old.sha256!=p.sha256){
            return Err("Local observation ownership changed".into())
        }
        if observed.values().any(|(owner,old)|owner!=chat&&old.sha256==p.sha256){
            return Err("Local observation belongs to another conversation".into())
        }
        if !observed.contains_key(&a.attachment_id)&&
            (observed.len()>=MAX_OBSERVED_IMAGES||observed.values().map(|(_,v)|v.bytes.len()).sum::<usize>()
                .saturating_add(p.bytes.len())>MAX_OBSERVED_BYTES){
            return Err("Local observation capacity reached".into())
        }
        observed.insert(a.attachment_id.clone(),(chat.into(),p));
        Ok(())
    }
    pub(crate) fn release_observed(&self,chat:&str,id:&str){
        if let Ok(mut observed)=self.observed.lock(){
            if observed.get(id).is_some_and(|(owner,_)|owner==chat){observed.remove(id);}
        }
    }
    pub(crate) fn observed_available(&self,chat:&str,a:&ImageAttachment)->bool{
        self.observed.lock().ok().is_some_and(|m|m.get(&a.attachment_id)
            .is_some_and(|(owner,p)|owner==chat&&matches(a,p)))
    }
    /// Caller must establish under gate that no original or draft manifest
    /// references this object. A saved-message release may never use this.
    pub(crate) fn remove_unreferenced(&self,chat:&str,id:&str)->Result<(),String>{
        let filename=name(chat,id)?;
        match self.root.remove_file(&filename){
            Ok(())=>self.sync_directory(),
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),
            Err(_)=>Err("Attachment cleanup unavailable".into()),
        }
    }
    /// Reconcile only recognized object filenames under the profile owner
    /// lease. Unreadable/unknown books, unexpected names and file types are
    /// retained; absence proven in the exact book permits deletion. The scan
    /// is bounded and never claims undiscovered files have been reconciled.
    pub(crate) fn reconcile_orphans(&self,chats:&crate::chat_store::ChatStore){
        let Ok(entries)=self.root.entries() else{return};
        for (index,result) in entries.take(4097).enumerate(){
            if index==4096{eprintln!("LU_CP01_ORPHAN_SCAN_BOUNDED");break}
            let Ok(entry)=result else{continue};
            let filename=entry.file_name();
            let Some(s)=filename.to_str() else{continue};
            let Some(parts)=s.strip_suffix(".blob") else{continue};
            if parts.len()!=71||!parts.is_ascii(){continue}
            let (chat,tail)=parts.split_at(34);
            let Some(id)=tail.strip_prefix('-') else{continue};
            if !chat_ok(chat)||!valid_attachment_id(id){continue}
            if !entry.file_type().ok().is_some_and(|t|t.is_file()){continue}
            if let Ok(book)=chats.require(chat){
                if matches!(book.image_attachment_exists(chat,id),Ok(false)){
                    let _=self.remove_unreferenced(chat,id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use image::{ImageBuffer,Rgb,DynamicImage,ImageFormat};
    use std::io::Cursor;
    fn png()->PinnedImage{
        let mut bytes=Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2,2,Rgb([19,81,207])))
            .write_to(&mut bytes,ImageFormat::Png).unwrap();
        admit_data_url(&format!("data:image/png;base64,{}",STANDARD.encode(bytes.into_inner()))).unwrap()
    }
    /// Real opt-in native host acceptance. The UI itself is separately
    /// exercised in CP-01F. This never downloads/starts arbitrary files:
    /// the operator supplies reviewed expected digests and canonical model
    /// directory. No fake server, external API, or answer hint is used.
    #[test]
    #[ignore="requires independently checksum-pinned local llama.cpp, GGUF and matching MMProj"]
    fn actual_cp01_saved_one_and_four_images_reach_supervised_mmproj(){
        use lu_core::{
            inference_supervisor::TrustedInferenceSupervisor,
            persistent_llama_backend::{PersistentEngineConfig,PersistentLlamaBackend},
            model_admission::ModelAdmissionManager,
            mmproj_admission::admit_projector,
            projector_identity::projector_filename_for_model,
            inference_wire::decode_frame,wire::proto,
        };
        use crate::generation_queue::{GenerationQueue,TurnClass};
        use rand::{seq::SliceRandom,rngs::OsRng};
        use std::{num::NonZeroUsize,sync::Arc,time::Duration};
        fn env(key:&str)->String{std::env::var(key).unwrap_or_else(|_|panic!("missing reviewed {key}"))}
        fn verified(path:&Path,expected:&str)->[u8;32]{
            let expected=crate::decode_sha(expected).expect("trusted expected SHA-256");
            assert!(fs::symlink_metadata(path).unwrap().file_type().is_file());
            let mut file=fs::File::open(path).unwrap();
            let mut hash=Sha256::new();let mut buf=[0u8;64*1024];
            loop{let n=file.read(&mut buf).unwrap();if n==0{break}hash.update(&buf[..n]);}
            let actual:[u8;32]=hash.finalize().into();
            assert_eq!(actual,expected,"reviewed file identity mismatch");actual
        }
        let root=std::path::PathBuf::from(env("LU_CP01_MODEL_DIR"));
        let model_name=env("LU_CP01_MODEL_FILE");
        let model_ref=env("LU_CP01_MODEL_REF");
        let projector_name=projector_filename_for_model(&model_ref).unwrap();
        let engine=std::path::PathBuf::from(env("LU_CP01_ENGINE"));
        let engine_sha=verified(&engine,&env("LU_CP01_ENGINE_SHA256"));
        let mut manager=ModelAdmissionManager::new();
        let model=manager.admit(&root,&model_name).unwrap().clone();
        let projector=admit_projector(&root,&projector_name).unwrap();
        verified(model.path(),&env("LU_CP01_MODEL_SHA256"));
        verified(projector.path(),&env("LU_CP01_MMPROJ_SHA256"));
        let context=8192;
        let sup=TrustedInferenceSupervisor::new("llama-cpu".into(),context,
            vec![(model_ref.clone(),model)],move||{
                PersistentLlamaBackend::new(PersistentEngineConfig{
                    executable:engine.clone(),expected_sha256:engine_sha,
                    threads:NonZeroUsize::new(4).unwrap(),context_tokens:context,
                    startup_timeout:Duration::from_secs(600),request_timeout:Duration::from_secs(600),
                    api_key:format!("{}{}",crate::next_id(""),crate::next_id("")),load_observer:None,
                })
            }).unwrap();
        sup.attach_projector(&model_ref,projector).unwrap();
        let session=crate::next_id("s-");
        sup.load(&model_ref,session.clone(),"cp01-load").unwrap();
        assert!(sup.status().unwrap().pid.is_some(),"actual owned child required");
        let profile=std::env::temp_dir().join(crate::next_id("lu-cp01-real-"));
        let chats=crate::chat_store::ChatStore::open(&profile).unwrap();
        let store=AttachmentStore::open(&profile).unwrap();
        let queue=Arc::new(GenerationQueue::default());
        let colors=[("red",[245,9,9]),("blue",[9,9,245]),
            ("green",[9,200,9]),("yellow",[245,230,9])];
        let mut order=[0usize,1,2,3];order.shuffle(&mut OsRng);
        let mut rotated=order;rotated.rotate_left(1);
        let sets=vec![vec![order[0]],vec![order[1]],order.to_vec(),rotated.to_vec()];
        let prompt="Look at each attached image in attachment order. Reply only with the dominant color of each image, using red, blue, green or yellow. Separate the color words with commas.";
        for indexes in sets{
            let chat=chats.create_chat().unwrap().chat_id;
            let book=chats.require(&chat).unwrap();
            let turn=crate::next_id("g-");let request=crate::next_id("req-");
            let mut manifest=Vec::new();let mut images=Vec::new();
            for i in &indexes{
                let mut bytes=Cursor::new(Vec::new());
                DynamicImage::ImageRgb8(ImageBuffer::from_pixel(256,256,Rgb(colors[*i].1)))
                    .write_to(&mut bytes,ImageFormat::Png).unwrap();
                let image=admit_data_url(&format!("data:image/png;base64,{}",STANDARD.encode(bytes.into_inner()))).unwrap();
                let a=AttachmentStore::metadata(crate::next_id("img-"),"sample.png",&image,false);
                store.write_new(&chat,&a,&image).unwrap();
                book.admit_draft_image(&chat,&a).unwrap();
                manifest.push(a);images.push(image);
            }
            let (ticket,_)=queue.enqueue_with_class_committing(&session,&chat,&turn,
                TurnClass::Foreground,||chats.append_user_with_image_draft(&chat,&turn,prompt,&manifest)
                    .map(|_|())).unwrap();
            ticket.wait_for_slot().unwrap();
            store.verify_saved_exact(&chats,&chat,&turn,&manifest,&images).unwrap();
            sup.reserve_generation_images_with_history(&session,&turn,&request,prompt,96,&images,&[]).unwrap();
            let bytes:usize=images.iter().map(|p|p.bytes.len()).sum();
            let mut text=String::new();let mut report=None;
            sup.run_reserved_images_reported(&session,&turn,&request,prompt.into(),96,images,vec![],|wire|{
                let frame=decode_frame(&wire).unwrap();
                if let Some(proto::inference_frame::Body::Event(event))=frame.body{
                    if let Some(proto::inference_event::Kind::TextChunk(chunk))=event.kind{
                        assert!(text.len()+chunk.text.len()<=2048);
                        text.push_str(&chunk.text);
                    }
                }
                Ok(())
            },&mut report).expect("actual supervised GGUF/MMProj inference failed");
            let report=report.expect("actual persistent backend receipt required");
            assert_eq!(report.image_count,indexes.len());assert_eq!(report.decoded_bytes,bytes);
            assert!(report.payload_verified&&report.upload_completed&&report.visible_answer&&
                report.stream_completed&&report.worker_idle_verified);
            assert!(report.failure_code.is_none());
            let words=text.split(|c:char|!c.is_ascii_alphabetic()).filter(|s|!s.is_empty())
                .map(str::to_ascii_lowercase).collect::<Vec<_>>();
            let expected=indexes.iter().map(|i|colors[*i].0.to_owned()).collect::<Vec<_>>();
            println!("CP01_REAL_IMAGE count={} expected={expected:?} actual={words:?} receipt={report:?}",
                indexes.len());
            assert_eq!(words,expected,"actual model must identify every pixel-only color IN ORDER");
            ticket.commit_completed(&session,||{
                chats.append_progress(&chat,&turn,&text)?;
                chats.complete(&chat,&turn)
            }).unwrap();
            assert_eq!(book.saved_image_manifest(&chat,&turn).unwrap(),manifest);
            drop(ticket);
        }
        sup.unload(&session,"cp01-unload").unwrap();
        drop(store);drop(chats);
        fs::remove_dir_all(profile).unwrap();
    }
    #[test]
    fn durable_bytes_are_scope_bound_and_revalidated(){
        let path=std::env::temp_dir().join(crate::next_id("lu-cp01-store-"));
        fs::create_dir_all(path.join("ansf")).unwrap();
        let p=png();let chat=crate::next_id("c-");let other=crate::next_id("c-");
        let a=AttachmentStore::metadata(crate::next_id("img-"),"image.png",&p,false);
        {
            let store=AttachmentStore::open(&path).unwrap();
            store.write_new(&chat,&a,&p).unwrap();
            assert!(store.write_new(&chat,&a,&p).is_err());
            assert!(store.read(&other,&a).is_err());
        }
        {
            let store=AttachmentStore::open(&path).unwrap();
            assert_eq!(store.read(&chat,&a).unwrap(),p);
            fs::write(path.join("ansf/attachment-objects").join(name(&chat,&a.attachment_id).unwrap()),
                vec![0u8;p.bytes.len()]).unwrap();
            assert!(store.read(&chat,&a).is_err());
        }
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn orphan_reconciliation_retains_original_and_draft_objects(){
        let path=std::env::temp_dir().join(crate::next_id("lu-cp01-orphan-"));
        let p=png();
        let (chat,keep,orphan,saved);
        {
            let chats=crate::chat_store::ChatStore::open(&path).unwrap();
            chat=chats.create_chat().unwrap().chat_id;
            let store=AttachmentStore::open(&path).unwrap();
            let book=chats.require(&chat).unwrap();
            saved=AttachmentStore::metadata(crate::next_id("img-"),"saved.png",&p,false);
            store.write_new(&chat,&saved,&p).unwrap();
            book.admit_draft_image(&chat,&saved).unwrap();
            chats.append_user_with_image_draft(&chat,&crate::next_id("g-"),"Inspect", &[saved.clone()]).unwrap();

            keep=AttachmentStore::metadata(crate::next_id("img-"),"keep.png",&p,false);
            store.write_new(&chat,&keep,&p).unwrap();
            book.admit_draft_image(&chat,&keep).unwrap();
            orphan=AttachmentStore::metadata(crate::next_id("img-"),"orphan.png",&p,false);
            store.write_new(&chat,&orphan,&p).unwrap();
        }
        {
            let chats=crate::chat_store::ChatStore::open(&path).unwrap();
            let store=AttachmentStore::open(&path).unwrap();
            store.reconcile_orphans(&chats);
            assert_eq!(store.read(&chat,&keep).unwrap(),p);
            assert_eq!(store.read(&chat,&saved).unwrap(),p);
            assert!(store.read(&chat,&orphan).is_err());
        }
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn approved_observation_never_writes_pixels_and_expires_on_restart(){
        let path=std::env::temp_dir().join(crate::next_id("lu-cp01-private-"));
        fs::create_dir_all(path.join("ansf")).unwrap();
        let p=png();let chat=crate::next_id("c-");
        let a=AttachmentStore::metadata(crate::next_id("img-"),"Local observation",&p,true);
        {
            let store=AttachmentStore::open(&path).unwrap();
            assert!(store.write_new(&chat,&a,&p).is_err());
            store.register_observed(&chat,&a,p.clone()).unwrap();
            assert!(store.reject_live_observed_digest(&p).is_err());
            assert_eq!(store.read(&chat,&a).unwrap(),p);
            assert!(store.read(&crate::next_id("c-"),&a).is_err());
            assert_eq!(fs::read_dir(path.join("ansf/attachment-objects")).unwrap().count(),0);
        }
        assert!(AttachmentStore::open(&path).unwrap().read(&chat,&a).is_err());
        fs::remove_dir_all(path).unwrap();
    }
}
