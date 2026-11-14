//! Read-only OS resource pressure sampling; admission remains Lu host-owned.
//! Missing/unavailable probes DO NOT grant more capacity. This is distinct
//! from reservation accounting and from backend-specific VRAM proofs.
use std::sync::Mutex;
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HostPressure{
    pub available_ram_mib:Option<u64>,
    pub cpu_busy_percent:Option<u8>,
}
#[derive(Default)]
pub struct PressureMonitor{ last_cpu:Mutex<Option<(u64,u64)>> }
impl PressureMonitor{
    pub fn new()->Self{Self::default()}
    pub fn sample(&self)->HostPressure{
        let available_ram_mib=available_ram();
        let observed=cpu_counters();
        let cpu_busy_percent=self.last_cpu.lock().ok().and_then(|mut prev|{
            let prior=*prev;
            *prev=observed;
            prior.zip(observed).and_then(|((total_a,idle_a),(total_b,idle_b))|{
                let total=total_b.checked_sub(total_a)?;
                let idle=idle_b.checked_sub(idle_a)?;
                if total==0||idle>total{return None}
                Some(((total-idle).saturating_mul(100)/total).min(100) as u8)
            })
        });
        HostPressure{available_ram_mib,cpu_busy_percent}
    }
}
pub fn available_ram()->Option<u64>{
    #[cfg(windows)]
    {return lu_win_worker_job::system_memory::available_ram_mib()}
    #[cfg(target_os="linux")]
    {return linux_ram()}
    #[cfg(not(any(windows,target_os="linux")))]
    {None}
}
fn cpu_counters()->Option<(u64,u64)>{
    #[cfg(windows)]
    {return lu_win_worker_job::system_memory::cpu_counters()}
    #[cfg(target_os="linux")]
    {return linux_cpu()}
    #[cfg(not(any(windows,target_os="linux")))]
    {None}
}
#[cfg(target_os="linux")]
fn linux_ram()->Option<u64>{
    let text=std::fs::read_to_string("/proc/meminfo").ok()?;
    parse_mem_available(&text)
}
#[cfg(target_os="linux")]
fn linux_cpu()->Option<(u64,u64)>{
    let text=std::fs::read_to_string("/proc/stat").ok()?;
    parse_proc_cpu(&text)
}
pub fn parse_mem_available(text:&str)->Option<u64>{
    let line=text.lines().find(|l|l.starts_with("MemAvailable:"))?;
    let mut words=line.split_whitespace();
    if words.next()?!="MemAvailable:"{return None}
    let kibs=words.next()?.parse::<u64>().ok()?;
    // Only canonical kernel unit; refuse ambiguous unlabelled values.
    if words.next()!=Some("kB"){return None}
    if kibs==0{return None}
    Some(kibs/1024)
}
pub fn parse_proc_cpu(text:&str)->Option<(u64,u64)>{
    let line=text.lines().next()?;
    let mut words=line.split_whitespace();
    if words.next()?!="cpu"{return None}
    let mut ticks=Vec::new();
    for word in words.take(8){
        ticks.push(word.parse::<u64>().ok()?);
    }
    if ticks.len()<4{return None}
    let idle=ticks[3].checked_add(*ticks.get(4).unwrap_or(&0))?;
    let total=ticks.into_iter().try_fold(0u64,|total,t|total.checked_add(t))?;
    if total<idle{return None}
    Some((total,idle))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn parses_real_kernel_memory_units_only(){
        assert_eq!(parse_mem_available("MemTotal: 999999 kB\nMemAvailable: 1048576 kB\n"),Some(1024));
        assert_eq!(parse_mem_available("MemAvailable: 1024 bytes\n"),None);
        assert_eq!(parse_mem_available("MemAvailable: invalid kB\n"),None);
    }
    #[test] fn cpu_total_idle_are_consistent(){
        assert_eq!(parse_proc_cpu("cpu  10 20 30 40 50 60 70 80\n"),Some((360,90)));
        assert_eq!(parse_proc_cpu("cpu0 4 3 2 1\n"),None);
    }
}
