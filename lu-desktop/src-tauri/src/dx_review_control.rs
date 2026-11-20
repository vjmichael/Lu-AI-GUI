//! CP-12B-R4: cancellation and final publication share one linearization gate.
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};

struct Active {
    stop: Arc<AtomicBool>,
    committed: bool,
}
#[derive(Default)]
pub(crate) struct ReviewControl {
    active: Mutex<Option<Active>>,
    closed: AtomicBool,
}
pub(crate) struct ReviewLease<'a> {
    control: &'a ReviewControl,
    stop: Arc<AtomicBool>,
}
impl ReviewControl {
    pub(crate) fn begin(&self) -> Result<ReviewLease<'_>, String> {
        let mut active = self.active.lock().map_err(|_| "Review unavailable")?;
        if self.closed.load(Ordering::Acquire) || active.is_some() {
            return Err("A review is already running or unavailable.".into());
        }
        let stop = Arc::new(AtomicBool::new(false));
        *active = Some(Active { stop: stop.clone(), committed: false });
        Ok(ReviewLease { control: self, stop })
    }
    /// True only if cancellation won before final publication. A completed
    /// review cannot be recalled; a published invitation can still be revoked.
    pub(crate) fn stop(&self) -> bool {
        self.active.lock().ok().and_then(|a| a.as_ref().map(|s| {
            !s.committed && !s.stop.swap(true, Ordering::AcqRel)
        })).unwrap_or(false)
    }
    pub(crate) fn shutdown(&self) {
        // Serialize the shutdown fence with begin/publication as well.
        if let Ok(active) = self.active.lock() {
            self.closed.store(true, Ordering::Release);
            if let Some(a) = active.as_ref() { a.stop.store(true, Ordering::Release); }
        } else {
            self.closed.store(true, Ordering::Release);
        }
    }
}
impl ReviewLease<'_> {
    pub(crate) fn stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire) || self.control.closed.load(Ordering::Acquire)
    }
    pub(crate) fn stop_signal(&self) -> &AtomicBool { &self.stop }
    pub(crate) fn publish<T>(&self, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let mut active = self.control.active.lock().map_err(|_| "Review unavailable")?;
        let a = active.as_mut().filter(|s| Arc::ptr_eq(&s.stop, &self.stop))
            .ok_or("Review unavailable")?;
        if self.stopped() || a.committed { return Err("Review stopped or already finished.".into()); }
        let result = f();
        if result.is_ok() { a.committed = true; }
        result
    }
}
impl Drop for ReviewLease<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.control.active.lock() {
            if active.as_ref().is_some_and(|s| Arc::ptr_eq(&s.stop, &self.stop)) { *active = None; }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cp12br3_cancellation_and_admission_are_fenced() {
        let c=ReviewControl::default(); let r=c.begin().unwrap();
        assert!(c.begin().is_err()); assert!(c.stop()); assert!(!c.stop());
        assert!(r.stop_signal().load(Ordering::Acquire));
        assert!(r.publish::<()>(||panic!("late publication")).is_err());
        drop(r); let r=c.begin().unwrap(); c.shutdown(); assert!(r.stopped());
        drop(r); assert!(c.begin().is_err());
    }
    #[test]
    fn cp12br4_publication_wins_then_stop_cannot_claim_success() {
        let c=ReviewControl::default(); let r=c.begin().unwrap();
        assert_eq!(r.publish(||Ok(42)).unwrap(),42); assert!(!c.stop());
        assert!(!r.stopped()); assert!(r.publish::<()>(||panic!("duplicate publication")).is_err());
        drop(r); let r=c.begin().unwrap(); assert!(c.stop()); assert!(r.stopped());
    }
    #[test]
    fn cp12br4_concurrent_stop_and_publication_have_one_winner() {
        for _ in 0..128 {
            let c=ReviewControl::default(); let r=c.begin().unwrap();
            let b=std::sync::Barrier::new(2);
            std::thread::scope(|s| {
                let stop=s.spawn(||{b.wait();c.stop()});
                b.wait(); let published=r.publish(||Ok(())).is_ok();
                assert_ne!(stop.join().unwrap(),published);
            });
        }
    }
    #[test]
    fn cp12br4_failed_publication_remains_cancellable() {
        let c=ReviewControl::default(); let r=c.begin().unwrap();
        assert!(r.publish::<()>(||Err("unavailable".into())).is_err());
        assert!(c.stop()); assert!(r.stopped());
    }
}
