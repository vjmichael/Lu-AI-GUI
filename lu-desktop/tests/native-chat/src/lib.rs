//! Directly compiles the production chat journal and runs its real native tests
//! without WebView, GTK, Tauri, or llama.cpp. The stub is the *trait only*:
//! origin verification logic and file operations run from production source.
extern crate self as lu_core;

pub mod conversation_authority {
    pub trait TrustedUserTurnVerifier: Send + Sync {
        fn verify_user_turn(&self, conversation_id: &str, user_turn_id: &str) -> bool;
    }
}

#[path = "../../../src-tauri/src/chat_store.rs"]
mod chat_store;

// Real M2 ANSF goal events and chat journal compile together without Tauri.
#[path = "../../../../src-tauri/lu-core/src/ansf_goals.rs"]
pub mod ansf_goals;
// ANSF core schema initializes the production artifact-reference table.
#[path = "../../../../src-tauri/lu-core/src/ansf_artifact_links.rs"]
pub mod ansf_artifact_links;
#[path = "../../../../src-tauri/lu-core/src/ansf_live_chat.rs"]
pub mod ansf_live_chat;
#[path = "../../../../src-tauri/lu-core/src/ansf_goal_records.rs"]
pub mod ansf_goal_records;
#[path = "../../../../src-tauri/lu-core/src/ansf_library.rs"]
pub mod ansf_library;
#[path = "../../../src-tauri/src/ansf_migration.rs"]
pub mod ansf_migration;

// The production ANSF SQLite module is compiled directly without a mock.
#[path = "../../../../src-tauri/lu-core/src/ansf.rs"]
pub mod ansf;


#[cfg(test)]
mod m2_native_memory_acceptance {
    use super::{ansf::AnsfStore,ansf_goals::{GoalEvent,GoalEventKind,project_goal_events},
        chat_store::ChatStore};
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    fn event(n:u8,k:GoalEventKind,q:Option<String>,text:&str,choice:Option<&str>)
        ->GoalEvent{
        GoalEvent{
            event_id:format!("m-{:032x}",n),
            goal_id:format!("goal-{:032x}",1),
            question_id:q,
            kind:k,text:text.into(),
            choice:choice.map(str::to_owned),comment:None,
        }
    }
    #[test]
    fn real_chat_journal_and_ansf_sqlite_survive_restart_without_memory_based_authority(){
        let tmp=std::env::temp_dir().join(format!("lu-m2-portable-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&tmp).unwrap();
        let chats=ChatStore::open(&tmp).unwrap();
        let a=chats.create_chat().unwrap();
        let b=chats.create_chat().unwrap();
        let records=[
            event(1,GoalEventKind::SetGoal,None,"Ship a stable alpha",None),
            event(2,GoalEventKind::AskQuestion,
                Some(format!("q-{:032x}",1)),"Does it require Windows?",None),
            event(3,GoalEventKind::RespondQuestion,
                Some(format!("q-{:032x}",1)),"",Some("not_now")),
        ];
        for e in &records {chats.append_goal_memory_event(&a.chat_id,e.clone()).unwrap();}
        let ansf=AnsfStore::open(&tmp.join("lu-ansf.sqlite")).unwrap();
        ansf.ensure_session(&a.chat_id,"lu-profile","Lu goals").unwrap();
        for e in chats.memory_events(&a.chat_id).unwrap(){
            ansf.mirror_goal_event(&a.chat_id,&e).unwrap();
        }
        assert_eq!(ansf.goal_events(&a.chat_id).unwrap(),records);
        assert_eq!(project_goal_events(&records).unwrap().questions[0].status,"deferred");
        assert!(chats.memory_events(&b.chat_id).unwrap().is_empty());
        assert!(chats.history(&a.chat_id).unwrap().is_empty());
        assert!(chats.trusted_user_turn_text(&a.chat_id,&format!("g-{:032x}",1)).is_none());
        drop(ansf);drop(chats);
        let chats=ChatStore::open(&tmp).unwrap();
        let ansf=AnsfStore::open(&tmp.join("lu-ansf.sqlite")).unwrap();
        for e in chats.memory_events(&a.chat_id).unwrap(){
            ansf.mirror_goal_event(&a.chat_id,&e).unwrap();
        }
        assert_eq!(ansf.goal_events(&a.chat_id).unwrap(),records);
        let yes=event(4,GoalEventKind::RespondQuestion,
            Some(format!("q-{:032x}",1)),"",Some("yes"));
        chats.append_goal_memory_event(&a.chat_id,yes.clone()).unwrap();
        ansf.mirror_goal_event(&a.chat_id,&yes).unwrap();
        assert_eq!(project_goal_events(&ansf.goal_events(&a.chat_id).unwrap())
            .unwrap().questions[0].status,"answered");
        assert!(chats.append_goal_memory_event(&a.chat_id,event(5,
            GoalEventKind::RespondQuestion,Some(format!("q-{:032x}",1)),"",Some("no"))).is_err());
        assert!(chats.memory_events(&b.chat_id).unwrap().is_empty());
        drop(ansf);drop(chats);
        fs::remove_dir_all(tmp).unwrap();
    }
}
