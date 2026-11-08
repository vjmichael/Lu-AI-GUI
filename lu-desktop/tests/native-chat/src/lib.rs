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

// Production stable source IDs and the host authorization contract are real.
#[path = "../../../../src-tauri/lu-core/src/ansf_source_ids.rs"]
pub mod ansf_source_ids;

// M3-S1/S2: compile the real semantic projection and hybrid ranker.
#[path = "../../../../src-tauri/lu-core/src/ansf_semantic.rs"]
pub mod ansf_semantic;
#[path = "../../../../src-tauri/lu-core/src/ansf_revisions.rs"]
pub mod ansf_revisions;
#[path = "../../../../src-tauri/lu-core/src/ansf_belief_revision.rs"]
pub mod ansf_belief_revision;
#[path = "../../../../src-tauri/lu-core/src/ansf_embedding_worker.rs"]
pub mod ansf_embedding_worker;

#[path = "../../../../src-tauri/lu-core/src/contracts.rs"]
pub mod contracts;
#[path = "../../../../src-tauri/lu-core/src/journal.rs"]
pub mod journal;

#[path = "../../../../src-tauri/lu-core/src/continuity.rs"]
pub mod continuity;
#[path = "../../../../src-tauri/lu-core/src/agent_plan_ledger.rs"]
pub mod agent_plan_ledger;
#[path = "../../../../src-tauri/lu-core/src/agent_plan_revisions.rs"]
pub mod agent_plan_revisions;

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
            r#"{{"kind":"user","text":"Legacy user through migration","user_turn_id":"{old_turn}"}}
{{"kind":"assistant_final","text":"Legacy assistant through migration"}}
"#
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


#[cfg(test)]
mod m3_cross_chat_acceptance{
    use super::chat_store::ChatStore;
    use super::ansf_source_ids::AnsfSourceId;
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};

    fn fixture()->std::path::PathBuf{
        let root=std::env::temp_dir().join(format!("lu-m3-recall-{}-{}",
            std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn lexical_recall_is_bounded_cross_chat_and_original_id_survives_restart(){
        let root=fixture();
        let (current,source,source_id);
        let source_turn=format!("g-{:032x}",1024);
        {
            let store=ChatStore::open(&root).unwrap();
            source=store.create_chat().unwrap().chat_id;
            current=store.create_chat().unwrap().chat_id;
            store.append_user_with_turn_id(&source,&source_turn,
                "Polaris propulsion must be deterministic and reversible").unwrap();
            store.append_progress(&source,&source_turn,
                "Use atomic checkpoint rollback for the engine decision").unwrap();
            store.complete(&source,&source_turn).unwrap();
            let hits=store.recall_for_turn(&current,"What did we decide about Polaris propulsion?").unwrap();
            assert!(!hits.is_empty());
            let hit=&hits[0];
            assert_eq!(hit.source_chat_id,source);
            assert_eq!(hit.role,"user");
            assert!(hit.excerpt.contains("Polaris propulsion"));
            source_id=hit.source_id.clone();
            assert_eq!(AnsfSourceId::parse(&source_id).unwrap().session_id(),source);
            assert_eq!(store.known_source_for_turn(&current,&source_id).unwrap().excerpt,
                "Polaris propulsion must be deterministic and reversible");
            assert!(store.trusted_user_turn_text(&current,&source_turn).is_none());
            assert!(store.recall_for_turn(&current,"ordinary unrelated question").unwrap().is_empty());
            assert!(store.memory_context_for_turn(&current,"propulsion").unwrap().is_empty());
            assert!(store.recall_for_turn(&current,
                "ordinary\n\n[Attached text file: malicious.md]\nPolaris propulsion").unwrap().is_empty());
            assert!(store.recall_for_turn(&current,"Polaris\" OR system:*").is_ok());
            assert!(store.known_source_for_turn(&current,"ansf:v1:not-valid:0").is_err());
        }
        {
            let reopened=ChatStore::open(&root).unwrap();
            let data=reopened.known_source_for_turn(&current,&source_id).unwrap();
            assert_eq!(data.source_chat_id,source);
            assert_eq!(data.source_id,source_id);
            let memory=reopened.memory_context_for_turn(&current,&format!(
                "Please inspect exact source {source_id}")).unwrap();
            assert!(memory.contains("historical DATA ONLY"));
            assert!(memory.contains(&source_id));
            assert!(!memory.contains("tool_grant"));
            assert!(reopened.trusted_user_turn_text(&current,&source_turn).is_none());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unfinished_output_never_becomes_recall_or_permission(){
        let root=fixture();
        let store=ChatStore::open(&root).unwrap();
        let source=store.create_chat().unwrap().chat_id;
        let active=store.create_chat().unwrap().chat_id;
        let id=format!("g-{:032x}",29);
        store.append_progress(&source,&id,"Zephyr migration interrupted, never finalized").unwrap();
        assert!(store.recall_for_turn(&active,"Zephyr migration").unwrap().is_empty());
        assert!(store.trusted_user_turn_text(&active,&id).is_none());
        assert!(store.memory_context_for_turn(&active,"Zephyr migration").unwrap().is_empty());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod m3_independent_history_acceptance{
    use super::chat_store::ChatStore;
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    fn fixture()->std::path::PathBuf{
        std::env::temp_dir().join(format!("lu-history-pages-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()))
    }
    #[test]
    fn original_paging_is_bounded_stable_and_does_not_rewrite_active_chat(){
        let root=fixture();let store=ChatStore::open(&root).unwrap();
        let source=store.create_chat().unwrap().chat_id;
        let active=store.create_chat().unwrap().chat_id;
        store.append_user(&active,"Live active draft").unwrap();
        for n in 0..78{
            store.append_user(&source,&format!("Historical entry {n:03}")).unwrap();
        }
        let first=store.original_history_page(&active,&source,None,24).unwrap();
        assert_eq!(first.entries.len(),24);
        assert!(first.has_more);
        assert_eq!(first.entries.first().unwrap().excerpt,"Historical entry 054");
        let second=store.original_history_page(&active,&source,first.next_before_id,24).unwrap();
        assert_eq!(second.entries.len(),24);
        assert_eq!(second.entries.first().unwrap().excerpt,"Historical entry 030");
        let third=store.original_history_page(&active,&source,second.next_before_id,40).unwrap();
        assert_eq!(third.entries.len(),30);
        assert!(!third.has_more);
        assert!(third.next_before_id.is_none());
        assert_eq!(third.entries.first().unwrap().excerpt,"Historical entry 000");
        assert!(store.original_history_page(&active,&source,None,41).is_err());
        assert!(store.original_history_page("bad",&source,None,24).is_err());
        assert_eq!(store.history(&active).unwrap().len(),1);
        assert!(store.trusted_user_turn_text(&active,"g-00000000000000000000000000000001").is_none());
        drop(store);
        let reopened=ChatStore::open(&root).unwrap();
        assert_eq!(reopened.original_history_page(&active,&source,None,24).unwrap().entries,
            first.entries);
        drop(reopened);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn active_window_persists_immutable_stream_projection_across_reopen(){
        let root=fixture();let chat;
        {
            let store=ChatStore::open(&root).unwrap();
            chat=store.create_chat().unwrap().chat_id;
            for n in 0..115{
                store.append_user(&chat,&format!("Visible message {n:03}")).unwrap();
            }
            let gen=format!("g-{:032x}",99);
            store.append_progress(&chat,&gen,"Started").unwrap();
            store.append_progress(&chat,&gen,"Final checkpoint").unwrap();
            store.complete(&chat,&gen).unwrap();
            let latest=store.history_window(&chat,None,"older",64).unwrap();
            assert_eq!(latest.entries.len(),64);
            assert!(latest.has_more);
            assert_eq!(latest.entries.last().unwrap().text,"Final checkpoint");
            assert!(!latest.entries.last().unwrap().interrupted);
            let old=store.history_window(&chat,latest.next_cursor,"older",64).unwrap();
            assert_eq!(old.entries.len(),52);
            assert!(!old.has_more);
            assert!(old.entries.last().unwrap().seq<latest.entries[0].seq);
            let forward=store.history_window(&chat,
                Some(old.entries.last().unwrap().seq),"newer",64).unwrap();
            assert_eq!(forward.entries.len(),64);
            assert_eq!(forward.entries.last().unwrap().text,"Final checkpoint");
            assert!(store.history_window(&chat,None,"newer",8).is_err());
            assert!(store.history_window(&chat,None,"older",65).is_err());
        }
        let store=ChatStore::open(&root).unwrap();
        let latest=store.history_window(&chat,None,"older",64).unwrap();
        assert_eq!(latest.entries.last().unwrap().text,"Final checkpoint");
        drop(store);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn history_hot_scroll_both_directions_across_many_original_pages(){
        let root=fixture();
        let store=ChatStore::open(&root).unwrap();
        let active=store.create_chat().unwrap().chat_id;
        let other=store.create_chat().unwrap().chat_id;
        for n in 0..520{
            let size=if n%17==0{3000}else{47};
            store.append_user(&other,&format!("{:04}:{}",n,"x".repeat(size))).unwrap();
        }
        let mut backwards=Vec::new();
        let mut cursor=None;
        loop{
            let page=store.original_history_page(&active,&other,cursor,40).unwrap();
            assert!(page.entries.len()<=40);
            for row in page.entries.iter().rev(){
                assert!(!backwards.contains(&row.source_id));
                backwards.push(row.source_id.clone());
            }
            cursor=page.next_before_id;
            if !page.has_more{break}
        }
        assert_eq!(backwards.len(),520);
        let mut forwards=Vec::new();
        let mut after=0i64;
        loop{
            let page=store.original_history_after(&active,&other,after,40).unwrap();
            assert!(page.entries.len()<=40);
            for row in &page.entries{
                assert!(!forwards.contains(&row.source_id));
                forwards.push(row.source_id.clone());
            }
            if !page.has_more{break}
            after=page.next_after_id.unwrap();
        }
        assert_eq!(forwards.len(),520);
        assert_eq!(forwards,backwards.into_iter().rev().collect::<Vec<_>>());
        assert_eq!(store.history(&active).unwrap().len(),0);
        assert!(store.original_history_after("not_a_chat",&other,0,20).is_err());
        assert!(store.original_history_after(&active,&other,-1,20).is_err());
        drop(store);
        let reopened=ChatStore::open(&root).unwrap();
        assert_eq!(reopened.original_history_after(&active,&other,0,40).unwrap().entries.len(),40);
        drop(reopened);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn months_scale_active_history_paging_is_bounded_after_restart(){
        let root=fixture();
        let chat;
        {
            let store=ChatStore::open(&root).unwrap();
            chat=store.create_chat().unwrap().chat_id;
            // 2,048 durable messages, enough to cross 32 native 64-row
            // windows without asking the model or WebView to load them all.
            for i in 0..2048{
                let text=if i%71==0{
                    format!("Long unicode 🪐 history {i:05}:{}", "x".repeat(4000))
                }else{format!("Durable local event {i:05}")};
                store.append_user(&chat,&text).unwrap();
            }
            let mut cursor=None;
            let mut seen=std::collections::HashSet::new();
            let mut previous=i64::MAX;
            let mut pages=0;
            loop{
                let page=store.history_window(&chat,cursor,"older",64).unwrap();
                assert!(!page.entries.is_empty());
                assert!(page.entries.len()<=64);
                for entry in page.entries.iter().rev(){
                    assert!(entry.seq<previous);
                    assert!(seen.insert(entry.seq));
                    previous=entry.seq;
                }
                pages+=1;
                if !page.has_more{break}
                cursor=page.next_cursor;
                assert!(cursor.is_some());
            }
            assert_eq!(pages,32);
            assert_eq!(seen.len(),2048);
        }
        let store=ChatStore::open(&root).unwrap();
        let latest=store.history_window(&chat,None,"older",64).unwrap();
        assert_eq!(latest.entries.len(),64);
        assert_eq!(latest.entries.last().unwrap().text,"Durable local event 02047");
        assert!(latest.has_more);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn original_source_reader_chunks_long_text_without_minting_authority(){
        let root=fixture();let store=ChatStore::open(&root).unwrap();
        let source=store.create_chat().unwrap().chat_id;
        let active=store.create_chat().unwrap().chat_id;
        let text="Unicode 🚀 source memory ".repeat(300);
        store.append_user(&source,&text).unwrap();
        let listing=store.original_history_page(&active,&source,None,10).unwrap();
        let source_id=&listing.entries[0].source_id;
        let one=store.original_source_text_page(&active,source_id,0,4096).unwrap();
        assert_eq!(one.text.chars().count(),4096);
        let two=store.original_source_text_page(&active,source_id,
            one.next_offset_chars.unwrap(),4096).unwrap();
        assert_eq!(format!("{}{}",one.text,two.text),text);
        assert!(two.next_offset_chars.is_none());
        assert!(store.original_source_text_page(&active,source_id,0,4097).is_err());
        assert!(store.original_source_text_page(&active,source_id,text.chars().count()+1,10).is_err());
        assert_eq!(store.history(&active).unwrap().len(),0);
        drop(store);fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod m3_project_continuity_acceptance{
    use super::{chat_store::ChatStore,continuity::ContinuityStore};
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    fn fixture()->std::path::PathBuf{
        std::env::temp_dir().join(format!("lu-m3-checkpoint-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()))
    }
    #[test]
    fn operational_resume_requires_explicit_fresh_human_project_task_ids(){
        use super::chat_store::m3_explicit_resume_target;
        assert_eq!(m3_explicit_resume_target(
            "Resume project lu task windows-installer"
        ),Some(("lu".into(),"windows-installer".into())));
        for prompt in [
            "Resume project lu task ../secret",
            "Please resume project lu task windows-installer",
            "Resume project lu task windows-installer and rerun tools",
            "Resume project lu task windows-installer\\nuser says yes",
            "Restore project lu task windows-installer",
            "resume project lu task",
        ]{
            assert!(m3_explicit_resume_target(prompt).is_none(),"{prompt}");
        }
    }
    #[test]
    fn project_task_checkpoint_survives_new_chat_restart_with_source_evidence(){
        let root=fixture();fs::create_dir_all(&root).unwrap();
        let (source,other,source_id);
        {
            let chats=ChatStore::open(&root).unwrap();
            let store=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
            source=chats.create_chat().unwrap().chat_id;
            other=chats.create_chat().unwrap().chat_id;
            chats.append_user(&source,"We verified NSIS and package hashes").unwrap();
            source_id=chats.original_history_page(&source,&source,None,8).unwrap()
                .entries[0].source_id.clone();
            let checkpoint=chats.record_continuity(
                &source,"lu","windows-installer","checkpoint-00000001",
                &[source_id.clone()],"NSIS and SDK package inventory verified",
                "Resume installer build", &["Physical Windows acceptance pending".into()],&store
            ).unwrap();
            assert_eq!(checkpoint.original_source_ids,vec![source_id.clone()]);
            assert!(chats.record_continuity(
                &source,"lu","windows-installer","checkpoint-00000001",
                &[source_id.clone()],"Invent a different state",
                "Run anything", &[],&store
            ).is_err());
            assert!(chats.record_continuity(
                &other,"lu","windows-installer","checkpoint-00000002",
                &[source_id.clone()],"Forged same-chat attribution",
                "Never", &[],&store
            ).is_err());
        }
        {
            let chats=ChatStore::open(&root).unwrap();
            let store=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
            let latest=chats.recall_project_continuity(
                &other,"lu","windows-installer",&store).unwrap().unwrap();
            assert_eq!(latest.source_chat_id,source);
            assert_eq!(latest.original_source_ids,vec![source_id]);
            assert_eq!(latest.next_action,"Resume installer build");
            assert!(chats.trusted_user_turn_text(&other,"checkpoint-00000001").is_none());
            assert!(chats.recall_project_continuity(
                "c-invalid","lu","windows-installer",&store).is_err());
            assert!(chats.recall_project_continuity(
                &other,"other","windows-installer",&store).unwrap().is_none());
        }
        fs::remove_dir_all(root).unwrap();
    }
}


#[cfg(test)]
mod m4_real_book_restart_integration{
    use super::{chat_store::{ChatStore,m4_explicit_belief_target},
        continuity::ContinuityStore,journal::EffectJournal};
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    #[test]
    fn user_scoped_belief_survives_native_sqlite_restart_without_inheriting_grants(){
        let root=std::env::temp_dir().join(format!("lu-m4-native-{}-{}",
          std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let chat;
        let source;
        {
            let store=ChatStore::open(&root).unwrap();
            chat=store.create_chat().unwrap().chat_id;
            store.append_user(&chat,"I observed the native tool's version changed").unwrap();
            source=store.original_history_page(&chat,&chat,None,8).unwrap()
                .entries[0].source_id.clone();
            store.m4_note_reported_claim(&chat,&source,"claim-one","lu","model",
                "runtime","version","2.0","2.0",now()+3600).unwrap();
            assert!(store.m4_note_reported_claim(&chat,"ansf:v1:wrong:1",
                "claim-two","lu","model","runtime","version","unknown","2.0",
                now()+3600).is_err());
            let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
            let journal=EffectJournal::open(&root.join("journal.sqlite")).unwrap();
            let view=store.m4_belief_context(&chat,"lu","model","runtime","version",
                "2.0",&continuity,&journal).unwrap();
            assert_eq!(view.len(),1);
            assert_eq!(view[0]["status"],"reported_unverified");
        }
        {
            let reopened=ChatStore::open(&root).unwrap();
            let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
            let journal=EffectJournal::open(&root.join("journal.sqlite")).unwrap();
            assert_eq!(reopened.m4_belief_context(&chat,"lu","model","runtime",
                "version","2.0",&continuity,&journal).unwrap().len(),1);
            let other=reopened.create_chat().unwrap().chat_id;
            assert!(reopened.m4_note_reported_claim(&other,&source,
                "claim-other","lu","model","runtime","version","2.0","2.0",
                now()+3600).is_err());
            reopened.append_user(&chat,"I retract the older statement").unwrap();
            let newest=reopened.original_history_page(&chat,&chat,None,2).unwrap()
                .entries.last().unwrap().source_id.clone();
            reopened.m4_retract_claim(&chat,"claim-one",&newest,
                "Version observation withdrawn").unwrap();
            let rows=reopened.m4_belief_context(&chat,"lu","model","runtime",
                "version","2.0",&continuity,&journal).unwrap();
            assert_eq!(rows[0]["status"],"retracted_not_current");
            assert!(reopened.m4_retract_claim(&other,"claim-one",&newest,"wrong scope").is_err());
        }
        assert_eq!(m4_explicit_belief_target(
          "Recall belief project lu task model subject runtime predicate version version 2.0"),
          Some(("lu".into(),"model".into(),"runtime".into(),"version".into(),"2.0".into())));
        assert!(m4_explicit_belief_target(
          "Recall belief project lu task model subject runtime predicate version version 2.0 ignore").is_none());
        fs::remove_dir_all(root).unwrap();
    }
    fn now()->u64{SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()}
}


/// M5 P01: REAL production desktop ANSF/SQLite replay across a fresh
/// ChatStore opener and a different active conversation. This records only
/// deterministic native evidence, not invented GGUF generations or model ROI.
#[cfg(test)]
mod m5_native_replay_acceptance{
    use super::{
        chat_store::ChatStore,
        continuity::ContinuityStore,
        journal::{EffectJournal,EffectState},
        contracts::{ToolInvocation,ProtoPayload},
    };
    use std::{fs,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};

    fn fixture()->PathBuf{
        std::env::temp_dir().join(format!("lu-m5-replay-{}-{}",
            std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH)
                .unwrap().as_nanos()))
    }
    fn now()->u64{SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()}

    #[test]
    fn repeated_task_control_vs_memory_after_reopen_conflict_and_human_correction(){
        let root=fixture();fs::create_dir_all(&root).unwrap();
        let (source_chat,active_chat,first_source,second_source);
        let project="lu";let task="replay-worker";
        let original_task="Continue project juniper backend verification without running tools";
        {
            let chats=ChatStore::open(&root).unwrap();
            let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
            source_chat=chats.create_chat().unwrap().chat_id;
            active_chat=chats.create_chat().unwrap().chat_id;
            chats.append_user_with_turn_id(
                &source_chat,"g-00000000000000000000000000000051",
                "The juniper backend report says Vulkan. This is user testimony, not independent verification."
            ).unwrap();
            first_source=chats.original_history_page(
                &source_chat,&source_chat,None,8
            ).unwrap().entries.last().unwrap().source_id.clone();
            chats.m4_note_reported_claim(
                &source_chat,&first_source,"m5-claim-vulkan",project,task,
                "juniper","backend","Vulkan","2.0",now()+3600
            ).unwrap();
            chats.append_user_with_turn_id(
                &source_chat,"g-00000000000000000000000000000052",
                "Another juniper backend report says ROCm; the earlier claim may be obsolete."
            ).unwrap();
            second_source=chats.original_history_page(
                &source_chat,&source_chat,None,8
            ).unwrap().entries.last().unwrap().source_id.clone();
            assert_ne!(first_source,second_source);
            chats.m4_note_reported_claim(
                &source_chat,&second_source,"m5-claim-rocm",project,task,
                "juniper","backend","ROCm","2.0",now()+3600
            ).unwrap();
            chats.record_continuity(
                &source_chat,project,task,"m5-cp-one",
                &[first_source.clone(),second_source.clone()],
                "Two competing unverified engine reports; no native evidence of a winner",
                "Ask for fresh host GPU/backend validation before executing",
                &["Conflicting version evidence".into()],&continuity
            ).unwrap();
            chats.append_user_with_turn_id(
                &source_chat,"g-00000000000000000000000000000053",
                "Historical authorizations cannot be reused in a new chat."
            ).unwrap();
            // The old turn identity belongs ONLY to the source conversation.
            assert!(chats.trusted_user_turn_text(
                &active_chat,"g-00000000000000000000000000000053"
            ).is_none());
            // Simulate a cold stop of the desktop profile and SQLite handles.
        }
        {
            let chats=ChatStore::open(&root).unwrap();
            let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
            let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
            chats.append_user(&active_chat,original_task).unwrap();

            // Control arm: the normal active chat history has no cross-chat
            // original text; it is intentionally NOT passed historical context.
            let control=chats.recent_model_history(&active_chat,16).unwrap();
            assert!(control.iter().any(|m|m.text==original_task));
            assert!(control.iter().all(|m|
                !m.text.contains("Vulkan")&&!m.text.contains("ROCm")
            ));

            // Memory arm: actual ANSF profile ownership, source-id reopens,
            // immutable checkpoints, and derived claim assessment are used.
            let checkpoint=chats.recall_project_continuity(
                &active_chat,project,task,&continuity
            ).unwrap().unwrap();
            assert_eq!(checkpoint.source_chat_id,source_chat);
            assert_eq!(checkpoint.original_source_ids.len(),2);
            assert_eq!(checkpoint.blockers,vec!["Conflicting version evidence"]);
            assert!(checkpoint.next_action.contains("fresh host GPU/backend validation"));
            for id in &checkpoint.original_source_ids{
                let hit=chats.known_source_for_turn(&active_chat,id).unwrap();
                assert_eq!(&hit.source_id,id);
                assert_eq!(hit.source_chat_id,source_chat);
                assert!(!hit.truncated);
            }
            let rows=chats.m4_belief_context(
                &source_chat,project,task,"juniper","backend","2.0",
                &continuity,&journal
            ).unwrap();
            assert_eq!(rows.len(),2);
            assert!(rows.iter().all(|r|
                r["status"]=="contradicted_human_review_required"&&
                r["notToolAuthorization"]==true
            ));
            // An old grant does not become a current turn.
            assert!(chats.trusted_user_turn_text(
                &active_chat,"g-00000000000000000000000000000053"
            ).is_none());
            assert!(chats.known_source_for_turn(
                &active_chat,"ansf:v1:invalid:100"
            ).is_err());

            // A genuine later human original is required to retract one
            // claim. Historical originals stay readable for audit.
            chats.append_user(&source_chat,
                "I withdraw the ROCm report; verify Vulkan independently."
            ).unwrap();
            let correction=chats.original_history_page(
                &source_chat,&source_chat,None,3
            ).unwrap().entries.last().unwrap().source_id.clone();
            assert!(chats.m4_retract_claim(
                &source_chat,"m5-claim-rocm",&first_source,"Earlier text is not a correction"
            ).is_err());
            chats.m4_retract_claim(
                &source_chat,"m5-claim-rocm",&correction,
                "Human-sourced withdrawal of conflicting report"
            ).unwrap();
            let updated=chats.m4_belief_context(
                &source_chat,project,task,"juniper","backend","2.0",
                &continuity,&journal
            ).unwrap();
            assert_eq!(updated.iter().filter(|r|
                r["status"]=="reported_unverified"
            ).count(),1);
            assert_eq!(updated.iter().filter(|r|
                r["status"]=="retracted_not_current"
            ).count(),1);
            let version_changed=chats.m4_belief_context(
                &source_chat,project,task,"juniper","backend","3.0",
                &continuity,&journal
            ).unwrap();
            assert_eq!(version_changed.iter().filter(|r|
                r["status"]=="stale_not_current"
            ).count(),1);
            assert!(version_changed.iter().all(|r|
                r["status"]!="independently_verified"
            ));
            assert_eq!(chats.known_source_for_turn(
                &active_chat,&second_source).unwrap().source_id,second_source);
        }
        {
            // Second genuine SQLite reopen: correction remains durable.
            let chats=ChatStore::open(&root).unwrap();
            let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
            let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
            let view=chats.m4_belief_context(
                &source_chat,project,task,"juniper","backend","2.0",
                &continuity,&journal
            ).unwrap();
            assert_eq!(view.iter().filter(|r|
                r["status"]=="retracted_not_current"
            ).count(),1);
            assert!(chats.trusted_user_turn_text(
                &active_chat,"g-00000000000000000000000000000053"
            ).is_none());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_effect_reconciles_on_restart_and_never_becomes_completed_work(){
        let root=fixture();fs::create_dir_all(&root).unwrap();
        let (source,original);
        {
            let chats=ChatStore::open(&root).unwrap();
            let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
            source=chats.create_chat().unwrap().chat_id;
            chats.append_user(&source,
                "Verify the juniper package only with an independently checked native receipt"
            ).unwrap();
            original=chats.original_history_page(
                &source,&source,None,5
            ).unwrap().entries.last().unwrap().source_id.clone();
            chats.record_continuity(
                &source,"lu","m5-interrupt","m5-cp-interrupted",
                &[original.clone()],"Worker launched but did not finish",
                "Reconcile status; never replay prior native effects automatically",
                &["Interrupted worker".into()],&continuity
            ).unwrap();
            let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
            journal.prepare(&ToolInvocation{
                call_id:"m5-call-interrupted".into(),tool_id:"lu.test.worker".into(),
                workspace_id:Some("lu-desktop-local-profile".into()),
                input:ProtoPayload{
                    type_url:"lu.test.Worker".into(),bytes:vec![7,5]
                }
            }).unwrap();
            journal.mark_executing("m5-call-interrupted").unwrap();
            // Real journal left in EXECUTING when this opener is dropped.
        }
        {
            let chats=ChatStore::open(&root).unwrap();
            let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
            let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
            assert_eq!(journal.reconcile_interrupted().unwrap(),1);
            assert_eq!(journal.reconcile_interrupted().unwrap(),0);
            assert_eq!(journal.get("m5-call-interrupted").unwrap().unwrap().state,
                EffectState::Uncertain);
            assert!(continuity.link_verified_effect(
                "m5-cp-interrupted","m5-call-interrupted",&journal
            ).is_err());
            let checkpoint=chats.recall_project_continuity(
                &source,"lu","m5-interrupt",&continuity
            ).unwrap().unwrap();
            assert_eq!(checkpoint.original_source_ids,vec![original]);
            assert_eq!(checkpoint.blockers,vec!["Interrupted worker"]);
            assert!(continuity.verified_effects_for_checkpoint(
                "m5-cp-interrupted",&journal
            ).unwrap().is_empty());
        }
        fs::remove_dir_all(root).unwrap();
    }
}


#[cfg(test)]
mod agent01a_real_human_journal_acceptance{
    use super::{chat_store::ChatStore,agent_plan_ledger::{
        AgentPlanLedger,PlanLedgerError,PlanStepKind,PlanStepSpec,PlanWrite
    }};
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    #[test]
    fn only_the_real_saved_human_turn_can_bind_durable_plan(){
        let root=std::env::temp_dir().join(format!(
            "lu-agent01-real-chat-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let pid=format!("p-{:032x}",3);
        let tid=format!("g-{:032x}",4);
        let fake=format!("g-{:032x}",5);
        let steps=[PlanStepSpec{kind:PlanStepKind::CollectSource,depends_on:None},
            PlanStepSpec{kind:PlanStepKind::HumanReview,depends_on:Some(0)}];
        let chat_id={
            let chats=ChatStore::open(&root).unwrap();
            let one=chats.create_chat().unwrap();
            let two=chats.create_chat().unwrap();
            let ledger=AgentPlanLedger::open(&root.join("agent.sqlite")).unwrap();
            assert_eq!(ledger.register(&pid,&one.chat_id,&tid,&steps,&chats),
                Err(PlanLedgerError::UnverifiedHumanTurn));
            chats.append_user_with_turn_id(
                &one.chat_id,&tid,"Inspect the reviewed facts first").unwrap();
            assert_eq!(ledger.register(&pid,&one.chat_id,&tid,&steps,&chats),
                Ok(PlanWrite::New));
            assert_eq!(ledger.register(&format!("p-{:032x}",9),
                &two.chat_id,&tid,&steps,&chats),
                Err(PlanLedgerError::UnverifiedHumanTurn));
            assert_eq!(ledger.register(&format!("p-{:032x}",10),
                &one.chat_id,&fake,&steps,&chats),
                Err(PlanLedgerError::UnverifiedHumanTurn));
            one.chat_id
        };
        let chats=ChatStore::open(&root).unwrap();
        let ledger=AgentPlanLedger::open(&root.join("agent.sqlite")).unwrap();
        assert_eq!(ledger.register(&pid,&chat_id,&tid,&steps,&chats),
            Ok(PlanWrite::AlreadyRecorded));
        assert_eq!(ledger.snapshot(&pid,&chat_id,&tid).unwrap().unwrap().steps.len(),2);
        drop(ledger);drop(chats);
        fs::remove_dir_all(root).unwrap();
    }
}


// AGENT-01B integration: both the real immutable ChatStore human turn and
// the real ANSF goal event must exist before any plan proposal is accepted.
// Approval is a separately matched, host-only human decision for an exact
// immutable revision hash, never authority to execute an application.
#[cfg(test)]
mod agent01b_real_chat_goal_revision_acceptance{
    use super::{
        ansf::AnsfStore,
        ansf_goals::{GoalEvent,GoalEventKind},
        chat_store::ChatStore,
        agent_plan_ledger::{AgentPlanLedger,PlanStepKind,PlanStepSpec},
        agent_plan_revisions::{
            AgentPlanRevisionStore,HostPlanReferenceVerifier,
            HostPlanReviewVerifier,HumanDecision,RevisionError,
            RevisionProposal,RevisionReason,RevisionStep,ExpectedDeliverable,
            RecordState,
        }
    };
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    struct RealSources<'a>{ansf:&'a AnsfStore}
    impl HostPlanReferenceVerifier for RealSources<'_>{
        fn goal_event_belongs_to_chat(&self,chat:&str,goal:&str,event:&str)->bool{
            self.ansf.goal_events(chat).ok().is_some_and(|events|
                events.iter().any(|entry|
                    entry.event_id==event&&entry.goal_id==goal
                    &&entry.kind==GoalEventKind::SetGoal))
        }
        fn assumption_source_is_readable(&self,_chat:&str,_source:&str)->bool{
            // Source-read authorization is a different host gate, intentionally
            // unavailable in this integration test and never guessed.
            false
        }
    }
    struct ExactReview<'a>{
        chat:&'a str,turn:&'a str,plan:&'a str,sha:&'a str,
    }
    impl HostPlanReviewVerifier for ExactReview<'_>{
        fn has_exact_human_decision(
            &self,chat:&str,turn:&str,plan:&str,sha:&str,decision:HumanDecision
        )->bool{
            decision==HumanDecision::Approve && chat==self.chat
                &&turn==self.turn&&plan==self.plan&&sha==self.sha
        }
    }
    #[test]
    fn real_saved_human_goal_and_exact_review_survive_restart_without_tool_grant(){
        let root=std::env::temp_dir().join(format!(
            "lu-agent01b-real-goal-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let plan=format!("p-{:032x}",9);
        let first=format!("g-{:032x}",10);
        let later=format!("g-{:032x}",11);
        let revision=format!("v-{:032x}",12);
        let goal_id=format!("goal-{:032x}",13);
        let goal_event_id=format!("m-{:032x}",14);
        let chat;
        let sha;
        {
            let chats=ChatStore::open(&root).unwrap();
            let ansf=AnsfStore::open(&root.join("ansf.sqlite")).unwrap();
            chat=chats.create_chat().unwrap().chat_id;
            let foreign=chats.create_chat().unwrap().chat_id;
            chats.append_user_with_turn_id(
                &chat,&first,"Build a research report with independently checked evidence"
            ).unwrap();
            let goal=GoalEvent{
                event_id:goal_event_id.clone(),goal_id:goal_id.clone(),
                question_id:None,kind:GoalEventKind::SetGoal,
                text:"Produce a source-backed technical report".into(),
                choice:None,comment:None
            };
            chats.append_goal_memory_event(&chat,goal.clone()).unwrap();
            ansf.ensure_session(&chat,"profile","Lu goal").unwrap();
            ansf.mirror_goal_event(&chat,&goal).unwrap();
            let file=root.join("plans.sqlite");
            let ledger=AgentPlanLedger::open(&file).unwrap();
            ledger.register(&plan,&chat,&first,&[
                PlanStepSpec{kind:PlanStepKind::CollectSource,depends_on:None},
                PlanStepSpec{kind:PlanStepKind::IndependentlyCheck,depends_on:Some(0)},
            ],&chats).unwrap();
            let proposals=AgentPlanRevisionStore::open(&file).unwrap();
            let request=RevisionProposal{
                goal_id:goal_id.clone(),goal_event_id:goal_event_id.clone(),
                assumption_sources:vec![],
                reason:RevisionReason::Initial,
                steps:vec![
                    RevisionStep{
                        kind:PlanStepKind::CollectSource,depends_on:None,
                        deliverable_id:format!("d-{:032x}",15),
                        expected:ExpectedDeliverable::SourceReferences,
                    },
                    RevisionStep{
                        kind:PlanStepKind::IndependentlyCheck,depends_on:Some(0),
                        deliverable_id:format!("d-{:032x}",16),
                        expected:ExpectedDeliverable::IndependentWitness,
                    },
                ],
            };
            let sources=RealSources{ansf:&ansf};
            assert_eq!(proposals.append_proposal(
                &plan,&foreign,&first,&revision,None,&request,&chats,&sources),
                Err(RevisionError::UnverifiedHumanTurn));
            let mut false_goal=request.clone();
            false_goal.goal_event_id=format!("m-{:032x}",99);
            assert_eq!(proposals.append_proposal(
                &plan,&chat,&first,&revision,None,&false_goal,&chats,&sources),
                Err(RevisionError::UnverifiedSource));
            let receipt=proposals.append_proposal(
                &plan,&chat,&first,&revision,None,&request,&chats,&sources
            ).unwrap();
            assert_eq!(receipt.record_state,RecordState::New);
            sha=receipt.revision_sha256;
            assert_eq!(proposals.latest(&plan,&chat).unwrap().unwrap().review_state,
                "PENDING_HUMAN_REVIEW");
            chats.append_user_with_turn_id(
                &chat,&later,"I explicitly approve the reviewed plan revision."
            ).unwrap();
            let decision=ExactReview{
                chat:&chat,turn:&later,plan:&plan,sha:&sha
            };
            assert_eq!(proposals.record_review(
                &plan,&chat,&later,&sha,HumanDecision::Approve,&chats,&decision),
                Ok(RecordState::New));
            assert_eq!(proposals.latest(&plan,&chat).unwrap().unwrap().execution_authority,
                "NOT_AUTHORIZED_FOR_EXECUTION");
        }
        {
            let chats=ChatStore::open(&root).unwrap();
            let proposals=AgentPlanRevisionStore::open(&root.join("plans.sqlite")).unwrap();
            assert_eq!(chats.trusted_user_turn_text(&chat,&later).as_deref(),
                Some("I explicitly approve the reviewed plan revision."));
            let reread=proposals.latest(&plan,&chat).unwrap().unwrap();
            assert_eq!(reread.review_state,"APPROVED_PLAN_ONLY");
            assert_eq!(reread.revision_sha256,sha);
            assert_eq!(reread.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        }
        fs::remove_dir_all(root).unwrap();
    }
}
