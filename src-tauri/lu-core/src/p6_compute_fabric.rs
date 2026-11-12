//! PERF-01 P6: host-only local GPU scheduling and remote-node admission.
//! Does not replace the original one-session Lu inference supervisor.
//! No remote RPC, automatic discovery or GPU memory claims are made here.
//! Local VRAM admission requires a FRESH measured device record supplied by
//! an independently trusted native probe; no such probe is enabled in the
//! normal installed desktop, so the default path remains one-GPU/offline.
use std::{
    collections::{BTreeMap,BTreeSet},
    ffi::OsString,
    sync::Mutex,
    time::{SystemTime,UNIX_EPOCH},
};

pub const MAX_LOCAL_DEVICES:usize=2;
pub const MAX_LOCAL_RESERVATIONS:usize=2;
pub const MAX_OBSERVATION_AGE_SECS:u64=5;
const VRAM_HEADROOM_MIB:u64=512;

fn now_unix()->u64{
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0,|x|x.as_secs())
}
fn valid_host_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=96&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn valid_device_name(s:&str)->bool{
    !s.is_empty()&&s.len()<=64&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'|b'.'|b':'|b'/'))
        && !s.contains("..") && s!="none"
}
fn nonzero_digest(digest:[u8;32])->bool{digest.iter().any(|b|*b!=0)}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ComputeMode {
    /// Exactly one model child bound to one native GPU.
    SingleDevice,
    /// One llama-server child splits contiguous model layers between GPUs.
    LayerSplit,
    /// Separate supervised model children, one per selected local GPU.
    IndependentInstance,
}
#[derive(Debug,Clone)]
pub struct GpuObservation{
    pub device_name:String, // exact name from a pinned engine --list-devices
    pub total_vram_mib:u64,
    pub available_vram_mib:Option<u64>,
    pub observed_at_unix:u64,
    pub pinned_engine_sha256:[u8;32],
    pub native_memory_probe_verified:bool,
}
impl GpuObservation{
    fn validate(&self,engine:[u8;32],now:u64)->Result<(),GpuScheduleError>{
        if !valid_device_name(&self.device_name) ||
            !self.native_memory_probe_verified ||
            self.pinned_engine_sha256!=engine ||
            self.total_vram_mib==0 ||
            now<self.observed_at_unix ||
            now-self.observed_at_unix>MAX_OBSERVATION_AGE_SECS {
            return Err(GpuScheduleError::UnverifiedDevice)
        }
        let available=self.available_vram_mib.ok_or(GpuScheduleError::UnverifiedVram)?;
        if available>self.total_vram_mib {
            return Err(GpuScheduleError::UnverifiedVram)
        }
        Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum GpuScheduleError{
    InvalidRequest,NoDeviceEvidence,UnverifiedDevice,UnverifiedVram,
    InsufficientVram,Busy,WrongSession,NotFound,StaleObservation,
    ProcessCustodyUncertain,Poisoned,RemoteTransportDisabled,
}
#[derive(Debug,Clone)]
pub struct GpuRequest{
    pub reservation_id:String,
    pub session_id:String,
    pub mode:ComputeMode,
    /// Exact engine-visible device names with independently witnessed peak
    /// bytes for each selected device. NEVER infer these from GGUF file size.
    pub devices:Vec<String>,
    pub measured_required_vram_mib:Vec<u64>,
    /// For layer split the fraction for each selected device, summing to 100.
    pub tensor_split_percent:Option<Vec<u8>>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
struct Lease {
    session:String,
    devices:Vec<(String,u64)>,
}
#[derive(Debug)]
struct GpuState{
    engine_sha256:[u8;32],
    observations:BTreeMap<String,GpuObservation>,
    leases:BTreeMap<String,Lease>,
}
/// Reservation ledger is host-owned. An absent native probe has no fabricated
/// free memory; no speculative GPU admission is made during offline startup.
pub struct LocalGpuCoordinator{inner:Mutex<GpuState>}
impl LocalGpuCoordinator {
    pub fn new(engine_sha256:[u8;32])->Result<Self,GpuScheduleError>{
        if !nonzero_digest(engine_sha256){return Err(GpuScheduleError::InvalidRequest)}
        Ok(Self{inner:Mutex::new(GpuState{
            engine_sha256,observations:BTreeMap::new(),leases:BTreeMap::new()
        })})
    }
    /// Update readings only when there are no outstanding native processes.
    /// Never treat reduced device inventory as proof an old GPU child died.
    pub fn set_verified_observations(
        &self,records:Vec<GpuObservation>,now:u64
    )->Result<(),GpuScheduleError>{
        if records.is_empty()||records.len()>MAX_LOCAL_DEVICES {
            return Err(GpuScheduleError::NoDeviceEvidence)
        }
        let mut state=self.inner.lock().map_err(|_|GpuScheduleError::Poisoned)?;
        if !state.leases.is_empty(){return Err(GpuScheduleError::Busy)}
        let mut devices=BTreeMap::new();
        for item in records {
            item.validate(state.engine_sha256,now)?;
            if devices.insert(item.device_name.clone(),item).is_some(){
                return Err(GpuScheduleError::InvalidRequest)
            }
        }
        state.observations=devices;
        Ok(())
    }
    /// Choose one local GPU for an independent, separately supervised model
    /// instance by max verified remaining memory. An expiring observation
    /// cannot create capacity. The subsequent reserve rechecks atomically,
    /// so a concurrent claimant either succeeds or is rejected, never
    /// overcommitting VRAM. This never starts inference by itself.
    pub fn reserve_best_independent(
        &self,reservation_id:&str,session_id:&str,required_vram_mib:u64,now:u64
    )->Result<ApprovedLocalGpuPlacement,GpuScheduleError>{
        if required_vram_mib==0||!valid_host_id(reservation_id)||
            !valid_host_id(session_id){return Err(GpuScheduleError::InvalidRequest)}
        let selected={
            let state=self.inner.lock().map_err(|_|GpuScheduleError::Poisoned)?;
            let mut best:Option<(u64,String)>=None;
            for (name,obs) in &state.observations{
                if obs.validate(state.engine_sha256,now).is_err(){continue}
                let already=state.leases.values().flat_map(|l|&l.devices)
                    .filter(|(device,_)|device==name)
                    .fold(0u64,|n,(_,mib)|n.saturating_add(*mib));
                let available=obs.available_vram_mib.unwrap_or(0);
                let needed=required_vram_mib.checked_add(already)
                    .and_then(|n|n.checked_add(VRAM_HEADROOM_MIB))
                    .ok_or(GpuScheduleError::InsufficientVram)?;
                if available<needed{continue}
                let spare=available-needed;
                if best.as_ref().is_none_or(|(old,_)|spare>*old){
                    best=Some((spare,name.clone()));
                }
            }
            best.map(|(_,name)|name).ok_or(GpuScheduleError::InsufficientVram)?
        };
        self.reserve(GpuRequest{
            reservation_id:reservation_id.into(),session_id:session_id.into(),
            mode:ComputeMode::IndependentInstance,
            devices:vec![selected],
            measured_required_vram_mib:vec![required_vram_mib],
            tensor_split_percent:None,
        },now)
    }
    pub fn reserve(
        &self,request:GpuRequest,now:u64
    )->Result<ApprovedLocalGpuPlacement,GpuScheduleError>{
        if !valid_host_id(&request.reservation_id)||
            !valid_host_id(&request.session_id)||
            request.devices.is_empty() ||
            request.devices.len()>MAX_LOCAL_DEVICES ||
            request.devices.len()!=request.measured_required_vram_mib.len() ||
            (request.mode==ComputeMode::LayerSplit && request.devices.len()!=2) ||
            (request.mode!=ComputeMode::LayerSplit && request.devices.len()!=1) {
            return Err(GpuScheduleError::InvalidRequest)
        }
        if request.mode==ComputeMode::LayerSplit {
            let fractions=request.tensor_split_percent.as_deref()
                .ok_or(GpuScheduleError::InvalidRequest)?;
            if fractions.len()!=2||fractions.iter().any(|x|*x==0)||
                fractions.iter().map(|x|u16::from(*x)).sum::<u16>()!=100 {
                return Err(GpuScheduleError::InvalidRequest)
            }
        }else if request.tensor_split_percent.is_some(){
            return Err(GpuScheduleError::InvalidRequest)
        }
        let mut state=self.inner.lock().map_err(|_|GpuScheduleError::Poisoned)?;
        if state.leases.len()>=MAX_LOCAL_RESERVATIONS ||
            state.leases.contains_key(&request.reservation_id){
            return Err(GpuScheduleError::Busy)
        }
        let mut seen=BTreeSet::new();
        let mut reservations=Vec::new();
        for (name,required) in request.devices.iter()
            .zip(&request.measured_required_vram_mib){
            if !seen.insert(name) || !valid_device_name(name) || *required==0 {
                return Err(GpuScheduleError::InvalidRequest)
            }
            let observation=state.observations.get(name)
                .ok_or(GpuScheduleError::NoDeviceEvidence)?;
            observation.validate(state.engine_sha256,now)?;
            let already_used=state.leases.values().flat_map(|l|&l.devices)
                .filter(|(d,_)|d==name)
                .fold(0u64,|sum,(_,used)|sum.saturating_add(*used));
            let available=observation.available_vram_mib
                .ok_or(GpuScheduleError::UnverifiedVram)?;
            let needed=required.checked_add(already_used)
                .and_then(|v|v.checked_add(VRAM_HEADROOM_MIB))
                .ok_or(GpuScheduleError::InsufficientVram)?;
            if needed>available {return Err(GpuScheduleError::InsufficientVram)}
            reservations.push((name.clone(),*required));
        }
        let approved=ApprovedLocalGpuPlacement{
            engine_sha256:state.engine_sha256,
            mode:request.mode,
            devices:request.devices.clone(),
            tensor_split_percent:request.tensor_split_percent.clone(),
        };
        state.leases.insert(request.reservation_id,Lease{
            session:request.session_id,devices:reservations,
        });
        Ok(approved)
    }
    /// Release only after the original supervisor has independently verified
    /// the exact child was reaped or unloaded. An interrupted/restarted turn
    /// with uncertain OS custody MUST keep resources quarantined.
    pub fn release_verified(
        &self,reservation_id:&str,session_id:&str,process_reaped:bool,
    )->Result<(),GpuScheduleError>{
        let mut state=self.inner.lock().map_err(|_|GpuScheduleError::Poisoned)?;
        let lease=state.leases.get(reservation_id)
            .ok_or(GpuScheduleError::NotFound)?;
        if lease.session!=session_id {return Err(GpuScheduleError::WrongSession)}
        if !process_reaped {return Err(GpuScheduleError::ProcessCustodyUncertain)}
        state.leases.remove(reservation_id);
        Ok(())
    }
    pub fn reservation_count(&self)->Result<usize,GpuScheduleError>{
        Ok(self.inner.lock().map_err(|_|GpuScheduleError::Poisoned)?.leases.len())
    }
    /// No device observation or reservation survives an uncertain host
    /// restart. Neither the availability of an OS PID nor a status text line
    /// proves a previous model process has been reaped. Native acceptance
    /// must reconcile children before new measurements and admissions.
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ApprovedLocalGpuPlacement{
    engine_sha256:[u8;32],
    mode:ComputeMode,
    devices:Vec<String>,
    tensor_split_percent:Option<Vec<u8>>,
}
impl ApprovedLocalGpuPlacement{
    pub fn verify_engine(&self,sha:[u8;32])->Result<(),GpuScheduleError>{
        if self.engine_sha256!=sha {
            return Err(GpuScheduleError::InvalidRequest)
        }
        Ok(())
    }
    pub fn mode(&self)->ComputeMode{self.mode}
    pub fn flags(&self)->Vec<OsString>{
        let mut flags=vec![
            "--device".into(),self.devices.join(",").into(),
        ];
        if self.mode==ComputeMode::LayerSplit {
            flags.extend([
                "--split-mode".into(),"layer".into(),
                "--tensor-split".into(),self.tensor_split_percent.as_ref()
                    .expect("validated split").iter()
                    .map(u8::to_string).collect::<Vec<_>>().join(",").into(),
            ]);
        }else{
            flags.extend(["--split-mode".into(),"none".into()]);
        }
        flags
    }
}

#[derive(Debug,Clone)]
pub struct RemoteNodeDescriptor {
    pub node_id:String,
    pub host:String,
    pub pinned_tls_cert_sha256:[u8;32],
    pub pinned_engine_sha256:[u8;32],
    pub pinned_model_sha256:[u8;32],
    pub operator_approved:bool,
}
pub struct OptionalComputeNodes{
    nodes:Mutex<BTreeMap<String,RemoteNodeDescriptor>>,
}
impl Default for OptionalComputeNodes {
    fn default()->Self{Self{nodes:Mutex::new(BTreeMap::new())}}
}
impl OptionalComputeNodes{
    /// Host approval records identity only. This intentionally does not
    /// expose remote model generation, browsing, file access or tool grants.
    pub fn register_reviewed(&self,record:RemoteNodeDescriptor)
        ->Result<(),GpuScheduleError>{
        let ok=valid_host_id(&record.node_id) &&
            valid_host_id(&record.host) &&
            record.operator_approved &&
            nonzero_digest(record.pinned_tls_cert_sha256) &&
            nonzero_digest(record.pinned_engine_sha256) &&
            nonzero_digest(record.pinned_model_sha256);
        if !ok {return Err(GpuScheduleError::InvalidRequest)}
        let mut nodes=self.nodes.lock().map_err(|_|GpuScheduleError::Poisoned)?;
        if nodes.len()>=4||nodes.contains_key(&record.node_id){
            return Err(GpuScheduleError::Busy)
        }
        nodes.insert(record.node_id.clone(),record);
        Ok(())
    }
    /// Never route prompts or images over a network until a separate mTLS
    /// transport, exact remote model identity, authorization, cancellation,
    /// recovery, and no-tool-egress acceptance is actually implemented.
    pub fn admit_remote_request(&self,_node_id:&str)
        ->Result<(),GpuScheduleError>{
        Err(GpuScheduleError::RemoteTransportDisabled)
    }
    pub fn known_nodes(&self)->Result<usize,GpuScheduleError>{
        Ok(self.nodes.lock().map_err(|_|GpuScheduleError::Poisoned)?.len())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn good(name:&str,t:u64)->GpuObservation{
        GpuObservation{
            device_name:name.into(),total_vram_mib:16384,
            available_vram_mib:Some(12000),observed_at_unix:t,
            pinned_engine_sha256:[1;32],native_memory_probe_verified:true,
        }
    }
    #[test]
    fn default_offline_has_no_invented_gpu_or_remote_capacity(){
        let co=LocalGpuCoordinator::new([1;32]).unwrap();
        assert!(matches!(co.reserve(GpuRequest{
            reservation_id:"r1".into(),session_id:"s1".into(),
            mode:ComputeMode::SingleDevice,devices:vec!["Vulkan0".into()],
            measured_required_vram_mib:vec![4000],tensor_split_percent:None,
        },100),Err(GpuScheduleError::NoDeviceEvidence)));
        let remote=OptionalComputeNodes::default();
        assert_eq!(remote.known_nodes().unwrap(),0);
        assert_eq!(remote.admit_remote_request("anything"),
            Err(GpuScheduleError::RemoteTransportDisabled));
    }
    #[test]
    fn split_one_engine_or_two_independent_model_instances(){
        let co=LocalGpuCoordinator::new([1;32]).unwrap();
        co.set_verified_observations(vec![good("Vulkan0",100),good("Vulkan1",100)],100)
            .unwrap();
        let first=co.reserve(GpuRequest{
            reservation_id:"split".into(),session_id:"s1".into(),
            mode:ComputeMode::LayerSplit,
            devices:vec!["Vulkan0".into(),"Vulkan1".into()],
            measured_required_vram_mib:vec![6000,6000],
            tensor_split_percent:Some(vec![50,50]),
        },101).unwrap();
        assert_eq!(first.flags().iter().map(|v|v.to_string_lossy().to_string())
            .collect::<Vec<_>>(),vec![
            "--device","Vulkan0,Vulkan1","--split-mode","layer",
            "--tensor-split","50,50"]);
        assert!(co.reserve(GpuRequest{
            reservation_id:"independent".into(),session_id:"s2".into(),
            mode:ComputeMode::IndependentInstance,devices:vec!["Vulkan0".into()],
            measured_required_vram_mib:vec![6000],tensor_split_percent:None
        },101).is_err());
        assert_eq!(co.release_verified("split","wrong",true),
            Err(GpuScheduleError::WrongSession));
        assert_eq!(co.release_verified("split","s1",false),
            Err(GpuScheduleError::ProcessCustodyUncertain));
        co.release_verified("split","s1",true).unwrap();
        let a=co.reserve(GpuRequest{
            reservation_id:"a".into(),session_id:"sa".into(),
            mode:ComputeMode::IndependentInstance,devices:vec!["Vulkan0".into()],
            measured_required_vram_mib:vec![8000],tensor_split_percent:None
        },102).unwrap();
        let b=co.reserve(GpuRequest{
            reservation_id:"b".into(),session_id:"sb".into(),
            mode:ComputeMode::IndependentInstance,devices:vec!["Vulkan1".into()],
            measured_required_vram_mib:vec![8000],tensor_split_percent:None
        },102).unwrap();
        assert_eq!(a.flags().len(),4);
        assert_eq!(b.flags().len(),4);
        assert_eq!(co.reservation_count().unwrap(),2);
    }
    #[test]
    fn host_can_choose_separate_devices_without_overcommitting(){
        let co=LocalGpuCoordinator::new([1;32]).unwrap();
        co.set_verified_observations(vec![good("Vulkan0",100),good("Vulkan1",100)],100)
            .unwrap();
        let first=co.reserve_best_independent("i1","s1",8000,101).unwrap();
        assert_eq!(first.flags()[1].to_string_lossy(),"Vulkan0");
        let second=co.reserve_best_independent("i2","s2",8000,101).unwrap();
        assert_eq!(second.flags()[1].to_string_lossy(),"Vulkan1");
        assert_eq!(co.reserve_best_independent("i3","s3",8000,101),
            Err(GpuScheduleError::InsufficientVram));
        assert!(co.release_verified("i1","wrong",true).is_err());
        assert_eq!(co.reservation_count().unwrap(),2);
        co.release_verified("i1","s1",true).unwrap();
        assert!(co.reserve_best_independent("i3","s3",8000,102).is_ok());
    }
    #[test]
    fn unknown_stale_or_unverified_device_is_never_admitted(){
        let co=LocalGpuCoordinator::new([1;32]).unwrap();
        let mut record=good("Vulkan0",100);
        record.native_memory_probe_verified=false;
        assert_eq!(co.set_verified_observations(vec![record],100),
            Err(GpuScheduleError::UnverifiedDevice));
        let record=good("Vulkan0",100);
        assert_eq!(co.set_verified_observations(vec![record.clone()],110),
            Err(GpuScheduleError::UnverifiedDevice));
        let mut wrong=record.clone();
        wrong.pinned_engine_sha256=[2;32];
        assert_eq!(co.set_verified_observations(vec![wrong],100),
            Err(GpuScheduleError::UnverifiedDevice));
        co.set_verified_observations(vec![record],100).unwrap();
        assert_eq!(co.reserve(GpuRequest{
            reservation_id:"late".into(),session_id:"s".into(),
            mode:ComputeMode::SingleDevice,devices:vec!["Vulkan0".into()],
            measured_required_vram_mib:vec![4000],tensor_split_percent:None,
        },110),Err(GpuScheduleError::UnverifiedDevice));
    }
    #[test]
    fn unsafe_names_and_remote_nodes_fail_closed(){
        for n in ["","none","CUDA0,--api-key","../../path","GPU name","Vulkan0\n--port"]{
            assert!(!valid_device_name(n),"{n:?}");
        }
        let remote=OptionalComputeNodes::default();
        assert_eq!(remote.register_reviewed(RemoteNodeDescriptor{
            node_id:"node1".into(),host:"external.example".into(),
            pinned_tls_cert_sha256:[0;32],pinned_engine_sha256:[1;32],
            pinned_model_sha256:[1;32],operator_approved:true,
        }),Err(GpuScheduleError::InvalidRequest));
    }
}
