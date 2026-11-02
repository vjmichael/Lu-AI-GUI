//! Restart recovery gate: do not replay interrupted effects or resume tasks.
use lu_core::{
    contracts::{ProtoPayload, ToolInvocation},
    journal::{EffectJournal, EffectState, PrepareOutcome},
    recovery::{reconcile_native_startup, RecoverySummary},
    tasks::{TaskState, TaskStore},
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn sandbox() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("lu-recovery-{}-{nonce}", std::process::id()));
    fs::create_dir(&root).unwrap();
    root
}

#[test]
fn restart_marks_running_tasks_interrupted_and_started_effects_uncertain() {
    let root = sandbox();
    let task_db = root.join("tasks.sqlite");
    let effect_db = root.join("effects.sqlite");
    let call = ToolInvocation {
        call_id: "effect-one".into(),
        tool_id: "lu.files.write".into(),
        workspace_id: Some("workspace-one".into()),
        input: ProtoPayload {
            type_url: "type.googleapis.com/lu.v1.WriteRequest".into(),
            bytes: vec![1, 2, 3],
        },
    };
    {
        let tasks = TaskStore::open(&task_db).unwrap();
        tasks
            .create("task-one", Some("workspace-one"), None)
            .unwrap();
        tasks.start("task-one").unwrap();
        let effects = EffectJournal::open(&effect_db).unwrap();
        assert_eq!(effects.prepare(&call).unwrap(), PrepareOutcome::New);
        effects.mark_executing(&call.call_id).unwrap();
    }
    // Exclusive startup begins only after the old worker has exited.
    let tasks = TaskStore::open(&task_db).unwrap();
    let effects = EffectJournal::open(&effect_db).unwrap();
    assert_eq!(
        reconcile_native_startup(&tasks, &effects).unwrap(),
        RecoverySummary {
            interrupted_tasks: 1,
            uncertain_effects: 1
        }
    );
    assert_eq!(
        tasks.get("task-one").unwrap().unwrap().state,
        TaskState::Interrupted
    );
    assert_eq!(
        effects.get(&call.call_id).unwrap().unwrap().state,
        EffectState::Uncertain
    );
    assert_eq!(
        effects.prepare(&call).unwrap(),
        PrepareOutcome::Existing(EffectState::Uncertain)
    );
    assert_eq!(
        reconcile_native_startup(&tasks, &effects).unwrap(),
        RecoverySummary {
            interrupted_tasks: 0,
            uncertain_effects: 0
        }
    );
    drop(effects);
    drop(tasks);
    fs::remove_dir_all(root).unwrap();
}
