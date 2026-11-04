//! Independent Lu desktop host. No Jan commands, plugins or route handlers.
//! WebView receives opaque model/image/session IDs only. All disk selection is
//! from native OS dialogs; only host code can resolve the signed engine package.
mod chat_store;
mod generation_queue;
mod load_trace;
mod native_host_read;
mod read_proposal;
mod write_proposal;
mod native_host_write;
mod edit_proposal;
mod native_host_edit;
mod rollback_review;
mod prompt_context;
mod native_browser_host;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chat_store::{ChatFolder, ChatMessage, ChatStore, ChatSummary};
use generation_queue::{CancelState, GenerationQueue};
use load_trace::{LoadTrace, LoadTraceView};
use lu_core::{
    inference_bridge::BridgeError,
    inference_supervisor::{SupervisorError, TrustedInferenceSupervisor},
    inference_wire::decode_frame,
    mmproj_admission::admit_projector,
    model_admission::ModelAdmissionManager,
    model_import::{import_selected_gguf, import_selected_projector},
    multimodal_saved_turn::{admit_data_url, PinnedImage, MAX_TOTAL_IMAGE_BYTES},
    persistent_llama_backend::{PersistentEngineConfig, PersistentLlamaBackend},
    projector_identity::projector_filename_for_model,
    wire::proto,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

const ENGINE_SHA256: &str = "36155bc14f8eb02224105377c946528ccac91337d715386a77d6187cb98105b4";
const ENGINE_FILES: [(&str, &str, u64); 4] = [
    ("llama-server.exe", ENGINE_SHA256, 55_099_904),
    (
        "msvcp140.dll",
        "0f885b509a685d2bbfa652fed26b5fb31d88fbdab0a978c641d1c7b8aa460aa9",
        557_728,
    ),
    (
        "vcruntime140.dll",
        "d5e4d9a3e835fa679450145d6a7d94e36573a509317111904d9b3712c30d9066",
        124_544,
    ),
    (
        "vcruntime140_1.dll",
        "1f2d41c4aa5db0bc33ebf7b66d72943a817d7ce6cbe880502a9403823633093f",
        49_792,
    ),
];
const MAX_CATALOG_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredModel {
    model_ref: String,
    filename: String,
    /// None is the R14/R15 legacy flat layout. New imports live in their
    /// own opaque model-ID directory; this is never a user-provided path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_subdir: Option<String>,
    projector_attached: bool,
}

fn model_directory(root: &Path, model: &StoredModel) -> PathBuf {
    match &model.model_subdir {
        Some(subdir) => root.join(subdir),
        None => root.to_path_buf(),
    }
}

fn is_real_directory(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.file_type().is_dir() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelView {
    model_ref: String,
    filename: String,
    projector_attached: bool,
    available: bool,
}
impl ModelView {
    fn for_record(v: &StoredModel, dir: &Path) -> Self {
        Self {
            model_ref: v.model_ref.clone(),
            filename: v.filename.clone(),
            projector_attached: v.projector_attached,
            available: {
                let folder = model_directory(dir, v);
                is_real_directory(&folder)
                    && fs::symlink_metadata(folder.join(&v.filename))
                        .is_ok_and(|meta| meta.file_type().is_file())
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeView {
    model_ref: Option<String>,
    session_id: Option<String>,
    generation_id: Option<String>,
    phase: String,
    pid: Option<u32>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageView {
    image_ref: String,
    mime_type: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatEvent {
    chat_id: String,
    generation_id: String,
    text: String,
    index: u32,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationEvent {
    chat_id: String,
    generation_id: String,
    phase: &'static str,
    position: usize,
}

/// Host-only browser task; the browser never chooses this identity or scope.
struct ActiveBrowserTask {
    task_id:String,
    chat_id:String,
    user_turn_id:String,
    origins:Vec<String>,
}
struct DesktopState {
    models_dir: PathBuf,
    catalog_path: PathBuf,
    chats: ChatStore,
    read_proposals: read_proposal::ReadApprovalQueue,
    write_proposals: write_proposal::WriteProposalQueue,
    write_approval_ledger: lu_core::write_approval_ledger::WriteApprovalLedger,
    edit_proposals: edit_proposal::EditProposalQueue,
    edit_approval_ledger: lu_core::edit_approval_ledger::EditApprovalLedger,
    rollback_reviews: rollback_review::RollbackReviewQueue,
    rollback_approval_ledger: lu_core::rollback_approval_ledger::RollbackApprovalLedger,
    documents_os_dir: PathBuf,
    document_preferences: lu_core::document_preferences::DocumentPreferences,
    browser_preferences: lu_core::browser_preferences::BrowserPreferences,
    browser_runtime: native_browser_host::DesktopBrowserHost,
    browser_authority: lu_core::conversation_authority::ConversationAuthority,
    browser_task: Mutex<Option<ActiveBrowserTask>>,
    _owner: lu_core::desktop_owner::DesktopOwner,
    effects: lu_core::journal::EffectJournal,
    read_approval_ledger: lu_core::read_approval_ledger::ReadApprovalLedger,
    catalog: Mutex<Vec<StoredModel>>,
    images: Mutex<HashMap<String, PinnedImage>>,
    generation_queue: Arc<GenerationQueue>,
    load_trace: Arc<Mutex<LoadTrace>>,
    supervisor: Arc<TrustedInferenceSupervisor<PersistentLlamaBackend>>,
}

fn next_id(prefix: &str) -> String {
    let mut random = [0u8; 16];
    OsRng.fill_bytes(&mut random);
    let mut s = prefix.to_owned();
    for byte in random {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}
fn decode_sha(raw: &str) -> Result<[u8; 32], String> {
    if raw.len() != 64 {
        return Err("invalid pinned engine SHA-256 length".into());
    }
    let mut bytes = [0; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&raw[i * 2..i * 2 + 2], 16)
            .map_err(|_| "invalid pinned engine SHA-256")?;
    }
    Ok(bytes)
}
fn check_file(path: &Path, sha: &str, bytes: u64) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "required Lu engine file missing")?;
    if !meta.file_type().is_file() || meta.len() != bytes {
        return Err(format!(
            "invalid bundled engine component: {}",
            path.display()
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("engine component is a reparse point".into());
        }
    }
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    if h.finalize().as_slice() != decode_sha(sha)? {
        return Err("Lu engine component hash mismatch".into());
    }
    Ok(())
}

fn checked_catalog(path: &Path) -> Result<Vec<StoredModel>, String> {
    let Ok(meta) = fs::metadata(path) else {
        return Ok(Vec::new());
    };
    if meta.len() > MAX_CATALOG_BYTES {
        return Err("model catalog exceeds limit".into());
    }
    let data = fs::read(path).map_err(|e| e.to_string())?;
    let models: Vec<StoredModel> = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    if models.len() > 64 {
        return Err("too many registered models".into());
    }
    let mut refs = HashSet::new();
    let mut filenames = HashSet::new();
    for model in &models {
        projector_filename_for_model(&model.model_ref)
            .map_err(|_| "invalid stored model identity")?;
        let name = Path::new(&model.filename);
        if model.filename.len() > 200
            || name.file_name().and_then(|n| n.to_str()) != Some(&model.filename)
            || model.filename.contains(['/', '\\', ':'])
            || !model.filename.to_ascii_lowercase().ends_with(".gguf")
            || model
                .model_subdir
                .as_ref()
                .is_some_and(|subdir| subdir != &model.model_ref)
            || !refs.insert(model.model_ref.clone())
            || !filenames.insert((model.model_subdir.clone(), model.filename.clone()))
        {
            return Err("invalid or duplicate model catalog entry".into());
        }
    }
    Ok(models)
}
fn persist_catalog(path: &Path, records: &[StoredModel]) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(records).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_CATALOG_BYTES {
        return Err("catalog too large".into());
    }
    let temp = path.with_file_name(format!(".model-index-{}.tmp", next_id("")));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        f.write_all(&bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
/// Scan an admitted directory for one main GGUF. The optional directory ID
/// already passed catalog validation and never comes from the WebView.
fn scan_one_directory(
    dir: &Path,
    subdir: Option<&str>,
    records: &mut Vec<StoredModel>,
) -> Result<usize, String> {
    if !is_real_directory(dir) {
        return Ok(0);
    }
    let mut added = 0;
    for item in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = item.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let Some(filename) = name.to_str() else {
            continue;
        };
        let lowered = filename.to_ascii_lowercase();
        if !lowered.ends_with(".gguf")
            || lowered.starts_with("mmproj")
            || filename.len() > 200
            || records
                .iter()
                .any(|r| r.model_subdir.as_deref() == subdir && r.filename == filename)
        {
            continue;
        }
        let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        if !meta.file_type().is_file() {
            continue;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        if records.len() >= 64 {
            break;
        }
        let mut admission = ModelAdmissionManager::new();
        if admission.admit(dir, filename).is_err() {
            continue;
        }
        let id = match subdir {
            Some(folder) => folder.to_owned(),
            None => {
                let digest = Sha256::digest(filename.as_bytes());
                format!(
                    "m-{}",
                    digest[..16]
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                )
            }
        };
        if records.iter().any(|r| r.model_ref == id) {
            continue;
        }
        records.push(StoredModel {
            model_ref: id,
            filename: filename.into(),
            model_subdir: subdir.map(str::to_owned),
            projector_attached: false,
        });
        added += 1;
        // Only one main model per opaque ID folder. Additional main GGUFs
        // require separate directories to avoid ambiguous projector bindings.
        if subdir.is_some() {
            break;
        }
    }
    Ok(added)
}

/// Discover loose legacy GGUFs and first-level model-ID folders; this never
/// starts llama-server, loads model weights, or moves existing user files.
fn scan_unindexed_models(
    models_dir: &Path,
    records: &mut Vec<StoredModel>,
) -> Result<usize, String> {
    if !is_real_directory(models_dir) {
        return Err("unsafe models directory".into());
    }
    let mut added = scan_one_directory(models_dir, None, records)?;
    for entry in fs::read_dir(models_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let Some(folder) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if projector_filename_for_model(&folder).is_err() || !is_real_directory(&entry.path()) {
            continue;
        }
        added += scan_one_directory(&entry.path(), Some(&folder), records)?;
    }
    Ok(added)
}

impl DesktopState {
    fn new(app: &AppHandle) -> Result<Self, String> {
        let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        // Establish process-exclusive ownership before touching recoverable
        // journals; no second live Lu may mark its effects UNCERTAIN.
        let owner = lu_core::desktop_owner::DesktopOwner::acquire(
            &root.join("desktop-owner.sqlite")
        ).map_err(|e| format!("Lu app data directory already in use or unavailable: {e:?}"))?;
        let effects = lu_core::journal::EffectJournal::open(
            &root.join("native-effects.sqlite")
        ).map_err(|e| format!("effect journal unavailable: {e:?}"))?;
        let approvals = lu_core::read_approval_ledger::ReadApprovalLedger::open(
            &root.join("read-approvals.sqlite")
        ).map_err(|e| format!("approval journal unavailable: {e:?}"))?;
        let write_approvals = lu_core::write_approval_ledger::WriteApprovalLedger::open(
            &root.join("write-approvals.sqlite")
        ).map_err(|e| format!("write approvals unavailable: {e:?}"))?;
        write_approvals.reconcile_pending()
            .map_err(|e| format!("write approval reconciliation failed: {e:?}"))?;
        let edit_approvals = lu_core::edit_approval_ledger::EditApprovalLedger::open(
            &root.join("edit-approvals.sqlite")
        ).map_err(|e| format!("edit approvals unavailable: {e:?}"))?;
        edit_approvals.reconcile_pending()
            .map_err(|e| format!("edit approval reconciliation failed: {e:?}"))?;
        let rollback_approvals=lu_core::rollback_approval_ledger::RollbackApprovalLedger::open(
            &root.join("rollback-approvals.sqlite")
        ).map_err(|e|format!("rollback approvals unavailable: {e:?}"))?;
        rollback_approvals.reconcile_pending()
            .map_err(|e|format!("rollback review reconciliation failed: {e:?}"))?;
        // Native OS Documents, never a private Lu app-data folder. Windows
        // honors redirected Known Folder Documents; Linux honors XDG when
        // reported by Tauri's platform path resolver.
        #[cfg(target_os="linux")]
        let documents_os_dir = app.path().document_dir()
            .or_else(|_|app.path().home_dir().map(|home|home.join("Documents")))
            .map_err(|e|format!(
                "Cannot resolve Linux user Documents folder; choose a folder in Advanced Settings: {e}"
            ))?;
        #[cfg(not(target_os="linux"))]
        let documents_os_dir = app.path().document_dir()
            .map_err(|e|format!(
                "System Documents known folder unavailable: {e}"
            ))?;
        let document_preferences = lu_core::document_preferences::DocumentPreferences::open(
            &root.join("documents-preferences.sqlite")
        ).map_err(|e|format!("document preferences unavailable: {e:?}"))?;
        let browser_preferences=lu_core::browser_preferences::BrowserPreferences::open(
            &root.join("browser-preferences.sqlite")
        ).map_err(|e|format!("browser preferences unavailable: {e:?}"))?;
        // All old owners are excluded. Interrupted actions are never retried;
        // these remain auditable, not executable grants.
        effects.reconcile_interrupted()
            .map_err(|e| format!("native effect startup reconciliation: {e:?}"))?;
        approvals.reconcile_pending()
            .map_err(|e| format!("read approval startup reconciliation: {e:?}"))?;

        let models_dir = root.join("lu-models");
        fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;
        let engine_dir = app
            .path()
            .resource_dir()
            .map_err(|e| e.to_string())?
            .join("lu-inference");
        for (name, sha, bytes) in ENGINE_FILES {
            check_file(&engine_dir.join(name), sha, bytes)?;
        }
        let executable = engine_dir.join("llama-server.exe");
        let catalog_path = root.join("model-index.json");
        let mut models = checked_catalog(&catalog_path)?;
        if scan_unindexed_models(&models_dir, &mut models)? > 0 {
            persist_catalog(&catalog_path, &models)?;
        }
        let mut admitted_models = Vec::new();
        let mut projectors = Vec::new();
        for record in &models {
            let folder = model_directory(&models_dir, record);
            if !is_real_directory(&folder) || !folder.join(&record.filename).is_file() {
                continue;
            }
            let mut admission = ModelAdmissionManager::new();
            admission
                .admit(&folder, &record.filename)
                .map_err(|e| format!("model admission failed: {e:?}"))?;
            admitted_models.push((
                record.model_ref.clone(),
                admission.current().unwrap().clone(),
            ));
            if record.projector_attached {
                let filename = projector_filename_for_model(&record.model_ref)
                    .map_err(|e| format!("invalid projector ID: {e:?}"))?;
                let projector = admit_projector(&folder, &filename)
                    .map_err(|e| format!("projector admission failed: {e:?}"))?;
                projectors.push((record.model_ref.clone(), projector));
            }
        }
        // Observer is constructed before the supervisor so its engine callback
        // can report real child lifecycle events without acquiring engine locks.
        let load_trace = Arc::new(Mutex::new(LoadTrace::default()));
        let engine_trace = Arc::clone(&load_trace);
        let supervisor = TrustedInferenceSupervisor::new(
            "llama-vulkan".into(),
            4096,
            admitted_models,
            move || {
                let mut raw = [0u8; 32];
                OsRng.fill_bytes(&mut raw);
                let key = raw.iter().map(|b| format!("{b:02x}")).collect::<String>();
                let engine_trace_for_load = Arc::clone(&engine_trace);
                PersistentLlamaBackend::new(PersistentEngineConfig {
                    executable: executable.clone(),
                    expected_sha256: decode_sha(ENGINE_SHA256)?,
                    threads: NonZeroUsize::new(4).unwrap(),
                    context_tokens: 4096,
                    startup_timeout: Duration::from_secs(180),
                    request_timeout: Duration::from_secs(180),
                    api_key: key,
                    load_observer: Some(Arc::new(move |phase, pid| {
                        if let Ok(mut trace) = engine_trace_for_load.lock() {
                            trace.record_engine(phase, None, pid);
                        }
                    })),
                })
            },
        )
        .map_err(|e| format!("supervisor init: {e:?}"))?;
        for (model_ref, projector) in projectors {
            supervisor
                .attach_projector(&model_ref, projector)
                .map_err(|e| format!("saved projector binding: {e:?}"))?;
        }
        Ok(Self {
            models_dir,
            catalog_path,
            chats: ChatStore::open(&root)?,
            read_proposals: read_proposal::ReadApprovalQueue::default(),
            write_proposals: write_proposal::WriteProposalQueue::default(),
            write_approval_ledger: write_approvals,
            edit_proposals: edit_proposal::EditProposalQueue::default(),
            edit_approval_ledger: edit_approvals,
            rollback_reviews: rollback_review::RollbackReviewQueue::default(),
            rollback_approval_ledger: rollback_approvals,
            documents_os_dir,
            document_preferences,
            browser_preferences,
            browser_runtime: native_browser_host::DesktopBrowserHost::new(
                app.path().resource_dir().map_err(|e|format!("browser resource root unavailable: {e}"))?,
                root.clone()
            ),
            browser_authority: lu_core::conversation_authority::ConversationAuthority::new(),
            browser_task: Mutex::new(None),
            _owner: owner,
            effects,
            read_approval_ledger: approvals,
            catalog: Mutex::new(models),
            images: Mutex::new(HashMap::new()),
            generation_queue: Arc::new(GenerationQueue::default()),
            load_trace,
            supervisor: Arc::new(supervisor),
        })
    }
}

#[tauri::command]
fn lu_status(state: State<'_, Arc<DesktopState>>) -> Result<RuntimeView, String> {
    let s = state.supervisor.status().map_err(|e| format!("{e:?}"))?;
    Ok(RuntimeView {
        session_id: s.session_id,
        model_ref: s.model_ref,
        generation_id: s.generation_id,
        phase: format!("{:?}", s.phase),
        pid: s.pid,
    })
}
#[tauri::command]
fn lu_list_models(state: State<'_, Arc<DesktopState>>) -> Result<Vec<ModelView>, String> {
    let models = state.catalog.lock().map_err(|_| "catalog poisoned")?;
    Ok(models
        .iter()
        .map(|m| ModelView::for_record(m, &state.models_dir))
        .collect())
}

fn native_pick_gguf(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    let file = app
        .dialog()
        .file()
        .add_filter("GGUF models", &["gguf"])
        .blocking_pick_file();
    file.map(|p| p.into_path().map_err(|e| e.to_string()))
        .transpose()
}
#[tauri::command]
async fn lu_import_model(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
) -> Result<Option<ModelView>, String> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let Some(source) = native_pick_gguf(&app)? else {
            return Ok(None);
        };
        if !is_real_directory(&state.models_dir) {
            return Err("unsafe models root; import refused".into());
        }
        let model_ref = next_id("m-");
        let folder = state.models_dir.join(&model_ref);
        // A unique model-ID directory is owned by the host and never reused.
        fs::create_dir(&folder).map_err(|e| format!("model folder creation: {e}"))?;
        let imported = match import_selected_gguf(&folder, &source) {
            Ok(imported) => imported,
            Err(e) => {
                let _ = fs::remove_dir(&folder); // only remove if empty
                return Err(format!("GGUF import: {e:?}"));
            }
        };
        let mut catalog = state.catalog.lock().map_err(|_| "catalog poisoned")?;
        let mut admission = ModelAdmissionManager::new();
        admission
            .admit(&folder, &imported.filename)
            .map_err(|e| format!("model admission: {e:?}"))?;
        let record = StoredModel {
            model_ref: model_ref.clone(),
            filename: imported.filename,
            model_subdir: Some(model_ref),
            projector_attached: false,
        };
        state
            .supervisor
            .register_model(
                record.model_ref.clone(),
                admission.current().unwrap().clone(),
            )
            .map_err(|e| format!("model registration: {e:?}"))?;
        let mut after = catalog.clone();
        after.push(record.clone());
        persist_catalog(&state.catalog_path, &after)?;
        *catalog = after;
        Ok(Some(ModelView::for_record(&record, &state.models_dir)))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn lu_attach_projector(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
    model_ref: String,
) -> Result<Option<ModelView>, String> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let Some(source) = native_pick_gguf(&app)? else {
            return Ok(None);
        };
        let mut catalog = state.catalog.lock().map_err(|_| "catalog poisoned")?;
        let index = catalog
            .iter()
            .position(|m| m.model_ref == model_ref && !m.projector_attached)
            .ok_or("model unavailable or already paired")?;
        let folder = model_directory(&state.models_dir, &catalog[index]);
        if !is_real_directory(&folder) {
            return Err("model storage folder is unavailable or unsafe".into());
        }
        import_selected_projector(&folder, &source, &model_ref)
            .map_err(|e| format!("projector import: {e:?}"))?;
        let filename = projector_filename_for_model(&model_ref).map_err(|e| format!("{e:?}"))?;
        let projector = admit_projector(&folder, &filename)
            .map_err(|e| format!("projector admission: {e:?}"))?;
        state
            .supervisor
            .attach_projector(&model_ref, projector)
            .map_err(|e| format!("projector binding: {e:?}"))?;
        let mut after = catalog.clone();
        after[index].projector_attached = true;
        persist_catalog(&state.catalog_path, &after)?;
        *catalog = after;
        Ok(Some(ModelView::for_record(
            &catalog[index],
            &state.models_dir,
        )))
    })
    .await
    .map_err(|e| e.to_string())?
}
/// Admit bytes explicitly supplied through a user paste or drop gesture.
/// This path never resolves a WebView-supplied filesystem location. The shared
/// Lu Core image admission checks canonical base64, actual image structure,
/// dimensions, MIME, and bounded decoded byte size.
#[tauri::command]
fn lu_ingest_image_data_url(
    state: State<'_, Arc<DesktopState>>,
    data_url: String,
) -> Result<ImageView, String> {
    // Check the encoded payload size before running any allocation-heavy decode.
    if data_url.len() > 11 * 1024 * 1024 || !data_url.starts_with("data:image/") {
        return Err("unsupported or oversized image attachment".into());
    }
    let pinned =
        admit_data_url(&data_url).map_err(|error| format!("image admission: {error:?}"))?;
    register_image(state.inner(), pinned)
}

fn register_image(state: &Arc<DesktopState>, pinned: PinnedImage) -> Result<ImageView, String> {
    let mut images = state.images.lock().map_err(|_| "image registry poisoned")?;
    if images.len() >= 4 && !images.contains_key(&pinned.image_ref) {
        return Err("four image limit".into());
    }
    let total = images
        .values()
        .map(|image| image.bytes.len())
        .sum::<usize>();
    if !images.contains_key(&pinned.image_ref) && total + pinned.bytes.len() > MAX_TOTAL_IMAGE_BYTES
    {
        return Err("total image size limit".into());
    }
    let view = ImageView {
        image_ref: pinned.image_ref.clone(),
        mime_type: pinned.mime_type.into(),
    };
    images.insert(pinned.image_ref.clone(), pinned);
    Ok(view)
}

#[tauri::command]
async fn lu_pick_image(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
) -> Result<Option<ImageView>, String> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let file = app
            .dialog()
            .file()
            .add_filter("Image", &["png", "jpg", "jpeg"])
            .blocking_pick_file();
        let Some(file) = file else { return Ok(None) };
        let path = file.into_path().map_err(|e| e.to_string())?;
        let mut f = File::open(path).map_err(|e| e.to_string())?;
        let size = f.metadata().map_err(|e| e.to_string())?.len();
        if size == 0 || size > 8 * 1024 * 1024 {
            return Err("unsupported image size".into());
        }
        let mut bytes = Vec::with_capacity(size as usize);
        (&mut f)
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png"
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "image/jpeg"
        } else {
            return Err("unsupported image format".into());
        };
        let pinned = admit_data_url(&format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
            .map_err(|e| format!("image admission: {e:?}"))?;
        Ok(Some(register_image(&state, pinned)?))
    })
    .await
    .map_err(|e| e.to_string())?
}
/// Releases an image selected through the trusted host picker. In-flight
/// generations own cloned pinned bytes, so clearing a UI selection cannot
/// invalidate a running request or grant access to another image.
#[tauri::command]
fn lu_release_image(
    state: State<'_, Arc<DesktopState>>,
    image_ref: String,
) -> Result<bool, String> {
    if image_ref.len() != 36
        || !image_ref.starts_with("img-")
        || !image_ref[4..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("invalid image reference".into());
    }
    let mut images = state.images.lock().map_err(|_| "image registry poisoned")?;
    Ok(images.remove(&image_ref).is_some())
}

/// A diagnostic snapshot can be polled while the blocking engine load runs.
/// It does not query the engine, acquire the model catalog lock, or expose paths.
#[tauri::command]
fn lu_load_diagnostics(state: State<'_, Arc<DesktopState>>) -> Result<LoadTraceView, String> {
    let trace = state.load_trace.lock().map_err(|_| "load trace poisoned")?;
    Ok(trace.snapshot())
}

#[tauri::command]
async fn lu_load_model(
    state: State<'_, Arc<DesktopState>>,
    model_ref: String,
) -> Result<String, String> {
    let state = Arc::clone(state.inner());
    // Register immediately, before spawn_blocking. The UI can distinguish a
    // responsive host from a stalled engine with lu_load_diagnostics.
    {
        let mut trace = state.load_trace.lock().map_err(|_| "load trace poisoned")?;
        trace.begin(next_id("load-"), model_ref.clone()).map_err(str::to_owned)?;
    }
    let worker_state = Arc::clone(&state);
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            if worker_state.generation_queue.has_jobs() {
                return Err("Finish or cancel queued generations before loading a model".into());
            }
            let catalog = worker_state.catalog.lock().map_err(|_| "catalog poisoned")?;
            if !catalog.iter().any(|m| m.model_ref == model_ref) {
                return Err("unknown model".into());
            }
            drop(catalog);
            {
                let mut trace = worker_state.load_trace.lock().map_err(|_| "load trace poisoned")?;
                trace.record("catalog_verified", None);
            }
            {
                let mut trace = worker_state.load_trace.lock().map_err(|_| "load trace poisoned")?;
                trace.record("supervisor_enter", None);
            }
            let session_id = next_id("s-");
            worker_state
                .supervisor
                .load(&model_ref, session_id.clone(), &next_id("req-"))
                .map_err(|e| format!("model load failed: {e:?}"))?;
            Ok(session_id)
        })();
        // Do not propagate raw engine diagnostics through the trace. The
        // original Result still returns to the invoking user.
        if let Ok(mut trace) = worker_state.load_trace.lock() {
            trace.finish(if result.is_ok() { "ok" } else { "error" });
        }
        result
    }).await;
    if joined.is_err() {
        if let Ok(mut trace) = state.load_trace.lock() {
            trace.finish("worker_join_error");
        }
    }
    joined.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn lu_unload_model(
    state: State<'_, Arc<DesktopState>>,
    session_id: String,
) -> Result<(), String> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        if state.generation_queue.has_jobs() {
            return Err("Finish or cancel queued generations before unloading the model".into());
        }
        state
            .supervisor
            .unload(&session_id, &next_id("req-"))
            .map_err(|e| format!("unload: {e:?}"))?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn lu_cancel(
    state: State<'_, Arc<DesktopState>>,
    session_id: String,
    generation_id: String,
) -> Result<(), String> {
    // A queued job is canceled in place. It must never cancel the distinct
    // generation currently executing on the single model session.
    match state.generation_queue.cancel(&session_id, &generation_id)? {
        CancelState::Queued => Ok(()),
        CancelState::Active => {
            match state
                .supervisor
                .cancel(&session_id, &generation_id, &next_id("req-"))
            {
                Ok(()) | Err(SupervisorError::WrongGeneration) => {
                    // A narrow pre-reservation window: ticket's cancellation bit
                    // is checked again after the supervisor reserves its turn.
                    Ok(())
                }
                Err(error) => Err(format!("cancel: {error:?}")),
            }
        }
    }
}

#[tauri::command]
async fn lu_generate(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
    session_id: String,
    chat_id: String,
    prompt: String,
    image_refs: Vec<String>,
    max_new_tokens: u32,
) -> Result<String, String> {
    let state = Arc::clone(state.inner());
    let images: Vec<PinnedImage> = {
        let registered = state.images.lock().map_err(|_| "image registry poisoned")?;
        let mut resolved = Vec::new();
        let mut seen = HashSet::new();
        for id in &image_refs {
            if !seen.insert(id) {
                return Err("duplicate image reference".into());
            }
            resolved.push(registered.get(id).ok_or("unknown image reference")?.clone());
        }
        resolved
    };
    if prompt.trim().is_empty()
        || prompt.len() > 16 * 1024
        || max_new_tokens == 0
        || max_new_tokens > 4096
    {
        return Err("invalid generation limits".into());
    }
    let runtime = state
        .supervisor
        .status()
        .map_err(|e| format!("status: {e:?}"))?;
    if runtime.session_id.as_deref() != Some(session_id.as_str()) {
        return Err("model session changed; reload before submitting".into());
    }
    let generation_id = next_id("g-");
    let request_id = next_id("req-");
    let supervisor = Arc::clone(&state.supervisor);
    // Admission is bounded and host-owned: at most one active/pending turn
    // per conversation and eight turns total for this loaded model session.
    let (ticket, position) =
        state
            .generation_queue
            .enqueue(&session_id, &chat_id, &generation_id)?;
    // Durable user journal before acknowledgment, including queued prompts.
    // Ticket drop releases the queue on any error before worker dispatch.
    state.chats.append_user_with_turn_id(&chat_id, &generation_id, &prompt)?;
    let queued = GenerationEvent {
        chat_id: chat_id.clone(),
        generation_id: generation_id.clone(),
        phase: "queued",
        position,
    };
    let _ = app.emit("lu-generation-queued", queued);
    let sid = session_id.clone();
    let gid = generation_id.clone();
    let rid = request_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let outcome = (|| -> Result<String, String> {
            ticket.wait_for_slot()?;
            if ticket.cancelled() {
                return Err("Cancelled while queued".into());
            }
            let events = state.chats.history(&chat_id)?;
            // With at most one turn per chat pending, the last user event is
            // this submission. Read history *after* waiting so later turns
            // use completed previous replies, but never duplicate ourselves.
            if !events
                .last()
                .is_some_and(|m| m.role == "user" && m.text == prompt)
            {
                return Err("conversation changed during queued generation".into());
            }
            let history = prompt_context::typed_history(&events[..events.len() - 1], &prompt);
            let model_prompt = prompt_context::model_file_creation_hint(&prompt);
            if ticket.cancelled() {
                return Err("Cancelled while queued".into());
            }
            let runtime = supervisor.status().map_err(|e| format!("status: {e:?}"))?;
            if runtime.session_id.as_deref() != Some(sid.as_str()) {
                return Err("model session changed while queued".into());
            }
            if images.is_empty() {
                supervisor
                    .reserve_generation_with_history(
                        &sid,
                        &gid,
                        &rid,
                        &model_prompt,
                        max_new_tokens,
                        &history,
                    )
                    .map_err(|e| format!("reserve: {e:?}"))?;
            } else {
                supervisor
                    .reserve_generation_images_with_history(
                        &sid,
                        &gid,
                        &rid,
                        &model_prompt,
                        max_new_tokens,
                        &images,
                        &history,
                    )
                    .map_err(|e| format!("reserve images: {e:?}"))?;
            }
            // Host cancellation can arrive between gate activation and the
            // engine reservation. Propagate the latched bit after reserving.
            if ticket.cancelled() {
                let _ = supervisor.cancel(&sid, &gid, &next_id("req-"));
            }
            let _ = app.emit(
                "lu-generation-start",
                GenerationEvent {
                    chat_id: chat_id.clone(),
                    generation_id: gid.clone(),
                    phase: "running",
                    position: 0,
                },
            );
            let mut answer = String::new();
            let mut last_checkpoint = std::time::Instant::now();
            let mut last_checkpoint_bytes = 0usize;
            let emit = |wire: Vec<u8>| -> Result<(), BridgeError> {
                let frame = decode_frame(&wire).map_err(BridgeError::Wire)?;
                if let Some(proto::inference_frame::Body::Event(event)) = frame.body {
                    if let Some(proto::inference_event::Kind::TextChunk(chunk)) = event.kind {
                        answer.push_str(&chunk.text);
                        if answer.len().saturating_sub(last_checkpoint_bytes) >= 512
                            || last_checkpoint.elapsed() >= Duration::from_millis(750)
                        {
                            state
                                .chats
                                .append_progress(&chat_id, &gid, &answer)
                                .map_err(|_| BridgeError::WrongOperation)?;
                            last_checkpoint_bytes = answer.len();
                            last_checkpoint = std::time::Instant::now();
                        }
                        app.emit(
                            "lu-token",
                            ChatEvent {
                                chat_id: chat_id.clone(),
                                generation_id: gid.clone(),
                                text: chunk.text,
                                index: chunk.chunk_index,
                            },
                        )
                        .map_err(|_| BridgeError::WrongOperation)?;
                    }
                }
                Ok(())
            };
            let result = if images.is_empty() {
                supervisor.run_reserved_with_history(
                    &sid,
                    &gid,
                    &rid,
                    model_prompt,
                    max_new_tokens,
                    history,
                    emit,
                )
            } else {
                supervisor.run_reserved_images_with_history(
                    &sid,
                    &gid,
                    &rid,
                    model_prompt,
                    max_new_tokens,
                    images,
                    history,
                    emit,
                )
            };
            match result {
                Ok(()) => {
                    state.chats.append_progress(&chat_id, &gid, &answer)?;
                    state.chats.complete(&chat_id, &gid)?;
                    // Only the completed model output (not frontend strings,
                    // retrieved documents or a partial stream) may propose
                    // a read. Staging is inert; execution requires a separate
                    // native-picker selection by the human.
                    let proposal_id = next_id("p-");
                    if let Ok(proposal) = state.read_proposals.stage(
                        &proposal_id, &chat_id, &gid, &answer,
                    ) {
                        // Persist host-bound intent BEFORE displaying a usable
                        // proposal card. No persistent record grants action.
                        if state.read_approval_ledger.stage(
                            &proposal_id, &chat_id, &gid,
                            &proposal.file_name, proposal.max_bytes,
                        ).is_ok() {
                            let _ = app.emit("lu-read-proposal", ReadProposalEvent {
                                chat_id: chat_id.clone(),
                                proposal_id,
                                file_name: proposal.file_name,
                                purpose: proposal.purpose,
                                max_bytes: proposal.max_bytes,
                            });
                        } else {
                            let _ = state.read_proposals.dismiss_for_conversation(
                                &proposal_id, &chat_id,
                            );
                        }
                    }
                    // A completed assistant message may *propose* writing
                    // text, but this is inert until a user reviews content,
                    // the exact Lu Drafts destination and confirms twice.
                    let write_id = next_id("pw-");
                    if let Ok(proposal) = state.write_proposals.stage(
                        &write_id, &chat_id, &gid, &answer
                    ) {
                        if state.write_approval_ledger.stage(
                            &write_id, &chat_id, &gid, &proposal.file_name,
                            proposal.bytes(), &proposal.digest(),
                        ).is_ok() {
                            let _ = app.emit("lu-write-proposal", WriteProposalEvent {
                                chat_id: chat_id.clone(),
                                proposal_id: write_id,
                                file_name: proposal.file_name.clone(),
                                purpose: proposal.purpose.clone(),
                                content: proposal.content.clone(),
                                bytes: proposal.bytes(),
                                sha256: proposal.digest(),
                                file_format: lu_core::write_format::identify_text_format(
                                    &proposal.file_name, &proposal.content
                                ).map(lu_core::write_format::format_label)
                                 .unwrap_or("UNVERIFIED_FORMAT").into(),
                            });
                        } else {
                            let _ = state.write_proposals.dismiss(&write_id, &chat_id);
                        }
                    }
                    // WRITE-01E.3: a completed model turn may only suggest
                    // replacement UTF-8 bytes and a basename. Selection of an
                    // existing target and approval remain native host actions.
                    let edit_id = next_id("pe-");
                    if let Ok(proposal) = state.edit_proposals.stage(
                        &edit_id, &chat_id, &gid, &answer
                    ) {
                        if state.edit_approval_ledger.stage(
                            &edit_id, &chat_id, &gid, &proposal.file_name,
                            proposal.bytes(), &proposal.digest()
                        ).is_ok() {
                            let _ = app.emit("lu-edit-proposal", EditProposalEvent {
                                chat_id: chat_id.clone(),
                                proposal_id: edit_id,
                                file_name: proposal.file_name,
                                purpose: proposal.purpose,
                                content: proposal.content,
                            });
                        } else {
                            let _ = state.edit_proposals.dismiss(&edit_id, &chat_id);
                        }
                    }
                    Ok(answer)
                }
                Err(error) => {
                    let message = format!("generation: {error:?}");
                    if message.contains("Cancelled") || message.contains("Canceled") {
                        if !answer.is_empty() {
                            state.chats.append_progress(&chat_id, &gid, &answer)?;
                        }
                        state.chats.cancel(&chat_id, &gid)?;
                    } else if !answer.is_empty() {
                        state.chats.append_progress(&chat_id, &gid, &answer)?;
                    }
                    Err(message)
                }
            }
        })();
        if outcome
            .as_ref()
            .err()
            .is_some_and(|err| err.contains("Cancelled"))
        {
            // Queued cancellation may happen before a stream exists; record
            // an explicit terminal event without fabricating assistant text.
            let _ = state.chats.cancel(&chat_id, &gid);
        }
        let phase = match &outcome {
            Ok(_) => "completed",
            Err(e) if e.contains("Cancelled") || e.contains("Canceled") => "cancelled",
            Err(_) => "failed",
        };
        let _ = app.emit(
            "lu-generation-finish",
            GenerationEvent {
                chat_id: chat_id.clone(),
                generation_id: gid.clone(),
                phase,
                position: 0,
            },
        );
        outcome
    })
    .await
    .map_err(|e| e.to_string())?
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatListView {
    chats: Vec<ChatSummary>,
    folders: Vec<ChatFolder>,
}
/// Explicit user-initiated OS dialog. The WebView cannot supply a path,
/// task grant, scope or model-generated approval.
#[tauri::command]
async fn lu_inspect_text_file(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
) -> Result<Option<native_host_read::InspectedFile>, String> {
    let state = Arc::clone(state.inner());
    let selected = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Text and source files", &[
                "txt", "md", "rs", "js", "jsx", "ts", "tsx", "json",
                "toml", "yaml", "yml", "py", "log", "csv",
            ])
            .blocking_pick_file()
            .map(|file| file.into_path().map_err(|e| e.to_string()))
            .transpose()
    })
    .await
    .map_err(|e| format!("file picker failed: {e}"))??;
    let Some(selected) = selected else { return Ok(None) };
    let turn_id = next_id("g-");
    let result = native_host_read::inspect_selected_file_with_receipt(
        &state.chats, &chat_id, &turn_id, &selected, 64 * 1024,
        Some(&state.effects),
    ).await?;
    Ok(Some(result))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WriteProposalEvent {
    chat_id: String,
    proposal_id: String,
    file_name: String,
    purpose: String,
    content: String,
    bytes: usize,
    sha256: String,
    file_format: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditProposalEvent {
    chat_id: String,
    proposal_id: String,
    file_name: String,
    purpose: String,
    content: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditRecoveryView {
    proposal_id: String,
    file_name: String,
    exact_target: Option<String>,
    original_sha256: Option<String>,
    replacement_sha256: String,
    approval: String,
    effect: String,
    effect_call_id: Option<String>,
    observation: String,
    assessment: String,
    receipt_verified: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RollbackPreviewView {
    file_name: String,
    original_sha256: String,
    replacement_sha256: String,
    current_text: String,
    restore_text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditRecoveryObservationView {
    proposal_id: String,
    observation: String,
    effect: String,
    assessment: String,
    rollback_preview: Option<RollbackPreviewView>,
    rollback_review_status: String,
    rollback_review_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
struct RollbackHistoryView {
    review_id:String,
    original_proposal_id:String,
    original_edit_id:String,
    file_name:String,
    exact_target:String,
    decision:String,
    effect:String,
    effect_call_id:Option<String>,
    restored_sha256:String,
    receipt_verified:bool,
    assessment:String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WriteDestinationView {
    exact_target: String,
    sha256: String,
    bytes: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentSettingsView {
    default_folder: String,
    system_documents_folder: String,
    is_custom: bool,
    is_available: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WriteRecoveryView {
    proposal_id: String,
    file_name: String,
    exact_target: Option<String>,
    sha256: String,
    bytes: u32,
    approval: String,
    effect: String,
    effect_call_id: Option<String>,
    observed_target: String,
    assessment: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadProposalEvent {
    chat_id: String,
    proposal_id: String,
    file_name: String,
    purpose: String,
    max_bytes: u32,
}

/// User approval is not a WebView-provided Boolean. The only approval path
/// opens a trusted OS picker, matches its selected filename to the exact
/// model proposal, and performs a separately granted native read.
#[tauri::command]
async fn lu_approve_read_proposal(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    proposal_id: String,
) -> Result<Option<native_host_read::InspectedFile>, String> {
    let state = Arc::clone(state.inner());
    let origin = state
        .read_proposals
        .origin_for_conversation(&proposal_id, &chat_id)
        .map_err(|e| format!("proposal unavailable: {e:?}"))?;
    let selected = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file()
            .add_filter("Text and source files", &[
                "txt", "md", "rs", "js", "jsx", "ts", "tsx",
                "json", "toml", "yaml", "yml", "py", "log", "csv",
            ])
            .blocking_pick_file()
            .map(|item| item.into_path().map_err(|e| e.to_string()))
            .transpose()
    })
    .await
    .map_err(|e| format!("approval picker failed: {e}"))??;
    let Some(selected) = selected else {
        let record = state.read_approval_ledger.get(&proposal_id)
            .map_err(|e| format!("approval record unavailable: {e:?}"))?
            .ok_or("unknown approval record")?;
        state.read_approval_ledger.decide(
            &proposal_id, &chat_id, &origin, &record.file_name,
            record.max_bytes, lu_core::read_approval_ledger::ReadDecision::Denied,
        ).map_err(|e| format!("approval denial not recorded: {e:?}"))?;
        state.read_proposals.dismiss_for_conversation(&proposal_id, &chat_id)
            .map_err(|e| format!("dismissal failed: {e:?}"))?;
        return Ok(None);
    };
    let approved = match state.read_proposals.approve_selected(
        &proposal_id, &chat_id, &origin, &selected,
    ) {
        Ok(approved) => approved,
        Err(error) => {
            // The in-memory proposal was consumed: record a denial if the
            // trusted ledger still names this exact originating task.
            if let Ok(Some(record)) = state.read_approval_ledger.get(&proposal_id) {
                let _ = state.read_approval_ledger.decide(
                    &proposal_id, &chat_id, &origin, &record.file_name,
                    record.max_bytes, lu_core::read_approval_ledger::ReadDecision::Denied,
                );
            }
            return Err(format!("selected file differs from requested read: {error:?}"));
        }
    };
    // Only the host may commit this user decision; a replayed/modified
    // proposal cannot pass the atomic pending-to-approved transition.
    state.read_approval_ledger.decide(
        &proposal_id, &chat_id, &origin, &approved.file_name,
        approved.max_bytes, lu_core::read_approval_ledger::ReadDecision::Approved,
    ).map_err(|e| format!("approval not durably committed: {e:?}"))?;
    let turn_id = next_id("g-");
    state.read_approval_ledger.link_effect(
        &proposal_id, &chat_id, &origin, &turn_id,
    ).map_err(|e| format!("approved effect identity not persisted: {e:?}"))?;
    native_host_read::inspect_selected_file_with_receipt(
        &state.chats, &chat_id, &turn_id, &selected, u64::from(approved.max_bytes),
        Some(&state.effects),
    ).await.map(Some)
}

#[tauri::command]
fn lu_dismiss_read_proposal(
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    proposal_id: String,
) -> Result<(), String> {
    let origin = state.read_proposals.origin_for_conversation(&proposal_id, &chat_id)
        .map_err(|e| format!("proposal unavailable: {e:?}"))?;
    let record = state.read_approval_ledger.get(&proposal_id)
        .map_err(|e| format!("approval record unavailable: {e:?}"))?
        .ok_or("unknown approval record")?;
    state.read_approval_ledger.decide(
        &proposal_id, &chat_id, &origin, &record.file_name,
        record.max_bytes, lu_core::read_approval_ledger::ReadDecision::Denied,
    ).map_err(|e| format!("proposal denial not recorded: {e:?}"))?;
    state.read_proposals
        .dismiss_for_conversation(&proposal_id, &chat_id)
        .map_err(|e| format!("proposal dismissal: {e:?}"))
}

/// A read-only Settings view. Neither model output nor WebView contents
/// may supply an arbitrary filesystem path.
/// User-facing browser visibility preference. The browser remains headed
/// in both modes; this setting confers no navigation or network authority.
#[tauri::command]
fn lu_browser_runtime_status(
    state: State<'_, Arc<DesktopState>>
) -> Result<native_browser_host::BrowserRuntimeView,String> {
    let status=state.browser_runtime.status(&state.browser_preferences)?;
    if !status.running{
        // A crashed browser must not strand an active grant that prevents
        // the next user-authorized session from starting.
        let stale=state.browser_task.lock()
            .map_err(|_|"browser task lock poisoned")?.take();
        if let Some(task)=stale{
            state.browser_authority.revoke_task(&task.task_id)
                .map_err(|e|format!("browser crash revocation failed: {e:?}"))?;
        }
    }
    Ok(status)
}

#[tauri::command]
fn lu_browser_stop(
    state: State<'_, Arc<DesktopState>>
) -> Result<bool,String> {
    // Revocation and worker shutdown are both host-only. Never leave a
    // still-valid network grant after the user stops a research browser.
    let active=state.browser_task.lock()
        .map_err(|_|"browser task lock poisoned")?.take();
    let revoke_result=active.as_ref().map(|task|
        state.browser_authority.revoke_task(&task.task_id));
    let stop_result=state.browser_runtime.stop();
    // Always terminate the owned processes even if task-state bookkeeping
    // encounters an error; failed revocation is reported after cleanup.
    if let Some(Err(e))=revoke_result {
        return Err(format!("browser stopped; grant revocation failed: {e:?}"));
    }
    stop_result
}

/// Native user approval is mandatory before any browser session starts.
/// The trusted host chooses an authenticated USER turn from the journal;
/// a web page or model cannot forge one with a message string.
#[tauri::command]
async fn lu_browser_start(
    app:AppHandle,
    state:State<'_,Arc<DesktopState>>,
    chat_id:String,
    origin:String,
)->Result<native_browser_host::BrowserRuntimeView,String>{
    use lu_core::{
        browser_authorized_execution::{BrowserOriginScope,BROWSER_READ_TOOL},
        conversation_authority::{AuthorizedCapability,TaskAuthority},
        contracts::ToolEffect,
    };
    let state=Arc::clone(state.inner());
    let scope=BrowserOriginScope::from_host_grant(vec![origin])
        .map_err(|_|"Invalid or non-public HTTPS browser origin")?;
    let user_turn_id=state.chats.latest_authenticated_user_turn_id(&chat_id)
        .ok_or("No authenticated user turn in this conversation")?;
    // Before presenting approval, check the actual bundled runtime.
    state.browser_runtime.require_launch_ready()?;
    let approved_origins=scope.origins();
    let dialog_text=format!(
        "Allow Lu to research this website in the current conversation?\n\n{}\n\nPermission: read-only navigation and page content. Form submissions, clicks and typing require a separate interaction grant.\n\nThe browser may temporarily appear for human verification.",
        approved_origins.join("\n")
    );
    let approved=tauri::async_runtime::spawn_blocking(move||{
        app.dialog().message(dialog_text)
            .title("Lu — Authorize research browser")
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "Allow website".into(),"Deny".into(),
            )).blocking_show()
    }).await.map_err(|e|format!("browser approval dialog failed: {e}"))?;
    if !approved{return Err("Browser request denied by user".into());}
    let task_id=next_id("browser-task-");
    state.browser_authority.open_user_task(
        TaskAuthority {
            conversation_id:chat_id.clone(),
            user_turn_id:user_turn_id.clone(),
            task_id:task_id.clone(),
            execution_scope_id:next_id("browser-scope-"),
            capabilities:vec![AuthorizedCapability{
                tool_id:BROWSER_READ_TOOL.into(),effect:ToolEffect::NetworkRead
            }],
            expires_at:Some(std::time::SystemTime::now()+Duration::from_secs(3600)),
        },
        &state.chats
    ).map_err(|e|format!("browser authority refused: {e:?}"))?;
    // Reserve the active task BEFORE the potentially blocking launch. Stop
    // can revoke this exact task even if a launch is still in progress.
    {
        let mut current=state.browser_task.lock()
            .map_err(|_|"browser task lock poisoned")?;
        if current.is_some(){
            let _=state.browser_authority.revoke_task(&task_id);
            return Err("A research browser task is already active".into());
        }
        *current=Some(ActiveBrowserTask{
            task_id:task_id.clone(),chat_id:chat_id.clone(),
            user_turn_id:user_turn_id.clone(),origins:approved_origins.clone()
        });
    }
    let native_state=Arc::clone(&state);
    let allowed=approved_origins.clone();
    let result=tauri::async_runtime::spawn_blocking(move||
        native_state.browser_runtime.launch_for_native_grant(
            allowed,&native_state.browser_preferences)
    ).await.map_err(|e|format!("browser supervisor panicked: {e}"))
        .and_then(|x|x);
    if let Err(error)=result{
        let mut current=state.browser_task.lock()
            .map_err(|_|"browser task lock poisoned")?;
        if current.as_ref().is_some_and(|task|task.task_id==task_id){
            current.take();
        }
        let _=state.browser_authority.revoke_task(&task_id);
        let _=state.browser_runtime.stop();
        return Err(error);
    }
    // Stop may have revoked the task during browser startup. Do not leave
    // an approved process alive after that revocation races completion.
    let still_authorized=state.browser_task.lock()
        .map_err(|_|"browser task lock poisoned")?
        .as_ref().is_some_and(|task|task.task_id==task_id);
    if !still_authorized{
        let _=state.browser_runtime.stop();
        return Err("Browser startup was cancelled by user".into());
    }
    state.browser_runtime.status(&state.browser_preferences)
}

#[tauri::command]
fn lu_browser_settings(state: State<'_, Arc<DesktopState>>) -> Result<String, String> {
    let value=state.browser_preferences.visibility()
        .map_err(|e|format!("browser preferences unavailable: {e:?}"))?;
    Ok(value.worker_mode().to_string())
}
/// This command is intended for a direct user settings event. It is not a
/// per-turn tool and never starts, navigates, or authorizes a browser.
#[tauri::command]
fn lu_set_browser_visibility(
    state: State<'_, Arc<DesktopState>>,
    mode: String
) -> Result<String,String> {
    use lu_core::browser_visibility_settings::BrowserVisibilityPreference as P;
    let preference=match mode.as_str() {
        "automatic"=>P::AutomaticHidden,
        "always-visible"=>P::AlwaysVisible,
        _=>return Err("Unsupported browser visibility mode".into()),
    };
    state.browser_preferences.set_visibility(preference)
        .map_err(|e|format!("cannot persist browser visibility: {e:?}"))?;
    Ok(preference.worker_mode().into())
}

#[tauri::command]
fn lu_document_settings(state: State<'_, Arc<DesktopState>>)
    -> Result<DocumentSettingsView, String> {
    let chosen = state.document_preferences.selected(&state.documents_os_dir);
    let is_custom = state.document_preferences.is_custom()
        .map_err(|e|format!("document preference state unavailable: {e:?}"))?;
    let folder = chosen.as_ref().unwrap_or(&state.documents_os_dir);
    Ok(DocumentSettingsView {
        default_folder: folder.to_string_lossy().into_owned(),
        system_documents_folder: state.documents_os_dir.to_string_lossy().into_owned(),
        is_custom,
        is_available: chosen.is_ok(),
    })
}

/// Only a native OS folder picker can replace the default Documents folder.
/// It is not an authorization for arbitrary future model edits.
#[tauri::command]
async fn lu_choose_default_document_folder(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
) -> Result<DocumentSettingsView, String> {
    let state = Arc::clone(state.inner());
    let selected = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().blocking_pick_folder()
            .map(|p|p.into_path().map_err(|e|e.to_string())).transpose()
    }).await.map_err(|e|format!("native folder picker failed: {e}"))??
        .ok_or("Folder selection cancelled")?;
    state.document_preferences.set_from_native_picker(&selected)
        .map_err(|e|format!("cannot set document folder: {e:?}"))?;
    let chosen = state.document_preferences.selected(&state.documents_os_dir)
        .map_err(|e|format!("document folder unavailable: {e:?}"))?;
    Ok(DocumentSettingsView {
        default_folder: chosen.to_string_lossy().into_owned(),
        system_documents_folder: state.documents_os_dir.to_string_lossy().into_owned(),
        is_custom: true,
        is_available: true,
    })
}

#[tauri::command]
fn lu_reset_default_document_folder(state: State<'_, Arc<DesktopState>>)
    -> Result<DocumentSettingsView, String> {
    let chosen = state.document_preferences
        .reset_to_system_documents(&state.documents_os_dir)
        .map_err(|e|format!("system Documents folder unavailable: {e:?}"))?;
    Ok(DocumentSettingsView {
        default_folder: chosen.to_string_lossy().into_owned(),
        system_documents_folder: chosen.to_string_lossy().into_owned(),
        is_custom: false,
        is_available: true,
    })
}

/// An Open Folder action is authorized by a *recorded, APPLIED* write receipt
/// bound to this chat, never a WebView-provided path. The file manager is
/// launched directly with a single argument, not via a shell.
#[tauri::command]
fn lu_open_created_folder(
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    proposal_id: String,
) -> Result<(), String> {
    state.chats.history(&chat_id)?;
    let record = state.write_approval_ledger.get(&proposal_id)
        .map_err(|e|format!("document receipt unavailable: {e:?}"))?
        .ok_or("Unknown created file")?;
    if record.chat_id != chat_id ||
        record.decision != lu_core::write_approval_ledger::WriteDecision::Approved {
        return Err("No approved file in this conversation".into());
    }
    let call_id = record.effect_call_id.as_deref().ok_or("Missing effect receipt")?;
    let effect = state.effects.get(&call_id)
        .map_err(|e|format!("effect receipt unavailable: {e:?}"))?
        .ok_or("No verified file creation")?;
    let folder = lu_core::document_preferences::verified_created_folder(&record, &effect)
        .map_err(|e|format!("Stored file link is not verified: {e:?}"))?;
    #[cfg(windows)]
    let executable = "explorer.exe";
    #[cfg(target_os="linux")]
    let executable = "xdg-open";
    #[cfg(target_os="macos")]
    let executable = "open";
    #[cfg(not(any(windows,target_os="linux",target_os="macos")))]
    return Err("Opening containing folders unsupported on this platform".into());
    #[cfg(any(windows,target_os="linux",target_os="macos"))]
    {
        std::process::Command::new(executable).arg(&folder)
            .spawn().map_err(|e|format!("Could not open containing folder: {e}"))?;
        Ok(())
    }
}

/// User's first review action: the destination is selected by the host,
/// NOT the model or a path supplied by the WebView.
#[tauri::command]
async fn lu_review_write_destination(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    proposal_id: String,
    destination_mode: Option<String>,
) -> Result<WriteDestinationView, String> {
    let state = Arc::clone(state.inner());
    // Destination is selected only by the trusted host. A model or WebView
    // string can pick a workflow, NEVER supply a writable filesystem path.
    let selected = match destination_mode.as_deref() {
        None | Some("user_documents") | Some("lu_drafts") =>
            state.document_preferences.selected(&state.documents_os_dir)
                .map_err(|e|format!(
                    "Default Documents folder unavailable. Choose a folder in Advanced Settings: {e:?}"
                ))?,
        Some("choose_folder") => {
            let selected = tauri::async_runtime::spawn_blocking(move || {
                app.dialog().file().blocking_pick_folder()
                    .map(|p| p.into_path().map_err(|e| e.to_string()))
                    .transpose()
            }).await.map_err(|e| format!("directory picker failed: {e}"))??;
            selected.ok_or("folder selection cancelled")?
        }
        _ => return Err("unsupported write destination workflow".into()),
    };
    let (target, origin, proposal) = state.write_proposals.choose_directory(
        &proposal_id, &chat_id, &selected,
    ).map_err(|e| format!("write review unavailable: {e:?}"))?;
    state.write_approval_ledger.choose_target(
        &proposal_id, &chat_id, &origin, &target,
    ).map_err(|e| format!("write destination review could not be recorded: {e:?}"))?;
    Ok(WriteDestinationView {
        exact_target: target,
        sha256: proposal.digest(),
        bytes: proposal.bytes(),
    })
}

/// Confirm only the currently host-selected destination. No WebView-provided
/// path, byte payload, model origin or permission is accepted here.
#[tauri::command]
async fn lu_confirm_write(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    proposal_id: String,
) -> Result<Option<native_host_write::CreatedFile>, String> {
    let state = Arc::clone(state.inner());
    let (initial_origin, initial_dir, initial_proposal) = state.write_proposals
        .review_details(&proposal_id, &chat_id)
        .map_err(|e| format!("write confirmation unavailable: {e:?}"))?;
    let target_preview = initial_dir.join(&initial_proposal.file_name)
        .to_string_lossy().into_owned();
    let sha = initial_proposal.digest();
    let confirm_text = format!(
        "Create a new file (existing files will NEVER be overwritten)?\n\nDestination: {}\nSize: {} bytes\nSHA-256: {}\n\nReview the full contents in Lu before approving. This authorization is single-use.",
        target_preview, initial_proposal.bytes(), sha,
    );
    // A WebView callback cannot count as the human's final native approval.
    // Keep this blocking modal off the main event loop.
    let user_confirmed = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().message(confirm_text)
            .title("Lu — Confirm exact new file")
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "Create new file".into(), "Cancel".into(),
            ))
            .blocking_show()
    }).await.map_err(|e| format!("native confirmation failed: {e}"))?;
    if !user_confirmed {
        state.write_approval_ledger.deny(&proposal_id, &chat_id)
            .map_err(|e| format!("native denial not recorded: {e:?}"))?;
        state.write_proposals.dismiss(&proposal_id, &chat_id)
            .map_err(|e| format!("write request not dismissed: {e:?}"))?;
        return Ok(None);
    }
    let (origin, directory, pinned_directory, proposal) = state.write_proposals.take_confirmed_pinned(
        &proposal_id, &chat_id,
    ).map_err(|e| format!("write confirmation unavailable: {e:?}"))?;
    if origin != initial_origin || directory != initial_dir
        || proposal != initial_proposal {
        return Err("write changed during native confirmation; refused".into());
    }
    let target = directory.join(&proposal.file_name).to_string_lossy().into_owned();
    let effect_id = next_id("g-");
    state.write_approval_ledger.approve(
        &proposal_id, &chat_id, &origin, &proposal.file_name,
        proposal.bytes(), &proposal.digest(), &target, &effect_id,
    ).map_err(|e| format!("write approval not committed: {e:?}"))?;
    let user_turn = next_id("g-");
    native_host_write::execute_approved_create_pinned(
        &state.chats, &state.effects, &chat_id, &user_turn, &effect_id,
        &directory, pinned_directory, &proposal,
    ).await.map(Some)
}

#[tauri::command]
fn lu_deny_write(
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    proposal_id: String,
) -> Result<(), String> {
    state.write_approval_ledger.deny(&proposal_id, &chat_id)
        .map_err(|e| format!("write denial could not be recorded: {e:?}"))?;
    state.write_proposals.dismiss(&proposal_id, &chat_id)
        .map_err(|e| format!("write request not dismissed: {e:?}"))?;
    Ok(())
}

/// A WebView request chooses only which inert proposal to review. The target
/// file comes exclusively from the native OS picker, not a JS pathname.
#[tauri::command]
async fn lu_review_edit_file(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    proposal_id: String,
) -> Result<edit_proposal::EditReviewView, String> {
    let state = Arc::clone(state.inner());
    state.chats.history(&chat_id)?;
    let chosen = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().blocking_pick_file()
            .map(|f|f.into_path().map_err(|e|e.to_string())).transpose()
    }).await.map_err(|e|format!("native file picker failed: {e}"))??
        .ok_or("File selection cancelled")?;
    let selected = state.edit_proposals.choose_file(&proposal_id, &chat_id, &chosen)
        .map_err(|e|format!("selected existing file cannot be reviewed: {e:?}"))?;
    let view = selected.view();
    state.edit_approval_ledger.choose(
        &proposal_id, &chat_id, &selected.generation_id,
        &view.exact_target, &view.original_sha256, &view.approval_sha256
    ).map_err(|e|format!("exact edit review not durable: {e:?}"))?;
    Ok(view)
}

/// The native modal, not the model or WebView, decides the consequential
/// action. The complete old/new text and line-by-line diff must remain
/// visible in Lu; the native modal binds that review to exact digests.
#[tauri::command]
async fn lu_confirm_edit_file(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    proposal_id: String,
) -> Result<Option<native_host_edit::EditedFile>, String> {
    let state = Arc::clone(state.inner());
    state.chats.history(&chat_id)?;
    let initial = state.edit_proposals.review_details(&proposal_id, &chat_id)
        .map_err(|e|format!("edit selection unavailable: {e:?}"))?;
    let preview = initial.view();
    let confirm_text = format!(
        "Replace this EXISTING file using the EXACT proposed text?\n\nFile: {}\nOriginal: {} bytes\nOriginal SHA-256: {}\nReplacement: {} bytes\nReplacement SHA-256: {}\nReview fingerprint: {}\nChange risk: {}\n\nThe complete before/after and change lines must be inspected in Lu. Lu saves a separate original-byte backup before replacing. This approval is single-use; interrupted edits are NEVER automatically retried.",
        preview.exact_target,preview.original_bytes,preview.original_sha256,
        preview.replacement_bytes,preview.replacement_sha256,
        preview.approval_sha256,preview.risk,
    );
    let confirmed = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().message(confirm_text)
            .title("Lu — Approve exact existing-file edit")
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "Approve exact edit".into(),"Cancel".into(),
            )).blocking_show()
    }).await.map_err(|e|format!("native edit confirmation failed: {e}"))?;
    if !confirmed {
        state.edit_approval_ledger.deny(&proposal_id,&chat_id)
            .map_err(|e|format!("edit denial unavailable: {e:?}"))?;
        state.edit_proposals.dismiss(&proposal_id,&chat_id)
            .map_err(|e|format!("edit proposal dismissal: {e:?}"))?;
        return Ok(None);
    }
    let selected = state.edit_proposals.take_confirmed(&proposal_id,&chat_id)
        .map_err(|e|format!("edit no longer available: {e:?}"))?;
    if selected.generation_id!=initial.generation_id ||
        selected.proposal!=initial.proposal ||
        selected.directory!=initial.directory ||
        selected.original!=initial.original ||
        selected.plan!=initial.plan ||
        selected.reviewed!=initial.reviewed ||
        !Arc::ptr_eq(&selected.directory_cap,&initial.directory_cap) {
        return Err("selected file or proposed bytes changed during native approval".into());
    }
    #[cfg(windows)]
    if selected.selected_file_identity!=initial.selected_file_identity {
        return Err("native-selected Windows file identity changed during confirmation".into());
    }
    let effect_call_id=next_id("g-");
    state.edit_approval_ledger.approve(
        &proposal_id,&chat_id,&selected.generation_id,
        &selected.proposal.file_name,selected.proposal.bytes(),
        &selected.proposal.digest(),&preview.exact_target,
        &selected.plan.expected_original_sha256,
        &selected.reviewed.approval_sha256,&effect_call_id
    ).map_err(|e|format!("exact edit approval not durable: {e:?}"))?;
    let user_turn_id=next_id("g-");
    native_host_edit::execute_approved_edit_pinned(
        &state.chats,&state.effects,&chat_id,&user_turn_id,
        &effect_call_id,&selected
    ).await.map(Some)
}

#[tauri::command]
fn lu_deny_edit_file(
    state: State<'_,Arc<DesktopState>>,
    chat_id:String, proposal_id:String,
)->Result<(),String>{
    state.chats.history(&chat_id)?;
    state.edit_approval_ledger.deny(&proposal_id,&chat_id)
        .map_err(|e|format!("cannot record edit denial: {e:?}"))?;
    state.edit_proposals.dismiss(&proposal_id,&chat_id)
        .map_err(|e|format!("cannot dismiss edit proposal: {e:?}"))
}

/// Durable history is read-only. Reopening an approved effect must NOT return
/// a cached authority token, resume a model, or rewrite the selected file.
#[tauri::command]
fn lu_edit_recovery(
    state: State<'_,Arc<DesktopState>>,chat_id:String
)->Result<Vec<EditRecoveryView>,String>{
    state.chats.history(&chat_id)?;
    let records=state.edit_approval_ledger.recent_for_chat(&chat_id)
        .map_err(|e|format!("edit recovery ledger unavailable: {e:?}"))?;
    records.into_iter().map(|record|{
        let effect_record=record.effect_call_id.as_ref().map(|id|
            state.effects.get(id)).transpose()
                .map_err(|e|format!("edit effect journal unavailable: {e:?}"))?
                .flatten();
        let effect_state=effect_record.as_ref().map(|r|r.state);
        let receipt_verified=effect_record.as_ref().zip(record.effect_call_id.as_deref())
            .is_some_and(|(effect,id)| record.original_sha256.as_deref()
                .is_some_and(|old|lu_core::native_edit_existing::verified_edit_receipt(
                    effect,id,old,&record.replacement_sha256,record.bytes as u64
                )));
        let effect=effect_state.map(|s|format!("{s:?}"))
            .unwrap_or_else(||"NOT_STARTED".into());
        let assessment=match (record.decision,effect_state,receipt_verified) {
            (lu_core::edit_approval_ledger::EditDecision::Interrupted,_,_) =>
                "INTERRUPTED_REVIEW_NOT_AUTHORIZED",
            (lu_core::edit_approval_ledger::EditDecision::Approved,
                Some(lu_core::journal::EffectState::Applied),true) =>
                "VERIFIED_APPLIED_RECEIPT_TARGET_NOT_RECHECKED",
            (lu_core::edit_approval_ledger::EditDecision::Approved,
                Some(lu_core::journal::EffectState::Applied),false) =>
                "APPLIED_ROW_BUT_RECEIPT_INVALID",
            (lu_core::edit_approval_ledger::EditDecision::Approved,
                Some(lu_core::journal::EffectState::Executing |
                    lu_core::journal::EffectState::Uncertain),_) =>
                "UNCERTAIN_NO_AUTOMATIC_RETRY",
            (lu_core::edit_approval_ledger::EditDecision::Approved,_,_) =>
                "APPROVED_NO_VERIFIED_SUCCESS",
            _=>"NOT_AUTHORIZED"
        };
        Ok(EditRecoveryView {
            proposal_id:record.proposal_id,
            file_name:record.filename,
            exact_target:record.selected_target,
            original_sha256:record.original_sha256,
            replacement_sha256:record.replacement_sha256,
            approval:format!("{:?}",record.decision),
            effect,effect_call_id:record.effect_call_id,
            observation:"NOT_RECHECKED_SELECT_FOLDER_FOR_OBSERVATION".into(),
            assessment:assessment.into(),receipt_verified,
        })
    }).collect()
}

/// Open only the containing folder of a host-reviewed, APPLIED edit. Never
/// accept a path from JavaScript, a model suggestion or retrieved text.
#[tauri::command]
fn lu_open_edited_folder(
    state:State<'_,Arc<DesktopState>>,chat_id:String,proposal_id:String,
)->Result<(),String>{
    state.chats.history(&chat_id)?;
    let record=state.edit_approval_ledger.get(&proposal_id)
        .map_err(|e|format!("edit history unavailable: {e:?}"))?
        .ok_or("no edit record")?;
    if record.chat_id!=chat_id ||
        record.decision!=lu_core::edit_approval_ledger::EditDecision::Approved {
        return Err("no approved edit in this conversation".into());
    }
    let effect_id=record.effect_call_id.ok_or("no edit effect recorded")?;
    let effect=state.effects.get(&effect_id)
        .map_err(|e|format!("edit journal unavailable: {e:?}"))?
        .ok_or("no edit effect receipt")?;
    if !record.original_sha256.as_deref().is_some_and(|old|
        lu_core::native_edit_existing::verified_edit_receipt(
            &effect,&effect_id,old,&record.replacement_sha256,record.bytes as u64
        )) {
        return Err("edit did not have a verified exact applied receipt".into());
    }
    let target=std::path::PathBuf::from(record.selected_target.ok_or("missing approved file path")?);
    if target.file_name().and_then(|s|s.to_str())!=Some(record.filename.as_str()) {
        return Err("edit target mismatches approved filename".into());
    }
    let folder=target.parent().ok_or("no containing folder")?;
    if !is_real_directory(folder) {
        return Err("original containing folder unavailable".into());
    }
    #[cfg(windows)]
    let executable="explorer.exe";
    #[cfg(target_os="linux")]
    let executable="xdg-open";
    #[cfg(target_os="macos")]
    let executable="open";
    #[cfg(not(any(windows,target_os="linux",target_os="macos")))]
    return Err("opening containing folders unsupported".into());
    #[cfg(any(windows,target_os="linux",target_os="macos"))]
    {
        std::process::Command::new(executable).arg(folder)
            .spawn().map_err(|e|format!("cannot open edited file folder: {e}"))?;
        Ok(())
    }
}

/// Native-picked containing folder permits read-only inspection even if the
/// target vanished. Never follow arbitrary paths from a WebView or replay a
/// crashed effect. Matched observed bytes are NOT causal proof of publication.
#[tauri::command]
async fn lu_inspect_edit_recovery(
    app: AppHandle,
    state: State<'_,Arc<DesktopState>>,
    chat_id:String,proposal_id:String,
)->Result<EditRecoveryObservationView,String>{
    let state=Arc::clone(state.inner());
    state.chats.history(&chat_id)?;
    let record=state.edit_approval_ledger.get(&proposal_id)
        .map_err(|e|format!("edit recovery unavailable: {e:?}"))?
        .ok_or("unknown edit recovery record")?;
    if record.chat_id!=chat_id ||
        record.decision!=lu_core::edit_approval_ledger::EditDecision::Approved {
        return Err("no approved edit for this conversation".into());
    }
    let call_id=record.effect_call_id.clone().ok_or("no effect call recorded")?;
    let exact_target=record.selected_target.clone().ok_or("no reviewed target recorded")?;
    let expected_folder=std::path::Path::new(&exact_target)
        .parent().ok_or("no original containing folder")?;
    let chosen=tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().blocking_pick_folder()
            .map(|p|p.into_path().map_err(|e|e.to_string())).transpose()
    }).await.map_err(|e|format!("recovery picker failed: {e}"))??
        .ok_or("Recovery inspection cancelled")?;
    if !is_real_directory(&chosen) {
        return Err("choose an ordinary containing folder".into());
    }
    let chosen=chosen.canonicalize().map_err(|e|format!("invalid folder: {e}"))?;
    if chosen!=expected_folder {
        return Err("selected folder is not the exact originally reviewed destination".into());
    }
    let dir=Arc::new(cap_std::fs::Dir::open_ambient_dir(&chosen,cap_std::ambient_authority())
        .map_err(|e|format!("recovery folder cannot be opened: {e}"))?);
    let observation=lu_core::native_edit_existing::observe_edit(&dir,&call_id);
    let effect_record=state.effects.get(&call_id)
        .map_err(|e|format!("effect journal unavailable: {e:?}"))?;
    let effect=effect_record.as_ref()
        .map(|row|format!("{:?}",row.state))
        .unwrap_or_else(||"NOT_STARTED".into());
    let receipt_verified=effect_record.as_ref().is_some_and(|row|
        record.original_sha256.as_deref().is_some_and(|old|
            lu_core::native_edit_existing::verified_edit_receipt(
                row,&call_id,old,&record.replacement_sha256,record.bytes as u64
            )
        )
    );
    let assessment=match (effect.as_str(),&observation,receipt_verified) {
        ("Applied",lu_core::native_edit_existing::EditObservation::ReplacementAndBackupVerified,true) =>
            "VERIFIED_APPLIED_RECEIPT_AND_OBSERVED_TARGET_BACKUP_MATCH",
        ("Applied",_,false)=>"APPLIED_ROW_BUT_RECEIPT_INVALID",
        ("Uncertain"|"Executing",
            lu_core::native_edit_existing::EditObservation::ReplacementAndBackupVerified,_) =>
            "UNCERTAIN_BYTES_MATCH_NOT_PROOF_NEVER_RETRY",
        ("Applied",_,true)=>"VERIFIED_APPLIED_RECEIPT_TARGET_DIFFERENT_OR_UNVERIFIED",
        ("Uncertain"|"Executing",_,_)=>"UNCERTAIN_EFFECT_RECONCILIATION_REQUIRED",
        _=>"NO_VERIFIED_SUCCESS_NO_AUTOMATIC_RETRY",
    };
    // Only a positively APPLIED journal entry, verified on-disk backup
    // and edited target, plus exact agreement with the trusted approval
    // record, may display an undo candidate. Display never grants rollback.
    let mut rollback_review_status = "NO_VERIFIED_APPLIED_EDIT".to_string();
    let mut rollback_review_id=None;
    let rollback_preview = if receipt_verified &&
        observation == lu_core::native_edit_existing::EditObservation::ReplacementAndBackupVerified {
        #[cfg(windows)]
        let identity_before_preview={
            let opened=dir.open(&record.filename)
                .map_err(|_|"native-selected rollback target unavailable")?.into_std();
            lu_core::windows_file_guard::inspect_open_file(&opened)
                .map_err(|_|"native-selected rollback target unsafe")?
        };
        match lu_core::native_edit_existing::inspect_rollback_candidate(
            &dir, &record.filename, &call_id
        ) {
            Ok(preview) if record.original_sha256.as_deref()
                == Some(preview.original_sha256.as_str()) &&
                record.replacement_sha256 == preview.replacement_sha256 &&
                record.filename == preview.file_name => {
                rollback_review_status = "VERIFIED_READ_ONLY_UNDO_CANDIDATE".into();
                // Native folder picker, verified original receipt and exact
                // bytes. A separate rollback review is persisted and pinned.
                let stale_reviews=state.rollback_reviews.clear_for(&chat_id,&proposal_id)
                    .map_err(|e|format!("rollback review queue unavailable: {e:?}"))?;
                for stale_id in stale_reviews {
                    let _=state.rollback_approval_ledger.deny(&stale_id,&chat_id);
                }
                let review_id=next_id("rb-");
                let approval=lu_core::rollback_approval_ledger::RollbackRecord {
                    review_id:review_id.clone(),chat_id:chat_id.clone(),
                    original_proposal_id:proposal_id.clone(),
                    original_edit_id:call_id.clone(),target:exact_target.clone(),
                    filename:record.filename.clone(),
                    expected_current_sha256:preview.replacement_sha256.clone(),
                    restored_sha256:preview.original_sha256.clone(),
                    restore_bytes:preview.restore_text.len() as u64,
                    decision:lu_core::rollback_approval_ledger::RollbackDecision::Reviewed,
                    rollback_effect_id:None,
                };
                #[cfg(windows)]
                let selected_file_identity={
                    let file=dir.open(&record.filename)
                        .map_err(|_|"native reselected file is unavailable")?.into_std();
                    let after=lu_core::windows_file_guard::inspect_open_file(&file)
                        .map_err(|_|"native reselected file identity is not safe")?;
                    if after!=identity_before_preview {
                        return Err("file ID changed while reviewing rollback; select again".into());
                    }
                    after
                };
                let selected=rollback_review::SelectedRollback {
                    record:approval.clone(),directory:chosen.clone(),
                    directory_cap:Arc::clone(&dir),preview:preview.clone(),
                    #[cfg(windows)] selected_file_identity,
                };
                match state.rollback_approval_ledger.stage(&approval) {
                    Ok(())=>match state.rollback_reviews.stage(selected) {
                        Ok(())=>rollback_review_id=Some(review_id),
                        Err(error)=>{
                            let _=state.rollback_approval_ledger.deny(
                                &approval.review_id,&chat_id
                            );
                            rollback_review_status=format!(
                                "UNDO_REVIEW_NOT_STAGED_{error:?}"
                            );
                        }
                    },
                    Err(error)=>rollback_review_status=format!(
                        "UNDO_REVIEW_LEDGER_UNAVAILABLE_{error:?}"
                    ),
                }
                Some(RollbackPreviewView {
                    file_name: preview.file_name,
                    original_sha256: preview.original_sha256,
                    replacement_sha256: preview.replacement_sha256,
                    current_text: preview.current_text,
                    restore_text: preview.restore_text,
                })
            }
            Ok(_) => {
                rollback_review_status = "APPROVAL_AND_ARTIFACT_MISMATCH".into();
                None
            }
            Err(error) => {
                rollback_review_status = format!("UNDO_PREVIEW_REFUSED_{error:?}");
                None
            }
        }
    } else {
        None
    };
    Ok(EditRecoveryObservationView {
        proposal_id,observation:format!("{observation:?}"),effect,
        assessment:assessment.into(),rollback_preview,rollback_review_status,
        rollback_review_id
    })
}

/// Only an ephemeral, native-picked restoration review may request this
/// dialog. The original edit approval NEVER authorizes undo. The WebView
/// supplies only opaque chat/review IDs, never bytes, path, or permission.
#[tauri::command]
async fn lu_confirm_rollback(
    app:AppHandle,
    state:State<'_,Arc<DesktopState>>,
    chat_id:String,review_id:String
)->Result<Option<native_host_edit::RestoredFile>,String> {
    let state=Arc::clone(state.inner());
    state.chats.history(&chat_id)?;
    let initial=state.rollback_reviews.inspect(&review_id,&chat_id)
        .map_err(|e|format!("rollback review unavailable: {e:?}"))?;
    let pending=state.rollback_approval_ledger.get(&review_id)
        .map_err(|e|format!("rollback review ledger unavailable: {e:?}"))?
        .ok_or("missing durable rollback review")?;
    if pending!=initial.record ||
        pending.decision!=lu_core::rollback_approval_ledger::RollbackDecision::Reviewed {
        return Err("rollback review is not an unspent durable approval candidate".into());
    }
    let original=state.edit_approval_ledger.get(&pending.original_proposal_id)
        .map_err(|e|format!("original edit history unavailable: {e:?}"))?
        .ok_or("original edit review not found")?;
    if original.chat_id!=chat_id ||
        original.decision!=lu_core::edit_approval_ledger::EditDecision::Approved ||
        original.effect_call_id.as_deref()!=Some(pending.original_edit_id.as_str()) ||
        original.selected_target.as_deref()!=Some(pending.target.as_str()) ||
        original.original_sha256.as_deref()!=Some(pending.restored_sha256.as_str()) ||
        original.replacement_sha256!=pending.expected_current_sha256 ||
        original.filename!=pending.filename {
        return Err("rollback candidate no longer matches trusted original approval".into());
    }
    let msg=format!(
        "RESTORE THE ORIGINAL TEXT? This changes an EXISTING file.\n\nFile: {}\nOriginal edit: {}\nCurrent file: {} bytes\nCurrent SHA-256: {}\nRestore: {} bytes\nRestore SHA-256: {}\n\nYou must have reviewed the complete before/after and change lines in Lu. Lu will journal this as a NEW effect and preserve another backup of the edited version. If the file changes meanwhile, restoration is refused. An interrupted restore is NEVER repeated automatically.",
        pending.target,pending.original_edit_id,
        initial.preview.current_text.len(),pending.expected_current_sha256,
        pending.restore_bytes,pending.restored_sha256
    );
    let confirmed=tauri::async_runtime::spawn_blocking(move || {
        app.dialog().message(msg)
            .title("Lu — Separately authorize restoring original")
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "Restore verified original".into(),"Keep current version".into()
            )).blocking_show()
    }).await.map_err(|e|format!("native rollback confirmation failed: {e}"))?;
    if !confirmed {
        state.rollback_approval_ledger.deny(&review_id,&chat_id)
            .map_err(|e|format!("rollback denial not durable: {e:?}"))?;
        state.rollback_reviews.dismiss(&review_id,&chat_id)
            .map_err(|e|format!("rollback review dismissal failed: {e:?}"))?;
        return Ok(None);
    }
    let selected=state.rollback_reviews.take(&review_id,&chat_id)
        .map_err(|e|format!("selected rollback no longer available: {e:?}"))?;
    if selected.record!=initial.record ||
        selected.preview!=initial.preview ||
        selected.directory!=initial.directory ||
        !Arc::ptr_eq(&selected.directory_cap,&initial.directory_cap) {
        return Err("selected rollback changed during native confirmation".into());
    }
    #[cfg(windows)]
    if selected.selected_file_identity!=initial.selected_file_identity {
        return Err("native-selected Windows restore target changed during confirmation".into());
    }
    let effect_id=next_id("g-");
    state.rollback_approval_ledger.approve(&selected.record,&effect_id)
        .map_err(|e|format!("rollback approval not durable: {e:?}"))?;
    let user_turn_id=next_id("g-");
    native_host_edit::execute_approved_rollback_pinned(
        &state.chats,&state.effects,&chat_id,&user_turn_id,
        &effect_id,&selected
    ).await.map(Some)
}

#[tauri::command]
fn lu_deny_rollback(
    state:State<'_,Arc<DesktopState>>,
    chat_id:String,review_id:String
)->Result<(),String>{
    state.chats.history(&chat_id)?;
    state.rollback_approval_ledger.deny(&review_id,&chat_id)
        .map_err(|e|format!("rollback denial unavailable: {e:?}"))?;
    state.rollback_reviews.dismiss(&review_id,&chat_id)
        .map_err(|e|format!("rollback review dismissal failed: {e:?}"))
}

/// Approvals and effect receipts remain visible after restart, even when a
/// prior reviewed capability was lost or an effect is UNCERTAIN. No replay.
#[tauri::command]
fn lu_rollback_history(
    state:State<'_,Arc<DesktopState>>,chat_id:String
)->Result<Vec<RollbackHistoryView>,String>{
    state.chats.history(&chat_id)?;
    let records=state.rollback_approval_ledger.recent_for_chat(&chat_id)
        .map_err(|e|format!("rollback history unavailable: {e:?}"))?;
    records.into_iter().map(|r|{
        let effect_record=r.rollback_effect_id.as_ref().map(|id|state.effects.get(id))
            .transpose().map_err(|e|format!("rollback effect journal unavailable: {e:?}"))?
            .flatten();
        let effect=effect_record.as_ref().map(|v|format!("{:?}",v.state))
            .unwrap_or_else(||"NOT_STARTED".into());
        let receipt_verified=effect_record.as_ref().zip(r.rollback_effect_id.as_deref())
            .is_some_and(|(row,id)|lu_core::native_edit_existing::verified_rollback_receipt(
                row,id,&r.original_edit_id,&r.restored_sha256,r.restore_bytes
            ));
        let assessment=match (r.decision,receipt_verified,effect.as_str()){
            (lu_core::rollback_approval_ledger::RollbackDecision::Interrupted,_,_)=>
                "INTERRUPTED_REVIEW_NO_AUTHORITY",
            (lu_core::rollback_approval_ledger::RollbackDecision::Approved,true,_)=>
                "VERIFIED_RESTORE_RECEIPT_TARGET_NOT_RECHECKED",
            (lu_core::rollback_approval_ledger::RollbackDecision::Approved,false,"Applied")=>
                "APPLIED_ROW_WITHOUT_VALID_ROLLBACK_RECEIPT",
            (lu_core::rollback_approval_ledger::RollbackDecision::Approved,false,
                "Uncertain"|"Executing")=>"UNCERTAIN_ROLLBACK_NO_AUTOMATIC_RETRY",
            (lu_core::rollback_approval_ledger::RollbackDecision::Approved,_,_)=>
                "APPROVED_RESTORE_NOT_CONFIRMED",
            _=>"NOT_AUTHORIZED",
        };
        Ok(RollbackHistoryView{
            review_id:r.review_id,original_proposal_id:r.original_proposal_id,
            original_edit_id:r.original_edit_id,file_name:r.filename,
            exact_target:r.target,decision:format!("{:?}",r.decision),
            effect,effect_call_id:r.rollback_effect_id,restored_sha256:r.restored_sha256,
            receipt_verified,assessment:assessment.into()
        })
    }).collect()
}

/// A *read-only* recovery screen. It reports durable evidence, never resumes
/// or reapplies an incomplete write. The effect journal is the source of truth.
#[tauri::command]
fn lu_write_recovery(
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
) -> Result<Vec<WriteRecoveryView>, String> {
    state.chats.history(&chat_id)?; // authenticated local desktop chat identity
    let current_documents = state.document_preferences
        .selected(&state.documents_os_dir).ok();
    let records = state.write_approval_ledger.recent_for_chat(&chat_id)
        .map_err(|e| format!("write recovery unavailable: {e:?}"))?;
    records.into_iter().map(|record| {
        let effect_state = if let Some(ref call_id) = record.effect_call_id {
            state.effects.get(call_id)
                .map_err(|e| format!("effect receipt unavailable: {e:?}"))?
                .map(|r| r.state)
        } else { None };
        let effect = effect_state
            .map(|state|format!("{:?}",state))
            .unwrap_or_else(|| "NOT_STARTED".into());
        let observed_target: String = match record.selected_target.as_deref() {
            Some(t) if current_documents.as_ref().is_some_and(|folder|
                folder.join(&record.file_name).to_string_lossy().as_ref() == t) =>
                native_host_write::inspect_recovery_target(
                    current_documents.as_ref().unwrap(),
                    &record.file_name, record.bytes, &record.sha256
                ).into(),
            _ => "EXTERNAL_TARGET_NOT_AUTOMATICALLY_INSPECTED".into(),
        };
        let observation = match observed_target.as_str() {
            "MATCHES_APPROVED_BYTES" => lu_core::write_recovery::TargetObservation::Matching,
            "MISSING" => lu_core::write_recovery::TargetObservation::Missing,
            "DIFFERENT_BYTES" | "NOT_ORDINARY_FILE" =>
                lu_core::write_recovery::TargetObservation::Different,
            "NOT_READABLE" => lu_core::write_recovery::TargetObservation::Unavailable,
            _ => lu_core::write_recovery::TargetObservation::NotChecked,
        };
        let assessment = lu_core::write_recovery::assess(
            record.decision, effect_state, observation
        ).label().to_owned();
        Ok(WriteRecoveryView {
            proposal_id: record.proposal_id,
            file_name: record.file_name,
            exact_target: record.selected_target,
            sha256: record.sha256,
            bytes: record.bytes,
            approval: format!("{:?}", record.decision),
            effect,
            effect_call_id: record.effect_call_id,
            observed_target,
            assessment,
        })
    }).collect()
}

#[tauri::command]
fn lu_chats(state: State<'_, Arc<DesktopState>>) -> Result<ChatListView, String> {
    let (chats, folders) = state.chats.list()?;
    Ok(ChatListView { chats, folders })
}
#[tauri::command]
fn lu_new_chat(state: State<'_, Arc<DesktopState>>) -> Result<ChatSummary, String> {
    state.chats.create_chat()
}
#[tauri::command]
fn lu_new_folder(state: State<'_, Arc<DesktopState>>, name: String) -> Result<ChatFolder, String> {
    state.chats.create_folder(&name)
}
#[tauri::command]
fn lu_favorite_chat(
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    favorite: bool,
) -> Result<(), String> {
    state.chats.favorite(&chat_id, favorite)
}
#[tauri::command]
fn lu_move_chat(
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    state.chats.move_chat(&chat_id, folder_id)
}
#[tauri::command]
fn lu_history(
    state: State<'_, Arc<DesktopState>>,
    chat_id: String,
) -> Result<Vec<ChatMessage>, String> {
    state.chats.history(&chat_id)
}
#[tauri::command]
fn lu_rescan_models(state: State<'_, Arc<DesktopState>>) -> Result<usize, String> {
    // The supervisor's registry is immutable while inference is loaded.
    let status = state.supervisor.status().map_err(|e| format!("{e:?}"))?;
    if status.session_id.is_some() {
        return Err("Unload the model before rescanning the models folder".into());
    }
    let mut catalog = state.catalog.lock().map_err(|_| "catalog poisoned")?;
    let mut snapshot = catalog.clone();
    let before = snapshot.len();
    scan_unindexed_models(&state.models_dir, &mut snapshot)?;
    for record in snapshot.iter().skip(before) {
        let mut admission = ModelAdmissionManager::new();
        admission
            .admit(
                &model_directory(&state.models_dir, record),
                &record.filename,
            )
            .map_err(|e| format!("admission: {e:?}"))?;
        state
            .supervisor
            .register_model(
                record.model_ref.clone(),
                admission.current().unwrap().clone(),
            )
            .map_err(|e| format!("registry: {e:?}"))?;
    }
    let count = snapshot.len() - before;
    if count > 0 {
        persist_catalog(&state.catalog_path, &snapshot)?;
    }
    *catalog = snapshot;
    Ok(count)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = DesktopState::new(app.handle()).map_err(std::io::Error::other)?;
            app.manage(Arc::new(state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            lu_status,
            lu_list_models,
            lu_import_model,
            lu_attach_projector,
            lu_pick_image,
            lu_ingest_image_data_url,
            lu_release_image,
            lu_load_model,
            lu_load_diagnostics,
            lu_unload_model,
            lu_cancel,
            lu_generate,
            lu_inspect_text_file,
            lu_approve_read_proposal,
            lu_dismiss_read_proposal,
            lu_document_settings,
            lu_browser_settings,
            lu_browser_runtime_status,
            lu_browser_start,
            lu_browser_stop,
            lu_set_browser_visibility,
            lu_choose_default_document_folder,
            lu_reset_default_document_folder,
            lu_open_created_folder,
            lu_review_write_destination,
            lu_confirm_write,
            lu_deny_write,
            lu_write_recovery,
            lu_review_edit_file,
            lu_confirm_edit_file,
            lu_deny_edit_file,
            lu_edit_recovery,
            lu_inspect_edit_recovery,
            lu_confirm_rollback,
            lu_deny_rollback,
            lu_rollback_history,
            lu_open_edited_folder,
            lu_history,
            lu_chats,
            lu_new_chat,
            lu_new_folder,
            lu_favorite_chat,
            lu_move_chat,
            lu_rescan_models
        ])
        .run(tauri::generate_context!())
        .expect("Lu native desktop failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_untrusted_model_catalog() {
        let dir = std::env::temp_dir().join(next_id("lu-catalog-test-"));
        fs::create_dir_all(&dir).unwrap();
        let catalog = dir.join("catalog.json");
        fs::write(
            &catalog,
            r#"[{"model_ref":"../../x","filename":"good.gguf","projector_attached":false}]"#,
        )
        .unwrap();
        assert!(checked_catalog(&catalog).is_err());
        fs::write(&catalog, r#"[{"model_ref":"m-00112233445566778899aabbccddeeff","filename":"../escape.gguf","projector_attached":false}]"#).unwrap();
        assert!(checked_catalog(&catalog).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn scans_lu_models_without_loading_or_importing_mmproj() {
        let root = std::env::temp_dir().join(next_id("lu-discovery-test-"));
        fs::create_dir_all(&root).unwrap();
        let header = [
            b"GGUF".as_slice(),
            &3u32.to_le_bytes(),
            &1u64.to_le_bytes(),
            &1u64.to_le_bytes(),
        ]
        .concat();
        fs::write(root.join("tiny.gguf"), &header).unwrap();
        fs::write(root.join("mmproj-other.gguf"), &header).unwrap();
        fs::write(root.join("corrupt.gguf"), b"not a model").unwrap();
        let mut records = Vec::new();
        assert_eq!(scan_unindexed_models(&root, &mut records).unwrap(), 1);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].filename, "tiny.gguf");
        assert!(records[0].model_subdir.is_none());
        let identity = records[0].model_ref.clone();
        assert_eq!(scan_unindexed_models(&root, &mut records).unwrap(), 0);
        assert_eq!(records[0].model_ref, identity);
        assert!(
            !root.join("model-index.json").exists(),
            "scanning cannot change model lifecycle"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn discovers_each_model_in_its_own_folder_without_loading() {
        let root = std::env::temp_dir().join(next_id("lu-model-folder-test-"));
        fs::create_dir_all(&root).unwrap();
        let id = "m-0123456789abcdef0123456789abcdef";
        let folder = root.join(id);
        fs::create_dir(&folder).unwrap();
        let header = [
            b"GGUF".as_slice(),
            &3u32.to_le_bytes(),
            &1u64.to_le_bytes(),
            &1u64.to_le_bytes(),
        ]
        .concat();
        fs::write(folder.join("main.gguf"), &header).unwrap();
        fs::write(folder.join(format!("mmproj-for-{id}.gguf")), &header).unwrap();
        let mut records = Vec::new();
        assert_eq!(scan_unindexed_models(&root, &mut records).unwrap(), 1);
        assert_eq!(records[0].model_ref, id);
        assert_eq!(records[0].model_subdir.as_deref(), Some(id));
        assert!(ModelView::for_record(&records[0], &root).available);
        assert_eq!(scan_unindexed_models(&root, &mut records).unwrap(), 0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn imported_model_and_projector_share_an_exclusive_directory() {
        let root = std::env::temp_dir().join(next_id("lu-import-folder-test-"));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("test-main.gguf");
        let projector_source = root.join("mmproj-test.gguf");
        let header = [
            b"GGUF".as_slice(),
            &3u32.to_le_bytes(),
            &1u64.to_le_bytes(),
            &1u64.to_le_bytes(),
        ]
        .concat();
        fs::write(&source, &header).unwrap();
        fs::write(&projector_source, &header).unwrap();
        let model_id = "m-00112233445566778899aabbccddeeff";
        let models_root = root.join("lu-models");
        fs::create_dir(&models_root).unwrap();
        let folder = models_root.join(model_id);
        fs::create_dir(&folder).unwrap();
        import_selected_gguf(&folder, &source).unwrap();
        import_selected_projector(&folder, &projector_source, model_id).unwrap();
        assert!(folder.join("test-main.gguf").is_file());
        assert!(folder.join(format!("mmproj-for-{model_id}.gguf")).is_file());
        assert!(!models_root.join("test-main.gguf").exists());
        assert_eq!(fs::read_dir(&models_root).unwrap().count(), 1);
        let mut records = Vec::new();
        assert_eq!(
            scan_unindexed_models(&models_root, &mut records).unwrap(),
            1
        );
        assert_eq!(records[0].model_ref, model_id);
        assert_eq!(records[0].model_subdir.as_deref(), Some(model_id));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn catalog_rejects_folder_traversal_and_preserves_legacy_entries() {
        let root = std::env::temp_dir().join(next_id("lu-catalog-folder-test-"));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("index.json");
        let base = r#"[{"model_ref":"m-0123456789abcdef0123456789abcdef","filename":"main.gguf","projector_attached":false}]"#;
        fs::write(&file, base).unwrap();
        let old = checked_catalog(&file).unwrap();
        assert!(old[0].model_subdir.is_none());
        let bad = r#"[{"model_ref":"m-0123456789abcdef0123456789abcdef","filename":"main.gguf","model_subdir":"../../escape","projector_attached":false}]"#;
        fs::write(&file, bad).unwrap();
        assert!(checked_catalog(&file).is_err());
        let wrong = r#"[{"model_ref":"m-0123456789abcdef0123456789abcdef","filename":"main.gguf","model_subdir":"m-ffffffffffffffffffffffffffffffff","projector_attached":false}]"#;
        fs::write(&file, wrong).unwrap();
        assert!(checked_catalog(&file).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn host_image_reference_matches_core_pinned_identity() {
        // Core's image digest uses its first 16 bytes (32 hex chars) and
        // the host prefix `img-` contributes 4 characters, total 36.
        let path = "img-0123456789abcdef0123456789abcdef";
        assert_eq!(path.len(), 36);
        assert!(path.starts_with("img-"));
    }
    #[test]
    fn decodes_pinned_engine_digest() {
        assert_eq!(decode_sha(ENGINE_SHA256).unwrap().len(), 32);
        assert!(decode_sha("bad").is_err());
    }
}
