//! Human takeover and window visibility for the *same* Lu-owned Chromium.
//! On Linux a trusted SEC-06C manager verifies Chromium's original kernel
//! PID and immutable root-owned leaf before this interface is created.
//! X11 search is only for that PID; no model-selected title, PID, or HWND.
use std::process::Child;
#[cfg(target_os="linux")]
use std::{fs,path::{Path,PathBuf},os::unix::fs::{MetadataExt,PermissionsExt}};
#[cfg(target_os="linux")]
use std::io;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum WindowOperationError {
    UnsupportedPlatform,NativeFailure,ChildNotOwned,
}
pub struct OwnedBrowserWindow {
    #[cfg(windows)]
    browser:Child,
    #[cfg(target_os="linux")]
    browser:Option<Child>,
    #[cfg(target_os="linux")]
    kernel_pid:Option<u32>,
    #[cfg(target_os="linux")]
    kernel_leaf:Option<PathBuf>,
    #[cfg(target_os="linux")]
    trusted_xdotool:PathBuf,
}
impl OwnedBrowserWindow {
    #[cfg(target_os="linux")]
    pub fn new(mut browser:Child,trusted_xdotool:PathBuf)
        ->Result<Self,WindowOperationError>{
        if browser.id()==0||!trusted_xdotool.is_absolute()
            ||!trusted_xdotool.is_file(){
            let _=browser.kill();let _=browser.wait();
            return Err(WindowOperationError::ChildNotOwned);
        }
        Ok(Self{browser:Some(browser),kernel_pid:None,kernel_leaf:None,
            trusted_xdotool})
    }
    /// Only a trusted SEC-06C kernel-admission result may reach this API.
    /// We independently inspect /proc and root cgroup ownership; an absent
    /// or user-writable cgroup denies the window operation. No guessed PID.
    #[cfg(target_os="linux")]
    pub(crate) fn from_sec06c_admission(
        pid:u32,leaf:PathBuf,trusted_xdotool:PathBuf,
    )->Result<Self,WindowOperationError>{
        if pid<=1||!trusted_xdotool.is_absolute()||!trusted_xdotool.is_file()
            ||!leaf.starts_with("/sys/fs/cgroup/lu-")
            &&!leaf.starts_with("/sys/fs/cgroup/system.slice/"){
            return Err(WindowOperationError::ChildNotOwned);
        }
        if !original_leaf_member(pid,&leaf){
            return Err(WindowOperationError::ChildNotOwned)
        }
        Ok(Self{browser:None,kernel_pid:Some(pid),kernel_leaf:Some(leaf),
            trusted_xdotool})
    }
    #[cfg(windows)]
    pub fn new(mut browser:Child)->Result<Self,WindowOperationError>{
        if browser.id()==0{
            let _=browser.kill();let _=browser.wait();
            return Err(WindowOperationError::ChildNotOwned);
        }
        Ok(Self{browser})
    }
    #[cfg(not(any(windows,target_os="linux")))]
    pub fn new(_browser:Child)->Result<Self,WindowOperationError>{
        Err(WindowOperationError::UnsupportedPlatform)
    }
    pub fn child_id(&self)->u32 {
        #[cfg(windows)]
        {return self.browser.id()}
        #[cfg(target_os="linux")]
        {return self.kernel_pid.or_else(||self.browser.as_ref().map(Child::id))
            .unwrap_or(0)}
        #[cfg(not(any(windows,target_os="linux")))]
        {0}
    }
    pub fn is_alive(&mut self)->bool {
        #[cfg(windows)]
        {return matches!(self.browser.try_wait(),Ok(None))}
        #[cfg(target_os="linux")]
        {
            if let (Some(pid),Some(leaf))=(self.kernel_pid,self.kernel_leaf.as_deref()){
                return original_leaf_member(pid,leaf);
            }
            self.browser.as_mut().is_some_and(|c|matches!(c.try_wait(),Ok(None)))
        }
        #[cfg(not(any(windows,target_os="linux")))]
        {false}
    }
    pub fn set_visible(&mut self,visible:bool)->Result<(),WindowOperationError>{
        if !self.is_alive(){return Err(WindowOperationError::ChildNotOwned);}
        #[cfg(windows)]
        return lu_win_worker_job::browser_window::set_owned_browser_visible(
            &self.browser,visible).map_err(|_|WindowOperationError::NativeFailure);
        #[cfg(target_os="linux")]
        return crate::browser_window_x11::set_verified_browser_pid_visible(
            self.child_id(),&self.trusted_xdotool,visible)
            .map_err(|_|WindowOperationError::NativeFailure);
        #[cfg(not(any(windows,target_os="linux")))]
        {let _=visible;Err(WindowOperationError::UnsupportedPlatform)}
    }
    pub fn hide_after_human_review(&mut self)->Result<(),WindowOperationError>{
        self.set_visible(false)
    }
}
#[cfg(target_os="linux")]
fn original_leaf_member(pid:u32,leaf:&Path)->bool{
    if pid<=1{return false}
    let meta=fs::symlink_metadata(leaf);
    if !meta.is_ok_and(|m|m.is_dir()&&!m.file_type().is_symlink()
        &&m.uid()==0&&m.permissions().mode()&0o022==0){return false}
    let Ok(relative)=leaf.strip_prefix("/sys/fs/cgroup") else{return false};
    let expected=format!("0::/{}",relative.to_string_lossy().trim_start_matches('/'));
    fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .is_ok_and(|s|s.trim_end()==expected)
}
impl Drop for OwnedBrowserWindow{
    fn drop(&mut self){
        #[cfg(windows)]
        {let _=self.browser.kill();let _=self.browser.wait();}
        #[cfg(target_os="linux")]
        if let Some(mut child)=self.browser.take(){
            let _=child.kill();let _=child.wait();
        }
        // SEC-06C's original kernel service, never a saved/possibly reused
        // Chromium PID, terminates the full browser subtree on Linux.
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn window_interface_never_exposes_untrusted_pid_selection(){
        let _method:fn(&mut OwnedBrowserWindow,bool)
            ->Result<(),WindowOperationError>=OwnedBrowserWindow::set_visible;
    }
}
