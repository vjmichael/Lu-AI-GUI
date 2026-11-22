import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL('../'+path,import.meta.url),'utf8');
const wf=read('.github/workflows/lu-core-c62b-reviewed-browser-nsis.yml');
const review=JSON.parse(read('docs/acceptance/LU_C62_WINDOWS_BROWSER_REVIEW.json'));
const promoter=read('lu-desktop/scripts/promote-reviewed-browser-candidate.mjs');
const native=read('lu-desktop/src-tauri/src/native_browser_host.rs');
const nsis=read('lu-desktop/packaging/NSIS_CURRENT_DEV_PREVIEW.nsi');
const m3=read('scripts/m3-windows-nsis-cycle.ps1');
const build=read('scripts/build-current-windows-nsis.ps1');
test('C6.2B admits only the separately audited exact Windows 482-file Chromium candidate',()=>{
 assert.equal(review.schema,'lu-c62-windows-browser-review-v1');
 assert.equal(review.disposition,'approved');
 assert.equal(review.sourceSha,'2da58f445d2466756140a95a1782cb26a37e41f3');
 assert.equal(review.runId,44976683630);
 assert.equal(review.artifactId,14735958441);
 assert.equal(review.fileCount,482);
 assert.equal(review.workerGitBlob,'68f7aa46c4cd2d6fdb0e951d8da5f057e1cafdd9');
 assert.equal(review.workerSha256,'c9ea3d8a6a6fe5d9864b095e1276c4284c296dcaedf8692b57cc958ecbf63dd9');
 assert.equal(review.independentReview.runId,44977571933);
 assert.equal(review.independentReview.artifactId,14735949862);
 assert.equal(review.independentReview.fileCount,482);
 assert.equal(JSON.parse(read('docs/acceptance/LU_C62_WINDOWS_BROWSER_REVIEW_LEGACY_D9AF093E.json')).sourceSha,'d9af093ef259d7279b5d1abb226ce307593b50cb');
 assert.equal(review.candidateManifestSha256,'23966eb91d2b933f4f6d9bed85f060cf6fe8c7e274adbb74374e105932cef890');
 assert.equal(review.reviewedManifestSha256,'f2948f9ef9d2b5bbac8a60e146bd4a18a0714a615883faaa0139b8d5828e5cf3');
 assert.equal(review.platform,'win32');
 assert.equal(review.arch,'x64');
 assert.equal(review.playwrightVersion,'1.64.1');
 assert.equal(review.observations.ownerPhysicalR9700,'NOT_TESTED');
 assert.equal(review.observations.modelPromptOrOriginScopeGrant,'NOT_GRANTED');
 assert.equal(review.observations.releaseStatus,'DEVELOPMENT_PREVIEW_ONLY');
 assert.match(promoter,/review\.disposition!=='approved'/);
 assert.match(promoter,/review\.workerSha256/);
 assert.match(promoter,/review\.independentReview/);
 assert.match(promoter,/review\.candidateManifestSha256/);
 assert.match(promoter,/review\.reviewedManifestSha256/);
 assert.match(promoter,/status='reviewed'/);
 assert.match(promoter,/await fileHash\(target\)!==f\.sha256/);
 assert.match(promoter,/Candidate contained undeclared executable\/data/);
 assert.match(promoter,/Candidate contains an unauthorized symlink/);
});
test('C6.2B runs only on one locked browser qualification branch, never automatic production promotion',()=>{
 const events=wf.split('\non:\n')[1].split('\npermissions:')[0];
 assert.match(events,/workflow_dispatch:/);assert.match(events,/workflow_call:/);
 assert.ok(events.includes('push:\n    branches:\n      - lu/cp10-c62b-reviewed-browser-20261121\n    paths:\n      - source-tree.lock.json'));
 assert.doesNotMatch(events,/pull_request:|pull_request_target:|schedule:/);
 assert.ok(wf.includes('group: lu-c62b-reviewed-browser-${{ github.ref }}'));
 assert.ok(wf.includes('cancel-in-progress: false'));
 assert.match(wf,/github\.sha/);
 assert.match(wf,/node scripts\/verify-source-tree\.mjs/);
 assert.match(wf,/run-id: 44976683630/);
 assert.match(wf,/Lu-C62B-REVIEW-REQUIRED-Windows-Browser-2da58f44/);
 assert.match(wf,/browser-ci-executable\.test\.mjs/);
 assert.match(wf,/promote-reviewed-browser-candidate\.mjs/);
 assert.match(wf,/LU_BROWSER_MANIFEST_SHA256: 'f2948f9ef9d2b5bbac8a60e146bd4a18a0714a615883faaa0139b8d5828e5cf3'/);
 assert.match(wf,/LU_REQUIRE_BROWSER: '1'/);
 assert.match(wf,/scripts\/build-current-windows-nsis\.ps1/);
 assert.match(wf,/scripts\/m3-windows-nsis-cycle\.ps1/);
 assert.doesNotMatch(wf,/continue-on-error: true/);
});
test('stale reviewed browser worker is not installed and is rebuilt only as unapproved evidence',()=>{
 const fields=[
  'worker_approved: ${{ steps.worker_review.outputs.worker_approved }}',
  'reviewed=68f7aa46c4cd2d6fdb0e951d8da5f057e1cafdd9',
  "needs.original-source-and-review.outputs.worker_approved == 'true'",
  "needs.original-source-and-review.outputs.worker_approved == 'false'",
  'git rev-parse HEAD:src-tauri/lu-core/workers/lu_browser_worker.mjs',
  'assemble-browser-ci-candidate.mjs',
  'browser-ci-candidate.test.mjs',
  'browser-ci-executable.test.mjs',
  'Lu-C62B-REVIEW-REQUIRED-Windows-Browser-',
  'REVIEW_REQUIRED: current browser is an UNAPPROVED candidate'
 ];
 for(const field of fields)assert.ok(wf.includes(field),field);
 assert.match(wf,/retention-days: 14/);
 assert.doesNotMatch(wf,/continue-on-error: true/);
});
test('independent archived Windows candidate review rehashes every byte without self approval',()=>{
 for(const literal of [
  'independently-verify-archived-candidate-for-review:',
  'run-id: 44976683630',
  'candidateArtifactId',
  'independentFileByteValidation',
  'manifest.sha256',
  'candidate-summary.json',
  'lu_browser_worker.mjs',
  'candidateStatus',
  'approval',
  'Lu-C62B-INDEPENDENT-REVIEW-EVIDENCE-',
  '[browser-review-evidence-only]'
 ])assert.ok(wf.includes(literal),literal);
 assert.ok(wf.includes('REVIEW_REQUIRED: current browser is an UNAPPROVED candidate'));
 assert.ok(wf.includes("if: ${{ needs.original-source-and-review.outputs.worker_approved == 'true' }}"));
 assert.doesNotMatch(wf,/continue-on-error: true/);
});
test('native Rust host verifies reviewed manifest and every byte without granting a browser turn',()=>{
 assert.match(native,/EMBEDDED_BROWSER_MANIFEST/);
 assert.match(native,/manifest\.status!="reviewed"/);
 assert.match(native,/pin_matches\(&dir\.join\(&file\.path\),&file\.sha256,file\.bytes\)/);
 assert.match(native,/fn c62_reviewed_runtime_exact_bytes_and_compiled_manifest/);
 assert.match(native,/assert!\(host\.package_verified\(\)/);
 assert.match(wf,/LU_C62_REVIEWED_RUNTIME_TEST: '1'/);
 assert.match(wf,/-- --ignored --exact --nocapture/);
 const desktop=read('lu-desktop/src-tauri/src/lib.rs');
 assert.match(desktop,/browser_runtime\.require_launch_ready\(\)\?/);
 assert.match(desktop,/browser_authority\.open_user_task/);
 assert.match(desktop,/Read-only Browser grant/);
 assert.match(desktop,/\.blocking_show\(\)/);
});
test('browser is actually installed, independently rehashed, and removed without deleting user data',()=>{
 assert.match(build,/browserManifestSha256=\$browserDigest/);
 assert.match(build,/LU_BROWSER_PACKAGE_DIR/);
 assert.match(build,/scripts\/stage-browser-runtime\.mjs/);
 assert.match(nsis,/!ifdef LU_BROWSER_DIR/);
 assert.match(nsis,/File \/r "\$\{LU_BROWSER_DIR\}\\\*\.\*"/);
 assert.match(nsis,/RMDir \/r "\$INSTDIR\\lu-browser"/);
 assert.match(m3,/ExpectedBrowserManifestSha256/);
 assert.equal((m3.match(/if\(\$ExpectedBrowserManifestSha256\)\{/g)||[]).length,1,'One real original NSIS cycle, no duplicated insertion');
 assert.ok(m3.length<18500,'Original installed script preserved without repeated injected content');
 assert.match(m3,/Installed Browser manifest differs from independently approved SHA/);
 assert.match(m3,/Installed Browser symlink in reviewed component parent/);
 assert.match(m3,/Installed Browser unexpected file/);
 assert.match(m3,/\$verifiedPaths=\[System\.Collections\.Generic\.HashSet\[string\]\]::new\(\[StringComparer\]::OrdinalIgnoreCase\)/);
 assert.match(m3,/if\(-not \$verifiedPaths\.Add\(\$fi\.FullName\)\)/);
 assert.match(m3,/Installed Browser duplicate physical component/);
 assert.match(m3,/\$verifiedPaths\.Count -ne \$seen\.Count/);
 assert.match(m3,/if\(\$actual\.Count -ne \(\$seen\.Count\+1\)\)/);
 assert.match(m3,/\$item\.FullName -ine \$mi\.FullName -and -not \$verifiedPaths\.Contains\(\$item\.FullName\)/);
 assert.match(m3,/GetRelativePath\(\$root,\$item\.FullName\)/);
 assert.doesNotMatch(m3,/\$item\.FullName\.Substring\(\$root\.Length\)/);
 assert.match(m3,/PASS_EXACT_BROWSER_FILES_AND_REAL_BUNDLED_NODE/);
 assert.match(m3,/PASS_UNTOUCHED_APPDATA_SENTINEL/);
 assert.match(wf,/PASS_INSTALLED_UNIFIED_TABS/);
 assert.match(wf,/PASS_BINARIES_ABSENT/);
});

test('C6.2B4 native Rust test uses precisely the staged NSIS resource map and restores config',()=>{
 const active=wf.split('      - name: Native Rust host independently recomputes')[1].split('      - name: Genuine hosted Windows install')[0];
 const buildMap=build.split('  $conf.bundle.resources=@{')[1].split('  }')[0];
 const testMap=active.split('            $conf.bundle.resources=@{')[1].split('            }')[0];
 const entries=s=>[...s.matchAll(/'([^']+)'='([^']+)'/g)].map(m=>m[1]+'='+m[2]).sort();
 assert.deepEqual(entries(testMap),[...entries(buildMap),'resources/lu-browser=lu-browser'].sort());
 assert.match(active,/Get-FileHash -LiteralPath \$browserManifest -Algorithm SHA256/);
 assert.match(active,/\$browserHash -cne \$env:LU_BROWSER_MANIFEST_SHA256/);
 assert.match(active,/-- --ignored --exact --nocapture/);
 assert.match(active,/finally\{\s*\[IO\.File\]::WriteAllText\(\$configPath,\$original\)/);
 assert.match(active,/& git diff --quiet --exit-code -- lu-desktop\/src-tauri\/tauri\.conf\.json/);
 assert.match(active,/if\(\$LASTEXITCODE -ne 0\)\{\s*throw 'Native Rust test did not restore/);
 assert.doesNotMatch(active,/git diff[^\n]*\.Trim\(\)/);
 assert.doesNotMatch(active,/vcruntime140_1\.dll|msvcp140\.dll/);
});
