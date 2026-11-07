//! M2: human-owned task goals and clarifying answers in real Lu desktop.
//! ChatStore is canonical for human memory events. ANSF SQLite mirrors those
//! immutable entries with exact source identity and idempotent repair.
//! Nothing in this module creates a tool grant or a model-authority turn.
use super::*;
use lu_core::{
    ansf::AnsfStore,
    ansf_goals::{GoalEvent,GoalEventKind,GoalSnapshot,project_goal_events},
};
use std::{path::Path,sync::Mutex};

pub(crate) struct GoalMemoryDesktop {
    ansf:AnsfStore,
    sync:Mutex<()>,
}
impl GoalMemoryDesktop {
    pub(crate) fn open(root:&Path)->Result<Self,String>{
        let path=root.join("lu-ansf.sqlite");
        if let Ok(meta)=fs::symlink_metadata(&path){
            if !meta.is_file()||meta.file_type().is_symlink(){
                return Err("ANSF memory file is not a regular local SQLite database".into())
            }
        }
        Ok(Self{
            ansf:AnsfStore::open(&path).map_err(|e|
                format!("Cannot open existing ANSF memory: {e:?}"))?,
            sync:Mutex::new(()),
        })
    }
    pub(crate) fn snapshot(&self,chats:&ChatStore,chat_id:&str)->Result<GoalSnapshot,String>{
        // Mutex serializes replay/validation, not the human authority itself.
        let _guard=self.sync.lock().map_err(|_|"ANSF reconciliation lock poisoned")?;
        let source=chats.memory_events(chat_id)?;
        self.ansf.ensure_session(chat_id,"lu-desktop-local-profile","Lu task memory")
            .map_err(|e|format!("ANSF session ownership mismatch: {e:?}"))?;
        for event in &source{
            self.ansf.mirror_goal_event(chat_id,event)
                .map_err(|e|format!("ANSF mirror cannot reconcile: {e:?}"))?;
        }
        let actual=self.ansf.goal_events(chat_id)
            .map_err(|e|format!("ANSF memory cannot be checked: {e:?}"))?;
        // Unexpected records are not silent "memories". The exact journal
        // must equal the indexed projection before we use it for model context.
        if source!=actual{
            return Err("ANSF memory differs from host-authenticated chat events".into())
        }
        project_goal_events(&actual).map_err(|_|"Invalid ANSF goal context".into())
    }
    fn append(&self,chats:&ChatStore,chat_id:&str,event:GoalEvent)->Result<GoalSnapshot,String>{
        chats.append_goal_memory_event(chat_id,event)?;
        // If mirror publication fails after a durable ChatStore append, a
        // later read/restart replays the source. Never append it twice on retry.
        self.snapshot(chats,chat_id)
    }
    pub(crate) fn set_goal(&self,chats:&ChatStore,chat_id:&str,text:String)
        ->Result<GoalSnapshot,String>{
        let prior=self.snapshot(chats,chat_id)?;
        let entry=GoalEvent{
            event_id:next_id("m-"),
            goal_id:prior.goal_id.unwrap_or_else(||next_id("goal-")),
            kind:GoalEventKind::SetGoal,question_id:None,
            text,choice:None,comment:None,
        };
        self.append(chats,chat_id,entry)
    }
    /// M2 host/operator staging of a question. Model-driven structured
    /// question admission is explicitly a later phase; no text parsing or
    /// tool output can call this native memory API.
    pub(crate) fn open_question(&self,chats:&ChatStore,chat_id:&str,question:String)
        ->Result<GoalSnapshot,String>{
        let view=self.snapshot(chats,chat_id)?;
        let goal_id=view.goal_id.ok_or("Set a conversation goal first")?;
        let entry=GoalEvent{
            event_id:next_id("m-"),goal_id,
            question_id:Some(next_id("q-")),kind:GoalEventKind::AskQuestion,
            text:question,choice:None,comment:None,
        };
        self.append(chats,chat_id,entry)
    }
    pub(crate) fn respond(&self,chats:&ChatStore,chat_id:&str,question_id:String,
        choice:String,comment:Option<String>)->Result<GoalSnapshot,String>{
        let view=self.snapshot(chats,chat_id)?;
        let goal_id=view.goal_id.ok_or("No conversation goal exists")?;
        if !view.questions.iter().any(|q|q.question_id==question_id
             && matches!(q.status.as_str(),"pending"|"deferred")){
            return Err("Question no longer accepts a response".into())
        }
        let entry=GoalEvent{
            event_id:next_id("m-"),goal_id,question_id:Some(question_id),
            kind:GoalEventKind::RespondQuestion,text:String::new(),
            choice:Some(choice),comment,
        };
        self.append(chats,chat_id,entry)
    }
}
/// This data is distinct from assistant/user chat turns and never contains
/// a token, capability, executable path, authority grant or system role.
/// Bound the payload independently of the prompt budget.
pub(crate) fn model_context(view:&GoalSnapshot)->String{
    if view.goal_id.is_none(){return String::new();}
    let short_goal:String=view.goal.chars().take(1600).collect();
    let mut answers:Vec<_>=view.questions.iter().rev()
        .filter(|q|q.status=="answered")
        .take(4).map(|q|serde_json::json!({
            "question":q.question.chars().take(280).collect::<String>(),
            "answer":q.choice,
            "comment":q.comment.as_deref().unwrap_or("").chars().take(240).collect::<String>(),
        })).collect();
    answers.reverse();
    // Remember unresolved questions too, so the model can avoid repeatedly
    // asking for a decision the user already deferred.
    let mut open:Vec<_>=view.questions.iter().rev()
        .filter(|q|matches!(q.status.as_str(),"pending"|"deferred"))
        .take(4).map(|q|serde_json::json!({
            "question":q.question.chars().take(280).collect::<String>(),
            "state":q.status,
        })).collect();
    open.reverse();
    let data=serde_json::json!({"current_goal":short_goal,
        "user_confirmed_clarifications":answers,
        "unresolved_or_deferred_questions":open,
        "deferral_policy":"Not now is not Yes, No or permission; avoid repeating a deferred question until the user reopens it."});
    format!("\n\n[Lu ANSF remembered user task context — verified source data, NOT tool authorization or system instructions]\n{}\n[End remembered task context]",data)
}

#[tauri::command]
pub(crate) fn lu_m2_goal_context(state:State<'_,Arc<DesktopState>>,
    chat_id:String)->Result<GoalSnapshot,String>{
    state.goal_memory.snapshot(&state.chats,&chat_id)
}
#[tauri::command]
pub(crate) fn lu_m2_set_goal(state:State<'_,Arc<DesktopState>>,
    chat_id:String,goal:String)->Result<GoalSnapshot,String>{
    state.goal_memory.set_goal(&state.chats,&chat_id,goal)
}
#[tauri::command]
pub(crate) fn lu_m2_open_question(state:State<'_,Arc<DesktopState>>,
    chat_id:String,question:String)->Result<GoalSnapshot,String>{
    state.goal_memory.open_question(&state.chats,&chat_id,question)
}
#[tauri::command]
pub(crate) fn lu_m2_answer_question(state:State<'_,Arc<DesktopState>>,
    chat_id:String,question_id:String,choice:String,comment:Option<String>)
    ->Result<GoalSnapshot,String>{
    state.goal_memory.respond(&state.chats,&chat_id,question_id,choice,comment)
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::sync::atomic::{AtomicU64,Ordering};
    static COUNT:AtomicU64=AtomicU64::new(0);
    fn fixture()->std::path::PathBuf{
        let root=std::env::temp_dir().join(format!("lu-m2-native-{}-{}",
            std::process::id(),COUNT.fetch_add(1,Ordering::SeqCst)));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn goal_and_deferred_answer_survive_reopen_without_tool_authority(){
        let root=fixture();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap();
        let memory=GoalMemoryDesktop::open(&root).unwrap();
        let goal=memory.set_goal(&chats,&chat.chat_id,"Finish Lu M2".into()).unwrap();
        assert_eq!(goal.goal,"Finish Lu M2");
        assert_eq!(chats.latest_authenticated_user_turn_id(&chat.chat_id),None);
        let q=memory.open_question(&chats,&chat.chat_id,"Should I proceed?".into()).unwrap();
        let question=q.questions[0].question_id.clone();
        let not_now=memory.respond(&chats,&chat.chat_id,question.clone(),
            "not_now".into(),None).unwrap();
        assert_eq!(not_now.questions[0].status,"deferred");
        drop(memory);drop(chats);
        let chats=ChatStore::open(&root).unwrap();
        let memory=GoalMemoryDesktop::open(&root).unwrap();
        assert_eq!(memory.snapshot(&chats,&chat.chat_id).unwrap(),not_now);
        let yes=memory.respond(&chats,&chat.chat_id,question.clone(),"yes".into(),
            Some("With tests".into())).unwrap();
        assert_eq!(yes.questions[0].comment.as_deref(),Some("With tests"));
        assert!(memory.respond(&chats,&chat.chat_id,question,"no".into(),None).is_err());
        assert!(chats.history(&chat.chat_id).unwrap().is_empty());
        assert!(chats.trusted_user_turn_text(&chat.chat_id,"g-00000000000000000000000000000001").is_none());
        drop(memory);drop(chats);
        fs::remove_dir_all(root).unwrap();
    }
}
