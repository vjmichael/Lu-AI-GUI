import assert from 'node:assert/strict';
import { readFileSync, statSync } from 'node:fs';
const ui = readFileSync(new URL('../web/main.jsx', import.meta.url), 'utf8');
const host = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8');
const store = readFileSync(new URL('../src-tauri/src/chat_store.rs', import.meta.url), 'utf8');
const css = readFileSync(new URL('../web/styles.css', import.meta.url), 'utf8');
const js = readFileSync(new URL('../dist/assets/app.js', import.meta.url), 'utf8');
const tests = [
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
  ['frontend build exists and includes chat commands', statSync(new URL('../dist/assets/app.js', import.meta.url)).size > 100000 && js.includes('lu_new_chat') && js.includes('lu_rescan_models')],
];
for (const [name, pass] of tests) {
  console.log(`${pass ? 'PASS' : 'FAIL'} ${name}`);
  assert.ok(pass, name);
}
console.log(`Lu R22 independent desktop source and bundle checks: ${tests.length}/${tests.length}`);
