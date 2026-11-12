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
#[repr(C)]
#[derive(Default)]
struct NativeTimeZoneInformation {
    bias:i32,
    standard_name:[u16;32],
    standard_date:NativeSystemTime,
    standard_bias:i32,
    daylight_name:[u16;32],
    daylight_date:NativeSystemTime,
    daylight_bias:i32,
}

#[cfg(windows)]
#[link(name="kernel32")]
extern "system" {
    fn GetLocalTime(system_time:*mut NativeSystemTime);
    fn GetTimeZoneInformation(info:*mut NativeTimeZoneInformation)->u32;
}

// Windows returns a UTC->local Bias in minutes. A positive Bias means
// a negative UTC offset. A bad or unavailable OS response is never guessed.
#[cfg(windows)]
fn windows_utc_offset(bias:i32,seasonal_bias:i32)->Option<String>{
    let east=-(i64::from(bias)+i64::from(seasonal_bias));
    if !(-840..=840).contains(&east){return None}
    let sign=if east<0{"-"}else{"+"};
    let magnitude=east.abs();
    Some(format!("UTC{sign}{:02}:{:02}",magnitude/60,magnitude%60))
}

#[cfg(windows)]
fn measured_windows_timezone()->Option<String>{
    let mut tz=NativeTimeZoneInformation::default();
    let state=unsafe{GetTimeZoneInformation(&mut tz)};
    let (seasonal,name)=match state{
        0=>(0,&tz.standard_name), // TIME_ZONE_ID_UNKNOWN
        1=>(tz.standard_bias,&tz.standard_name),
        2=>(tz.daylight_bias,&tz.daylight_name),
        _=>return None, // TIME_ZONE_ID_INVALID
    };
    let offset=windows_utc_offset(tz.bias,seasonal)?;
    let length=name.iter().position(|&ch|ch==0).unwrap_or(name.len());
    let os_label=String::from_utf16_lossy(&name[..length]);
    let safe_label=os_label.chars()
        .filter(|ch|ch.is_alphanumeric()||ch.is_whitespace()
            ||matches!(ch,'-'|'_'|'(' | ')'))
        .take(64).collect::<String>();
    if safe_label.trim().is_empty(){Some(offset)}
    else{Some(format!("{offset}; Windows timezone: {}",safe_label.trim()))}
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
    let zone=measured_windows_timezone().unwrap_or_else(||
        "Device timezone offset was not measured".into());
    Some(format!(
        "Windows device LOCAL clock: {:04}-{:02}-{:02} {:02}:{:02}:{:02}; {}.",
        value.year,value.month,value.day,value.hour,value.minute,value.second,zone
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
    let era=(if z>=0 {z} else {z-146_096})/146_097;
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

/// Only the original CURRENT human request can select this host utility.
/// Never route quoted instructions, attachments or time estimates.
pub(crate) fn explicit_current_time_request(prompt:&str)->bool{
    let s=prompt.trim().to_ascii_lowercase();
    if s.len()>256 || s.contains("[attached text file:") || s.contains("<|")
        || ["don't","do not","without","how to","what is time complexity",
            "time travel","time zone difference","timing estimate"]
            .iter().any(|bad|s.contains(bad)){return false}
    ["what time is it","what's the time","what is the current time",
     "what time is it right now","tell me the time",
     "give me the current time","what is my local time",
     "what's the current time","what's the local time"]
        .iter().any(|q|s.starts_with(q))
}
pub(crate) fn fresh_local_clock_answer()->String{
    let observed=device_clock().unwrap_or_else(||
        "Device clock unavailable; Lu cannot report a verified current time".to_owned());
    format!("{observed}\n\nSource: Lu's native operating-system clock, observed for this request. No model guess, network lookup, or shell execution was used.")
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
    fn explicit_current_time_intent_is_narrow_and_host_only(){
        for input in ["What is the current time?","What's the time?",
                      "what time is it right now?","Tell me the time"]{
            assert!(explicit_current_time_request(input),"{input}");
        }
        for input in ["Explain time travel","How to tell the time",
                      "Don't tell me the time","Time complexity of mergesort"]{
            assert!(!explicit_current_time_request(input),"{input}");
        }
        let answer=fresh_local_clock_answer();
        assert!(answer.contains("Source: Lu's native operating-system clock"));
    }
    #[test]
    fn fresh_clock_has_bounded_read_only_context(){
        let s=fresh_local_clock_context();
        assert!(s.len()<750);
        assert!(s.contains("FRESH HOST-CLOCK OBSERVATION"));
        assert!(s.contains("UTC Unix epoch seconds: "));
        assert!(!s.contains("permission granted"));
    }
    #[cfg(windows)]
    #[test]
    fn native_windows_timezone_bias_and_dst_are_explicitly_measured(){
        assert_eq!(windows_utc_offset(420,0).as_deref(),Some("UTC-07:00"));
        assert_eq!(windows_utc_offset(300,-60).as_deref(),Some("UTC-04:00"));
        assert_eq!(windows_utc_offset(-330,0).as_deref(),Some("UTC+05:30"));
        assert!(windows_utc_offset(10_000,0).is_none());
        let current=measured_windows_timezone();
        if let Some(zone)=current{
            assert!(zone.starts_with("UTC+")||zone.starts_with("UTC-"));
        }
    }
    #[cfg(not(windows))]
    #[test]
    fn unix_calendar_handles_leap_day(){
        assert_eq!(civil_from_days(0),(1970,1,1));
        assert_eq!(civil_from_days(19_782),(2024,2,29));
        assert_eq!(civil_from_days(21_550),(2029,1,1));
    }
}
