//! Host-owned browser process/window custody. This is not a generic
//! "hide a window by PID" tool; the only target is a Child owned by Lu.
//! The future Chromium launcher must transfer its directly spawned browser
//! process here, not merely the Node/Playwright worker process.
//! No model-provided HWND, PID or executable is accepted.
use std::{io,process::Child};
#[cfg(target_os="linux")]
use std::path::PathBuf;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum WindowOperationError {
    UnsupportedPlatform,
    NativeFailure,
    ChildNotOwned,
}
pub struct OwnedBrowserWindow {
    browser:Child,
    #[cfg(target_os="linux")]
    trusted_xdotool:PathBuf,
}
impl OwnedBrowserWindow {
    #[cfg(target_os="linux")]
    pub fn new(browser:Child,trusted_xdotool:PathBuf)
        ->Result<Self,WindowOperationError>{
        if browser.id()==0 || !trusted_xdotool.is_absolute()
          || !trusted_xdotool.is_file(){
            return Err(WindowOperationError::ChildNotOwned);
        }
        Ok(Self{browser,trusted_xdotool})
    }
    #[cfg(windows)]
    pub fn new(browser:Child)->Result<Self,WindowOperationError>{
        if browser.id()==0 {return Err(WindowOperationError::ChildNotOwned);}
        Ok(Self{browser})
    }
    #[cfg(not(any(windows,target_os="linux")))]
    pub fn new(_browser:Child)->Result<Self,WindowOperationError>{
        Err(WindowOperationError::UnsupportedPlatform)
    }
    pub fn child_id(&self)->u32 {self.browser.id()}
    pub fn set_visible(&mut self,visible:bool)->Result<(),WindowOperationError>{
        if self.browser.try_wait().map_err(|_|WindowOperationError::NativeFailure)?
           .is_some(){return Err(WindowOperationError::ChildNotOwned);}
        #[cfg(windows)]
        return lu_win_worker_job::browser_window::set_owned_browser_visible(
            &self.browser,visible).map_err(|_|WindowOperationError::NativeFailure);
        #[cfg(target_os="linux")]
        return crate::browser_window_x11::set_owned_browser_visible(
            &self.browser,&self.trusted_xdotool,visible)
            .map_err(|_|WindowOperationError::NativeFailure);
        #[cfg(not(any(windows,target_os="linux")))]
        {let _=visible;Err(WindowOperationError::UnsupportedPlatform)}
    }
    /// Called only after trusted user takeover is completed. A failed
    /// native hide leaves the caller in visible/human-control state.
    pub fn hide_after_human_review(&mut self)->Result<(),WindowOperationError>{
        self.set_visible(false)
    }
}
impl Drop for OwnedBrowserWindow{
    fn drop(&mut self){
        // Killing and reaping the owned browser process prevents orphaned
        // headed browsers on crash/recovery. Platform process-group/Job
        // containment is a *separate* integration acceptance gate.
        let _=self.browser.kill();
        let _=self.browser.wait();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_interface_does_not_expose_pid_or_title_selection(){
        // Ensure window control is structurally tied to a Child; no
        // model-facing window-select or arbitrary process-scanning API.
        let _method:fn(&mut OwnedBrowserWindow,bool)
          ->Result<(),WindowOperationError>=OwnedBrowserWindow::set_visible;
    }
}
