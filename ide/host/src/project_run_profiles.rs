//! DEV-03.1: registered-project build/test *discovery*, never execution.
//!
//! A filename is not an approved manifest; a Git HEAD is not a snapshot;
//! a package.json script is untrusted project code. The existing WO6
//! isolated, approved Cargo worker is the only execution route.
use crate::{browser::BrowserError, IdeWorkspaceRegistry};
use serde::Serialize;
use std::io::ErrorKind;

const MAX_PROFILE_COUNT: usize = 7;
const MAX_HINTS_PER_LANGUAGE: usize = 20;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRunProfile {
    pub profile_id: &'static str,
    pub title: &'static str,
    pub language: &'static str,
    pub operation: &'static str,
    pub manifest_relative_path: &'static str,
    pub manifest_state: &'static str,
    pub manifest_sha256: Option<String>,
    pub execution_route: &'static str,
    pub existing_native_recipe_id: Option<&'static str>,
    pub execution_authorized: bool,
    pub test_candidates: usize,
    pub test_hint_paths: Vec<String>,
    pub test_scan_state: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRunProfileDiscovery {
    pub schema_version: u32,
    pub project_id: String,
    pub source: &'static str,
    pub profiles: Vec<ProjectRunProfile>,
    pub manifest_contents_exported: bool,
    pub process_started: bool,
    pub worker_grant_issued: bool,
    pub task_accepted: bool,
}

struct Spec {
    id: &'static str,
    title: &'static str,
    language: &'static str,
    operation: &'static str,
    manifest: &'static str,
    native_recipe: Option<&'static str>,
}
const SPECS: [Spec; MAX_PROFILE_COUNT] = [
    Spec {
        id: "rust_check",
        title: "Rust · Cargo check",
        language: "rust",
        operation: "check",
        manifest: "Cargo.toml",
        native_recipe: Some("rust_check"),
    },
    Spec {
        id: "rust_tests",
        title: "Rust · Cargo tests",
        language: "rust",
        operation: "test",
        manifest: "Cargo.toml",
        native_recipe: Some("rust_tests"),
    },
    Spec {
        id: "python_pytest",
        title: "Python · pytest (not yet runnable)",
        language: "python",
        operation: "test",
        manifest: "pyproject.toml",
        native_recipe: None,
    },
    Spec {
        id: "js_tests",
        title: "JavaScript · tests (not yet runnable)",
        language: "javascript",
        operation: "test",
        manifest: "package.json",
        native_recipe: None,
    },
    Spec {
        id: "cpp_cmake_build",
        title: "C/C++ · CMake (not yet runnable)",
        language: "cpp",
        operation: "build",
        manifest: "CMakeLists.txt",
        native_recipe: None,
    },
    Spec {
        id: "lu_ide_host_tests",
        title: "Lu IDE host tests · Cargo",
        language: "rust",
        operation: "test",
        manifest: "ide/host/Cargo.toml",
        native_recipe: Some("lu_ide_host_tests"),
    },
    Spec {
        id: "rust_build",
        title: "Rust · Cargo build",
        language: "rust",
        operation: "build",
        manifest: "Cargo.toml",
        native_recipe: Some("rust_build"),
    },
];

fn manifest_observation(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
    relative_path: &str,
) -> (&'static str, Option<String>) {
    match registry.preview_source_file(project_id, relative_path) {
        Ok(observed)
            if observed.project_id == project_id && observed.relative_path == relative_path =>
        {
            (
                "saved_file_sha_observed_not_validated",
                Some(observed.revision),
            )
        }
        Ok(_) => ("identity_mismatch", None),
        Err(BrowserError::Io(error)) if error.kind() == ErrorKind::NotFound => ("not_found", None),
        Err(_) => ("not_readable_or_unverifiable", None),
    }
}

fn test_name(language: &str, path: &str) -> bool {
    let Some(name) = path.rsplit('/').next() else {
        return false;
    };
    match language {
        "rust" => name.ends_with(".rs"),
        "python" => {
            name.starts_with("test_") && name.ends_with(".py") || name.ends_with("_test.py")
        }
        "javascript" => [
            ".test.js",
            ".spec.js",
            ".test.ts",
            ".spec.ts",
            ".test.jsx",
            ".spec.jsx",
            ".test.tsx",
            ".spec.tsx",
        ]
        .iter()
        .any(|ext| name.ends_with(ext)),
        "cpp" => {
            (name.ends_with(".cpp") || name.ends_with(".cc") || name.ends_with(".cxx"))
                && (name.contains("test") || name.contains("Test"))
        }
        _ => false,
    }
}

fn test_hints(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
    language: &str,
) -> (usize, Vec<String>, &'static str) {
    // One nonrecursive, bounded metadata-only scan. A missing tests/ directory
    // does not mean the project contains no tests elsewhere.
    match registry.list_source_directory(project_id, "tests") {
        Ok(list) if list.project_id == project_id && list.relative_dir == "tests" => {
            let mut count = 0;
            let mut paths = Vec::new();
            for entry in list.entries {
                if entry.kind == "file" && test_name(language, &entry.relative_path) {
                    count += 1;
                    if paths.len() < MAX_HINTS_PER_LANGUAGE {
                        paths.push(entry.relative_path);
                    }
                }
            }
            let state = if list.truncated || count > MAX_HINTS_PER_LANGUAGE {
                "partial_conventional_tests_only"
            } else {
                "bounded_conventional_tests_only_not_discovery_complete"
            };
            (count, paths, state)
        }
        Ok(_) => (0, Vec::new(), "native_identity_mismatch"),
        Err(BrowserError::Io(error)) if error.kind() == ErrorKind::NotFound => {
            (0, Vec::new(), "tests_directory_absent_not_no_tests")
        }
        Err(_) => (0, Vec::new(), "tests_directory_unverifiable"),
    }
}

impl IdeWorkspaceRegistry {
    /// Host-only fixed probes. Caller cannot submit a command, path,
    /// executable, environment, script, or desired provider.
    pub fn discover_project_run_profiles(
        &self,
        project_id: &str,
    ) -> Result<ProjectRunProfileDiscovery, &'static str> {
        // Validate the registered active project *before* any fixed probing.
        self.source_capability(project_id)
            .map_err(|_| "DEV03_PROJECT_NOT_OPEN")?;
        let mut profiles = Vec::with_capacity(MAX_PROFILE_COUNT);
        for spec in SPECS.iter() {
            let (state, revision) = manifest_observation(self, project_id, spec.manifest);
            let (count, paths, test_state) = if spec.operation == "test" {
                test_hints(self, project_id, spec.language)
            } else {
                (0, Vec::new(), "not_test_profile")
            };
            profiles.push(ProjectRunProfile {
                profile_id: spec.id,
                title: spec.title,
                language: spec.language,
                operation: spec.operation,
                manifest_relative_path: spec.manifest,
                manifest_state: state,
                manifest_sha256: revision,
                execution_route: if spec.native_recipe.is_some() {
                    "existing_wo6_protected_cargo_requires_fresh_authorization"
                } else {
                    "not_implemented_requires_reviewed_native_adapter"
                },
                existing_native_recipe_id: spec.native_recipe,
                execution_authorized: false,
                test_candidates: count,
                test_hint_paths: paths,
                test_scan_state: test_state,
            });
        }
        Ok(ProjectRunProfileDiscovery {
            schema_version: 1,
            project_id: project_id.to_owned(),
            source: "fixed_host_manifest_and_conventional_tests_inspection",
            profiles,
            manifest_contents_exported: false,
            process_started: false,
            worker_grant_issued: false,
            task_accepted: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_fixed_existing_cargo_recipes_may_be_selected() {
        assert_eq!(SPECS.len(), 7);
        for spec in SPECS.iter().filter(|s| s.native_recipe.is_some()) {
            assert!(matches!(
                spec.native_recipe,
                Some("rust_check" | "rust_build" | "rust_tests" | "lu_ide_host_tests")
            ));
        }
        assert_eq!(
            SPECS.iter().filter(|s| s.native_recipe.is_none()).count(),
            3
        );
        assert!(!test_name("javascript", "tests/setup.js"));
        assert!(test_name("python", "tests/test_loading.py"));
        assert!(test_name("cpp", "tests/RendererTest.cpp"));
    }
}
