//! Content-free progress projection for supervised native turns. Nested model
//! generation IDs are host-only cancellation targets; a tool checkpoint ends
//! the previous model step, and is NOT a verified tool/effect outcome.
use lu_core::agent_tool_loop::ToolDispatchCheckpoint;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase { ModelStep, ToolStarted, ToolReturned }

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress { chat_id:String, generation_id:String, phase:Phase }

#[derive(Clone)]
pub(crate) struct NativeTurnProgress {
    app:AppHandle,
    chat_id:String,
    generation_id:String,
    active_step:Arc<Mutex<Option<String>>>,
}
impl NativeTurnProgress {
    pub(crate) fn new(app:&AppHandle, chat:&str, generation:&str,
        active_step:&Arc<Mutex<Option<String>>>) -> Self {
        Self { app:app.clone(), chat_id:chat.into(), generation_id:generation.into(),
            active_step:Arc::clone(active_step) }
    }
    fn emit(&self, phase:Phase) {
        let _=self.app.emit("lu-native-tool-progress",Progress {
            chat_id:self.chat_id.clone(), generation_id:self.generation_id.clone(), phase,
        });
    }
    pub(crate) fn model_observer(&self) -> Arc<dyn Fn(&str)+Send+Sync> {
        let owned=self.clone();
        Arc::new(move |step|{
            set_step(&owned.active_step,Some(step));
            owned.emit(Phase::ModelStep);
        })
    }
    pub(crate) fn dispatch_observer(&self) -> impl Fn(ToolDispatchCheckpoint)+Send+Sync+'static {
        let owned=self.clone();
        move |checkpoint| {
            if checkpoint==ToolDispatchCheckpoint::FeedbackAudited{return}
            let phase=dispatch_phase(&owned.active_step,checkpoint);
            owned.emit(phase);
        }
    }
}
fn set_step(active:&Mutex<Option<String>>, step:Option<&str>) {
    if let Ok(mut current)=active.lock(){*current=step.map(str::to_owned);}
}
fn dispatch_phase(active:&Mutex<Option<String>>, checkpoint:ToolDispatchCheckpoint)->Phase {
    // At dispatch entry the preceding synchronous generation has finished.
    // During native work Stop acts through task cancellation and worker custody,
    // not an old inference ID. Returned likewise is only a checkpoint.
    set_step(active,None);
    match checkpoint {
        ToolDispatchCheckpoint::Entered=>Phase::ToolStarted,
        ToolDispatchCheckpoint::Returned|ToolDispatchCheckpoint::FeedbackAudited=>Phase::ToolReturned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tool_dispatch_clears_stale_generation_and_preserves_existing_wire_phase() {
        let active=Mutex::new(None);
        set_step(&active,Some("host-private-step-1"));
        assert_eq!(active.lock().unwrap().as_deref(),Some("host-private-step-1"));
        let phase=dispatch_phase(&active,ToolDispatchCheckpoint::Entered);
        assert!(active.lock().unwrap().is_none());
        let payload=serde_json::to_value(Progress{chat_id:"c-original".into(),generation_id:"g-original".into(),phase}).unwrap();
        assert_eq!(payload,serde_json::json!({"chatId":"c-original","generationId":"g-original","phase":"tool_started"}));
        assert_eq!(serde_json::to_value(dispatch_phase(&active,ToolDispatchCheckpoint::Returned)).unwrap(),"tool_returned");
        set_step(&active,Some("host-private-step-2"));
        assert_eq!(active.lock().unwrap().as_deref(),Some("host-private-step-2"));
        assert_eq!(serde_json::to_value(Phase::ModelStep).unwrap(),"model_step");
    }
}
