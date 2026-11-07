//! OBSERVE-SCOPE-03: one shared, compositional observation boundary for a
//! user-selected window, app, monitor, or virtual desktop (including sets).
//! This is a native admission contract, NOT a screen-capture implementation.
//!
//! The trusted OS adapter must mint opaque identities from OS process/window,
//! display and virtual-desktop APIs. The WebView, LLM, test scripts and raw
//! screenshot bytes never supply trusted surface identity. Re-verify on
//! process/window re-creation, display hot-plug and workspace switching.
use std::collections::BTreeSet;

pub const MAX_SCOPE_ITEMS:usize=8;
const MAX_LEASE_MS:u64=2*60*60*1000;

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct WindowIdentity {
    pub app_id:String,
    pub window_id:String,
    /// OS-stable *instance* token, not PID alone (which may be reused).
    pub process_instance_id:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum SelectedTargets {
    Windows(Vec<WindowIdentity>),
    Applications(Vec<String>),
    Monitors(Vec<String>),
    VirtualDesktops(Vec<String>),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ScopedObservationGrant {
    pub lease_id:String,
    pub expires_at_ms:u64,
    /// Exactly one primary selection. Multiple items require an explicit
    /// user grant; absent dimensions never widen another dimension.
    pub selected:SelectedTargets,
    /// Optional intersections, not additional union-style permissions.
    pub app_filter:Vec<String>,
    pub monitor_filter:Vec<String>,
    pub virtual_desktop_filter:Vec<String>,
    pub allow_redacted_snapshots:bool,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ObservePayloadKind {
    SemanticEvent,
    WindowFrame,
    MonitorFrame,
}
/// Only a native OS adapter may produce verified SurfaceEvidence. This
/// type is never a Tauri command argument or a model-exposed schema.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SurfaceEvidence {
    pub app_id:String,
    pub window_id:String,
    pub process_instance_id:String,
    pub monitor_id:Option<String>,
    pub virtual_desktop_id:Option<String>,
    /// A virtual desktop is not a magic offscreen framebuffer. Capturing
    /// inactive desktops is never assumed to work.
    pub virtual_desktop_is_active:bool,
    pub os_identity_reverified:bool,
    pub secure_or_sensitive_surface:bool,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ScopeError {
    InvalidGrant,
    Expired,
    OsIdentityUnverified,
    ProtectedSurface,
    OutsideSelectedTargets,
    MissingMonitorProof,
    MissingDesktopProof,
    InactiveVirtualDesktop,
    SnapshotNotAuthorized,
    WrongCaptureSource,
}
fn token(s:&str)->bool {
    !s.is_empty() && s.len()<=128 &&
    s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn distinct_ids(items:&[String])->bool {
    items.len()<=MAX_SCOPE_ITEMS &&
        items.iter().all(|s|token(s)) &&
        items.iter().collect::<BTreeSet<_>>().len()==items.len()
}
impl ScopedObservationGrant {
    pub fn validate(&self,now_ms:u64)->Result<(),ScopeError>{
        if !token(&self.lease_id)||!self.lease_id.starts_with("obs-")||
            now_ms>=self.expires_at_ms||
            self.expires_at_ms-now_ms>MAX_LEASE_MS||
            !distinct_ids(&self.app_filter)||
            !distinct_ids(&self.monitor_filter)||
            !distinct_ids(&self.virtual_desktop_filter){
            return Err(ScopeError::InvalidGrant);
        }
        let valid=match &self.selected {
            SelectedTargets::Windows(xs)=>
                !xs.is_empty()&&xs.len()<=MAX_SCOPE_ITEMS&&
                xs.iter().all(|w|token(&w.app_id)&&token(&w.window_id)
                    &&token(&w.process_instance_id))&&
                xs.iter().map(|w|&w.window_id)
                    .collect::<BTreeSet<_>>().len()==xs.len(),
            SelectedTargets::Applications(xs)
            |SelectedTargets::Monitors(xs)
            |SelectedTargets::VirtualDesktops(xs)=>
                !xs.is_empty()&&distinct_ids(xs),
        };
        if !valid{return Err(ScopeError::InvalidGrant);}
        Ok(())
    }
    /// "Observe one app" and "observe one monitor" are different primary
    /// authorities. Every nonempty filter further narrows the primary.
    /// Window/monitor pixels are never interchangeable.
    pub fn check(
        &self,now_ms:u64,payload:ObservePayloadKind,surface:&SurfaceEvidence
    )->Result<(),ScopeError>{
        if now_ms>=self.expires_at_ms{return Err(ScopeError::Expired);}
        self.validate(now_ms)?;
        if !surface.os_identity_reverified{
            return Err(ScopeError::OsIdentityUnverified);
        }
        if surface.secure_or_sensitive_surface{
            return Err(ScopeError::ProtectedSurface);
        }
        // A monitor frame is a display surface, NOT an application
        // window. Its native display identity must be reverified, but it
        // cannot truthfully name a single window/app/process.
        if payload!=ObservePayloadKind::MonitorFrame
            &&(!token(&surface.app_id)||!token(&surface.window_id)
                ||!token(&surface.process_instance_id)){
            return Err(ScopeError::OsIdentityUnverified);
        }
        let selected=match &self.selected {
            SelectedTargets::Windows(windows)=>windows.iter().any(|w|
                w.window_id==surface.window_id&&
                w.app_id==surface.app_id&&
                w.process_instance_id==surface.process_instance_id),
            SelectedTargets::Applications(xs)=>xs.contains(&surface.app_id),
            SelectedTargets::Monitors(xs)=>surface.monitor_id.as_ref()
                .is_some_and(|id|xs.contains(id)),
            SelectedTargets::VirtualDesktops(xs)=>
                surface.virtual_desktop_id.as_ref()
                    .is_some_and(|id|xs.contains(id)),
        };
        if !selected{return Err(ScopeError::OutsideSelectedTargets);}
        if !self.app_filter.is_empty()
            &&!self.app_filter.contains(&surface.app_id){
            return Err(ScopeError::OutsideSelectedTargets);
        }
        if !self.monitor_filter.is_empty()
            ||matches!(self.selected,SelectedTargets::Monitors(_)){
            let monitor=surface.monitor_id.as_ref()
                .ok_or(ScopeError::MissingMonitorProof)?;
            if !token(monitor){
                return Err(ScopeError::MissingMonitorProof);
            }
            if !self.monitor_filter.is_empty()
                && !self.monitor_filter.contains(monitor){
                return Err(ScopeError::OutsideSelectedTargets);
            }
        }
        if !self.virtual_desktop_filter.is_empty()
            ||matches!(self.selected,SelectedTargets::VirtualDesktops(_)){
            let desktop=surface.virtual_desktop_id.as_ref()
                .ok_or(ScopeError::MissingDesktopProof)?;
            if !token(desktop){
                return Err(ScopeError::MissingDesktopProof);
            }
            if !self.virtual_desktop_filter.is_empty()
                &&!self.virtual_desktop_filter.contains(desktop){
                return Err(ScopeError::OutsideSelectedTargets);
            }
        }
        // Do not switch workspaces to fulfill an observation grant.
        if !surface.virtual_desktop_is_active{
            return Err(ScopeError::InactiveVirtualDesktop);
        }
        if payload!=ObservePayloadKind::SemanticEvent{
            if !self.allow_redacted_snapshots{
                return Err(ScopeError::SnapshotNotAuthorized);
            }
            match payload {
                ObservePayloadKind::WindowFrame=>
                    if matches!(self.selected,SelectedTargets::Monitors(_)){
                        return Err(ScopeError::WrongCaptureSource);
                    },
                ObservePayloadKind::MonitorFrame=>
                    if !matches!(self.selected,SelectedTargets::Monitors(_))
                    || !self.app_filter.is_empty(){
                        // A full-monitor frame may contain apps outside an
                        // allowlisted app filter, even if one matches.
                        return Err(ScopeError::WrongCaptureSource);
                    },
                ObservePayloadKind::SemanticEvent=>(),
            }
        }
        Ok(())
    }
    /// App-based UI status hints are weaker than native OS observations.
    /// Only the already-existing Lu-only pilot uses this method. It MUST
    /// NOT authorize frames, verified test results, memory or OS actions.
    pub fn accepts_untrusted_app_hint(&self,app_id:&str)->bool{
        !self.allow_redacted_snapshots&&
        matches!(&self.selected,SelectedTargets::Applications(xs)
            if xs.len()==1&&xs[0]=="lu-desktop"&&app_id=="lu-desktop")&&
        self.app_filter.is_empty()&&self.monitor_filter.is_empty()&&
        self.virtual_desktop_filter.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn grant(selected:SelectedTargets,frames:bool)->ScopedObservationGrant{
        ScopedObservationGrant{
            lease_id:"obs-permitted".into(),expires_at_ms:60_000,
            selected,app_filter:vec![],monitor_filter:vec![],
            virtual_desktop_filter:vec![],allow_redacted_snapshots:frames,
        }
    }
    fn seen()->SurfaceEvidence{SurfaceEvidence{
        app_id:"lu-desktop".into(),window_id:"wnd-1".into(),
        process_instance_id:"pid-start-1".into(),
        monitor_id:Some("display-a".into()),
        virtual_desktop_id:Some("vd-1".into()),
        virtual_desktop_is_active:true,
        os_identity_reverified:true,secure_or_sensitive_surface:false,
    }}
    #[test]
    fn one_window_rejects_window_recreation_and_other_apps(){
        let g=grant(SelectedTargets::Windows(vec![WindowIdentity{
            app_id:"lu-desktop".into(),window_id:"wnd-1".into(),
            process_instance_id:"pid-start-1".into(),
        }]),true);
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&seen()),Ok(()));
        let mut v=seen();v.process_instance_id="pid-start-2".into();
        assert_eq!(g.check(2,ObservePayloadKind::WindowFrame,&v),
            Err(ScopeError::OutsideSelectedTargets));
        v=seen();v.window_id="wnd-2".into();
        assert_eq!(g.check(2,ObservePayloadKind::WindowFrame,&v),
            Err(ScopeError::OutsideSelectedTargets));
        assert_eq!(g.check(2,ObservePayloadKind::MonitorFrame,&seen()),
            Err(ScopeError::WrongCaptureSource));
    }
    #[test]
    fn one_or_multiple_apps_only_reveal_explicitly_approved_apps(){
        let g=grant(SelectedTargets::Applications(vec![
            "lu-desktop".into(),"lu-ide".into()
        ]),false);
        assert_eq!(g.check(1,ObservePayloadKind::SemanticEvent,&seen()),Ok(()));
        let mut v=seen();v.app_id="lu-ide".into();
        assert_eq!(g.check(2,ObservePayloadKind::SemanticEvent,&v),Ok(()));
        v.app_id="private-browser".into();
        assert_eq!(g.check(3,ObservePayloadKind::SemanticEvent,&v),
            Err(ScopeError::OutsideSelectedTargets));
        assert_eq!(g.check(3,ObservePayloadKind::WindowFrame,&seen()),
            Err(ScopeError::SnapshotNotAuthorized));
    }
    #[test]
    fn monitors_are_not_the_same_as_apps_or_virtual_workspaces(){
        let g=grant(SelectedTargets::Monitors(vec!["display-a".into()]),true);
        assert_eq!(g.check(1,ObservePayloadKind::MonitorFrame,&seen()),Ok(()));
        // No fake per-window identity is required for an OS display surface.
        let mut display=seen();
        display.app_id.clear();
        display.window_id.clear();
        display.process_instance_id.clear();
        assert_eq!(g.check(1,ObservePayloadKind::MonitorFrame,&display),Ok(()));
        let mut v=seen();v.monitor_id=Some("display-b".into());
        assert_eq!(g.check(1,ObservePayloadKind::MonitorFrame,&v),
            Err(ScopeError::OutsideSelectedTargets));
        v=seen();v.monitor_id=None;
        assert_eq!(g.check(1,ObservePayloadKind::MonitorFrame,&v),
            Err(ScopeError::OutsideSelectedTargets));
        assert_eq!(g.check(2,ObservePayloadKind::WindowFrame,&seen()),
            Err(ScopeError::WrongCaptureSource));
    }
    #[test]
    fn multiple_virtual_desktops_require_active_proven_window_membership(){
        let g=grant(SelectedTargets::VirtualDesktops(vec![
            "vd-1".into(),"vd-2".into()
        ]),true);
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&seen()),Ok(()));
        let mut v=seen();v.virtual_desktop_id=Some("vd-2".into());
        v.virtual_desktop_is_active=false;
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&v),
            Err(ScopeError::InactiveVirtualDesktop));
        v.virtual_desktop_is_active=true;
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&v),Ok(()));
        v.virtual_desktop_id=Some("vd-3".into());
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&v),
            Err(ScopeError::OutsideSelectedTargets));
        assert_eq!(g.check(1,ObservePayloadKind::MonitorFrame,&seen()),
            Err(ScopeError::WrongCaptureSource));
    }
    #[test]
    fn filters_are_intersections_not_scope_widening(){
        let mut g=grant(SelectedTargets::Applications(vec!["lu-desktop".into()]),true);
        g.monitor_filter=vec!["display-a".into()];
        g.virtual_desktop_filter=vec!["vd-1".into()];
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&seen()),Ok(()));
        let mut v=seen();v.monitor_id=Some("display-b".into());
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&v),
            Err(ScopeError::OutsideSelectedTargets));
        v=seen();v.virtual_desktop_id=None;
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&v),
            Err(ScopeError::MissingDesktopProof));
        v=seen();v.os_identity_reverified=false;
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&v),
            Err(ScopeError::OsIdentityUnverified));
    }
    #[test]
    fn secure_scope_expiry_missing_proof_and_invalid_grants_fail_closed(){
        let g=grant(SelectedTargets::Applications(vec!["lu-desktop".into()]),true);
        let mut v=seen();v.secure_or_sensitive_surface=true;
        assert_eq!(g.check(1,ObservePayloadKind::WindowFrame,&v),
            Err(ScopeError::ProtectedSurface));
        assert_eq!(g.check(60_000,ObservePayloadKind::WindowFrame,&seen()),
            Err(ScopeError::Expired));
        let mut bad=grant(SelectedTargets::Monitors(vec![
            "display-a".into(),"display-a".into()
        ]),true);
        assert_eq!(bad.validate(1),Err(ScopeError::InvalidGrant));
        bad.selected=SelectedTargets::Applications(vec![]);
        assert_eq!(bad.validate(1),Err(ScopeError::InvalidGrant));
        assert!(!g.accepts_untrusted_app_hint("lu-desktop"));
        let pilot=grant(SelectedTargets::Applications(vec!["lu-desktop".into()]),false);
        assert!(pilot.accepts_untrusted_app_hint("lu-desktop"));
        assert!(!pilot.accepts_untrusted_app_hint("other-app"));
    }
}
