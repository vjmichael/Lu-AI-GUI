//! CP-11B-R1 host-owned scheduled inference. Direct text stream only: no
//! conversation history, tool router, memory retrieval, or effect authority.
use crate::{generation_queue::BenchmarkLease, next_id, DesktopState};
use lu_core::{
    inference_bridge::BridgeError,
    inference_host::EnginePhase,
    inference_supervisor::SupervisorStatus,
    inference_wire::decode_frame,
    native_background_scheduler::{now_unix, MAX_SCHEDULE_OUTPUT},
    wire::proto,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
const MAX_TOKENS: u32 = 128;
const LIMIT: Duration = Duration::from_secs(35);

// The watchdog never waits for generation; one idle-only lease is held from
// before spawning through terminal journal commit. Chat arrivals wait only
// for this bounded occurrence, then have priority over further schedules.
pub(crate) fn try_start(
    app: &AppHandle,
    state: &Arc<DesktopState>,
    snapshot: &SupervisorStatus,
) -> bool {
    if snapshot.phase != EnginePhase::Ready || snapshot.generation_id.is_some() {
        return false;
    }
    let Some(model) = snapshot.model_ref.as_deref() else {
        return false;
    };
    let Ok(rows) = state.background_scheduler.ai_schedules() else {
        return false;
    };
    let now = now_unix();
    let Some(row) = rows.into_iter().find(|r| {
        r.state == "ACTIVE"
            && r.model_ref == model
            && r.next_due <= now
            && r.run_count < 32
            && state
                .chats
                .history_window(&r.workspace_id, None, "older", 1)
                .is_ok()
    }) else {
        return false;
    };
    let Ok(lease) = state.generation_queue.try_benchmark() else {
        return false;
    };
    let app = app.clone();
    let owner = state.clone();
    let snapshot = snapshot.clone();
    std::thread::spawn(move || run(app, owner, snapshot, lease, row));
    true
}
fn run(
    app: AppHandle,
    owner: Arc<DesktopState>,
    snapshot: SupervisorStatus,
    lease: BenchmarkLease,
    row: lu_core::native_background_scheduler::AiSchedule,
) {
    let Ok(_fence) = owner.model_profile_admission.try_lock() else {
        return;
    };
    if lease.yield_requested()
        || !owner.supervisor.status().is_ok_and(|s| s == snapshot)
        || owner
            .chats
            .history_window(&row.workspace_id, None, "older", 1)
            .is_err()
    {
        return;
    }
    let Some(sid) = snapshot.session_id.as_deref() else {
        return;
    };
    let Ok(Some((row, seq))) = owner.background_scheduler.claim_ai_at(
        &row.id,
        &row.workspace_id,
        &row.model_ref,
        now_unix(),
    ) else {
        return;
    };
    let _ = app.emit("lu-scheduled-ai-changed", ());
    let result = if lease.yield_requested() {
        Err("STOPPED")
    } else {
        generate(&owner, &row.id, seq, sid, &row.prompt)
    };
    let (status, output) = match result {
        Ok(text)
            if owner
                .chats
                .history_window(&row.workspace_id, None, "older", 1)
                .is_ok() =>
        {
            ("COMPLETED", text)
        }
        Ok(_) => ("STOPPED", String::new()),
        Err(status) => (status, String::new()),
    };
    for i in 0..4 {
        if owner
            .background_scheduler
            .finish_ai(&row.id, seq, status, &output)
            .is_ok()
        {
            break;
        }
        if i < 3 {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let _ = app.emit("lu-scheduled-ai-changed", ());
}
struct Done<'a>(&'a AtomicBool);
impl Drop for Done<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
fn generate(
    owner: &DesktopState,
    id: &str,
    sequence: u64,
    sid: &str,
    prompt: &str,
) -> Result<String, &'static str> {
    let supervisor = &owner.supervisor;
    let gid = next_id("scheduled-");
    let rid = next_id("req-");
    if !owner
        .background_scheduler
        .ai_running(id, sequence)
        .unwrap_or(false)
    {
        return Err("STOPPED");
    }
    supervisor
        .reserve_generation_with_history(sid, &gid, &rid, prompt, MAX_TOKENS, &[])
        .map_err(|_| "FAILED")?;
    let done = AtomicBool::new(false);
    let start = Instant::now();
    let mut text = String::new();
    let mut pieces = 0u32;
    let mut terminal = false;
    let (outcome, stopped) = std::thread::scope(|scope| {
        let monitor = scope.spawn(|| loop {
            if done.load(Ordering::Acquire) {
                break false;
            }
            if !owner
                .background_scheduler
                .ai_running(id, sequence)
                .unwrap_or(false)
                || start.elapsed() >= LIMIT
            {
                let _ = supervisor.cancel(sid, &gid, &next_id("req-"));
                break true;
            }
            std::thread::sleep(Duration::from_millis(25));
        });
        let guard = Done(&done);
        let res = supervisor.run_reserved_with_history(
            sid,
            &gid,
            &rid,
            prompt.into(),
            MAX_TOKENS,
            vec![],
            |wire| {
                let frame = decode_frame(&wire)?;
                if frame.request_id != rid {
                    return Err(BridgeError::Consumer);
                }
                let Some(proto::inference_frame::Body::Event(event)) = frame.body else {
                    return Err(BridgeError::Consumer);
                };
                if event.session_id != sid || event.generation_id != gid || terminal {
                    return Err(BridgeError::Consumer);
                }
                match event.kind {
                    Some(proto::inference_event::Kind::TextChunk(chunk)) => {
                        if chunk.text.is_empty()
                            || chunk.chunk_index != pieces
                            || text.len() + chunk.text.len() > MAX_SCHEDULE_OUTPUT
                        {
                            return Err(BridgeError::Consumer);
                        }
                        text.push_str(&chunk.text);
                        pieces += 1;
                    }
                    Some(proto::inference_event::Kind::Finished(end)) => {
                        if end.cancelled
                            || end.generated_bytes != text.len() as u64
                            || end.output_pieces != pieces
                        {
                            return Err(BridgeError::Consumer);
                        }
                        terminal = true;
                    }
                    Some(proto::inference_event::Kind::State(_))
                    | Some(proto::inference_event::Kind::Failure(_)) => (),
                    _ => return Err(BridgeError::Consumer),
                }
                Ok(())
            },
        );
        drop(guard);
        (res, monitor.join().unwrap_or(true))
    });
    if stopped
        || !owner
            .background_scheduler
            .ai_running(id, sequence)
            .unwrap_or(false)
    {
        return Err("STOPPED");
    }
    if outcome.is_err() || !terminal || text.trim().is_empty() {
        return Err("FAILED");
    }
    Ok(text)
}
