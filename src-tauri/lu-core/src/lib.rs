//! Lu's general-purpose, Tauri-independent core contracts.
//!
//! Foundation only: this crate does not replace Jan's Cowork agent loop,
//! expose new IPC commands, load workspace packages, or execute untrusted code.
//! Registering a native tool never requires an MCP server.
//!
//! The internal payload envelope names a Protobuf message type. Later adapters
//! will decode and validate generated Protobuf messages at each boundary; this
//! crate deliberately does not pretend opaque bytes have been validated.

// Unsafe is denied across Lu Core. Only the isolated, reviewed native
// libarchive FFI adapter is allowed to bind an OS-owned shared library.
#![deny(unsafe_code)]

pub mod agent_tool_loop;
pub mod agent_plan_ledger;
pub mod agent_plan_revisions;
pub mod agent_deliverable_receipts;
pub mod agent_host_scheduler;
pub mod agent_continuation_intent;
pub mod agent_replan_lineage;
pub mod agent_lineage_recovery;
pub mod agent_recovery_decision;
pub mod agent_os_custody;
pub mod agent_reviewed_broker_bridge;
pub mod ai_operations;
pub mod ansf;
pub mod ansf_live_chat;
pub mod ansf_goal_records;
pub mod ansf_source_ids;
pub mod ansf_semantic;
pub mod ansf_verified_procedures;
pub mod ansf_belief_revision;
pub mod ansf_embedding_worker;
pub mod ansf_artifact_links;
pub mod ansf_goals;
pub mod ansf_import;
pub mod ansf_library;
pub mod ansf_revisions;
pub mod ansf_sync;
pub mod benchmark_contract;
pub mod capability_router;
pub mod browser_visibility_settings;
pub mod browser_preferences;
pub mod web_http_fetch;
pub mod browser_process_owner;
pub mod browser_host_bridge;
pub mod browser_authorized_execution;
pub mod browser_model_codec;
pub mod native_browser_tool;
pub mod browser_tool_stream;
pub mod browser_model_request;
pub mod browser_worker_supervisor;
pub mod browser_chromium_launcher;
#[cfg(target_os="linux")]
pub mod browser_window_x11;
pub mod web_catalog_crossref;
pub mod web_search_openalex;
pub mod native_web_research;
pub mod web_research_evidence;
pub mod web_research_citation;
pub mod web_research_styles;
pub mod web_research_document;
pub mod web_research_report;
pub mod web_research_analysis;
pub mod web_research_premise;
pub mod web_research_sufficiency;
pub mod web_research_maturity;
pub mod web_research_planner;
pub mod native_capability_inventory;
pub mod native_observe_learn;
pub mod native_observe_egress;
pub mod native_observe_image_admission;
pub mod native_image_revision;
pub mod native_image_revision_history;
pub mod native_awareness_lane;
pub mod native_observe_ansf;
pub mod native_observe_scope;
pub mod native_gui_verification;
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
pub mod operational_asset_ledger;
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
pub mod gguf_metadata_summary;
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
pub mod model_provider_catalog;
pub mod model_import;
pub mod model_json_boundary;
pub mod model_turn_journal;
pub mod multimodal_http;
pub mod multimodal_saved_turn;
pub mod native_archive_inspect;
pub mod native_artifact_inspect;
pub mod native_external_app_action;
pub mod native_app_admission;
pub mod native_app_catalog;
pub mod native_app_execution;
pub mod native_app_output_oracle;
pub mod native_app_artifact_custody;
pub mod native_app_adapter;
pub mod native_app_version_execution;
pub mod native_app_typed_git;
pub mod native_app_typed_ffprobe;
pub mod native_reviewed_app_tool;
pub mod independent_git_history;
pub mod independent_git_pack;
pub mod independent_git_status;
pub mod independent_mp4;
pub mod reviewed_outcome_oracle;
pub mod reviewed_source_guard;
pub mod native_zip_extract;
pub mod native_archive_tree;
pub mod native_archive_batch;
pub mod native_archive_process;
pub mod native_archive_tree_tool;
pub mod native_zip_extract_tool;
pub mod native_iso9660;
pub mod native_linux_archive;
#[allow(unsafe_code)] // FFI boundary only: bounded libarchive calls and dynamic loader.
pub mod native_libarchive;
pub mod native_seven_rar;
pub mod native_rpm;
pub mod native_cpio;
pub mod native_ar_deb;
pub mod native_linux_archive_tool;
pub mod native_iso_extract_tool;
pub mod native_archive_create;
pub mod native_archive_split;
pub mod native_reader;
pub mod native_ocr;
pub mod native_reader_docx;
pub mod native_reader_pdf;
pub mod native_reader_openxml;
pub mod native_reader_office;
pub mod native_spreadsheet_create;
pub mod native_csv_edit;
pub mod native_docx_create;
pub mod native_office_revision;
pub mod native_office_replace_all;
pub mod native_office_revision_tool;
pub mod native_office_batch_tool;
pub mod native_pptx_create;
pub mod native_pptx_tool;
pub mod native_docx_tool;
pub mod native_spreadsheet_tool;
pub mod native_reader_odf;
pub mod native_reader_rtf;
pub mod native_reader_epub;
pub mod native_reader_turn;
pub mod native_workspace_read_turn;
pub mod native_reader_tool;
pub mod vision_routing;
pub mod reversible_edit;
pub mod native_background_status;
pub mod native_background_result;
pub mod native_background_control;
pub mod native_background_process;
pub mod native_background_scheduler;
pub mod native_resource_pressure;
pub mod native_file_extra;
pub mod native_file_discovery;
pub mod native_file_transfer;
pub mod native_file_transfer_tool;
pub mod native_file_trash_tool;
pub mod native_trash_custody;
pub mod native_git_inspect;
pub mod native_files;
pub mod native_atomic_write;
pub mod native_create_new;
pub mod native_edit_existing;
#[cfg(windows)]
pub mod windows_file_guard;
pub mod write_format;
pub mod write_recovery;
pub mod document_preferences;
pub mod edit_safety;
pub mod edit_approval_ledger;
pub mod rollback_approval_ledger;
pub mod ownership_store;
pub mod passive_diagnostics;
pub mod passive_localization;
pub mod persistent_llama_backend;
pub mod platform_compute;
pub mod policy;
pub mod read_approval_ledger;
pub mod write_approval_ledger;
pub mod desktop_owner;
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
