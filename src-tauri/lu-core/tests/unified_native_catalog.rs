//! Integration proof: existing Reader, ZIP and background observation share
//! one registry/catalog while the host still controls exact per-turn exposure.

use std::{collections::BTreeSet, sync::Arc};
use lu_core::{
    model::ModelToolCatalog,
    native_archive_inspect::register_archive_inspection_tool,
    native_background_result::register_model_background_observation,
    native_reader_tool::register_reader_pinned,
    registry::NativeToolRegistry,
    tasks::TaskStore,
};

#[test]
fn independent_domains_compose_without_ambient_model_authority() {
    let registry = NativeToolRegistry::new();
    // The trusted host has pinned the *contents*, not an arbitrary path.
    let mut model = register_reader_pinned(
        &registry, "approved.txt", b"Host-selected read-only text".to_vec()
    ).unwrap();
    // A host-selected directory is passed as a native capability, never
    // provided by the model or inferred from the chat message.
    register_archive_inspection_tool(
        &registry, &mut model, &std::env::temp_dir()
    ).unwrap();
    let task_store = Arc::new(TaskStore::open(
        std::path::Path::new(":memory:")
    ).unwrap());
    let background = register_model_background_observation(
        &registry, task_store
    ).unwrap();
    model.absorb_registered(&registry, background).unwrap();

    let selected = BTreeSet::from([
        "lu_reader_read".into(),
        "lu_archive_list".into(),
    ]);
    let schemas = model.schemas(&selected).unwrap();
    assert_eq!(schemas.len(), 2);
    assert!(schemas.iter().all(|schema|
        schema["function"]["name"] == "lu_reader_read"
        || schema["function"]["name"] == "lu_archive_list"
    ));
    // Registration never advertised status to this model turn. A valid
    // identifier cannot trick the catalog into expanding the host allowlist.
    assert!(model.encode_call(&selected, "lu_background_status", "call-2",
        Some("c-owner"), r#"{"task_id":"task-1"}"#).is_err());
    assert!(model.encode_call(&selected, "lu_archive_list", "call-3",
        Some("c-owner"), r#"{"path":"../other.zip"}"#).is_err());
    assert!(model.encode_call(&selected, "lu_reader_read", "call-4",
        Some("c-owner"), r#"{"path":"unapproved.txt"}"#).is_ok());
    // The catalog can encode a request, but the registry's policy and
    // ConversationAuthority remain the only source of execution grants.
}

#[test]
fn cannot_absorb_a_catalog_whose_native_modules_are_not_installed() {
    let source = NativeToolRegistry::new();
    let from_other_registry = register_reader_pinned(
        &source, "a.txt", b"never admitted".to_vec()
    ).unwrap();
    let target = NativeToolRegistry::new();
    let mut catalog = ModelToolCatalog::new();
    assert!(catalog.absorb_registered(&target, from_other_registry).is_err());
    assert!(catalog.schemas(&BTreeSet::new()).unwrap().is_empty());
}
