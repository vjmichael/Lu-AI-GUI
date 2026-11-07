//! Directly compiles the production chat journal and runs its real native tests
//! without WebView, GTK, Tauri, or llama.cpp. The stub is the *trait only*:
//! origin verification logic and file operations run from production source.
extern crate self as lu_core;

pub mod conversation_authority {
    pub trait TrustedUserTurnVerifier: Send + Sync {
        fn verify_user_turn(&self, conversation_id: &str, user_turn_id: &str) -> bool;
    }
}

#[path = "../../../src-tauri/src/ansf_chat_store.rs"]
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
    /// A real OS-process boundary, not only dropping/reopening an SQLite handle:
    /// the first child exits without Rust destructors after successful commits.
    /// The next independent child and the parent must see the same canonical
    /// ANSF history while historical imported user turns remain unauthorized.
    #[test]
    fn process_restart_migration_and_new_conversation_survive(){
        const TEST: &str =
            "m2_native_memory_acceptance::process_restart_migration_and_new_conversation_survive";
        let source_chat=format!("c-{:032x}",0x91u64);
        let old_turn=format!("g-{:032x}",0x92u64);
        let first_turn=format!("g-{:032x}",0x93u64);
        let next_turn=format!("g-{:032x}",0x94u64);
        if let Ok(stage)=std::env::var("LU_M2_PROCESS_RESTART_STAGE"){
            let root=std::path::PathBuf::from(
                std::env::var("LU_M2_PROCESS_RESTART_ROOT").expect("fixture root"));
            let store=ChatStore::open(&root).expect("open actual production ANSF store");
            let old=store.history(&source_chat).unwrap();
            assert_eq!(old[0].text,"Legacy user through migration");
            assert_eq!(old[1].text,"Legacy assistant through migration");
            assert!(store.trusted_user_turn_text(&source_chat,&old_turn).is_none());
            let (summaries,folders)=store.list().unwrap();
            assert_eq!(summaries.len(),1);
            assert_eq!(summaries[0].title,"Legacy project");
            assert!(summaries[0].favorite);
            assert_eq!(folders.len(),1);
            assert_eq!(folders[0].name,"Original folder");
            match stage.as_str(){
                "first"=>{
                    assert_eq!(old.len(),2);
                    store.append_user_with_turn_id(&source_chat,&first_turn,"Fresh Lu user turn").unwrap();
                    store.append_progress(&source_chat,&first_turn,"Incomplete reply").unwrap();
                    store.append_progress(&source_chat,&first_turn,"Durable completed reply").unwrap();
                    store.complete(&source_chat,&first_turn).unwrap();
                    assert_eq!(store.history(&source_chat).unwrap().len(),4);
                    // Simulate immediate process death after committed SQLite writes.
                    std::process::exit(0);
                }
                "second"=>{
                    assert_eq!(old.len(),4);
                    assert_eq!(old[3].text,"Durable completed reply");
                    assert!(!old[3].interrupted);
                    assert_eq!(store.trusted_user_turn_text(&source_chat,&first_turn).as_deref(),
                        Some("Fresh Lu user turn"));
                    assert_eq!(store.recent_model_history(&source_chat,4).unwrap().len(),4);
                    store.append_user_with_turn_id(&source_chat,&next_turn,"After cold restart").unwrap();
                }
                _=>panic!("unexpected M2 restart phase"),
            }
            return;
        }
        let root=std::env::temp_dir().join(format!("lu-m2-process-restart-{}-{}",
            std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(root.join("conversations")).unwrap();
        let folder=format!("f-{:032x}",0x95u64);
        let index=format!(
            r#"{{"version":1,"sequence":3,"folders":[{{"folderId":"{folder}","name":"Original folder"}}],"chats":[{{"chatId":"{source_chat}","title":"Legacy project","folderId":"{folder}","favorite":true,"updatedSeq":3}}]}}"#
        );
        let original=format!(
            "{{\\\"kind\\\":\\\"user\\\",\\\"text\\\":\\\"Legacy user through migration\\\",\\\"user_turn_id\\\":\\\"{old_turn}\\\"}}\\n\\
             {{\\\"kind\\\":\\\"assistant_final\\\",\\\"text\\\":\\\"Legacy assistant through migration\\\"}}\\n"
        );
        let legacy=root.join("conversations").join(format!("{source_chat}.jsonl"));
        fs::write(root.join("conversation-index.json"),&index).unwrap();
        fs::write(&legacy,&original).unwrap();
        for phase in ["first","second"]{
            let result=std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--exact").arg(TEST).arg("--nocapture")
                .env("LU_M2_PROCESS_RESTART_STAGE",phase)
                .env("LU_M2_PROCESS_RESTART_ROOT",&root)
                .status().expect("spawn independent native Lu storage process");
            assert!(result.success(),"M2 native process stage {phase} failed: {result}");
        }
        let final_store=ChatStore::open(&root).unwrap();
        let final_history=final_store.history(&source_chat).unwrap();
        assert_eq!(final_history.len(),5);
        assert_eq!(final_history[4].text,"After cold restart");
        assert!(final_store.trusted_user_turn_text(&source_chat,&old_turn).is_none());
        assert!(final_store.trusted_user_turn_text(&source_chat,&next_turn).is_some());
        assert_eq!(fs::read_to_string(&legacy).unwrap(),original);
        assert_eq!(fs::read_to_string(root.join("conversation-index.json")).unwrap(),index);
        assert!(root.join("ansf/books").join(format!("{source_chat}.sqlite")).is_file());
        drop(final_store);
        fs::remove_dir_all(root).unwrap();
    }

}
