//! Generated Protobuf and semantic validation for signed Core Extension tools.
//! A descriptor is host-owned, but a publisher does not get to invent an opaque
//! payload type. Until a separately audited host schema registry is available,
//! only the six generated effect-specific messages below are executable.
use prost::Message;

use crate::{
    ce_broker::MAX_BROKER_FILE_BYTES,
    contracts::{ToolDescriptor, ToolEffect},
    wire::proto,
};

const PREFIX: &str = "type.googleapis.com/lu.v1.";
const MAX_PAYLOAD: usize = MAX_BROKER_FILE_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeSchemaError {
    UnsupportedType,
    EffectMismatch,
    InvalidPayload,
    InvalidPath,
    InvalidOperation,
    InvalidResult,
    Oversized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeToolSchema {
    Read,
    Write,
    Execute,
}

fn relative_path(path: &str) -> Result<(), CeSchemaError> {
    if path.is_empty()
        || path.len() > 512
        || path.chars().any(|c| matches!(c, '\\' | ':' | '\0'))
        || path.starts_with('/')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(CeSchemaError::InvalidPath);
    }
    Ok(())
}

fn decode_exact<M: Message + Default>(data: &[u8]) -> Result<M, CeSchemaError> {
    if data.len() > MAX_PAYLOAD {
        return Err(CeSchemaError::Oversized);
    }
    let decoded = M::decode(data).map_err(|_| CeSchemaError::InvalidPayload)?;
    if decoded.encode_to_vec() != data {
        return Err(CeSchemaError::InvalidPayload);
    }
    Ok(decoded)
}

impl CeToolSchema {
    /// Fail closed for missing, unregistered or mismatched generated types.
    pub fn for_descriptor(descriptor: &ToolDescriptor) -> Result<Self, CeSchemaError> {
        let pair = (
            descriptor.input_type_url.strip_prefix(PREFIX),
            descriptor.output_type_url.strip_prefix(PREFIX),
            descriptor.effect,
        );
        match pair {
            (Some("CeReadRequest"), Some("CeReadResult"), ToolEffect::ReadWorkspace) => {
                Ok(Self::Read)
            }
            (Some("CeWriteRequest"), Some("CeWriteResult"), ToolEffect::WriteWorkspace) => {
                Ok(Self::Write)
            }
            (Some("CeExecuteRequest"), Some("CeExecuteResult"), ToolEffect::ExecuteCode) => {
                Ok(Self::Execute)
            }
            (Some("CeReadRequest" | "CeWriteRequest" | "CeExecuteRequest"), _, _) => {
                Err(CeSchemaError::EffectMismatch)
            }
            _ => Err(CeSchemaError::UnsupportedType),
        }
    }

    pub fn validate_input(
        self,
        descriptor: &ToolDescriptor,
        bytes: &[u8],
    ) -> Result<(), CeSchemaError> {
        if Self::for_descriptor(descriptor)? != self {
            return Err(CeSchemaError::EffectMismatch);
        }
        match self {
            Self::Read => {
                let msg = decode_exact::<proto::CeReadRequest>(bytes)?;
                relative_path(&msg.relative_path)?;
                if msg.max_bytes == 0 || msg.max_bytes as usize > MAX_PAYLOAD {
                    return Err(CeSchemaError::Oversized);
                }
            }
            Self::Write => {
                let msg = decode_exact::<proto::CeWriteRequest>(bytes)?;
                relative_path(&msg.relative_path)?;
                if msg.data.is_empty() || msg.data.len() > MAX_PAYLOAD {
                    return Err(CeSchemaError::Oversized);
                }
            }
            Self::Execute => {
                let msg = decode_exact::<proto::CeExecuteRequest>(bytes)?;
                if msg.operation != descriptor.id || msg.arguments.len() > 16 {
                    return Err(CeSchemaError::InvalidOperation);
                }
                let mut seen = std::collections::BTreeSet::new();
                for argument in &msg.arguments {
                    if argument.name.is_empty()
                        || argument.name.len() > 64
                        || !argument
                            .name
                            .bytes()
                            .all(|c| c.is_ascii_lowercase() || c == b'_')
                        || !seen.insert(argument.name.as_str())
                        || argument.value.len() > 1024
                        || argument.value.contains('\0')
                    {
                        return Err(CeSchemaError::InvalidOperation);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn validate_output(
        self,
        descriptor: &ToolDescriptor,
        input: &[u8],
        output: &[u8],
    ) -> Result<(), CeSchemaError> {
        self.validate_input(descriptor, input)?;
        match self {
            Self::Read => {
                let request = decode_exact::<proto::CeReadRequest>(input)?;
                let result = decode_exact::<proto::CeReadResult>(output)?;
                if result.bytes_read != result.data.len() as u64
                    || result.data.len() > request.max_bytes as usize
                    || (result.truncated && result.data.len() != request.max_bytes as usize)
                {
                    return Err(CeSchemaError::InvalidResult);
                }
            }
            Self::Write => {
                let request = decode_exact::<proto::CeWriteRequest>(input)?;
                let result = decode_exact::<proto::CeWriteResult>(output)?;
                if (result.committed && result.bytes_written != request.data.len() as u64)
                    || (!result.committed && result.bytes_written != 0)
                {
                    return Err(CeSchemaError::InvalidResult);
                }
            }
            Self::Execute => {
                let result = decode_exact::<proto::CeExecuteResult>(output)?;
                if !matches!(result.status.as_str(), "ok" | "failed")
                    || (result.status == "failed" && !result.output.is_empty())
                {
                    return Err(CeSchemaError::InvalidResult);
                }
            }
        }
        Ok(())
    }

    /// Worker-originated broker subrequests are reconstructed as generated
    /// request messages and validated before the capability broker is called.
    /// The worker cannot supply authority by selecting a tool ID.
    pub fn validate_broker_read(
        descriptor: &ToolDescriptor,
        path: &str,
    ) -> Result<Vec<u8>, CeSchemaError> {
        let schema = Self::for_descriptor(descriptor)?;
        if schema != Self::Read {
            return Err(CeSchemaError::EffectMismatch);
        }
        let bytes = proto::CeReadRequest {
            relative_path: path.to_owned(),
            max_bytes: MAX_PAYLOAD as u32,
        }
        .encode_to_vec();
        schema.validate_input(descriptor, &bytes)?;
        Ok(bytes)
    }

    pub fn validate_broker_write(
        descriptor: &ToolDescriptor,
        path: &str,
        data: &[u8],
    ) -> Result<Vec<u8>, CeSchemaError> {
        let schema = Self::for_descriptor(descriptor)?;
        if schema != Self::Write {
            return Err(CeSchemaError::EffectMismatch);
        }
        let bytes = proto::CeWriteRequest {
            relative_path: path.to_owned(),
            data: data.to_vec(),
        }
        .encode_to_vec();
        schema.validate_input(descriptor, &bytes)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn descriptor(kind: CeToolSchema) -> ToolDescriptor {
        let (i, o, effect) = match kind {
            CeToolSchema::Read => ("CeReadRequest", "CeReadResult", ToolEffect::ReadWorkspace),
            CeToolSchema::Write => (
                "CeWriteRequest",
                "CeWriteResult",
                ToolEffect::WriteWorkspace,
            ),
            CeToolSchema::Execute => (
                "CeExecuteRequest",
                "CeExecuteResult",
                ToolEffect::ExecuteCode,
            ),
        };
        ToolDescriptor {
            id: "publisher.worker.run".into(),
            display_name: "test".into(),
            effect,
            input_type_url: format!("{PREFIX}{i}"),
            output_type_url: format!("{PREFIX}{o}"),
        }
    }
    #[test]
    fn generated_contracts_enforce_every_input_and_output_type() {
        let rd = descriptor(CeToolSchema::Read);
        let read = proto::CeReadRequest {
            relative_path: "inside.txt".into(),
            max_bytes: 64,
        }
        .encode_to_vec();
        let good = proto::CeReadResult {
            data: b"safe".to_vec(),
            bytes_read: 4,
            truncated: false,
        }
        .encode_to_vec();
        assert_eq!(CeToolSchema::for_descriptor(&rd), Ok(CeToolSchema::Read));
        CeToolSchema::Read
            .validate_output(&rd, &read, &good)
            .unwrap();
        let bad = proto::CeReadResult {
            data: b"safe".to_vec(),
            bytes_read: 300,
            truncated: false,
        }
        .encode_to_vec();
        assert_eq!(
            CeToolSchema::Read.validate_output(&rd, &read, &bad),
            Err(CeSchemaError::InvalidResult)
        );
        let unsafe_path = proto::CeReadRequest {
            relative_path: "../secret".into(),
            max_bytes: 64,
        }
        .encode_to_vec();
        assert_eq!(
            CeToolSchema::Read.validate_input(&rd, &unsafe_path),
            Err(CeSchemaError::InvalidPath)
        );

        let wr = descriptor(CeToolSchema::Write);
        let write = proto::CeWriteRequest {
            relative_path: "out.txt".into(),
            data: b"safe".to_vec(),
        }
        .encode_to_vec();
        let good_write = proto::CeWriteResult {
            bytes_written: 4,
            committed: true,
        }
        .encode_to_vec();
        CeToolSchema::Write
            .validate_output(&wr, &write, &good_write)
            .unwrap();
        let false_success = proto::CeWriteResult {
            bytes_written: 400,
            committed: true,
        }
        .encode_to_vec();
        assert_eq!(
            CeToolSchema::Write.validate_output(&wr, &write, &false_success),
            Err(CeSchemaError::InvalidResult)
        );

        let ex = descriptor(CeToolSchema::Execute);
        let execute = proto::CeExecuteRequest {
            operation: ex.id.clone(),
            arguments: vec![],
        }
        .encode_to_vec();
        let good_result = proto::CeExecuteResult {
            status: "ok".into(),
            output: b"safe".to_vec(),
        }
        .encode_to_vec();
        CeToolSchema::Execute
            .validate_output(&ex, &execute, &good_result)
            .unwrap();
        let wrong_op = proto::CeExecuteRequest {
            operation: "shell".into(),
            arguments: vec![],
        }
        .encode_to_vec();
        assert_eq!(
            CeToolSchema::Execute.validate_input(&ex, &wrong_op),
            Err(CeSchemaError::InvalidOperation)
        );
        let bad_result = proto::CeExecuteResult {
            status: "ok-ish".into(),
            output: vec![],
        }
        .encode_to_vec();
        assert_eq!(
            CeToolSchema::Execute.validate_output(&ex, &execute, &bad_result),
            Err(CeSchemaError::InvalidResult)
        );
    }
    #[test]
    fn unknown_and_noncanonical_types_are_not_executable() {
        let mut desc = descriptor(CeToolSchema::Execute);
        desc.output_type_url = "type.googleapis.com/acme.DynamicOutput".into();
        assert!(CeToolSchema::for_descriptor(&desc).is_err());
        let desc = descriptor(CeToolSchema::Execute);
        let mut canonical = proto::CeExecuteRequest {
            operation: desc.id.clone(),
            arguments: vec![],
        }
        .encode_to_vec();
        canonical.extend_from_slice(&[0x78, 0x01]);
        assert_eq!(
            CeToolSchema::Execute.validate_input(&desc, &canonical),
            Err(CeSchemaError::InvalidPayload)
        );
        assert_eq!(
            CeToolSchema::Execute.validate_input(&desc, &vec![0; MAX_PAYLOAD + 1]),
            Err(CeSchemaError::Oversized)
        );
    }

    #[test]
    fn adversarial_protobuf_wire_forms_are_not_canonical() {
        let read = descriptor(CeToolSchema::Read);
        let canonical = proto::CeReadRequest {
            relative_path: "inside.txt".into(),
            max_bytes: 64,
        }
        .encode_to_vec();
        let mut unknown_field = canonical.clone();
        unknown_field.extend_from_slice(&[0x78, 0x01]);
        let mut duplicate_scalar = canonical.clone();
        duplicate_scalar.extend_from_slice(&[0x10, 0x01]);
        let mut malformed_length = canonical.clone();
        malformed_length.extend_from_slice(&[0x0a, 0x7f]);
        for malicious in [unknown_field, duplicate_scalar, malformed_length] {
            assert_eq!(
                CeToolSchema::Read.validate_input(&read, &malicious),
                Err(CeSchemaError::InvalidPayload)
            );
        }
        let result = proto::CeReadResult {
            data: b"safe".to_vec(),
            bytes_read: 4,
            truncated: false,
        }
        .encode_to_vec();
        let mut unknown_result = result;
        unknown_result.extend_from_slice(&[0x78, 0x01]);
        assert_eq!(
            CeToolSchema::Read.validate_output(&read, &canonical, &unknown_result),
            Err(CeSchemaError::InvalidPayload)
        );
    }

    #[test]
    fn adversarial_generated_message_semantics_fail_closed() {
        let read = descriptor(CeToolSchema::Read);
        for path in [
            "../secret",
            "a/../secret",
            "a//b",
            "/absolute",
            "C:/drive",
            "a\\\\b",
            "a/./b",
            "a/..",
            "",
        ] {
            let input = proto::CeReadRequest {
                relative_path: path.into(),
                max_bytes: 64,
            }
            .encode_to_vec();
            assert_eq!(
                CeToolSchema::Read.validate_input(&read, &input),
                Err(CeSchemaError::InvalidPath),
                "unexpected accepted path: {path:?}"
            );
        }
        for max_bytes in [0, 65_537] {
            let input = proto::CeReadRequest {
                relative_path: "inside".into(),
                max_bytes,
            }
            .encode_to_vec();
            assert_eq!(
                CeToolSchema::Read.validate_input(&read, &input),
                Err(CeSchemaError::Oversized)
            );
        }
        let request = proto::CeReadRequest {
            relative_path: "inside".into(),
            max_bytes: 2,
        }
        .encode_to_vec();
        for result in [
            proto::CeReadResult {
                data: b"abc".to_vec(),
                bytes_read: 3,
                truncated: false,
            },
            proto::CeReadResult {
                data: b"a".to_vec(),
                bytes_read: 1,
                truncated: true,
            },
            proto::CeReadResult {
                data: b"ab".to_vec(),
                bytes_read: 1,
                truncated: false,
            },
        ] {
            assert_eq!(
                CeToolSchema::Read.validate_output(&read, &request, &result.encode_to_vec()),
                Err(CeSchemaError::InvalidResult)
            );
        }

        let write = descriptor(CeToolSchema::Write);
        let write_request = proto::CeWriteRequest {
            relative_path: "inside".into(),
            data: b"abc".to_vec(),
        }
        .encode_to_vec();
        for result in [
            proto::CeWriteResult {
                committed: false,
                bytes_written: 1,
            },
            proto::CeWriteResult {
                committed: true,
                bytes_written: 2,
            },
        ] {
            assert_eq!(
                CeToolSchema::Write.validate_output(
                    &write,
                    &write_request,
                    &result.encode_to_vec()
                ),
                Err(CeSchemaError::InvalidResult)
            );
        }

        let execute = descriptor(CeToolSchema::Execute);
        for arguments in [
            vec![
                proto::CeExecuteArgument {
                    name: "arg".into(),
                    value: "one".into(),
                },
                proto::CeExecuteArgument {
                    name: "arg".into(),
                    value: "two".into(),
                },
            ],
            vec![proto::CeExecuteArgument {
                name: "UPPER".into(),
                value: "one".into(),
            }],
            vec![proto::CeExecuteArgument {
                name: "arg".into(),
                value: "nul\0byte".into(),
            }],
        ] {
            let request = proto::CeExecuteRequest {
                operation: execute.id.clone(),
                arguments,
            }
            .encode_to_vec();
            assert_eq!(
                CeToolSchema::Execute.validate_input(&execute, &request),
                Err(CeSchemaError::InvalidOperation)
            );
        }
        let request = proto::CeExecuteRequest {
            operation: execute.id.clone(),
            arguments: vec![],
        }
        .encode_to_vec();
        let false_result = proto::CeExecuteResult {
            status: "failed".into(),
            output: b"secret".to_vec(),
        }
        .encode_to_vec();
        assert_eq!(
            CeToolSchema::Execute.validate_output(&execute, &request, &false_result),
            Err(CeSchemaError::InvalidResult)
        );
    }
}
