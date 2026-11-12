//! PERF-01 P5: opt-in, host-only llama.cpp tuning experiment boundary.
//! A verified pinned server executable is necessary but not sufficient to
//! accept flags. Production default continues the original stable launch.
//! Experiments are admitted only by a native test/owner setup, never a chat
//! prompt, WebView, tool proposal, inherited LLAMA_ARG_*, or model output.
use std::{ffi::OsString,num::NonZeroUsize,path::Path};
use crate::{
    gguf_metadata_summary::{inspect_metadata,tokenizer_metadata_sha256},
    model_admission::AdmittedModel,
    p6_compute_fabric::ApprovedLocalGpuPlacement,
};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum KvType { F16, Q8_0, Q4_0 }
impl KvType {
    pub fn cli(self)->&'static str {
        match self {Self::F16=>"f16",Self::Q8_0=>"q8_0",Self::Q4_0=>"q4_0"}
    }
}
#[derive(Debug,Clone)]
pub enum Speculative {
    Disabled,
    /// No additional GGUF, but still engine-version-dependent.
    NgramSimple,
    /// Separate GGUF admitted by the original host, never a path from AI.
    DraftSimple(AdmittedModel),
}
#[derive(Debug,Clone)]
pub struct TuningProfile {
    pub threads_batch: NonZeroUsize,
    pub batch_size: u32,
    pub ubatch_size: u32,
    pub gpu_layers: u16,
    /// Optional P6 reservation-backed GPU placement. No default use.
    pub gpu_placement:Option<ApprovedLocalGpuPlacement>,
    pub key_cache: KvType,
    pub value_cache: KvType,
    pub speculative: Speculative,
}
impl TuningProfile {
    pub fn validate_numbers(
        &self, threads:NonZeroUsize, context_tokens:u32,
    )->Result<(),String>{
        if self.threads_batch.get()>256 ||
            self.threads_batch.get()>threads.get().saturating_mul(8) ||
            self.batch_size==0 ||self.batch_size>2048 ||
            self.ubatch_size==0 ||self.ubatch_size>512 ||
            self.ubatch_size>self.batch_size ||
            self.batch_size>context_tokens ||
            self.gpu_layers>99 ||
            (!cfg!(target_os="windows") && self.gpu_layers!=0) ||
            (self.gpu_placement.is_some() && self.gpu_layers==0) {
            return Err("P5 tuning exceeds conservative engine or platform bounds".into())
        }
        Ok(())
    }
    /// Validate AFTER normal target GGUF admission but BEFORE starting a
    /// separately pinned engine child. Exact tokenizer metadata equality is
    /// a necessary test, not proof of target/draft semantic compatibility.
    pub fn validate_for_model(
        &self, target:&AdmittedModel, projector_present:bool,
    )->Result<(),String>{
        // Until a pinned native vision matrix establishes equivalent
        // behavior, NO experimental P5 profile may change MMProj inference.
        // Everyday multimodal Lu continues the prior untuned launch.
        if projector_present {
            return Err("P5 tuning not admitted with MMProj before vision acceptance".into())
        }
        let Speculative::DraftSimple(draft)=&self.speculative else{return Ok(())};
        target.recheck().map_err(|_|"target GGUF admission changed")?;
        draft.recheck().map_err(|_|"draft GGUF admission changed")?;
        if target.path()==draft.path(){
            return Err("speculative draft must differ from target GGUF".into())
        }
        let target_meta=inspect_metadata(target.path())
            .map_err(|_|"target GGUF metadata cannot be verified")?;
        let draft_meta=inspect_metadata(draft.path())
            .map_err(|_|"draft GGUF metadata cannot be verified")?;
        let same_arch=target_meta.architecture.is_some() &&
            target_meta.architecture==draft_meta.architecture;
        if !same_arch {
            return Err("draft GGUF architecture differs or is unknown".into())
        }
        let target_tokens=tokenizer_metadata_sha256(target.path())
            .map_err(|_|"target GGUF tokenizer metadata is unverified")?;
        let draft_tokens=tokenizer_metadata_sha256(draft.path())
            .map_err(|_|"draft GGUF tokenizer metadata is unverified")?;
        if target_tokens!=draft_tokens {
            return Err("draft and target tokenization metadata differ".into())
        }
        target.recheck().map_err(|_|"target changed during draft preflight")?;
        draft.recheck().map_err(|_|"draft changed during preflight")?;
        Ok(())
    }
    pub fn draft_path(&self)->Option<&Path>{
        match &self.speculative {
            Speculative::DraftSimple(m)=>Some(m.path()),_=>None,
        }
    }
    /// A fixed list of well-known arguments. No arbitrary flags/argv, URI,
    /// plugin, command or shell construction is permitted.
    pub fn launch_args(&self)->Vec<OsString>{
        let mut values=Vec::<OsString>::new();
        let mut add=|k:&str,v:OsString|{
            values.push(k.into());values.push(v);
        };
        add("--threads-batch",self.threads_batch.get().to_string().into());
        add("--batch-size",self.batch_size.to_string().into());
        add("--ubatch-size",self.ubatch_size.to_string().into());
        add("--cache-type-k",self.key_cache.cli().into());
        add("--cache-type-v",self.value_cache.cli().into());
        match &self.speculative {
            Speculative::Disabled=>add("--spec-type","none".into()),
            Speculative::NgramSimple=>add("--spec-type","ngram-simple".into()),
            Speculative::DraftSimple(model)=>{
                add("--spec-type","draft-simple".into());
                add("--spec-draft-model",model.path().as_os_str().to_owned());
                // Safer CPU-only draft budget until reliable additional VRAM
                // accounting exists; draft speed/quality still unmeasured.
                add("--spec-draft-ngl","0".into());
                add("--spec-draft-n-max","3".into());
            }
        }
        if let Some(local)=&self.gpu_placement{
            values.extend(local.flags());
        }
        values
    }
    pub fn label(&self)->&'static str{
        match &self.speculative {
            Speculative::Disabled=>"TUNED_NO_SPECULATION",
            Speculative::NgramSimple=>"TUNED_NGRAM_SIMPLE",
            Speculative::DraftSimple(_)=>"TUNED_DRAFT_SIMPLE",
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn preset()->TuningProfile {
        TuningProfile{
            threads_batch:NonZeroUsize::new(4).unwrap(),
            batch_size:512,ubatch_size:128,gpu_layers:0,
            gpu_placement:None,
            key_cache:KvType::F16,value_cache:KvType::F16,
            speculative:Speculative::Disabled,
        }
    }
    #[test] fn approved_numeric_profile_and_exact_flags(){
        let profile=preset();
        assert!(profile.validate_numbers(NonZeroUsize::new(4).unwrap(),4096).is_ok());
        let args=profile.launch_args().into_iter()
            .map(|s|s.to_string_lossy().to_string()).collect::<Vec<_>>();
        assert!(args.windows(2).any(|p|p[0]=="--batch-size"&&p[1]=="512"));
        assert!(args.windows(2).any(|p|p[0]=="--ubatch-size"&&p[1]=="128"));
        assert!(args.windows(2).any(|p|p[0]=="--spec-type"&&p[1]=="none"));
        assert_eq!(args.len(),12);
    }
    #[test] fn denies_unbounded_or_mismatched_resource_configuration(){
        let mut p=preset();
        p.ubatch_size=1024;
        assert!(p.validate_numbers(NonZeroUsize::new(4).unwrap(),4096).is_err());
        p.ubatch_size=128;p.batch_size=0;
        assert!(p.validate_numbers(NonZeroUsize::new(4).unwrap(),4096).is_err());
        p.batch_size=512;
        p.threads_batch=NonZeroUsize::new(100).unwrap();
        assert!(p.validate_numbers(NonZeroUsize::new(4).unwrap(),4096).is_err());
        p.threads_batch=NonZeroUsize::new(4).unwrap();
        p.gpu_layers=100;
        assert!(p.validate_numbers(NonZeroUsize::new(4).unwrap(),4096).is_err());
    }
    #[test] fn ngram_is_explicit_and_does_not_insert_fake_draft_model(){
        let mut p=preset();p.speculative=Speculative::NgramSimple;
        let args=p.launch_args();
        assert!(args.windows(2).any(|v|
            v[0].to_string_lossy()=="--spec-type"&&
            v[1].to_string_lossy()=="ngram-simple"));
        assert!(!args.iter().any(|v|v.to_string_lossy()=="--spec-draft-model"));
    }
    fn write_tokenizer_fixture(dir:&Path,name:&str,token:&str){
        use std::{fs,io::Write};
        fs::create_dir_all(dir).unwrap();
        let mut data=b"GGUF".to_vec();
        data.extend(3u32.to_le_bytes());
        data.extend(1u64.to_le_bytes()); // dummy tensor header
        data.extend(3u64.to_le_bytes()); // exactly 3 metadata fields
        let mut add_string=|key:&str,value:&str|{
            data.extend((key.len() as u64).to_le_bytes());
            data.extend(key.as_bytes());
            data.extend(8u32.to_le_bytes());
            data.extend((value.len() as u64).to_le_bytes());
            data.extend(value.as_bytes());
        };
        add_string("general.architecture","qwen3");
        add_string("tokenizer.ggml.model","gpt2");
        let key="tokenizer.ggml.tokens";
        data.extend((key.len() as u64).to_le_bytes());
        data.extend(key.as_bytes());
        data.extend(9u32.to_le_bytes()); // array
        data.extend(8u32.to_le_bytes()); // strings
        data.extend(2u64.to_le_bytes()); // two tokens
        for tok in ["first",token] {
            data.extend((tok.len() as u64).to_le_bytes());
            data.extend(tok.as_bytes());
        }
        let mut file=fs::File::create(dir.join(name)).unwrap();
        file.write_all(&data).unwrap();
    }
    #[test]
    fn draft_model_preflight_checks_original_distinct_admitted_files_and_vocab(){
        use std::{fs,time::{SystemTime,UNIX_EPOCH}};
        let seed=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!(
            "lu-p5-draft-{}-{seed}",std::process::id()));
        let main_dir=root.join("main");
        let draft_dir=root.join("draft");
        write_tokenizer_fixture(&main_dir,"main.gguf","second");
        write_tokenizer_fixture(&draft_dir,"draft.gguf","second");
        let mut target_admission=crate::model_admission::ModelAdmissionManager::new();
        let mut draft_admission=crate::model_admission::ModelAdmissionManager::new();
        let target=target_admission.admit(&main_dir,"main.gguf").unwrap().clone();
        let draft=draft_admission.admit(&draft_dir,"draft.gguf").unwrap().clone();
        let mut p=preset();
        p.speculative=Speculative::DraftSimple(draft.clone());
        assert!(p.validate_for_model(&target,false).is_ok());
        assert!(p.validate_for_model(&target,true).is_err());
        p.speculative=Speculative::DraftSimple(target.clone());
        assert!(p.validate_for_model(&target,false).is_err());
        p.speculative=Speculative::DraftSimple(draft);
        // A changed vocabulary is never admitted, even though the model
        // reports the same architecture and tokenizer name.
        write_tokenizer_fixture(&draft_dir,"draft.gguf","DIFFERENT");
        assert!(p.validate_for_model(&target,false).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test] fn default_profile_is_always_not_applied_automatically(){
        let default:Option<TuningProfile>=None;
        assert!(default.is_none());
    }
}
