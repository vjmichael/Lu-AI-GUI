//! Wave C2: non-executing file evidence probe scoped to a registered project.
//! A byte digest is evidence of observed bytes, never proof a tool can run.
use crate::{
    browser::{descend, validate_relative, BrowserError},
    IdeWorkspaceRegistry,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{io::Read, time::UNIX_EPOCH};

const MAX_DIGEST_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResourceProbe {
    pub schema_version: u32,
    pub project_id: String,
    pub relative_path: String,
    pub state: &'static str,
    pub byte_length: Option<u64>,
    pub sha256: Option<String>,
    pub modified_unix_seconds: Option<u64>,
    pub digest_status: &'static str,
    pub executable_usable: &'static str,
    pub last_trusted_execution: &'static str,
    pub inspection_scope: &'static str,
}

fn modified_time(metadata: &cap_std::fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .into_std()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

impl IdeWorkspaceRegistry {
    /// Recheck an exact resource relative to the host-selected project root.
    /// The returned hash identifies bytes actually read; it does not load or
    /// launch an executable, install a package or contact any remote server.
    pub fn probe_project_resource(
        &self,
        project_id: &str,
        relative_path: &str,
    ) -> Result<ProjectResourceProbe, BrowserError> {
        let mut components = validate_relative(relative_path, false)?;
        let file = components.pop().ok_or(BrowserError::InvalidPath)?;
        let root = self.source_capability(project_id)?;
        let folder = descend(root, &components)?;
        let state = |availability, size, sha, stamp, digest| ProjectResourceProbe {
            schema_version: 1,
            project_id: project_id.to_owned(),
            relative_path: relative_path.to_owned(),
            state: availability,
            byte_length: size,
            sha256: sha,
            modified_unix_seconds: stamp,
            digest_status: digest,
            executable_usable: "not_tested",
            last_trusted_execution: "not_inspected",
            inspection_scope: "registered_project_only",
        };
        let metadata = match folder.symlink_metadata(&file) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(state(
                    "missing_in_registered_project",
                    None,
                    None,
                    None,
                    "not_applicable",
                ))
            }
            Err(e) => return Err(BrowserError::Io(e)),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Ok(state(
                "not_regular_file",
                Some(metadata.len()),
                None,
                modified_time(&metadata),
                "not_applicable",
            ));
        }
        if metadata.len() > MAX_DIGEST_BYTES {
            return Ok(state(
                "present_unhashed_over_limit",
                Some(metadata.len()),
                None,
                modified_time(&metadata),
                "not_computed",
            ));
        }
        let mut reader = folder.open(&file)?;
        let opened = reader.metadata()?;
        if !opened.is_file() || opened.len() > MAX_DIGEST_BYTES {
            return Ok(state(
                "changed_during_probe",
                Some(opened.len()),
                None,
                modified_time(&opened),
                "unverifiable",
            ));
        }
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut count = 0u64;
        loop {
            let len = reader.read(&mut buffer)?;
            if len == 0 {
                break;
            }
            count = count.saturating_add(len as u64);
            if count > MAX_DIGEST_BYTES {
                return Ok(state(
                    "changed_during_probe",
                    Some(count),
                    None,
                    modified_time(&opened),
                    "unverifiable",
                ));
            }
            hasher.update(&buffer[..len]);
        }
        let after = folder.symlink_metadata(&file)?;
        if after.file_type().is_symlink()
            || !after.is_file()
            || count != metadata.len()
            || after.len() != metadata.len()
            || modified_time(&metadata) != modified_time(&after)
        {
            return Ok(state(
                "changed_during_probe",
                Some(count),
                None,
                modified_time(&after),
                "unverifiable",
            ));
        }
        Ok(state(
            "bytes_observed",
            Some(count),
            Some(format!("{:x}", hasher.finalize())),
            modified_time(&after),
            "sha256_observed_not_execution_verified",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn digest_limit_is_bound_to_one_registered_file() {
        assert_eq!(MAX_DIGEST_BYTES, 64 * 1024 * 1024);
    }
}
