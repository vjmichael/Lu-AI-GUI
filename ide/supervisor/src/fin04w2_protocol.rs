//! FIN-04W2-A: fixed-width review-only debugger broker message grammar.
//!
//! This module is deliberately neither an IPC server nor an execution API.
//! A parsed packet is untrusted data, NOT a creator grant, SCM identity,
//! service receipt, OS Job admission or authorization to launch a process.
//! An independent service will need to check all of those before dispatch.
use crate::{JobPlan, SupervisorError};

const MAGIC: &[u8; 8] = b"LUFW2A01";
const WIRE_SIZE: usize = 92;
const PLAN_START: usize = 12;
const TARGET_START: usize = 44;
const NONCE_START: usize = 76;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReviewedOperation {
    Initialize,
    LaunchSealed,
    Threads,
    StackTrace,
    Disconnect,
    Cancel,
}
impl ReviewedOperation {
    fn from_byte(value: u8) -> Result<Self, SupervisorError> {
        match value {
            1 => Ok(Self::Initialize),
            2 => Ok(Self::LaunchSealed),
            3 => Ok(Self::Threads),
            4 => Ok(Self::StackTrace),
            5 => Ok(Self::Disconnect),
            6 => Ok(Self::Cancel),
            _ => Err(SupervisorError::InvalidPlan),
        }
    }
}

/// Shape-only review. There are no path, PID, command-line, environment,
/// expression, attach, reverse request or user-selected method fields.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ReviewedPacket {
    pub(crate) operation: ReviewedOperation,
}

fn digest_bytes(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut result = [0u8; 32];
    for (i, place) in result.iter_mut().enumerate() {
        *place = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(result)
}

/// The caller-provided plan hash and challenge are only correlation values.
/// The future service must independently authenticate the client token,
/// installed host SHA, original journal, one-use consent and Job receipt.
/// This method never gives the caller a runnable service request.
pub(crate) fn inspect_shape_only(
    bytes: &[u8],
    sealed_plan: &JobPlan,
    expected_plan_hash: &str,
    expected_nonce: &[u8; 16],
) -> Result<ReviewedPacket, SupervisorError> {
    if bytes.len() != WIRE_SIZE
        || bytes.get(..8) != Some(MAGIC.as_slice())
        || bytes[8] != 1
        || bytes[10..12] != [0, 0]
        || !crate::validate_plan(sealed_plan)
        || sealed_plan.recipe_id != "dap_lldb_project"
        || sealed_plan.effect_class != "debug_inspection_isolated"
        || bytes[PLAN_START..TARGET_START]
            != digest_bytes(expected_plan_hash).ok_or(SupervisorError::InvalidPlan)?
        || bytes[TARGET_START..NONCE_START]
            != digest_bytes(
                sealed_plan.debug_target_binding_sha256.as_deref()
                    .ok_or(SupervisorError::InvalidPlan)?,
            ).ok_or(SupervisorError::InvalidPlan)?
        || bytes[NONCE_START..WIRE_SIZE] != expected_nonce[..]
    {
        return Err(SupervisorError::InvalidPlan);
    }
    Ok(ReviewedPacket {
        operation: ReviewedOperation::from_byte(bytes[9])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> JobPlan {
        JobPlan {
            project_id: format!("ide-{}", "a".repeat(32)),
            recipe_id: "dap_lldb_project".into(),
            source_head: "b".repeat(40),
            input_snapshot_sha256: "c".repeat(64),
            manifest_sha256: "d".repeat(64),
            toolchain_sha256: "e".repeat(64),
            debug_target_binding_sha256: Some("f".repeat(64)),
            runtime_limit_ms: 90_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 16,
            log_limit_bytes: 64 * 1024,
            effect_class: "debug_inspection_isolated".into(),
        }
    }
    fn packet(verb: u8) -> [u8; WIRE_SIZE] {
        let mut data = [0u8; WIRE_SIZE];
        data[..8].copy_from_slice(MAGIC);
        data[8] = 1;
        data[9] = verb;
        data[PLAN_START..TARGET_START].copy_from_slice(&[0xaa; 32]);
        data[TARGET_START..NONCE_START].copy_from_slice(&[0xff; 32]);
        data[NONCE_START..].copy_from_slice(&[0x42; 16]);
        data
    }
    #[test]
    fn exact_fixed_verbs_but_no_execution_authority() {
        for verb in 1..=6 {
            assert!(inspect_shape_only(&packet(verb), &plan(), &"aa".repeat(32), &[0x42;16]).is_ok());
        }
        for verb in [0, 7, 32, 255] {
            assert!(inspect_shape_only(&packet(verb), &plan(), &"aa".repeat(32), &[0x42;16]).is_err());
        }
    }
    #[test]
    fn reject_wrong_plan_stale_nonce_extra_bytes_and_unsealed_target() {
        let original = packet(2);
        assert!(inspect_shape_only(&original, &plan(), &"bb".repeat(32), &[0x42;16]).is_err());
        assert!(inspect_shape_only(&original, &plan(), &"aa".repeat(32), &[0x41;16]).is_err());
        let mut extra = original.to_vec();
        extra.push(0);
        assert!(inspect_shape_only(&extra, &plan(), &"aa".repeat(32), &[0x42;16]).is_err());
        let mut changed = original;
        changed[10] = 1;
        assert!(inspect_shape_only(&changed, &plan(), &"aa".repeat(32), &[0x42;16]).is_err());
        changed = original;
        changed[0] ^= 1; // fixed magic
        assert!(inspect_shape_only(&changed, &plan(), &"aa".repeat(32), &[0x42;16]).is_err());
        changed = original;
        changed[8] = 2; // wrong version
        assert!(inspect_shape_only(&changed, &plan(), &"aa".repeat(32), &[0x42;16]).is_err());
        changed = original;
        changed[TARGET_START] ^= 1; // target changed
        assert!(inspect_shape_only(&changed, &plan(), &"aa".repeat(32), &[0x42;16]).is_err());
        let mut wrong = plan();
        wrong.debug_target_binding_sha256 = None;
        assert!(inspect_shape_only(&original, &wrong, &"aa".repeat(32), &[0x42;16]).is_err());
        wrong = plan();
        wrong.process_limit = 48;
        #[cfg(windows)]
        assert!(inspect_shape_only(&original, &wrong, &"aa".repeat(32), &[0x42;16]).is_err());
    }
}
