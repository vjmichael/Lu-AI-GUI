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

const MAX_JOBS: usize = 8;
// Leave at least six queue places for explicit human requests.
const MAX_BACKGROUND_JOBS: usize = 2;
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
        for i in 0..6 {
            assert!(q.enqueue("s", &format!("human-{i}"),
                &format!("h{i}")).is_ok());
            // Here the ticket drops at end of statement, so this proves
            // foreground admission rather than simultaneous occupancy.
        }
        assert!(q.enqueue("s","human","h").is_ok());
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
