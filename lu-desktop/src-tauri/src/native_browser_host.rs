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
    browser_authorized_execution::{BrowserOriginScope,execute_authorized_browser_operation},
    conversation_authority::{ConversationAuthority,BoundCallOrigin},
    contracts::{ToolAuthorizer,ToolInvocation},
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

// The manifest is compiled into Lu's native binary. The bundled on-disk
// manifest and EVERY runtime component must match these baked bytes/digests.
// The checked-in manifest intentionally has status=not-packaged; staging a
// reviewed package before building produces a separately pinned app binary.
const EMBEDDED_BROWSER_MANIFEST:&str=include_str!("../resources/lu-browser/manifest.json");
#[derive(Debug,serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserManifest {
    schema:u32,
    runtime:String,
    status:String,
    platform:String,
    arch:String,
    #[serde(rename="playwrightVersion")]
    playwright_version:String,
    files:Vec<BrowserManifestEntry>,
}
#[derive(Debug,serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserManifestEntry {
    path:String,
    sha256:String,
    bytes:u64,
}
const PLAYWRIGHT_ENTRY:&str="node_modules/playwright/index.mjs";

#[derive(Debug,Serialize,Clone,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct BrowserRuntimeView {
    pub visibility_mode:String,
    pub package_available:bool,
    pub launch_ready:bool,
    pub running:bool,
    pub human_verification_required:bool,
    pub window_visibility:Option<String>,
    pub reason:String,
}
pub struct DesktopBrowserHost {
    resources:PathBuf,
    app_data:PathBuf,
    session:Mutex<Option<OwnedBrowserWorker>>,
}
fn safe_manifest_path(path:&str)->bool{
    !path.is_empty()&&path.len()<240&&!path.starts_with('/')
      &&path.split('/').all(|part|!part.is_empty()&&part!="."&&part!=".."
        &&part.trim()==part
        &&part.bytes().all(|c|c.is_ascii_alphanumeric()
          ||matches!(c,b'.'|b'_'|b'-'|b' ')))
}
fn pin_matches(path:&Path,expected_sha:&str,expected_len:u64)->bool {
    use sha2::{Digest,Sha256};
    use std::{fs::File,io::Read};
    let Ok(meta)=std::fs::symlink_metadata(path) else{return false;};
    if !meta.file_type().is_file()||meta.len()!=expected_len
       ||expected_sha.len()!=64||!expected_sha.bytes().all(|b|b.is_ascii_hexdigit()){
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
    format!("{:x}",hasher.finalize())==expected_sha
}
fn enumerate_package_files(dir:&Path,base:&Path,out:&mut Vec<String>)->bool {
    let Ok(entries)=std::fs::read_dir(dir) else{return false;};
    for entry in entries {
        let Ok(entry)=entry else{return false;};
        let path=entry.path();
        let Ok(meta)=std::fs::symlink_metadata(&path) else{return false;};
        if meta.file_type().is_symlink(){return false;}
        if meta.is_dir(){
            if !enumerate_package_files(&path,base,out){return false;}
        }else if meta.is_file(){
            let Ok(relative)=path.strip_prefix(base) else{return false;};
            let Some(name)=relative.to_str() else{return false;};
            let name=name.replace('\\',"/");
            if !safe_manifest_path(&name){return false;}
            out.push(name);
        }else{return false;}
        if out.len()>10000{return false;}
    }
    true
}
impl DesktopBrowserHost {
    /// SEC-06C sole Linux authority: the old direct Node/Chromium spawns
    /// must never turn a reviewed browser grant into uncontained execution.
    /// This intentionally remains a denial until the dedicated finite,
    /// jointly OS-owned browser session is installed and natively verified.
    fn require_browser_kernel_custody()->Result<(),String>{
        #[cfg(target_os="linux")]
        {return Err("SEC06C_BROWSER_JOINT_KERNEL_CUSTODY_UNAVAILABLE".into())}
        #[cfg(not(target_os="linux"))]
        {Ok(())}
    }
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
        use std::collections::BTreeSet;
        let Ok(manifest)=serde_json::from_str::<BrowserManifest>(EMBEDDED_BROWSER_MANIFEST)
            else{return false;};
        let platform=if cfg!(windows){"win32"}else if cfg!(target_os="linux"){"linux"}else{"unsupported"};
        let arch=if cfg!(target_arch="x86_64"){"x64"}else{"unsupported"};
        if manifest.schema!=1||manifest.runtime!="lu-browser"
          ||manifest.status!="reviewed"||manifest.platform!=platform
          ||manifest.arch!=arch||manifest.playwright_version!="1.64.1"
          ||manifest.files.len()<4||manifest.files.len()>10000{
            return false;
        }
        let dir=self.package_dir();
        if std::fs::read_to_string(dir.join("manifest.json")).ok().as_deref()
            !=Some(EMBEDDED_BROWSER_MANIFEST){return false;}
        let mut expected=BTreeSet::new();
        for file in &manifest.files{
            if !safe_manifest_path(&file.path)||!expected.insert(file.path.clone())
               ||!pin_matches(&dir.join(&file.path),&file.sha256,file.bytes){
                return false;
            }
        }
        let mut required=vec![CHROMIUM_NAME,NODE_NAME,BROWSER_WORKER_NAME,PLAYWRIGHT_ENTRY];
        #[cfg(target_os="linux")]
        required.push("xdotool");
        for file in required{
            if !expected.contains(file){return false;}
        }
        let mut actual=Vec::new();
        if !enumerate_package_files(&dir,&dir,&mut actual){return false;}
        actual.into_iter().collect::<BTreeSet<_>>()
            ==expected.into_iter().chain(std::iter::once("manifest.json".into())).collect()
    }
    pub fn status(&self,preferences:&BrowserPreferences)->Result<BrowserRuntimeView,String>{
        let mode=preferences.visibility()
            .map_err(|e|format!("browser preferences unavailable: {e:?}"))?;
        let mut session=self.session.lock()
            .map_err(|_|"browser supervisor lock poisoned")?;
        if session.as_mut().is_some_and(|worker|!worker.is_alive()){
            session.take(); // crash recovery closes stale worker ownership
        }
        let running=session.is_some();
        let mut human_verification_required=false;
        let mut window_visibility=None;
        if let Some(worker)=session.as_mut(){
            if let Ok(report)=worker.execute(BrowserOperation::Status,serde_json::json!({})){
                if report.get("ok").and_then(serde_json::Value::as_bool)==Some(true){
                    let result=&report["result"];
                    human_verification_required=result["status"]=="NEEDS_HUMAN";
                    window_visibility=result["windowVisibility"].as_str().map(str::to_owned);
                }
            }
        }
        drop(session);
        let package_available=self.packaged_components_present();
        let launch_ready=self.package_verified()
            &&Self::require_browser_kernel_custody().is_ok();
        let reason=if launch_ready{
            "Verified browser package ready".into()
        }else if package_available{
            "Browser files exist, but signed/pinned runtime admission is not complete".into()
        }else{
            "Lu Chromium/Playwright runtime is not packaged in this build".into()
        };
        Ok(BrowserRuntimeView{
            visibility_mode:mode.worker_mode().to_string(),
            package_available,launch_ready,running,
            human_verification_required,window_visibility,reason
        })
    }
    /// Host-only entrypoint. The caller must first establish a separate
    /// trusted per-turn research grant; no WebView/model command maps to this.
    /// A verified signed browser package is mandatory before spawning.
    #[allow(dead_code)]
    pub(crate) fn launch_for_native_grant(
        &self, approved_origins:Vec<String>,preferences:&BrowserPreferences
    )->Result<(),String>{
        Self::require_browser_kernel_custody()?;
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
    /// Internal model-tool dispatch. The host scheduler supplies the
    /// immutable task, invocation and previously approved origin scope.
    /// The WebView never gets a general-purpose browser execute command.
    #[allow(dead_code)]
    pub(crate) fn execute_authorized(
        &self,
        authority:&ConversationAuthority,
        origin:&BoundCallOrigin,
        invocation:&ToolInvocation,
        host_policy:&dyn ToolAuthorizer,
        scope:&BrowserOriginScope,
        operation:BrowserOperation,
        arguments:serde_json::Value,
    )->Result<serde_json::Value,String>{
        let mut session=self.session.lock()
            .map_err(|_|"browser supervisor lock poisoned")?;
        let worker=session.as_mut().ok_or("No browser worker is running")?;
        execute_authorized_browser_operation(
            authority,origin,invocation,host_policy,scope,
            worker,operation,arguments
        ).map_err(|e|format!("browser task rejected: {e:?}"))
    }
    /// User watch/hide controls the same Lu-owned headed browser session.
    /// Model tools never receive this control. The browser worker refuses
    /// hiding while a human challenge is unresolved.
    pub fn set_user_visible(&self,visible:bool)->Result<(),String>{
        let mut session=self.session.lock()
            .map_err(|_|"browser supervisor lock poisoned")?;
        let worker=session.as_mut().ok_or("Browser session is not running")?;
        let operation=if visible{BrowserOperation::Show}else{BrowserOperation::Hide};
        let response=worker.execute(operation,serde_json::json!({}))
            .map_err(|e|format!("browser window transition refused: {e:?}"))?;
        let expected=if visible{"VISIBLE"}else{"HIDDEN"};
        if response["ok"]!=true||response["result"]["windowVisibility"]!=expected{
            return Err("Native browser window did not confirm requested visibility".into());
        }
        Ok(())
    }
    /// A user, not a model, confirms they have completed verification.
    /// The worker independently rechecks that the challenge text is gone
    /// and that the page remains on a trusted HTTPS origin.
    pub fn resolve_human_verification(&self)->Result<(),String>{
        let mut owned=self.session.lock()
            .map_err(|_|"browser supervisor lock poisoned")?;
        let worker=owned.as_mut().ok_or("Browser session is not running")?;
        let result=worker.execute(BrowserOperation::HumanResolved,serde_json::json!({}))
            .map_err(|e|format!("browser verification resume refused: {e:?}"))?;
        if result["ok"]!=true||result["result"]["status"]!="RESUMED"{
            return Err("Browser verification remains required or was not accepted".into());
        }
        Ok(())
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
/// The only bridge used by the scoped native tool. Browser authorization,
/// typed operation/argument parity and origin grants are validated by
/// native_browser_tool immediately before entering this runner.
impl lu_core::native_browser_tool::BrowserTaskRunner for DesktopBrowserHost {
    fn run(&self,operation:BrowserOperation,arguments:serde_json::Value)
      ->Result<serde_json::Value,String>{
        let mut session=self.session.lock()
            .map_err(|_|"browser supervisor lock poisoned")?;
        let worker=session.as_mut().ok_or("Browser session is not running")?;
        if !worker.is_alive(){
            session.take();
            return Err("Browser worker or Chromium has terminated".into());
        }
        let response=worker.execute(operation,arguments)
            .map_err(|e|format!("Browser supervisor declined operation: {e:?}"))?;
        // A worker-level rejection is not a successful research source.
        // Keep the error category, never interpolate untrusted page text.
        if response.get("ok").and_then(serde_json::Value::as_bool)!=Some(true){
            let code=response.pointer("/error/code")
                .and_then(serde_json::Value::as_str)
                .filter(|s|s.len()<=64&&s.bytes().all(|b|
                    b.is_ascii_uppercase()||b==b'_'))
                .unwrap_or("BROWSER_REJECTED");
            return Err(format!("Browser worker rejected operation: {code}"));
        }
        Ok(response)
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
