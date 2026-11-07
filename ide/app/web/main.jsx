import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { CodeEditor, createEditorState } from './code_editor.jsx';
import { locateMatches, planReplacement } from './find_replace.js';
import { buildDevelopmentCheckpoint } from './development_checkpoint.js';
import { languageForPath, matchesSource, visibleDiagnostics, visibleCompletions, visibleHover, visibleDefinition } from './lsp_editor.js';
import { reconcileDevelopmentEvidence } from './development_reconciliation.js';

const sections = [
  { id: 'projects', title: 'Projects', glyph: '▣' },
  { id: 'explorer', title: 'Explorer', glyph: '▤' },
  { id: 'proposals', title: 'Proposals', glyph: '⇄' },
  { id: 'capabilities', title: 'Capabilities', glyph: '◇' },
  { id: 'continuity', title: 'Continuity', glyph: '▦' },
  { id: 'languages', title: 'Language services', glyph: '⌁' },
  { id: 'packages', title: 'Packages & workflows', glyph: '⬡' },
  { id: 'source', title: 'Source control', glyph: '⑂' },
  { id: 'tasks', title: 'Job requests', glyph: '▶' },
];

const keyFor = (projectId, path) => projectId + ':' + path;
const leafFor = path => path.split('/').pop() || path;
const taskGateChoices = [
  { id: 'source_review', title: 'Source review' },
  { id: 'exact_head_ci', title: 'Exact-commit CI' },
  { id: 'installed_windows', title: 'Installed Windows acceptance' },
  { id: 'human_approval', title: 'Human approval' },
];

function App() {
  const [section, setSection] = useState('projects');
  const [projects, setProjects] = useState([]);
  const [workflowSteps, setWorkflowSteps] = useState([]);
  const [workflowRuns, setWorkflowRuns] = useState([]);
  const [workflowRunBusy, setWorkflowRunBusy] = useState(false);
  const [gitReviewPath, setGitReviewPath] = useState('');
  const [gitReview, setGitReview] = useState(null);
  const [gitStageReceipt, setGitStageReceipt] = useState(null);
  const [gitCommitReceipt, setGitCommitReceipt] = useState(null);
  const [gitCommitMessage, setGitCommitMessage] = useState('');
  const [gitAuthorName, setGitAuthorName] = useState('');
  const [gitAuthorEmail, setGitAuthorEmail] = useState('');
  const [gitWriteBusy, setGitWriteBusy] = useState(false);
  const [workflowProjectIds, setWorkflowProjectIds] = useState([]);
  const [workflowInspection, setWorkflowInspection] = useState(null);
  const [workflowRevalidated, setWorkflowRevalidated] = useState(false);
  const [workflowExport, setWorkflowExport] = useState(null);
  const [workflowBundleExport, setWorkflowBundleExport] = useState(null);
  const [workflowBusy, setWorkflowBusy] = useState(false);
  const workflowRequest = useRef(0);

  const [pending, setPending] = useState(false);
  const [problem, setProblem] = useState('');
  const [hint, setHint] = useState('');
  const [findOpen, setFindOpen] = useState(false);
  const [findQuery, setFindQuery] = useState('');
  const [replaceVisible, setReplaceVisible] = useState(false);
  const [replaceText, setReplaceText] = useState('');
  const [caseSensitiveFind, setCaseSensitiveFind] = useState(false);
  const [replaceRequest, setReplaceRequest] = useState(null);
  const [findIndex, setFindIndex] = useState(-1);
  const [findRequest, setFindRequest] = useState(null);
  const [cursorByTab, setCursorByTab] = useState({});
  const [referencePending, setReferencePending] = useState(false);
  const [activeProject, setActiveProject] = useState(null);
  const [directory, setDirectory] = useState('');
  const [listing, setListing] = useState(null);
  const [explorerPending, setExplorerPending] = useState(false);
  const [tabs, setTabs] = useState([]);
  const [activeTabKey, setActiveTabKey] = useState(null);
  const [savingKey, setSavingKey] = useState(null);
  const [review, setReview] = useState(null);
  const [reviewPending, setReviewPending] = useState(false);
  const [proposalInput, setProposalInput] = useState('');
  const [proposals, setProposals] = useState([]);
  const [proposalDetail, setProposalDetail] = useState(null);
  const [inboxBusy, setInboxBusy] = useState(false);
  const [capabilityCatalog, setCapabilityCatalog] = useState(null);
  const [languageServers, setLanguageServers] = useState([]);
  const [lspSession, setLspSession] = useState(null);
  const [lspBusy, setLspBusy] = useState(false);
  const [lspWorker, setLspWorker] = useState({
    ready: false, code: 'LSP_PROTECTED_WORKER_UNVERIFIED',
    detail: 'Protected language-server admission has not been accepted.'
  });
  const [lspNativeDiagnostics, setLspNativeDiagnostics] = useState({});
  const [lspCompletions, setLspCompletions] = useState(null);
  const [lspHover, setLspHover] = useState(null);
  const [lspJumpRequest, setLspJumpRequest] = useState(null);
  const [lspCompletionApplyRequest, setLspCompletionApplyRequest] = useState(null);
  const [cursorPositions, setCursorPositions] = useState({});

  const [capabilityBusy, setCapabilityBusy] = useState(false);
  const [projectContext, setProjectContext] = useState(null);
  const [intentList, setIntentList] = useState(null);
  const [intentRecipe, setIntentRecipe] = useState('lu_ide_host_tests');
  const [intentBusy, setIntentBusy] = useState(false);
  const [buildStatus, setBuildStatus] = useState(null);
  const [buildBusy, setBuildBusy] = useState(false);
  const [buildCancelBusy, setBuildCancelBusy] = useState(false);
  const [buildResult, setBuildResult] = useState(null);
  const buildStatusRequest = useRef(0);
  const [debugStatus, setDebugStatus] = useState(null);
  const [debugBusy, setDebugBusy] = useState(false);
  const [debugResult, setDebugResult] = useState(null);
  const [reviewIntentId, setReviewIntentId] = useState(null);
  const intentRequest = useRef(0);
  const [contextBusy, setContextBusy] = useState(false);
  const [checkpointBusy, setCheckpointBusy] = useState(false);
  const [taskIdInput, setTaskIdInput] = useState('');
  const [taskTitleInput, setTaskTitleInput] = useState('');
  const [taskStateInput, setTaskStateInput] = useState('in_progress');
  const [taskNextActionInput, setTaskNextActionInput] = useState('');
  const [taskRequiredGates, setTaskRequiredGates] = useState([
    'source_review', 'exact_head_ci',
  ]);
  const [taskBusy, setTaskBusy] = useState(false);
  const taskRequest = useRef(0);
  const contextRequest = useRef(0);
  const checkpointRequest = useRef(0);
  const [gitStatus, setGitStatus] = useState(null);
  const [gitBusy, setGitBusy] = useState(false);
  const [gitDiffScope, setGitDiffScope] = useState('worktree');
  const [gitDiff, setGitDiff] = useState(null);
  const [gitHistory, setGitHistory] = useState(null);
  const [gitEvidenceBusy, setGitEvidenceBusy] = useState(false);
  const gitEvidenceRequest = useRef(0);
  const [githubBusy, setGithubBusy] = useState(false);
  const [githubComparison, setGithubComparison] = useState(null);
  const [exactCi, setExactCi] = useState(null);
  const [exactCiBusy, setExactCiBusy] = useState(false);
  const exactCiRequest = useRef(0);
  const [snapshotBusy, setSnapshotBusy] = useState(false);
  const [resourceProbe, setResourceProbe] = useState(null);
  const [resourceProbeBusy, setResourceProbeBusy] = useState(false);
  const [inventoryComparison, setInventoryComparison] = useState(null);
  // CodeMirror EditorState is retained for each tab, including its undo stack.
  const editorStates = useRef(new Map());
  useEffect(() => {
    // Read-only, project-independent inventory. No provider lookup executes.
    Promise.all([invoke('ide_list_language_servers'), invoke('ide_lsp_worker_status')])
      .then(([servers, worker]) => {
        setLanguageServers(Array.isArray(servers) ? servers.slice(0, 16) : []);
        setLspWorker(worker && typeof worker.ready === 'boolean' ? worker : {
          ready: false, code: 'LSP_HOST_UNAVAILABLE',
          detail: 'Native language-service status is not a trusted response.'
        });
      })
      .catch(() => setLspWorker({
        ready: false, code: 'LSP_HOST_UNAVAILABLE',
        detail: 'Native language-service status could not be verified.'
      }));
  }, []);

  const activeProjectIdRef = useRef(null);
  activeProjectIdRef.current = activeProject?.projectId ?? null;

  useEffect(() => {
    // Invalidate in-flight project evidence when the selected project changes.
    gitEvidenceRequest.current += 1;
    setGitEvidenceBusy(false);
    setGitDiff(null);
    setGitHistory(null);
    setResourceProbe(null);
    contextRequest.current += 1;
    checkpointRequest.current += 1;
    setContextBusy(false);
    setCheckpointBusy(false);
    setProjectContext(null);
    setInventoryComparison(null);
    intentRequest.current += 1;
    setIntentList(null);
    setIntentBusy(false);
    setReviewIntentId(null);
    buildStatusRequest.current += 1;
    setBuildStatus(null);
    setBuildResult(null);
    setBuildBusy(false);
    setBuildCancelBusy(false);
    setDebugStatus(null);
    setDebugBusy(false);
    setDebugResult(null);
    setGitStatus(null);
    setGithubComparison(null);
    exactCiRequest.current += 1;
    setExactCi(null);
    setExactCiBusy(false);
    taskRequest.current += 1;
    setTaskBusy(false);
    setTaskIdInput('');
    setTaskTitleInput('');
    setTaskStateInput('in_progress');
    setTaskNextActionInput('');
    setTaskRequiredGates(['source_review', 'exact_head_ci']);
    // Invalidate any provider data from the previous workspace. No cross-
    // project response may be shown or applied to the newly selected editor.
    setLspNativeDiagnostics({});
    setLspCompletions(null);
    setLspHover(null);
    setLspJumpRequest(null);
    setLspCompletionApplyRequest(null);
  }, [activeProject?.projectId]);

  const activeTab = tabs.find(tab => tab.key === activeTabKey) || null;
  const currentLanguageId = languageForPath(activeTab?.relativePath);
  const lspAvailableForTab = !!lspWorker?.ready && !!activeTab &&
    !!currentLanguageId && Array.isArray(lspWorker.supportedLanguages) &&
    lspWorker.supportedLanguages.includes(currentLanguageId);
  const lspProviderForTab = currentLanguageId === 'rust' ? 'rust-analyzer' :
    currentLanguageId === 'python' ? 'Ruff' : 'clangd';
  const lspSupportsNavigation = currentLanguageId !== 'python';
  const lspRunningForTab = !!lspSession && !!activeTab &&
    lspSession.projectId === activeTab.projectId &&
    lspSession.relativePath === activeTab.relativePath;

  const activeLspDiagnostics = useMemo(() => activeTab
    ? visibleDiagnostics(activeTab, lspNativeDiagnostics[activeTab.key]) : [],
    [activeTab, lspNativeDiagnostics]);
  const activeLspCompletions = useMemo(() => activeTab && lspCompletions?.tabKey === activeTab.key
    ? visibleCompletions(activeTab, lspCompletions.response) : [],
    [activeTab, lspCompletions]);
  const activeLspHover = useMemo(() => activeTab && lspHover?.tabKey === activeTab.key
    ? visibleHover(activeTab, lspHover.response) : '',
    [activeTab, lspHover]);

  const projectTabs = tabs.filter(tab => tab.projectId === activeProject?.projectId);
  const anyDirty = tabs.some(tab => tab.dirty);
  const isExplorer = section === 'explorer';
  const isProposals = section === 'proposals';
  const isCapabilities = section === 'capabilities';
  const isLanguages = section === 'languages';
  const isContinuity = section === 'continuity';
  const isJobs = section === 'tasks';
  useEffect(() => {
    const projectId = activeProject?.projectId;
    if (!isJobs || !projectId) return;
    let alive = true;
    async function update() {
      try {
        const debug = await invoke('ide_debug_status', { projectId });
        if (alive && activeProjectIdRef.current === projectId &&
            debug && typeof debug.ready === 'boolean' &&
            Array.isArray(debug.journal) && Array.isArray(debug.quarantined))
          setDebugStatus(debug);
      } catch {
        if (alive && activeProjectIdRef.current === projectId)
          setDebugStatus({ ready:false, code:'DEBUG_HOST_UNAVAILABLE',
            description:'Protected debugger status unavailable', current:null, last:null,
            journal: [], quarantined: [] });
      }
      try {
        const value = await invoke('ide_build_status', { projectId });
        if (!alive || activeProjectIdRef.current !== projectId) return;
        if (value && typeof value.ready === 'boolean' &&
            Array.isArray(value.recent) && Array.isArray(value.journal) &&
            Array.isArray(value.quarantined))
          setBuildStatus(value);
      } catch {
        if (alive && activeProjectIdRef.current === projectId)
          setBuildStatus({
            ready:false, code:'BUILD_HOST_UNAVAILABLE',
            description:'Protected worker status unavailable',
            current:null, recent:[], quarantined:[]
          });
      }
    }
    void update();
    const poll = window.setInterval(() => { void update(); }, 1000);
    return () => { alive = false; window.clearInterval(poll); };
  }, [isJobs, activeProject?.projectId]);
  // Advisory only: derives labels from two existing, separately observed native responses.
  const evidenceReconciliation = activeProject?.projectId &&
    projectContext?.projectId === activeProject.projectId &&
    inventoryComparison?.projectId === activeProject.projectId
      ? reconcileDevelopmentEvidence(activeProject.projectId, projectContext, inventoryComparison, exactCi)
      : null;
  const isSource = section === 'source';
  const isPackages = section === 'packages';
  const parentPath = directory.split('/').slice(0, -1).join('/');
  // Both search and replacement operate on the active unsaved buffer only.
  // The pure planner counts matches before anything is changed.
  const matchPlan = locateMatches(activeTab?.draft ?? '', findQuery, caseSensitiveFind);
  const matches = matchPlan.matches;
  function navigateMatch(delta) {
    if (matchPlan.truncated || !matches.length || !activeTab) return;
    const next = findIndex < 0 ? (delta < 0 ? matches.length - 1 : 0) :
      (findIndex + delta + matches.length) % matches.length;
    setFindIndex(next);
    setFindRequest({ tabKey: activeTab.key,
      from: matches[next].from, to: matches[next].to, nonce: Date.now() });
  }

  function replaceMatches(all = false) {
    if (!activeTab || savingKey || explorerPending || !findQuery || matchPlan.truncated) return;
    const planned = planReplacement(activeTab.draft, findQuery, replaceText, {
      caseSensitive: caseSensitiveFind, all, selectedIndex: findIndex,
    });
    if (planned.status !== 'ready') return;
    if (all && !window.confirm(
      'Replace all ' + planned.count + ' match(es) in the unsaved editor buffer of ' +
      activeTab.relativePath + '? This does NOT save anything to disk. You can Undo.'
    )) return;
    setReplaceRequest({
      tabIdentity: activeTab.key + ':' + activeTab.generation,
      tabKey: activeTab.key, expectedDraft: activeTab.draft,
      changes: planned.changes, nonce: Date.now(),
    });
    setFindIndex(-1);
    setFindRequest(null);
    setHint((all ? 'Replaced ' + planned.count + ' matches' : 'Replaced one match') +
      ' in the editor draft. Use Undo to reverse or Save to write the file.');
    setProblem('');
  }

  async function copySourceReference(tab) {
    if (!tab || referencePending) return;
    if (tab.dirty) {
      setProblem('Save this file before copying a saved-source reference. Your unsaved changes are preserved.');
      return;
    }
    const line = cursorByTab[tab.key] || 1;
    setReferencePending(true);
    setProblem('');
    try {
      const span = await invoke('ide_read_source_span', {
        projectId: tab.projectId, relativePath: tab.relativePath,
        expectedRevision: tab.revision, firstLine: line, lastLine: line,
      });
      const reference = `ide-source:v1 project=${span.projectId} path=${JSON.stringify(span.relativePath)} revision=${span.revision} line=${line} bytes=${span.lines[0].startByte}..${span.lines[0].endByte}`;
      await navigator.clipboard.writeText(reference);
      setHint(`Copied source reference for ${tab.relativePath}, line ${line}.`);
    } catch (error) {
      setProblem(String(error).includes('FILE_CHANGED_ON_DISK')
        ? 'File changed on disk. Reload or review the latest source before copying a reference.'
        : 'Could not copy a verified source reference: ' + String(error));
    } finally { setReferencePending(false); }
  }

  async function refresh() {
    setPending(true);
    setProblem('');
    try {
      const next = await invoke('ide_list_projects');
      setProjects(next);
      if (activeProject && !next.some(project =>
        project.projectId === activeProject.projectId && project.active)) {
        setActiveProject(null);
        setListing(null);
        setSection('projects');
      }
    } catch (error) { setProblem(String(error)); }
    finally { setPending(false); }
  }

  function chooseWorkflowProject(projectId, selected) {
    workflowRequest.current += 1;
    setWorkflowInspection(null);
    setWorkflowRevalidated(false);
    setWorkflowExport(null);
    setWorkflowBundleExport(null);
    setWorkflowProjectIds(previous =>
      selected ? [...new Set([...previous, projectId])].slice(0, 8)
        : previous.filter(id => id !== projectId));
  }

  async function previewMultiProjectWorkflow() {
    const chosen = [...workflowProjectIds];
    if (chosen.length === 0 || chosen.length > 8) return;
    const serial = ++workflowRequest.current;
    setWorkflowBusy(true);
    setWorkflowInspection(null);
    setWorkflowRevalidated(false);
    setWorkflowExport(null);
    setWorkflowBundleExport(null);
    setProblem('');
    try {
      const value = await invoke('ide_workflow_preview', { projectIds: chosen });
      if (serial === workflowRequest.current) setWorkflowInspection(value);
    } catch (error) {
      if (serial === workflowRequest.current) setProblem('Native workflow inspection rejected: ' + String(error));
    } finally {
      if (serial === workflowRequest.current) setWorkflowBusy(false);
    }
  }

  async function revalidateMultiProjectWorkflow() {
    if (!workflowInspection || workflowBusy) return;
    const serial = ++workflowRequest.current;
    const projectIds = workflowInspection.projects.map(project => project.projectId);
    setWorkflowBusy(true);
    setWorkflowRevalidated(false);
    setProblem('');
    try {
      const verified = await invoke('ide_workflow_verify', {
        projectIds, expectedFingerprint: workflowInspection.fingerprint,
      });
      if (serial === workflowRequest.current &&
          verified.fingerprint === workflowInspection.fingerprint) {
        setWorkflowRevalidated(true);
        setHint('Exact workflow evidence rechecked. No package, Git write, or execution occurred.');
      }
    } catch (error) {
      if (serial === workflowRequest.current) {
        setWorkflowInspection(null);
        setProblem('Workflow changed or is blocked; inspect again before review: ' + String(error));
      }
    } finally {
      if (serial === workflowRequest.current) setWorkflowBusy(false);
    }
  }

  async function exportMultiProjectManifest() {
    if (!workflowInspection?.readyForReview || !workflowRevalidated || workflowBusy) return;
    const serial = ++workflowRequest.current;
    const projectIds = workflowInspection.projects.map(item => item.projectId);
    setWorkflowBusy(true);
    setProblem('');
    try {
      // The destination is selected in a native OS dialog by the trusted Tauri
      // host. The WebView never supplies or receives an absolute path.
      const exported = await invoke('ide_workflow_export_manifest', {
        projectIds, expectedFingerprint: workflowInspection.fingerprint,
      });
      if (serial === workflowRequest.current) {
        setWorkflowExport(exported);
        setWorkflowRevalidated(false); // one explicit review per export
        setHint('Workflow review manifest exported through native folder picker.');
      }
    } catch (error) {
      if (serial === workflowRequest.current) {
        setWorkflowRevalidated(false);
        setProblem('Workflow manifest export denied or cancelled: ' + String(error));
      }
    } finally {
      if (serial === workflowRequest.current) setWorkflowBusy(false);
    }
  }

  async function exportMultiProjectBundle() {
    if (!workflowInspection?.readyForReview || !workflowRevalidated || workflowBusy) return;
    const serial = ++workflowRequest.current;
    const projectIds = workflowInspection.projects.map(item => item.projectId);
    setWorkflowBusy(true);
    setProblem('');
    try {
      // ZIP contains only verified previously discovered project artifacts.
      // Native OS selection controls destination; no arbitrary path in IPC.
      const zipped = await invoke('ide_workflow_export_bundle', {
        projectIds, expectedFingerprint: workflowInspection.fingerprint,
      });
      if (serial === workflowRequest.current) {
        setWorkflowBundleExport(zipped);
        setWorkflowRevalidated(false);
        setHint('Verified artifact ZIP created without running project code or modifying Git.');
      }
    } catch (error) {
      if (serial === workflowRequest.current) {
        setWorkflowRevalidated(false);
        setProblem('Workflow archive denied or cancelled: ' + String(error));
      }
    } finally {
      if (serial === workflowRequest.current) setWorkflowBusy(false);
    }
  }

  async function reviewGitFileForStage() {
    if (!activeProject || gitWriteBusy || !gitReviewPath.trim()) return;
    setGitWriteBusy(true);
    setGitReview(null); setGitStageReceipt(null); setGitCommitReceipt(null);
    setProblem('');
    try {
      const preview = await invoke('ide_git_review_file', {
        projectId: activeProject.projectId, relativePath: gitReviewPath.trim(),
      });
      if (preview.projectId === activeProject.projectId) setGitReview(preview);
    } catch (error) { setProblem('One-file Git preview denied: ' + String(error)); }
    finally { setGitWriteBusy(false); }
  }

  async function stageReviewedGitFile() {
    if (!gitReview || gitWriteBusy || !activeProject ||
        gitReview.projectId !== activeProject.projectId) return;
    setGitWriteBusy(true);
    setProblem('');
    try {
      const staged = await invoke('ide_git_stage_reviewed', { expected: gitReview });
      if (staged.projectId === activeProject.projectId &&
          staged.sourceSha256 === gitReview.worktreeSha256) {
        setGitStageReceipt(staged); setGitReview(null);
        setHint('One reviewed source file staged using native Git. A separate explicit commit is required.');
      }
    } catch (error) { setGitReview(null); setProblem('Native Git stage denied: ' + String(error)); }
    finally { setGitWriteBusy(false); void refreshGitStatus(); }
  }

  async function commitReviewedGitFile() {
    if (!gitStageReceipt || gitWriteBusy || !activeProject ||
        gitStageReceipt.projectId !== activeProject.projectId) return;
    setGitWriteBusy(true); setProblem('');
    try {
      const done = await invoke('ide_git_commit_reviewed', {
        stage: gitStageReceipt, message: gitCommitMessage,
        authorName: gitAuthorName, authorEmail: gitAuthorEmail,
      });
      if (done.projectId === activeProject.projectId) {
        setGitCommitReceipt(done); setGitStageReceipt(null); setGitCommitMessage('');
        setHint('Local Git commit created. No hooks, push or release action occurred.');
      }
    } catch (error) { setProblem('Native Git commit denied: ' + String(error)); }
    finally { setGitWriteBusy(false); void refreshGitStatus(); }
  }

  function addWorkflowStep() {
    if (!workflowInspection?.readyForReview || workflowSteps.length >= 12) return;
    const choices = workflowInspection.projects;
    if (!choices.length) return;
    setWorkflowSteps(before => [...before, {
      projectId: choices[0].projectId, recipeId: 'rust_check',
      dependsOn: before.length ? [before.length - 1] : [],
    }]);
  }

  async function readWorkflowRuns() {
    try {
      const runs = await invoke('ide_workflow_list_runs');
      setWorkflowRuns(Array.isArray(runs) ? runs.slice(-16) : []);
    } catch (error) { setProblem('Durable workflow status unavailable: ' + String(error)); }
  }

  async function createDurableWorkflow() {
    if (!workflowInspection?.readyForReview || !workflowRevalidated ||
        workflowRunBusy || !workflowSteps.length) return;
    setWorkflowRunBusy(true);
    setProblem('');
    try {
      const run = await invoke('ide_workflow_create', {
        projectIds: workflowInspection.projects.map(item => item.projectId),
        expectedFingerprint: workflowInspection.fingerprint, steps: workflowSteps,
      });
      setWorkflowRuns(prev => [...prev, run].slice(-16));
      setWorkflowRevalidated(false);
      setWorkflowSteps([]);
      setHint('Durable dependency workflow recorded; no worker was started.');
    } catch (error) { setProblem('Workflow creation denied: ' + String(error)); }
    finally { setWorkflowRunBusy(false); }
  }

  async function runDurableStep(run, index) {
    if (workflowRunBusy) return;
    setWorkflowRunBusy(true);
    setProblem('');
    try {
      const updated = await invoke('ide_workflow_run_step', {
        workflowId: run.workflowId, step: index,
      });
      setWorkflowRuns(prev => prev.map(item =>
        item.workflowId === updated.workflowId ? updated : item));
      setHint('Native step returned. Inspect OS receipts and any dependency quarantine.');
    } catch (error) {
      setProblem('Protected workflow step was denied or interrupted: ' + String(error));
      await readWorkflowRuns();
    } finally { setWorkflowRunBusy(false); }
  }

  async function cancelDurableStep(run, index) {
    if (workflowRunBusy) return;
    setWorkflowRunBusy(true);
    setProblem('');
    try {
      const updated = await invoke('ide_workflow_cancel_pending', {
        workflowId: run.workflowId, step: index,
      });
      setWorkflowRuns(prev => prev.map(item =>
        item.workflowId === updated.workflowId ? updated : item));
    } catch (error) { setProblem('Cancellation denied: ' + String(error)); }
    finally { setWorkflowRunBusy(false); }
  }

  async function refreshGitStatus() {
    if (!activeProject?.projectId) {
      setGitStatus(null);
      return;
    }
    setGitBusy(true);
    setGitDiff(null);
    setGitHistory(null);
    gitEvidenceRequest.current += 1;
    setGithubComparison(null); // A local refresh invalidates any prior live comparison.
    exactCiRequest.current += 1;
    setExactCi(null);
    setExactCiBusy(false);
    setProblem('');
    try {
      setGitStatus(await invoke('ide_read_git_status', { projectId: activeProject.projectId }));
    } catch (error) {
      setGitStatus(null);
      setProblem('Could not inspect source control: ' + String(error));
    } finally { setGitBusy(false); }
  }

  async function loadGitEvidence() {
    if (!activeProject?.projectId || gitEvidenceBusy) return;
    const projectId = activeProject.projectId;
    const request = ++gitEvidenceRequest.current;
    setGitEvidenceBusy(true);
    setGitDiff(null);
    setGitHistory(null);
    setProblem('');
    try {
      const [diff, history] = await Promise.all([
        invoke('ide_read_git_diff', { projectId, scope: gitDiffScope }),
        invoke('ide_read_git_history', { projectId }),
      ]);
      if (request !== gitEvidenceRequest.current) return;
      if (diff.projectId !== projectId || history.projectId !== projectId) {
        throw Error('GIT_PROJECT_CONTEXT_CHANGED');
      }
      setGitDiff(diff);
      setGitHistory(history);
    } catch (error) {
      if (request === gitEvidenceRequest.current)
        setProblem('Could not inspect local Git diff/history: ' + String(error));
    } finally {
      if (request === gitEvidenceRequest.current) setGitEvidenceBusy(false);
    }
  }

  async function compareLiveGitHub() {
    if (!activeProject?.projectId || githubBusy) return;
    setGithubBusy(true);
    setGithubComparison(null);
    setProblem('');
    try {
      const observed = await invoke('ide_compare_live_github', {
        projectId: activeProject.projectId,
      });
      setGithubComparison(observed);
    } catch (error) {
      setProblem('Could not compare with GitHub: ' + String(error));
    } finally { setGithubBusy(false); }
  }

  async function inspectExactHeadCi() {
    if (!activeProject?.projectId || exactCiBusy || gitBusy) return;
    const projectId = activeProject.projectId;
    const request = ++exactCiRequest.current;
    setExactCiBusy(true);
    setExactCi(null);
    setProblem('');
    try {
      const result = await invoke('ide_read_exact_head_ci', { projectId });
      if (request !== exactCiRequest.current || activeProjectIdRef.current !== projectId) return;
      if (result.projectId !== projectId) throw Error('GITHUB_CI_PROJECT_CONTEXT_CHANGED');
      setExactCi(result);
    } catch (error) {
      if (request === exactCiRequest.current && activeProjectIdRef.current === projectId)
        setProblem('Could not inspect exact-commit CI: ' + String(error));
    } finally {
      if (request === exactCiRequest.current) setExactCiBusy(false);
    }
  }

  async function refreshIntents() {
    const projectId = activeProject?.projectId;
    if (!projectId) { setIntentList(null); return; }
    const request = ++intentRequest.current;
    setIntentBusy(true);
    setProblem('');
    try {
      const value = await invoke('ide_list_execution_intents', { projectId });
      if (request !== intentRequest.current || activeProjectIdRef.current !== projectId) return;
      if (value.projectId !== projectId || value.workerExecutionEnabled !== false)
        throw Error('EXECUTION_INTENT_BOUNDARY_MISMATCH');
      setIntentList(value);
    } catch (error) {
      if (request === intentRequest.current && activeProjectIdRef.current === projectId) {
        setIntentList(null);
        setProblem('Could not read job-intent history: ' + String(error));
      }
    } finally {
      if (request === intentRequest.current) setIntentBusy(false);
    }
  }

  async function prepareIntent() {
    const projectId = activeProject?.projectId;
    if (!projectId || intentBusy) return;
    const request = ++intentRequest.current;
    setIntentBusy(true);
    setReviewIntentId(null);
    setProblem('');
    try {
      const prepared = await invoke('ide_prepare_execution_intent', {
        projectId, recipeId: intentRecipe,
      });
      if (request !== intentRequest.current || activeProjectIdRef.current !== projectId) return;
      if (prepared.projectId !== projectId || prepared.execution !== 'never_started'
          || prepared.grantTokenIssued !== false)
        throw Error('EXECUTION_PREPARE_UNEXPECTED_AUTHORITY');
      const value = await invoke('ide_list_execution_intents', { projectId });
      if (request !== intentRequest.current || activeProjectIdRef.current !== projectId) return;
      if (value.projectId !== projectId || value.workerExecutionEnabled !== false)
        throw Error('EXECUTION_HISTORY_BOUNDARY_MISMATCH');
      setIntentList(value);
      setHint('Job request prepared for review. No worker or build/test process was started.');
    } catch (error) {
      if (request === intentRequest.current && activeProjectIdRef.current === projectId)
        setProblem('Could not prepare job intent: ' + String(error));
    } finally {
      if (request === intentRequest.current) setIntentBusy(false);
    }
  }

  async function decideIntent(intent, decision) {
    const projectId = activeProject?.projectId;
    if (!projectId || intentBusy || intent.projectId !== projectId) return;
    if (decision === 'record_review' && reviewIntentId !== intent.intentId) {
      setProblem('Review the exact source and manifest, then confirm the no-execution notice.');
      return;
    }
    const request = ++intentRequest.current;
    setIntentBusy(true);
    setProblem('');
    try {
      const updated = await invoke('ide_decide_execution_intent', {
        projectId, intentId: intent.intentId, decision,
      });
      if (request !== intentRequest.current || activeProjectIdRef.current !== projectId) return;
      if (updated.intentId !== intent.intentId || updated.execution !== 'never_started'
          || updated.grantTokenIssued !== false)
        throw Error('EXECUTION_REVIEW_UNEXPECTED_AUTHORITY');
      const value = await invoke('ide_list_execution_intents', { projectId });
      if (request !== intentRequest.current || activeProjectIdRef.current !== projectId) return;
      if (value.projectId !== projectId || value.workerExecutionEnabled !== false)
        throw Error('EXECUTION_HISTORY_BOUNDARY_MISMATCH');
      setIntentList(value);
      setReviewIntentId(null);
      setHint(decision === 'cancel'
        ? 'Job request cancelled or review withdrawn. No execution occurred.'
        : 'Human review recorded. Execution remains unavailable until OS-supervised workers pass acceptance.');
    } catch (error) {
      if (request === intentRequest.current && activeProjectIdRef.current === projectId)
        setProblem('Could not update job intent: ' + String(error));
    } finally {
      if (request === intentRequest.current) setIntentBusy(false);
    }
  }

  async function refreshProtectedBuildStatus() {
    const projectId = activeProject?.projectId;
    if (!projectId) { setBuildStatus(null); return; }
    const request = ++buildStatusRequest.current;
    try {
      const value = await invoke('ide_build_status', { projectId });
      if (request !== buildStatusRequest.current ||
          activeProjectIdRef.current !== projectId) return;
      if (!value || typeof value.ready !== 'boolean' ||
          !Array.isArray(value.recent) || !Array.isArray(value.journal) ||
           !Array.isArray(value.quarantined))
        throw Error('BUILD_STATUS_SHAPE_DENIED');
      setBuildStatus(value);
    } catch {
      if (request === buildStatusRequest.current &&
          activeProjectIdRef.current === projectId)
        setBuildStatus({
          ready: false, code: 'BUILD_HOST_UNAVAILABLE',
          description: 'Trusted native worker status is unavailable.',
          current: null, recent: [], journal: [], quarantined: []
        });
    }
  }

  async function startProtectedBuild() {
    const projectId = activeProject?.projectId;
    if (!projectId || buildBusy || !buildStatus?.ready ||
        buildStatus.current || !['rust_check', 'rust_tests', 'lu_ide_host_tests'].includes(intentRecipe)) return;
    setBuildBusy(true);
    setBuildResult(null);
    setProblem('');
    try {
      // This is a REQUEST, not an execution grant. A separate native OS
      // confirmation follows sealed-source and release-pin admission.
      const outcome = await invoke('ide_build_start', {
        projectId, recipeId: intentRecipe
      });
      if (activeProjectIdRef.current !== projectId) return;
      if (!outcome || outcome.projectId !== projectId ||
          outcome.recipeId !== intentRecipe || !/^[a-f0-9]{64}$/.test(outcome.planSha256))
        throw Error('BUILD_RESULT_SCOPE_DENIED');
      setBuildResult(outcome);
      setHint('Protected worker returned a durable result. Native and installed acceptance are separate gates.');
    } catch (error) {
      if (activeProjectIdRef.current === projectId)
        setProblem('Protected build/test was denied or failed: ' + String(error));
    } finally {
      setBuildBusy(false);
      await refreshProtectedBuildStatus();
    }
  }

  async function cancelProtectedBuild() {
    const projectId = activeProject?.projectId;
    const current = buildStatus?.current;
    if (!projectId || !current || current.projectId !== projectId ||
        buildCancelBusy) return;
    setBuildCancelBusy(true);
    try {
      await invoke('ide_build_cancel', { projectId, jobId: current.jobId || null });
      if (activeProjectIdRef.current === projectId)
        setHint('Cancellation requested. Worker is not marked cancelled until the OS process tree is verified empty.');
    } catch (error) {
      if (activeProjectIdRef.current === projectId)
        setProblem('Could not request protected cancellation: ' + String(error));
    } finally {
      setBuildCancelBusy(false);
      await refreshProtectedBuildStatus();
    }
  }

  async function reconcileProtectedBuild(job) {
    const projectId = activeProject?.projectId;
    if (!job || job.projectId !== projectId ||
        !job.requiresReconciliation || buildBusy) return;
    setBuildBusy(true);
    setProblem('');
    try {
      await invoke('ide_build_reconcile', { projectId, jobId: job.jobId });
      if (activeProjectIdRef.current === projectId)
        setHint('Interrupted worker reconciliation verified by the native OS broker. No job was restarted.');
    } catch (error) {
      if (activeProjectIdRef.current === projectId)
        setProblem('Kernel recovery remains quarantined: ' + String(error));
    } finally {
      setBuildBusy(false);
      await refreshProtectedBuildStatus();
    }
  }

  async function runProtectedDebuggerFixture() {
    const projectId = activeProject?.projectId;
    if (!projectId || !debugStatus?.ready || !!debugStatus.current || debugBusy) return;
    setDebugBusy(true);
    setProblem('');
    setDebugResult(null);
    try {
      const value = await invoke('ide_debug_run_fixture', { projectId });
      if (activeProjectIdRef.current !== projectId) return;
      if (!value || value.projectId !== projectId ||
          value.target !== 'fixed_installer_selftest_only_not_project_debugger')
        throw Error('DEBUG_RESULT_SCOPE_DENIED');
      setDebugResult(value);
      setHint('The fixed debugger self-test returned a native stop/stack and cleanup receipt.');
    } catch (error) {
      if (activeProjectIdRef.current === projectId)
        setProblem('Protected debugger self-test denied or failed: ' + String(error));
    } finally {
      setDebugBusy(false);
      const current = activeProjectIdRef.current;
      if (current === projectId) {
        invoke('ide_debug_status', { projectId }).then(setDebugStatus).catch(() => {});
      }
    }
  }

  async function reconcileProtectedDebugger(item) {
    const projectId = activeProject?.projectId;
    if (!item || item.projectId !== projectId ||
        !item.requiresReconciliation || debugBusy) return;
    setDebugBusy(true);
    setProblem('');
    try {
      await invoke('ide_debug_reconcile', { projectId, jobId: item.jobId });
      if (activeProjectIdRef.current === projectId)
        setHint('Interrupted debugger was cleaned by the native OS broker; nothing was restarted.');
    } catch (error) {
      if (activeProjectIdRef.current === projectId)
        setProblem('Debug process tree remains quarantined: ' + String(error));
    } finally {
      setDebugBusy(false);
      if (activeProjectIdRef.current === projectId)
        invoke('ide_debug_status', { projectId }).then(setDebugStatus).catch(() => {});
    }
  }

  async function cancelProtectedDebugger() {
    const projectId = activeProject?.projectId;
    if (!projectId || debugStatus?.current?.projectId !== projectId) return;
    try {
      await invoke('ide_debug_cancel', {
        projectId, jobId: debugStatus.current.jobId || null
      });
      setHint('Debugger cancellation requested; OS process tree must be verified empty.');
    } catch (error) {
      setProblem('Debugger cancellation denied: ' + String(error));
    }
  }

  async function refreshContext() {
    const projectId = activeProject?.projectId;
    if (!projectId) {
      setProjectContext(null);
      setInventoryComparison(null);
      return;
    }
    const request = ++contextRequest.current;
    setContextBusy(true);
    setProblem('');
    try {
      const [latest, compared] = await Promise.all([
        invoke('ide_read_project_context', { projectId }),
        invoke('ide_compare_project_inventory', { projectId }),
      ]);
      if (request !== contextRequest.current || activeProjectIdRef.current !== projectId) return;
      if (latest.projectId !== projectId || compared.projectId !== projectId)
        throw Error('PROJECT_CONTEXT_CHANGED');
      setProjectContext(latest);
      setInventoryComparison(compared);
    } catch (error) {
      if (request === contextRequest.current) {
        setProjectContext(null);
        setInventoryComparison(null);
        setProblem('Could not read project continuity: ' + String(error));
      }
    } finally {
      if (request === contextRequest.current) setContextBusy(false);
    }
  }

  function editTask(task) {
    if (taskBusy) return;
    setTaskIdInput(task.taskId);
    setTaskTitleInput(task.title);
    setTaskStateInput(task.status);
    setTaskNextActionInput(task.nextAction);
    setTaskRequiredGates(task.requiredGates);
    setHint('Task loaded for revision. Saving appends a new historical event; it never rewrites old evidence.');
  }

  async function recordTaskUpdate() {
    const projectId = activeProject?.projectId;
    const taskId = taskIdInput.trim();
    const title = taskTitleInput.trim();
    const nextAction = taskNextActionInput.trim();
    if (!projectId || taskBusy || contextBusy) return;
    if (!/^[A-Za-z0-9_.-]{1,64}$/.test(taskId) ||
        !title || title.length > 120 || !nextAction || nextAction.length > 240 ||
        !taskRequiredGates.length) {
      setProblem('Enter a task ID, short title, next action, and at least one acceptance gate.');
      return;
    }
    const request = ++taskRequest.current;
    setTaskBusy(true);
    setProblem('');
    try {
      const recorded = await invoke('ide_record_task_update', {
        projectId, taskId, title, status: taskStateInput, nextAction,
        requiredGates: taskRequiredGates,
      });
      if (request !== taskRequest.current || activeProjectIdRef.current !== projectId) return;
      if (recorded.taskId !== taskId) throw Error('TASK_REVISION_MISMATCH');
      setHint('Task progress recorded in the private development ledger. Acceptance is not verified.');
      await refreshContext();
    } catch (error) {
      if (request === taskRequest.current && activeProjectIdRef.current === projectId)
        setProblem('Could not record task progress: ' + String(error));
    } finally {
      if (request === taskRequest.current) setTaskBusy(false);
    }
  }

  async function observeTaskCi(task) {
    const projectId = activeProject?.projectId;
    if (!projectId || taskBusy || contextBusy || !task.linkedCommit) return;
    const request = ++taskRequest.current;
    setTaskBusy(true);
    setProblem('');
    try {
      // Native host fetches CI itself; no status or GitHub receipt comes from the UI.
      const updated = await invoke('ide_observe_task_ci', {
        projectId, taskId: task.taskId,
      });
      if (request !== taskRequest.current || activeProjectIdRef.current !== projectId) return;
      if (updated.taskId !== task.taskId) throw Error('TASK_EVIDENCE_MISMATCH');
      setHint('Exact-commit CI observation linked to task revision; installed app and task acceptance remain open.');
      await refreshContext();
    } catch (error) {
      if (request === taskRequest.current && activeProjectIdRef.current === projectId)
        setProblem('Could not attach task CI observation: ' + String(error));
    } finally {
      if (request === taskRequest.current) setTaskBusy(false);
    }
  }

  async function copyDevelopmentCheckpoint() {
    const projectId = activeProject?.projectId;
    if (!projectId || checkpointBusy || contextBusy) return;
    const request = ++checkpointRequest.current;
    setCheckpointBusy(true);
    setProblem('');
    try {
      // Never copy a remembered UI snapshot: re-observe both independent native sources.
      const [context, comparison] = await Promise.all([
        invoke('ide_read_project_context', { projectId }),
        invoke('ide_compare_project_inventory', { projectId }),
      ]);
      if (request !== checkpointRequest.current || activeProjectIdRef.current !== projectId) return;
      const checkpoint = buildDevelopmentCheckpoint(projectId, context, comparison,
        new Date().toISOString(), exactCi);
      if (typeof navigator.clipboard?.writeText !== 'function')
        throw Error('CLIPBOARD_UNAVAILABLE');
      await navigator.clipboard.writeText(JSON.stringify(checkpoint, null, 2));
      if (request !== checkpointRequest.current || activeProjectIdRef.current !== projectId) return;
      setProjectContext(context);
      setInventoryComparison(comparison);
      setHint('Read-only development checkpoint copied. Local checks are not CI or installer acceptance.');
    } catch (error) {
      if (request === checkpointRequest.current && activeProjectIdRef.current === projectId)
        setProblem('Could not copy development checkpoint: ' + String(error));
    } finally {
      if (request === checkpointRequest.current) setCheckpointBusy(false);
    }
  }

  async function inspectResource(relativePath) {
    if (!activeProject?.projectId || resourceProbeBusy) return;
    const projectId = activeProject.projectId;
    setResourceProbeBusy(true);
    setResourceProbe(null);
    setProblem('');
    try {
      const observed = await invoke('ide_probe_project_resource', {
        projectId, relativePath,
      });
      if (observed.projectId !== projectId || observed.relativePath !== relativePath) {
        throw Error('RESOURCE_PROBE_CONTEXT_CHANGED');
      }
      setResourceProbe(observed);
    } catch (error) {
      setProblem('Could not verify current resource bytes: ' + String(error));
    } finally { setResourceProbeBusy(false); }
  }

  async function captureInventorySnapshot() {
    if (!activeProject?.projectId || snapshotBusy) return;
    setSnapshotBusy(true);
    setProblem('');
    try {
      const recorded = await invoke('ide_capture_project_inventory', {
        projectId: activeProject.projectId,
      });
      setHint('Evidence snapshot recorded for ' + recorded.artifacts.length +
        ' files. This does not verify an installer or execute a tool.');
      await refreshContext();
    } catch (error) {
      setProblem('Could not record project evidence: ' + String(error));
    } finally {
      setSnapshotBusy(false);
    }
  }

  async function refreshCapabilities() {
    setCapabilityBusy(true);
    setProblem('');
    try {
      setCapabilityCatalog(await invoke('ide_list_capabilities'));
    } catch (error) {
      setProblem('Could not inspect IDE capabilities: ' + String(error));
    } finally {
      setCapabilityBusy(false);
    }
  }

  async function refreshProposals() {
    setInboxBusy(true);
    setProblem('');
    try {
      setProposals(await invoke('ide_list_proposals'));
    } catch (error) { setProblem(String(error)); }
    finally { setInboxBusy(false); }
  }

  // Useful for local UI tests even while Lu is disconnected. The generated
  // JSON is DATA only: queue/import/review never runs code or saves a file.
  function prepareLocalProposal() {
    if (!activeTab) {
      setProblem('Open a source file in the Explorer before preparing a proposal.');
      return;
    }
    setProposalInput(JSON.stringify({
      schemaVersion: 1,
      taskId: 'local-' + Date.now(),
      projectId: activeTab.projectId,
      claimedProducer: 'Local draft (unverified)',
      rationale: 'Review this proposed edit before staging anything.',
      edits: [{
        relativePath: activeTab.relativePath,
        expectedRevision: activeTab.revision,
        proposedText: activeTab.draft,
      }],
    }, null, 2));
    setHint('Prepared a local proposal draft. Edit the JSON, then add it to the inbox.');
    setProblem('');
  }

  async function importProposal() {
    if (!proposalInput.trim() || inboxBusy) return;
    setInboxBusy(true);
    setProblem('');
    try {
      await invoke('ide_import_proposal', { proposalJson: proposalInput });
      setProposals(await invoke('ide_list_proposals'));
      setProposalInput('');
      setHint('Proposal added to the review inbox. No project files were changed.');
    } catch (error) {
      setProblem(String(error).includes('FILE_CHANGED_ON_DISK')
        ? 'File changed on disk. Refresh the source revision before importing this proposal.'
        : String(error));
    } finally { setInboxBusy(false); }
  }

  async function openProposal(inboxId) {
    if (inboxBusy) return;
    setInboxBusy(true);
    setProblem('');
    try {
      setProposalDetail(await invoke('ide_review_proposal', { inboxId }));
    } catch (error) {
      setProblem(String(error).includes('FILE_CHANGED_ON_DISK')
        ? 'File changed on disk. The proposal no longer matches the current source.'
        : String(error));
    } finally { setInboxBusy(false); }
  }

  async function decideProposal(approveForStaging) {
    const detail = proposalDetail;
    if (!detail || inboxBusy || detail.summary.status !== 'pending') return;
    if (approveForStaging && detail.reviews.some(item => item.truncated || !item.countsExact)) {
      setProblem('Review incomplete. This proposal cannot be approved for staging.');
      return;
    }
    const msg = approveForStaging
      ? 'Approve this exact proposal for manual staging? This records a review decision only. It does NOT edit, save, or execute any files.'
      : 'Reject this proposal? No project files will be changed.';
    if (!window.confirm(msg)) return;
    setInboxBusy(true);
    setProblem('');
    try {
      await invoke('ide_decide_proposal', {
        inboxId: detail.summary.inboxId,
        expectedDigest: detail.summary.digest,
        approveForStaging,
      });
      setProposals(await invoke('ide_list_proposals'));
      setProposalDetail(null);
      setHint(approveForStaging
        ? 'Review approval recorded. No source files were modified.'
        : 'Proposal rejected. No source files were modified.');
    } catch (error) { setProblem(String(error)); }
    finally { setInboxBusy(false); }
  }

  async function stageProposal(item) {
    if (inboxBusy || pending || savingKey || explorerPending) return;
    const project = projects.find(entry => entry.projectId === item.projectId && entry.active);
    if (!project) {
      setProblem('Open this project from the Projects screen before staging its proposed edits.');
      return;
    }
    if (tabs.some(tab => tab.projectId === item.projectId && tab.dirty) &&
      !window.confirm('Staging will replace unsaved drafts in this project. Continue? No disk files will be changed.')) {
      return;
    }
    setInboxBusy(true);
    setProblem('');
    try {
      const approved = await invoke('ide_stage_proposal', {
        inboxId: item.inboxId,
        expectedDigest: item.digest,
      });
      // Every source is reread before changing editor buffers. No partial
      // staging occurs when one file became unavailable or changed on disk.
      const originals = await Promise.all(approved.edits.map(edit =>
        invoke('ide_preview_source_file', {
          projectId: item.projectId,
          relativePath: edit.relativePath,
        })
      ));
      for (let index = 0; index < originals.length; index++) {
        if (originals[index].revision !== approved.edits[index].expectedRevision) {
          throw new Error('File changed on disk. Nothing was staged.');
        }
      }
      const rootListing = await invoke('ide_list_source_directory', {
        projectId: item.projectId, relativeDir: '',
      });
      const stagedTabs = approved.edits.map((edit, index) => {
        const key = keyFor(item.projectId, edit.relativePath);
        const previous = tabs.find(tab => tab.key === key);
        editorStates.current.set(key, createEditorState(
          edit.proposedText, edit.relativePath,
          (state, draft) => onEditorChange(key, state, draft)
        ));
        return {
          key, projectId: item.projectId, relativePath: edit.relativePath,
          revision: edit.expectedRevision, savedText: originals[index].text,
          draft: edit.proposedText,
          dirty: edit.proposedText !== originals[index].text,
          generation: (previous?.generation ?? 0) + 1,
          bufferRevision: (previous?.bufferRevision ?? 0) + 1,
        };
      });
      const stagedKeys = new Set(stagedTabs.map(tab => tab.key));
      setTabs(previous => [...previous.filter(tab => !stagedKeys.has(tab.key)), ...stagedTabs]);
      setActiveProject(project);
      setListing(rootListing);
      setDirectory('');
      setSection('explorer');
      setActiveTabKey(stagedTabs[0].key);
      setProposalDetail(null);
      setHint('Proposal staged as editor drafts. Review and Save each file yourself; nothing was written to disk.');
    } catch (error) {
      setProblem(String(error).includes('FILE_CHANGED_ON_DISK')
        ? 'File changed on disk. The proposal was not staged.'
        : String(error));
    } finally { setInboxBusy(false); }
  }

  async function discardProposal(item) {
    if (inboxBusy || !window.confirm('Dismiss this proposal from the IDE inbox? This will not delete any project files.')) return;
    setInboxBusy(true);
    setProblem('');
    try {
      setProposals(await invoke('ide_discard_proposal', { inboxId: item.inboxId }));
      setProposalDetail(null);
      setHint('Proposal dismissed from the review inbox.');
    } catch (error) { setProblem(String(error)); }
    finally { setInboxBusy(false); }
  }

  useEffect(() => { void refresh(); }, []);

  function discardProjectTabs(projectId) {
    setReview(previous => previous?.projectId === projectId ? null : previous);
    const affected = tabs.filter(tab => tab.projectId === projectId);
    affected.forEach(tab => editorStates.current.delete(tab.key));
    setTabs(previous => previous.filter(tab => tab.projectId !== projectId));
    if (activeTabKey && affected.some(tab => tab.key === activeTabKey)) {
      setActiveTabKey(null);
    }
  }

  async function operation(command, args) {
    if (pending || savingKey || explorerPending) return;
    if (command === 'ide_close_project' &&
      tabs.some(tab => tab.projectId === args.projectId && tab.dirty) &&
      !window.confirm('Discard unsaved changes in this project?')) return;
    setPending(true);
    setProblem('');
    setHint('');
    try {
      const next = await invoke(command, args);
      setProjects(next);
      // Do not discard unsaved buffers until the native close succeeds.
      if (command === 'ide_close_project' || command === 'ide_unregister_project') {
        discardProjectTabs(args.projectId);
      }
      if (command === 'ide_choose_project') {
        setHint('Project selection complete. No scripts or project tools were executed.');
      }
      if ((command === 'ide_close_project' || command === 'ide_unregister_project') &&
        activeProject?.projectId === args?.projectId) {
        setActiveProject(null);
        setListing(null);
        setSection('projects');
      }
    } catch (error) { setProblem(String(error)); }
    finally { setPending(false); }
  }

  // This operation only removes the IDE's registration. It never issues a
  // filesystem deletion; the native host cannot accept an OS path from here.
  function confirmUnregister(project) {
    if (pending || savingKey || explorerPending) return;
    const hasDirty = tabs.some(tab => tab.projectId === project.projectId && tab.dirty);
    const message = 'Remove "' + project.displayName + '" from Lu IDE?\n\n' +
      'The original project folder and ALL files on disk will be kept. ' +
      'Only the IDE registration will be removed.\n\n' +
      'You can add the folder again using Open folder.' +
      (hasDirty ? '\n\nWARNING: Unsaved edits in its open tabs will be discarded.' : '');
    if (window.confirm(message)) {
      void operation('ide_unregister_project', { projectId: project.projectId });
    }
  }

  async function openDirectory(project, path) {
    if (explorerPending) return;
    setExplorerPending(true);
    setProblem('');
    try {
      const result = await invoke('ide_list_source_directory', {
        projectId: project.projectId, relativeDir: path
      });
      if (activeProject?.projectId !== project.projectId) {
        const previous = tabs.filter(tab => tab.projectId === project.projectId);
        setActiveTabKey(previous.length ? previous[previous.length - 1].key : null);
      }
      setActiveProject(project);
      setDirectory(path);
      setListing(result);
      setSection('explorer');
    } catch (error) { setProblem(String(error)); }
    finally { setExplorerPending(false); }
  }

  async function startLspSession() {
    const tab = activeTab;
    if (!tab || !lspAvailableForTab || lspSession || lspBusy) return;
    setLspBusy(true);
    setProblem('');
    try {
      const response = await invoke('ide_lsp_start_session',{request:{
        projectId:tab.projectId,relativePath:tab.relativePath,
        expectedRevision:tab.revision,bufferRevision:tab.bufferRevision ?? 0,
        languageId:languageForPath(tab.relativePath),text:tab.draft,
        kind:null,position:null,
      }});
      if (!matchesSource(tab,response)) {
        await invoke('ide_lsp_stop_session');
        throw new Error('Source changed during language-server admission');
      }
      setLspSession({projectId:tab.projectId,relativePath:tab.relativePath});
      setLspNativeDiagnostics(previous=>({...previous,[tab.key]:response}));
      setHint('Protected '+lspProviderForTab+' session started for '+tab.relativePath+'.');
    } catch (error) {
      setProblem('Language server was not started: '+String(error));
    } finally {setLspBusy(false);}
  }
  async function stopLspSession() {
    if (!lspSession || lspBusy) return;
    setLspBusy(true);
    try {
      await invoke('ide_lsp_stop_session');
      setHint('Language-server session stopped and native worker released.');
    } catch(error) {
      setProblem('Language-server cleanup must be verified: '+String(error));
    } finally {
      setLspSession(null);
      setLspNativeDiagnostics({});
      setLspCompletions(null);
      setLspHover(null);
      setLspBusy(false);
    }
  }
  useEffect(() => {
    if (lspSession && (!activeTab ||
        lspSession.projectId!==activeTab.projectId ||
        lspSession.relativePath!==activeTab.relativePath)) {
      void stopLspSession();
    }
  },[lspSession,activeTabKey,activeProject?.projectId]);
  useEffect(() => {
    if (!lspSession || !activeTab ||
        lspSession.projectId!==activeTab.projectId ||
        lspSession.relativePath!==activeTab.relativePath) return;
    const tab = activeTab;
    const timer=setTimeout(() => {
      const request={
        projectId:tab.projectId,relativePath:tab.relativePath,
        expectedRevision:tab.revision,bufferRevision:tab.bufferRevision ?? 0,
        languageId:languageForPath(tab.relativePath),text:tab.draft,
        kind:null,position:null,
      };
      void invoke('ide_lsp_change_document',{request}).then(changed=>{
        if (!matchesSource(tab,changed)) return;
        return invoke('ide_lsp_read_diagnostics',{request});
      }).then(result=>{
        if (!result || !matchesSource(tab,result)) return;
        setLspNativeDiagnostics(previous=>({...previous,[tab.key]:result}));
      }).catch(error=>{
        if (String(error).includes('LSP_STALE') ||
            String(error).includes('LSP_BUFFER_REVISION')) return;
        setProblem('Language service could not synchronize this draft: '+String(error));
      });
    },400);
    return ()=>clearTimeout(timer);
  },[lspSession,activeTab?.key,activeTab?.bufferRevision,activeTab?.revision]);

  async function requestLspFeature(kind) {
    const tab = activeTab;
    if (!tab || !lspRunningForTab || !lspAvailableForTab) return;
    if (!['completion', 'hover', 'definition', 'diagnostics'].includes(kind)) return;
    if (kind !== 'diagnostics' && !lspSupportsNavigation) return;
    // The native host must independently revalidate this current project,
    // saved disk hash, exact unsaved editor buffer revision, worker scope,
    // peer identity and user approval before forwarding an LSP message.
    const position = cursorPositions[tab.key] || {line: 0, character: 0};
    try {
      const command = kind === 'diagnostics' ? 'ide_lsp_read_diagnostics' : 'ide_lsp_feature';
      const response = await invoke(command, {request: {
        projectId: tab.projectId, relativePath: tab.relativePath,
        expectedRevision: tab.revision, bufferRevision: tab.bufferRevision ?? 0,
        languageId: languageForPath(tab.relativePath), text: tab.draft,
        kind, position: {line:position.line, character:position.character}
      }});
      if (!matchesSource(tab,response)) return;
      if (kind === 'diagnostics') {
        setLspNativeDiagnostics(previous => ({...previous, [tab.key]: response}));
      } else if (kind === 'completion') {
        setLspCompletions({tabKey:tab.key,response});
      } else if (kind === 'hover') {
        setLspHover({tabKey:tab.key,response});
      } else if (kind === 'definition') {
        const target=visibleDefinition(tab,response);
        if (target) setLspJumpRequest({...target,
          tabIdentity:tab.key + ':' + tab.generation, nonce:Date.now()});
      }
    } catch (error) {
      // No automatic external interpreter, shell, installer or unsupervised
      // fallback. The human can keep editing normally.
      setProblem('Language service unavailable: ' + String(error));
    }
  }

  function applyLspCompletion(item) {
    if (!activeTab || !lspRunningForTab ||
        !activeLspCompletions.some(choice => choice === item)) return;
    setLspCompletionApplyRequest({
      tabIdentity: activeTab.key + ':' + activeTab.generation,
      expectedDraft: activeTab.draft,
      insertText: item.insertText,
      replaceFrom: item.replaceFrom, replaceTo: item.replaceTo,
      nonce: Date.now(),
    });
    setLspCompletions(null);
  }

  function onEditorChange(key, state, draft) {
    editorStates.current.set(key, state);
    setReview(previous => previous?.tabKey === key ? null : previous);
    setTabs(previous => previous.map(tab =>
      tab.key === key ? { ...tab, draft, dirty: draft !== tab.savedText,
        bufferRevision: (tab.bufferRevision ?? 0) + 1 } : tab));
    setLspCompletions(null);
    setLspHover(null);
  }

  async function selectEntry(entry) {
    if (!activeProject || explorerPending) return;
    if (entry.kind === 'directory') {
      await openDirectory(activeProject, entry.relativePath);
    } else if (entry.kind === 'file') {
      const key = keyFor(activeProject.projectId, entry.relativePath);
      if (tabs.some(tab => tab.key === key)) {
        setActiveTabKey(key);
        return;
      }
      setExplorerPending(true);
      setProblem('');
      try {
        const opened = await invoke('ide_preview_source_file', {
          projectId: activeProject.projectId,
          relativePath: entry.relativePath,
        });
        const newTab = {
          key, projectId: opened.projectId,
          relativePath: opened.relativePath, revision: opened.revision,
          savedText: opened.text, draft: opened.text,
          dirty: false, generation: 0, bufferRevision: 0,
        };
        editorStates.current.set(key, createEditorState(
          opened.text, opened.relativePath,
          (state, draft) => onEditorChange(key, state, draft)
        ));
        setTabs(previous => [...previous, newTab]);
        setActiveTabKey(key);
      } catch (error) { setProblem(String(error)); }
      finally { setExplorerPending(false); }
    }
  }

  function closeTab(key) {
    const selected = tabs.find(tab => tab.key === key);
    if (!selected) return;
    if (savingKey === key) return;
    if (selected.dirty && !window.confirm(
      'Discard unsaved changes in ' + selected.relativePath + '?')) return;
    if (lspSession?.projectId === selected.projectId &&
        lspSession.relativePath === selected.relativePath) {
      void stopLspSession();
    }
    editorStates.current.delete(key);
    setReview(previous => previous?.tabKey === key ? null : previous);
    const other = tabs.filter(tab => tab.key !== key);
    setTabs(other);
    if (activeTabKey === key) {
      const candidates = other.filter(tab => tab.projectId === selected.projectId);
      setActiveTabKey(candidates.length ? candidates[candidates.length - 1].key : null);
    }
  }

  async function reloadTab(tab) {
    if (!tab || savingKey || reviewPending) return;
    setReview(null);
    if (tab.dirty && !window.confirm('Discard your unsaved edits and reload from disk?')) return;
    if (lspSession?.projectId === tab.projectId &&
        lspSession.relativePath === tab.relativePath) {
      await stopLspSession();
    }
    setExplorerPending(true);
    setProblem('');
    try {
      const opened = await invoke('ide_preview_source_file', {
        projectId: tab.projectId, relativePath: tab.relativePath,
      });
      editorStates.current.set(tab.key, createEditorState(
        opened.text, opened.relativePath,
        (state, draft) => onEditorChange(tab.key, state, draft)
      ));
      setTabs(previous => previous.map(item => item.key === tab.key ? {
        ...item, revision: opened.revision, savedText: opened.text,
        draft: opened.text, dirty: false, generation: item.generation + 1,
        bufferRevision: (item.bufferRevision ?? 0) + 1,
      } : item));
      setHint('Reloaded ' + tab.relativePath + ' from disk.');
    } catch (error) { setProblem(String(error)); }
    finally { setExplorerPending(false); }
  }

  async function saveTab(tab) {
    if (!tab || !tab.dirty || savingKey || explorerPending) return;
    const content = tab.draft;
    const base = tab.revision;
    setSavingKey(tab.key);
    setProblem('');
    setHint('');
    try {
      const saved = await invoke('ide_save_source_file', {
        projectId: tab.projectId,
        relativePath: tab.relativePath,
        expectedRevision: base,
        text: content,
      });
      // If typing continued during the save, the newer draft is kept dirty.
      setTabs(previous => previous.map(item => item.key === tab.key ? {
        ...item, revision: saved.revision, savedText: content,
        dirty: item.draft !== content,
      } : item));
      if (lspSession?.projectId === tab.projectId &&
          lspSession.relativePath === tab.relativePath) {
        await stopLspSession();
      }
      setHint('Saved ' + tab.relativePath + ' to disk.');
      return true;
    } catch (error) {
      if (String(error).includes('FILE_CHANGED_ON_DISK')) {
        setProblem('File changed on disk. This file was updated outside Lu IDE. ' +
          'Your unsaved edits are preserved in the tab. Compare the changes before reloading.');
      } else {
        setProblem(String(error));
      }
    } finally { setSavingKey(null); }
    return false;
  }

  async function reviewTab(tab) {
    if (!tab || !tab.dirty || reviewPending || explorerPending || savingKey) return;
    setReviewPending(true);
    setProblem('');
    try {
      const proposal = await invoke('ide_review_source_change', {
        projectId: tab.projectId,
        relativePath: tab.relativePath,
        expectedRevision: tab.revision,
        proposedText: tab.draft,
      });
      setReview({
        ...proposal,
        tabKey: tab.key,
        draft: tab.draft,
      });
    } catch (error) {
      setProblem(String(error).includes('FILE_CHANGED_ON_DISK')
        ? 'File changed on disk. The source was updated outside Lu IDE. Your unsaved edits are safe.'
        : String(error));
    } finally { setReviewPending(false); }
  }

  async function saveReviewedChange() {
    if (!review || review.truncated || savingKey || reviewPending) return;
    const tab = tabs.find(item => item.key === review.tabKey);
    if (!tab || !tab.dirty || tab.draft !== review.draft ||
        tab.revision !== review.expectedRevision ||
        tab.projectId !== review.projectId) {
      setReview(null);
      setProblem('The reviewed edit changed. Review the latest draft before saving.');
      return;
    }
    const saved = await saveTab(tab);
    if (saved) setReview(null);
  }

  useEffect(() => {
    const shortcut = event => {
      if (!(event.ctrlKey || event.metaKey)) return;
      const key = event.key.toLowerCase();
      if (key === 'o') {
        event.preventDefault();
        if (!pending && !explorerPending && !savingKey) void operation('ide_choose_project');
      } else if (key === 'h' && section === 'explorer' && activeTab) {
        event.preventDefault();
        setFindOpen(true);
        setReplaceVisible(true);
        setFindIndex(-1);
      } else if (key === 'f' && section === 'explorer' && activeTab) {
        event.preventDefault();
        setFindOpen(true);
        setFindIndex(-1);
      } else if (key === 's') {
        event.preventDefault();
        if (activeTab) void saveTab(activeTab);
      }
    };
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, [pending, explorerPending, savingKey, activeTab, tabs, section]);

  useEffect(() => {
    const warn = event => {
      if (!anyDirty) return;
      event.preventDefault();
      event.returnValue = '';
    };
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [anyDirty]);

  return <div className="ideShell">
    <aside className="ideRail">
      <div className="ideBrand"><div className="brandSymbol">L</div>
        <span><strong>Lu IDE</strong><small>Standalone alpha</small></span>
      </div>
      <nav aria-label="IDE sections" className="ideNav">
        {sections.map(item => <button key={item.id} type="button"
          className={'navItem' + (section === item.id ? ' selected' : '')}
          disabled={item.pending || (item.id === 'explorer' && !activeProject)}
          aria-current={section === item.id ? 'page' : undefined}
          title={item.pending ? item.title + ' — not enabled yet' : item.title}
          onClick={() => { setSection(item.id); if (item.id === 'proposals') void refreshProposals(); if (item.id === 'capabilities') void refreshCapabilities(); if (item.id === 'continuity') void refreshContext(); if (item.id === 'source') void refreshGitStatus(); if (item.id === 'packages') void readWorkflowRuns(); if (item.id === 'tasks') void refreshIntents(); if (item.id === 'languages') void invoke('ide_list_language_servers').then(setLanguageServers).catch(() => setLanguageServers([])); }}>
          <span aria-hidden="true">{item.glyph}</span>{item.title}
          {item.pending && <small>Soon</small>}
        </button>)}
      </nav>
      <div className="ideRailFooter">
        <div className="bridgeLine"><span className="bridgeDot"/>
          <span>Lu connection</span><strong>Not connected</strong>
        </div>
        <p>Lu integration will use an optional, versioned bridge. This app works independently.</p>
      </div>
    </aside>
    <main className="ideMain">
      <header className="ideTopBar">
        <div><div className="eyebrow">LOCAL DEVELOPMENT ENVIRONMENT</div>
          <h1>{isJobs ? 'Execution requests' : isPackages ? 'Packages & workflows' : isSource ? 'Source control' : isContinuity ? 'Project continuity' : isCapabilities ? 'IDE capabilities' : isLanguages ? 'Language services' : isProposals ? 'Coding proposals' : isExplorer ? (activeProject?.displayName || 'Source editor') : 'Workspace projects'}</h1>
          <p>{isJobs ? 'Historical review intents · Protected native Run/Test · Explicit cancellation' :
            isPackages ? 'Multi-project Git and artifact review · No release permission' :
            isSource ? 'Read-only local Git status · No staging, commits, fetch or push' :
            isContinuity ? 'Development history · Revisioned sources · Explicit unknowns' :
            isCapabilities ? 'Host-published operations · Descriptions only · Never permission grants' :
            isLanguages ? 'Open-source LSP engines · Protected worker status · No auto-install' :
            isProposals ? 'Data-only review inbox · No automatic changes · Lu disconnected' :
            isExplorer ? 'Tabbed code editor · Explicit, revision-checked saves · No code execution' :
            'Choose projects without executing their code.'}</p>
        </div>
        <div className="ideControls">
          {isExplorer && <button className="ghostBtn" onClick={() => setSection('projects')}>
            All projects</button>}
          <button className="ghostBtn" disabled={pending || explorerPending || !!savingKey}
            onClick={() => isJobs ? (refreshIntents(), refreshProtectedBuildStatus()) : isPackages ? (readWorkflowRuns(), workflowInspection && previewMultiProjectWorkflow()) : isSource ? refreshGitStatus() : isContinuity ? refreshContext() : isCapabilities ? refreshCapabilities() : isLanguages ? invoke('ide_list_language_servers').then(setLanguageServers).catch(() => setLanguageServers([])) : isProposals ? refreshProposals() : isExplorer && activeProject ? openDirectory(activeProject, directory) : refresh()}>
            ↻ Refresh</button>
          <button className="primaryBtn" disabled={pending || explorerPending || !!savingKey}
            onClick={() => operation('ide_choose_project')}>+ Open folder</button>
        </div>
      </header>
      {problem && <div className="errorMsg appError" role="alert">{problem}
        <button type="button" onClick={() => setProblem('')}>Dismiss</button></div>}
      {hint && <div className={'successMsg appMessage noticeBanner' +
        (hint.startsWith('Saved ') ? ' saveConfirmation' :
          hint.startsWith('Proposal staged') ? ' stageConfirmation' : '')}
        role="status">
        <span className="noticeContent">{hint}</span>
        <button type="button" className="noticeDismiss" aria-label="Dismiss notification"
          onClick={() => setHint('')}>×</button>
      </div>}
      {isJobs
        ? <section className="workspaceContent continuityWorkspace" aria-label="Execution intent review">
            <div className="sectionHeading">
              <h2>Build, test and job requests</h2>
              <span>WO6 · Native OS admission required</span>
            </div>
            <div className="continuityDocument jobIntentComposer" aria-label="Protected native build and test">
              <h3>Protected Run / Test</h3>
              <p>Only approved offline Cargo recipes can be executed from a frozen source snapshot
                inside a restricted OS worker. A fresh native confirmation is required for each run.
                The default desktop and machines without pinned toolchains cannot launch one.</p>
              <p>Native worker: <strong>{buildStatus?.ready ? 'Release resources verified; OS admission checked on every run' :
                (buildStatus?.code || 'Checking trusted native admission…')}</strong></p>
              <p>{buildStatus?.description || 'No unprotected fallback is provided.'}</p>
              <button className="primaryBtn" type="button"
                disabled={!activeProject || !buildStatus?.ready || !!buildStatus.current || buildBusy ||
                  !['rust_check','rust_tests','lu_ide_host_tests'].includes(intentRecipe)}
                onClick={() => startProtectedBuild()}>
                {buildBusy ? 'Protected job in progress…' : 'Request protected Run / Test'}
              </button>
              {buildStatus?.current?.projectId === activeProject?.projectId && <>
                <p role="status">Active protected job: {buildStatus.current.recipeId} ·
                  {buildStatus.current.jobId || 'Preparing frozen snapshot'}
                  {buildStatus.current.cancellationRequested ? ' · cancellation requested' : ''}</p>
                <button type="button" className="ghostBtn"
                  disabled={buildCancelBusy || buildStatus.current.cancellationRequested}
                  onClick={() => cancelProtectedBuild()}>
                  {buildCancelBusy ? 'Requesting cancellation…' : 'Cancel and verify process-tree termination'}
                </button>
              </>}
              {buildResult?.projectId === activeProject?.projectId && <div role="status">
                <p>Last worker result: <strong>{buildResult.phase}</strong> · Job {buildResult.jobId}</p>
                <p>Native exit: {String(buildResult.nativeExitCode ?? 'unknown')} ·
                  Fixed OS error category: {buildResult.nativeErrorCategory || 'not reported'} · Timed out:
                  {buildResult.timedOut ? 'yes' : 'no'} · Reconciliation:
                  {buildResult.requiresReconciliation ? 'required' : 'not indicated'}</p>
                <p>Source: <code>{buildResult.inputSnapshotSha256}</code></p>
                <p>Toolchain: <code>{buildResult.toolchainSha256}</code></p>
                <p>Worker output seal: {buildResult.outputSha256 || 'not independently published'}</p>
                <p>Read-only artifact witness: {buildResult.artifactEvidenceState || 'unavailable'} ·
                  {buildResult.artifactCount ?? 0} files · {buildResult.artifactTruncated ? 'display truncated' : 'bounded listing'}</p>
                <p>Artifact manifest SHA-256: <code>{buildResult.artifactManifestSha256 || 'unavailable'}</code></p>
                {Array.isArray(buildResult.artifactFiles) && buildResult.artifactFiles.length > 0 &&
                  <ul>{buildResult.artifactFiles.map(item =>
                    <li key={item.relativePath}><code>{item.relativePath}</code> ({item.bytes} bytes)
                      · SHA-256 <code>{item.sha256}</code></li>)}</ul>}
                <p>Installed-app acceptance: <strong>Not established by this result</strong>.</p>
              </div>}
              {buildStatus?.quarantined?.filter(item => item.requiresReconciliation).map(job =>
                <div className="continuityEvent" key={job.jobId}>
                  <p>Interrupted job <code>{job.jobId}</code> · {job.phase}.
                    Execution stays quarantined until native kernel cleanup is verified.</p>
                  <button type="button" className="ghostBtn"
                    disabled={buildBusy || !!buildStatus.current}
                    onClick={() => reconcileProtectedBuild(job)}>Verify OS-only cleanup (no rerun)</button>
                </div>)}
              {Array.isArray(buildStatus?.journal) && buildStatus.journal.length > 0 &&
                <section className="continuityDocument" aria-label="Durable native job journal">
                  <h3>Durable native job journal (survives restart)</h3>
                  <p>Exact project-scoped supervisor records. The journal is evidence of
                    recorded state, not proof that all OS descendants are gone.</p>
                  {buildStatus.journal.slice(-16).map(item =>
                    <article className="continuityEvent" key={item.jobId}>
                      <strong>Durable protected job <code>{item.jobId}</code></strong>
                      <p>Journal phase: {item.phase} ·
                        Kernel cleanup reconciliation: {item.requiresReconciliation ?
                          'required' : 'not indicated'}</p>
                      <p>Frozen plan digest: <code>{item.planSha256}</code></p>
                    </article>)}
                </section>}
              {buildStatus?.recent?.length > 0 && <section aria-label="Recent native worker receipts">
                <p>Recent native worker receipts: {buildStatus.recent.length}.
                  These results do not substitute for installed-app tests.</p>
                <div className="continuityEvents">
                  {buildStatus.recent.slice(-8).map(item =>
                    <article className="continuityEvent" key={item.jobId}>
                      <strong>Recorded protected job <code>{item.jobId}</code></strong>
                      <p>Recipe: {item.recipeId} · Phase: {item.phase} ·
                        Reconciliation: {item.requiresReconciliation ? 'required' : 'not indicated'}</p>
                      <p>Source HEAD: <code>{item.sourceHead}</code></p>
                      <p>Frozen input: <code>{item.inputSnapshotSha256}</code></p>
                      <p>Native result: {String(item.nativeExitCode ?? 'not reported')} ·
                        OS error: {item.nativeErrorCategory || 'not reported'}</p>
                      <p>Kernel cleanup must be independently verified; an IDE receipt alone
                        is not installed-app release acceptance.</p>
                    </article>)}
                </div>
              </section>}
            </div>
            <div className="continuityDocument jobIntentComposer" aria-label="Protected native debugger self-test">
              <h3>Protected debugger (LLDB-DAP)</h3>
              <p>Only a reviewed, installer-pinned fixed debuggee can be launched for native
                stopped-thread/stack acceptance. Arbitrary project executables, process attach,
                terminal commands, expressions and debugger script execution are not available.</p>
              <p>Debugger: <strong>{debugStatus?.ready
                ? 'Signed fixture bundle verified; OS containment checked per run'
                : (debugStatus?.code || 'Checking approved debugger assets…')}</strong></p>
              <p>{debugStatus?.description || 'An unpinned adapter cannot execute.'}</p>
              <button type="button" className="ghostBtn"
                disabled={!activeProject || !debugStatus?.ready || !!debugStatus.current || debugBusy}
                onClick={() => runProtectedDebuggerFixture()}>
                {debugBusy ? 'Inspecting fixed native debuggee…' : 'Run protected debugger self-test'}
              </button>
              {debugStatus?.current?.projectId === activeProject?.projectId && <>
                <p role="status">Active debug session: {debugStatus.current.jobId || 'preflight'}
                  {debugStatus.current.cancellationRequested ? ' · cancelling' : ''}</p>
                <button type="button" className="ghostBtn"
                  disabled={debugStatus.current.cancellationRequested}
                  onClick={() => cancelProtectedDebugger()}>
                  Request debugger cancellation
                </button>
              </>}
              {Array.isArray(debugStatus?.quarantined) && debugStatus.quarantined
                .filter(item => item.projectId === activeProject?.projectId &&
                  item.requiresReconciliation)
                .map(item => <div className="continuityEvent" key={item.jobId}>
                  <p>Interrupted debugger job <code>{item.jobId}</code> · {item.phase}.
                    No debuggee can restart while OS cleanup is unverified.</p>
                  <button type="button" className="ghostBtn"
                    disabled={debugBusy || !!debugStatus?.current}
                    onClick={() => reconcileProtectedDebugger(item)}>
                    Verify protected debugger cleanup (no execution)
                  </button>
                </div>)}
              {Array.isArray(debugStatus?.journal) && debugStatus.journal.length > 0 &&
                <section className="continuityDocument" aria-label="Durable protected debugger journal">
                  <h3>Durable debugger jobs (survive restart)</h3>
                  {debugStatus.journal.slice(-16).map(item =>
                    <article className="continuityEvent" key={item.jobId}>
                      <strong>Durable debugger job <code>{item.jobId}</code></strong>
                      <p>Journal phase: {item.phase} ·
                        OS reconciliation: {item.requiresReconciliation ?
                          'required' : 'not indicated'}</p>
                      <p>Frozen plan digest: <code>{item.planSha256}</code></p>
                    </article>)}
                  <p>Journal entries are not simulated debugger results or release acceptance.</p>
                </section>}
              {debugResult?.projectId === activeProject?.projectId && <div role="status">
                <p>Native LLDB self-test: <strong>{debugResult.terminalPhase}</strong></p>
                <p>Stopped thread: {debugResult.stoppedThread ?? 'none'} · Thread inspection:
                  {debugResult.threadsResponseObserved ? 'observed' : 'not proven'} · Stack:
                  {debugResult.stackResponseObserved ? 'observed' : 'not proven'}</p>
                <p>Kernel tree cleanup: {debugResult.verifiedOsTreeReleaseByWorker
                  ? 'reported by protected supervisor' : 'not verified'}.</p>
                <p>Installed/debugger project release acceptance: <strong>Not established.</strong></p>
              </div>}
            </div>
            <p className="capabilityNotice"><strong>Historical D1 intents below are non-executing.</strong>
              Reviewing or cancelling an intent never issues a worker grant. A protected job
              requires its own independently verified native authorization.</p>
            <div className="continuityDocument jobIntentComposer">
              <h3>Prepare a fixed recipe</h3>
              <p>Requires a registered project with a clean local Git HEAD and
                a non-symlink Cargo manifest. The host verifies the manifest digest.
                Unsaved, ignored, or external files are not certified by a Git HEAD.</p>
              <label htmlFor="intentRecipe">Recipe</label>
              <select id="intentRecipe" value={intentRecipe} disabled={intentBusy}
                onChange={event => setIntentRecipe(event.target.value)}>
                <option value="lu_ide_host_tests">Lu IDE host tests</option>
                <option value="rust_check">Rust project type-check</option>
                <option value="rust_tests">Rust project tests</option>
              </select>
              <button type="button" className="ghostBtn"
                disabled={!activeProject || intentBusy}
                onClick={() => prepareIntent()}>
                {intentBusy ? 'Checking…' : 'Prepare for human review'}
              </button>
              <p>Preparing records an intent in the private app-profile journal,
                valid for 120 seconds in this IDE session. It never runs the recipe.</p>
            </div>
            {!activeProject && <p>Open a project to prepare a job request.</p>}
            {intentBusy && <p role="status">Checking the native job-intent journal…</p>}
            {intentList && !intentList.workerExecutionEnabled && <>
              <p>Recorded requests: {intentList.entries.length} / 16 per project.
                Worker containment: <strong>Not implemented</strong>.
                Execution authority: <strong>Not issued</strong>.</p>
              {intentList.entries.length === 0 &&
                <p>No job intentions have been prepared for this project.</p>}
              <div className="continuityEvents">
                {intentList.entries.map(entry => <article className="continuityEvent" key={entry.intentId}>
                  <div className="capabilityHeading">
                    <strong>{entry.displayName}</strong>
                    <span>{entry.state.replaceAll('_', ' ')}</span>
                  </div>
                  <p>Proposed, not executed: <code>{entry.proposedAction}</code></p>
                  <p>Source HEAD: <code>{entry.sourceHead}</code></p>
                  <p>Manifest: <code>{entry.manifestRelativePath}</code></p>
                  <p>Manifest SHA-256: <code>{entry.manifestSha256}</code></p>
                  <p>Expires: {new Date(entry.expiresUnixSeconds * 1000).toLocaleString()}</p>
                  <p>Execution: <strong>Never started</strong>.
                    Grant token: <strong>Not issued</strong>.</p>
                  {entry.canRecordReview && <>
                    <label className="jobIntentConsent">
                      <input type="checkbox" checked={reviewIntentId === entry.intentId}
                        disabled={intentBusy}
                        onChange={event => setReviewIntentId(event.target.checked
                          ? entry.intentId : null)} />
                      I reviewed this source revision and understand no code will execute.
                    </label>
                    <button type="button" className="ghostBtn"
                      disabled={reviewIntentId !== entry.intentId || intentBusy}
                      onClick={() => decideIntent(entry, 'record_review')}>
                      Record human review (no execution)
                    </button>
                  </>}
                  {(entry.state === 'prepared_not_authorized' ||
                    entry.state === 'human_review_recorded_no_execution' ||
                    entry.state === 'requires_new_session_review') &&
                    <button type="button" className="ghostBtn"
                      disabled={intentBusy} onClick={() => decideIntent(entry, 'cancel')}>
                      Cancel intent / withdraw review
                    </button>}
                </article>)}
              </div>
            </>}
          </section>
        : isProposals
        ? <section className="proposalWorkspace" aria-label="Coding proposal inbox">
          <div className="proposalIntro">
            <div>
              <h2>Proposals awaiting review</h2>
              <p>Import a bounded proposal or prepare one from the open editor.
                No model is connected; author claims in proposals are unverified.
                Nothing in this inbox can write or execute code.</p>
            </div>
            <span>{proposals.length} / 16 proposals</span>
          </div>
          <div className="proposalGrid">
            <div className="proposalCompose">
              <h3>Prepare a proposal</h3>
              <p>Open a project source file, then create a local proposal draft.
                You can edit its JSON text to suggest a replacement. Importing
                is read-only and validates the file's saved revision.</p>
              <button type="button" className="ghostBtn"
                disabled={!activeTab || inboxBusy}
                onClick={prepareLocalProposal}>Prepare from open editor</button>
              <label htmlFor="proposal-json">Proposal JSON</label>
              <textarea id="proposal-json" className="proposalJsonInput"
                value={proposalInput} spellCheck={false} maxLength={650000}
                onChange={event => setProposalInput(event.target.value)}
                placeholder="Paste a schemaVersion: 1 proposal or prepare one from the editor." />
              <button type="button" className="primaryBtn"
                disabled={!proposalInput.trim() || inboxBusy}
                onClick={() => importProposal()}>
                {inboxBusy ? 'Working…' : 'Add to review inbox'}
              </button>
              <p className="proposalNotice">No source files are saved by importing a proposal.
                The inbox exists only while this IDE instance is running.</p>
            </div>
            <div className="proposalList">
              <h3>Review inbox</h3>
              {proposals.length === 0
                ? <div className="proposalEmpty">No coding proposals have been queued.
                    Open a source tab to prepare a local example.</div>
                : proposals.map(item => <article key={item.inboxId} className="proposalItem">
                  <div className="proposalItemHeader">
                    <strong>{item.taskId}</strong>
                    <span className={'proposalStatus ' + item.status}>{item.status.replaceAll('_', ' ')}</span>
                  </div>
                  <p>{item.rationale || 'No rationale provided.'}</p>
                  <small>{item.editCount} file(s) · Claimed source: {item.claimedProducer} (unverified)</small>
                  <div className="proposalItemActions">
                    <button type="button" className="ghostBtn" disabled={inboxBusy}
                      onClick={() => openProposal(item.inboxId)}>Review</button>
                    {item.status === 'approved_for_staging' &&
                      <button type="button" className="primaryBtn"
                        disabled={inboxBusy || !projects.some(project =>
                          project.projectId === item.projectId && project.active)}
                        onClick={() => stageProposal(item)}>Stage in editor</button>}
                    <button type="button" className="ghostBtn" disabled={inboxBusy}
                      onClick={() => discardProposal(item)}>Dismiss</button>
                  </div>
                </article>)}
            </div>
          </div>
        </section>
        : isPackages
        ? <section className="workspaceContent continuityWorkspace" aria-label="Advanced packaging and multi-project workflows">
            <div className="sectionHeading">
              <h2>Multi-project release review</h2>
              <span>WO9 · Native Git and artifact inspection</span>
            </div>
            <p>Select up to eight projects that you registered using the operating-system
              folder picker. Lu IDE observes each exact Git HEAD, working tree and bounded
              artifact hashes. A review is never a signed release or permission to execute.</p>
            <div className="continuityDocument">
              <h3>Projects in workflow</h3>
              <fieldset className="taskGateGrid" disabled={workflowBusy}>
                <legend>Include registered, open projects</legend>
                {projects.filter(project => project.available).map(project =>
                  <label className="taskGateChoice" key={project.projectId}>
                    <input type="checkbox"
                      checked={workflowProjectIds.includes(project.projectId)}
                      disabled={!project.active || (!workflowProjectIds.includes(project.projectId) &&
                        workflowProjectIds.length >= 8)}
                      onChange={event => chooseWorkflowProject(project.projectId, event.target.checked)} />
                    {project.displayName} ({project.active ? 'open' : 'closed'})
                  </label>)}
              </fieldset>
              {projects.length === 0 && <p>Register projects from the Projects screen first.</p>}
              <button className="primaryBtn" type="button"
                disabled={workflowBusy || workflowProjectIds.length === 0}
                onClick={() => previewMultiProjectWorkflow()}>
                {workflowBusy ? 'Inspecting native evidence…' : 'Inspect selected workflow'}
              </button>
            </div>
            {workflowInspection && <div className="continuityDocument"
              aria-label="Multi-project native workflow evidence">
              <h3>Native workflow review</h3>
              <p>Plan digest: <code>{workflowInspection.fingerprint}</code></p>
              <p>Local review: <strong>{workflowInspection.readyForReview ?
                'Eligible for human source review' : 'Blocked by native evidence'}</strong> ·
                Remote: not queried · Execution: never started ·
                Packaging: not performed · Release: not approved</p>
              {workflowInspection.projects.map(item =>
                <article className="continuityEvent" key={item.projectId}>
                  <strong>{item.displayName}</strong>
                  <p>Local branch: {item.branch || 'Unavailable'} · Git HEAD:
                    <code> {item.head || 'Unverified'}</code></p>
                  <p>Worktree: {item.worktree} · Verified artifacts:
                    {item.artifacts.length}</p>
                  {item.blockers.length > 0 &&
                    <p role="status">Blockers: {item.blockers.join(', ')}</p>}
                  {item.artifacts.map(artifact =>
                    <p key={artifact.relativePath}><code>{artifact.relativePath}</code>
                      {' · '}{artifact.byteLength.toLocaleString()} bytes
                      {' · '}{artifact.sha256}</p>)}
                </article>)}
              <button className="ghostBtn" type="button"
                disabled={workflowBusy || !workflowInspection.readyForReview}
                onClick={() => revalidateMultiProjectWorkflow()}>
                Verify frozen Git and artifact evidence again
              </button>
              {workflowRevalidated && <p role="status">
                Exact evidence matched. This does not stage files, create an archive,
                grant Git write rights, or satisfy native OS and signing gates.</p>}
              <button className="primaryBtn" type="button"
                disabled={workflowBusy || !workflowRevalidated}
                onClick={() => exportMultiProjectManifest()}>
                Export reviewed manifest to OS-selected folder
              </button>
              <button className="ghostBtn" type="button"
                disabled={workflowBusy || !workflowRevalidated}
                onClick={() => exportMultiProjectBundle()}>
                Export verified artifacts as ZIP (no code execution)
              </button>
              {workflowBundleExport && <div role="status">
                <p>Verified ZIP: <strong>{workflowBundleExport.fileName}</strong> ·
                  {' '}{workflowBundleExport.artifactCount} source artifacts ·
                  {' '}{workflowBundleExport.byteLength.toLocaleString()} bytes</p>
                <p>Archive SHA-256: <code>{workflowBundleExport.sha256}</code></p>
                <p>Git mutations: <strong>None</strong> ·
                  Project code executed: <strong>No</strong> ·
                  Installer/release accepted: <strong>No</strong></p>
              </div>}
              {workflowExport && <div role="status">
                <p>Exported: <strong>{workflowExport.fileName}</strong> ·
                  {' '}{workflowExport.byteLength.toLocaleString()} bytes</p>
                <p>SHA-256: <code>{workflowExport.sha256}</code></p>
                <p>Files included in package: <strong>No</strong> ·
                  Signing: <strong>Not performed</strong> ·
                  Release approval: <strong>Not granted</strong></p>
              </div>}
            </div>}
            <div className="continuityDocument" aria-label="Durable multi-project dependency workflows">
              <h3>Durable dependent build/test workflow</h3>
              <p>Define a sequence of approved offline Cargo steps on the selected projects.
                The native supervisor requests separate human permission and protected OS
                admission for every step. No job starts when you create the graph.
                Crashed and denied jobs block dependents; there is never automatic replay.</p>
              <button className="ghostBtn" type="button"
                disabled={!workflowInspection?.readyForReview || workflowSteps.length >= 12}
                onClick={() => addWorkflowStep()}>Add dependent step</button>
              {workflowSteps.map((step, index) =>
                <div className="continuityEvent" key={index}>
                  <strong>Step {index + 1}</strong>
                  <label>Project
                    <select value={step.projectId} disabled={workflowRunBusy}
                      onChange={event => setWorkflowSteps(previous =>
                        previous.map((item, i) => i === index ?
                          {...item, projectId: event.target.value} : item))}>
                      {workflowInspection?.projects.map(item =>
                        <option key={item.projectId} value={item.projectId}>
                          {item.displayName}</option>)}
                    </select>
                  </label>
                  <label>Approved offline recipe
                    <select value={step.recipeId} disabled={workflowRunBusy}
                      onChange={event => setWorkflowSteps(previous =>
                        previous.map((item, i) => i === index ?
                          {...item, recipeId: event.target.value} : item))}>
                      <option value="rust_check">Cargo check</option>
                      <option value="rust_tests">Cargo tests</option>
                      <option value="lu_ide_host_tests">Lu IDE host tests</option>
                    </select>
                  </label>
                  <fieldset><legend>Requires completed steps</legend>
                    {workflowSteps.slice(0,index).map((previous, prior) =>
                      <label key={prior}>
                        <input type="checkbox" checked={step.dependsOn.includes(prior)}
                          disabled={workflowRunBusy}
                          onChange={event => setWorkflowSteps(items =>
                            items.map((item,i) => i === index ? {...item,
                              dependsOn: event.target.checked ?
                                [...item.dependsOn,prior].sort((a,b)=>a-b) :
                                item.dependsOn.filter(id => id !== prior)} : item))} />
                        Step {prior + 1}
                      </label>)}
                  </fieldset>
                </div>)}
              <button className="primaryBtn" type="button"
                disabled={!workflowRevalidated || workflowRunBusy || !workflowSteps.length}
                onClick={() => createDurableWorkflow()}>
                Record exact dependency workflow (no execution)
              </button>
              <button className="ghostBtn" type="button"
                disabled={workflowRunBusy} onClick={() => readWorkflowRuns()}>
                Refresh durable workflows
              </button>
              {workflowRuns.map(run => <article key={run.workflowId}
                className="continuityEvent">
                <strong>Workflow <code>{run.workflowId}</code></strong>
                <p>Frozen evidence: <code>{run.sourceFingerprint}</code></p>
                <p>Auto-run: disabled · Release approved: no</p>
                {run.tasks.map((task,index) => {
                  const ready = task.state === 'pending' &&
                    task.spec.dependsOn.every(dep => run.tasks[dep]?.state === 'passed') &&
                    !run.tasks.some(other => ['running','interrupted'].includes(other.state));
                  return <div className="continuityEvent" key={index}>
                    <p>Step {index + 1} · {task.spec.recipeId} ·
                      <strong> {task.state}</strong></p>
                    <p>Project <code>{task.spec.projectId}</code> ·
                      Prerequisites: {task.spec.dependsOn.map(dep => dep+1).join(', ') || 'none'}</p>
                    {task.nativeJobId && <p>Native job <code>{task.nativeJobId}</code> ·
                      Output <code>{task.nativeOutputSha256 || 'unverified'}</code></p>}
                    {task.errorCode && <p role="status">{task.errorCode}</p>}
                    {ready && <button type="button" className="primaryBtn"
                      disabled={workflowRunBusy}
                      onClick={() => runDurableStep(run,index)}>
                      Request separately approved protected step
                    </button>}
                    {task.state === 'pending' && <button type="button" className="ghostBtn"
                      disabled={workflowRunBusy}
                      onClick={() => cancelDurableStep(run,index)}>Cancel pending step</button>}
                    {task.state === 'interrupted' &&
                      <p role="status">Native OS reconciliation required.
                        No automatic replay. Use Job requests to inspect exact kernel cleanup.</p>}
                  </div>;
                })}
              </article>)}
            </div>
          </section>
        : isSource
        ? <section className="workspaceContent continuityWorkspace" aria-label="Read-only source control">
            <div className="sectionHeading">
              <h2>Local Git worktree</h2>
              <span>{gitStatus ? gitStatus.worktree.replaceAll('_', ' ') : 'Not inspected'}</span>
            </div>
            <p className="capabilityNotice">
              Local status is read directly from the registered project. WO9 permits one-file,
              human-reviewed local staging and commit through native libgit2 only. No hooks,
              arbitrary commands, remote fetch, push or branch checkout are exposed.
              Tracking information is locally stored and may be stale.
            </p>
            {!activeProject && <p>Open a project to inspect its source control.</p>}
            {activeProject && <div className="continuityDocument" aria-label="Reviewed local Git staging and commit">
              <h3>Review, stage, commit one tracked file</h3>
              <p>Only a tracked, regular file under your OS-picker-selected project.
                Native host checks the exact source SHA, Git HEAD and index before
                staging its reviewed bytes. Separate operating-system confirmations
                are required for Stage and Commit. No project scripts or Git hooks run.</p>
              <label htmlFor="wo9GitPath">Project-relative tracked file</label>
              <input id="wo9GitPath" type="text" value={gitReviewPath}
                placeholder="src/main.rs" maxLength={240}
                disabled={gitWriteBusy || !!gitStageReceipt}
                onChange={event => { setGitReviewPath(event.target.value);
                  setGitReview(null); setGitCommitReceipt(null); }} />
              <button type="button" className="ghostBtn"
                disabled={gitWriteBusy || !!gitStageReceipt || !gitReviewPath.trim()}
                onClick={() => reviewGitFileForStage()}>Inspect exact Git file</button>
              {gitReview && <article className="continuityEvent">
                <strong>Reviewed local file: {gitReview.relativePath}</strong>
                <p>HEAD: <code>{gitReview.localHead}</code></p>
                <p>Exact file SHA: <code>{gitReview.worktreeSha256}</code></p>
                <p>Index revision: <code>{gitReview.indexFingerprint}</code></p>
                <p>Size: {gitReview.fileSize.toLocaleString()} bytes. Review this file's
                  changes in the read-only Diff view before staging.</p>
                <button type="button" className="primaryBtn"
                  disabled={gitWriteBusy} onClick={() => stageReviewedGitFile()}>
                  Confirm and stage these reviewed bytes
                </button>
              </article>}
              {gitStageReceipt && <article className="continuityEvent">
                <strong>Staged exact file: {gitStageReceipt.relativePath}</strong>
                <p>Git blob: <code>{gitStageReceipt.stagedBlobOid}</code></p>
                <p>SHA-256: <code>{gitStageReceipt.sourceSha256}</code></p>
                <label htmlFor="wo9CommitMessage">Local commit message</label>
                <input id="wo9CommitMessage" type="text" value={gitCommitMessage}
                  maxLength={1200} disabled={gitWriteBusy}
                  onChange={event => setGitCommitMessage(event.target.value)} />
                <label htmlFor="wo9CommitName">Author name</label>
                <input id="wo9CommitName" type="text" value={gitAuthorName}
                  maxLength={96} disabled={gitWriteBusy}
                  onChange={event => setGitAuthorName(event.target.value)} />
                <label htmlFor="wo9CommitEmail">Author email</label>
                <input id="wo9CommitEmail" type="email" value={gitAuthorEmail}
                  maxLength={96} disabled={gitWriteBusy}
                  onChange={event => setGitAuthorEmail(event.target.value)} />
                <button type="button" className="primaryBtn"
                  disabled={gitWriteBusy || !gitCommitMessage.trim() ||
                    !gitAuthorName.trim() || !gitAuthorEmail.trim()}
                  onClick={() => commitReviewedGitFile()}>
                  Confirm separate local commit (no push)
                </button>
              </article>}
              {gitCommitReceipt && <p role="status">
                Local commit: <code>{gitCommitReceipt.commitSha}</code>.
                Remote unchanged; release unapproved.</p>}
            </div>}

            {gitBusy && <p role="status">Inspecting local Git state…</p>}
            {!gitBusy && gitStatus && <div className="continuityBody">
              <p><strong>{gitStatus.state.replaceAll('_', ' ')}</strong>
                {' · '} Branch: <code>{gitStatus.branch || 'Unknown or detached'}</code>
              </p>
              {gitStatus.localHead && <p>Local commit: <code>{gitStatus.localHead}</code></p>}
              <p>Stored upstream: <strong>{gitStatus.upstreamComparison.replaceAll('_', ' ')}</strong>
                {' · '} Live GitHub: <strong>Not queried</strong></p>
              {gitStatus.commitsAhead !== null && <p>
                Ahead of stored upstream: {gitStatus.commitsAhead}
                {' · '} Behind: {gitStatus.commitsBehind}</p>}
              <p>Modified or staged files: <strong>{gitStatus.modifiedOrStagedCount}</strong>
                {' · '} Untracked files: <strong>{gitStatus.untrackedCount}</strong></p>
              {gitStatus.displayedChanges.length === 0 && gitStatus.worktree === 'clean' &&
                <p>No local modifications detected in this worktree.</p>}
              <div className="continuityEvents">
                {gitStatus.displayedChanges.map((item, index) =>
                  <article className="continuityEvent" key={item.relativePath + item.stage + index}>
                    <strong>{item.relativePath}</strong>
                    <span>{item.kind.replaceAll('_', ' ')}</span>
                    <p>{item.stage.replaceAll('_', ' ')}</p>
                  </article>)}
              </div>
              {gitStatus.truncated && <p role="status">
                More than 120 changed files were found. The visible list is truncated.</p>}
              <div className="continuityCapture">
                <h3>Review local differences and commits</h3>
                <div className="gitReviewControls">
                  <label htmlFor="git-diff-scope">Diff source</label>
                  <select id="git-diff-scope" value={gitDiffScope}
                    disabled={gitEvidenceBusy}
                    onChange={event => {
                      setGitDiffScope(event.target.value);
                      setGitDiff(null);
                      setGitHistory(null);
                      gitEvidenceRequest.current += 1;
                      setGitEvidenceBusy(false);
                    }}>
                    <option value="worktree">Unstaged changes</option>
                    <option value="staged">Staged changes</option>
                  </select>
                  <button type="button" className="ghostBtn" disabled={gitEvidenceBusy || gitBusy}
                    onClick={() => loadGitEvidence()}>
                    {gitEvidenceBusy ? 'Inspecting…' : 'Inspect diff and history'}
                  </button>
                </div>
              </div>
              <p>Local-only evidence. Untracked file contents are not included in patch text.
                Diff previews are capped at 64 KiB and may be incomplete. No Git command is executed.</p>
              {gitDiff && gitDiff.projectId === activeProject?.projectId &&
                <div className="continuityDocument" aria-label="Local Git patch">
                <p><strong>{gitDiff.scope === 'staged' ? 'Staged' : 'Unstaged'} changes</strong>
                  {' · '}{gitDiff.filesChanged} changed file(s)
                  {' · '}{gitDiff.patchBytes.toLocaleString()} patch bytes
                  {gitDiff.truncated && ' · truncated'}</p>
                {gitDiff.patch
                  ? <pre className="gitPatchPreview">{gitDiff.patch}</pre>
                  : <p>No tracked patch changes in the selected scope.</p>}
              </div>}
              {gitHistory && gitHistory.projectId === activeProject?.projectId &&
                <div className="continuityDocument" aria-label="Local Git history">
                <h4>Local HEAD history</h4>
                {gitHistory.commits.length === 0 && <p>No local commits found.</p>}
                {gitHistory.commits.map(item => <p key={item.commitSha}>
                  <code>{item.commitSha.slice(0, 12)}</code> · {item.summary}
                </p>)}
                {gitHistory.truncated && <p role="status">History limited to 30 commits.</p>}
                <p>Locally stored history only; not evidence of current GitHub state.</p>
              </div>}
              <div className="continuityCapture">
                <h3>Compare with live GitHub</h3>
                <button type="button" className="ghostBtn"
                  disabled={githubBusy || gitBusy}
                  onClick={() => compareLiveGitHub()}>
                  {githubBusy ? 'Checking GitHub…' : 'Check GitHub'}
                </button>
              </div>
              <p>This explicitly checks the current head of a public GitHub origin branch.
                It does not fetch files into your project, use credentials, or modify Git.</p>
              <div className="continuityCapture">
                <h3>Exact-commit CI evidence</h3>
                <button type="button" className="ghostBtn"
                  disabled={!activeProject || exactCiBusy || gitBusy}
                  onClick={() => inspectExactHeadCi()}>
                  {exactCiBusy ? 'Reading GitHub Actions…' : 'Check exact HEAD CI'}
                </button>
              </div>
              <p>This is an explicit public GitHub Actions metadata check for the local HEAD
                commit. It does not run tests or prove unsaved changes, a packaged installer,
                or an installed Windows application.</p>
              {exactCi && exactCi.projectId === activeProject?.projectId &&
                <div className="continuityDocument" aria-label="Exact-head GitHub CI evidence">
                  <h4>Observed CI: {exactCi.verdict.replaceAll('_', ' ')}</h4>
                  <p>Commit: <code>{exactCi.localHead || 'No local commit'}</code>
                    {' · '} Worktree: <strong>{exactCi.worktree.replaceAll('_', ' ')}</strong></p>
                  {exactCi.checkedUnixSeconds &&
                    <p>Observed at {new Date(exactCi.checkedUnixSeconds * 1000).toLocaleString()}</p>}
                  {exactCi.errorCode && <p role="status">
                    Incomplete verification: {exactCi.errorCode.replaceAll('_', ' ')}</p>}
                  {exactCi.workflows.map(w => <article className="continuityEvent" key={w.workflow}>
                    <strong>{w.workflow.replaceAll('_', ' ')} · {w.verdict.replaceAll('_', ' ')}</strong>
                    {w.runUrl && <p>Run: <a href={w.runUrl} target="_blank"
                      rel="noreferrer">{w.runId}</a> · {w.runStatus.replaceAll('_', ' ')}</p>}
                    {w.jobs.map(job => <p key={job.name}>{job.name}:
                      {' '}{job.status.replaceAll('_', ' ')} / {job.conclusion.replaceAll('_', ' ')}</p>)}
                  </article>)}
                  <p>Installed app: <strong>Not tested</strong>. Task acceptance: <strong>
                    Not verified</strong>. CI receipts are commit-specific and may become stale.</p>
                </div>}
              {githubComparison && <div className="continuityDocument" aria-label="Live GitHub comparison">
                <p><strong>{githubComparison.state.replaceAll('_', ' ')}</strong>
                  {githubComparison.repository && <> · <code>{githubComparison.repository}</code></>}
                  {githubComparison.branch && <> / <code>{githubComparison.branch}</code></>}
                </p>
                {githubComparison.githubHead && <p>GitHub branch commit:
                  <code> {githubComparison.githubHead}</code></p>}
                <p>Local HEAD vs GitHub: <strong>
                  {githubComparison.localVsGithub.replaceAll('_', ' ')}</strong></p>
                <p>Stored tracking ref vs GitHub: <strong>
                  {githubComparison.trackingVsGithub.replaceAll('_', ' ')}</strong></p>
                <p>Local worktree: <strong>{githubComparison.worktree.replaceAll('_', ' ')}</strong>
                  {githubComparison.dirtyFileCount !== null &&
                    <> · Changed/untracked files: {githubComparison.dirtyFileCount}</>}</p>
                {githubComparison.checkedUnixSeconds !== null &&
                  <p>Checked: {new Date(githubComparison.checkedUnixSeconds * 1000).toLocaleString()}</p>}
                {githubComparison.errorCode && <p role="status">
                  GitHub could not be verified: {githubComparison.errorCode.replaceAll('_', ' ')}</p>}
                <p>Commit differences do not establish ancestry. GitHub evidence can become stale
                  after the check. Public, unauthenticated API only.</p>
              </div>}
            </div>}
          </section>
        : isContinuity
        ? <section className="workspaceContent continuityWorkspace" aria-label="Project continuity">
            <div className="sectionHeading">
              <h2>Development ledger</h2>
              <span>{projectContext?.events.length ?? 0} recent recorded events</span>
            </div>
            <p className="capabilityNotice">
              Local IDE observations are historical evidence, not execution authority.
              Project notes are untrusted source content; missing Git, toolchain, and
              artifact checks are explicitly not verified.
            </p>
            <div className="continuityCapture">
              <h3>Development handoff</h3>
              <button type="button" className="ghostBtn"
                disabled={!activeProject || contextBusy || checkpointBusy}
                onClick={() => copyDevelopmentCheckpoint()}>
                {checkpointBusy ? 'Refreshing evidence…' : 'Copy development checkpoint'}
              </button>
            </div>
            <p>Copies a bounded JSON snapshot of fresh host observations, revisions and
              explicit unknowns. Excludes source text and private root paths.
              Reads are not atomic, and copying grants no authority.</p>
            {evidenceReconciliation &&
              <section className="continuityDocument" aria-label="Development evidence reconciliation">
                <h3>Development evidence — observation, not acceptance</h3>
                <p>Separate host reads. Native Git and known artifact bytes are local observations;
                  passing CI, installed desktop behavior and toolchain execution are not inspected here.</p>
                <div className="continuityEvents">
                  {evidenceReconciliation.checks.map(check =>
                    <article className="continuityEvent" key={check.id}>
                      <strong>{check.title}</strong>
                      <span>{check.state.replaceAll('_', ' ')}</span>
                      <p>{check.detail}</p>
                      <small>Evidence: {check.provenance.replaceAll('_', ' ')}</small>
                    </article>)}
                </div>
              </section>}
            <section className="continuityDocument" aria-label="Development task ledger">
              <h3>Development tasks and acceptance gates</h3>
              <p>Task updates are human-authored history, not proof of completion.
                The IDE records the current local commit and keeps CI, installed
                Windows acceptance and human approval separate.</p>
              {activeProject && <div className="taskEditor">
                <label htmlFor="ideTaskId">Task ID</label>
                <input id="ideTaskId" type="text" maxLength={64}
                  value={taskIdInput} onChange={event => setTaskIdInput(event.target.value)}
                  placeholder="IDE-C6" disabled={taskBusy} />
                <label htmlFor="ideTaskTitle">Short title</label>
                <input id="ideTaskTitle" type="text" maxLength={120}
                  value={taskTitleInput} onChange={event => setTaskTitleInput(event.target.value)}
                  placeholder="Verify task evidence" disabled={taskBusy} />
                <label htmlFor="ideTaskStatus">Human progress</label>
                <select id="ideTaskStatus" value={taskStateInput}
                  disabled={taskBusy} onChange={event => setTaskStateInput(event.target.value)}>
                  <option value="planned">Planned</option>
                  <option value="in_progress">In progress</option>
                  <option value="blocked">Blocked</option>
                  <option value="ready_for_review">Ready for review</option>
                </select>
                <label htmlFor="ideTaskNext">Next action</label>
                <input id="ideTaskNext" type="text" maxLength={240}
                  value={taskNextActionInput} onChange={event => setTaskNextActionInput(event.target.value)}
                  placeholder="Review latest CI and installed acceptance" disabled={taskBusy} />
                <fieldset className="taskGateChoices">
                  <legend>Required acceptance gates</legend>
                  {taskGateChoices.map(gate => <label key={gate.id}>
                    <input type="checkbox" disabled={taskBusy}
                      checked={taskRequiredGates.includes(gate.id)}
                      onChange={event => setTaskRequiredGates(previous =>
                        event.target.checked
                          ? [...previous, gate.id]
                          : previous.filter(value => value !== gate.id))} />
                    {gate.title}
                  </label>)}
                </fieldset>
                <button className="ghostBtn" type="button"
                  disabled={taskBusy || contextBusy || !activeProject}
                  onClick={() => recordTaskUpdate()}>
                  {taskBusy ? 'Recording…' : 'Record task update'}
                </button>
              </div>}
              {!contextBusy && projectContext?.tasks.length === 0 &&
                <p>No task records yet. Recording one does not change your source files.</p>}
              {!contextBusy && projectContext?.tasks.map(task =>
                <article className="continuityEvent" key={task.taskId}>
                  <div className="capabilityHeading">
                    <strong>{task.taskId}: {task.title}</strong>
                    <span>{task.status.replaceAll('_', ' ')}</span>
                  </div>
                  <p>Next: {task.nextAction}</p>
                  <p>Linked source commit: <code>{task.linkedCommit || 'not observed'}</code>
                    {' · '}Worktree: {task.linkedWorktree.replaceAll('_', ' ')}</p>
                  <p>Last progress: {new Date(task.updatedUnixSeconds * 1000).toLocaleString()}
                    {' · '}Task acceptance: <strong>Not verified</strong></p>
                  <p>CI observation: {task.ciLinkState.replaceAll('_', ' ')}
                    {task.observedCi && <> · {task.observedCi.verdict.replaceAll('_', ' ')}</>}</p>
                  {task.acceptanceGates.map(gate =>
                    <p key={gate.gate}>{gate.gate.replaceAll('_', ' ')}:
                      <strong> {gate.state.replaceAll('_', ' ')}</strong></p>)}
                  <div className="continuityCapture">
                    <button className="ghostBtn" type="button" disabled={taskBusy || contextBusy}
                      onClick={() => editTask(task)}>Edit progress</button>
                    <button className="ghostBtn" type="button"
                      disabled={taskBusy || contextBusy || !task.linkedCommit ||
                        task.linkedCommit !== projectContext?.gitWorktree.localHead}
                      onClick={() => observeTaskCi(task)}>
                      {taskBusy ? 'Working…' : 'Check & record task CI'}
                    </button>
                  </div>
                </article>)}
            </section>
            {!activeProject && <p>Open a project to inspect its development context.</p>}
            {contextBusy && <p role="status">Reading project history…</p>}
            {!contextBusy && projectContext && <div className="continuityBody">
              <p><strong>{projectContext.projectName}</strong> · Project ID <code>{projectContext.projectId}</code></p>
              <p>Remote Git: <strong>{projectContext.remoteGitState.replaceAll('_', ' ')}</strong>
                {' · '} Local artifacts: <strong>{projectContext.localArtifacts.replaceAll('_', ' ')}</strong>
                {' · '} Toolchains: <strong>{projectContext.currentToolchainReadiness.replaceAll('_', ' ')}</strong></p>
              <div className="continuityCapture">
                <h3>Local artifacts and benchmark evidence</h3>
                <button type="button" className="ghostBtn" disabled={snapshotBusy || contextBusy}
                  onClick={() => captureInventorySnapshot()}>
                  {snapshotBusy ? 'Recording…' : 'Record evidence snapshot'}
                </button>
              </div>
              <p>Bounded scan of common project folders only. A SHA-256 digest verifies
                inspected bytes, not whether an installer works. No binaries are launched.</p>
              <p>Local Git worktree: <strong>{projectContext.gitWorktree.worktree.replaceAll('_', ' ')}</strong>
                {' · '} Tracked/staged changes: {projectContext.gitWorktree.modifiedOrStagedCount}
                {' · '} Untracked: {projectContext.gitWorktree.untrackedCount}
                {' · '} Locally known upstream: {projectContext.gitWorktree.upstreamComparison.replaceAll('_', ' ')}
                {' · '} Live GitHub: not queried</p>
              {projectContext.gitWorktree.commitsAhead !== null &&
                <p>Local branch commits ahead of stored upstream: {projectContext.gitWorktree.commitsAhead}
                  {' · '} Behind: {projectContext.gitWorktree.commitsBehind}</p>}
              {projectContext.gitWorktree.displayedChanges.map(item =>
                <p key={item.relativePath + item.stage}>
                  <code>{item.relativePath}</code> · {item.kind.replaceAll('_', ' ')}
                  {' · '}{item.stage.replaceAll('_', ' ')}
                </p>)}
              {projectContext.gitWorktree.truncated &&
                <p role="status">Git changes truncated; more are present.</p>}
              <p>Git observation: <strong>{projectContext.inventory.git.state.replaceAll('_', ' ')}</strong>
                {projectContext.inventory.git.branch && <> · Branch <code>{projectContext.inventory.git.branch}</code></>}
                {' · '}Local tracking: <strong>{projectContext.inventory.git.trackingComparison.replaceAll('_', ' ')}</strong>
                {' · '}Working tree: <strong>{projectContext.inventory.git.workingTree.replaceAll('_', ' ')}</strong>
                {' · '}Live GitHub: <strong>{projectContext.inventory.git.liveRemote.replaceAll('_', ' ')}</strong>
              </p>
              {!projectContext.inventory.discoveryComplete && <p role="status">
                Inventory limit reached. More project artifacts may exist.</p>}
              {projectContext.inventory.files.length === 0 && <p>
                No files found in the scanned folders; other local folders were not inspected.</p>}
              <div className="continuityEvents">
                {projectContext.inventory.files.map(file => <article className="continuityEvent" key={file.relativePath}>
                  <strong>{file.relativePath}</strong>
                  <span>{file.category.replaceAll('_', ' ')}</span>
                  <p>{file.sizeBytes.toLocaleString()} bytes · {file.state.replaceAll('_', ' ')}
                    {' · '}{file.digestStatus.replaceAll('_', ' ')}</p>
                  {file.sha256 && <code>SHA-256: {file.sha256}</code>}
                  <button type="button" className="ghostBtn" disabled={resourceProbeBusy}
                    onClick={() => inspectResource(file.relativePath)}>
                    {resourceProbeBusy && resourceProbe?.relativePath === file.relativePath
                      ? 'Inspecting…' : 'Recheck file bytes'}
                  </button>
                </article>)}
              </div>
              {resourceProbe && resourceProbe.projectId === activeProject?.projectId &&
                <div className="continuityDocument" aria-label="Current resource evidence">
                  <h4>Current resource: <code>{resourceProbe.relativePath}</code></h4>
                  <p><strong>{resourceProbe.state.replaceAll('_', ' ')}</strong>
                    {' · '} {resourceProbe.byteLength === null
                      ? 'Size unavailable' : resourceProbe.byteLength.toLocaleString() + ' bytes'}
                    {' · '} {resourceProbe.digestStatus.replaceAll('_', ' ')}</p>
                  {resourceProbe.sha256 && <p>Observed SHA-256:
                    <code> {resourceProbe.sha256}</code></p>}
                  <p>Executable usability: <strong>Not tested</strong>.
                    A file hash does not prove an installer or compiler launches.</p>
                </div>}
              <h3>Changes since recorded evidence</h3>
              {!inventoryComparison?.baselinePresent
                ? <p>No earlier snapshot recorded for this project. Record one to compare later.</p>
                : <>
                  <p>{inventoryComparison.baselineCount} recorded snapshot(s) ·
                    Latest baseline: {new Date(inventoryComparison.baselineUnixSeconds * 1000).toLocaleString()}
                    {' · '} Local Git HEAD: {inventoryComparison.localGitHeadChange.replaceAll('_', ' ')}
                    {' · '} Worktree vs baseline: {inventoryComparison.gitWorktreeChange.replaceAll('_', ' ')}
                    {' · '} Live remote: {inventoryComparison.liveRemote.replaceAll('_', ' ')}</p>
                  {inventoryComparison.entries.length === 0 &&
                    <p>No files were present in either scanned inventory.</p>}
                  {inventoryComparison.entries.map(item => <article className="continuityEvent" key={item.relativePath}>
                    <strong>{item.relativePath}</strong>
                    <span>{item.status.replaceAll('_', ' ')}</span>
                    {item.baselineSha256 && <p>Earlier SHA-256: <code>{item.baselineSha256}</code></p>}
                    {item.currentSha256 && <p>Current SHA-256: <code>{item.currentSha256}</code></p>}
                  </article>)}
                </>}
              <h3>Recent host observations</h3>
              {projectContext.events.length === 0 && <p>No development events recorded by this IDE yet.</p>}
              <div className="continuityEvents">
                {projectContext.events.map(item => <article key={item.eventId} className="continuityEvent">
                  <strong>{item.operation.replaceAll('_', ' ')}</strong>
                  <span>{new Date(item.observedUnixSeconds * 1000).toLocaleString()}</span>
                  <p>{item.summary}</p>
                  {item.sourcePath && <p>File: <code>{item.sourcePath}</code></p>}
                  {item.sourceRevision && <code>Revision: {item.sourceRevision}</code>}
                </article>)}
              </div>
              <h3>Project orientation documents</h3>
              {projectContext.documents.map(item => <article key={item.relativePath} className="continuityDocument">
                <div className="capabilityHeading">
                  <strong>{item.relativePath}</strong>
                  <span>{item.availability.replaceAll('_', ' ')}</span>
                </div>
                {item.sourceRevision && <p>Saved revision <code>{item.sourceRevision}</code></p>}
                {item.excerpt && <pre>{item.excerpt}</pre>}
                {item.excerptTruncated && <p>Excerpt limited to 1,200 characters. Open the file for full content.</p>}
              </article>)}
            </div>}
          </section>
        : isCapabilities
        ? <section className="workspaceContent capabilityWorkspace" aria-label="IDE capability directory">
            <div className="sectionHeading">
              <h2>Available host operations</h2>
              <span>{capabilityCatalog?.capabilities?.length ?? 0} capabilities · Schema {capabilityCatalog?.schemaVersion ?? '—'}</span>
            </div>
            <p className="capabilityNotice">
              This directory describes operations already supported by the IDE.
              It does not run them, grant permissions, or connect to Lu.
              File saves still require an explicit human action.
            </p>
            {capabilityBusy && <p role="status">Loading capabilities…</p>}
            {!capabilityBusy && capabilityCatalog && <div className="capabilityGrid">
              {capabilityCatalog.capabilities.map(item =>
                <article key={item.id} className="capabilityCard">
                  <div className="capabilityHeading">
                    <h3>{item.title}</h3>
                    <span className="capabilityEffect">{item.effect.replaceAll('_', ' ')}</span>
                  </div>
                  <p>{item.description}</p>
                  <dl>
                    <dt>Operation</dt><dd><code>{item.id}</code> · v{item.version}</dd>
                    <dt>Inputs</dt>
                    <dd>{Object.entries(item.requestSchema.properties ?? {}).length
                      ? Object.entries(item.requestSchema.properties).map(([name, type]) =>
                        name + ': ' + (type.type ?? type.$ref?.split('/').pop() ?? 'union')).join(', ')
                      : 'None'}</dd>
                    <dt>Result</dt>
                    <dd><code>{item.resultSchema.$ref?.split('/').pop() ??
                      item.resultSchema.type ?? 'Typed result'}</code></dd>
                    <dt>Before use</dt><dd>{item.precondition}</dd>
                    <dt>Evidence</dt><dd>{item.evidence}</dd>
                  </dl>
                  <details className="capabilitySchemaDetails">
                    <summary>Inspect full typed request, result and error schemas</summary>
                    <pre>{JSON.stringify({
                      request: item.requestSchema, result: item.resultSchema,
                      error: item.errorSchema,
                    }, null, 2)}</pre>
                  </details>
                </article>)}
            </div>}
          </section>
        : isLanguages
        ? <section className="workspaceContent capabilityWorkspace" aria-label="Language services">
            <div className="sectionHeading">
              <h2>Language server providers</h2>
              <span>{languageServers.length} catalogued · {lspWorker?.ready ? 'Pinned binaries verified · OS admission on start' : 'Execution gated'}</span>
            </div>
            <p className="capabilityNotice" role="status">
              <strong>{lspWorker?.ready ? 'Reviewed local server assets available; none auto-started.' : 'Language servers are not running.'}</strong>
              {' '}{lspWorker?.detail || 'Worker state unavailable.'}
              {' '}Server discovery does not grant execution or install packages.
            </p>
            <div className="capabilityGrid">
              {languageServers.map(server => <article className="capabilityCard" key={server.id}>
                <div className="capabilityHeading">
                  <h3>{server.label}</h3><span className="capabilityEffect">{server.transport}</span>
                </div>
                <p>{server.languageIds?.join(', ') || 'Unknown language'}</p>
                <dl>
                  <dt>License</dt><dd>{server.license}</dd>
                  <dt>Launcher hint</dt><dd><code>{server.executableBasename}</code></dd>
                  <dt>Availability</dt><dd>{server.lifecycle === 'catalog_only'
                    ? 'Catalogued only · Installed binary not verified' : server.lifecycle}</dd>
                  <dt>Source</dt><dd><a href={server.upstream} target="_blank"
                    rel="noreferrer">Publisher documentation</a></dd>
                </dl>
              </article>)}
            </div>
            <p className="capabilityNotice">
              Installer candidates: native rust-analyzer, clangd and Ruff;
              Node-dependent Pyright and TypeScript are separate license and dependency
              reviews. No binary is packaged or downloaded by the current alpha.
            </p>
          </section>
        : !isExplorer
        ? <section aria-label="Projects" className="workspaceContent">
          <div className="sectionHeading"><h2>Registered projects</h2>
            <span>{projects.length} project{projects.length === 1 ? '' : 's'}</span></div>
          {projects.length === 0
            ? <div className="emptyState"><div className="emptySymbol">⌘</div>
                <h3>No projects yet</h3>
                <p>Choose a folder through the system dialog. Project scripts,
                  package installations and language servers remain disabled.</p>
                <button className="primaryBtn" disabled={pending}
                  onClick={() => operation('ide_choose_project')}>Choose a folder</button>
              </div>
            : <div className="projectGrid">
                {projects.map(project => <article className="projectCard" key={project.projectId}>
                  <div className="projectIcon" aria-hidden="true">▤</div>
                  <div className="projectDetails">
                    <h3>{project.displayName}</h3>
                    <p>{project.available ? (project.active ? 'Open · editor available' :
                      'Closed · ready to reopen') : 'Directory unavailable'}</p>
                    <small>{project.projectId.slice(0, 12)}…</small>
                  </div>
                  <div className="projectActions">
                    {project.active && <button className="primaryBtn"
                      disabled={explorerPending || pending}
                      onClick={() => openDirectory(project, '')}>Edit</button>}
                    {project.active
                      ? <button className="ghostBtn" disabled={pending || !!savingKey}
                          onClick={() => operation('ide_close_project',
                            {projectId: project.projectId})}>Close</button>
                      : <button className="ghostBtn" disabled={pending || !project.available}
                          onClick={() => operation('ide_open_project',
                            {projectId: project.projectId})}>Reopen</button>}
                    <button className="removeProjectBtn"
                      disabled={pending || explorerPending || !!savingKey}
                      aria-label={'Remove ' + project.displayName + ' from IDE registration'}
                      onClick={() => confirmUnregister(project)}>Remove from IDE</button>
                  </div>
                </article>)}
              </div>}
          <div className="infoNotice"><strong>Human-owned editing only.</strong>
            Saving is explicit and revision-checked. This alpha does not execute scripts,
            install packages, or run a language server.</div>
        </section>
        : <section className="sourceWorkspace" aria-label="Source explorer and editor">
          <div className="sourceTree">
            <div className="sourceTreeHeader">
              <strong>Files</strong><span>Project</span>
            </div>
            <div className="sourcePath" title={directory || '/'}>
              <button type="button" className="pathLink" disabled={explorerPending}
                onClick={() => activeProject && openDirectory(activeProject, '')}>
                {activeProject?.displayName}
              </button>
              {directory && <span> / {directory}</span>}
            </div>
            {directory && <button type="button" className="fileRow backRow"
              disabled={explorerPending} onClick={() => openDirectory(activeProject, parentPath)}>↰ ..</button>}
            {listing?.entries.map(entry =>
              <button type="button" key={entry.relativePath}
                className={'fileRow' + (activeTab?.relativePath === entry.relativePath ? ' active' : '')}
                disabled={explorerPending || (entry.kind !== 'directory' && entry.kind !== 'file')}
                title={entry.relativePath}
                onClick={() => selectEntry(entry)}>
                <span aria-hidden="true" className="fileGlyph">{entry.kind === 'directory' ? '▸' :
                  entry.kind === 'file' ? '▤' : '⤷'}</span>
                <span className="fileName">{entry.name}</span>
              </button>)}
            {listing?.truncated && <p className="treeCaution">Large directory: showing first 400 entries.</p>}
            {!explorerPending && listing && listing.entries.length === 0 &&
              <p className="treeCaution">Empty directory.</p>}
          </div>
          <div className="sourcePreview" role="region" aria-label="Source code editor">
            <div className="editorTabs" role="tablist" aria-label="Open source files">
              {projectTabs.map(tab => <div className={'editorTab' +
                (tab.key === activeTabKey ? ' selected' : '')} key={tab.key}>
                <button type="button" role="tab" aria-selected={tab.key === activeTabKey}
                  className="editorTabSelect" title={tab.relativePath}
                  onClick={() => setActiveTabKey(tab.key)}>
                  <span>{leafFor(tab.relativePath)}</span>
                  {tab.dirty && <span className="unsavedDot" title="Unsaved changes">●</span>}
                </button>
                <button type="button" className="editorTabClose"
                  disabled={savingKey === tab.key} title={'Close ' + tab.relativePath}
                  aria-label={'Close ' + tab.relativePath} onClick={() => closeTab(tab.key)}>×</button>
              </div>)}
            </div>
            {activeTab && activeTab.projectId === activeProject?.projectId ? <>
              <div className="sourcePreviewHeader">
                <strong>{activeTab.relativePath}</strong>
                <span className="editorToolbar">
                  <span>{activeTab.draft.length.toLocaleString()} chars ·
                    {activeTab.dirty ? ' Unsaved' : ' Saved'}</span>
                  <button className="ghostBtn editorToolButton"
                    disabled={referencePending || explorerPending || !!savingKey}
                    title="Copy a reference to the current saved file and cursor line"
                    onClick={() => copySourceReference(activeTab)}>
                    {referencePending ? 'Checking…' : 'Copy source reference'}
                  </button>
                  {lspAvailableForTab && !lspSession && <button type="button"
                    className="ghostBtn editorToolButton" disabled={lspBusy}
                    title={'Require native human consent, then start isolated '+lspProviderForTab}
                    onClick={() => void startLspSession()}>
                    {lspBusy ? 'Starting…' : 'Start '+lspProviderForTab}</button>}
                  {lspRunningForTab && <button type="button"
                    className="ghostBtn editorToolButton" disabled={lspBusy}
                    onClick={() => void stopLspSession()}>
                    {lspBusy ? 'Stopping…' : 'Stop '+lspProviderForTab}</button>}
                  <button type="button" className="ghostBtn editorToolButton"
                    disabled={!lspRunningForTab || lspBusy || !lspSupportsNavigation}
                    title={lspRunningForTab ? 'Ask protected '+lspProviderForTab+' for completions' :
                      'Start a consented protected clangd session first'}
                    onClick={() => void requestLspFeature('completion')}>Complete</button>
                  <button type="button" className="ghostBtn editorToolButton"
                    disabled={!lspRunningForTab || lspBusy || !lspSupportsNavigation}
                    onClick={() => void requestLspFeature('hover')}>Hover</button>
                  <button type="button" className="ghostBtn editorToolButton"
                    disabled={!lspRunningForTab || lspBusy || !lspSupportsNavigation}
                    onClick={() => void requestLspFeature('definition')}>Definition</button>
                  <button type="button" className="ghostBtn editorToolButton"
                    disabled={!lspRunningForTab || lspBusy}
                    onClick={() => void requestLspFeature('diagnostics')}>Problems</button>
                  <button className="ghostBtn editorToolButton" aria-label="Find in file"
                    onClick={() => { setFindOpen(true); setFindIndex(-1); }}>Find · Ctrl+F</button>
                  <button className="ghostBtn editorToolButton" disabled={explorerPending || !!savingKey}
                    onClick={() => reloadTab(activeTab)}>Reload</button>
                  <button className="ghostBtn editorToolButton"
                    disabled={!activeTab.dirty || explorerPending || !!savingKey || reviewPending}
                    onClick={() => reviewTab(activeTab)}>
                    {reviewPending ? 'Reviewing…' : 'Review changes'}
                  </button>
                  <button className="primaryBtn editorToolButton"
                    disabled={!activeTab.dirty || explorerPending || !!savingKey}
                    onClick={() => saveTab(activeTab)}>
                    {savingKey === activeTab.key ? 'Saving…' : 'Save · Ctrl+S'}
                  </button>
                </span>
              </div>
              {findOpen && <div className="editorFindPanel" role="search" aria-label="Find in active file">
                <div className="editorFindBar">
                  <input aria-label="Find text" placeholder="Find in file"
                    value={findQuery} autoFocus
                    onChange={event => { setFindQuery(event.target.value); setFindIndex(-1); }}
                    onKeyDown={event => {
                      if (event.key === 'Enter') { event.preventDefault(); navigateMatch(event.shiftKey ? -1 : 1); }
                      if (event.key === 'Escape') { event.preventDefault(); setFindOpen(false); }
                    }}/>
                  <label className="editorFindOption"><input type="checkbox"
                    checked={caseSensitiveFind} onChange={event => {
                      setCaseSensitiveFind(event.target.checked); setFindIndex(-1);
                    }}/> Match case</label>
                  <span className="editorFindCount">
                    {matchPlan.truncated ? 'Too many matches or query too long' :
                      matches.length ? `${Math.max(0, Math.min(findIndex + 1, matches.length))} of ${matches.length}` : 'No matches'}
                  </span>
                  <button type="button" aria-label="Previous match" disabled={!matches.length || matchPlan.truncated}
                    onClick={() => navigateMatch(-1)}>↑</button>
                  <button type="button" aria-label="Next match" disabled={!matches.length || matchPlan.truncated}
                    onClick={() => navigateMatch(1)}>↓</button>
                  <button type="button" aria-label="Toggle replace"
                    onClick={() => setReplaceVisible(value => !value)}>
                    {replaceVisible ? 'Hide replace' : 'Replace'}
                  </button>
                  <button type="button" aria-label="Close find" onClick={() => setFindOpen(false)}>×</button>
                </div>
                {replaceVisible && <div className="editorReplaceBar">
                  <input aria-label="Replacement text" placeholder="Replace with (literal text)"
                    value={replaceText} onChange={event => setReplaceText(event.target.value)}
                    onKeyDown={event => { if (event.key === 'Escape') setFindOpen(false); }}/>
                  <button type="button" aria-label="Replace current match"
                    disabled={!matches.length || matchPlan.truncated || !!savingKey || explorerPending}
                    onClick={() => replaceMatches(false)}>Replace</button>
                  <button type="button" aria-label="Replace all matches"
                    disabled={!matches.length || matchPlan.truncated || !!savingKey || explorerPending}
                    onClick={() => replaceMatches(true)}>Replace all</button>
                  <span className="editorFindCount">Draft only · Undo available · Save required</span>
                </div>}
              </div>}
              <CodeEditor
                diagnostics={activeLspDiagnostics}
                jumpRequest={lspJumpRequest}
                completionApplyRequest={lspCompletionApplyRequest}
                onCompletionApplied={(nonce, applied) => {
                  setLspCompletionApplyRequest(previous => previous?.nonce === nonce ? null : previous);
                  if (!applied) setProblem('Source changed before completion could be inserted.');
                }}
                onCursorPosition={position => setCursorPositions(previous => {
                  const prior = previous[activeTab.key];
                  if (prior?.line === position.line && prior?.character === position.character) return previous;
                  return {...previous, [activeTab.key]:position};
                })}
                findRequest={findRequest?.tabKey === activeTab.key ? findRequest : null}
                replaceRequest={replaceRequest?.tabKey === activeTab.key ? replaceRequest : null}
                onReplaceApplied={(nonce, applied) => {
                  setReplaceRequest(previous => previous?.nonce === nonce ? null : previous);
                  if (!applied) setProblem('Editor changed before replacement. Review the current draft and retry.');
                }}
                key={activeTab.key + ':' + activeTab.generation}
                tabIdentity={activeTab.key + ':' + activeTab.generation}
                editorState={editorStates.current.get(activeTab.key)}
                onCursorLine={line => setCursorByTab(previous =>
                  previous[activeTab.key] === line ? previous :
                    { ...previous, [activeTab.key]: line })}
              />
              {activeLspDiagnostics.length > 0 && <section className="lspResultPanel"
                aria-label="Language server problems">
                <strong>Problems · {activeLspDiagnostics.length}</strong>
                {activeLspDiagnostics.map((diagnostic, index) =>
                  <button key={index} className="lspDiagnostic" type="button"
                    onClick={() => setLspJumpRequest({
                      from:diagnostic.from, to:diagnostic.to,
                      tabIdentity:activeTab.key + ':' + activeTab.generation,
                      nonce:Date.now()
                    })}>
                    <span>{diagnostic.severity === 1 ? 'Error' :
                      diagnostic.severity === 2 ? 'Warning' : 'Info'} · {diagnostic.line}</span>
                    <span>{diagnostic.message}</span>
                  </button>)}
              </section>}
              {activeLspCompletions.length > 0 && <section className="lspResultPanel"
                aria-label="Language server completions">
                <strong>Suggestions</strong>
                {activeLspCompletions.slice(0, 20).map((item, index) =>
                  <button type="button" key={index} className="lspSuggestion"
                    onClick={() => applyLspCompletion(item)}>
                    <code>{item.label}</code> <span>{item.detail}</span>
                  </button>)}
              </section>}
              {activeLspHover && <section className="lspResultPanel"
                aria-label="Language server hover information">
                <strong>Symbol information</strong>
                <pre>{activeLspHover}</pre>
              </section>}
              <div className="editorInfoBar">
                {lspRunningForTab ? 'Protected '+lspProviderForTab+' running · approved session · ' :
                  'Language intelligence requires protected worker and native consent · '}
                Edits stay in memory until Save.
                External changes cause a conflict instead of silently overwriting them.
              </div>
            </> : <div className="sourcePreviewEmpty">
              <div className="emptySymbol">⌘</div>
              <h3>Select a source file</h3>
              <p>Open a file from the Explorer to edit it. Each tab keeps its own undo/redo history.
                The editor never executes code or sends your files to Lu.</p>
              {explorerPending && <p role="status">Loading…</p>}
            </div>}
          </div>
        </section>}
      {review && <div className="reviewOverlay" role="presentation">
        <section className="reviewDialog" role="dialog" aria-modal="true"
          aria-label={'Review proposed changes to ' + review.relativePath}>
          <div className="reviewHeader">
            <div>
              <h2>Review changes</h2>
              <p>{review.relativePath} · {review.countsExact
                ? review.removedLines + ' removed, ' + review.addedLines + ' added'
                : 'Diff exceeds safe processing limits'} · {review.hunks.length} visible hunk(s)</p>
            </div>
            <button type="button" className="ghostBtn"
              onClick={() => setReview(null)}>Close review</button>
          </div>
          <p className="reviewNotice">
            This is a preview of the current human-owned draft, not a Lu-generated suggestion.
            No files are changed by reviewing. Future Lu proposals will require
            independent approval before the IDE applies them.
          </p>
          <div className="reviewLines" aria-label="Proposed source changes">
            {review.hunks.map((hunk, hunkIndex) =>
              <section className="reviewHunk" key={hunkIndex}
                aria-label={'Diff hunk ' + (hunkIndex + 1)}>
                <div className="reviewHunkHeader">{hunk.header}</div>
                {hunk.lines.map((line, lineIndex) =>
                  <div key={lineIndex} className={'reviewLine ' + line.kind}>
                    <span className="reviewOldNumber" title="Original line">
                      {line.oldLine ?? ''}
                    </span>
                    <span className="reviewNewNumber" title="Proposed line">
                      {line.newLine ?? ''}
                    </span>
                    <span className="reviewMark" aria-hidden="true">
                      {line.kind === 'added' ? '+' : line.kind === 'removed' ? '−' : ' '}
                    </span>
                    <code>{line.text.endsWith('\n') ? line.text.slice(0, -1) : line.text}
                      {!line.text.endsWith('\n') &&
                        <span className="reviewNoNewline"> [no newline]</span>}
                    </code>
                  </div>)}
              </section>)}
            {review.unchanged && <p>No differences from the saved version.</p>}
            {!review.unchanged && review.hunks.length === 0 &&
              <p>Diff too large to render safely. Review the source in the editor.</p>}
          </div>
          {review.truncated && <p className="reviewWarning" role="alert">
            Preview truncated to prevent excessive output. Save from this review is
            unavailable; use the regular editor Save only after inspecting the full draft.
          </p>}
          <div className="reviewActions">
            <button type="button" className="ghostBtn"
              onClick={() => setReview(null)}>Return to editor</button>
            <button type="button" className="primaryBtn"
              disabled={review.truncated || review.unchanged || !!savingKey}
              onClick={() => saveReviewedChange()}>
              {savingKey ? 'Saving…' : 'Save reviewed changes'}
            </button>
          </div>
        </section>
      </div>}
      {proposalDetail && <div className="reviewOverlay" role="presentation">
        <section className="reviewDialog proposalReviewDialog" role="dialog" aria-modal="true"
          aria-label={'Review coding proposal ' + proposalDetail.summary.taskId}>
          <div className="reviewHeader">
            <div>
              <h2>Review coding proposal</h2>
              <p>{proposalDetail.summary.taskId} · {proposalDetail.summary.editCount} file(s)</p>
            </div>
            <button type="button" className="ghostBtn"
              onClick={() => setProposalDetail(null)}>Close review</button>
          </div>
          <p className="proposalNotice">The claimed source
            “{proposalDetail.summary.claimedProducer}” is unverified. Approval
            records a review decision only. It does NOT modify files or authorize
            Lu, scripts, or tools to execute.</p>
          <p className="proposalRationale">{proposalDetail.summary.rationale}</p>
          <div className="proposalReviewFiles">
            {proposalDetail.reviews.map((file, fileIndex) =>
              <section className="proposalReviewFile" key={fileIndex}>
                <h3>{file.relativePath}</h3>
                <p>{file.countsExact
                  ? file.removedLines + ' removed · ' + file.addedLines + ' added'
                  : 'Diff too large to compute completely'}</p>
                <div className="reviewLines" aria-label={'Changes to ' + file.relativePath}>
                  {file.hunks.map((hunk, hunkIndex) =>
                    <section className="reviewHunk" key={hunkIndex}>
                      <div className="reviewHunkHeader">{hunk.header}</div>
                      {hunk.lines.map((line, lineIndex) =>
                        <div className={'reviewLine ' + line.kind} key={lineIndex}>
                          <span className="reviewOldNumber">{line.oldLine ?? ''}</span>
                          <span className="reviewNewNumber">{line.newLine ?? ''}</span>
                          <span className="reviewMark" aria-hidden="true">
                            {line.kind === 'added' ? '+' : line.kind === 'removed' ? '−' : ' '}
                          </span>
                          <code>{line.text}</code>
                        </div>)}
                    </section>)}
                </div>
                {file.truncated && <p className="reviewWarning" role="alert">
                  Review incomplete. This file cannot be approved for staging.
                </p>}
              </section>)}
          </div>
          <div className="reviewActions">
            <button type="button" className="ghostBtn"
              onClick={() => setProposalDetail(null)}>Return to inbox</button>
            {proposalDetail.summary.status === 'approved_for_staging' &&
              <button type="button" className="primaryBtn" disabled={inboxBusy}
                onClick={() => stageProposal(proposalDetail.summary)}>Stage in editor</button>}
            {proposalDetail.summary.status === 'pending' && <>
              <button type="button" className="removeProjectBtn" disabled={inboxBusy}
                onClick={() => decideProposal(false)}>Reject proposal</button>
              <button type="button" className="primaryBtn"
                disabled={inboxBusy || proposalDetail.reviews.some(file =>
                  file.truncated || !file.countsExact)}
                onClick={() => decideProposal(true)}>Approve for staging only</button>
            </>}
          </div>
        </section>
      </div>}
      <footer className="ideStatusBar"><span>● Local standalone application</span>
        <span>{savingKey ? 'Saving…' : pending || explorerPending ? 'Working…' :
          anyDirty ? '● Unsaved changes' : 'Ready'} · Ctrl+O folder · Ctrl+S save</span>
      </footer>
    </main>
  </div>;
}

createRoot(document.getElementById('root')).render(<App />);
