//! FIN-04W2: fixed-purpose native LLDB-DAP attach witness, never a model,
//! WebView, caller-selected PID, evaluate, launch, shell or arbitrary command.
//! The PID is a kernel-observed target received only from the attested target
//! SCM lease. DAP data is UNTRUSTED until the other SCM Job independently
//! reobserves thread RIP, loaded PE, source and PDB. This module never
//! authorizes a debugger action or asserts installed/runtime acceptance.
#![cfg(all(windows,debug_assertions,feature="fin04w2-split-native"))]
use crate::{windows_fin04w2_split_launcher::PendingDebugger,SupervisorError};
use serde_json::{json,Value};
use std::{ptr,thread,time::{Duration,Instant}};
use windows_sys::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::{ReadFile,WriteFile},
    System::Pipes::PeekNamedPipe,
};
const MAX_FRAME:usize=64*1024;
const MAX_HEADER:usize=512;
const MAX_BUFFER:usize=MAX_FRAME+MAX_HEADER+4;
const MAX_TOTAL_BYTES:usize=256*1024;
fn denied()->SupervisorError{SupervisorError::NoContainment}
pub(crate) struct StoppedCandidate{
    pub(crate) thread_id:u32,
    pub(crate) observed_pc:u64,
    pub(crate) source_line:u32,
    pub(crate) source_path:String,
}
fn extract_frame(buffer:&mut Vec<u8>)->Result<Option<Value>,SupervisorError>{
    if buffer.len()>MAX_BUFFER{return Err(denied());}
    let header_end=match buffer.windows(4).position(|w|w==b"\r\n\r\n"){
        Some(n)=>n,
        None=>{
            if buffer.len()>MAX_HEADER{return Err(denied());}
            return Ok(None);
        }
    };
    if header_end==0||header_end>MAX_HEADER{return Err(denied());}
    let header=std::str::from_utf8(&buffer[..header_end]).map_err(|_|denied())?;
    let mut length=None;
    for line in header.split("\r\n"){
        let number=line.strip_prefix("Content-Length: ").ok_or_else(denied)?;
        if length.is_some(){return Err(denied());}
        let value=number.parse::<usize>().map_err(|_|denied())?;
        if value==0||value>MAX_FRAME{return Err(denied());}
        length=Some(value);
    }
    let len=length.ok_or_else(denied)?;
    let end=header_end.checked_add(4).and_then(|v|v.checked_add(len))
        .ok_or_else(denied)?;
    if end>MAX_BUFFER{return Err(denied());}
    if buffer.len()<end{return Ok(None);}
    let message:Value=serde_json::from_slice(&buffer[header_end+4..end])
        .map_err(|_|denied())?;
    buffer.drain(..end);
    if !message.is_object(){return Err(denied());}
    Ok(Some(message))
}
struct NativeDap{
    input:HANDLE,
    output:HANDLE,
    incoming:Vec<u8>,
    transferred:usize,
}
impl NativeDap{
    fn new(debugger:&PendingDebugger)->Self{
        let (input,output)=debugger.stdio_handles();
        Self{input,output,incoming:Vec::new(),transferred:0}
    }
    fn send(&mut self,seq:u32,command:&'static str,args:Value)
        ->Result<(),SupervisorError>{
        if !matches!((seq,command),(1,"initialize")|(2,"attach")|
            (3,"configurationDone")|(4,"stackTrace")){return Err(denied());}
        let body=serde_json::to_vec(&json!({
            "seq":seq,"type":"request","command":command,"arguments":args,
        })).map_err(|_|denied())?;
        if body.is_empty()||body.len()>2048{return Err(denied());}
        let mut frame=format!("Content-Length: {}\r\n\r\n",body.len()).into_bytes();
        frame.extend_from_slice(&body);
        self.transferred=self.transferred.checked_add(frame.len()).ok_or_else(denied)?;
        if self.transferred>MAX_TOTAL_BYTES{return Err(denied());}
        let mut written=0u32;
        if unsafe{WriteFile(self.input,frame.as_ptr().cast(),
            frame.len() as u32,&mut written,ptr::null_mut())}==0
            ||written!=frame.len() as u32{return Err(denied());}
        Ok(())
    }
    fn receive(&mut self,deadline:Instant)->Result<Value,SupervisorError>{
        loop{
            if let Some(item)=extract_frame(&mut self.incoming)?{
                if item.get("type").and_then(Value::as_str)==Some("request"){
                    return Err(denied()); // never grant reverse runInTerminal
                }
                return Ok(item);
            }
            if Instant::now()>=deadline{return Err(denied());}
            let mut available=0u32;
            if unsafe{PeekNamedPipe(self.output,ptr::null_mut(),0,ptr::null_mut(),
                &mut available,ptr::null_mut())}==0{return Err(denied());}
            if available>0{
                if available as usize>MAX_BUFFER{return Err(denied());}
                let mut scratch=[0u8;4096];
                let cap=available.min(scratch.len() as u32);
                let mut got=0u32;
                if unsafe{ReadFile(self.output,scratch.as_mut_ptr().cast(),cap,
                    &mut got,ptr::null_mut())}==0||got==0{return Err(denied());}
                self.transferred=self.transferred.checked_add(got as usize)
                    .ok_or_else(denied)?;
                if self.transferred>MAX_TOTAL_BYTES{return Err(denied());}
                self.incoming.extend_from_slice(&scratch[..got as usize]);
            }else{
                thread::sleep(Duration::from_millis(15));
            }
        }
    }
}
fn response_ok(v:&Value,sequence:u64,command:&str)->Result<bool,SupervisorError>{
    if v.get("type").and_then(Value::as_str)!=Some("response"){return Ok(false);}
    if v.get("request_seq").and_then(Value::as_u64)!=Some(sequence){
        return Ok(false); // caller rejects unexpected unhandled response
    }
    if v.get("command").and_then(Value::as_str)!=Some(command)
        ||v.get("success").and_then(Value::as_bool)!=Some(true){
        return Err(denied());
    }
    Ok(true)
}
fn stopped_id(v:&Value)->Result<Option<u32>,SupervisorError>{
    if v.get("type").and_then(Value::as_str)!=Some("event"){return Ok(None);}
    match v.get("event").and_then(Value::as_str){
        Some("stopped")=>{
            let raw=v.pointer("/body/threadId").and_then(Value::as_u64)
                .ok_or_else(denied)?;
            let tid=u32::try_from(raw).map_err(|_|denied())?;
            if tid==0{return Err(denied());}
            let reason=v.pointer("/body/reason").and_then(Value::as_str)
                .ok_or_else(denied)?;
            if !matches!(reason,"entry"|"pause"|"breakpoint"|"exception"){
                return Err(denied());
            }
            Ok(Some(tid))
        }
        Some("terminated"|"exited"|"continued")=>Err(denied()),
        _=>Ok(None),
    }
}
fn source_from_stack(v:&Value,tid:u32)->Result<StoppedCandidate,SupervisorError>{
    if !response_ok(v,4,"stackTrace")?{return Err(denied());}
    let frame=v.pointer("/body/stackFrames/0").ok_or_else(denied)?;
    let ip=frame.get("instructionPointerReference").and_then(Value::as_str)
        .ok_or_else(denied)?;
    let hex=ip.strip_prefix("0x").ok_or_else(denied)?;
    if hex.is_empty()||hex.len()>16
        ||!hex.bytes().all(|c|c.is_ascii_hexdigit()){return Err(denied());}
    let rip=u64::from_str_radix(hex,16).map_err(|_|denied())?;
    let line=frame.get("line").and_then(Value::as_u64).ok_or_else(denied)?;
    let line=u32::try_from(line).map_err(|_|denied())?;
    let path=frame.pointer("/source/path").and_then(Value::as_str)
        .ok_or_else(denied)?;
    if rip==0||line==0||line>10_000_000
        ||path.len()>4096||path.is_empty()||path.contains('\0'){
        return Err(denied());
    }
    Ok(StoppedCandidate{
        thread_id:tid,observed_pc:rip,source_line:line,
        source_path:path.to_owned(),
    })
}
/// Actual named pinned LLDB-DAP process is already RUNNING within the
/// debugger SCM-held Job; the untrusted target PID originates from the
/// target SCM's independent kernel GetProcessId and sealed original request.
/// Returned candidate is NOT an OS stop, source line, PDB or release proof.
pub(crate) fn attach_and_observe_stop(
    debugger:&PendingDebugger,target_pid:u32,
)->Result<StoppedCandidate,SupervisorError>{
    if target_pid==0{return Err(denied());}
    let mut dap=NativeDap::new(debugger);
    dap.send(1,"initialize",json!({
        "adapterID":"lu-ide","clientID":"lu-ide",
        "linesStartAt1":true,"columnsStartAt1":true,
        "pathFormat":"path","supportsRunInTerminalRequest":false,
        "supportsVariableType":false,
    }))?;
    let deadline=Instant::now()+Duration::from_secs(20);
    let mut initialized=false;
    let mut init_ack=false;
    while !init_ack{
        let v=dap.receive(deadline)?;
        if response_ok(&v,1,"initialize")?{init_ack=true;continue;}
        if v.get("type").and_then(Value::as_str)==Some("event"){
            if v.get("event").and_then(Value::as_str)==Some("initialized"){
                initialized=true;
            }else{let _=stopped_id(&v)?;}
        }else{return Err(denied());}
    }
    dap.send(2,"attach",json!({"pid":target_pid,"stopOnEntry":true}))?;
    let mut attach_ack=false;
    let mut stopped=None;
    let mut configured=false;
    while !(attach_ack&&stopped.is_some()){
        let v=dap.receive(deadline)?;
        if response_ok(&v,2,"attach")?{
            attach_ack=true;
        }else if response_ok(&v,3,"configurationDone")?{
            // A configurationDone ACK never substitutes for actual attach.
        }else if v.get("type").and_then(Value::as_str)==Some("event"){
            if v.get("event").and_then(Value::as_str)==Some("initialized"){
                initialized=true;
            }else if let Some(id)=stopped_id(&v)?{
                if stopped.replace(id).is_some(){return Err(denied());}
            }
        }else{return Err(denied());}
        if initialized&&!configured{
            dap.send(3,"configurationDone",json!({}))?;
            configured=true;
        }
    }
    let tid=stopped.ok_or_else(denied)?;
    dap.send(4,"stackTrace",json!({"threadId":tid,"startFrame":0,"levels":1}))?;
    loop{
        let v=dap.receive(deadline)?;
        if v.get("type").and_then(Value::as_str)==Some("response"){
            if response_ok(&v,3,"configurationDone")?{continue;}
            return source_from_stack(&v,tid);
        }
        if v.get("type").and_then(Value::as_str)==Some("event"){
            let _=stopped_id(&v)?;
        }else{return Err(denied());}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn strict_fixed_dap_framing_and_no_reverse_request(){
        let mut bytes=b"Content-Length: 46\r\n\r\n".to_vec();
        bytes.extend_from_slice(br#"{"type":"event","event":"initialized","seq":1}"#);
        let value=extract_frame(&mut bytes).unwrap().unwrap();
        assert_eq!(value.get("event").and_then(Value::as_str),Some("initialized"));
        assert!(bytes.is_empty());
        assert!(extract_frame(&mut b"Content-Length: 999999\r\n\r\n{}".to_vec()).is_err());
        assert!(extract_frame(&mut b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}".to_vec()).is_err());
    }
    #[test]
    fn forged_stack_line_ip_or_thread_is_not_a_valid_candidate(){
        let good=json!({"type":"response","request_seq":4,"command":"stackTrace",
            "success":true,"body":{"stackFrames":[{"line":15,
                "instructionPointerReference":"0x140001234",
                "source":{"path":"C:\\sealed\\src\\main.rs"}}]}});
        assert_eq!(source_from_stack(&good,12).unwrap().observed_pc,0x140001234);
        let mut x=good.clone();x["body"]["stackFrames"][0]["line"]=json!(0);
        assert!(source_from_stack(&x,12).is_err());
        let mut x=good.clone();x["body"]["stackFrames"][0]["instructionPointerReference"]=json!("not-a-pc");
        assert!(source_from_stack(&x,12).is_err());
        let mut x=good.clone();x["request_seq"]=json!(5);
        assert!(source_from_stack(&x,12).is_err());
    }
}