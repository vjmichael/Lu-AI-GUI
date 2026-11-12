//! PERF-01 P3 — bounded, low-latency model-to-WebView event coalescer.
//!
//! This is volatile transport only. Original text is checkpointed in ANSF
//! separately. No model action, authority or persistent replay is inferred.
use std::time::{Duration, Instant};

pub const MAX_STREAM_BYTES:usize=1024*1024;
pub const MAX_EVENT_BYTES:usize=64*1024;
pub const COALESCE_BYTES:usize=2048;
pub const COALESCE_DELAY:Duration=Duration::from_millis(40);

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct DeliveryChunk { pub index:u32,pub text:String }

pub struct BoundedStreamDelivery {
    pending:String,
    next_index:u32,
    total_bytes:usize,
    first_forwarded:bool,
    last_flush:Instant,
}
impl Default for BoundedStreamDelivery {
    fn default()->Self{
        Self{pending:String::new(),next_index:0,total_bytes:0,
            first_forwarded:false,last_flush:Instant::now()}
    }
}
impl BoundedStreamDelivery {
    /// Return 0–2 bounded chunks. Forward the first token immediately,
    /// coalesce subsequent small tokens up to a byte/time threshold, and
    /// refuse unbounded backend frames before any user-visible publication.
    pub fn accept(&mut self,text:&str)->Result<Vec<DeliveryChunk>,&'static str>{
        if text.is_empty(){return Ok(Vec::new())}
        if text.len()>MAX_EVENT_BYTES||text.contains('\0'){
            return Err("untrusted oversized or invalid model text frame")
        }
        let next_total=self.total_bytes.checked_add(text.len())
            .ok_or("stream byte counter overflow")?;
        if next_total>MAX_STREAM_BYTES{return Err("stream exceeded 1 MiB cap")}
        let mut ready=Vec::with_capacity(2);
        if self.pending.len().saturating_add(text.len())>MAX_EVENT_BYTES {
            if let Some(chunk)=self.flush(){ready.push(chunk);}
        }
        self.total_bytes=next_total;
        self.pending.push_str(text);
        if !self.first_forwarded||self.pending.len()>=COALESCE_BYTES||
            self.last_flush.elapsed()>=COALESCE_DELAY {
            if let Some(chunk)=self.flush(){ready.push(chunk);}
        }
        Ok(ready)
    }
    /// Flush even a subthreshold tail at completion/failure/cancellation.
    /// The caller must still perform exact-session and cancellation checks.
    pub fn flush(&mut self)->Option<DeliveryChunk>{
        if self.pending.is_empty(){return None}
        if self.next_index==u32::MAX{return None}
        let text=std::mem::take(&mut self.pending);
        let index=self.next_index;
        self.next_index+=1;
        self.first_forwarded=true;
        self.last_flush=Instant::now();
        Some(DeliveryChunk{index,text})
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn first_chunk_is_not_delayed_and_small_followups_coalesce(){
        let mut stream=BoundedStreamDelivery::default();
        assert_eq!(stream.accept("First").unwrap(),
            vec![DeliveryChunk{index:0,text:"First".into()}]);
        assert!(stream.accept(" tiny").unwrap().is_empty());
        assert!(stream.accept(" pieces").unwrap().is_empty());
        assert_eq!(stream.flush(),
            Some(DeliveryChunk{index:1,text:" tiny pieces".into()}));
        assert!(stream.flush().is_none());
    }
    #[test]
    fn utf8_and_threshold_preserve_order_and_bound_each_event(){
        let mut stream=BoundedStreamDelivery::default();
        let mut output=stream.accept("✓").unwrap();
        output.extend(stream.accept(&"x".repeat(COALESCE_BYTES)).unwrap());
        output.extend(stream.accept("end").unwrap());
        if let Some(end)=stream.flush(){output.push(end)}
        assert_eq!(output.iter().map(|p|p.text.as_str()).collect::<String>(),
            format!("✓{}end","x".repeat(COALESCE_BYTES)));
        assert_eq!(output.iter().map(|p|p.index).collect::<Vec<_>>(),
            (0..output.len() as u32).collect::<Vec<_>>());
        assert!(output.iter().all(|p|p.text.len()<=MAX_EVENT_BYTES));
    }
    #[test]
    fn rejects_oversize_and_never_mutates_prior_stream(){
        let mut stream=BoundedStreamDelivery::default();
        assert_eq!(stream.accept("original").unwrap()[0].text,"original");
        assert!(stream.accept(&"x".repeat(MAX_EVENT_BYTES+1)).is_err());
        assert!(stream.accept("\0").is_err());
        assert!(stream.accept(&"x".repeat(MAX_STREAM_BYTES)).is_err());
        assert_eq!(stream.accept("safe").unwrap().len(),0);
        assert_eq!(stream.flush().unwrap().text,"safe");
    }
    #[test]
    fn buffered_tail_is_split_before_a_large_incoming_frame(){
        let mut stream=BoundedStreamDelivery::default();
        stream.accept("first").unwrap();
        stream.accept("tail").unwrap();
        let delivered=stream.accept(&"z".repeat(MAX_EVENT_BYTES)).unwrap();
        assert_eq!(delivered.len(),2);
        assert_eq!(delivered[0],DeliveryChunk{index:1,text:"tail".into()});
        assert_eq!(delivered[1].index,2);
        assert_eq!(delivered[1].text.len(),MAX_EVENT_BYTES);
    }
}
