//! Bounded, host-owned FIFO scheduler for one physical model session.
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

#[derive(Default)]
struct State {
    waiting: VecDeque<String>,
    active: Option<String>,
    jobs: HashMap<String, Job>,
}
struct Job {
    chat_id: String,
    session_id: String,
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
        let mut state = self.state.lock().map_err(|_| "generation queue poisoned")?;
        if state.jobs.len() >= MAX_JOBS {
            return Err("generation queue full (8 turns maximum)".into());
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
                cancelled: false,
            },
        );
        state.waiting.push_back(generation_id.into());
        let position = state.waiting.len() + usize::from(state.active.is_some());
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
            state.waiting.retain(|x| x != generation_id);
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
            if state.active.is_none() && state.waiting.front() == Some(&self.id) {
                state.waiting.pop_front();
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
