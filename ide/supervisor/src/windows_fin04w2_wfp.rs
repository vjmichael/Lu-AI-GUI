//! FIN-04W2 native live WFP readback, never a self-certified firewall flag.
//! Bounded enumeration of currently installed BFE filters. A positive result
//! proves only an exact application, enabled BLOCK rule at outbound IPv4/IPv6
//! ALE connect layers with no narrower conditions. It cannot replace a
//! hostile-socket probe or prove the rule survives a session. The SCM service
//! rechecks OS process tokens, Jobs and original approval separately.
#![cfg(all(windows, debug_assertions, feature="fin04w2-split-native"))]
use crate::{windows_fin04w2_kernel,SupervisorError};
use std::{ffi::OsStr,os::windows::ffi::OsStrExt,path::Path,ptr,slice};
use windows_sys::Win32::{
    Foundation::HANDLE,
    NetworkManagement::WindowsFilteringPlatform::{
        FwpmEngineOpen0,FwpmEngineClose0,FwpmFilterCreateEnumHandle0,
        FwpmFilterDestroyEnumHandle0,FwpmFilterEnum0,FwpmFreeMemory0,
        FwpmGetAppIdFromFileName0,FWPM_FILTER0,FWP_BYTE_BLOB,
        FWPM_LAYER_ALE_AUTH_CONNECT_V4,FWPM_LAYER_ALE_AUTH_CONNECT_V6,
        FWPM_CONDITION_ALE_APP_ID,FWP_ACTION_BLOCK,FWP_BYTE_BLOB_TYPE,
        FWP_MATCH_EQUAL,FWPM_FILTER_FLAG_DISABLED,
    },
};
fn denied()->SupervisorError{SupervisorError::NoContainment}
struct Engine(HANDLE);
impl Drop for Engine{
    fn drop(&mut self){if !self.0.is_null(){unsafe{FwpmEngineClose0(self.0);}}}
}
struct Enumerator {engine:HANDLE,handle:HANDLE}
impl Drop for Enumerator {
    fn drop(&mut self){
        if !self.handle.is_null(){unsafe{FwpmFilterDestroyEnumHandle0(self.engine,self.handle);}}
    }
}
struct Blob(*mut FWP_BYTE_BLOB);
impl Drop for Blob {
    fn drop(&mut self){
        if !self.0.is_null(){
            unsafe{FwpmFreeMemory0((&mut self.0 as *mut *mut FWP_BYTE_BLOB).cast());}
        }
    }
}
fn application_id(path:&Path)->Result<Blob,SupervisorError>{
    if !path.is_absolute(){return Err(denied());}
    let wide:Vec<u16>=path.as_os_str().encode_wide().chain([0]).collect();
    let mut ptr=ptr::null_mut();
    if unsafe{FwpmGetAppIdFromFileName0(wide.as_ptr(),&mut ptr)}!=0
        ||ptr.is_null(){return Err(denied());}
    let blob=Blob(ptr);
    if unsafe{(*ptr).size}<2||unsafe{(*ptr).size}>65536
        ||unsafe{(*ptr).data.is_null()}{return Err(denied());}
    Ok(blob)
}
fn matches_app(expected:&Blob,cond:&windows_sys::Win32::NetworkManagement::
    WindowsFilteringPlatform::FWPM_FILTER_CONDITION0)->bool{
    if cond.fieldKey!=FWPM_CONDITION_ALE_APP_ID
        ||cond.matchType!=FWP_MATCH_EQUAL
        ||cond.conditionValue.r#type!=FWP_BYTE_BLOB_TYPE{return false;}
    let ptr=unsafe{cond.conditionValue.Anonymous.byteBlob};
    if ptr.is_null(){return false;}
    let a=unsafe{&*ptr};
    let b=unsafe{&*expected.0};
    if a.size<2||a.size>65536||a.size!=b.size
        ||a.data.is_null()||b.data.is_null(){return false;}
    let bytes=unsafe{slice::from_raw_parts(a.data,a.size as usize)};
    let wanted=unsafe{slice::from_raw_parts(b.data,b.size as usize)};
    bytes==wanted
}
/// Require both installed outbound IPv4/IPv6 application BLOCK filters.
/// This is a live OS readback, NOT effective hostile-socket proof.
pub(crate) fn require_exact_outbound_blocks(debugger:&Path,target:&Path)
    ->Result<(),SupervisorError>{
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    if debugger==target{return Err(denied());}
    require_application_blocks(&[debugger,target])
}
/// The protected debugger SCM also constrains the actual first-party LLVM
/// symbolizer subprocess; it is not exempt from the same WFP V4/V6 blocks.
pub(crate) fn require_debugger_tool_outbound_blocks(tool:&Path)
    ->Result<(),SupervisorError>{
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    require_application_blocks(&[tool])
}
/// The target broker independently checks its sealed AppContainer executable
/// under its own restricted SCM identity, with no debugger service token.
pub(crate) fn require_target_outbound_blocks(target:&Path)
    ->Result<(),SupervisorError>{
    windows_fin04w2_kernel::inspect_current_target_service_principal()?;
    require_application_blocks(&[target])
}
fn require_application_blocks(paths:&[&Path])->Result<(),SupervisorError>{
    if paths.is_empty()||paths.len()>2||paths.iter().any(|p|!p.is_absolute()){
        return Err(denied());
    }
    let apps:Vec<Blob>=paths.iter().map(|p|application_id(p)).collect::<Result<_,_>>()?;
    let mut raw=ptr::null_mut();
    if unsafe{FwpmEngineOpen0(ptr::null(),10,ptr::null(),ptr::null(),&mut raw)}!=0
        ||raw.is_null(){return Err(denied());}
    let engine=Engine(raw);
    let mut handle=ptr::null_mut();
    if unsafe{FwpmFilterCreateEnumHandle0(engine.0,ptr::null(),&mut handle)}!=0
        ||handle.is_null(){return Err(denied());}
    let en=Enumerator{engine:engine.0,handle};
    let mut matched=vec![[false;2];apps.len()];
    let mut total=0usize;
    loop{
        let mut page:*mut *mut FWPM_FILTER0=ptr::null_mut();let mut count=0u32;
        let status=unsafe{FwpmFilterEnum0(engine.0,en.handle,64,&mut page,&mut count)};
        if status!=0{return Err(denied());}
        if count>64||total+count as usize>8192{
            if !page.is_null(){unsafe{FwpmFreeMemory0(
                (&mut page as *mut *mut *mut FWPM_FILTER0).cast()
            );}}
            return Err(denied());
        }
        if count>0&&page.is_null(){return Err(denied());}
        for i in 0..count as usize{
            let pointer=unsafe{*page.add(i)};
            if pointer.is_null(){continue;}
            let f=unsafe{&*pointer};
            let family=if f.layerKey==FWPM_LAYER_ALE_AUTH_CONNECT_V4{0}
                else if f.layerKey==FWPM_LAYER_ALE_AUTH_CONNECT_V6{1}
                else {continue};
            if f.flags&FWPM_FILTER_FLAG_DISABLED!=0
                ||f.action.r#type!=FWP_ACTION_BLOCK||f.numFilterConditions!=1
                ||f.filterCondition.is_null(){continue;}
            let condition=unsafe{&*f.filterCondition};
            for (j,app) in apps.iter().enumerate(){
                if matches_app(app,condition){matched[j][family]=true;}
            }
        }
        total+=count as usize;
        if !page.is_null(){unsafe{FwpmFreeMemory0(
            (&mut page as *mut *mut *mut FWPM_FILTER0).cast()
        );}}
        if count<64{break;}
    }
    if matched.iter().any(|v|*v!=[true,true]){return Err(denied());}
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_ci_user_cannot_certify_restricted_service_wfp(){
        assert!(require_exact_outbound_blocks(
            Path::new(r"C:\not-a-reviewed-lldb-dap.exe"),
            Path::new(r"C:\not-a-reviewed-debug-target.exe"),
        ).is_err());
    }
}
