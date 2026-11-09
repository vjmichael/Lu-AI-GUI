//! DEV-04.5D5 — fixed-width binary owner-event sideband from reviewed LLDB.
//! No DAP JSON, model, file path, user PID, generic process launcher or
//! debugger command can assert these events. Only the protected Windows
//! worker's private, one-use inherited pipe and fresh host nonce can do so.
//! The owner record is source evidence; it is NOT a new execution approval.
#![cfg(all(windows, feature = "dev045d5-ci-event-owner", debug_assertions))]
use crate::SupervisorError;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};
const SIZE: usize = 76;
const MAX_SEQ: u64 = 4096;
const MAGIC: &[u8; 8] = b"LUOE0001";
const RECEIVED: u32 = 1;
const CONTINUED: u32 = 2;
fn denied() -> SupervisorError { SupervisorError::NoContainment }
fn le_u32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn le_u64(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OwnerEvent {
    pub seq: u64,
    pub kind: u32,
    pub pid: u32,
    pub tid: u32,
    pub owner_tid: u32,
    pub exception_code: u32,
    pub first_chance: u32,
    pub rip: u64,
    pub exception_address: u64,
    pub continue_status: u32,
}
pub(crate) fn decode(
    bytes: &[u8], original_nonce: &[u8; 16],
) -> Result<OwnerEvent, SupervisorError> {
    if bytes.len() != SIZE || &bytes[0..8] != MAGIC
        || &bytes[8..24] != original_nonce
    { return Err(denied()); }
    let event = OwnerEvent {
        seq: le_u64(bytes, 24),
        kind: le_u32(bytes, 32),
        pid: le_u32(bytes, 36),
        tid: le_u32(bytes, 40),
        owner_tid: le_u32(bytes, 44),
        exception_code: le_u32(bytes, 48),
        first_chance: le_u32(bytes, 52),
        rip: le_u64(bytes, 56),
        exception_address: le_u64(bytes, 64),
        continue_status: le_u32(bytes, 72),
    };
    if event.seq == 0 || event.seq > MAX_SEQ
        || !matches!(event.kind, RECEIVED | CONTINUED)
        || event.pid == 0 || event.tid == 0 || event.owner_tid == 0
        || event.rip == 0 || event.exception_address == 0
        || event.first_chance > 1
        || (event.kind == RECEIVED && event.continue_status != 0)
        || (event.kind == CONTINUED
            && !matches!(event.continue_status, 1 | 2))
    { return Err(denied()); }
    Ok(event)
}
#[derive(Default)]
pub(crate) struct OwnerEventLedger {
    next_sequence: u64,
    active: Option<OwnerEvent>,
    revoked: bool,
}
impl OwnerEventLedger {
    pub(crate) fn observe(&mut self, event: OwnerEvent)
        -> Result<(), SupervisorError>
    {
        match event.kind {
            RECEIVED => {
                if self.active.is_some() || event.seq != self.next_sequence + 1
                    || event.continue_status != 0 { return Err(denied()); }
                self.next_sequence = event.seq;
                self.active = Some(event);
                self.revoked = false;
            }
            CONTINUED => {
                let old = self.active.as_ref().ok_or_else(denied)?;
                if old.seq != event.seq || old.pid != event.pid
                    || old.tid != event.tid || old.owner_tid != event.owner_tid
                    || old.exception_code != event.exception_code
                    || old.first_chance != event.first_chance
                    || old.rip != event.rip
                    || old.exception_address != event.exception_address
                { return Err(denied()); }
                self.active = None;
                self.revoked = true;
            }
            _ => return Err(denied()),
        }
        Ok(())
    }
    pub(crate) fn matching_stop(&self, pid: u32, tid: u32, rip: u64)
        -> Result<OwnerEvent, SupervisorError>
    {
        let event = self.active.as_ref().ok_or_else(denied)?;
        if self.revoked || event.kind != RECEIVED || event.pid != pid
            || event.tid != tid || event.rip != rip { return Err(denied()); }
        Ok(event.clone())
    }
}

/// The supervisor supplies an owned, private, non-reparse directory derived
/// from its trusted journal root, NOT a user project path. The fsync+reopen
/// record binds a one-use OS owner event to the native Running receipt.
/// The event source's authenticity was already proven via kernel handle list,
/// compiled LLDB pin, nonce, owner thread PID and Job.
pub(crate) fn persist_native_owner_receipt(
    root: &Path, job_id: &str, plan_sha256: &str,
    running_receipt_sha256: &str, event: &OwnerEvent,
) -> Result<String, SupervisorError> {
    use sha2::{Digest, Sha256};
    if !root.is_absolute()
        || !job_id.starts_with("job-") || job_id.len() != 36
        || !job_id[4..].bytes().all(|b| b.is_ascii_hexdigit())
        || plan_sha256.len() != 64 || running_receipt_sha256.len() != 64
        || !plan_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || !running_receipt_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || event.kind != RECEIVED { return Err(denied()); }
    let directory = fs::symlink_metadata(root).map_err(|_| denied())?;
    if !directory.is_dir() || directory.file_type().is_symlink() {
        return Err(denied());
    }
    let filename = format!("lu-owner-{job_id}-{:016x}.receipt", event.seq);
    let file = root.join(filename);
    let body = format!(
        "lu-owner-v1\n{job_id}\n{plan_sha256}\n{running_receipt_sha256}\n\
         {}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
        event.seq, event.pid, event.tid, event.owner_tid,
        event.exception_code, event.first_chance,
        event.rip, event.exception_address
    );
    if body.len() > 1024 { return Err(denied()); }
    let hash = format!("{:x}", Sha256::digest(body.as_bytes()));
    let mut f = OpenOptions::new().write(true).create_new(true)
        .open(&file).map_err(|_| denied())?;
    f.write_all(body.as_bytes()).map_err(|_| denied())?;
    f.sync_all().map_err(|_| denied())?;
    drop(f);
    let verify = fs::read(&file).map_err(|_| denied())?;
    if verify != body.as_bytes() { return Err(denied()); }
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(seq: u64, kind: u32) -> OwnerEvent {
        OwnerEvent {
            seq, kind, pid: 55, tid: 75, owner_tid: 90,
            exception_code: 0x80000003, first_chance: 1,
            rip: 0x140001123, exception_address: 0x140001122,
            continue_status: if kind == CONTINUED { 1 } else { 0 },
        }
    }
    fn bytes(e: &OwnerEvent, nonce: [u8; 16]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&e.seq.to_le_bytes());
        for value in [e.kind, e.pid, e.tid, e.owner_tid,
                      e.exception_code, e.first_chance] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&e.rip.to_le_bytes());
        out.extend_from_slice(&e.exception_address.to_le_bytes());
        out.extend_from_slice(&e.continue_status.to_le_bytes());
        assert_eq!(out.len(), SIZE);
        out
    }
    #[test]
    fn genuine_received_then_continue_revokes_exact_owner_stop() {
        let nonce = [0x5a; 16];
        let received = decode(&bytes(&record(1, RECEIVED), nonce), &nonce).unwrap();
        let mut ledger = OwnerEventLedger::default();
        ledger.observe(received).unwrap();
        assert_eq!(ledger.matching_stop(55, 75, 0x140001123).unwrap().seq, 1);
        assert!(ledger.matching_stop(55, 75, 0x140001124).is_err());
        ledger.observe(record(1, CONTINUED)).unwrap();
        assert!(ledger.matching_stop(55, 75, 0x140001123).is_err());
        assert!(ledger.observe(record(1, CONTINUED)).is_err());
        ledger.observe(record(2, RECEIVED)).unwrap();
        assert_eq!(ledger.matching_stop(55, 75, 0x140001123).unwrap().seq, 2);
    }
    #[test]
    fn forged_nonce_bad_sequence_and_cross_thread_rejected() {
        let nonce = [0x01; 16];
        assert!(decode(&bytes(&record(1, RECEIVED), nonce), &[0x02; 16]).is_err());
        let mut ledger = OwnerEventLedger::default();
        assert!(ledger.observe(record(2, RECEIVED)).is_err());
        ledger.observe(record(1, RECEIVED)).unwrap();
        let mut wrong = record(1, CONTINUED);
        wrong.tid = 100;
        assert!(ledger.observe(wrong).is_err());
        assert!(ledger.matching_stop(55, 100, 0x140001123).is_err());
    }
    #[test]
    fn rejects_truncation_unknown_kind_and_status() {
        let nonce = [1u8; 16];
        let mut buffer = bytes(&record(1, RECEIVED), nonce);
        assert!(decode(&buffer[..74], &nonce).is_err());
        buffer[32..36].copy_from_slice(&3u32.to_le_bytes());
        assert!(decode(&buffer, &nonce).is_err());
        buffer[32..36].copy_from_slice(&1u32.to_le_bytes());
        buffer[72..76].copy_from_slice(&1u32.to_le_bytes());
        assert!(decode(&buffer, &nonce).is_err());
    }
}
