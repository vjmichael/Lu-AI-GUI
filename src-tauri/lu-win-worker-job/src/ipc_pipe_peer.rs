//! LU-IPC-01D-R2: isolated Win32 OS observation and secure named-pipe DACL.
//! This crate owns the narrowly reviewed unsafe Win32 boundary. Lu Core
//! remains #![deny(unsafe_code)]. A token, PID, SID, or executable PATH alone
//! is NOT a signed DLC package or a user/effect authorization.
#![cfg(windows)]
use std::{
    ffi::c_void, io, mem, os::windows::io::AsRawHandle, ptr,
};
use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};

type Handle=*mut c_void;
const TOKEN_QUERY:u32=0x0008;
const TOKEN_USER:u32=1;
const TOKEN_IMPERSONATION_LEVEL:u32=9;
const PROCESS_QUERY_LIMITED_INFORMATION:u32=0x1000;
const SDDL_REVISION_1:u32=1;
const MAX_TOKEN_USER_BYTES:u32=16384;
const MAX_IMAGE_CHARS:usize=32768;
#[repr(C)]
struct SecurityAttributes {
    length:u32,
    security_descriptor:*mut c_void,
    inherit_handle:i32,
}
#[repr(C)]
struct SidAndAttributes {
    sid:*mut c_void,
    attributes:u32,
}
#[repr(C)]
struct TokenUser {
    user:SidAndAttributes,
}
#[repr(C)]
#[derive(Default)]
struct FileTime { low:u32, high:u32 }
#[link(name="kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess()->Handle;
    fn GetCurrentThread()->Handle;
    fn GetProcessTimes(process:Handle,creation:*mut FileTime,exit:*mut FileTime,
        kernel:*mut FileTime,user:*mut FileTime)->i32;
    fn OpenProcess(access:u32,inherit:i32,pid:u32)->Handle;
    fn GetProcessId(process:Handle)->u32;
    fn ProcessIdToSessionId(pid:u32,session_id:*mut u32)->i32;
    fn CloseHandle(handle:Handle)->i32;
    fn LocalFree(handle:Handle)->Handle;
    fn GetNamedPipeClientProcessId(pipe:Handle,pid:*mut u32)->i32;
    fn GetNamedPipeClientSessionId(pipe:Handle,session_id:*mut u32)->i32;
    fn QueryFullProcessImageNameW(
        process:Handle,flags:u32,buffer:*mut u16,chars:*mut u32,
    )->i32;
}
#[link(name="advapi32")]
unsafe extern "system" {
    fn ImpersonateNamedPipeClient(pipe:Handle)->i32;
    fn RevertToSelf()->i32;
    fn OpenThreadToken(thread:Handle,access:u32,open_as_self:i32,token:*mut Handle)->i32;
    fn OpenProcessToken(process:Handle,access:u32,token:*mut Handle)->i32;
    fn GetTokenInformation(token:Handle,info_class:u32,data:*mut c_void,
        length:u32,returned:*mut u32)->i32;
    fn ConvertSidToStringSidW(sid:*const c_void,unicode:*mut *mut u16)->i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        sddl:*const u16,revision:u32,descriptor:*mut *mut c_void,
        descriptor_size:*mut u32,
    )->i32;
}
struct OwnedHandle(Handle);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {unsafe{CloseHandle(self.0)};}
    }
}
struct OwnedLocal(Handle);
impl Drop for OwnedLocal {
    fn drop(&mut self) {
        if !self.0.is_null(){unsafe{LocalFree(self.0)};}
    }
}
fn token(process:Handle)->io::Result<OwnedHandle>{
    let mut out:Handle=ptr::null_mut();
    if unsafe{OpenProcessToken(process,TOKEN_QUERY,&mut out)}==0||
        out.is_null(){
        return Err(io::Error::last_os_error())
    }
    Ok(OwnedHandle(out))
}
fn sid_string(token:Handle)->io::Result<String>{
    let mut bytes=0u32;
    // A first call with no buffer is expected to fail with insufficient size.
    unsafe{GetTokenInformation(token,TOKEN_USER,ptr::null_mut(),0,&mut bytes)};
    if bytes<(mem::size_of::<TokenUser>() as u32)
        ||bytes>MAX_TOKEN_USER_BYTES{
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "invalid token-user length"));
    }
    // u64 storage maintains pointer-alignment for TOKEN_USER.
    let mut buffer=vec![0u64;(bytes as usize).div_ceil(8)];
    let mut returned=0u32;
    let ok=unsafe{
        GetTokenInformation(token,TOKEN_USER,buffer.as_mut_ptr().cast(),
            bytes,&mut returned)
    };
    if ok==0||returned<(mem::size_of::<TokenUser>() as u32)||
       returned>bytes {
        return Err(io::Error::last_os_error());
    }
    let user=unsafe{&*(buffer.as_ptr() as *const TokenUser)};
    if user.user.sid.is_null(){
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,"missing SID"));
    }
    let mut sid_ptr:*mut u16=ptr::null_mut();
    if unsafe{ConvertSidToStringSidW(user.user.sid.cast(),&mut sid_ptr)}==0||
        sid_ptr.is_null(){
        return Err(io::Error::last_os_error());
    }
    let _sid_owner=OwnedLocal(sid_ptr.cast());
    let mut count=0usize;
    while count<256 && unsafe{*sid_ptr.add(count)}!=0{count+=1;}
    if count==0||count==256{
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,"SID too long"));
    }
    let sid=String::from_utf16(unsafe{std::slice::from_raw_parts(sid_ptr,count)})
        .map_err(|_|io::Error::new(io::ErrorKind::InvalidData,"invalid SID"))?;
    if !sid.starts_with("S-1-")||
       !sid.bytes().all(|b|b.is_ascii_digit()||b==b'S'||b==b'-'){
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,"SID syntax"));
    }
    Ok(sid)
}
fn current_sid()->io::Result<String>{
    let owner=token(unsafe{GetCurrentProcess()})?;
    sid_string(owner.0)
}
struct OwnedSecurityDescriptor(Handle);
impl Drop for OwnedSecurityDescriptor{
    fn drop(&mut self){if !self.0.is_null(){unsafe{LocalFree(self.0)};}}
}

/// Create PIPE DACL AT CREATION: only current OS owner SID and LocalSystem.
/// Other processes of that user may connect, but never gain enrolled-module
/// authority unless host separately checks package + session + HMAC.
pub fn create_owner_only_pipe(name:&str)->io::Result<NamedPipeServer>{
    if !name.starts_with(r"\\.\pipe\lu-ipc-")||
        name.len()!=r"\\.\pipe\lu-ipc-".len()+32||
        !name[r"\\.\pipe\lu-ipc-".len()..].bytes().all(|b|
            b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()){
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "invalid Lu pipe endpoint"));
    }
    let sid=current_sid()?;
    let sddl=format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})");
    let sddl_w:Vec<u16>=sddl.encode_utf16().chain(std::iter::once(0)).collect();
    let mut descriptor:Handle=ptr::null_mut();
    if unsafe{ConvertStringSecurityDescriptorToSecurityDescriptorW(
        sddl_w.as_ptr(),SDDL_REVISION_1,&mut descriptor,ptr::null_mut()
    )}==0||descriptor.is_null(){
        return Err(io::Error::last_os_error());
    }
    let _sd=OwnedSecurityDescriptor(descriptor);
    let mut attributes=SecurityAttributes{
        length:mem::size_of::<SecurityAttributes>() as u32,
        security_descriptor:descriptor,
        inherit_handle:0,
    };
    let opts=ServerOptions::new()
        .pipe_mode(PipeMode::Byte)
        .reject_remote_clients(true)
        .first_pipe_instance(true)
        .max_instances(1)
        .in_buffer_size(64*1024)
        .out_buffer_size(64*1024)
        .clone();
    // SAFETY: attributes and descriptor are owned and live through the
    // synchronous CreateNamedPipeW call; pointer is noninheritable.
    unsafe{opts.create_with_security_attributes_raw(
        name,(&mut attributes as *mut SecurityAttributes).cast())}
}

/// Native observations, NOT a registration grant. Never return these fields
/// to model output or logs. An approved host policy must compare exact SID,
/// package digest/signature, executable identity and process lifetime.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct WindowsPipePeer {
    pub creation_filetime:u64,
    pub pid:u32,
    pub session_id:u32,
    pub user_sid:String,
    pub image_path:String,
}
/// Independently capture the exact OS identity of a process *already*
/// supervised and launched by the trusted host. Capturing an arbitrary PID
/// does NOT by itself prove which product the process belongs to.
pub fn observe_host_process(pid:u32)->io::Result<WindowsPipePeer>{
    if pid==0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,"zero process ID"));
    }
    let mut session_id=0u32;
    if unsafe{ProcessIdToSessionId(pid,&mut session_id)}==0{
        return Err(io::Error::last_os_error());
    }
    let handle=unsafe{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,0,pid)};
    if handle.is_null(){return Err(io::Error::last_os_error());}
    let process=OwnedHandle(handle);
    if unsafe{GetProcessId(process.0)}!=pid {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "process identity changed"));
    }
    let mut created=FileTime::default();
    let mut exited=FileTime::default();
    let mut kernel_time=FileTime::default();
    let mut user_time=FileTime::default();
    if unsafe{GetProcessTimes(process.0,&mut created,&mut exited,
        &mut kernel_time,&mut user_time)}==0 {
        return Err(io::Error::last_os_error());
    }
    let creation_filetime=(u64::from(created.high)<<32)|u64::from(created.low);
    if creation_filetime==0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "missing process generation"));
    }
    let primary=token(process.0)?;
    let sid=sid_string(primary.0)?;
    let mut path=vec![0u16;MAX_IMAGE_CHARS];
    let mut chars=MAX_IMAGE_CHARS as u32;
    if unsafe{QueryFullProcessImageNameW(
        process.0,0,path.as_mut_ptr(),&mut chars
    )}==0||chars==0||chars as usize>=MAX_IMAGE_CHARS{
        return Err(io::Error::last_os_error());
    }
    let image_path=String::from_utf16(&path[..chars as usize])
        .map_err(|_|io::Error::new(io::ErrorKind::InvalidData,
            "invalid process image path"))?;
    if image_path.is_empty(){
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "missing process image"));
    }
    Ok(WindowsPipePeer{pid,session_id,user_sid:sid,image_path,
        creation_filetime})
}
pub fn observe_connected_peer(pipe:&NamedPipeServer)
    ->io::Result<WindowsPipePeer>{
    let raw=pipe.as_raw_handle() as Handle;
    let mut pid=0u32;
    if unsafe{GetNamedPipeClientProcessId(raw,&mut pid)}==0||pid==0{
        return Err(io::Error::last_os_error());
    }
    let mut session_id=0u32;
    if unsafe{GetNamedPipeClientSessionId(raw,&mut session_id)}==0{
        return Err(io::Error::last_os_error());
    }
    let witness=observe_host_process(pid)?;
    if witness.session_id!=session_id {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "kernel client session disagrees with process"));
    }
    Ok(witness)
}

struct RevertClientOnDrop;
impl Drop for RevertClientOnDrop {
    fn drop(&mut self) {
        // Continuing as an unidentified impersonated client can cross the
        // host privilege boundary. Failure to restore our token is fatal.
        if unsafe{RevertToSelf()}==0 { std::process::abort(); }
    }
}
/// Synchronous-only: call immediately after a *completed* client message
/// ReadFile on this very pipe, before any further read or async suspension.
fn last_message_user_sid(pipe:Handle)->io::Result<String>{
    if unsafe{ImpersonateNamedPipeClient(pipe)}==0 {
        return Err(io::Error::last_os_error());
    }
    let _revert=RevertClientOnDrop;
    let mut handle:Handle=ptr::null_mut();
    if unsafe{OpenThreadToken(GetCurrentThread(),TOKEN_QUERY,1,&mut handle)}==0
        || handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    let access=OwnedHandle(handle);
    let mut level=0u32;
    let mut returned=0u32;
    if unsafe{GetTokenInformation(access.0,TOKEN_IMPERSONATION_LEVEL,
        (&mut level as *mut u32).cast(),mem::size_of::<u32>() as u32,
        &mut returned)}==0 ||
        returned!=mem::size_of::<u32>() as u32 ||
        !(1..=2).contains(&level) {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "anonymous, delegated or unqueryable pipe client token"));
    }
    sid_string(access.0)
}
/// After a complete framed Hello read, compare the effective message token
/// SID with the originating process's primary token and process generation.
/// Never infer Authenticode, installed package ownership or user-turn consent.
pub fn observe_message_token_peer(pipe:&NamedPipeServer)
    ->io::Result<WindowsPipePeer>{
    let before=observe_connected_peer(pipe)?;
    let token_sid=last_message_user_sid(pipe.as_raw_handle() as Handle)?;
    let after=observe_connected_peer(pipe)?;
    if before!=after || token_sid!=after.user_sid {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "pipe message token differs from stable client generation"));
    }
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pipe_endpoint_rejects_arbitrary_names(){
        let bad=[r"\\.\pipe\lu-ipc-short",r"\\.\pipe\other-abc",
            r"\\.\pipe\lu-ipc-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"];
        for name in bad{assert!(create_owner_only_pipe(name).is_err());}
    }
    #[test]
    fn current_sid_has_native_user_prefix(){
        assert!(current_sid().unwrap().starts_with("S-1-"));
    }
}
