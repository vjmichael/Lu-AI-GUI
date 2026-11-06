//! Wave A3: bounded, read-only project evidence inventory.
//!
//! Uses only the registered project directory capability. No shell process,
//! network request, package execution, or ambient filesystem discovery.
//! A file hash proves inspected bytes, NOT that a binary is safe or usable.
use std::io::Read;

use cap_std::fs::Dir;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    browser::{descend, validate_relative, BrowserError},
    IdeWorkspaceRegistry,
};

const MAX_CANDIDATES: usize = 80;
const MAX_HASH_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SCAN_HASH_BYTES: u64 = 128 * 1024 * 1024;
const MAX_GIT_REF_BYTES: u64 = 1024;
const DISCOVERY_DIRS: [&str; 16] = [
    "",
    "Delivered",
    "artifacts",
    "benchmarks",
    "benchmark-results",
    "results",
    "dist",
    "ide/Delivered",
    "ide/dist",
    "ide/app/src-tauri/target/release/bundle/nsis",
    "target/release/bundle/nsis",
    "tools",
    "bin",
    "ide/tools",
    ".venv/Scripts",
    ".venv/bin",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceFile {
    pub relative_path: String,
    pub category: &'static str,
    pub state: &'static str,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub digest_status: &'static str,
    pub trust: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitObservation {
    pub state: &'static str,
    pub branch: Option<String>,
    pub local_head: Option<String>,
    pub local_tracking_head: Option<String>,
    pub tracking_comparison: &'static str,
    pub working_tree: &'static str,
    pub live_remote: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceInventory {
    pub schema_version: u32,
    pub project_id: String,
    pub scope: &'static str,
    pub discovery_complete: bool,
    pub files: Vec<EvidenceFile>,
    pub git: GitObservation,
    pub toolchain_readiness: &'static str,
    pub execution_performed: bool,
}

fn read_small(dir: &Dir, path: &str) -> Option<String> {
    let mut parts = validate_relative(path, false).ok()?;
    let leaf = parts.pop()?;
    let folder = descend(dir.try_clone().ok()?, &parts).ok()?;
    let metadata = folder.symlink_metadata(&leaf).ok()?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_GIT_REF_BYTES
    {
        return None;
    }
    let mut data = String::new();
    folder
        .open(&leaf)
        .ok()?
        .take(MAX_GIT_REF_BYTES + 1)
        .read_to_string(&mut data)
        .ok()?;
    if data.len() as u64 > MAX_GIT_REF_BYTES {
        return None;
    }
    Some(data.trim().to_owned())
}

fn hex_commit(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn git_snapshot(dir: &Dir) -> GitObservation {
    let unknown = || GitObservation {
        state: "unverifiable",
        branch: None,
        local_head: None,
        local_tracking_head: None,
        tracking_comparison: "not_inspected",
        working_tree: "not_verified",
        live_remote: "not_queried",
    };
    // Worktree .git files and packed refs must not be resolved through
    // arbitrary gitdir pointers. They are reported as unsupported.
    let Some(head) = read_small(dir, ".git/HEAD") else {
        return unknown();
    };
    if hex_commit(&head) {
        return GitObservation {
            state: "detached_head",
            branch: None,
            local_head: Some(head),
            local_tracking_head: None,
            tracking_comparison: "not_applicable",
            working_tree: "not_verified",
            live_remote: "not_queried",
        };
    }
    let Some(branch) = head.strip_prefix("ref: refs/heads/") else {
        return unknown();
    };
    if validate_relative(branch, false).is_err() {
        return unknown();
    }
    let local = read_small(dir, &format!(".git/refs/heads/{branch}")).filter(|s| hex_commit(s));
    let tracking =
        read_small(dir, &format!(".git/refs/remotes/origin/{branch}")).filter(|s| hex_commit(s));
    let comparison = match (&local, &tracking) {
        (Some(a), Some(b)) if a == b => "local_tracking_equal",
        (Some(_), Some(_)) => "local_tracking_differs",
        _ => "not_verified",
    };
    GitObservation {
        state: if local.is_some() {
            "loose_ref_observed"
        } else {
            "ref_unverifiable"
        },
        branch: Some(branch.to_owned()),
        local_head: local,
        local_tracking_head: tracking,
        tracking_comparison: comparison,
        working_tree: "not_verified",
        live_remote: "not_queried",
    }
}

fn root_artifact_candidate(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        ".exe",
        ".msi",
        ".zip",
        ".tar",
        ".gz",
        ".deb",
        ".rpm",
        ".appimage",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
        || lower.contains("benchmark")
        || lower.contains("results")
        || lower.contains("manifest")
}

fn category(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.contains("bench") || lower.contains("/results/") || lower.starts_with("results/") {
        "benchmark_or_result_evidence"
    } else if lower.starts_with("tools/")
        || lower.starts_with("bin/")
        || lower.starts_with("ide/tools/")
        || lower.starts_with(".venv/")
    {
        "toolchain_candidate"
    } else if lower.ends_with(".exe")
        || lower.ends_with(".msi")
        || lower.ends_with(".deb")
        || lower.ends_with(".rpm")
        || lower.ends_with(".appimage")
    {
        "installer_or_executable"
    } else if lower.ends_with(".zip") || lower.ends_with(".tar") || lower.ends_with(".gz") {
        "archive"
    } else {
        "build_artifact_or_metadata"
    }
}

fn verify_file(
    folder: &Dir,
    name: &str,
    path: String,
    remaining: &mut u64,
) -> Option<EvidenceFile> {
    let metadata = folder.symlink_metadata(name).ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return None;
    }
    let size = metadata.len();
    let (sha, status) = if size <= MAX_HASH_BYTES && size <= *remaining {
        let mut file = folder.open(name).ok()?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut total = 0u64;
        loop {
            let count = file.read(&mut buffer).ok()?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > MAX_HASH_BYTES {
                return None;
            }
            hasher.update(&buffer[..count]);
        }
        if total != size {
            (None, "changed_during_probe")
        } else {
            *remaining -= total;
            (Some(format!("{:x}", hasher.finalize())), "sha256_observed")
        }
    } else {
        (
            None,
            if size > MAX_HASH_BYTES {
                "over_digest_limit"
            } else {
                "scan_budget_exhausted"
            },
        )
    };
    Some(EvidenceFile {
        relative_path: path.clone(),
        category: category(&path),
        state: "present_unverified",
        size_bytes: size,
        sha256: sha,
        digest_status: status,
        trust: "untrusted_project_file",
    })
}

impl IdeWorkspaceRegistry {
    /// Only predetermined directories, one level each; no recursive walk,
    /// subprocess, dynamic PATH probing or download. Absence of evidence here
    /// cannot establish that another user-owned folder has no artifacts.
    pub fn inspect_project_evidence(
        &self,
        project_id: &str,
    ) -> Result<EvidenceInventory, BrowserError> {
        let root = self.source_capability(project_id)?;
        let git = git_snapshot(&root);
        let mut files = Vec::new();
        let mut complete = true;
        let mut remaining_digest_bytes = MAX_SCAN_HASH_BYTES;
        for name in DISCOVERY_DIRS {
            let Ok(parts) = validate_relative(name, true) else {
                continue;
            };
            let Ok(folder) = descend(root.try_clone()?, &parts) else {
                continue;
            };
            let mut entries = Vec::new();
            for entry in folder.entries()? {
                let entry = entry?;
                let file_name = entry.file_name();
                let Some(file_name) = file_name.to_str() else {
                    continue;
                };
                if validate_relative(file_name, false).is_err()
                    || (name.is_empty() && !root_artifact_candidate(file_name))
                {
                    continue;
                }
                entries.push(file_name.to_owned());
                if entries.len() > MAX_CANDIDATES {
                    complete = false;
                    break;
                }
            }
            entries.sort();
            for filename in entries {
                if files.len() >= MAX_CANDIDATES {
                    complete = false;
                    break;
                }
                let path = if name.is_empty() {
                    filename.clone()
                } else {
                    format!("{name}/{filename}")
                };
                if let Some(item) =
                    verify_file(&folder, &filename, path, &mut remaining_digest_bytes)
                {
                    files.push(item);
                }
            }
            if files.len() >= MAX_CANDIDATES {
                break;
            }
        }
        let toolchain_readiness = if files
            .iter()
            .any(|file| file.category == "toolchain_candidate")
        {
            "present_unverified"
        } else {
            "not_verified"
        };
        Ok(EvidenceInventory {
            schema_version: 1,
            project_id: project_id.to_owned(),
            scope: "bounded_known_project_directories_only",
            discovery_complete: complete,
            files,
            git,
            toolchain_readiness,
            execution_performed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_hex_is_strict() {
        assert!(hex_commit(&"a".repeat(40)));
        assert!(!hex_commit("deadbeef"));
        assert!(!hex_commit(&"z".repeat(40)));
    }
    #[test]
    fn benchmark_and_installer_classification_is_descriptive_only() {
        assert_eq!(category("Delivered/app.exe"), "installer_or_executable");
        assert_eq!(
            category("benchmarks/results.json"),
            "benchmark_or_result_evidence"
        );
    }
}
