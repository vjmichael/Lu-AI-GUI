//! Observe & Learn desktop host adapter. This shares the existing TaskStore
//! and in-process Lu host; it does not spawn an agent runtime or observe the
//! rest of the OS. Initial source is Lu-UI semantic events ONLY.
//! The WebView is NOT an independent test oracle: no renderer event is
//! promoted to verified PASS/FAIL, memory, screen or computer authority.
use std::{sync::{Arc,Mutex},time::Instant};
use serde::Serialize;
use lu_core::{
    native_observe_learn::{Consent,Controller,Decision,Error as ObserveError,Mode,NativeEvent,Signal},
    native_observe_scope::{ScopedObservationGrant,SelectedTargets},
    native_awareness_lane::{AwarenessLane,NativeSignal as AwarenessSignal,
        Signal as AwarenessKind,Source as AwarenessSource,HostReadiness},
    tasks::TaskStore,
    native_background_process::{BackgroundProcessSupervisor,TrustedWorkerPlan},
};

const LEASE_MS:u64=30*60*1000;
const APP_ID:&str="lu-desktop";

#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ObserveView {
    pub mode:&'static str,
    pub session_id:Option<String>,
    pub chat_id:Option<String>,
    pub app_scope:&'static str,
    pub can_see_desktop:bool,
    pub screen_capture_enabled:bool,
    pub native_event_count:u64,
    pub inference_recommendations:u64,
    pub inference_performed:u64,
    pub notes_logged:u64,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ObserveNoteView{
    pub sequence:u64,
    pub kind:String,
    pub source:String,
    pub priority:String,
    pub created_at:String,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ObserveHandoffView{
    pub session_id:String,
    pub verified_failures:usize,
    pub unverified_indicators:usize,
    pub recommendation:String,
    pub requires_separate_tool_authorization:bool,
    pub main_model_may_treat_as_verified_failure:bool,
}

struct Session {
    controller:Controller,
    awareness:Option<AwarenessLane>,
    notes_logged:u64,
    scope:Option<ScopedObservationGrant>,
    capture_task_id:Option<String>,
    session_id:Option<String>,
    chat_id:Option<String>,
    sequence:u64,
    event_count:u64,
    recommendations:u64,
}
pub(crate) struct ObserveHost {
    inner:Mutex<Session>,
    task_store:Arc<TaskStore>,
    capture_processes:Arc<BackgroundProcessSupervisor>,
    epoch:Instant,
}
fn format_error(e:ObserveError)->String{format!("Observe session refused: {e:?}")}
impl ObserveHost {
    pub(crate) fn new(task_store:Arc<TaskStore>,capture_processes:Arc<BackgroundProcessSupervisor>)->Self{
        Self{
            task_store,capture_processes,
            epoch:Instant::now(),
            inner:Mutex::new(Session{
                controller:Controller::default(),awareness:None,notes_logged:0,
                scope:None,capture_task_id:None,
                session_id:None,chat_id:None,sequence:0,
                event_count:0,recommendations:0,
            }),
        }
    }
    fn now(&self)->u64{self.epoch.elapsed().as_millis().min(u128::from(u64::MAX)) as u64}
    fn view(s:&Session)->ObserveView{
        ObserveView{
            mode:match s.controller.mode(){
                Mode::Off=>"off",Mode::Active=>"active",Mode::Paused=>"paused",
            },
            session_id:s.session_id.clone(),
            chat_id:s.chat_id.clone(),
            app_scope:"Lu application UI events only",
            can_see_desktop:false,
            screen_capture_enabled:false,
            native_event_count:s.event_count,
            inference_recommendations:s.recommendations,
            inference_performed:0,
            notes_logged:s.notes_logged,
        }
    }
    /// Called only after the separate native dialog explicitly confirmed a
    /// time-limited observation of Lu's own interface.
    pub(crate) fn start(&self,chat_id:&str,session_id:String)->Result<ObserveView,String>{
        if !chat_id.starts_with("c-")||chat_id.len()>128||
            !chat_id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_'){
            return Err("Invalid conversation for observation".into());
        }
        let mut session=self.inner.lock().map_err(|_|"Observer lock unavailable")?;
        if session.controller.mode()!=Mode::Off{
            return Err("Stop the existing observation first".into());
        }
        let now=self.now();
        // The pilot has exactly one *host-selected* target, Lu itself.
        // Future OS pickers may construct other scope variants, but an
        // untrusted renderer cannot widen this target list.
        let bound_scope=ScopedObservationGrant{
            lease_id:session_id.clone(),
            expires_at_ms:now.saturating_add(LEASE_MS),
            selected:SelectedTargets::Applications(vec![APP_ID.to_owned()]),
            app_filter:vec![],monitor_filter:vec![],
            virtual_desktop_filter:vec![],
            allow_redacted_snapshots:false,
        };
        bound_scope.validate(now)
            .map_err(|_|"Invalid native observation scope")?;
        session.controller.start(Consent{
            session_id:session_id.clone(),
            app_ids:vec![APP_ID.to_owned()],
            expires_ms:now.saturating_add(LEASE_MS),
            allow_redacted_snapshot:false,
        },now).map_err(format_error)?;
        if let Err(e)=self.task_store.create(&session_id,Some(chat_id),None){
            session.controller.stop();
            return Err(format!("Could not record observation task: {e:?}"));
        }
        if let Err(e)=self.task_store.start(&session_id){
            session.controller.stop();
            let _=self.task_store.fail_pending(&session_id);
            return Err(format!("Could not start observation task: {e:?}"));
        }
        session.awareness=Some(AwarenessLane::new(&session_id,chat_id)
            .map_err(|_|"Could not initialize scoped awareness lane")?);
        session.notes_logged=0;
        session.scope=Some(bound_scope);
        session.capture_task_id=None;
        session.session_id=Some(session_id);
        session.chat_id=Some(chat_id.to_owned());
        session.sequence=0;session.event_count=0;session.recommendations=0;
        Ok(Self::view(&session))
    }
    /// Immediately wipe the in-memory consent before marking the durable
    /// task terminal. No worker process is associated with this event session.
    fn revoke(&self,session:&mut Session)->Result<(),String>{
        // Stop any captured window/display operation (including an open OS
        // picker) before dropping consent. Uncertain reaping is an error.
        if let Some(capture_id)=session.capture_task_id.as_ref(){
            let _=self.capture_processes.poll();
            if self.capture_processes.can_cancel(capture_id){
                let chat=session.chat_id.as_deref()
                    .ok_or("Missing owned capture workspace")?;
                self.capture_processes.cancel_owned(capture_id,chat)
                    .map_err(|e|format!("Capture process could not be reaped: {e:?}"))?;
            }
        }
        session.capture_task_id=None;
        let task_id=session.session_id.take();
        session.controller.stop();
        if let Some(mut lane)=session.awareness.take(){lane.stop();}
        session.scope=None;
        session.chat_id=None;
        session.sequence=0;
        if let Some(task_id)=task_id{
            self.task_store.cancelled(&task_id)
                .map_err(|e|format!("Observation was revoked, task receipt pending: {e:?}"))?;
        }
        Ok(())
    }
    fn refresh_expiry(&self,s:&mut Session)->Result<(),String>{
        if s.controller.mode()!=Mode::Off
            &&matches!(s.controller.tick(self.now()),Err(ObserveError::Expired)){
            self.revoke(s)?;
        }
        Ok(())
    }
    pub(crate) fn status(&self,chat_id:&str)->Result<ObserveView,String>{
        let mut session=self.inner.lock().map_err(|_|"Observer lock unavailable")?;
        self.refresh_expiry(&mut session)?;
        if session.chat_id.as_deref()!=Some(chat_id){
            // Observation must not remain silently active when the user
            // leaves the consenting conversation (including a new chat).
            if session.controller.mode()!=Mode::Off{
                self.revoke(&mut session)?;
            }
            return Ok(Self::view(&Session{
                controller:Controller::default(),awareness:None,notes_logged:0,
                scope:None,capture_task_id:None,
                session_id:None,chat_id:None,
                sequence:0,event_count:0,recommendations:0,
            }));
        }
        Ok(Self::view(&session))
    }
    pub(crate) fn tick(&self){
        if let Ok(mut session)=self.inner.lock(){
            let _=self.refresh_expiry(&mut session);
        }
    }
    fn owned<'a>(s:&'a mut Session,chat_id:&str)->Result<&'a mut Session,String>{
        if s.chat_id.as_deref()!=Some(chat_id){
            return Err("Observation belongs to a different conversation".into());
        }
        Ok(s)
    }
    pub(crate) fn pause(&self,chat_id:&str)->Result<ObserveView,String>{
        let mut s=self.inner.lock().map_err(|_|"Observer lock unavailable")?;
        self.refresh_expiry(&mut s)?;
        let s=Self::owned(&mut s,chat_id)?;
        if s.controller.mode()!=Mode::Active{return Err("Observation is not active".into());}
        let task_id=s.session_id.as_deref().ok_or("Missing observation task")?;
        self.task_store.pause(task_id)
            .map_err(|e|format!("Cannot pause observation task: {e:?}"))?;
        if let Err(e)=s.controller.pause(){
            s.controller.stop();
            return Err(format_error(e));
        }
        Ok(Self::view(s))
    }
    pub(crate) fn resume(&self,chat_id:&str)->Result<ObserveView,String>{
        let mut s=self.inner.lock().map_err(|_|"Observer lock unavailable")?;
        self.refresh_expiry(&mut s)?;
        let s=Self::owned(&mut s,chat_id)?;
        if s.controller.mode()!=Mode::Paused{return Err("Observation is not paused".into());}
        let task_id=s.session_id.as_deref().ok_or("Missing observation task")?.to_owned();
        s.controller.resume(self.now()).map_err(format_error)?;
        if let Err(e)=self.task_store.resume(&task_id){
            let _=self.revoke(s);
            return Err(format!("Could not resume durable task: {e:?}"));
        }
        Ok(Self::view(s))
    }
    pub(crate) fn stop(&self,chat_id:&str)->Result<ObserveView,String>{
        let mut s=self.inner.lock().map_err(|_|"Observer lock unavailable")?;
        let s=Self::owned(&mut s,chat_id)?;
        self.revoke(s)?;
        Ok(Self::view(s))
    }
    /// Extra user-approved OS picker: chooses ONE window or monitor, not
    /// automatically within the Lu-only semantic observation app scope.
    /// No image bytes are returned to the model or WebView.
    pub(crate) fn launch_one_shot(
        &self,chat_id:&str,plan:TrustedWorkerPlan
    )->Result<String,String>{
        if !plan.task_id.starts_with("obs-frame-")
          ||plan.workspace_id!=chat_id
          ||plan.args.len()!=2
          ||plan.args[0]!="--lu-observe-one-shot"
          ||plan.args[1].to_string_lossy().len()!=32
          ||!plan.args[1].to_string_lossy().bytes().all(|b|b.is_ascii_hexdigit())
          ||!plan.progress_protocol
          ||plan.max_runtime_seconds!=Some(90){
            return Err("Invalid host-owned capture plan".into());
        }
        let mut session=self.inner.lock().map_err(|_|"Observer lock unavailable")?;
        self.refresh_expiry(&mut session)?;
        let s=Self::owned(&mut session,chat_id)?;
        if s.controller.mode()!=Mode::Active{
            return Err("Start Observe & Learn for this conversation first".into());
        }
        if s.capture_task_id.as_ref().is_some_and(|task|
            self.capture_processes.can_cancel(task)){
            return Err("An approved capture picker is already running".into());
        }
        let id=plan.task_id.clone();
        self.capture_processes.start_host_approved(plan)
            .map_err(|e|format!("Trusted capture worker could not start: {e:?}"))?;
        s.capture_task_id=Some(id.clone());
        Ok(id)
    }

    /// Only the trusted task session's enum-only annotations can be seen;
    /// no cross-conversation note retrieval or unapproved ANSF write.
    pub(crate) fn notes(
        &self,chat_id:&str
    )->Result<Vec<ObserveNoteView>,String>{
        let mut s=self.inner.lock().map_err(|_|"Observer lock unavailable")?;
        self.refresh_expiry(&mut s)?;
        let owner=Self::owned(&mut s,chat_id)?;
        let task=owner.session_id.as_deref().ok_or("Missing observe session")?;
        self.task_store.awareness_notes(task,chat_id,24)
            .map_err(|_|"Observation notations unavailable")?
            .into_iter().map(|n|Ok(ObserveNoteView{
                sequence:n.sequence,kind:n.code,source:n.source,
                priority:n.priority,created_at:n.created_at,
            })).collect()
    }
    /// Produces a content-minimized main-model review handoff. No model run,
    /// filesystem change, repair, screenshot or tool grant occurs here.
    pub(crate) fn handoff(
        &self,chat_id:&str
    )->Result<Option<ObserveHandoffView>,String>{
        let mut s=self.inner.lock().map_err(|_|"Observer lock unavailable")?;
        self.refresh_expiry(&mut s)?;
        let owner=Self::owned(&mut s,chat_id)?;
        let task=owner.session_id.as_deref().ok_or("Missing observe session")?;
        let h=owner.awareness.as_ref().ok_or("Awareness unavailable")?
            .handoff(task,chat_id)
            .map_err(|_|"Awareness review is not authorized")?;
        Ok(h.map(|h|ObserveHandoffView{
            session_id:h.observation_session_id,
            verified_failures:h.verified_failures,
            unverified_indicators:h.unverified_indicators,
            recommendation:h.recommended_action.to_owned(),
            requires_separate_tool_authorization:h.requires_separate_tool_authorization,
            main_model_may_treat_as_verified_failure:
                h.main_model_may_treat_as_verified_failure,
        }))
    }

    /// Renderer hints contain no text or pixels. They are NOT verified GUI
    /// outcomes, even when a label says "error". Trustworthy pass/fail must
    /// originate in an independent, native test verifier.
    pub(crate) fn self_signal(
        &self,chat_id:&str,kind:&str,model_ready_idle:bool
    )->Result<ObserveView,String>{
        let signal=match kind{
            "ui_changed"=>Signal::UiChanged,
            "error_indicator"=>Signal::ErrorIndicator,
            _=>return Err("Untrusted or unsupported observation event".into()),
        };
        let mut s=self.inner.lock().map_err(|_|"Observer lock unavailable")?;
        self.refresh_expiry(&mut s)?;
        let s=Self::owned(&mut s,chat_id)?;
        if s.controller.mode()!=Mode::Active{
            return Err("Observation is not active".into());
        }
        if !s.scope.as_ref().is_some_and(|g|
            g.accepts_untrusted_app_hint(APP_ID)){
            return Err("This source is not within the native observation scope".into());
        }
        let sequence=s.sequence.checked_add(1).ok_or("Observation sequence overflow")?;
        let decision=s.controller.ingest(NativeEvent{
            app_id:APP_ID.into(),sequence,signal,
            requests_redacted_snapshot:false,
        },self.now(),model_ready_idle).map_err(format_error)?;
        s.sequence=sequence;s.event_count=s.event_count.saturating_add(1);
        // Lu's background awareness is native-first and does not auto-run
        // a small/large model. Renderer hints are UNVERIFIED by definition.
        // An authorized tiny model can be routed through the same supervisor
        // once the hardware/backend and separate reasoning grant are verified.
        let session_id=s.session_id.as_deref()
            .ok_or("Observation session missing")?.to_owned();
        let awareness=s.awareness.as_mut().ok_or("Awareness lane missing")?;
        let observed=awareness.ingest(&session_id,chat_id,AwarenessSignal{
            sequence,source:AwarenessSource::RendererHint,
            signal:if kind=="error_indicator"{
                AwarenessKind::ErrorIndicator
            }else{AwarenessKind::UiChanged},
        },self.now(),HostReadiness::default())
            .map_err(|_|"Awareness observation rejected")?;
        if let Some(note)=observed.notation{
            self.task_store.append_awareness_note(&session_id,chat_id,&note)
                .map_err(|_|"Failed to record scoped awareness notation")?;
            s.notes_logged=s.notes_logged.saturating_add(1);
        }
        if matches!(decision,Decision::RecommendInference{..}){
            // A recommendation does not submit a model request; actual
            // inference must use existing interactive queue and host grants.
            s.recommendations=s.recommendations.saturating_add(1);
        }
        Ok(Self::view(s))
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use lu_core::tasks::TaskState;
    use std::path::Path;
    #[test]
    fn native_tasks_are_reused_and_observation_is_stopped_by_default(){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let processes=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
        let host=ObserveHost::new(Arc::clone(&store),processes);
        assert_eq!(host.status("c-main").unwrap().mode,"off");
        let initial=host.start("c-main","obs-one".into()).unwrap();
        assert_eq!(initial.mode,"active");
        assert!(!initial.can_see_desktop);
        assert_eq!(store.get("obs-one").unwrap().unwrap().state,TaskState::Running);
        assert!(host.self_signal("c-other","ui_changed",true).is_err());
        assert!(host.self_signal("c-main","test_passed",true).is_err());
        assert_eq!(host.self_signal("c-main","ui_changed",false)
            .unwrap().inference_performed,0);
        host.self_signal("c-main","error_indicator",false).unwrap();
        let notes=host.notes("c-main").unwrap();
        assert_eq!(notes.len(),1);
        assert_eq!(notes[0].kind,"error_indicator");
        assert_eq!(notes[0].source,"renderer_hint");
        assert_eq!(host.handoff("c-main").unwrap().unwrap().verified_failures,0);
        assert!(host.notes("c-other").is_err());
        assert_eq!(host.pause("c-main").unwrap().mode,"paused");
        assert_eq!(store.get("obs-one").unwrap().unwrap().state,TaskState::Paused);
        assert!(host.self_signal("c-main","ui_changed",true).is_err());
        assert_eq!(host.resume("c-main").unwrap().mode,"active");
        assert_eq!(host.stop("c-main").unwrap().mode,"off");
        assert_eq!(store.get("obs-one").unwrap().unwrap().state,TaskState::Cancelled);
        assert_eq!(host.status("c-main").unwrap().mode,"off");
        assert!(host.self_signal("c-main","ui_changed",true).is_err());
    }
    #[test]
    fn another_conversation_cannot_steal_session_and_no_restart_grant(){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let processes=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
        let host=ObserveHost::new(Arc::clone(&store),processes);
        host.start("c-owner","obs-scoped".into()).unwrap();
        assert!(host.pause("c-intruder").is_err());
        assert!(host.stop("c-intruder").is_err());
        // Switching chat immediately revokes observation rather than
        // disguising a still-running session as "off" in the new chat.
        assert_eq!(host.status("c-intruder").unwrap().mode,"off");
        assert_eq!(store.get("obs-scoped").unwrap().unwrap().state,TaskState::Cancelled);
        host.start("c-owner","obs-restart".into()).unwrap();
        drop(host);
        // The existing TaskStore startup recovery marks running records as
        // interrupted; a newly constructed host has no observation grant.
        assert_eq!(store.reconcile_interrupted().unwrap(),1);
        let after=ObserveHost::new(Arc::clone(&store),Arc::new(
            BackgroundProcessSupervisor::new(Arc::clone(&store))));
        assert_eq!(after.status("c-owner").unwrap().mode,"off");
        assert_eq!(store.get("obs-scoped").unwrap().unwrap().state,TaskState::Cancelled);
        assert_eq!(store.get("obs-restart").unwrap().unwrap().state,TaskState::Interrupted);
    }
}
