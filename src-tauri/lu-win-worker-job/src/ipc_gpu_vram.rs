//! LU-IPC-GPU-01B: Windows-native, OS-owned global VRAM observation.
//!
//! Never use NVP's telemetry, a client PID, or DXGI's PROCESS budget alone as
//! evidence of global free VRAM. The only positive result combines SAME-LUID
//! DXGI adapter physical capacity + WDDM "GPU Adapter Memory" dedicated usage
//! and the process budget as a conservative cap. Unknown provider/counter,
//! multiple physical nodes, malformed values or mismatched LUID fail CLOSED.
//!
//! This is a read-only observer, NOT a hard VRAM allocation, signed installer,
//! worker-stop receipt, or physical R9700 acceptance. Windows WDDM counters
//! may be absent or delayed. A successful result is a bounded measurement,
//! not permission to interrupt Lu inference or override user consent.
#![cfg(windows)]
use std::{ffi::c_void,io,mem,ptr,sync::atomic::{AtomicU64,Ordering}};
type Raw=*mut c_void;
const MAX_GPU_BYTES:u64=512*1024*1024*1024;
const PDH_MORE_DATA:u32=0x800007d2;
const PDH_FMT_LARGE:u32=0x0000_0400;
const MAX_PATH_CHARS:u32=65_536;
static SEQ:AtomicU64=AtomicU64::new(0);
#[repr(C)]
#[derive(Clone,Copy)]
struct Guid{data1:u32,data2:u16,data3:u16,data4:[u8;8]}
const IID_FACTORY1:Guid=Guid{
    data1:0x770aae78,data2:0xf26f,data3:0x4dba,
    data4:[0xa8,0x29,0x25,0x3c,0x83,0xd1,0xb3,0x87]};
const IID_ADAPTER3:Guid=Guid{
    data1:0x645967a4,data2:0x1392,data3:0x4310,
    data4:[0xa7,0x98,0x80,0x53,0xce,0x3e,0x93,0xfd]};
#[repr(C)]
#[derive(Default,Clone,Copy)]
struct Luid{low:u32,high:i32}
#[repr(C)]
struct AdapterDesc1{
    description:[u16;128],vendor:u32,device:u32,subsystem:u32,revision:u32,
    dedicated_video_memory:usize,dedicated_system_memory:usize,
    shared_system_memory:usize,luid:Luid,flags:u32,
}
impl Default for AdapterDesc1{
    fn default()->Self{Self{description:[0;128],vendor:0,device:0,
        subsystem:0,revision:0,dedicated_video_memory:0,
        dedicated_system_memory:0,shared_system_memory:0,
        luid:Luid::default(),flags:0}}
}
#[repr(C)]
#[derive(Default)]
struct VideoMemoryInfo{
    budget:u64,current_usage:u64,available_for_reservation:u64,
    current_reservation:u64,
}
#[repr(C)]
#[derive(Default)]
struct PdhValue{status:u32,_padding:u32,large:i64}
#[link(name="dxgi")]
unsafe extern "system" {
    fn CreateDXGIFactory1(iid:*const Guid,out:*mut Raw)->i32;
}
#[link(name="pdh")]
unsafe extern "system" {
    fn PdhExpandWildCardPathW(
        data_source:*const u16,path:*const u16,list:*mut u16,
        len:*mut u32,flags:u32
    )->u32;
    fn PdhOpenQueryW(source:*const u16,user_data:usize,out:*mut Raw)->u32;
    fn PdhAddEnglishCounterW(query:Raw,path:*const u16,
        user_data:usize,out:*mut Raw)->u32;
    fn PdhCollectQueryData(query:Raw)->u32;
    fn PdhGetFormattedCounterValue(counter:Raw,format:u32,
        kind:*mut u32,value:*mut PdhValue)->u32;
    fn PdhCloseQuery(query:Raw)->u32;
}
fn unavailable(reason:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,reason)
}
/// Only canonical host-selected DXGI LUID / physical node zero.
pub fn checked_device(device:&str)->io::Result<(u32,u32)>{
    let Some(raw)=device.strip_prefix("dxgi:luid_0x")else{
        return Err(unavailable("GPU identifier must be a DXGI LUID"))
    };
    let (high,low)=raw.split_once("_0x")
        .ok_or_else(||unavailable("GPU LUID missing low word"))?;
    let low=low.strip_suffix("_phys_0")
        .ok_or_else(||unavailable("GPU physical node unsupported"))?;
    if high.len()!=8||low.len()!=8||
       !high.bytes().all(|b|b.is_ascii_hexdigit())||
       !low.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err(unavailable("GPU LUID must contain exact hex words"))
    }
    let high=u32::from_str_radix(high,16).map_err(|_|unavailable("bad high LUID"))?;
    let low=u32::from_str_radix(low,16).map_err(|_|unavailable("bad low LUID"))?;
    Ok((high,low))
}
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain([0]).collect()}
struct Com(Raw);
impl Drop for Com{
    fn drop(&mut self){
        if !self.0.is_null(){
            // SAFETY: a successful DXGI COM API returned this object. The
            // IUnknown Release slot is always vtable index 2.
            unsafe{
                let call:unsafe extern "system" fn(Raw)->u32=method(self.0,2);
                call(self.0);
            }
        }
    }
}
unsafe fn method<T:Copy>(ptr:Raw,index:usize)->T{
    // SAFETY: caller uses the index and exact ABI of a previously obtained
    // DXGI interface. No client-provided pointer is ever accepted.
    let table=unsafe{*(ptr as *const *const *const c_void)};
    let raw=unsafe{*table.add(index)};
    unsafe{mem::transmute_copy::<*const c_void,T>(&raw)}
}
fn dxgi_adapter(luid:(u32,u32))->io::Result<(Com,AdapterDesc1)>{
    let mut factory=ptr::null_mut();
    if unsafe{CreateDXGIFactory1(&IID_FACTORY1,&mut factory)}<0||
       factory.is_null(){
        return Err(unavailable("DXGI factory unavailable"))
    }
    let factory=Com(factory);
    let enum_adapter:unsafe extern "system" fn(Raw,u32,*mut Raw)->i32=
        unsafe{method(factory.0,12)};
    for index in 0..16u32{
        let mut raw=ptr::null_mut();
        let hr=unsafe{enum_adapter(factory.0,index,&mut raw)};
        if hr<0 {break;}
        if raw.is_null(){return Err(unavailable("empty DXGI adapter"))}
        let adapter=Com(raw);
        let f:unsafe extern "system" fn(Raw,*mut AdapterDesc1)->i32=
            unsafe{method(adapter.0,10)};
        let mut desc=AdapterDesc1::default();
        if unsafe{f(adapter.0,&mut desc)}<0{
            return Err(unavailable("DXGI adapter LUID unavailable"))
        }
        if (desc.luid.high as u32,desc.luid.low)==luid{
            if desc.dedicated_video_memory==0 ||
               (desc.flags & 2)!=0 {
                return Err(unavailable("software or nonlocal GPU cannot grant VRAM"))
            }
            return Ok((adapter,desc))
        }
    }
    Err(unavailable("DXGI GPU LUID not installed"))
}
fn query_dxgi_budget(adapter:&Com)->io::Result<VideoMemoryInfo>{
    let query_interface:unsafe extern "system" fn(Raw,*const Guid,*mut Raw)->i32=
        unsafe{method(adapter.0,0)};
    let mut raw=ptr::null_mut();
    if unsafe{query_interface(adapter.0,&IID_ADAPTER3,&mut raw)}<0||
       raw.is_null(){
        return Err(unavailable("DXGIAdapter3 unavailable"))
    }
    let extended=Com(raw);
    let query:unsafe extern "system" fn(Raw,u32,u32,*mut VideoMemoryInfo)->i32=
        unsafe{method(extended.0,14)};
    let mut info=VideoMemoryInfo::default();
    // Node 0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL=0.
    if unsafe{query(extended.0,0,0,&mut info)}<0||
       info.budget==0||info.budget>MAX_GPU_BYTES||
       info.current_usage>info.budget{
        return Err(unavailable("DXGI local memory budget invalid"))
    }
    Ok(info)
}
fn matched_instance(path:&str,luid:(u32,u32))->bool{
    let name=path.to_ascii_lowercase();
    let exact=format!(
        "luid_0x{:08x}_0x{:08x}_phys_0",luid.0,luid.1);
    // Wildcard expanded path is expected to be exactly:
    // \GPU Adapter Memory(luid_...) \ Dedicated Usage (no spaces around slash).
    let Some((prefix,tail))=name.split_once('(')else{return false};
    prefix=="\\gpu adapter memory" &&
        tail==format!("{exact})\\dedicated usage")
}
fn global_wddm_used(luid:(u32,u32))->io::Result<u64>{
    let wildcard=wide("\\GPU Adapter Memory(*)\\Dedicated Usage");
    let mut needed=0u32;
    let first=unsafe{PdhExpandWildCardPathW(
        ptr::null(),wildcard.as_ptr(),ptr::null_mut(),&mut needed,0)};
    if first!=PDH_MORE_DATA || needed<2||needed>MAX_PATH_CHARS{
        return Err(unavailable("WDDM global GPU counters unavailable"))
    }
    let mut buffer=vec![0u16;needed as usize];
    if unsafe{PdhExpandWildCardPathW(ptr::null(),wildcard.as_ptr(),
        buffer.as_mut_ptr(),&mut needed,0)}!=0{
        return Err(unavailable("WDDM GPU counter enumeration failed"))
    }
    let mut found:Option<String>=None;
    for item in buffer.split(|c|*c==0).filter(|s|!s.is_empty()){
        let path=String::from_utf16(item)
            .map_err(|_|unavailable("noncanonical WDDM counter path"))?;
        if matched_instance(&path,luid){
            if found.replace(path).is_some(){
                return Err(unavailable("duplicate WDDM physical node counters"))
            }
        }
    }
    let path=wide(&found.ok_or_else(||unavailable("matching global WDDM GPU counter absent"))?);
    let mut query=ptr::null_mut();
    if unsafe{PdhOpenQueryW(ptr::null(),0,&mut query)}!=0||
       query.is_null(){
        return Err(unavailable("cannot open WDDM GPU query"))
    }
    struct Query(Raw);
    impl Drop for Query{fn drop(&mut self){unsafe{PdhCloseQuery(self.0);}}}
    let query=Query(query);
    let mut counter=ptr::null_mut();
    if unsafe{PdhAddEnglishCounterW(query.0,path.as_ptr(),0,&mut counter)}!=0||
       counter.is_null()||
       unsafe{PdhCollectQueryData(query.0)}!=0{
        return Err(unavailable("WDDM GPU dedicated usage cannot be sampled"))
    }
    let mut value=PdhValue::default();
    let mut kind=0u32;
    if unsafe{PdhGetFormattedCounterValue(counter,PDH_FMT_LARGE,
        &mut kind,&mut value)}!=0 ||
        value.status!=0 ||value.large<0{
        return Err(unavailable("WDDM GPU counter not independently valid"))
    }
    Ok(value.large as u64)
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct WindowsGpuObservation{
    pub device_ref:String,
    pub physical_capacity_bytes:u64,
    pub global_dedicated_used_bytes:u64,
    pub process_budget_bytes:u64,
    pub available_vram_bytes:u64,
    pub sequence:u64,
}
/// Independent, native Lu-owned OS measurement. This is the only positive
/// Windows capacity API for Core; never accept measured bytes from NVP IPC.
pub fn sample_device(device_ref:&str)->io::Result<WindowsGpuObservation>{
    let luid=checked_device(device_ref)?;
    let (adapter,desc)=dxgi_adapter(luid)?;
    let budget=query_dxgi_budget(&adapter)?;
    let used=global_wddm_used(luid)?;
    let capacity=u64::try_from(desc.dedicated_video_memory)
        .map_err(|_|unavailable("GPU capacity exceeds integer"))?;
    if capacity==0||capacity>MAX_GPU_BYTES||used>capacity{
        return Err(unavailable("WDDM global usage exceeds GPU capacity"))
    }
    let physical_free=capacity-used;
    // DXGI Budget minus CURRENT PROCESS usage is NOT global VRAM capacity.
    // It serves only as a further ceiling; WDDM adapter-global usage is
    // required above before any positive measurement is allowed.
    let process_headroom=budget.budget-budget.current_usage;
    let available=physical_free.min(process_headroom);
    if available==0{return Err(unavailable("no verified device headroom"))}
    Ok(WindowsGpuObservation{
        device_ref:device_ref.to_owned(),physical_capacity_bytes:capacity,
        global_dedicated_used_bytes:used,
        process_budget_bytes:budget.budget,
        available_vram_bytes:available,
        sequence:SEQ.fetch_add(1,Ordering::SeqCst)+1,
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn device_id_rejects_foreign_names_and_malformed_nodes(){
        assert_eq!(checked_device("dxgi:luid_0x12345678_0xabcdef12_phys_0").unwrap(),
            (0x12345678,0xabcdef12));
        for bad in ["gpu-0","dxgi:0","dxgi:luid_0x11111111_0x00000001_phys_1",
            "dxgi:luid_0x00000000_0x00000000_phys_0/path",
            "dxgi:luid_0x0000000_0x00000000_phys_0"] {
            assert!(checked_device(bad).is_err());
        }
    }
    #[test]
    fn exact_global_wddm_adapter_counter_required(){
        let p="\\GPU Adapter Memory(luid_0x12345678_0xabcdef12_phys_0)\\Dedicated Usage";
        assert!(matched_instance(p,(0x12345678,0xabcdef12)));
        assert!(!matched_instance(p,(0x12345678,0xabcdef13)));
        assert!(!matched_instance("\\GPU Process Memory(luid_0x12345678_0xabcdef12_phys_0)\\Dedicated Usage",(0x12345678,0xabcdef12)));
        assert!(!matched_instance("\\GPU Adapter Memory(luid_0x12345678_0xabcdef12_phys_1)\\Dedicated Usage",(0x12345678,0xabcdef12)));
    }
}
