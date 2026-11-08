//! Wave A2: bounded, host-authored discovery of real standalone IDE commands.
//! The schemas describe shapes and limits, not authorization or a dispatch route.
use serde::Serialize;
use serde_json::Value;

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDescriptor {
    pub id: &'static str,
    pub version: u32,
    pub command: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub effect: &'static str,
    /// JSON Schema 2020-12 request object. References resolve against the
    /// companion schema_definitions in this same catalog response.
    pub request_schema: Value,
    pub result_schema: Value,
    pub error_schema: Value,
    pub precondition: &'static str,
    pub evidence: &'static str,
    pub cancellation: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityCatalog {
    pub schema_version: u32,
    pub workspace: &'static str,
    pub connected_to_lu: bool,
    pub descriptions_only: bool,
    pub schema_definitions: Value,
    pub capabilities: Vec<CapabilityDescriptor>,
}

macro_rules! capability {
    ($id:literal, $command:literal, $title:literal, $description:literal,
     $effect:literal, $precondition:literal, $evidence:literal) => {
        (
            $id,
            $command,
            $title,
            $description,
            $effect,
            $precondition,
            $evidence,
        )
    };
}

type CapabilityMeta = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

fn metadata() -> Vec<CapabilityMeta> {
    vec![
        capability!(
            "lsp.catalog",
            "ide_list_language_servers",
            "Discover supported free language servers",
            "Read a fixed open-source language-server catalog; no process or project code is executed.",
            "read",
            "IDE host available; no project trust grant or executable permission",
            "versioned server IDs, language coverage, license and fixed launcher hints; execution disabled"
        ),
        capability!(
            "job.prepare",
            "ide_prepare_execution_intent",
            "Prepare a non-executable job request",
            "Persist an allowlisted recipe intention bound to the exact clean local Git HEAD and manifest digest. No worker can launch.",
            "catalog_write",
            "human-selected active registered project, clean Git HEAD and fixed native recipe",
            "durable prepared intent with bounded expiration; no execution token or process"
        ),
        capability!(
            "job.review",
            "ide_decide_execution_intent",
            "Record or withdraw human job review",
            "Revalidate source HEAD and manifest before recording review, or cancel an intent. No execution permission is issued.",
            "catalog_write",
            "human decision on exact private intent ID, same app session, source still clean",
            "durable single-use review/cancellation record with explicit no-execution status"
        ),
        capability!(
            "job.list",
            "ide_list_execution_intents",
            "Read prepared job-intent history",
            "List bounded project-scoped job intentions and their authoritative non-execution states.",
            "read",
            "active registered project",
            "never-started job history, expiration and stale session visibility"
        ),
        capability!(
            "resource.probe",
            "ide_probe_project_resource",
            "Inspect resource bytes",
            "Read exact scoped file metadata and SHA-256 without executing it.",
            "read",
            "open registered project, validated relative file path",
            "observed bytes and digest, missing/changed state, explicit untested execution"
        ),
        capability!(
            "inventory.capture",
            "ide_capture_project_inventory",
            "Record verified inventory snapshot",
            "Persist observed artifact paths and hashes for future comparison; no execution.",
            "catalog_write",
            "open project and explicit user snapshot request",
            "historical observed byte hashes; no software usability claim"
        ),
        capability!(
            "inventory.compare",
            "ide_compare_project_inventory",
            "Compare inventory",
            "Read-only comparison of latest bounded project evidence with saved baseline.",
            "read",
            "open project; baseline optional",
            "matched/changed/missing/new/unverifiable observed bytes"
        ),
        capability!(
            "git.compareLive",
            "ide_compare_live_github",
            "Compare with live GitHub",
            "Explicit read-only HTTPS check against the public GitHub origin branch head.",
            "network_read",
            "human request, supported public github.com origin, exact registered project",
            "remote commit SHA and checked time; independently labeled local and dirty state"
        ),
        capability!(
            "task.update",
            "ide_record_task_update",
            "Record a development task update",
            "Append an explicit human task note and native observed source HEAD to the existing private ledger; not task acceptance.",
            "catalog_write",
            "active registered project and bounded task ID, title, status, next action and enumerated required gates",
            "new ledger event and task revision; source commit captured by native host, no claim of passing tests"
        ),
        capability!(
            "task.ciObserve",
            "ide_observe_task_ci",
            "Check and link exact-commit CI for a task",
            "Explicit public GitHub Actions inspection from the host, linked to an unchanged task update and current HEAD, stored in the same ledger.",
            "catalog_write",
            "active registered project, existing task with same source HEAD, explicit user click; may make bounded GitHub read",
            "append-only task CI observation bound to exact task event and commit SHA; not installation or task acceptance"
        ),
        capability!(
            "git.ciExactHead",
            "ide_read_exact_head_ci",
            "Inspect exact-head CI evidence",
            "Explicit bounded public GitHub Actions read for the registered project's exact local HEAD; no build or installation.",
            "network_read",
            "active registered Git worktree, supported public github.com origin, human action",
            "workflow run and required job receipts bound to exact local SHA; unknowns and untested installer remain explicit"
        ),
        capability!(
            "git.diff",
            "ide_read_git_diff",
            "Review Git changes",
            "Read staged or unstaged local Git patch text, capped at 64 KiB.",
            "read",
            "active registered Git project, staged/worktree scope",
            "bounded patch and local HEAD, with explicit truncation and untracked exclusion"
        ),
        capability!(
            "git.history",
            "ide_read_git_history",
            "Review local commit history",
            "Read recent local HEAD commits without fetching or changing Git.",
            "read",
            "active registered Git project with optional local HEAD",
            "up to 30 local commit IDs, summaries and times; no remote assertion"
        ),
        capability!(
            "git.reviewOne",
            "ide_git_review_file",
            "Review one exact tracked Git file",
            "Native libgit2 and registered project capability validate a tracked regular file, SHA and index before staging.",
            "read",
            "active native registered project; only tracked regular source; no hooks/network",
            "file SHA256, index fingerprint and local HEAD; no Git writes"
        ),
        capability!(
            "git.stageOne",
            "ide_git_stage_reviewed",
            "Stage one reviewed Git file",
            "After separate OS confirmation, stage the exact in-memory reviewed bytes with libgit2; never run hooks or push.",
            "file_write",
            "prior exact review, unchanged source SHA/HEAD/index and native OS one-use consent",
            "pinned Git index entry and blob SHA; no process launched"
        ),
        capability!(
            "git.commitOne",
            "ide_git_commit_reviewed",
            "Commit one reviewed staged Git file",
            "After separate OS confirmation, create a local commit and conditionally update unchanged branch reference; never run hooks or push.",
            "file_write",
            "exact staged receipt, unchanged HEAD and index, reviewed message and identity, native consent",
            "new local commit SHA; remote unchanged and no installer approval"
        ),
        capability!(
            "workflow.create",
            "ide_workflow_create",
            "Create durable dependent build/test graph",
            "Persist finite recipe dependencies from an exact local Git/artifact workflow fingerprint; no process or authorization.",
            "catalog_write",
            "registered active projects, fresh exact fingerprint, earlier-step-only dependencies and fixed recipes",
            "fsynced native workflow ID and pending steps; auto execution forbidden"
        ),
        capability!(
            "workflow.listRuns",
            "ide_workflow_list_runs",
            "Read durable workflow state",
            "Read bounded projects/steps and native supervisor job receipts from a private journal; interrupted steps remain quarantined.",
            "read",
            "local native app profile available",
            "actual durable states, not independently installed release proof"
        ),
        capability!(
            "workflow.cancelPending",
            "ide_workflow_cancel_pending",
            "Cancel a not-started workflow step",
            "Persist pending-step cancellation and prevent dependents; executing OS jobs must use protected supervisor cancel instead.",
            "catalog_write",
            "exact workflow and pending step; no process to kill",
            "fsynced cancelled pending state, no worker started"
        ),
        capability!(
            "workflow.preview",
            "ide_workflow_preview",
            "Inspect registered multi-project workflow",
            "Read only native Git and bounded artifact hashes from up to eight OS-picker-registered projects.",
            "read",
            "explicit selected active project IDs; no paths, scripts, Git writes or release authority",
            "deterministic source fingerprint, per-project Git HEAD and artifact digest blockers"
        ),
        capability!(
            "workflow.verify",
            "ide_workflow_verify",
            "Revalidate exact workflow evidence",
            "Independently re-inspect every selected Git HEAD, working tree and hashed candidate and deny stale fingerprints.",
            "read",
            "prior native preview, exact SHA-256, registered active projects",
            "same native fingerprint or stable drift/blocked error; no execution"
        ),
        capability!(
            "workflow.manifest",
            "ide_workflow_export_manifest",
            "Export reviewed workflow manifest",
            "Use a native OS folder chooser to create a new fsynced review JSON manifest, never overwriting a file.",
            "file_write",
            "human click, native-picked destination outside project roots, verified unchanged workflow",
            "new named bounded review JSON and SHA-256; no source writes, signatures or release"
        ),
        capability!(
            "workflow.bundle",
            "ide_workflow_export_bundle",
            "Export bounded verified artifact ZIP",
            "Use a native OS folder picker to write a new portable stored ZIP of rehashed verified project artifacts only.",
            "file_write",
            "human click, registered sources, exact revalidated fingerprint, bounded files and trusted native destination",
            "ZIP file name, bytes, count and digest; project code never executed or signed"
        ),
        capability!(
            "git.status",
            "ide_read_git_status",
            "Inspect source control",
            "Read local Git modified, staged and untracked paths with trusted native libgit2.",
            "read",
            "registered open project; no network or Git write",
            "bounded local worktree status and stored upstream comparison"
        ),
        capability!(
            "continuity.read",
            "ide_read_project_context",
            "Read project continuity",
            "Read bounded host observations and revisioned untrusted project notes.",
            "read",
            "registered and active project",
            "historical events, document revisions and explicitly unknown external state"
        ),
        capability!(
            "project.list",
            "ide_list_projects",
            "List projects",
            "Read IDE-registered project summaries.",
            "read",
            "IDE host available",
            "registered project summaries"
        ),
        capability!(
            "project.select",
            "ide_choose_project",
            "Choose project folder",
            "Open native OS directory picker and register selected project.",
            "catalog_write",
            "explicit human picker selection",
            "updated project catalog"
        ),
        capability!(
            "project.open",
            "ide_open_project",
            "Open registered project",
            "Activate an existing project in the IDE.",
            "session_write",
            "registered project ID",
            "active project summary"
        ),
        capability!(
            "project.close",
            "ide_close_project",
            "Close project",
            "Close an open IDE project, without deleting its files.",
            "session_write",
            "open project ID",
            "inactive project summary"
        ),
        capability!(
            "project.unregister",
            "ide_unregister_project",
            "Remove project from IDE",
            "Remove project registration only; never remove files on disk.",
            "catalog_write",
            "registered project ID and user intent",
            "catalog without project"
        ),
        capability!(
            "draft.checkpoint",
            "ide_draft_checkpoint",
            "Preserve unsaved editor checkpoint",
            "Write a bounded immutable revisioned draft only to IDE private app data with CAS; never Save project files.",
            "catalog_write",
            "active OS-selected project, validated source, current buffer ID and expected private head",
            "fsynced private checkpoint ID, full draft digest and exact retained content; no project write"
        ),
        capability!(
            "draft.latest",
            "ide_draft_latest",
            "Inspect private preserved draft",
            "Read the latest bound private draft checkpoint for a registered project file without auto-restoring.",
            "read",
            "active OS-selected project and registered source file",
            "verified checkpoint data or none; saved-file conflict independently checked"
        ),
        capability!(
            "draft.load",
            "ide_draft_load",
            "Recover exact preserved checkpoint",
            "Read an immutable content-addressed private snapshot; never automatically replace editor or saved files.",
            "read",
            "active OS-selected project and exact private checkpoint SHA-256",
            "verified original buffer revision and content with no effect on project source"
        ),
        capability!(
            "draft.discard",
            "ide_draft_discard",
            "Discard preserved private draft history",
            "Explicitly delete a selected file's private checkpoint history only after exact current-head match.",
            "catalog_write",
            "active registered source, exact private checkpoint ID and human discard decision",
            "private checkpoint history removed; original project file unchanged"
        ),
        capability!(
            "source.findProject",
            "ide_find_in_project",
            "Find saved source in project",
            "Read-only bounded literal search of registered project's saved UTF-8 files; generated folders excluded.",
            "read",
            "registered open project and nonempty literal query",
            "revision-tagged source matches, exact byte ranges, skipped counts and truncation"
        ),
        capability!(
            "source.list",
            "ide_list_source_directory",
            "List source directory",
            "Capability-scoped, bounded listing of one directory.",
            "read",
            "registered and open project",
            "relative paths and truncation flag"
        ),
        capability!(
            "source.preview",
            "ide_preview_source_file",
            "Read source file",
            "Read bounded UTF-8 text and its saved SHA-256 revision.",
            "read",
            "open project and safe relative path",
            "text and saved source revision"
        ),
        capability!(
            "source.readSpan",
            "ide_read_source_span",
            "Read source lines",
            "Read exact numbered lines from a specified saved revision.",
            "read",
            "open project and matching saved revision",
            "line text, UTF-8 byte offsets and newline identity"
        ),
        capability!(
            "source.review",
            "ide_review_source_change",
            "Review source change",
            "Calculate bounded diff preview without saving.",
            "read",
            "matching saved revision and bounded proposed text",
            "diff hunks with exactness and truncation"
        ),
        capability!(
            "source.save",
            "ide_save_source_file",
            "Save source file",
            "Explicit host revision-checked file write; discovery does not authorize it.",
            "file_write",
            "explicit human Save and matching saved revision",
            "post-write file content and revision"
        ),
        capability!(
            "proposal.import",
            "ide_import_proposal",
            "Import proposal",
            "Validate untrusted proposal as data in volatile inbox.",
            "session_write",
            "bounded valid proposal",
            "inbox summary; no filesystem write"
        ),
        capability!(
            "proposal.list",
            "ide_list_proposals",
            "List proposals",
            "Inspect volatile proposal inbox.",
            "read",
            "IDE host available",
            "proposal summaries"
        ),
        capability!(
            "proposal.review",
            "ide_review_proposal",
            "Review proposal",
            "Read verified proposal details and bounded diffs.",
            "read",
            "existing inbox item",
            "preview with exactness and truncation"
        ),
        capability!(
            "proposal.decide",
            "ide_decide_proposal",
            "Decide proposal",
            "Record human approval for staging only or rejection.",
            "session_write",
            "human review and unchanged proposal digest",
            "recorded review decision; no file write"
        ),
        capability!(
            "proposal.stage",
            "ide_stage_proposal",
            "Stage proposal",
            "Return approved edit data for unsaved editor drafts only.",
            "read",
            "previous human approval and current source revisions",
            "validated staging data; no file write"
        ),
        capability!(
            "proposal.discard",
            "ide_discard_proposal",
            "Discard proposal",
            "Remove an inbox item without changing source files.",
            "session_write",
            "existing inbox item",
            "updated volatile inbox"
        ),
    ]
}

/// This is an embedded, versioned and reviewable schema contract. It is not
/// loaded from projects, model output or the network.
fn schema_document() -> Value {
    serde_json::from_str(include_str!("capability_schemas.json"))
        .expect("maintained capability schema JSON must be valid")
}

/// Make each response schema independently valid JSON Schema 2020-12:
/// its nested $ref values must resolve without requiring a separate file.
fn self_contained_schema(mut shape: Value, definitions: &Value) -> Value {
    if let Some(properties) = shape.as_object_mut() {
        properties.insert(
            "$schema".to_owned(),
            Value::String("https://json-schema.org/draft/2020-12/schema".to_owned()),
        );
        properties.insert("$defs".to_owned(), definitions.clone());
    }
    shape
}

/// Advertise only real registered native commands. A descriptor supplies no
/// grant, transport endpoint, authenticated identity or permission to execute.
pub fn list_capabilities() -> CapabilityCatalog {
    let schemas = schema_document();
    let definitions = schemas["definitions"].clone();
    let operations = &schemas["operations"];
    let capabilities = metadata()
        .into_iter()
        .map(
            |(id, command, title, description, effect, precondition, evidence)| {
                let schema = operations
                    .get(id)
                    .expect("every native descriptor must have an operation schema");
                CapabilityDescriptor {
                    id,
                    version: SCHEMA_VERSION,
                    command,
                    title,
                    description,
                    effect,
                    request_schema: self_contained_schema(schema["request"].clone(), &definitions),
                    result_schema: self_contained_schema(schema["result"].clone(), &definitions),
                    error_schema: serde_json::json!({"type":"string","minLength":1,
                    "description":"Stable native error code; not execution authority"}),
                    precondition,
                    evidence,
                    cancellation: "single bounded host call; no background task",
                }
            },
        )
        .collect();

    CapabilityCatalog {
        schema_version: SCHEMA_VERSION,
        workspace: "lu-ide-standalone",
        connected_to_lu: false,
        descriptions_only: true,
        schema_definitions: definitions,
        capabilities,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn typed_schemas_cover_every_real_capability() {
        let catalog = list_capabilities();
        assert_eq!(catalog.schema_version, 2);
        assert!(!catalog.connected_to_lu);
        assert!(catalog.descriptions_only);
        assert_eq!(catalog.capabilities.len(), 46);
        assert!(catalog.capabilities.len() <= 48);
        let mut ids = HashSet::new();
        let mut commands = HashSet::new();
        for item in &catalog.capabilities {
            assert_eq!(item.version, 2);
            assert!(ids.insert(item.id));
            assert!(commands.insert(item.command));
            assert!(item.command.starts_with("ide_"));
            assert!(!item.precondition.is_empty());
            assert!(!item.evidence.is_empty());
            assert_eq!(item.request_schema["type"], "object");
            assert_eq!(item.request_schema["additionalProperties"], false);
            assert!(item.request_schema["required"].is_array());
            assert!(item.request_schema["properties"].is_object());
            let required = item.request_schema["required"].as_array().unwrap();
            assert_eq!(
                required.len(),
                item.request_schema["properties"].as_object().unwrap().len()
            );
            for property in item.request_schema["properties"]
                .as_object()
                .unwrap()
                .values()
            {
                assert!(property.get("type").is_some() || property.get("$ref").is_some());
            }
            assert!(
                item.result_schema.get("type").is_some()
                    || item.result_schema.get("$ref").is_some()
            );
            assert!(matches!(
                item.effect,
                "read" | "network_read" | "session_write" | "catalog_write" | "file_write"
            ));
        }
        assert_eq!(catalog.schema_definitions.as_object().unwrap().len(), 42);
    }

    #[test]
    fn typed_nested_results_and_file_write_boundaries() {
        let items = list_capabilities().capabilities;
        let save = items.iter().find(|x| x.id == "source.save").unwrap();
        let stage = items.iter().find(|x| x.id == "proposal.stage").unwrap();
        let spans = items.iter().find(|x| x.id == "source.readSpan").unwrap();
        assert_eq!(save.effect, "file_write");
        assert_eq!(stage.effect, "read");
        assert_eq!(save.result_schema["$ref"], "#/$defs/TextPreview");
        assert!(save.result_schema["$defs"]["TextPreview"].is_object());
        assert_eq!(stage.result_schema["$ref"], "#/$defs/ApprovedStaging");
        assert_eq!(
            spans.request_schema["properties"]["firstLine"]["type"],
            "integer"
        );
        assert_eq!(spans.result_schema["$ref"], "#/$defs/SourceSpan");
        assert!(save.precondition.contains("explicit human Save"));
        assert!(stage.evidence.contains("no file write"));
    }
}
