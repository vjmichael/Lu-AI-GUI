//! Lu's general-purpose, Tauri-independent core contracts.
//!
//! Foundation only: this crate does not replace Jan's Cowork agent loop,
//! expose new IPC commands, load workspace packages, or execute untrusted code.
//! Registering a native tool never requires an MCP server.
//!
//! The internal payload envelope names a Protobuf message type. Later adapters
//! will decode and validate generated Protobuf messages at each boundary; this
//! crate deliberately does not pretend opaque bytes have been validated.

#![forbid(unsafe_code)]

pub mod agent_tool_loop;
pub mod ansf;
pub mod ansf_import;
pub mod ansf_library;
pub mod ansf_revisions;
pub mod ansf_sync;
pub mod benchmark_contract;
pub mod capability_router;
pub mod ce_approval;
pub mod ce_broker;
pub mod ce_packages;
pub mod ce_permissions;
pub mod ce_schema;
pub mod ce_signing;
pub mod ce_staging;
pub mod ce_turn;
pub mod compute_inventory;
pub mod compute_probe;
pub mod compute_runtime;
pub mod continuity;
pub mod contracts;
pub mod conversation_authority;
pub mod core_extension;
pub mod cpu_instructions;
pub mod cpu_topology;
pub mod cross_chat_reference;
pub mod diagnostic_contract;
pub mod diagnostic_localization;
pub mod durable;
pub mod export_contract;
pub mod gguf_preflight;
pub mod host_resource_authority;
pub mod host_session;
pub mod host_typed_evidence;
pub mod inference_bridge;
pub mod inference_host;
pub mod inference_image_refs;
pub mod inference_supervisor;
pub mod inference_wire;
pub mod journal;
pub mod linux_device_hints;
pub mod linux_gpu_nodes;
pub mod llama_cli_backend;
pub mod mmproj_admission;
pub mod model;
pub mod model_admission;
pub mod model_import;
pub mod model_json_boundary;
pub mod model_turn_journal;
pub mod multimodal_http;
pub mod multimodal_saved_turn;
pub mod native_file_extra;
pub mod native_files;
pub mod ownership_store;
pub mod passive_diagnostics;
pub mod passive_localization;
pub mod persistent_llama_backend;
pub mod platform_compute;
pub mod policy;
pub mod privilege_guard;
pub mod projector_identity;
pub mod saved_chat_prompt;
pub mod supervised_tool_generation;
pub use policy::{CapabilityGrant, PolicyEngine, PolicyError};
pub mod recovery;
pub mod registry;
pub mod tasks;
pub mod wire;
pub mod workspace;
pub mod workspace_quarantine;

pub use ansf::{AnsfError, AnsfStore, Chapter as AnsfChapter, Entry as AnsfEntry, EntryRole};
pub use ansf_import::{
    import_jan_jsonl, import_jan_jsonl_with_policy, ImportError as AnsfImportError, ImportStats,
    JanImportPolicy,
};
pub use ansf_library::{AnsfLibrary, BookIndex, ChapterIndex, LibraryError};
pub use ansf_revisions::{CurrentMessage, RevisionLink, RevisionOrigin};
pub use ansf_sync::{reconcile_book_index, sync_verified_snapshot, SyncError};
pub use ce_approval::{CeApprovalGate, CeApprovalPrompt, TrustedCeApprovalSource};
pub use ce_broker::{CeBrokerError, CeResourceBroker, MAX_BROKER_FILE_BYTES};
pub use ce_packages::{
    CeDependency, CePackageError, CePackageManager, CePackageManifest, CePackageRecord,
    CePackageStatus, CeVersion,
};
pub use ce_permissions::{
    CeGrantAuthorizer, CeGrantRequest, CePermissionError, CePermissionStore,
    CePublisherKeyEnrollment, CeRevocation,
};
pub use ce_schema::{CeSchemaError, CeToolSchema};
pub use ce_signing::{CePublisherTrust, SignatureError, VerifiedCePackage};
pub use ce_staging::{CeExecutableStager, CeStageError, StagedCeExecutable};
pub use ce_turn::PersistedJanTurnVerifier;
pub use continuity::{ContinuityError, ContinuityStore, TaskCheckpoint};
pub use contracts::{
    AuthorizationError, ModuleKind, ModuleManifest, NativeTool, PanelContribution, ProtoPayload,
    ToolAuthorizer, ToolDescriptor, ToolEffect, ToolFailure, ToolFuture, ToolInvocation,
    ToolOutput, WorkflowContribution, CORE_CONTRACT_VERSION,
};
pub use conversation_authority::{
    AuthorityError, AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
    TrustedUserTurnVerifier,
};
pub use core_extension::{CoreExtensionError, CoreExtensionHost};
pub use durable::DurableDispatcher;
pub use host_resource_authority::{
    AuthorizedResource, HostPrincipal, HostResourceAuthority, ResourceAction, ResourceDenial,
    ResourceRecord, Sensitivity,
};
pub use journal::{EffectJournal, EffectRecord, EffectState, JournalError, PrepareOutcome};
pub use ownership_store::{OwnershipStore, OwnershipStoreError};
pub use registry::{DispatchError, NativeToolRegistry, RegistryError, ToolResponse};
pub use tasks::{TaskRecord, TaskState, TaskStore, TaskStoreError};
pub use workspace::{WorkspaceError, WorkspaceHost, WorkspaceInstance, WorkspaceState};
pub use workspace_quarantine::{IntakeReceipt, IntakeStatus, QuarantineError, WorkspaceQuarantine};
