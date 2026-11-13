use lu_core::tasks::{TaskState, TaskStore, TaskStoreError};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn durable_progress_survives_restart_without_forced_timeout() {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("lu-task-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let db = dir.join("tasks.sqlite");
    {
        let tasks = TaskStore::open(&db).unwrap();
        tasks
            .create("run-1", Some("nvp-session"), Some("results"))
            .unwrap();
        tasks.start("run-1").unwrap();
        tasks.heartbeat("run-1").unwrap();
        tasks
            .checkpoint("run-1", 25, "saved first scene", b"protobuf-task-state")
            .unwrap();
        assert!(matches!(
            tasks.checkpoint("run-1", 12, "progress backwards", b"bad"),
            Err(TaskStoreError::InvalidTransition)
        ));
        let record = tasks.get("run-1").unwrap().unwrap();
        assert_eq!(record.progress, 25);
        assert_eq!(record.output_folder.as_deref(), Some("results"));
    }
    {
        let tasks = TaskStore::open(&db).unwrap();
        assert_eq!(tasks.reconcile_interrupted().unwrap(), 1);
        let record = tasks.get("run-1").unwrap().unwrap();
        assert_eq!(record.state, TaskState::Interrupted);
        assert_eq!(record.checkpoint, b"protobuf-task-state");
        // Restart preserves the immutable original checkpoint, not the
        // execution authority. INTERRUPTED cannot be promoted back to
        // RUNNING, even by a caller using the original task ID.
        assert_eq!(
            tasks.resume("run-1"),
            Err(TaskStoreError::InvalidTransition)
        );
        assert_eq!(
            tasks.heartbeat("run-1"),
            Err(TaskStoreError::InvalidTransition)
        );
        assert_eq!(
            tasks.checkpoint("run-1", 90, "unapproved replay", b"replay"),
            Err(TaskStoreError::InvalidTransition)
        );
        assert_eq!(
            tasks.complete("run-1"),
            Err(TaskStoreError::InvalidTransition)
        );
        let original = tasks.get("run-1").unwrap().unwrap();
        assert_eq!(original.state, TaskState::Interrupted);
        assert_eq!(original.progress, 25);
        assert_eq!(original.checkpoint, b"protobuf-task-state");
        assert_eq!(original.output_folder.as_deref(), Some("results"));

        // An independently authorized caller may start a NEW task identity.
        // TaskStore::create does not itself grant fresh tool authorization.
        tasks
            .create("run-1-new", Some("nvp-session"), Some("results"))
            .unwrap();
        tasks.start("run-1-new").unwrap();
        tasks
            .checkpoint("run-1-new", 90, "new task progress", b"new checkpoint")
            .unwrap();
        tasks.complete("run-1-new").unwrap();
        assert_eq!(
            tasks.get("run-1-new").unwrap().unwrap().state,
            TaskState::Completed
        );
        assert_eq!(
            tasks.get("run-1").unwrap().unwrap().state,
            TaskState::Interrupted
        );
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn completed_tasks_are_not_implicitly_resumed() {
    let tasks = TaskStore::open(std::path::Path::new(":memory:")).unwrap();
    tasks.create("run-2", None, None).unwrap();
    assert_eq!(tasks.reconcile_interrupted().unwrap(), 0);
    assert!(matches!(
        tasks.resume("run-2"),
        Err(TaskStoreError::InvalidTransition)
    ));
    tasks.start("run-2").unwrap();
    tasks.complete("run-2").unwrap();
    assert_eq!(tasks.reconcile_interrupted().unwrap(), 0);
    assert!(matches!(
        tasks.resume("run-2"),
        Err(TaskStoreError::InvalidTransition)
    ));
}
