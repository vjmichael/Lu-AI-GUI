//! Bounded, host-owned, foreground-first scheduler for one physical model session.
//!
//! Accepts turns from different conversations without attempting to run two
//! simultaneous generations against a single loaded llama-server. A ticket
//! holds its chat/session binding, and always releases its slot on drop.
//! Waiting and cancellation never hold the inference engine mutex.
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Condvar, Mutex},
};
use serde::Serialize;

const MAX_JOBS: usize = 8;
// Leave at least six queue places for explicit human requests.
const MAX_BACKGROUND_JOBS: usize = 2;
/// One bounded live response per generation. The ANSF SQLite journal is
/// the durable source; this RAM snapshot only repairs a live WebView reload.
pub const MAX_LIVE_STREAM_BYTES: usize = 1024 * 1024;
pub const MAX_CHUNK_BYTES: usize = 64 * 1024;
// Admit one already-approved background turn after four consecutive
// foreground turns, preventing indefinite starvation without preempting UI.
const MAX_FOREGROUND_BURST: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnClass { Foreground, Background }


#[derive(Default)]
struct State {
    foreground_waiting: VecDeque<String>,
    background_waiting: VecDeque<String>,
    active: Option<String>,
    foreground_streak: u8,
    jobs: HashMap<String, Job>,
}
impl State {
    fn next_ready(&self) -> Option<&str> {
        let foreground = self.foreground_waiting.front();
        let background = self.background_waiting.front();
        if foreground.is_some()
            && (self.foreground_streak < MAX_FOREGROUND_BURST || background.is_none()) {
            foreground.map(String::as_str)
        } else {
            background.or(foreground).map(String::as_str)
        }
    }
}
struct Job {
    chat_id: String,
    session_id: String,
    class: TurnClass,
    cancelled: bool,
    completed: bool,
    text: String,
    next_index: u32,
}

#[derive(Default)]
pub struct GenerationQueue {
    state: Mutex<State>,
    ready: Condvar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelState {
    Queued,
    Active,
}

/// Read-only, exact-session, exact-chat snapshot for a reconnecting WebView.
/// The host never replays a generation, grants permission or supplies another
/// conversation's bytes. The cursor refers to the next stream event index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStreamSnapshot {
    pub chat_id: String,
    pub generation_id: String,
    pub phase: &'static str,
    pub text: String,
    pub next_index: u32,
    pub cancelling: bool,
}

/// Owns its place in the queue; releases the slot on every exit path.
pub struct Ticket {
    queue: Arc<GenerationQueue>,
    id: String,
}
impl GenerationQueue {
    pub fn enqueue(
        self: &Arc<Self>,
        session_id: &str,
        chat_id: &str,
        generation_id: &str,
    ) -> Result<(Ticket, usize), String> {
        self.enqueue_with_class(session_id, chat_id, generation_id, TurnClass::Foreground)
    }
    /// Only the native host assigns the class. Browser/model arguments cannot
    /// promote a background continuation to foreground authority.
    pub fn enqueue_with_class(
        self: &Arc<Self>,
        session_id: &str,
        chat_id: &str,
        generation_id: &str,
        class: TurnClass,
    ) -> Result<(Ticket, usize), String> {
        let mut state = self.state.lock().map_err(|_| "generation queue poisoned")?;
        if state.jobs.len() >= MAX_JOBS {
            return Err("generation queue full (8 turns maximum)".into());
        }
        if class == TurnClass::Background && state.jobs.values()
            .filter(|j| j.class == TurnClass::Background).count() >= MAX_BACKGROUND_JOBS {
            return Err("background continuation quota reached".into());
        }
        if state.jobs.contains_key(generation_id)
            || state.jobs.values().any(|j| j.chat_id == chat_id)
        {
            return Err("conversation already has a pending generation".into());
        }
        // An older pending session must never share a queue with newly loaded
        // weights, even if model unloading later acquires the engine lock.
        if state.jobs.values().any(|j| j.session_id != session_id) {
            return Err("generation queue belongs to a different model session".into());
        }
        state.jobs.insert(
            generation_id.into(),
            Job {
                chat_id: chat_id.into(),
                session_id: session_id.into(),
                class,
                cancelled: false,
                completed: false,
                text: String::new(),
                next_index: 0,
            },
        );
        match class {
            TurnClass::Foreground => state.foreground_waiting.push_back(generation_id.into()),
            TurnClass::Background => state.background_waiting.push_back(generation_id.into()),
        }
        // Snapshot of scheduling position; later arrivals can reprioritize it.
        let position = usize::from(state.active.is_some()) + match class {
            TurnClass::Foreground => state.foreground_waiting.len()
                + usize::from(state.foreground_streak >= MAX_FOREGROUND_BURST
                    && !state.background_waiting.is_empty()),
            TurnClass::Background => state.foreground_waiting.len() + state.background_waiting.len(),
        };
        Ok((
            Ticket {
                queue: Arc::clone(self),
                id: generation_id.into(),
            },
            position,
        ))
    }

    pub fn cancel(&self, session_id: &str, generation_id: &str) -> Result<CancelState, String> {
        let mut state = self.state.lock().map_err(|_| "generation queue poisoned")?;
        let job = state
            .jobs
            .get_mut(generation_id)
            .ok_or("generation not found")?;
        if job.session_id != session_id {
            return Err("wrong model session".into());
        }
        if job.completed { return Err("generation already completed".into()); }
        job.cancelled = true;
        let status = if state.active.as_deref() == Some(generation_id) {
            CancelState::Active
        } else {
            CancelState::Queued
        };
        if status == CancelState::Queued {
            // A cancelled waiter must not hold the head of either priority
            // lane while its worker is unwinding and releasing its ticket.
            state.foreground_waiting.retain(|id| id != generation_id);
            state.background_waiting.retain(|id| id != generation_id);
        }
        self.ready.notify_all();
        Ok(status)
    }
    /// Cancel only the uniquely registered generation for a specific
    /// host-owned conversation label, without accepting an arbitrary
    /// generation ID from the renderer. Used by PLAN-07B's task window.
    pub fn cancel_for_chat(
        &self,chat_id:&str
    )->Result<Option<(String,String,CancelState)>,String>{
        let mut state=self.state.lock()
            .map_err(|_|"generation queue poisoned")?;
        let Some((generation_id,session_id))=state.jobs.iter()
            .find(|(_,job)|job.chat_id==chat_id)
            .map(|(id,job)|(id.clone(),job.session_id.clone())) else{
            return Ok(None)
        };
        let job=state.jobs.get_mut(&generation_id)
            .ok_or("generation disappeared during scoped cancellation")?;
        if job.completed { return Ok(None); }
        job.cancelled=true;
        let phase=if state.active.as_deref()==Some(generation_id.as_str()){
            CancelState::Active
        }else{CancelState::Queued};
        if phase==CancelState::Queued {
            state.foreground_waiting.retain(|id|id!=&generation_id);
            state.background_waiting.retain(|id|id!=&generation_id);
        }
        self.ready.notify_all();
        Ok(Some((session_id,generation_id,phase)))
    }

    /// P0-AUTH: read-only, exact-chat observation for native operator
    /// custody/cancel UI. No model/PID/workspace identity leaks out.
    pub fn scoped_job_active(&self,chat_id:&str)
        ->Result<Option<bool>,String>{
        let state=self.state.lock()
            .map_err(|_|"generation queue poisoned")?;
        Ok(state.jobs.iter().find(|(_,job)|job.chat_id==chat_id)
            .map(|(id,_)|state.active.as_deref()==Some(id.as_str())))
    }
    /// Conservative foreground gate for background-worker admission.
    /// Include queued and active human work; fail closed on lock poisoning.
    pub fn has_foreground_jobs(&self) -> bool {
        self.state.lock()
            .map(|state| state.jobs.iter().any(|(id,job)|
                job.class == TurnClass::Foreground
                && (!job.cancelled || state.active.as_deref()==Some(id.as_str()))))
            .unwrap_or(true)
    }
    /// A chunk must arrive from the currently running original generation,
    /// in order, under bounded memory. Reconnect repairs missed UI events
    /// from this in-memory text; it is never used as a durable ANSF replacement.
    pub fn record_chunk(
        &self, session_id:&str, generation_id:&str, index:u32, text:&str,
    ) -> Result<(),String> {
        if text.is_empty() || text.len()>MAX_CHUNK_BYTES || text.contains('\0') {
            return Err("invalid bounded streaming chunk".into());
        }
        let mut state=self.state.lock().map_err(|_|"generation queue poisoned")?;
        if state.active.as_deref()!=Some(generation_id) {
            return Err("generation is not active for streaming".into());
        }
        let job=state.jobs.get_mut(generation_id)
            .ok_or("generation ticket missing during stream")?;
        if job.session_id!=session_id || job.cancelled || job.completed {
            return Err("generation stream cancelled, completed or session changed".into());
        }
        if job.next_index!=index || index==u32::MAX {
            return Err("out-of-order or unbounded generation chunk".into());
        }
        let total=job.text.len().checked_add(text.len())
            .ok_or("generation stream size overflow")?;
        if total>MAX_LIVE_STREAM_BYTES {
            return Err("generation stream reached the 1 MiB maximum".into());
        }
        job.text.push_str(text);
        job.next_index+=1;
        Ok(())
    }
    pub fn reconnect_snapshot(
        &self, session_id:&str, chat_id:&str,
    )->Result<Option<LiveStreamSnapshot>,String>{
        let state=self.state.lock().map_err(|_|"generation queue poisoned")?;
        Ok(state.jobs.iter()
            .find(|(_,job)|job.chat_id==chat_id && job.session_id==session_id)
            .map(|(generation_id,job)|LiveStreamSnapshot {
                chat_id:chat_id.to_owned(),
                generation_id:generation_id.clone(),
                phase:if state.active.as_deref()==Some(generation_id.as_str()){
                    "running"
                }else{"queued"},
                text:job.text.clone(),next_index:job.next_index,
                cancelling:job.cancelled,
            }))
    }
    pub fn has_jobs(&self) -> bool {
        self.state
            .lock()
            .map(|x| !x.jobs.is_empty())
            .unwrap_or(true)
    }
    fn finish(&self, generation_id: &str) {
        if let Ok(mut state) = self.state.lock() {
            if state.active.as_deref() == Some(generation_id) {
                state.active = None;
            }
            state.foreground_waiting.retain(|x| x != generation_id);
            state.background_waiting.retain(|x| x != generation_id);
            state.jobs.remove(generation_id);
            self.ready.notify_all();
        }
    }
}
impl Ticket {
    /// May run on one of a bounded number of blocking host worker threads.
    pub fn wait_for_slot(&self) -> Result<(), String> {
        let mut state = self
            .queue
            .state
            .lock()
            .map_err(|_| "generation queue poisoned")?;
        loop {
            let job = state
                .jobs
                .get(&self.id)
                .ok_or("generation reservation lost")?;
            if job.cancelled {
                return Err("Cancelled while queued".into());
            }
            if state.active.is_none() && state.next_ready() == Some(self.id.as_str()) {
                let class = job.class;
                let removed = match class {
                    TurnClass::Foreground => state.foreground_waiting.pop_front(),
                    TurnClass::Background => state.background_waiting.pop_front(),
                };
                if removed.as_deref() != Some(self.id.as_str()) {
                    return Err("generation scheduler lost FIFO custody".into());
                }
                state.foreground_streak = match class {
                    TurnClass::Foreground => state.foreground_streak
                        .saturating_add(1).min(MAX_FOREGROUND_BURST),
                    TurnClass::Background => 0,
                };
                state.active = Some(self.id.clone());
                return Ok(());
            }
            state = self
                .queue
                .ready
                .wait(state)
                .map_err(|_| "generation queue poisoned")?;
        }
    }
    /// Final ANSF progress+complete commit and a concurrent native Stop are
    /// ordered by this same queue mutex. Cancellation wins if it was
    /// recorded first; a post-commit Stop cannot change a complete answer.
    /// Hold this small lock only for the final existing journal write, not
    /// during model inference or streaming.
    pub fn commit_completed(
        &self, session_id:&str, commit:impl FnOnce()->Result<(),String>,
    )->Result<(),String>{
        let mut state=self.queue.state.lock()
            .map_err(|_|"generation queue poisoned")?;
        if state.active.as_deref()!=Some(self.id.as_str()){
            return Err("generation is not active during completion".into());
        }
        let job=state.jobs.get_mut(&self.id)
            .ok_or("generation ticket missing at completion")?;
        if job.session_id!=session_id || job.cancelled || job.completed {
            return Err("Cancelled before generation completion".into());
        }
        commit()?;
        job.completed=true;
        Ok(())
    }
    pub fn cancelled(&self) -> bool {
        self.queue
            .state
            .lock()
            .map(|s| s.jobs.get(&self.id).is_some_and(|j| j.cancelled))
            .unwrap_or(true)
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        self.queue.finish(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, thread, time::Duration};
    #[test]
    fn final_journal_commit_is_ordered_against_cancellation(){
        let q=Arc::new(GenerationQueue::default());
        let (a,_)=q.enqueue("s","c","g").unwrap();
        a.wait_for_slot().unwrap();
        q.record_chunk("s","g",0,"answer").unwrap();
        assert_eq!(q.cancel("s","g"),Ok(CancelState::Active));
        let mut published=false;
        assert!(a.commit_completed("s",||{published=true;Ok(())}).is_err());
        assert!(!published,"cancelled generation cannot publish a completed answer");
        drop(a);
        let (b,_)=q.enqueue("s","c","g2").unwrap();
        b.wait_for_slot().unwrap();
        drop(b);
    }
    #[test]
    fn completed_journal_turn_rejects_late_native_stop(){
        let q=Arc::new(GenerationQueue::default());
        let (a,_)=q.enqueue("s","c","g").unwrap();
        a.wait_for_slot().unwrap();
        let mut commits=0;
        a.commit_completed("s",||{commits+=1;Ok(())}).unwrap();
        assert_eq!(commits,1);
        assert!(q.cancel("s","g").is_err());
        assert!(!a.cancelled());
        drop(a);
    }
    #[test]
    fn live_reconnect_is_exact_chat_and_session_and_byte_bounded(){
        let q=Arc::new(GenerationQueue::default());
        let (ticket,_)=q.enqueue("session-a","chat-a","g-1").unwrap();
        assert_eq!(q.reconnect_snapshot("session-b","chat-a").unwrap(),None);
        let queued=q.reconnect_snapshot("session-a","chat-a").unwrap().unwrap();
        assert_eq!(queued.phase,"queued");
        assert_eq!(queued.next_index,0);
        ticket.wait_for_slot().unwrap();
        q.record_chunk("session-a","g-1",0,"alpha").unwrap();
        q.record_chunk("session-a","g-1",1,"✓").unwrap();
        assert_eq!(q.record_chunk("session-a","g-1",1,"duplicate"),
            Err("out-of-order or unbounded generation chunk".into()));
        assert!(q.record_chunk("session-b","g-1",2,"wrong session").is_err());
        assert_eq!(q.reconnect_snapshot("session-a","chat-b").unwrap(),None);
        let live=q.reconnect_snapshot("session-a","chat-a").unwrap().unwrap();
        assert_eq!(live.phase,"running");
        assert_eq!(live.text,"alpha✓");
        assert_eq!(live.next_index,2);
        assert!(q.record_chunk("session-a","g-1",2,&"x".repeat(MAX_CHUNK_BYTES+1)).is_err());
        // Oversize rejection does not change the retained, exact stream.
        assert_eq!(q.reconnect_snapshot("session-a","chat-a").unwrap().unwrap().next_index,2);
        drop(ticket);
        assert!(q.reconnect_snapshot("session-a","chat-a").unwrap().is_none());
    }
    #[test]
    fn cancelled_generation_cannot_append_or_leak_across_reconnect(){
        let q=Arc::new(GenerationQueue::default());
        let (ticket,_)=q.enqueue("s","c","g").unwrap();
        ticket.wait_for_slot().unwrap();
        q.record_chunk("s","g",0,"partial").unwrap();
        assert_eq!(q.cancel("s","g"),Ok(CancelState::Active));
        assert!(q.record_chunk("s","g",1,"after cancellation").is_err());
        let snap=q.reconnect_snapshot("s","c").unwrap().unwrap();
        assert!(snap.cancelling);
        assert_eq!(snap.text,"partial");
        drop(ticket);
        assert!(q.reconnect_snapshot("s","c").unwrap().is_none());
    }
    #[test]
    fn another_chat_queues_instead_of_interrupting() {
        let gate = Arc::new(GenerationQueue::default());
        let (a, first) = gate.enqueue("s1", "chat1", "g1").unwrap();
        let (b, second) = gate.enqueue("s1", "chat2", "g2").unwrap();
        assert_eq!((first, second), (1, 2));
        a.wait_for_slot().unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            b.wait_for_slot().unwrap();
            tx.send(()).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
        drop(a);
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
        assert!(!gate.has_jobs());
    }
    #[test]
    fn queued_cancellation_does_not_cancel_active_chat() {
        let gate = Arc::new(GenerationQueue::default());
        let (a, _) = gate.enqueue("s1", "A", "g1").unwrap();
        let (b, _) = gate.enqueue("s1", "B", "g2").unwrap();
        a.wait_for_slot().unwrap();
        assert_eq!(gate.cancel("s1", "g2").unwrap(), CancelState::Queued);
        assert!(b.wait_for_slot().is_err());
        assert!(!a.cancelled());
        drop(b);
        drop(a);
        assert!(!gate.has_jobs());
    }
    #[test]
    fn rejects_second_turn_for_same_chat_and_cross_session_mix() {
        let gate = Arc::new(GenerationQueue::default());
        let (_a, _) = gate.enqueue("s1", "A", "g1").unwrap();
        assert!(gate.enqueue("s1", "A", "g2").is_err());
        assert!(gate.enqueue("s2", "B", "g2").is_err());
        assert!(gate.cancel("wrong", "g1").is_err());
    }
    #[test]
    fn caps_pending_jobs_and_drop_recovers_capacity() {
        let gate = Arc::new(GenerationQueue::default());
        let mut tickets = Vec::new();
        for i in 0..8 {
            tickets.push(
                gate.enqueue("s", &format!("c{i}"), &format!("g{i}"))
                    .unwrap()
                    .0,
            );
        }
        assert!(gate.enqueue("s", "extra", "g8").is_err());
        drop(tickets.pop());
        assert!(gate.enqueue("s", "extra", "g8").is_ok());
    }
    #[test]
    fn cancel_just_before_start_is_still_observed() {
        let gate = Arc::new(GenerationQueue::default());
        let (a, _) = gate.enqueue("s", "c", "g").unwrap();
        a.wait_for_slot().unwrap();
        assert_eq!(gate.cancel("s", "g").unwrap(), CancelState::Active);
        assert!(a.cancelled());
    }
    #[test]
    fn plan07b_scoped_cancel_cannot_touch_unrelated_active_chat(){
        let queue=Arc::new(GenerationQueue::default());
        let (normal,_)=queue.enqueue("s","normal-user-chat","g-normal").unwrap();
        normal.wait_for_slot().unwrap();
        let (task,_)=queue.enqueue("s","lu-plan07b-host-suggestions","g-plan07b").unwrap();
        let hit=queue.cancel_for_chat("lu-plan07b-host-suggestions").unwrap().unwrap();
        assert_eq!(hit,("s".into(),"g-plan07b".into(),CancelState::Queued));
        assert!(task.wait_for_slot().is_err());
        assert!(!normal.cancelled());
        assert!(queue.cancel_for_chat("nonexistent").unwrap().is_none());
        drop(task);drop(normal);
        assert!(!queue.has_jobs());
    }
    #[test]
    fn human_turn_overtakes_queued_background_without_preemption() {
        let q = Arc::new(GenerationQueue::default());
        let (active, _) = q.enqueue("s", "human-active", "h1").unwrap();
        active.wait_for_slot().unwrap();
        let (background, _) = q.enqueue_with_class(
            "s", "auto", "b1", TurnClass::Background).unwrap();
        let (human, _) = q.enqueue("s", "human-new", "h2").unwrap();
        assert!(q.has_foreground_jobs());
        drop(active);
        human.wait_for_slot().unwrap(); // queued auto did not block a human send
        assert!(q.scoped_job_active("auto").unwrap() == Some(false));
        drop(human);
        background.wait_for_slot().unwrap();
        assert!(!q.has_foreground_jobs());
        drop(background);
        assert!(!q.has_jobs());
    }
    #[test]
    fn background_eventually_gets_one_slot_after_foreground_burst() {
        let q = Arc::new(GenerationQueue::default());
        for n in 0..MAX_FOREGROUND_BURST {
            let (ticket, _) = q.enqueue("s", &format!("chat-{n}"),
                &format!("human-{n}")).unwrap();
            ticket.wait_for_slot().unwrap();
            drop(ticket);
        }
        let (auto, _) = q.enqueue_with_class(
            "s", "background", "b", TurnClass::Background).unwrap();
        let (human, _) = q.enqueue("s", "later-human", "h").unwrap();
        auto.wait_for_slot().unwrap();
        drop(auto);
        human.wait_for_slot().unwrap();
        drop(human);
        assert!(!q.has_jobs());
    }
    #[test]
    fn background_queue_cannot_consume_all_human_capacity() {
        let q = Arc::new(GenerationQueue::default());
        let (_a, _) = q.enqueue_with_class("s","auto1","b1",
            TurnClass::Background).unwrap();
        let (_b, _) = q.enqueue_with_class("s","auto2","b2",
            TurnClass::Background).unwrap();
        assert!(q.enqueue_with_class("s","auto3","b3",
            TurnClass::Background).is_err());
        let mut foreground = Vec::new();
        for i in 0..6 {
            foreground.push(q.enqueue("s", &format!("human-{i}"),
                &format!("h{i}")).unwrap().0);
        }
        assert!(q.enqueue("s","human-extra","h-extra").is_err());
        assert_eq!(foreground.len(),6);
    }
    #[test]
    fn cancelled_foreground_waiter_does_not_block_background_lane() {
        let q = Arc::new(GenerationQueue::default());
        let (active, _) = q.enqueue("s", "human-active", "h1").unwrap();
        active.wait_for_slot().unwrap();
        let (cancelled, _) = q.enqueue("s", "human-cancel", "h2").unwrap();
        let (auto, _) = q.enqueue_with_class(
            "s", "background", "b", TurnClass::Background).unwrap();
        assert_eq!(q.cancel("s", "h2"), Ok(CancelState::Queued));
        assert!(cancelled.wait_for_slot().is_err());
        drop(active);
        assert!(!q.has_foreground_jobs());
        auto.wait_for_slot().unwrap();
        drop(cancelled);
        drop(auto);
        assert!(!q.has_jobs());
    }
    #[test]
    fn cancelling_queued_auto_does_not_affect_foreground() {
        let q = Arc::new(GenerationQueue::default());
        let (active, _) = q.enqueue("s", "human", "h").unwrap();
        active.wait_for_slot().unwrap();
        let (auto, _) = q.enqueue_with_class(
            "s", "auto", "b", TurnClass::Background).unwrap();
        assert_eq!(q.cancel("s", "b"), Ok(CancelState::Queued));
        assert!(auto.wait_for_slot().is_err());
        assert!(!active.cancelled());
        drop(auto);
        drop(active);
        assert!(!q.has_jobs());
    }
    #[test]
    fn plan07b_scoped_active_cancel_marks_ticket_before_model_storage(){
        let queue=Arc::new(GenerationQueue::default());
        let (task,_)=queue.enqueue("s","lu-plan07b-host-suggestions","g-plan07b").unwrap();
        task.wait_for_slot().unwrap();
        let hit=queue.cancel_for_chat("lu-plan07b-host-suggestions").unwrap().unwrap();
        assert_eq!(hit,("s".into(),"g-plan07b".into(),CancelState::Active));
        assert!(task.cancelled());
        drop(task);
        assert!(queue.cancel_for_chat("lu-plan07b-host-suggestions").unwrap().is_none());
    }

}
