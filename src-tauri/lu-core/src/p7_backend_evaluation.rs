//! PERF-01 P7: evaluation-only alternative inference backend boundary.
//! Source-level preflight and model-neutral conformance, NOT a second runnable
//! production backend. No network, process spawn, tool approval, backend
//! selection, or automatic installation. The original pinned llama.cpp
//! Vulkan/CPU runtime remains the sole normal installed inference path.
//!
//! A native candidate must separately implement the original InferenceBackend
//! trait, including trusted process custody and protocol parsing, before its
//! wrapper can be used for actual GGUF evaluation. API compatibility or vendor
//! capability labels are never equivalent to executable verification.

use crate::{
    inference_host::{Cancellation,EngineError,GenerationRequest,InferenceBackend,TokenChunk},
    mmproj_admission::AdmittedProjector,
    model_admission::AdmittedModel,
    multimodal_saved_turn::PinnedImage,
};
use sha2::{Digest,Sha256};
use std::{
    fs::{self,File},
    io::Read,
    path::{Path,PathBuf},
    time::SystemTime,
};

const MAX_EVALUATION_BINARY_BYTES:u64=1024*1024*1024;
const MAX_EVALUATION_GGUF_BYTES:u64=128*1024*1024*1024;
const MAX_EVALUATION_MMProj_BYTES:u64=16*1024*1024*1024;
const MAX_OUTPUT_CHUNK_BYTES:usize=64*1024;
const MAX_OUTPUT_BYTES:usize=1024*1024;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum CandidateKind {
    MistralRs,
    KoboldCpp,
    VllmGguf,
}
impl CandidateKind {
    pub fn id(self)->&'static str {
        match self {
            Self::MistralRs=>"mistral_rs",
            Self::KoboldCpp=>"koboldcpp",
            Self::VllmGguf=>"vllm_gguf",
        }
    }
    /// Research candidates, not a claim that a user's exact GGUF loads.
    pub fn needs_separate_gguf_plugin(self)->bool {
        matches!(self,Self::VllmGguf)
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum CandidateBlocker {
    Unreviewed,InvalidDigest,UnsafeFile,ChangedArtifact,
    ModelMismatch,ProjectorMismatch,IncompleteProtocolEvidence,
    VisionUnavailable,ToolSchemaUnavailable,Cancelled,
    OutputViolation,ExperimentalOnly,
}
#[derive(Clone,Debug)]
pub struct CandidateManifest {
    pub kind:CandidateKind,
    pub executable:PathBuf,
    pub expected_executable_sha256:[u8;32],
    pub expected_model_sha256:[u8;32],
    pub expected_projector_sha256:Option<[u8;32]>,
    /// Independent owner review is required. It is not executable authority.
    pub operator_reviewed:bool,
    pub local_only:bool,
}
/// The original, exact bytes were opened and hashed. This is NOT evidence
/// that a candidate can safely spawn: dependency/package verification,
/// containment and engine-specific startup contracts remain separate.
pub struct VerifiedCandidate {
    pub kind:CandidateKind,
    executable_path:PathBuf,
    model_path:PathBuf,
    projector_path:Option<PathBuf>,
    _pinned_executable:File,
    _pinned_model:File,
    _pinned_projector:Option<File>,
    engine_sha256:[u8;32],
    model_sha256:[u8;32],
}
impl VerifiedCandidate {
    pub fn engine_sha256(&self)->[u8;32]{self.engine_sha256}
    pub fn model_sha256(&self)->[u8;32]{self.model_sha256}
    pub fn executable_path(&self)->&Path{&self.executable_path}
}
fn nonzero_sha(sha:&[u8;32])->bool {
    sha.iter().any(|value|*value!=0)
}
fn digest_original_file(
    path:&Path,max_bytes:u64,
)->Result<(File,PathBuf,[u8;32]),CandidateBlocker>{
    let meta=fs::symlink_metadata(path).map_err(|_|CandidateBlocker::UnsafeFile)?;
    if !meta.file_type().is_file()||meta.len()==0||meta.len()>max_bytes {
        return Err(CandidateBlocker::UnsafeFile)
    }
    let canonical=path.canonicalize().map_err(|_|CandidateBlocker::UnsafeFile)?;
    let mut file=File::open(&canonical).map_err(|_|CandidateBlocker::UnsafeFile)?;
    let opened=file.metadata().map_err(|_|CandidateBlocker::UnsafeFile)?;
    if !opened.file_type().is_file()||opened.len()!=meta.len(){
        return Err(CandidateBlocker::ChangedArtifact)
    }
    let original_modified:Option<SystemTime>=opened.modified().ok();
    let mut digest=Sha256::new();
    let mut buffer=[0u8;128*1024];
    let mut n_total=0u64;
    loop {
        let count=file.read(&mut buffer).map_err(|_|CandidateBlocker::ChangedArtifact)?;
        if count==0 {break}
        n_total=n_total.saturating_add(count as u64);
        if n_total>max_bytes{return Err(CandidateBlocker::UnsafeFile)}
        digest.update(&buffer[..count]);
    }
    let after=file.metadata().map_err(|_|CandidateBlocker::ChangedArtifact)?;
    if n_total!=meta.len()||after.len()!=meta.len()||
        after.modified().ok()!=original_modified{
        return Err(CandidateBlocker::ChangedArtifact)
    }
    Ok((file,canonical,digest.finalize().into()))
}
impl CandidateManifest {
    pub fn preflight_for_model(
        &self,admitted:&AdmittedModel,
    )->Result<VerifiedCandidate,CandidateBlocker>{
        self.preflight_for_model_and_projector(admitted,None)
    }
    pub fn preflight_for_model_and_projector(
        &self,admitted:&AdmittedModel,projector:Option<&AdmittedProjector>,
    )->Result<VerifiedCandidate,CandidateBlocker>{
        if projector.is_some()!=self.expected_projector_sha256.is_some(){
            return Err(CandidateBlocker::ProjectorMismatch)
        }
        if !self.operator_reviewed||!self.local_only {
            return Err(CandidateBlocker::Unreviewed)
        }
        if !nonzero_sha(&self.expected_executable_sha256)||
            !nonzero_sha(&self.expected_model_sha256){
            return Err(CandidateBlocker::InvalidDigest)
        }
        admitted.recheck().map_err(|_|CandidateBlocker::ChangedArtifact)?;
        let (exe,canonical,sha)=digest_original_file(
            &self.executable,MAX_EVALUATION_BINARY_BYTES)?;
        if sha!=self.expected_executable_sha256{
            return Err(CandidateBlocker::InvalidDigest)
        }
        let (model,model_path,model_sha)=digest_original_file(
            admitted.path(),MAX_EVALUATION_GGUF_BYTES)?;
        if model_sha!=self.expected_model_sha256{
            return Err(CandidateBlocker::ModelMismatch)
        }
        let (projector_file,projector_path)=if let Some(p)=projector {
            p.recheck().map_err(|_|CandidateBlocker::ChangedArtifact)?;
            let expected=self.expected_projector_sha256
                .ok_or(CandidateBlocker::ProjectorMismatch)?;
            if !nonzero_sha(&expected){
                return Err(CandidateBlocker::InvalidDigest)
            }
            let (file,path,actual)=digest_original_file(
                p.path(),MAX_EVALUATION_MMProj_BYTES)?;
            if actual!=expected{return Err(CandidateBlocker::ProjectorMismatch)}
            p.recheck().map_err(|_|CandidateBlocker::ChangedArtifact)?;
            (Some(file),Some(path))
        }else{
            (None,None)
        };
        admitted.recheck().map_err(|_|CandidateBlocker::ChangedArtifact)?;
        Ok(VerifiedCandidate{
            kind:self.kind,executable_path:canonical,model_path,
            projector_path,
            _pinned_executable:exe,_pinned_model:model,
            _pinned_projector:projector_file,
            engine_sha256:sha,model_sha256:model_sha,
        })
    }
}

/// These flags describe observations from original native conformance tests,
/// never vendor documentation or the executable's self-reported capability.
/// Every selected surface needs an independently recorded exact-head test
/// before it can enter comparative evaluation. They do not create authority.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct NativeConformance {
    pub exact_pinned_process_custody:bool,
    pub authenticated_loopback_only:bool,
    pub deterministic_gguf_load:bool,
    pub bounded_chat_sse:bool,
    pub role_history_isolation:bool,
    pub real_cancel_and_reap:bool,
    pub restart_reconciliation:bool,
    pub gguf_mmproj_vision:bool,
    pub structured_read_tool_calls:bool,
    pub approved_one_use_tool_boundary:bool,
    pub protected_kv_identity:bool,
}
impl NativeConformance {
    pub fn text_evaluation_ready(self)->bool {
        self.exact_pinned_process_custody&&
        self.authenticated_loopback_only&&
        self.deterministic_gguf_load&&
        self.bounded_chat_sse&&
        self.role_history_isolation&&
        self.real_cancel_and_reap&&
        self.restart_reconciliation
    }
    pub fn vision_evaluation_ready(self)->bool {
        self.text_evaluation_ready()&&self.gguf_mmproj_vision
    }
    pub fn tool_evaluation_ready(self)->bool {
        self.text_evaluation_ready()&&
        self.structured_read_tool_calls&&
        self.approved_one_use_tool_boundary
    }
    /// No alternative is promoted from operator declarations or unit tests.
    /// An independent installed/native owner acceptance gate remains absent.
    pub fn production_selectable(self)->bool{false}
}

/// An evaluation guard over an independently implemented original backend.
/// This guard is NOT itself a concrete mistral.rs/KoboldCpp/vLLM adapter.
/// Underlying process launch/authenticated streaming must still be proven.
pub struct EvaluatedInferenceBackend<B:InferenceBackend>{
    native:B,
    candidate:VerifiedCandidate,
    proof:NativeConformance,
    loaded:bool,
}
impl<B:InferenceBackend> EvaluatedInferenceBackend<B>{
    pub fn new(
        native:B,candidate:VerifiedCandidate,proof:NativeConformance,
    )->Result<Self,CandidateBlocker>{
        if !proof.text_evaluation_ready(){
            return Err(CandidateBlocker::IncompleteProtocolEvidence)
        }
        Ok(Self{native,candidate,proof,loaded:false})
    }
    pub fn candidate_id(&self)->&'static str{self.candidate.kind.id()}
    /// Explicit text-only comparative evaluation may use a candidate even
    /// when vision/tool parity is unproven. It can never replace the normal
    /// multimodal/tool-capable Lu production runtime.
    pub fn candidate_can_replace_production(&self)->bool{false}
    fn stream_checked(
        &mut self,request:&GenerationRequest,cancel:&Cancellation,
        emit:&mut dyn FnMut(TokenChunk)->Result<(),EngineError>,
        images:Option<&[PinnedImage]>,
    )->Result<(),String>{
        if !self.loaded{return Err("P7 candidate not loaded".into())}
        if cancel.is_cancelled(){return Err("P7 candidate cancelled".into())}
        let mut expected=0u32;
        let mut total=0usize;
        let mut invalid=false;
        let mut checked=|chunk:TokenChunk|->Result<(),EngineError>{
            if cancel.is_cancelled(){return Err(EngineError::Cancelled)}
            total=total.saturating_add(chunk.text.len());
            if chunk.index!=expected||
                chunk.text.is_empty()||
                chunk.text.len()>MAX_OUTPUT_CHUNK_BYTES||
                total>MAX_OUTPUT_BYTES||
                chunk.text.contains('\0'){
                invalid=true;
                return Err(EngineError::InvalidRequest)
            }
            expected=expected.checked_add(1).ok_or(EngineError::InvalidRequest)?;
            emit(chunk)
        };
        let result=if let Some(images)=images{
            self.native.generate_with_images(request,images,cancel,&mut checked)
        }else{
            self.native.generate(request,cancel,&mut checked)
        };
        if invalid{return Err("P7 candidate violates bounded stream order".into())}
        if cancel.is_cancelled(){return Err("P7 candidate cancelled".into())}
        result?;
        if expected==0{return Err("P7 candidate emitted no visible tokens".into())}
        Ok(())
    }
}
impl<B:InferenceBackend> InferenceBackend for EvaluatedInferenceBackend<B>{
    fn process_id(&self)->Option<u32>{self.native.process_id()}
    fn load(&mut self,model:&AdmittedModel)->Result<(),String>{
        if self.loaded {return Err("P7 candidate already loaded".into())}
        if self.candidate.projector_path.is_some() ||
            model.path()!=self.candidate.model_path.as_path() ||
            model.recheck().is_err(){
            return Err("P7 candidate GGUF differs from pinned evaluation model".into())
        }
        self.native.load(model)?;
        self.loaded=true;
        Ok(())
    }
    fn load_with_projector(
        &mut self,model:&AdmittedModel,projector:Option<&AdmittedProjector>
    )->Result<(),String>{
        if projector.is_some()&&!self.proof.vision_evaluation_ready(){
            return Err("P7 candidate MMProj not independently accepted".into())
        }
        if self.loaded {return Err("P7 candidate already loaded".into())}
        if model.path()!=self.candidate.model_path.as_path() ||
            model.recheck().is_err(){
            return Err("P7 candidate GGUF differs from pinned evaluation model".into())
        }
        if projector.map(|p|p.path())!=self.candidate.projector_path.as_deref()||
            projector.is_some_and(|p|p.recheck().is_err()){
            return Err("P7 MMProj differs from pinned candidate evaluation".into())
        }
        self.native.load_with_projector(model,projector)?;
        self.loaded=true;
        Ok(())
    }
    fn unload(&mut self){
        if self.loaded{
            self.native.unload();
            self.loaded=false;
        }
    }
    fn invalidate_prefix_cache(&mut self){self.native.invalidate_prefix_cache()}
    fn set_trusted_cache_scope(&mut self,scope:Option<&str>){
        if self.proof.protected_kv_identity {
            self.native.set_trusted_cache_scope(scope)
        }else{
            self.native.set_trusted_cache_scope(None)
        }
    }
    fn generate(
        &mut self,request:&GenerationRequest,cancel:&Cancellation,
        emit:&mut dyn FnMut(TokenChunk)->Result<(),EngineError>
    )->Result<(),String>{
        self.stream_checked(request,cancel,emit,None)
    }
    fn generate_with_images(
        &mut self,request:&GenerationRequest,images:&[PinnedImage],
        cancel:&Cancellation,
        emit:&mut dyn FnMut(TokenChunk)->Result<(),EngineError>,
    )->Result<(),String>{
        if !self.proof.vision_evaluation_ready() {
            return Err("P7 vision capability not verified; no text fallback".into())
        }
        self.stream_checked(request,cancel,emit,Some(images))
    }
    fn generate_browser_function_calls(
        &mut self,request:&GenerationRequest,
        admitted_read_schemas:&[serde_json::Value],
        cancel:&Cancellation,
    )->Result<Vec<crate::browser_tool_stream::CompletedToolCall>,String>{
        if !self.proof.tool_evaluation_ready(){
            return Err("P7 tool-call parity absent; model text is not a tool".into())
        }
        if !self.loaded{return Err("P7 candidate not loaded".into())}
        if cancel.is_cancelled(){return Err("P7 candidate cancelled".into())}
        self.native.generate_browser_function_calls(
            request,admitted_read_schemas,cancel
        )
    }
}
impl<B:InferenceBackend> Drop for EvaluatedInferenceBackend<B>{
    fn drop(&mut self){self.unload()}
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::{io::Write,sync::{Arc,atomic::{AtomicUsize,Ordering}}};
    #[derive(Clone)]
    struct Mock(Arc<AtomicUsize>,bool);
    impl InferenceBackend for Mock{
        fn load(&mut self,_:&AdmittedModel)->Result<(),String>{Ok(())}
        fn unload(&mut self){self.0.fetch_add(1,Ordering::SeqCst);}
        fn generate(
            &mut self,_:&GenerationRequest,_:&Cancellation,
            emit:&mut dyn FnMut(TokenChunk)->Result<(),EngineError>
        )->Result<(),String>{
            emit(TokenChunk{text:"first".into(),index:0})
                .map_err(|_|"P7 sink refused".to_owned())?;
            emit(TokenChunk{text:"second".into(),index:if self.1{0}else{1}})
                .map_err(|_|"P7 sink refused".to_owned())
        }
    }
    fn fixture()->(std::path::PathBuf,AdmittedModel,CandidateManifest){
        let root=std::env::temp_dir().join(format!(
            "lu-p7-{}-{:?}",std::process::id(),std::thread::current().id()
        ));
        fs::create_dir_all(&root).unwrap();
        let model_path=root.join("tiny.gguf");
        let mut model=File::create(&model_path).unwrap();
        model.write_all(b"GGUF").unwrap();
        model.write_all(&3u32.to_le_bytes()).unwrap();
        model.write_all(&1u64.to_le_bytes()).unwrap();
        model.write_all(&1u64.to_le_bytes()).unwrap();
        drop(model);
        let exe=root.join("candidate.bin");
        fs::write(&exe,b"pinned fixture; not an executable candidate").unwrap();
        let mut catalog=crate::model_admission::ModelAdmissionManager::new();
        let admitted=catalog.admit(&root,"tiny.gguf").unwrap().clone();
        let sha_exe: [u8;32]=Sha256::digest(fs::read(&exe).unwrap()).into();
        let sha_model:[u8;32]=Sha256::digest(fs::read(&model_path).unwrap()).into();
        let reviewed=CandidateManifest{
            kind:CandidateKind::MistralRs,executable:exe,
            expected_executable_sha256:sha_exe,
            expected_model_sha256:sha_model,
            expected_projector_sha256:None,
            operator_reviewed:true,local_only:true
        };
        (root,admitted,reviewed)
    }
    fn text_proof()->NativeConformance{
        NativeConformance{
            exact_pinned_process_custody:true,
            authenticated_loopback_only:true,
            deterministic_gguf_load:true,
            bounded_chat_sse:true,
            role_history_isolation:true,
            real_cancel_and_reap:true,
            restart_reconciliation:true,
            ..Default::default()
        }
    }
    #[test] fn no_candidate_is_promoted_from_documentation(){
        assert!(!NativeConformance::default().text_evaluation_ready());
        assert!(!text_proof().production_selectable());
        assert!(CandidateKind::VllmGguf.needs_separate_gguf_plugin());
        assert_eq!(CandidateKind::MistralRs.id(),"mistral_rs");
    }
    #[test] fn exact_executable_and_model_sha_are_required(){
        let (dir,admitted,mut manifest)=fixture();
        let verified=manifest.preflight_for_model(&admitted).unwrap();
        assert_eq!(verified.kind,CandidateKind::MistralRs);
        assert_eq!(verified.model_sha256(),manifest.expected_model_sha256);
        manifest.expected_executable_sha256=[0;32];
        assert!(matches!(manifest.preflight_for_model(&admitted),
            Err(CandidateBlocker::InvalidDigest)));
        manifest.expected_executable_sha256=verified.engine_sha256();
        manifest.expected_model_sha256=[1;32];
        assert!(matches!(manifest.preflight_for_model(&admitted),
            Err(CandidateBlocker::ModelMismatch)));
        manifest.operator_reviewed=false;
        assert!(matches!(manifest.preflight_for_model(&admitted),
            Err(CandidateBlocker::Unreviewed)));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn projector_hash_is_required_for_multimodal_evaluation(){
        let (dir,admitted,mut manifest)=fixture();
        let path=dir.join("mmproj.gguf");
        let mut header=b"GGUF".to_vec();
        header.extend(3u32.to_le_bytes());
        header.extend(1u64.to_le_bytes());
        header.extend(1u64.to_le_bytes());
        fs::write(&path,&header).unwrap();
        let projector=crate::mmproj_admission::admit_projector(
            &dir,"mmproj.gguf"
        ).unwrap();
        let digest:[u8;32]=Sha256::digest(&header).into();
        manifest.expected_projector_sha256=Some(digest);
        assert!(matches!(manifest.preflight_for_model(&admitted),
            Err(CandidateBlocker::ProjectorMismatch)));
        let reviewed=manifest.preflight_for_model_and_projector(
            &admitted,Some(&projector)).unwrap();
        assert_eq!(reviewed.projector_path.as_deref(),Some(path.as_path()));
        let mut wrong=manifest.clone();
        wrong.expected_projector_sha256=Some([9;32]);
        assert!(matches!(wrong.preflight_for_model_and_projector(
            &admitted,Some(&projector)),
            Err(CandidateBlocker::ProjectorMismatch)));
        fs::write(&path,b"invalid changed image projector").unwrap();
        assert!(manifest.preflight_for_model_and_projector(
            &admitted,Some(&projector)).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn candidate_text_stream_is_bounded_and_vision_and_tools_fail_closed(){
        let (dir,admitted,manifest)=fixture();
        let proof=text_proof();
        let verified=manifest.preflight_for_model(&admitted).unwrap();
        assert!(EvaluatedInferenceBackend::new(
            Mock(Arc::new(AtomicUsize::new(0)),false),
            verified,NativeConformance::default()
        ).is_err());
        let verified=manifest.preflight_for_model(&admitted).unwrap();
        let unloads=Arc::new(AtomicUsize::new(0));
        let mut backend=EvaluatedInferenceBackend::new(
            Mock(Arc::clone(&unloads),false),verified,proof
        ).unwrap();
        assert!(!backend.candidate_can_replace_production());
        backend.load(&admitted).unwrap();
        let request=GenerationRequest{
            prompt:"hello".into(),history:Vec::new(),max_new_tokens:32
        };
        let mut texts=Vec::new();
        backend.generate(&request,&Cancellation::default(),&mut|chunk|{
            texts.push(chunk.text);Ok(())
        }).unwrap();
        assert_eq!(texts,vec!["first","second"]);
        assert!(backend.generate_with_images(&request,&[],&Cancellation::default(),
            &mut |_|Ok(())).is_err());
        assert!(backend.generate_browser_function_calls(
            &request,&[],&Cancellation::default()).is_err());
        let canceled=Cancellation::default();canceled.cancel();
        assert!(backend.generate(&request,&canceled,&mut |_|Ok(())).is_err());
        backend.unload();
        assert!(unloads.load(Ordering::SeqCst)>=1);
        let bad=manifest.preflight_for_model(&admitted).unwrap();
        let mut backend=EvaluatedInferenceBackend::new(
            Mock(Arc::new(AtomicUsize::new(0)),true),bad,proof
        ).unwrap();
        backend.load(&admitted).unwrap();
        assert!(backend.generate(&request,&Cancellation::default(),
            &mut |_|Ok(())).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
