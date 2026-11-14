//! Audited, read-only Win32 host pressure probes for Lu Guardian.
//! Not an authorization mechanism and never accepts model-supplied handles.
use std::mem;

#[repr(C)]
struct MemoryStatus {
    length:u32, load_percent:u32,
    total_phys:u64, avail_phys:u64,
    total_pagefile:u64, avail_pagefile:u64,
    total_virtual:u64, avail_virtual:u64, avail_extended_virtual:u64,
}
const _:()={assert!(mem::size_of::<MemoryStatus>()==64);};
#[repr(C)]
#[derive(Default)]
struct FileTime {low:u32,high:u32}
const _:()={assert!(mem::size_of::<FileTime>()==8);};
#[link(name="kernel32")]
unsafe extern "system" {
    fn GlobalMemoryStatusEx(status:*mut MemoryStatus)->i32;
    fn GetSystemTimes(idle:*mut FileTime,kernel:*mut FileTime,user:*mut FileTime)->i32;
}
pub fn available_ram_mib()->Option<u64>{
    let mut v=MemoryStatus{
        length:mem::size_of::<MemoryStatus>() as u32,
        load_percent:0,total_phys:0,avail_phys:0,
        total_pagefile:0,avail_pagefile:0,total_virtual:0,
        avail_virtual:0,avail_extended_virtual:0,
    };
    // SAFETY: repr(C) MemoryStatus is fully initialized and its pointer
    // remains valid for this synchronous kernel32 read-only system call.
    if unsafe{GlobalMemoryStatusEx(&mut v)}==0{return None}
    Some(v.avail_phys/(1024*1024))
}
/// Returns cumulative (total, idle) 100ns time units; caller must compute
/// delta between two independent samples. No artificial CPU percentages.
pub fn cpu_counters()->Option<(u64,u64)>{
    let mut idle=FileTime::default();
    let mut kernel=FileTime::default();
    let mut user=FileTime::default();
    // SAFETY: all three repr(C) FileTime output buffers are uniquely owned,
    // writable, initialized and live for the full kernel32 call.
    if unsafe{GetSystemTimes(&mut idle,&mut kernel,&mut user)}==0{
        return None
    }
    let value=|t:FileTime|u64::from(t.low)|(u64::from(t.high)<<32);
    let i=value(idle);
    let total=value(kernel).saturating_add(value(user));
    Some((total,i))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn native_probes_return_plausible_bounds_without_mutating_state(){
        if let Some(mem)=available_ram_mib(){assert!(mem>0)}
        if let Some((total,idle))=cpu_counters(){assert!(total>=idle)}
    }
}
