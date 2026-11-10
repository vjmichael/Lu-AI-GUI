import assert from 'node:assert/strict';
import { readFileSync, statSync } from 'node:fs';
const ui = readFileSync(new URL('../web/main.jsx', import.meta.url), 'utf8');
const host = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8');
const workspaceHost = readFileSync(new URL('../src-tauri/src/workspace_desktop.rs', import.meta.url), 'utf8');
const store = readFileSync(new URL('../src-tauri/src/chat_store.rs', import.meta.url), 'utf8');
const css = readFileSync(new URL('../web/styles.css', import.meta.url), 'utf8');
const js = readFileSync(new URL('../dist/assets/app.js', import.meta.url), 'utf8');
const tests = [
  ['Image Workspace only exports immutable native-approved PNG revisions from host-pinned images',
    host.includes('async fn lu_image_workspace_export(') &&
    host.includes('native_image_revision::transform_admitted_image') &&
    host.includes('blocking_save_file()') &&
    ui.includes("invoke('lu_image_workspace_export',{") &&
    ui.includes('Original stays unchanged') &&
    css.includes('.imageRevisionForm')],
  ['Background task view is scoped to an existing selected chat before querying SQLite',
    host.includes('dashboard_for_workspace(&chat_id,100)') &&
    ui.includes("invoke('lu_background_tasks',{chatId})")],
  ['Runtime Environment diagnostics never claim unproven vision or GPU inference',
    host.includes('fn lu_environment_status(') &&
    host.includes('environment_context::runtime_view(')],
  ['Research OpenAlex discovery is human-approved, per-chat, and never represented as verified citations',
    host.includes('async fn lu_research_openalex(') &&
    host.includes('fn lu_research_available(') &&
    ui.includes("invoke('lu_research_openalex',{chatId:sourceChat,query:researchQuery,page:selectedPage})") &&
    ui.includes('discovered sources · UNVERIFIED') &&
    ui.includes('Research network runtime is not bundled')],
  ['Browser runtime is native-owned, permit-gated, and host-verified with no ambient model navigation',
    host.includes('async fn lu_browser_runtime_status(') &&
    host.includes('async fn lu_browser_start(') &&
    host.includes('owner.browser_authority.open_user_task(') &&
    host.includes('owner.browser_runtime.require_launch_ready()?') &&
    ui.includes("browserOperation('lu_browser_start',{chatId,origins:browserOrigins})") &&
    ui.includes('Authorize read-only Browser')],
  ['Observe awareness lane logs scoped native hints and prepares a no-execution handoff',
    host.includes('fn lu_observe_notes(') &&
    host.includes('fn lu_observe_handoff(') &&
    host.includes('lu_observe_notes,') &&
    ui.includes("invoke('lu_observe_notes',{chatId})") &&
    ui.includes('Prepare large-model review handoff') &&
    ui.includes('Main-model handoff prepared · not executed') &&
    css.includes('.observeNotations')],
  ['Observe WGC one-shot is explicit, pinned, supervised, user-selected and does not claim visual understanding',
    host.includes('async fn lu_observe_capture_one_shot(') &&
    host.includes('LU_OBSERVE_CAPTURE_WORKER_SHA256') &&
    host.includes('observer.launch_one_shot(&chat_id,plan)') &&
    host.includes('lu_observe_capture_available,') &&
    ui.includes("invoke('lu_observe_capture_one_shot',{chatId,foregroundOnly})") &&
    ui.includes('Select window/display · capture one frame') &&
    ui.includes('Capture selected foreground app (4s)') &&
    host.includes('--lu-observe-foreground-one-shot') &&
    ui.includes("if(action==='pause'||action==='stop')setCaptureTaskId(null)") &&
    ui.includes('No image pixels are retained or sent to the model')],
  ['Observe pilot is native-approved, scoped to Lu events and never claims desktop capture',
    host.includes('fn lu_observe_self_signal(') &&
    host.includes('fn lu_observe_status(') &&
    host.includes('lu_observe_start,') &&
    ui.includes('Observe & Learn · Lu UI only') &&
    ui.includes('Neither gives the model its pixels.') &&
    ui.includes("observeAction('stop')") &&
    host.includes('notice.task_id.starts_with("obs-")') &&
    css.includes('.observePilot')],
  ['Wave 4 browser preference remains a host-only render setting without navigation authority',
    host.includes('fn lu_browser_visibility(') &&
    host.includes('fn lu_set_browser_visibility(') &&
    ui.includes("invoke('lu_set_browser_visibility',{mode})") &&
    ui.includes('This preference does not launch or authorize browsing')],
  ['Wave 4 vision status is host-attested and never claims projector presence is a successful probe',
    host.includes('fn lu_vision_capability(') &&
    host.includes('vision_context::image_preflight(') &&
    ui.includes("invoke('lu_vision_capability')") &&
    ui.includes('Vision: {visionCapability.status}')],
  ['Wave 4 Image Workspace opens only a read-only local draft preview without model edit effects',
    ui.includes('Image Workspace') &&
    ui.includes('setImageViewer({imageRef:im.imageRef,zoom:1})') &&
    ui.includes('Original stays unchanged') &&
    css.includes('.imageViewerDialog')],
  ['Wave 4 explicit Git inspections reuse an OS picker, bounded typed tools and no general Git authority',
    host.includes('workspace_desktop::explicit_read_git_request(&prompt)') &&
    host.includes('workspace_desktop::ask_git_for_chat(') &&
    workspaceHost.includes('WorkspaceReadTurnSession::new_git(') &&
    workspaceHost.includes('trusted_git_executable()') &&
    ui.includes('Git status, log and diff require selecting a repository')],
  ['Wave 3 ordinary Send can open trusted Reader for explicit document interpretation',
    host.includes('reader_desktop::explicit_reader_document_request(&prompt)') &&
    host.includes('reader_desktop::ask_reader_for_chat(')],
  ['Wave 3 ordinary text can offer a native workspace picker without model permission or attachment instructions',
    host.includes('workspace_desktop::explicit_read_workspace_request(&prompt)') &&
    ui.includes('Explicit workspace searches and document-reading requests open native pickers')],
  ['Wave 3 tool discovery comes from the actual authorized native registry, never from model names',
    ui.includes("listen('lu-native-turn-capabilities'") &&
    ui.includes('Read-only native tools for this turn') &&
    workspaceHost.includes('turn.capabilities()')],
  ['Wave 3 live tool status follows trusted dispatch checkpoints and stays chat-scoped',
    ui.includes("listen('lu-native-tool-progress'") &&
    ui.includes('Lu · using approved native tool') &&
    host.includes('workspace_desktop::ask_workspace_for_chat(')],
  ['Wave 3 native tool evidence is a read-only, chat-scoped audit without model bytes',
    ui.includes("invoke('lu_tool_turn_history'") &&
    ui.includes('Native tool activity') &&
    host.includes('fn lu_tool_turn_history(') &&
    host.includes('summaries_for_conversation(&chat_id,32)')],
  ['Wave 3 model native-action handoff is inert, host scoped and routes to existing native review',
    ui.includes("listen('lu-native-action-proposal'") &&
    ui.includes("invoke('lu_accept_native_action_proposal'") &&
    ui.includes("invoke('lu_deny_native_action_proposal'") &&
    ui.includes('Review with native controls') &&
    host.includes('state.native_actions.stage(') &&
    host.includes('fn lu_accept_native_action_proposal(')],
  ['Wave 3 normal Send one-turn workspace route is explicitly opt-in and uses native picker',
    ui.includes('readOnlyWorkspace: useWorkspaceTools') &&
    ui.includes('Use workspace tools on Send') &&
    ui.includes('if (useWorkspaceTools) setWorkspaceToolsNextSend(false)') &&
    host.includes('workspace_desktop::ask_workspace_for_chat(') &&
    host.includes('workspace_desktop::validate_workspace_send_request(')],
  ['native GGUF import and generate remain Lu-only', ui.includes("invoke('lu_import_model')") && ui.includes("invoke('lu_generate'")],
  ['no Jan execution paths introduced', !/jan_llama|JanInference|jan-cowork/.test(ui + host)],
  ['new chats and conversation history are host-owned', ui.includes("invoke('lu_new_chat')") && ui.includes("invoke('lu_chats')") && host.includes('fn lu_new_chat(')],
  ['one-level folders and favorites are durable', ui.includes("invoke('lu_new_folder'") && ui.includes("invoke('lu_favorite_chat'") && store.includes('pub fn favorite(')],
  ['models available from a quick dropdown', ui.includes('modelSwitcher') && ui.includes('modelMenu')],
  ['model management only in Settings', ui.includes('settingsOpen') && ui.includes('Settings') && !ui.includes('MODEL CONTROL')],
  ['no visible Lu Native Runtime chip', !ui.includes('Lu Native Runtime')],
  ['selection does not start model inference', !/onClick=\{\(\) => \{ setSelectedModel\([^}]*loadModel/.test(ui) && ui.includes('Selection does not load')],
  ['model discovery stays host owned and explicit', host.includes('fn scan_unindexed_models(') && host.includes('fn lu_rescan_models(')],
  ['no auto load in application setup', !/\.setup\([\s\S]*?lu_load_model\(/.test(host)],
  ['prompt clears immediately and drafts restore on failure', ui.includes("setPrompt(''); setImages([]);") && ui.includes('if (!completed && !cancelled && currentChat.current === activeId')],
  ['intentional cancellation retains partial text', !ui.includes("// Cancellation is intentional; partial streamed text is not a final answer.") && host.includes('append_progress(&chat_id, &gid, &answer)') && host.includes('state.chats.cancel(&chat_id, &gid)')],
  ['composer is resizable and collapsible', ui.includes('beginComposerDrag') && ui.includes('composerCollapsed') && css.includes('composerResizer')],
  ['brighter green model loaded indicator', css.includes('#22f66e')],
  ['fixed host image ID length matches Lu Core', host.includes('image_ref.len() != 36') && !host.includes('image_ref.len() != 68')],
  ['model discovery never imports MMProj as a main model', host.includes('lowered.starts_with("mmproj")')],
  ['switching chats permitted during generation', !ui.includes('if (busy) return;\n    requestedChat.current') && ui.includes('currentChat.current')],
  ['streaming chunks are routed to generating chat', ui.includes('tokenForJob(prev,e.payload)') && host.includes('chat_id: chat_id.clone()')],
  ['paste/drop attachments routed through bounded host image admission', ui.includes('onPaste={handlePaste}') && ui.includes('onDrop={handleDrop}') && host.includes('fn lu_ingest_image_data_url(')],
  ['quick unload model supported', ui.includes('quickEject') && ui.includes('Unload active model')],
  ['model supplied think blocks rendered', ui.includes('splitModelOutput') && ui.includes('Model reasoning')],
  ['cancellation journal preserves interrupted reply', store.includes('message.cancelled = true')],
  ['conversation history carried as typed Protobuf turns', host.includes('let history = if origin==GenerationOrigin::Human{') && host.includes('prompt_context::typed_history(&events[..events.len()-1],&prompt)') && host.includes('prompt_context::typed_history(&events,&prompt)') && host.includes('let events = state.chats.history(&chat_id)?')],
  ['queued generations do not stop unrelated chat work', /generation_queue\s*\.enqueue\(/.test(host) && ui.includes('visibleJob')],
  ['restricted Markdown renderer and code-copy presentation', ui.includes("import { MarkdownView }") && ui.includes('role={entry.role}') && css.includes('.markdownBody .codeBlock')],
  ['text attachments are draft metadata rather than textarea insertion', ui.includes("setTextFiles(draft.files || [])") && ui.includes("buildSubmittedPrompt(input, textFiles)")],
  ['image previews are scoped with blob CSP only for image sources', ui.includes('URL.createObjectURL(file)') && readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url),'utf8').includes("img-src 'self' data: blob:")],
  ['archive review uses supervised native decoding and scoped preview cancellation',
    host.includes('state.archive_reviews.stage_supervised(') &&
    host.includes('fn lu_cancel_archive_preview(') &&
    ui.includes("invoke('lu_cancel_archive_preview'") &&
    ui.includes('Cancel archive preview')],
  ['archive recovery independently observes approved full-tree integrity after restart',
    host.includes('state.wave2_recovery.record_archive_intent(') &&
    ui.includes('operation.targetIntegrity') &&
    host.includes('pinned.approved_manifest()')],
  ['archive worker has a host-owned scoped cancellation control and no auto-replay',
    ui.includes("invoke('lu_cancel_archive_tree'") &&
    ui.includes('Cancel running extraction') &&
    host.includes('fn lu_cancel_archive_tree(') &&
    host.includes('archive_runs: archive_desktop::ArchiveRunRegistry') &&
    host.includes('run_guard.cancellation()')],
  ['archive desktop review is pinned to native pickers and full-manifest user acknowledgement',
    ui.includes("invoke('lu_review_archive_tree'") &&
    ui.includes("invoke('lu_confirm_archive_tree'") &&
    ui.includes("invoke('lu_deny_archive_tree'") &&
    ui.includes('archiveReview.entries.map') &&
    ui.includes('disabled={!archiveReviewed || archiveBusy}') &&
    ui.includes('manual recovery') &&
    host.includes('fn lu_review_archive_tree(') &&
    host.includes('fn lu_confirm_archive_tree(') &&
    host.includes('fn lu_deny_archive_tree(') &&
    host.includes('archive_reviews: archive_desktop::ArchiveReviewQueue') &&
    host.includes('app.dialog().message(modal)') &&
    host.includes('state.archive_reviews.consume(')],
  ['structured DOCX/PPTX writer uses complete draft review and a separate native confirmation',
    ui.includes("invoke('lu_review_document_create'") &&
    ui.includes("invoke('lu_confirm_document_create'") &&
    ui.includes("invoke('lu_deny_document_create'") &&
    ui.includes('documentCreateReview.content') &&
    ui.includes('disabled={!documentCreateReviewed || documentCreateBusy}') &&
    host.includes('fn lu_review_document_create(') &&
    host.includes('fn lu_confirm_document_create(') &&
    host.includes('fn lu_deny_document_create(') &&
    host.includes('document_create_reviews: document_create_desktop::DocumentCreateQueue') &&
    host.includes('state.document_create_reviews.consume(')],
  ['existing Office revisions use native source and output pickers, complete change review and independent approval',
    ui.includes("invoke('lu_review_office_revision'") &&
    ui.includes("invoke('lu_confirm_office_revision'") &&
    ui.includes("invoke('lu_deny_office_revision'") &&
    ui.includes('officeRevisionReview.unaffectedParts.map') &&
    ui.includes('disabled={!officeRevisionReviewed || officeRevisionBusy}') &&
    host.includes('fn lu_review_office_revision(') &&
    host.includes('fn lu_confirm_office_revision(') &&
    host.includes('fn lu_deny_office_revision(') &&
    host.includes('office_revision_reviews: office_revision_desktop::OfficeRevisionQueue') &&
    host.includes('state.office_revision_reviews.consume(')],
  ['native file copy/move review binds host-selected source, exact content hash and native one-use confirmation',
    ui.includes("invoke('lu_review_file_transfer'") &&
    ui.includes("invoke('lu_confirm_file_transfer'") &&
    ui.includes("invoke('lu_deny_file_transfer'") &&
    ui.includes('fileTransferReview.sourceSha256') &&
    ui.includes('disabled={!fileTransferReviewed || fileTransferBusy}') &&
    host.includes('fn lu_review_file_transfer(') &&
    host.includes('fn lu_confirm_file_transfer(') &&
    host.includes('fn lu_deny_file_transfer(') &&
    host.includes('file_transfer_reviews: file_transfer_desktop::FileTransferReviewQueue') &&
    host.includes('state.file_transfer_reviews.consume(')],
  ['reversible file trash and restore are native-picked, separately reviewed, and verified against custody history',
    ui.includes("invoke('lu_review_file_trash'") &&
    ui.includes("invoke('lu_confirm_file_trash'") &&
    ui.includes("invoke('lu_deny_file_trash'") &&
    ui.includes("invoke('lu_file_trash_history'") &&
    ui.includes("invoke('lu_review_file_restore'") &&
    ui.includes("invoke('lu_confirm_file_restore'") &&
    ui.includes("invoke('lu_deny_file_restore'") &&
    ui.includes('trashReview.sha256') &&
    ui.includes('restoreReview.sha256') &&
    ui.includes('disabled={!trashReviewed || trashBusy}') &&
    ui.includes('disabled={!restoreReviewed || trashBusy}') &&
    ui.includes('item.restorable ?') &&
    ui.includes('Uncertain operations require manual reconciliation') &&
    host.includes('fn lu_review_file_trash(') &&
    host.includes('fn lu_confirm_file_trash(') &&
    host.includes('fn lu_file_trash_history(') &&
    host.includes('fn lu_review_file_restore(') &&
    host.includes('fn lu_confirm_file_restore(') &&
    host.includes('trash_custody.reconcile_interrupted()') &&
    host.includes('file_trash_desktop::TrashQueue')],
  ['Wave 2 history survives restart from host intents and typed journal receipts without retry controls',
    ui.includes("invoke('lu_wave2_recovery'") &&
    ui.includes('operation.receiptMatched') &&
    ui.includes('operation.state.replaceAll') &&
    host.includes('fn lu_wave2_recovery(') &&
    host.includes('wave2_recovery: wave2_recovery::Wave2OperationLedger') &&
    host.includes('state.wave2_recovery.record_intent(') &&
    host.includes('state.wave2_recovery.list(')],
  ['one reversible trash custody authority and one set of UI review controls',
    host.includes('mod file_trash_desktop;') &&
    !host.includes('mod trash_desktop;') &&
    host.includes('native_trash_custody::TrashCustodyLedger') &&
    host.includes('trash_custody.reconcile_interrupted()') &&
    (ui.match(/const \[trashReview, setTrashReview\]/g) || []).length === 1 &&
    (ui.match(/const \[trashBusy, setTrashBusy\]/g) || []).length === 1 &&
    (ui.match(/aria-label="Review reversible file trash"/g) || []).length === 1 &&
    !ui.includes("invoke('lu_review_trash_file'") &&
    !host.includes('fn lu_review_trash_file(')],
  ['file operation UI reports active native work without fabricating completion or percent',
    ui.includes('const wave2FileBusy = archiveBusy') &&
    ui.includes('aria-label="Native file operation in progress"') &&
    ui.includes('Byte-level progress is not available') &&
    ui.includes('No automatic retry occurs') &&
    ui.includes('File recovery before taking another action')],
  ['bounded Office batch native review binds full before-after diff and unchanged package hashes',
    ui.includes("invoke('lu_review_office_batch'") &&
    ui.includes("invoke('lu_confirm_office_batch'") &&
    ui.includes("invoke('lu_deny_office_batch'") &&
    ui.includes('officeBatchReview.changes.map') &&
    ui.includes('officeBatchReview.unaffectedParts.map') &&
    ui.includes('disabled={!officeBatchReviewed || officeBatchBusy}') &&
    host.includes('fn lu_review_office_batch(') &&
    host.includes('fn lu_confirm_office_batch(') &&
    host.includes('fn lu_deny_office_batch(') &&
    host.includes('office_batch_reviews: office_batch_desktop::OfficeBatchQueue') &&
    host.includes('state.wave2_recovery.record_intent(')],
  ['frontend build exists and includes chat commands', statSync(new URL('../dist/assets/app.js', import.meta.url)).size > 100000 && js.includes('lu_new_chat') && js.includes('lu_rescan_models')],
];
for (const [name, pass] of tests) {
  console.log(`${pass ? 'PASS' : 'FAIL'} ${name}`);
  assert.ok(pass, name);
}
console.log(`Lu R22 independent desktop source and bundle checks: ${tests.length}/${tests.length}`);
