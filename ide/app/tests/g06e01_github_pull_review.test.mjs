import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const native = readFileSync('../host/src/github_pull_review.rs', 'utf8');
const host = readFileSync('src-tauri/src/lib.rs', 'utf8');
const ui = readFileSync('web/main.jsx', 'utf8');

test('6E01 uses fixed GitHub public REST origin and same local repository', () => {
  assert.match(native, /git_branch_operations::\{exact_repo, safe_local_name\}/);
  assert.match(native, /github_live::github_repo/);
  assert.match(native, /https:\/\/api\.github\.com\/repos\/\{repository\}\/pulls/);
  assert.match(native, /\.redirects\(0\)/);
  assert.match(native, /\.take\(MAX_RESPONSE \+ 1\)/);
  assert.match(native, /string\(pr, \&\["head", "repo", "full_name"\]\)/);
  assert.match(native, /string\(pr, \&\["base", "repo", "full_name"\]\)/);
  assert.match(native, /G06E01_ORIGIN_OR_HEAD_CHANGED/);
});

test('6E01 never trusts arbitrary PR URL, approval, or CI metadata', () => {
  assert.match(native, /https:\/\/github\.com\/\{repository\}\/pull\/\{number\}/);
  assert.match(native, /review_approval_verified: false, checks_verified: false/);
  assert.match(native, /operation_authorized: false/);
  assert.match(native, /listing_partial = next_page \|\| pulls\.len\(\) == MAX_PRS/);
  assert.match(native, /never_accept_foreign_pr_wrong_commit_as_approval_or_malicious_title/);
});

test('6E01 Tauri and UI are explicit read-only routes with project-change guards', () => {
  assert.match(host, /async fn ide_read_github_pull_requests\(/);
  assert.match(host, /registry\.inspect_github_pull_requests\(&project_id\)/);
  assert.match(ui, /invoke\('ide_read_github_pull_requests'/);
  assert.match(ui, /result\.credentialsUsed !== false/);
  assert.match(ui, /result\.remoteRefModified !== false/);
  assert.match(ui, /result\.reviewSubmitted !== false/);
  assert.match(ui, /result\.taskCompletionVerified !== false/);
  assert.match(ui, /activeProjectIdRef\.current !== projectId/);
  assert.match(ui, /G06E01 exact origin read-only pull-request inspection/);
});
