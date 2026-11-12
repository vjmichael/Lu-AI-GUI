//! PERF-01 P5: opt-in, host-only llama.cpp tuning experiment boundary.
//! A verified pinned server executable is necessary but not sufficient to
//! accept flags. Production default continues the original stable launch.
//! Experiments are admitted only by a native test/owner setup, never a chat
//! prompt, WebView, tool proposal, inherited LLAMA_ARG_*, or model output.
use std::{ffi::OsString,num::NonZeroUsize,path::Path};
use crate::{
    gguf_metadata_summary::{inspect_metadata,tokenizer_metadata_sha256},
    model_admission::AdmittedModel,
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
            (!cfg!(target_os="windows") && self.gpu_layers!=0) {
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
        if projector_present && !matches!(self.speculative,Speculative::Disabled){
            return Err("speculative decoding not admitted with MMProj".into())
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
        values
    }
    pub fn label(&self)->&'static str{
        match self.speculative {
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
            key_cache:KvType::F16,value_cache:KvType::F16,
            speculative:Speculative::Disabled,
        }
    }
    #[test] fn approved_numeric_profile_and_exact_flags(){
        let profile=preset();
        assert!(profile.validate_numbers(NonZeroUsize::new(4).unwrap(),4096).is_ok());
        let args=profile.launch_args().into_iter()
            .map(|s|s.to_string_lossy().to_string()).collect::<Vec<_>>();
        assert!(args.windows(2).any(|p|p==["--batch-size","512"]));
        assert!(args.windows(2).any(|p|p==["--ubatch-size","128"]));
        assert!(args.windows(2).any(|p|p==["--spec-type","none"]));
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
            v[0]=="--spec-type"&&v[1]=="ngram-simple"));
        assert!(!args.iter().any(|v|v=="--spec-draft-model"));
    }
    #[test] fn default_profile_is_always_not_applied_automatically(){
        let default:Option<TuningProfile>=None;
        assert!(default.is_none());
    }
}
