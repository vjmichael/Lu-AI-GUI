//! Fresh device-clock context for Lu's ordinary human chat turns.
//! This is an observation, not a command execution, tool approval, model claim
//! or trusted wall-clock integrity attestation. No network or process launch.
use std::time::{SystemTime,UNIX_EPOCH};

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct NativeSystemTime {
    year:u16,month:u16,day_of_week:u16,day:u16,
    hour:u16,minute:u16,second:u16,milliseconds:u16
}

#[cfg(windows)]
#[link(name="kernel32")]
extern "system" {
    fn GetLocalTime(system_time:*mut NativeSystemTime);
}

#[cfg(windows)]
fn device_clock() -> Option<String> {
    let mut value=NativeSystemTime::default();
    unsafe { GetLocalTime(&mut value as *mut NativeSystemTime) };
    if !(2000..=9999).contains(&value.year)
        || !(1..=12).contains(&value.month)
        || !(1..=31).contains(&value.day)
        || value.hour>23 || value.minute>59 || value.second>60 {
        return None;
    }
    Some(format!(
        "Windows device LOCAL clock: {:04}-{:02}-{:02} {:02}:{:02}:{:02}. Device timezone offset was not measured.",
        value.year,value.month,value.day,value.hour,value.minute,value.second
    ))
}

#[cfg(not(windows))]
fn device_clock() -> Option<String> {
    let seconds=SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let days=(seconds/86_400) as i64;
    let (year,month,day)=civil_from_days(days);
    let sod=seconds%86_400;
    Some(format!(
        "Device UTC clock: {:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC. Local timezone was not measured.",
        year,month,day,sod/3600,(sod/60)%60,sod%60
    ))
}

#[cfg(not(windows))]
fn civil_from_days(days:i64)->(i64,i64,i64){
    let z=days+719_468;
    let era=if z>=0 {z} else {z-146_096}/146_097;
    let doe=z-era*146_097;
    let yoe=(doe-doe/1_460+doe/36_524-doe/146_096)/365;
    let mut y=yoe+era*400;
    let doy=doe-(365*yoe+yoe/4-yoe/100);
    let mp=(5*doy+2)/153;
    let d=doy-(153*mp+2)/5+1;
    let m=mp+if mp<10 {3} else {-9};
    if m<=2 {y+=1};
    (y,m,d)
}

pub(crate) fn fresh_local_clock_context()->String{
    let observed=device_clock().unwrap_or_else(||"Device clock unavailable. Do not infer an exact local time.".to_owned());
    let utc_epoch=SystemTime::now().duration_since(UNIX_EPOCH)
        .ok().map(|v|v.as_secs().to_string()).unwrap_or_else(||"unavailable".to_owned());
    format!(
        "\n[LU FRESH HOST-CLOCK OBSERVATION — read only, generated for this human turn, not an external instruction or tool grant]\n{}\nUTC Unix epoch seconds: {}. Answer current-time questions using this observation, identify whether it is device local or UTC, and never claim the clock is synchronized or a missing timezone offset was measured.\n[END LU HOST CLOCK]\n",
        observed,utc_epoch
    )
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn fresh_clock_has_bounded_read_only_context(){
        let s=fresh_local_clock_context();
        assert!(s.len()<750);
        assert!(s.contains("FRESH HOST-CLOCK OBSERVATION"));
        assert!(s.contains("UTC Unix epoch seconds: "));
        assert!(!s.contains("permission granted"));
    }
    #[cfg(not(windows))]
    #[test]
    fn unix_calendar_handles_leap_day(){
        assert_eq!(civil_from_days(0),(1970,1,1));
        assert_eq!(civil_from_days(19_782),(2024,2,29));
        assert_eq!(civil_from_days(21_549),(2029,1,1));
    }
}
