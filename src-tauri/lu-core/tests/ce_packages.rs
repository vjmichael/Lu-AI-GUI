use lu_core::{
    ce_packages::{
        CePackageError, CePackageManager, CePackageManifest, CePackageStatus, CeVersion,
    },
    CoreExtensionHost, ModuleKind, ModuleManifest, NativeToolRegistry, ToolDescriptor, ToolEffect,
    CORE_CONTRACT_VERSION,
};
use serde_json::{json, Value};

const DIGEST_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn json_manifest(id: &str, version: &str, deps: Value, tools: Value) -> String {
    json!({
        "format_version": 1,
        "id": id,
        "version": version,
        "contract_version": CORE_CONTRACT_VERSION,
        "publisher": "Lu bundled development",
        "artifact_sha256": DIGEST_ABC,
        "dependencies": deps,
        "tools": tools
    })
    .to_string()
}

fn manifest(id: &str, version: &str, deps: Value) -> CePackageManifest {
    CePackageManifest::parse(&json_manifest(id, version, deps, json!([]))).unwrap()
}

fn module(id: &str) -> ModuleManifest {
    ModuleManifest {
        id: id.into(),
        display_name: id.into(),
        contract_version: CORE_CONTRACT_VERSION,
        kind: ModuleKind::CoreExtension,
        tools: vec![],
        panels: vec![],
        workflows: vec![],
    }
}

fn manager() -> (CePackageManager, NativeToolRegistry) {
    let registry = NativeToolRegistry::new();
    (
        CePackageManager::new(CoreExtensionHost::new(registry.clone())),
        registry,
    )
}

#[test]
fn strict_manifest_rejects_unknown_fields_and_ambiguous_versions() {
    let mut obj: Value =
        serde_json::from_str(&json_manifest("lu.alpha", "1.2.3", json!([]), json!([]))).unwrap();
    obj["exec_path"] = json!("/bin/sh");
    assert!(matches!(
        CePackageManifest::parse(&obj.to_string()),
        Err(CePackageError::InvalidManifest(_))
    ));
    for version in ["1", "01.2.3", "1.2.3-beta", "1.2.3+build", "1.2.-3"] {
        assert!(CeVersion::parse(version).is_err(), "{version}");
    }
    assert!(CePackageManifest::parse(&json_manifest(
        "lu.alpha",
        "1.2.3",
        json!([{"id":"lu.alpha","min_version":"1.0.0"}]),
        json!([])
    ))
    .is_err());
}

#[test]
fn external_archive_remains_quarantined_even_with_matching_digest() {
    let (manager, registry) = manager();
    let raw = json_manifest("lu.external", "1.0.0", json!([]), json!([]));
    assert_eq!(
        manager.stage_external(&raw, b"wrong"),
        Err(CePackageError::DigestMismatch)
    );
    manager.stage_external(&raw, b"abc").unwrap();
    assert_eq!(
        manager.get("lu.external").unwrap().unwrap().status,
        CePackageStatus::Quarantined
    );
    assert_eq!(
        manager.enable_bundled_trusted("lu.external", module("lu.external"), vec![]),
        Err(CePackageError::QuarantinedCannotExecute)
    );
    assert!(registry.modules().unwrap().is_empty());
}

#[test]
fn dependencies_require_compatible_enabled_packages_and_disable_in_reverse_order() {
    let (manager, registry) = manager();
    manager
        .stage_bundled_trusted(manifest("lu.alpha", "1.2.0", json!([])))
        .unwrap();
    manager
        .stage_bundled_trusted(manifest(
            "lu.beta",
            "1.0.0",
            json!([
                {"id":"lu.alpha","min_version":"1.1.0","max_exclusive":"2.0.0"}
            ]),
        ))
        .unwrap();
    assert_eq!(
        manager.dependency_order("lu.beta").unwrap(),
        vec!["lu.alpha", "lu.beta"]
    );
    assert_eq!(
        manager.enable_bundled_trusted("lu.beta", module("lu.beta"), vec![]),
        Err(CePackageError::MissingDependency("lu.alpha".into()))
    );
    manager
        .enable_bundled_trusted("lu.alpha", module("lu.alpha"), vec![])
        .unwrap();
    manager
        .enable_bundled_trusted("lu.beta", module("lu.beta"), vec![])
        .unwrap();
    assert_eq!(
        manager.disable_bundled_trusted("lu.alpha"),
        Err(CePackageError::EnabledDependent("lu.beta".into()))
    );
    manager.disable_bundled_trusted("lu.beta").unwrap();
    manager.disable_bundled_trusted("lu.alpha").unwrap();
    assert!(registry.modules().unwrap().is_empty());
}

#[test]
fn missing_version_and_cycle_errors_do_not_register_any_code() {
    let (manager, registry) = manager();
    manager
        .stage_bundled_trusted(manifest(
            "lu.alpha",
            "1.0.0",
            json!([
                {"id":"lu.beta","min_version":"1.0.0"}
            ]),
        ))
        .unwrap();
    assert_eq!(
        manager.dependency_order("lu.alpha"),
        Err(CePackageError::MissingDependency("lu.beta".into()))
    );
    manager
        .stage_bundled_trusted(manifest(
            "lu.beta",
            "0.9.0",
            json!([
                {"id":"lu.alpha","min_version":"1.0.0"}
            ]),
        ))
        .unwrap();
    assert_eq!(
        manager.dependency_order("lu.alpha"),
        Err(CePackageError::DependencyVersion("lu.beta".into()))
    );
    manager
        .replace_disabled_trusted(
            "lu.beta",
            manifest(
                "lu.beta",
                "1.0.0",
                json!([
                    {"id":"lu.alpha","min_version":"1.0.0"}
                ]),
            ),
        )
        .unwrap();
    assert!(matches!(
        manager.dependency_order("lu.alpha"),
        Err(CePackageError::DependencyCycle(_))
    ));
    assert!(registry.modules().unwrap().is_empty());
}

#[test]
fn enabled_packages_block_uninstall_and_failed_upgrade_preserves_metadata() {
    let (manager, _) = manager();
    manager
        .stage_bundled_trusted(manifest("lu.alpha", "1.0.0", json!([])))
        .unwrap();
    manager
        .stage_bundled_trusted(manifest(
            "lu.beta",
            "1.0.0",
            json!([
                {"id":"lu.alpha","min_version":"1.0.0","max_exclusive":"2.0.0"}
            ]),
        ))
        .unwrap();
    assert_eq!(
        manager.uninstall_disabled("lu.alpha"),
        Err(CePackageError::EnabledDependent("lu.beta".into()))
    );
    assert_eq!(
        manager.replace_disabled_trusted("lu.alpha", manifest("lu.alpha", "2.0.0", json!([]))),
        Err(CePackageError::DependencyVersion("lu.beta".into()))
    );
    assert_eq!(
        manager.get("lu.alpha").unwrap().unwrap().manifest.version,
        CeVersion::parse("1.0.0").unwrap()
    );
}

#[test]
fn no_exec_effect_can_enter_the_bundled_registry_without_worker_isolation() {
    let (manager, registry) = manager();
    let raw = json_manifest(
        "lu.terminal",
        "1.0.0",
        json!([]),
        json!(["lu.terminal.run"]),
    );
    manager
        .stage_bundled_trusted(CePackageManifest::parse(&raw).unwrap())
        .unwrap();
    let mut m = module("lu.terminal");
    m.tools.push(ToolDescriptor {
        id: "lu.terminal.run".into(),
        display_name: "Run".into(),
        effect: ToolEffect::ExecuteCode,
        input_type_url: "type.googleapis.com/lu.v1.LegacyJsonArguments".into(),
        output_type_url: "type.googleapis.com/lu.v1.LegacyToolResult".into(),
    });
    assert_eq!(
        manager.enable_bundled_trusted("lu.terminal", m, vec![]),
        Err(CePackageError::IsolationRequired)
    );
    assert!(registry.modules().unwrap().is_empty());
}
