//! TOOLS-10B: Tauri-owned browser runtime boundary.
//!
//! Until the Chromium/Node/Playwright package has pinned release digests,
//! browser execution stays unavailable. An ordinary desktop command must never
//! treat installed browsers, PATH, or a model-provided executable as trusted.
//! Runtime status does not authorize navigation.
use lu_core::{
    browser_preferences::BrowserPreferences,
    browser_worker_supervisor::{OwnedBrowserWorker,BrowserLaunchConfig,BrowserOperation},
    browser_chromium_launcher::launch_owned_headed_chromium,
};
use serde::Serialize;
use std::{
    path::{Path,PathBuf},
    sync::Mutex,
    time::Duration,
};

pub const BROWSER_PACKAGE_SUBDIR:&str="lu-browser";
pub const BROWSER_WORKER_NAME:&str="lu_browser_worker.mjs";
#[cfg(windows)]
pub const CHROMIUM_NAME:&str="chrome.exe";
#[cfg(windows)]
pub const NODE_NAME:&str="node.exe";
#[cfg(target_os="linux")]
pub const CHROMIUM_NAME:&str="chrome";
#[cfg(target_os="linux")]
pub const NODE_NAME:&str="node";
#[cfg(not(any(windows,target_os="linux")))]
pub const CHROMIUM_NAME:&str="unsupported-chromium";
#[cfg(not(any(windows,target_os="linux")))]
pub const NODE_NAME:&str="unsupported-node";

// These are deliberately absent pending a reviewed, reproducible browser
// package. Adding binaries to app resources alone does NOT authorize launch.
// Release engineering must pin the exact length and SHA-256 for every binary,
// the JS worker and its Playwright runtime before enabling the launcher.
#[derive(Debug,Clone,Copy)]
struct ComponentPin {
    sha256:&'static str,
    len:u64,
}
const CHROMIUM_PIN:Option<ComponentPin>=None;
const NODE_PIN:Option<ComponentPin>=None;
const WORKER_PIN:Option<ComponentPin>=None;
const PLAYWRIGHT_PIN:Option<ComponentPin>=None;
const PLAYWRIGHT_ENTRY:&str="node_modules/playwright/index.mjs";

#[derive(Debug,Serialize,Clone,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct BrowserRuntimeView {
    pub visibility_mode:String,
    pub package_available:bool,
    pub launch_ready:bool,
    pub running:bool,
    pub reason:String,
}
pub struct DesktopBrowserHost {
    resources:PathBuf,
    app_data:PathBuf,
    session:Mutex<Option<OwnedBrowserWorker>>,
}
fn pin_matches(path:&Path,pin:Option<ComponentPin>)->bool {
    use sha2::{Digest,Sha256};
    use std::{fs::File,io::Read};
    let Some(pin)=pin else {return false;};
    let Ok(meta)=std::fs::symlink_metadata(path) else{return false;};
    if !meta.file_type().is_file()||meta.len()!=pin.len||pin.sha256.len()!=64 {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {return false;}
    }
    let Ok(mut file)=File::open(path) else{return false;};
    let mut hasher=Sha256::new();
    let mut buf=[0u8;65536];
    loop {
        let Ok(n)=file.read(&mut buf) else{return false;};
        if n==0 {break;}
        hasher.update(&buf[..n]);
    }
    format!("{:x}",hasher.finalize())==pin.sha256
}
impl DesktopBrowserHost {
    pub fn new(resources:PathBuf,app_data:PathBuf)->Self {
        Self {resources,app_data,session:Mutex::new(None)}
    }
    fn package_dir(&self)->PathBuf {self.resources.join(BROWSER_PACKAGE_SUBDIR)}
    fn packaged_components_present(&self)->bool {
        let dir=self.package_dir();
        [CHROMIUM_NAME,NODE_NAME,BROWSER_WORKER_NAME,PLAYWRIGHT_ENTRY]
            .iter().all(|name|dir.join(name).is_file())
    }
    fn package_verified(&self)->bool {
        let dir=self.package_dir();
        pin_matches(&dir.join(CHROMIUM_NAME),CHROMIUM_PIN)
            &&pin_matches(&dir.join(NODE_NAME),NODE_PIN)
            &&pin_matches(&dir.join(BROWSER_WORKER_NAME),WORKER_PIN)
            &&pin_matches(&dir.join(PLAYWRIGHT_ENTRY),PLAYWRIGHT_PIN)
    }
    pub fn status(&self,preferences:&BrowserPreferences)->Result<BrowserRuntimeView,String>{
        let mode=preferences.visibility()
            .map_err(|e|format!("browser preferences unavailable: {e:?}"))?;
        let running=self.session.lock().map_err(|_|"browser supervisor lock poisoned")?
            .is_some();
        let package_available=self.packaged_components_present();
        let launch_ready=self.package_verified();
        let reason=if launch_ready{
            "Verified browser package ready".into()
        }else if package_available{
            "Browser files exist, but signed/pinned runtime admission is not complete".into()
        }else{
            "Lu Chromium/Playwright runtime is not packaged in this build".into()
        };
        Ok(BrowserRuntimeView{
            visibility_mode:mode.worker_mode().to_string(),
            package_available,launch_ready,running,reason
        })
    }
    /// Host-only entrypoint. The caller must first establish a separate
    /// trusted per-turn research grant; no WebView/model command maps to this.
    /// A verified signed browser package is mandatory before spawning.
    #[allow(dead_code)]
    pub(crate) fn launch_for_native_grant(
        &self, approved_origins:Vec<String>,preferences:&BrowserPreferences
    )->Result<(),String>{
        self.require_launch_ready()?;
        let mut owned=self.session.lock()
            .map_err(|_|"browser supervisor lock poisoned")?;
        if owned.is_some(){return Err("Research browser is already running".into());}
        // The browser profile is in Lu app data, never inside installed
        // resources and never selected by a model response.
        let profile=self.app_data.join("lu-browser-profile");
        std::fs::create_dir_all(&profile)
            .map_err(|e|format!("cannot initialize browser profile: {e}"))?;
        let metadata=std::fs::symlink_metadata(&profile)
            .map_err(|e|format!("cannot verify browser profile: {e}"))?;
        if !metadata.file_type().is_dir()||metadata.file_type().is_symlink(){
            return Err("Browser profile is not an ordinary directory".into());
        }
        let preference=preferences.visibility()
            .map_err(|e|format!("browser visibility unavailable: {e:?}"))?;
        let directory=self.package_dir();
        let launched=launch_owned_headed_chromium(
            &directory.join(CHROMIUM_NAME),&profile,Duration::from_secs(20)
        ).map_err(|e|format!("browser process launch refused: {e:?}"))?;
        let config=BrowserLaunchConfig {
            trusted_node:directory.join(NODE_NAME),
            trusted_script:directory.join(BROWSER_WORKER_NAME),
            trusted_profile:profile,
            approved_origins,
            cdp_endpoint:None,
            visibility:preference,
            command_timeout:Duration::from_secs(25),
        };
        #[cfg(windows)]
        let worker=OwnedBrowserWorker::launch_attached(config,launched);
        #[cfg(target_os="linux")]
        let worker=OwnedBrowserWorker::launch_attached(config,launched,
            directory.join("xdotool"));
        #[cfg(not(any(windows,target_os="linux")))]
        return Err("Native browser host unsupported on this OS".into());
        #[cfg(any(windows,target_os="linux"))]
        {
            let mut worker=worker.map_err(|e|format!("browser worker refused: {e:?}"))?;
            let status=worker.execute(BrowserOperation::Status,serde_json::json!({}))
                .map_err(|e|format!("browser startup handshake failed: {e:?}"))?;
            if status.get("ok").and_then(serde_json::Value::as_bool)!=Some(true){
                return Err("Browser worker did not acknowledge startup".into());
            }
            *owned=Some(worker);
            Ok(())
        }
    }
    pub fn stop(&self)->Result<bool,String>{
        let mut session=self.session.lock()
            .map_err(|_|"browser supervisor lock poisoned")?;
        Ok(session.take().is_some()) // Drops worker; kills/reaps child processes
    }
    /// Host call only. The native Tauri API must not accept a model-supplied
    /// executable, browser profile, PID or uncontrolled origin grant.
    pub fn require_launch_ready(&self)->Result<(),String>{
        if !self.package_verified(){
            return Err("Lu browser runtime unavailable: missing reviewed, pinned package".into());
        }
        Ok(())
    }
}
impl Drop for DesktopBrowserHost {
    fn drop(&mut self) {
        if let Ok(mut guard)=self.session.lock() {guard.take();}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_unsigned_or_unpinned_package_can_launch(){
        let host=DesktopBrowserHost::new(PathBuf::from("/not/an/installed/lu/bundle"),
            PathBuf::from("/not/an/installed/lu/data"));
        assert!(!host.packaged_components_present());
        assert!(!host.package_verified());
        assert!(host.require_launch_ready().is_err());
    }
    #[test]
    fn preferences_and_process_ownership_are_separate_authorities(){
        let _mode=lu_core::browser_visibility_settings::BrowserVisibilityPreference::default();
        let _status:fn(&DesktopBrowserHost,&BrowserPreferences)
            ->Result<BrowserRuntimeView,String>=DesktopBrowserHost::status;
        let _stop:fn(&DesktopBrowserHost)->Result<bool,String>=DesktopBrowserHost::stop;
    }
}
