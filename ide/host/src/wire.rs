//! Strict, bounded, path-free Protobuf UI/host lifecycle envelope.
//! Executable operations and authorization tokens are *not* transported here.
use prost::Message;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/lu.ide.v1.rs"));
}

pub const VERSION: u32 = 1;
pub const MAX_REQUEST_BYTES: usize = 16 * 1024;
pub const MAX_REPLY_BYTES: usize = 128 * 1024;
pub const MAX_PROJECTS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    Oversized,
    Noncanonical,
    Invalid(&'static str),
    Decode,
}

fn valid_correlation(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn valid_project_id(id: &str) -> bool {
    let Some(suffix) = id.strip_prefix("ide-") else {
        return false;
    };
    suffix.len() == 32
        && suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_request(msg: &proto::WorkspaceRequest) -> Result<(), WireError> {
    use proto::workspace_request::Operation;
    if msg.protocol_version != VERSION {
        return Err(WireError::Invalid("protocol version"));
    }
    if !valid_correlation(&msg.request_id) {
        return Err(WireError::Invalid("request ID"));
    }
    match msg
        .operation
        .as_ref()
        .ok_or(WireError::Invalid("operation"))?
    {
        Operation::List(_) | Operation::Select(_) => {}
        Operation::Reopen(op) if valid_project_id(&op.project_id) => {}
        Operation::Close(op) if valid_project_id(&op.project_id) => {}
        _ => return Err(WireError::Invalid("project ID")),
    }
    Ok(())
}

fn validate_project(view: &proto::ProjectView) -> Result<(), WireError> {
    if !valid_project_id(&view.project_id)
        || view.display_name.is_empty()
        || view.display_name.len() > 255
        || view.display_name.chars().any(char::is_control)
    {
        return Err(WireError::Invalid("project view"));
    }
    Ok(())
}

fn validate_reply(msg: &proto::WorkspaceReply) -> Result<(), WireError> {
    use proto::workspace_reply::Outcome;
    if msg.protocol_version != VERSION || !valid_correlation(&msg.request_id) {
        return Err(WireError::Invalid("reply header"));
    }
    match msg.outcome.as_ref().ok_or(WireError::Invalid("outcome"))? {
        Outcome::Project(view) => validate_project(view)?,
        Outcome::Projects(list) => {
            if list.projects.len() > MAX_PROJECTS {
                return Err(WireError::Oversized);
            }
            for project in &list.projects {
                validate_project(project)?;
            }
        }
        Outcome::Failure(err) => {
            if err.code.is_empty()
                || err.code.len() > 64
                || !err
                    .code
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b == b'_')
            {
                return Err(WireError::Invalid("failure code"));
            }
        }
    }
    Ok(())
}

pub fn encode_request(request: &proto::WorkspaceRequest) -> Result<Vec<u8>, WireError> {
    validate_request(request)?;
    if request.encoded_len() > MAX_REQUEST_BYTES {
        return Err(WireError::Oversized);
    }
    Ok(request.encode_to_vec())
}

pub fn decode_request(bytes: &[u8]) -> Result<proto::WorkspaceRequest, WireError> {
    if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
        return Err(WireError::Oversized);
    }
    let value = proto::WorkspaceRequest::decode(bytes).map_err(|_| WireError::Decode)?;
    validate_request(&value)?;
    if value.encode_to_vec() != bytes {
        return Err(WireError::Noncanonical);
    }
    Ok(value)
}

pub fn encode_reply(reply: &proto::WorkspaceReply) -> Result<Vec<u8>, WireError> {
    validate_reply(reply)?;
    if reply.encoded_len() > MAX_REPLY_BYTES {
        return Err(WireError::Oversized);
    }
    Ok(reply.encode_to_vec())
}

pub fn decode_reply(bytes: &[u8]) -> Result<proto::WorkspaceReply, WireError> {
    if bytes.is_empty() || bytes.len() > MAX_REPLY_BYTES {
        return Err(WireError::Oversized);
    }
    let value = proto::WorkspaceReply::decode(bytes).map_err(|_| WireError::Decode)?;
    validate_reply(&value)?;
    if value.encode_to_vec() != bytes {
        return Err(WireError::Noncanonical);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::workspace_request::Operation;
    #[test]
    fn canonical_request_rejects_unknown_duplicate_and_version() {
        let request = proto::WorkspaceRequest {
            protocol_version: VERSION,
            request_id: "q-1".into(),
            operation: Some(Operation::List(proto::ListProjects {})),
        };
        let valid = encode_request(&request).unwrap();
        assert_eq!(decode_request(&valid).unwrap(), request);
        let mut unknown = valid.clone();
        unknown.extend_from_slice(&[0x78, 0x01]);
        assert!(decode_request(&unknown).is_err());
        let mut duplicate = valid.clone();
        duplicate.extend_from_slice(&[0x08, 0x01]);
        assert!(decode_request(&duplicate).is_err());
        let mut invalid_version = request.clone();
        invalid_version.protocol_version = 42;
        assert!(encode_request(&invalid_version).is_err());
        assert!(decode_request(&vec![0; MAX_REQUEST_BYTES + 1]).is_err());
    }

    #[test]
    fn protocol_never_carries_project_paths_or_authority() {
        let raw = include_str!("../../proto/lu/ide/v1/workspace.proto");
        for forbidden in [
            "absolute_path",
            "shell_command",
            "access_token",
            "workspace_root",
            "privileged_path",
        ] {
            assert!(!raw.contains(&format!("string {forbidden} =")));
        }
        assert!(!valid_project_id("../../etc"));
        assert!(!valid_project_id("ide-ABCDEFABCDEFABCDEFABCDEFABCDEFAB"));
    }
}
