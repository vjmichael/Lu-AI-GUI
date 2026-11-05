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
        ($id, $command, $title, $description, $effect, $precondition, $evidence)
    };
}

type CapabilityMeta = (&'static str, &'static str, &'static str,
    &'static str, &'static str, &'static str, &'static str);

fn metadata() -> Vec<CapabilityMeta> {
    vec![
            capability!("inventory.capture", "ide_capture_project_inventory", "Record verified inventory snapshot",
                "Persist observed artifact paths and hashes for future comparison; no execution.",
                "catalog_write", "open project and explicit user snapshot request",
                "historical observed byte hashes; no software usability claim"),
            capability!("inventory.compare", "ide_compare_project_inventory", "Compare inventory",
                "Read-only comparison of latest bounded project evidence with saved baseline.",
                "read", "open project; baseline optional",
                "matched/changed/missing/new/unverifiable observed bytes"),
            capability!("git.status", "ide_read_git_status", "Inspect source control",
                "Read local Git modified, staged and untracked paths with trusted native libgit2.",
                "read", "registered open project; no network or Git write",
                "bounded local worktree status and stored upstream comparison"),
            capability!("continuity.read", "ide_read_project_context", "Read project continuity",
                "Read bounded host observations and revisioned untrusted project notes.",
                "read", "registered and active project",
                "historical events, document revisions and explicitly unknown external state"),
            capability!("project.list", "ide_list_projects", "List projects",
                "Read IDE-registered project summaries.", "read", "IDE host available", "registered project summaries"),
            capability!("project.select", "ide_choose_project", "Choose project folder",
                "Open native OS directory picker and register selected project.", "catalog_write", "explicit human picker selection", "updated project catalog"),
            capability!("project.open", "ide_open_project", "Open registered project",
                "Activate an existing project in the IDE.", "session_write", "registered project ID", "active project summary"),
            capability!("project.close", "ide_close_project", "Close project",
                "Close an open IDE project, without deleting its files.", "session_write", "open project ID", "inactive project summary"),
            capability!("project.unregister", "ide_unregister_project", "Remove project from IDE",
                "Remove project registration only; never remove files on disk.", "catalog_write", "registered project ID and user intent", "catalog without project"),
            capability!("source.list", "ide_list_source_directory", "List source directory",
                "Capability-scoped, bounded listing of one directory.", "read", "registered and open project", "relative paths and truncation flag"),
            capability!("source.preview", "ide_preview_source_file", "Read source file",
                "Read bounded UTF-8 text and its saved SHA-256 revision.", "read", "open project and safe relative path", "text and saved source revision"),
            capability!("source.readSpan", "ide_read_source_span", "Read source lines",
                "Read exact numbered lines from a specified saved revision.", "read", "open project and matching saved revision", "line text, UTF-8 byte offsets and newline identity"),
            capability!("source.review", "ide_review_source_change", "Review source change",
                "Calculate bounded diff preview without saving.", "read", "matching saved revision and bounded proposed text", "diff hunks with exactness and truncation"),
            capability!("source.save", "ide_save_source_file", "Save source file",
                "Explicit host revision-checked file write; discovery does not authorize it.", "file_write", "explicit human Save and matching saved revision", "post-write file content and revision"),
            capability!("proposal.import", "ide_import_proposal", "Import proposal",
                "Validate untrusted proposal as data in volatile inbox.", "session_write", "bounded valid proposal", "inbox summary; no filesystem write"),
            capability!("proposal.list", "ide_list_proposals", "List proposals",
                "Inspect volatile proposal inbox.", "read", "IDE host available", "proposal summaries"),
            capability!("proposal.review", "ide_review_proposal", "Review proposal",
                "Read verified proposal details and bounded diffs.", "read", "existing inbox item", "preview with exactness and truncation"),
            capability!("proposal.decide", "ide_decide_proposal", "Decide proposal",
                "Record human approval for staging only or rejection.", "session_write", "human review and unchanged proposal digest", "recorded review decision; no file write"),
            capability!("proposal.stage", "ide_stage_proposal", "Stage proposal",
                "Return approved edit data for unsaved editor drafts only.", "read", "previous human approval and current source revisions", "validated staging data; no file write"),
            capability!("proposal.discard", "ide_discard_proposal", "Discard proposal",
                "Remove an inbox item without changing source files.", "session_write", "existing inbox item", "updated volatile inbox")
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
        properties.insert("$schema".to_owned(), Value::String(
            "https://json-schema.org/draft/2020-12/schema".to_owned(),
        ));
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
        .map(|(id, command, title, description, effect, precondition, evidence)| {
            let schema = operations.get(id)
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
        })
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
        assert_eq!(catalog.capabilities.len(), 20);
        assert!(catalog.capabilities.len() <= 32);
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
            assert_eq!(required.len(), item.request_schema["properties"].as_object().unwrap().len());
            for property in item.request_schema["properties"].as_object().unwrap().values() {
                assert!(property.get("type").is_some() || property.get("$ref").is_some());
            }
            assert!(item.result_schema.get("type").is_some()
                || item.result_schema.get("$ref").is_some());
            assert!(matches!(item.effect,
                "read" | "session_write" | "catalog_write" | "file_write"));
        }
        assert_eq!(catalog.schema_definitions.as_object().unwrap().len(), 25);
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
        assert_eq!(spans.request_schema["properties"]["firstLine"]["type"], "integer");
        assert_eq!(spans.result_schema["$ref"], "#/$defs/SourceSpan");
        assert!(save.precondition.contains("explicit human Save"));
        assert!(stage.evidence.contains("no file write"));
    }
}
