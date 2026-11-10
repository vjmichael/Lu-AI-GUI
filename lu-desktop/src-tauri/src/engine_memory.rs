//! Read-only, measured OS process memory. GPU counters are observed via
//! native Windows PDH when available, never inferred from GGUF size, wall time,
//! progress animation, or OS-wide GPU memory totals.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct EngineMemorySample {
    pub engine_pid:Option<u32>,
    pub working_set_ram_bytes:Option<u64>,
    pub private_ram_bytes:Option<u64>,
    pub dedicated_gpu_bytes:Option<u64>,
    pub shared_gpu_bytes:Option<u64>,
    // Process allocation includes runtime scratch/heap and does not isolate
    // GGUF tensor bytes. Do NOT turn these numbers into model load percent.
    pub verified_model_load_percent:Option<u8>,
    pub measurement_type:&'static str,
}
impl EngineMemorySample{
    pub(crate) fn unavailable()->Self{
        Self{
            engine_pid:None,working_set_ram_bytes:None,private_ram_bytes:None,
            dedicated_gpu_bytes:None,shared_gpu_bytes:None,
            verified_model_load_percent:None,measurement_type:"unavailable"
        }
    }
}
pub(crate) fn sample_host_engine(pid:u32)->EngineMemorySample{
    let mut result=EngineMemorySample::unavailable();
    if pid==0 {return result}
    result.engine_pid=Some(pid);
    let (rss,private,gpu,shared)=platform::sample(pid);
    result.working_set_ram_bytes=rss;
    result.private_ram_bytes=private;
    result.dedicated_gpu_bytes=gpu;
    result.shared_gpu_bytes=shared;
    result.measurement_type=if rss.is_some()||gpu.is_some(){
        "observed_process_allocation_not_tensor_progress"
    }else{"unavailable"};
    result
}
#[cfg(target_os="linux")]
mod platform {
    use std::fs;
    pub(super) fn sample(pid:u32)->(Option<u64>,Option<u64>,Option<u64>,Option<u64>){
        // /proc is host-owned, no shell, no guessed GPU allocation.
        let text=fs::read_to_string(format!("/proc/{pid}/status")).ok();
        let read=|field:&str|->Option<u64>{
            let row=text.as_ref()?.lines().find(|line|line.starts_with(field))?;
            let mut parts=row[field.len()..].split_whitespace();
            let kib=parts.next()?.parse::<u64>().ok()?;
            if parts.next()!=Some("kB"){return None}
            kib.checked_mul(1024)
        };
        (read("VmRSS:"),read("RssAnon:"),None,None)
    }
}
#[cfg(windows)]
mod platform {
    use std::{ffi::c_void,iter,mem,ptr};
    type Handle=*mut c_void;
    #[repr(C)]
    #[derive(Default)]
    struct ProcessMemoryCountersEx{
        cb:u32,page_fault_count:u32,
        peak_working_set_size:usize,working_set_size:usize,
        quota_peak_paged_pool_usage:usize,quota_paged_pool_usage:usize,
        quota_peak_non_paged_pool_usage:usize,quota_non_paged_pool_usage:usize,
        pagefile_usage:usize,peak_pagefile_usage:usize,private_usage:usize,
    }
    #[link(name="kernel32")]
    extern "system" {
        fn OpenProcess(access:u32,inherit:i32,pid:u32)->Handle;
        fn CloseHandle(handle:Handle)->i32;
    }
    #[link(name="psapi")]
    extern "system" {
        fn GetProcessMemoryInfo(handle:Handle,counters:*mut ProcessMemoryCountersEx,size:u32)->i32;
    }
    #[repr(C)]
    #[derive(Clone,Copy)]
    struct PdhFmtValue{status:u32,value:i64}
    #[repr(C)]
    #[derive(Clone,Copy)]
    struct PdhItem{name:*const u16,value:PdhFmtValue}
    #[link(name="pdh")]
    extern "system" {
        fn PdhOpenQueryW(source:*const u16,user_data:usize,handle:*mut isize)->u32;
        fn PdhAddEnglishCounterW(query:isize,path:*const u16,user_data:usize,counter:*mut isize)->u32;
        fn PdhCollectQueryData(query:isize)->u32;
        fn PdhGetFormattedCounterArrayW(counter:isize,format:u32,size:*mut u32,count:*mut u32,items:*mut PdhItem)->u32;
        fn PdhCloseQuery(query:isize)->u32;
    }
    fn ram(pid:u32)->(Option<u64>,Option<u64>){
        // Query only a native process identifier recorded at engine spawn.
        let handle=unsafe{OpenProcess(0x400|0x10,0,pid)};
        if handle.is_null(){return (None,None)}
        let mut data=ProcessMemoryCountersEx::default();
        data.cb=mem::size_of::<ProcessMemoryCountersEx>() as u32;
        let ok=unsafe{GetProcessMemoryInfo(handle,&mut data,data.cb)};
        unsafe{CloseHandle(handle)};
        if ok==0 {(None,None)}
        else {(Some(data.working_set_size as u64),Some(data.private_usage as u64))}
    }
    fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(iter::once(0)).collect()}
    fn instance_name(p:*const u16)->Option<String>{
        if p.is_null(){return None}
        let mut len=0usize;
        while len<512 && unsafe{*p.add(len)}!=0 {len+=1;}
        if len==512{return None}
        Some(String::from_utf16_lossy(unsafe{std::slice::from_raw_parts(p,len)}))
    }
    fn gpu_counter(pid:u32,kind:&str)->Option<u64>{
        let mut query:isize=0;
        if unsafe{PdhOpenQueryW(ptr::null(),0,&mut query)}!=0{return None}
        let sample=(||{
            let mut counter:isize=0;
            let path=wide(&format!(r"\GPU Process Memory(*)\{kind}"));
            if unsafe{PdhAddEnglishCounterW(query,path.as_ptr(),0,&mut counter)}!=0{return None}
            if unsafe{PdhCollectQueryData(query)}!=0{return None}
            let mut bytes=0u32;
            let mut count=0u32;
            // Windows documents PDH_MORE_DATA (0x800007d2) as normal sizing.
            let code=unsafe{PdhGetFormattedCounterArrayW(counter,0x400,&mut bytes,&mut count,ptr::null_mut())};
            if code!=0x8000_07d2||bytes==0||bytes>4*1024*1024||count>10000{return None}
            let mut buf=vec![0usize;(bytes as usize+mem::size_of::<usize>()-1)/mem::size_of::<usize>()];
            if unsafe{PdhGetFormattedCounterArrayW(counter,0x400,&mut bytes,&mut count,buf.as_mut_ptr().cast())}!=0{return None}
            // The returned count is external OS data, not a Rust slice length.
            // Defend against driver/provider errors before constructing a slice.
            if (count as usize).saturating_mul(mem::size_of::<PdhItem>())>buf.len()*mem::size_of::<usize>(){return None}
            let needle=format!("pid_{pid}_");
            let exact=format!("pid_{pid}");
            let mut total=0u64;
            let mut matching=false;
            for item in unsafe{std::slice::from_raw_parts(buf.as_ptr().cast::<PdhItem>(),count as usize)}{
                let Some(instance)=instance_name(item.name)else{continue};
                let lower=instance.to_ascii_lowercase();
                if lower.contains(&needle)||lower==exact {
                    if item.value.status>1||item.value.value<0{continue}
                    total=total.saturating_add(item.value.value as u64);
                    matching=true;
                }
            }
            if matching{Some(total)}else{None}
        })();
        unsafe{PdhCloseQuery(query)};
        sample
    }
    pub(super) fn sample(pid:u32)->(Option<u64>,Option<u64>,Option<u64>,Option<u64>){
        let (ram,private)=ram(pid);
        (ram,private,gpu_counter(pid,"Local Usage"),gpu_counter(pid,"Non Local Usage"))
    }
}
#[cfg(not(any(target_os="linux",windows)))]
mod platform{
    pub(super) fn sample(_:u32)->(Option<u64>,Option<u64>,Option<u64>,Option<u64>){
        (None,None,None,None)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn no_fabricated_percentage_or_unowned_process_sample(){
        let unknown=sample_host_engine(0);
        assert!(unknown.engine_pid.is_none());
        assert!(unknown.verified_model_load_percent.is_none());
        let self_sample=sample_host_engine(std::process::id());
        assert!(self_sample.verified_model_load_percent.is_none());
        #[cfg(any(target_os="linux",windows))]
        assert!(self_sample.working_set_ram_bytes.unwrap_or(0)>0);
    }
}
