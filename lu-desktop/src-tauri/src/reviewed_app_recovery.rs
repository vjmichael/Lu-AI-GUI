//! E1-P08: fail-closed restart reconciliation of E1-P07 reviewed-app turns.
//!
//! The caller MUST hold the profile's exclusive DesktopOwner writer lease
//! before this function runs. No worker is spawned, no effect is retried,
//! no user turn is re-authorized, and no SQLite evidence is deleted.
//! This is ledger recovery, NOT proof that old OS descendants are reaped.
use std::{collections::BTreeMap,fs,path::{Path,PathBuf}};
use lu_core::{journal::EffectJournal,tasks::TaskStore};

const TASK_PREFIX:&str="lu-reviewed-task-";
const EFFECT_PREFIX:&str="lu-reviewed-effects-";
const MAX_RECOVERY_ENTRIES:usize=4096;

#[derive(Debug,Default,Clone,Copy,PartialEq,Eq)]
pub(crate) struct ReviewedRestartRecovery{
    pub task_journals:usize,
    pub effect_journals:usize,
    pub interrupted_tasks:usize,
    pub uncertain_effects:usize,
    pub partial_turns:usize,
}
#[derive(Default)]
struct RecoveryPair{
    task:Option<PathBuf>,
    effect:Option<PathBuf>,
}

fn real_local_file(path:&Path)->Result<(),String>{
    let metadata=fs::symlink_metadata(path)
        .map_err(|_|"Reviewed recovery file unavailable")?;
    if !metadata.is_file()||metadata.file_type().is_symlink(){
        return Err("Reviewed recovery refused a non-regular or linked journal".into())
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0{
            return Err("Reviewed recovery refused a reparse-point journal".into())
        }
    }
    Ok(())
}
fn valid_turn_id(id:&str)->bool{
    id.len()==34 && id.starts_with("g-")
        &&id[2..].bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
/// Only a *host-generated* E1-P07 journal name is eligible for recovery.
/// SQLite's -wal/-shm/-journal sidecars are checked for linked/reparse paths,
/// but never individually opened as effect/task stores.
fn classify(name:&str)->Result<Option<(String,bool,bool)>,String>{
    let (stem,is_task)=if let Some(stem)=name.strip_prefix(TASK_PREFIX){
        (stem,true)
    }else if let Some(stem)=name.strip_prefix(EFFECT_PREFIX){
        (stem,false)
    }else{return Ok(None)};
    let (database,sidecar)=if let Some(db)=stem.strip_suffix(".sqlite"){
        (db,false)
    }else if let Some(db)=stem.strip_suffix(".sqlite-wal")
        .or_else(||stem.strip_suffix(".sqlite-shm"))
        .or_else(||stem.strip_suffix(".sqlite-journal")){
        (db,true)
    }else{
        return Err("Malformed reviewed application journal filename".into())
    };
    if !valid_turn_id(database){
        return Err("Invalid reviewed application turn identity on disk".into())
    }
    Ok(Some((database.to_owned(),is_task,sidecar)))
}
/// Called once at desktop startup, *after* DesktopOwner::acquire and before
/// the desktop exposes tool execution. No recovery may run concurrently
/// with a live Lu desktop using the same profile.
pub(crate) fn reconcile_reviewed_application_journals(
    root:&Path
)->Result<ReviewedRestartRecovery,String>{
    let mut turns=BTreeMap::<String,RecoveryPair>::new();
    let mut count=0usize;
    for entry in fs::read_dir(root).map_err(|_|"Cannot inspect reviewed journals")?{
        count=count.checked_add(1).ok_or("Recovery directory entry overflow")?;
        if count>MAX_RECOVERY_ENTRIES{
            return Err("Too many profile files for bounded reviewed recovery".into())
        }
        let entry=entry.map_err(|_|"Cannot read reviewed journal entry")?;
        let filename=entry.file_name();
        let Some(filename)=filename.to_str()else{continue};
        let Some((turn,is_task,sidecar))=classify(filename)?else{continue};
        real_local_file(&entry.path())?;
        if sidecar{continue}
        let pair=turns.entry(turn).or_default();
        let slot=if is_task{&mut pair.task}else{&mut pair.effect};
        if slot.replace(entry.path()).is_some(){
            return Err("Duplicate reviewed application journal".into())
        }
    }
    let mut recovered=ReviewedRestartRecovery::default();
    for pair in turns.values(){
        if pair.task.is_none()||pair.effect.is_none(){
            recovered.partial_turns+=1;
        }
        if let Some(path)=&pair.task{
            let tasks=TaskStore::open(path)
                .map_err(|_|"Reviewed task journal cannot be safely reopened")?;
            recovered.interrupted_tasks+=tasks.reconcile_interrupted()
                .map_err(|_|"Reviewed task restart reconciliation failed")?;
            recovered.task_journals+=1;
        }
        if let Some(path)=&pair.effect{
            let effects=EffectJournal::open(path)
                .map_err(|_|"Reviewed effect journal cannot be safely reopened")?;
            recovered.uncertain_effects+=effects.reconcile_interrupted()
                .map_err(|_|"Reviewed effect restart reconciliation failed")?;
            recovered.effect_journals+=1;
        }
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests{
    use super::*;
    use lu_core::{
        contracts::{ProtoPayload,ToolInvocation},
        journal::{EffectState,PrepareOutcome},
        tasks::TaskState,
    };
    fn root()->PathBuf{
        let path=std::env::temp_dir().join(format!(
            "lu-e1-p08-restart-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
    fn identity(ch:char)->String{format!("g-{}",ch.to_string().repeat(32))}
    fn task_db(root:&Path,id:&str)->PathBuf{
        root.join(format!("{TASK_PREFIX}{id}.sqlite"))
    }
    fn effect_db(root:&Path,id:&str)->PathBuf{
        root.join(format!("{EFFECT_PREFIX}{id}.sqlite"))
    }
    fn effect(id:&str)->ToolInvocation{
        ToolInvocation{
            call_id:id.into(),tool_id:"lu.reviewed.git.head".into(),
            workspace_id:Some("approved-space".into()),
            input:ProtoPayload{type_url:"lu.test.request".into(),bytes:vec![1]},
        }
    }
    #[test]
    fn interrupted_os_worker_is_fenced_not_replayed_and_applied_receipt_survives(){
        let root=root();
        let id=identity('a');
        {
            let tasks=TaskStore::open(&task_db(&root,&id)).unwrap();
            tasks.create("running-child",Some("approved-space"),None).unwrap();
            tasks.start("running-child").unwrap();
            tasks.create("completed-child",Some("approved-space"),None).unwrap();
            tasks.start("completed-child").unwrap();
            tasks.complete("completed-child").unwrap();
            let effects=EffectJournal::open(&effect_db(&root,&id)).unwrap();
            assert_eq!(effects.prepare(&effect("outer-executing")).unwrap(),
                PrepareOutcome::New);
            effects.mark_executing("outer-executing").unwrap();
            effects.prepare(&effect("inner-applied")).unwrap();
            effects.mark_executing("inner-applied").unwrap();
            effects.mark_applied("inner-applied",b"verified-native-result").unwrap();
        }
        let result=reconcile_reviewed_application_journals(&root).unwrap();
        assert_eq!(result,ReviewedRestartRecovery{
            task_journals:1,effect_journals:1,
            interrupted_tasks:1,uncertain_effects:1,partial_turns:0
        });
        let tasks=TaskStore::open(&task_db(&root,&id)).unwrap();
        assert_eq!(tasks.get("running-child").unwrap().unwrap().state,
            TaskState::Interrupted);
        assert_eq!(tasks.get("completed-child").unwrap().unwrap().state,
            TaskState::Completed);
        let effects=EffectJournal::open(&effect_db(&root,&id)).unwrap();
        assert_eq!(effects.get("outer-executing").unwrap().unwrap().state,
            EffectState::Uncertain);
        let applied=effects.get("inner-applied").unwrap().unwrap();
        assert_eq!(applied.state,EffectState::Applied);
        assert_eq!(applied.receipt.as_deref(),Some(b"verified-native-result".as_slice()));
        drop(tasks);drop(effects);
        let again=reconcile_reviewed_application_journals(&root).unwrap();
        assert_eq!(again.interrupted_tasks,0);
        assert_eq!(again.uncertain_effects,0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn partial_crash_between_journal_creations_never_creates_or_replays_missing_db(){
        let root=root();
        let first=identity('b');let second=identity('c');
        let tasks=TaskStore::open(&task_db(&root,&first)).unwrap();
        tasks.create("task-only",Some("approved-space"),None).unwrap();
        tasks.start("task-only").unwrap();
        drop(tasks);
        let effects=EffectJournal::open(&effect_db(&root,&second)).unwrap();
        effects.prepare(&effect("effect-only")).unwrap();
        effects.mark_executing("effect-only").unwrap();
        drop(effects);
        let report=reconcile_reviewed_application_journals(&root).unwrap();
        assert_eq!(report.partial_turns,2);
        assert_eq!(report.interrupted_tasks,1);
        assert_eq!(report.uncertain_effects,1);
        assert!(!effect_db(&root,&first).exists());
        assert!(!task_db(&root,&second).exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn malformed_reviewed_identity_is_refused_without_creating_database(){
        let root=root();
        fs::write(root.join("lu-reviewed-effects-user-picked.sqlite"),b"not db").unwrap();
        assert!(reconcile_reviewed_application_journals(&root).is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(),1);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_reviewed_journal_is_rejected_and_unrelated_file_is_untouched(){
        use std::os::unix::fs::symlink;
        let root=root();
        let target=root.join("unrelated.txt");
        fs::write(&target,b"do not touch").unwrap();
        symlink(&target,effect_db(&root,&identity('d'))).unwrap();
        assert!(reconcile_reviewed_application_journals(&root).is_err());
        assert_eq!(fs::read(&target).unwrap(),b"do not touch");
        fs::remove_dir_all(root).unwrap();
    }
}
