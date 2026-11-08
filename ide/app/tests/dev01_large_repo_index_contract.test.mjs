import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const native=read('../../host/src/source_index.rs');
const web=read('../web/main.jsx');

test('large repository native index has host-side hard budgets and no content read',()=>{
 for(const marker of [
  'MAX_DIRECTORIES: usize = 128', 'MAX_FILES: usize = 4000',
  'MAX_ENTRIES_PER_DIRECTORY: usize = 1000',
  'MAX_TOTAL_ENTRIES: usize = 20000',
  'Duration::from_secs(8)', 'started.elapsed()', 'truncated = true',
  'source_capability(project_id)', 'validate_relative(&relative_path, false)',
  'file_type.is_symlink()', 'elapsed_ms'
 ]) assert.ok(native.includes(marker),marker);
 for(const forbidden of ['read_to_string(', 'read_to_end(', 'Command::new(', 'fs::read(',
   'std::fs::read(', '.rename(', 'remove_file(', 'ambient_authority()']) {
  assert.ok(!native.includes(forbidden),forbidden);
 }
});
test('quick open receives one project-bound native reply and rejects stale or malformed indexes',()=>{
 for(const marker of [
   "invoke('ide_index_project_files', {projectId})",
   "result.projectId !== projectId",
   "result.paths.length > 4000",
   "result.directoriesVisited > 128",
   "result.entriesExamined > 20000",
   "result.paths.some(path => !safeProjectRelativePath(path)",
   "INVALID_BOUNDED_NATIVE_PROJECT_INDEX",
   "activeProjectIdRef.current !== projectId",
   "setQuickOpenLimited(result.truncated)",
   "quickOpenMetrics.elapsedMs",
   "useMemo(",
 ]) assert.ok(web.includes(marker),marker);
});
