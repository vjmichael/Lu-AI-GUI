import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { CodeEditor, createEditorState, isLargeSourceEditor } from './code_editor.jsx';
import { secondaryEditor, initialSecondaryKey, canSaveFocusedPane } from './editor_layout.js';
import { captureProjectView, restoreProjectView } from './editor_project_view.js';
import { planProjectPaneSelection, planProjectSwitchView,
  mayCommitProjectNavigation } from './editor_project_navigation.js';
import { captureLiveEditorIntent, stillCurrentEditorIntent,
  reconcileSavedEditorTab } from './editor_live_write.js';
import { planCleanBackgroundTabClosure } from './editor_clean_tab_reclamation.js';
import { checkedQuickOpenResult, checkedDirectoryPage } from './quick_open_view.js';
import { rememberedWorkspaceFromTabs, checkedRememberedWorkspace } from './workspace_layout.js';
import { checkedPrivateDraftHead, planRestartReconciliation } from './editor_restart_reconciliation.js';
import { diskAuditTargets, classifyDiskAudit,
  summarizeExternalDifference } from './editor_disk_drift.js';
import { planThreeWayMerge, renderReviewedMerge,
  conflictExcerpt, mergeCanApply } from './editor_conflict_merge.js';
import { planTaskSourceLink, sourceLinkDisplay,
  checkedSourceReobservation } from './task_source_evidence.js';
import { safeProjectRunProfileDiscovery,
  selectExistingProtectedRecipe } from './project_run_profiles.js';
import { planReviewedAdapterRequest,
  checkedReviewedAdapterContract } from './reviewed_adapters.js';
import { admitOpenTabs, tabWindow, pageTabWindow, centerSelectedTab,
  visibleTreeEntries, MAX_OPEN_TABS_PER_PROJECT, MAX_OPEN_TABS_TOTAL,
  INITIAL_VISIBLE_TREE_ENTRIES,
  TREE_ENTRY_PAGE, verifiedRelativeSourcePath, lateSourceOpenDecision,
  lateReloadCanCommit } from './editor_working_set.js';
import { locateMatches, planReplacement } from './find_replace.js';
import {
  newBufferIdentity, advanceBufferIdentity, rebaseBufferAfterSave,
  captureBufferSpan, resolveBufferSpan, formatBufferReference,
  formatPersistentDraftReference, draftRecoveryAssessment,
  parsePersistentDraftReference,
} from './buffer_identity.js';
import { buildDevelopmentCheckpoint } from './development_checkpoint.js';
import { languageForPath, matchesSource, lspPositionOffset, visibleDiagnostics, visibleCompletions, visibleHover, visibleDefinition, visibleCrossFileDefinition } from './lsp_editor.js';
import { captureLiveLspSource, stillCurrentLspSource, lspNativeReplyMatches,
  lspRequestForSource, sourceMatchesSession, providerServesLanguage,
  sessionMemberForSource, sessionCloseRequest } from './lsp_live_source.js';
import { projectProblems, stillCurrentProblem } from './lsp_problems.js';
import { languageServiceCapabilities, languageMethodAvailable } from './lsp_language_capabilities.js';
import { reconcileDevelopmentEvidence } from './development_reconciliation.js';
import { belongsToSelectedProject } from './job_render_guard.js';
import { ingestJobProgress, visibleJobProgress } from './job_progress.js';
import { safeConventionalTestDiscovery } from './conventional_tests.js';
import { projectJobLifecycle } from './protected_job_lifecycle.js';
import { safeDebugTargetReview } from './debug_target_review.js';
import { safeDebugBinaryEvidence } from './debug_binary_evidence.js';
import { safeDebugSymbolEvidence } from './debug_symbol_evidence.js';
import { checkedProjectDebugWitness } from './project_debug_authority.js';
import { acceptNativeOutput } from './native_build_output.js';

const sections = [
  { id: 'projects', title: 'Projects', glyph: '▣' },
  { id: 'explorer', title: 'Explorer', glyph: '▤' },
  { id: 'project-search', title: 'Find in project', glyph: '⌕' },
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
  const [gitBatchPaths, setGitBatchPaths] = useState('');
  const [gitBatchReview, setGitBatchReview] = useState(null);
  const [gitBatchBusy, setGitBatchBusy] = useState(false);
  const gitBatchRequest = useRef(0);
  const [gitHunkPath, setGitHunkPath] = useState('');
  const [gitHunkPlan, setGitHunkPlan] = useState(null);
  const [gitSelectedHunkIds, setGitSelectedHunkIds] = useState([]);
  const [gitHunkSelection, setGitHunkSelection] = useState(null);
  const [gitHunkBusy, setGitHunkBusy] = useState(false);
  const [gitPartialStageBusy, setGitPartialStageBusy] = useState(false);
  const [gitPartialStageReceipt, setGitPartialStageReceipt] = useState(null);
  const [gitPartialRecovery, setGitPartialRecovery] = useState(null);
  const [gitPartialRecoveryBusy, setGitPartialRecoveryBusy] = useState(false);
  const [reviewedCommitMessage, setReviewedCommitMessage] = useState('');
  const [reviewedCommitAuthor, setReviewedCommitAuthor] = useState('');
  const [reviewedCommitEmail, setReviewedCommitEmail] = useState('');
  const [reviewedCommitPlan, setReviewedCommitPlan] = useState(null);
  const [reviewedCommitReceipt, setReviewedCommitReceipt] = useState(null);
  const [reviewedCommitBusy, setReviewedCommitBusy] = useState(false);
  const [reviewedCommitRecovery, setReviewedCommitRecovery] = useState(null);
  const [reviewedCommitRecoveryBusy, setReviewedCommitRecoveryBusy] = useState(false);
  const gitHunkRequest = useRef(0);
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
  // IDE-05.1: only data from host capability-scoped fixed manifest probes.
  const [packageInspection, setPackageInspection] = useState(null);
  const [packageInspectionBusy, setPackageInspectionBusy] = useState(false);
  const packageInspectionRequest = useRef(0);
  // IDE-05.2: typed data-only operation proposal, not a manager execution.
  const [packageManagerChoice, setPackageManagerChoice] = useState('uv');
  const [packageOperation, setPackageOperation] = useState('add');
  const [packageName, setPackageName] = useState('');
  const [packageExactVersion, setPackageExactVersion] = useState('');
  const [packagePreview, setPackagePreview] = useState(null);
  const [packageRevalidated, setPackageRevalidated] = useState(null);
  const [packagePreviewBusy, setPackagePreviewBusy] = useState(false);
  const packagePreviewRequest = useRef(0);
  const [packageRunBusy, setPackageRunBusy] = useState(false);
  const [packageRunResult, setPackageRunResult] = useState(null);
  const workflowRequest = useRef(0);

  const [pending, setPending] = useState(false);
  const [problem, setProblem] = useState('');
  const [hint, setHint] = useState('');
  const [projectSearchQuery, setProjectSearchQuery] = useState('');
  const [projectSearchCase, setProjectSearchCase] = useState(false);
  const [projectSearchResults, setProjectSearchResults] = useState(null);
  const [projectSearchBusy, setProjectSearchBusy] = useState(false);
  const projectSearchGeneration = useRef(0);
  const projectSearchOpenNonce = useRef(0);
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
  const [draftReferencePending, setDraftReferencePending] = useState(false);
  const [draftRecoveryBusy, setDraftRecoveryBusy] = useState(false);
  const [draftRecoveryByTab, setDraftRecoveryByTab] = useState({});
  const [persistentReferenceInput, setPersistentReferenceInput] = useState('');
  const [activeProject, setActiveProject] = useState(null);
  // B9 private navigation metadata: opt-in manual Remember and Restore.
  // Never persist CodeMirror EditorState, source content or B2 private drafts.
  const [rememberedLayout, setRememberedLayout] = useState(null);
  const [workspaceLayoutBusy, setWorkspaceLayoutBusy] = useState(false);
  const [workspaceRecoveryReport, setWorkspaceRecoveryReport] = useState(null);
  const [diskAuditReport, setDiskAuditReport] = useState(null);
  const [diskComparison, setDiskComparison] = useState(null);
  const [diskAuditBusy, setDiskAuditBusy] = useState(false);
  const diskAuditSerial = useRef(0);
  // B11: transient user-reviewed merge only. Never durable, never a Save.
  const [mergeReview,setMergeReview] = useState(null);
  const [mergeChoices,setMergeChoices] = useState({});
  const [mergeApplyRequest,setMergeApplyRequest] = useState(null);
  const [mergeBusy,setMergeBusy] = useState(false);
  const mergeSerial = useRef(0);
  const mergeNonce = useRef(0);
  const workspaceLayoutOp = useRef(null);
  const workspaceLayoutReadVersion = useRef(0);
  const [directory, setDirectory] = useState('');
  const [relativeOpenPath, setRelativeOpenPath] = useState('');
  const [quickOpenQuery, setQuickOpenQuery] = useState('');
  const quickOpenInputRef = useRef(null);
  const [quickOpenFocusRequest,setQuickOpenFocusRequest] = useState(0);
  const [quickOpenResults, setQuickOpenResults] = useState(null);
  const [quickOpenBusy, setQuickOpenBusy] = useState(false);
  const quickOpenBusyRef = useRef(false);
  const quickOpenSerial = useRef(0);
  const [directoryPageBusy, setDirectoryPageBusy] = useState(false);
  const [listing, setListing] = useState(null);
  const [visibleTreeCount, setVisibleTreeCount] = useState(INITIAL_VISIBLE_TREE_ENTRIES);
  const [explorerPending, setExplorerPending] = useState(false);
  // Synchronous reservation; React state alone does not stop same-tick double opens.
  const explorerIoTicket = useRef(null);
  // One workspace transition at a time, including asynchronous proposal
  // staging. This is navigation custody, never execution authority.
  const workspaceTransitionRef = useRef(null);
  const projectNavigationEpoch = useRef(0);
  const [tabs, setTabs] = useState([]);
  const [activeTabKey, setActiveTabKey] = useState(null);
  const [tabWindowStart, setTabWindowStart] = useState(0);
  const [splitEditorOpen, setSplitEditorOpen] = useState(false);
  const [secondaryTabKey, setSecondaryTabKey] = useState(null);
  // Focus is transient UI state; it never grants file or worker access.
  const [focusedEditorPane,setFocusedEditorPane] = useState('primary');
  const focusedEditorPaneRef = useRef('primary');
  const [editorFocusRequest,setEditorFocusRequest] = useState(null);
  const focusNonce = useRef(0);
  const projectProblemsPanelRef = useRef(null);
  const [problemsFocusRequest,setProblemsFocusRequest] = useState(0);
  const rovingTabFocusRef = useRef(null);
  const [savingKey, setSavingKey] = useState(null);
  // Synchronous reservation: React state cannot stop same-tick duplicate Saves.
  const savingKeyRef = useRef(null);
  const [review, setReview] = useState(null);
  const [reviewPending, setReviewPending] = useState(false);
  const [proposalInput, setProposalInput] = useState('');
  const [proposals, setProposals] = useState([]);
  const [proposalDetail, setProposalDetail] = useState(null);
  const [inboxBusy, setInboxBusy] = useState(false);
  const [capabilityCatalog, setCapabilityCatalog] = useState(null);
  const [languageServers, setLanguageServers] = useState([]);
  const [lspSession, setLspSession] = useState(null);
  const lspSessionRef=useRef(null);
  const [lspBusy, setLspBusy] = useState(false);
  const lspBusyRef=useRef(false);
  const lspLifecycleEpoch=useRef(0);
  const lspSyncSerial=useRef(0);
  const lspFeatureSerial=useRef(0);
  const [lspWorker, setLspWorker] = useState({
    ready: false, code: 'LSP_PROTECTED_WORKER_UNVERIFIED',
    detail: 'Protected language-server admission has not been accepted.'
  });
  const [lspNativeDiagnostics, setLspNativeDiagnostics] = useState({});
  const [projectProblemsOpen, setProjectProblemsOpen] = useState(false);
  const [projectProblemSeverity, setProjectProblemSeverity] = useState('all');
  const [projectProblemSearch, setProjectProblemSearch] = useState('');
  const problemJumpNonce = useRef(0);
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
  // DEV-03.1: per-project read-only profile discovery. The only effect of
  // choosing a profile is selecting an existing approved Cargo recipe ID.
  const [projectRunProfiles,setProjectRunProfiles] = useState(null);
  const [testDiscovery,setTestDiscovery] = useState(null);
  const [testDiscoveryBusy,setTestDiscoveryBusy] = useState(false);
  const testDiscoveryTicket=useRef(0);
  const [profileDiscoveryBusy,setProfileDiscoveryBusy] = useState(false);
  const profileDiscoveryRequest=useRef(0);
  // DEV-03.2B: nonexecuting review of three native fixed interpreter
  // contracts. Never a build grant, worker handle or installed toolchain.
  const [adapterReview,setAdapterReview]=useState(null);
  const [adapterReviewBusy,setAdapterReviewBusy]=useState(false);
  const adapterReviewRequest=useRef(0);
  const [buildBusy, setBuildBusy] = useState(false);
  const [buildCancelBusy, setBuildCancelBusy] = useState(false);
  const [buildResult, setBuildResult] = useState(null);
  const [jobProgressEvents, setJobProgressEvents] = useState([]);
  const [nativeJobOutput,setNativeJobOutput] = useState(null);
  const nativeOutputCursor = useRef({projectId:null,jobId:null,sequence:0});
  const nativeOutputPending = useRef(false);
  const nativeJobOutputRef = useRef(null);
  const buildStatusRequest = useRef(0);
  const [debugStatus, setDebugStatus] = useState(null);
  const [debugTargetReview,setDebugTargetReview]=useState(null);
  const [debugTargetBusy,setDebugTargetBusy]=useState(false);
  const debugTargetRequest=useRef(0);
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
  // C7 editor task links reuse C6's native append-only development ledger.
  // No independent task store, source text, draft or automatic status change.
  const [editorTaskCatalog,setEditorTaskCatalog] = useState(null);
  const [editorTaskId,setEditorTaskId] = useState('');
  const [editorTaskBusy,setEditorTaskBusy] = useState(false);
  const editorTaskRequest = useRef(0);
  const [taskSourceChecks,setTaskSourceChecks] = useState({});
  const taskSourceCheckRequest = useRef(0);
  const contextRequest = useRef(0);
  const checkpointRequest = useRef(0);
  const [gitStatus, setGitStatus] = useState(null);
  const [gitInventory, setGitInventory] = useState(null);
  const [gitRefsHealth, setGitRefsHealth] = useState(null);
  const [branchReviewKind, setBranchReviewKind] = useState('merge');
  const [branchReviewTarget, setBranchReviewTarget] = useState('');
  const [branchReview, setBranchReview] = useState(null);
  const [branchReviewBusy, setBranchReviewBusy] = useState(false);
  const branchReviewRequest = useRef(0);
  const [replayPlan, setReplayPlan] = useState(null);
  const [replayPlanBusy, setReplayPlanBusy] = useState(false);
  const replayPlanRequest = useRef(0);
  const [replayResult, setReplayResult] = useState(null);
  const [replayResultBusy, setReplayResultBusy] = useState(false);
  const [replayExecBusy, setReplayExecBusy] = useState(false);
  const [replayExecReceipt, setReplayExecReceipt] = useState(null);
  const [replayRecovery, setReplayRecovery] = useState(null);
  const [replayRecoveryBusy, setReplayRecoveryBusy] = useState(false);
  const replayResultRequest = useRef(0);
  const [mergePlan, setMergePlan] = useState(null);
  const [mergePlanBusy, setMergePlanBusy] = useState(false);
  const mergePlanRequest = useRef(0);
  const [mergeExecBusy, setMergeExecBusy] = useState(false);
  const [mergeExecReceipt, setMergeExecReceipt] = useState(null);
  const mergeExecRequest = useRef(0);
  const [mergeRecovery, setMergeRecovery] = useState(null);
  const [mergeRecoveryBusy, setMergeRecoveryBusy] = useState(false);
  const [divergentResult, setDivergentResult] = useState(null);
  const [divergentBusy, setDivergentBusy] = useState(false);
  const [divergentExecBusy, setDivergentExecBusy] = useState(false);
  const [divergentReceipt, setDivergentReceipt] = useState(null);
  const [divergentRecovery, setDivergentRecovery] = useState(null);
  const [divergentRecoveryBusy, setDivergentRecoveryBusy] = useState(false);
  const divergentRequest = useRef(0);
  const [nativeConflicts, setNativeConflicts] = useState(null);
  const [nativeConflictsBusy, setNativeConflictsBusy] = useState(false);
  const nativeConflictsRequest = useRef(0);
  const [conflictDraft, setConflictDraft] = useState(null);
  const [conflictDraftMode, setConflictDraftMode] = useState('manual');
  const [conflictReview, setConflictReview] = useState(null);
  const [conflictReviewBusy, setConflictReviewBusy] = useState(false);
  const [conflictApplyBusy, setConflictApplyBusy] = useState(false);
  const [conflictApplyReceipt, setConflictApplyReceipt] = useState(null);
  const [conflictRecovery, setConflictRecovery] = useState(null);
  const [conflictRecoveryBusy, setConflictRecoveryBusy] = useState(false);
  const conflictRequest = useRef(0);
  const [gitFileHistory, setGitFileHistory] = useState(null);
  const [gitFilePath, setGitFilePath] = useState('');
  const [gitFileBusy, setGitFileBusy] = useState(false);
  const gitSnapshotRequest = useRef(0);
  const [gitBusy, setGitBusy] = useState(false);
  const [gitDiffScope, setGitDiffScope] = useState('worktree');
  const [gitDiff, setGitDiff] = useState(null);
  const [gitHistory, setGitHistory] = useState(null);
  const [gitEvidenceBusy, setGitEvidenceBusy] = useState(false);
  const gitEvidenceRequest = useRef(0);
  const [githubBusy, setGithubBusy] = useState(false);
  const [githubComparison, setGithubComparison] = useState(null);
  const [githubPulls, setGithubPulls] = useState(null);
  const [githubPullsBusy, setGithubPullsBusy] = useState(false);
  const githubPullsRequest = useRef(0);
  const [exactCi, setExactCi] = useState(null);
  const [exactCiBusy, setExactCiBusy] = useState(false);
  const exactCiRequest = useRef(0);
  const [snapshotBusy, setSnapshotBusy] = useState(false);
  const [resourceProbe, setResourceProbe] = useState(null);
  const [resourceProbeBusy, setResourceProbeBusy] = useState(false);
  const [inventoryComparison, setInventoryComparison] = useState(null);
  // CodeMirror EditorState is retained for each tab, including its undo stack.
  const editorStates = useRef(new Map());
  const bufferIdentities = useRef(new Map());
  const draftHeadByTab = useRef(new Map());
  const draftAutoPreserve = useRef(new Set());
  const draftTimers = useRef(new Map());
  const draftInFlight = useRef(new Set());
  const draftLookupVersion = useRef(new Map());
  const tabsRef = useRef([]);
  // In-memory navigation only. Draft text/Undo remain owned by original tabs.
  // No localStorage, OS-path, background worker or implicit native file reopen.
  const projectEditorViews = useRef(new Map());
  tabsRef.current = tabs;
  const paneSnapshotRef = useRef(null);
  paneSnapshotRef.current = {primaryKey:activeTabKey,
    secondaryKey:secondaryTabKey,splitOpen:splitEditorOpen,tabWindowStart};
  function resetBufferIdentity(key, projectId, relativePath, savedRevision) {
    const identity = newBufferIdentity(projectId, relativePath,
      savedRevision, crypto.randomUUID());
    bufferIdentities.current.set(key, identity);
    return identity;
  }
  function onEditorSelection(key, state) {
    editorStates.current.set(key,state);
    const position=state.selection.main.head;
    const line=state.doc.lineAt(position);
    setCursorPositions(previous=>{
      const next={line:line.number-1,character:position-line.from};
      const prior=previous[key];
      return prior?.line===next.line&&prior?.character===next.character
        ? previous : {...previous,[key]:next};
    });
  }
  function clearPrivateDraftSession(key) {
    const timer = draftTimers.current.get(key);
    if (timer) clearTimeout(timer);
    draftTimers.current.delete(key);
    draftAutoPreserve.current.delete(key);
    draftHeadByTab.current.delete(key);
    bufferIdentities.current.delete(key);
    draftLookupVersion.current.set(key, (draftLookupVersion.current.get(key) || 0) + 1);
    setDraftRecoveryByTab(previous => {
      const next = { ...previous };
      delete next[key];
      return next;
    });
  }

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
  const selectedMergeTabRef = useRef(null);
  selectedMergeTabRef.current = activeTabKey;
  const editorVisibleRef=useRef(false);
  editorVisibleRef.current=section==='explorer';

  useEffect(() => {
    // Filename lookup is an observation scoped to one active project only.
    // A new project discards pending results and untrusted old paths.
    quickOpenSerial.current += 1;
    quickOpenBusyRef.current = false;
    setQuickOpenBusy(false);
    setQuickOpenResults(null);
    setQuickOpenQuery('');
    setDirectoryPageBusy(false);
  }, [activeProject?.projectId]);

  useEffect(() => {
    // Load only a navigation hint for the active registered project. No
    // sources are opened and no private draft recovery is automatic.
    const projectId=activeProject?.projectId;
    const serial=++workspaceLayoutReadVersion.current;
    setRememberedLayout(null);
    setWorkspaceRecoveryReport(null);
    if (!projectId) return () => {workspaceLayoutReadVersion.current++;};
    void invoke('ide_workspace_layout_read',{projectId}).then(layout=>{
      if (serial!==workspaceLayoutReadVersion.current ||
          activeProjectIdRef.current!==projectId) return;
      setRememberedLayout(layout
        ? checkedRememberedWorkspace(layout,projectId) : null);
    }).catch(error=>{
      if (serial===workspaceLayoutReadVersion.current &&
          activeProjectIdRef.current===projectId)
        setProblem('Remembered tab layout unavailable: '+String(error));
    });
    return () => { workspaceLayoutReadVersion.current++; };
  },[activeProject?.projectId]);

  useEffect(() => {
    // Invalidate in-flight project evidence when the selected project changes.
    projectSearchGeneration.current += 1;
    setProjectSearchResults(null);
    setProjectSearchBusy(false);
    // No source revision assessment follows a different registered project.
    diskAuditSerial.current += 1;
    setDiskAuditReport(null);
    setDiskComparison(null);
    setDiskAuditBusy(false);
    mergeSerial.current += 1;
    setMergeReview(null);
    setMergeChoices({});
    setMergeApplyRequest(null);
    setMergeBusy(false);
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
    profileDiscoveryRequest.current += 1;
    adapterReviewRequest.current += 1;
    setProjectRunProfiles(null);
    setProfileDiscoveryBusy(false);
    packageInspectionRequest.current += 1;
    setPackageInspection(null);
    setPackageInspectionBusy(false);
    packagePreviewRequest.current += 1;
    setPackagePreview(null);
    setPackageRevalidated(null);
    setPackagePreviewBusy(false);
    setPackageRunBusy(false);
    setPackageRunResult(null);
    setAdapterReview(null);
    setAdapterReviewBusy(false);
    setBuildResult(null);
    setBuildBusy(false);
    setBuildCancelBusy(false);
    setDebugStatus(null);
    setDebugBusy(false);
    setDebugResult(null);
    setGitStatus(null);
    setGitInventory(null);
    setGitRefsHealth(null);
    branchReviewRequest.current += 1;
    setBranchReview(null);
    setBranchReviewBusy(false);
    setBranchReviewTarget('');
    replayPlanRequest.current += 1;
    setReplayPlan(null);
    setReplayPlanBusy(false);
    replayResultRequest.current += 1;
    setReplayResult(null);
    setReplayResultBusy(false);
    setReplayExecBusy(false);
    setReplayExecReceipt(null);
    setReplayRecovery(null);
    setReplayRecoveryBusy(false);
    mergePlanRequest.current += 1;
    setMergePlan(null);
    setMergePlanBusy(false);
    mergeExecRequest.current += 1;
    setMergeExecBusy(false);
    setMergeExecReceipt(null);
    setMergeRecovery(null);
    setMergeRecoveryBusy(false);
    divergentRequest.current += 1;
    setDivergentResult(null);
    setDivergentBusy(false);
    setDivergentExecBusy(false);
    setDivergentReceipt(null);
    setDivergentRecovery(null);
    setDivergentRecoveryBusy(false);
    nativeConflictsRequest.current += 1;
    setNativeConflicts(null);
    conflictRequest.current += 1;
    setConflictReview(null);
    setConflictReviewBusy(false);
    setConflictApplyReceipt(null);
    setConflictRecovery(null);
    setConflictDraft(null);
    setConflictDraftMode('manual');
    setNativeConflictsBusy(false);
    setGitFileHistory(null);
    setGitFileBusy(false);
    gitSnapshotRequest.current += 1;
    gitBatchRequest.current += 1;
    setGitBatchReview(null);
    setGitBatchBusy(false);
    gitHunkRequest.current += 1;
    setGitHunkPlan(null);
    setGitSelectedHunkIds([]);
    setGitHunkSelection(null);
    setGitHunkBusy(false);
    setGitPartialStageBusy(false);
    setGitPartialStageReceipt(null);
    setGitPartialRecovery(null);
    setGitPartialRecoveryBusy(false);
    setReviewedCommitPlan(null);
    setReviewedCommitReceipt(null);
    setReviewedCommitBusy(false);
    setReviewedCommitRecovery(null);
    setReviewedCommitRecoveryBusy(false);
    setGithubComparison(null);
    githubPullsRequest.current += 1;
    setGithubPulls(null);
    setGithubPullsBusy(false);
    exactCiRequest.current += 1;
    setExactCi(null);
    setExactCiBusy(false);
    taskRequest.current += 1;
    setTaskBusy(false);
    editorTaskRequest.current += 1;
    taskSourceCheckRequest.current += 1;
    setEditorTaskCatalog(null);
    setEditorTaskId('');
    setEditorTaskBusy(false);
    setTaskSourceChecks({});
    setTaskIdInput('');
    setTaskTitleInput('');
    setTaskStateInput('in_progress');
    setTaskNextActionInput('');
    setTaskRequiredGates(['source_review', 'exact_head_ci']);
    // Invalidate any provider data from the previous workspace. No cross-
    // project response may be shown or applied to the newly selected editor.
    setLspNativeDiagnostics({});
    // B7: project switch restores its own pane selection in openDirectory.
    // Do not reset that restored split view in this asynchronous effect.
    setProjectProblemsOpen(false);
    setProjectProblemSearch('');
    setLspCompletions(null);
    setLspHover(null);
    setLspJumpRequest(null);
    setLspCompletionApplyRequest(null);
  }, [activeProject?.projectId]);

  // A selected key from another project must never supply LSP, editor, Save
  // or revision state while React is switching registered workspaces.
  const activeTab = useMemo(() => tabs.find(tab => tab.key === activeTabKey &&
    tab.projectId === activeProject?.projectId) || null,
    [tabs,activeTabKey,activeProject?.projectId]);
  const currentLanguageId = languageForPath(activeTab?.relativePath);
  const lspAvailableForTab = !!lspWorker?.ready && !!activeTab &&
    !!currentLanguageId && Array.isArray(lspWorker.supportedLanguages) &&
    lspWorker.supportedLanguages.includes(currentLanguageId);
  const lspCapabilities = languageServiceCapabilities(currentLanguageId,lspWorker);
  const lspProviderForTab = lspSession?.projectId===activeTab?.projectId &&
    providerServesLanguage(lspSession.providerId,currentLanguageId)
    ? lspSession.providerId : (lspCapabilities?.provider || 'unavailable language provider');
  const lspMayAddCurrentFile = !!activeTab && !!lspSession &&
    !lspSession.cleanupRequired &&
    lspSession.projectId===activeTab.projectId &&
    providerServesLanguage(lspSession.providerId,currentLanguageId) &&
    !lspSession.members?.[activeTab.key] &&
    Object.keys(lspSession.members||{}).length<16 && lspAvailableForTab;
  const lspSupportsNavigation = languageMethodAvailable(currentLanguageId,
    'definition',lspWorker) &&
    (currentLanguageId!=='python' || !lspSession ||
      lspSession.providerId==='pyright');
  const lspRunningForTab = !!activeTab && !lspSession?.cleanupRequired &&
    sourceMatchesSession(
      captureLiveLspSource(activeTab,
        bufferIdentities.current.get(activeTab.key),
        editorStates.current.get(activeTab.key),
        activeProject?.projectId),lspSession);

  const activeLspDiagnostics = useMemo(() => activeTab
    ? visibleDiagnostics(activeTab, lspNativeDiagnostics[activeTab.key]) : [],
    [activeTab, lspNativeDiagnostics]);
  const activeLspCompletions = useMemo(() => activeTab &&
      lspCompletions?.tabKey===activeTab.key &&
      stillCurrentLspSource(lspCompletions.source,activeTab,
        bufferIdentities.current.get(activeTab.key),
        editorStates.current.get(activeTab.key),activeProject?.projectId)
    ? visibleCompletions(activeTab,lspCompletions.response) : [],
    [activeTab,lspCompletions,activeProject?.projectId]);
  const activeLspHover = useMemo(() => activeTab &&
      lspHover?.tabKey===activeTab.key &&
      stillCurrentLspSource(lspHover.source,activeTab,
        bufferIdentities.current.get(activeTab.key),
        editorStates.current.get(activeTab.key),activeProject?.projectId)
    ? visibleHover(activeTab,lspHover.response) : '',
    [activeTab,lspHover,activeProject?.projectId]);

  const projectTabs = useMemo(() => tabs.filter(tab =>
    tab.projectId === activeProject?.projectId),[tabs,activeProject?.projectId]);
  const visibleEditorTabs = useMemo(() => tabWindow(projectTabs,tabWindowStart),
    [projectTabs,tabWindowStart]);
  const diskAuditCounts = useMemo(()=>{
    if (diskAuditReport?.projectId!==activeProject?.projectId)
      return {changed:0,current:0,unavailable:0,stale:0,unsaved:0};
    const totals={changed:0,current:0,unavailable:0,stale:0,unsaved:0};
    for(const row of diskAuditReport.rows){
      if (row.status==='changed') totals.changed+=1;
      else if (row.status==='current') totals.current+=1;
      else if (row.status==='unavailable') totals.unavailable+=1;
      else totals.stale+=1;
      if(row.liveUnsaved) totals.unsaved+=1;
    }
    return totals;
  },[diskAuditReport,activeProject?.projectId]);
  const currentJobEvents=useMemo(()=>
    visibleJobProgress(jobProgressEvents,activeProject?.projectId),
    [jobProgressEvents,activeProject?.projectId]);
  const visibleTree = visibleTreeEntries(listing?.entries,visibleTreeCount);
  const projectTabCount = projectTabs.length;
  const workingSetNotice = projectTabCount + ' / ' + MAX_OPEN_TABS_PER_PROJECT +
    ' tabs in this project · ' + tabs.length + ' / ' + MAX_OPEN_TABS_TOTAL +
    ' across workspaces. Clean background tabs can be closed manually.';
  useEffect(() => {
    // An external source hit, diagnostic or programmatic tab selection must
    // make that tab reachable. Manual paging itself never changes selection.
    setTabWindowStart(previous=>centerSelectedTab(projectTabs,activeTabKey,previous));
  },[activeTabKey,projectTabs.length,activeProject?.projectId]);
  useEffect(()=>{
    setVisibleTreeCount(INITIAL_VISIBLE_TREE_ENTRIES);
  },[directory,listing?.projectId,listing?.relativeDir]);
  // A CodeMirror EditorState is owned by one tab; mounting the same tab in
  // both panes is forbidden, including during project switches and close.
  const secondaryTab = splitEditorOpen ? secondaryEditor(
    projectTabs, activeProject?.projectId, activeTab?.key, secondaryTabKey) : null;
  const secondaryLspDiagnostics = useMemo(() =>
    secondaryTab ? visibleDiagnostics(
      secondaryTab,lspNativeDiagnostics[secondaryTab.key]) : [],
    [secondaryTab,lspNativeDiagnostics]);
  useEffect(()=>{
    focusedEditorPaneRef.current='primary';
    setFocusedEditorPane('primary');
    setEditorFocusRequest(null);
  },[activeProject?.projectId]);

  function requestEditorFocus(pane='primary') {
    const target=pane==='secondary' && splitEditorOpen ? secondaryTab : activeTab;
    const desired=pane==='secondary' && splitEditorOpen ? 'secondary' : 'primary';
    if (!target || target.projectId!==activeProjectIdRef.current ||
        !activeProject?.projectId || workspaceTransitionRef.current) return;
    focusedEditorPaneRef.current=desired;
    setFocusedEditorPane(desired);
    setSection('explorer');
    setEditorFocusRequest({pane:desired,projectId:target.projectId,
      tabIdentity:target.key+':'+target.generation,nonce:++focusNonce.current});
  }
  function onEditorFocused(pane,identity) {
    const target=pane==='secondary' ? secondaryTab : activeTab;
    if (!target || target.projectId!==activeProjectIdRef.current ||
        identity!==target.key+':'+target.generation) return;
    focusedEditorPaneRef.current=pane;
    setFocusedEditorPane(pane);
  }
  useEffect(()=>{
    if (!projectProblemsOpen || !problemsFocusRequest) return;
    projectProblemsPanelRef.current?.focus();
  },[projectProblemsOpen,problemsFocusRequest]);
  function showProjectProblems() {
    if (!activeProject?.projectId) return;
    setProjectProblemsOpen(true);
    setProblemsFocusRequest(previous=>previous+1);
    setSection('explorer');
  }
  useEffect(()=>{
    const target=rovingTabFocusRef.current;
    if (!target || target.projectId!==activeProject?.projectId ||
        target.key!==activeTabKey) return;
    const element=[...document.querySelectorAll(
      'button[role="tab"][data-ide-tab-key]')]
      .find(node=>node.getAttribute('data-ide-tab-key')===target.key);
    if (element) {
      rovingTabFocusRef.current=null;
      element.focus();
    }
  },[activeTabKey,visibleEditorTabs.start,activeProject?.projectId]);
  function selectKeyboardTab(event) {
    const origin=event.target?.closest?.('button[role="tab"][data-ide-tab-key]');
    if (!origin || !projectTabs.length || !activeProject?.projectId ||
        !['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) return;
    const current=projectTabs.findIndex(tab=>
      tab.key===origin.getAttribute('data-ide-tab-key'));
    if(current<0) return;
    event.preventDefault();
    let index=current;
    if(event.key==='ArrowLeft') index=(current-1+projectTabs.length)%projectTabs.length;
    if(event.key==='ArrowRight') index=(current+1)%projectTabs.length;
    if(event.key==='Home') index=0;
    if(event.key==='End') index=projectTabs.length-1;
    const next=projectTabs[index];
    if (!next || next.projectId!==activeProject.projectId) return;
    rovingTabFocusRef.current={key:next.key,projectId:next.projectId};
    selectPrimaryEditorTab(next.key);
  }
  function selectPrimaryEditorTab(key) {
    if (mergeBusy || mergeApplyRequest || !activeProject?.projectId) return;
    const choice=planProjectPaneSelection(tabsRef.current,
      activeProject.projectId,activeTabKey,secondaryTabKey,splitEditorOpen,key);
    if (!choice.allowed) {
      setProblem('Cannot select a tab outside the current registered project.');
      return;
    }
    if (key!==activeTabKey) {
      setMergeReview(null);
      setMergeChoices({});
    }
    setTabWindowStart(previous=>centerSelectedTab(projectTabs,key,previous));
    setSecondaryTabKey(choice.secondaryKey);
    setActiveTabKey(choice.primaryKey);
    focusedEditorPaneRef.current='primary';
    setFocusedEditorPane('primary');
  }
  function toggleSplitEditor() {
    if (!activeProject?.projectId || !activeTab) return;
    if (splitEditorOpen) {
      setSplitEditorOpen(false);
      setSecondaryTabKey(null);
      focusedEditorPaneRef.current='primary';
      setFocusedEditorPane('primary');
      return;
    }
    setSecondaryTabKey(initialSecondaryKey(projectTabs,
      activeProject.projectId,activeTabKey));
    setSplitEditorOpen(true);
  }
  const problemsForProject = useMemo(() => projectProblems(
    projectTabs, lspNativeDiagnostics, activeProject?.projectId || 'no-active-project',
    { severity: projectProblemSeverity, query: projectProblemSearch }
  ), [tabs, activeProject?.projectId, lspNativeDiagnostics,
    projectProblemSeverity, projectProblemSearch]);
  function navigateProjectProblem(item) {
    // Cross-file navigation only among already-open, project-registered
    // CodeMirror tabs and only at this *current* verified source revision.
    const tab = tabs.find(candidate => candidate.key === item.tabKey &&
      candidate.projectId === activeProjectIdRef.current);
    if (!tab || !stillCurrentProblem(tab, lspNativeDiagnostics[tab.key], item)) {
      setProblem('Diagnostic became stale. Refresh the protected language session.');
      return;
    }
    selectPrimaryEditorTab(tab.key);
    setSection('explorer');
    setProjectProblemsOpen(false);
    setLspJumpRequest({
      from: item.from, to: item.to,
      tabIdentity: tab.key + ':' + tab.generation,
      nonce: ++problemJumpNonce.current,
    });
  }
  const anyDirty = tabs.some(tab => tab.dirty);
  const isExplorer = section === 'explorer';
  const isProjectSearch = section === 'project-search';
  const isProposals = section === 'proposals';
  const isCapabilities = section === 'capabilities';
  const isLanguages = section === 'languages';
  const isContinuity = section === 'continuity';
  const isJobs = section === 'tasks';
  useEffect(() => {
    const projectId = activeProject?.projectId;
    if ((section !== 'tasks' && section !== 'packages') || !projectId) return;
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
          {
            setBuildStatus(value);
            setJobProgressEvents(previous =>
              ingestJobProgress(previous, projectId, value));
            // A job ID originates only in the native broker status.
            // Never pass a path, stream label or executable from the WebView.
            const activeJobId = value.current?.projectId === projectId
              ? value.current?.jobId : null;
            // After an error or verified cancellation, the native in-memory
            // window can outlive current. Only a matching *durable journal*
            // identity may select that bounded advisory output. Restart does
            // not recover or replay the log; no execution authority changes.
            const lastOutputId = typeof value.lastOutputJobId === 'string' &&
              value.journal.some(entry =>
                entry.projectId === projectId &&
                entry.jobId === value.lastOutputJobId &&
                /^[a-f0-9]{64}$/.test(entry.planSha256))
              ? value.lastOutputJobId : null;
            const jobId = activeJobId || lastOutputId;
            if (jobId && !nativeOutputPending.current) {
              const cursor=nativeOutputCursor.current;
              const after=cursor.projectId===projectId &&
                cursor.jobId===jobId ? cursor.sequence : 0;
              nativeOutputPending.current=true;
              try {
                const page=await invoke('ide_build_read_output',
                  {projectId,jobId,after});
                if (alive && activeProjectIdRef.current===projectId) {
                  const latest=nativeOutputCursor.current;
                  if (latest.projectId!==projectId ||
                      latest.jobId!==jobId || latest.sequence===after) {
                    const next=acceptNativeOutput(
                      latest.projectId===projectId && latest.jobId===jobId
                        ? nativeJobOutputRef.current : null,
                      projectId,jobId,page);
                    if (next) {
                      nativeOutputCursor.current={projectId,jobId,
                        sequence:next.lastSequence};
                      nativeJobOutputRef.current=next;
                      setNativeJobOutput(next);
                    }
                  }
                }
              } catch {
                // Status and cancellation remain authoritative even when
                // live output transport is temporarily unavailable.
              } finally {
                nativeOutputPending.current=false;
              }
            }
          }
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
  }, [section, activeProject?.projectId]);
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
  const activeEditorState = activeTab ? editorStates.current.get(activeTab.key) : null;
  // CodeMirror positions are UTF-16 in its canonical LF Text tree; serialized
  // source can use CRLF. Search and replace offsets must refer to the tree.
  const matchPlan = locateMatches(activeEditorState?.doc.toString() ?? '',
    findQuery, caseSensitiveFind);
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
    const searchText = editorStates.current.get(activeTab.key)?.doc.toString();
    if (searchText === undefined) return;
    const planned = planReplacement(searchText, findQuery, replaceText, {
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


  async function copyDraftReference(tab) {
    if (!tab || draftReferencePending) return;
    const state = editorStates.current.get(tab.key);
    const identity = bufferIdentities.current.get(tab.key);
    if (!state || !identity || state.sliceDoc() !== tab.draft) {
      setProblem('This editor draft changed; choose the tab and try again.');
      return;
    }
    const selection = state.selection.main;
    const from = selection.empty ? state.doc.lineAt(selection.head).from : selection.from;
    const to = selection.empty ? state.doc.lineAt(selection.head).to : selection.to;
    let snapshot;
    try { snapshot = captureBufferSpan(identity,state,from,to); }
    catch (error) { setProblem('Cannot capture this draft span: ' + String(error)); return; }
    setDraftReferencePending(true);
    setProblem('');
    try {
      const digest=await crypto.subtle.digest('SHA-256',
        new TextEncoder().encode(state.sliceDoc()));
      const hex=Array.from(new Uint8Array(digest),
        byte => byte.toString(16).padStart(2,'0')).join('');
      const current=bufferIdentities.current.get(tab.key);
      const currentState=editorStates.current.get(tab.key);
      if (resolveBufferSpan(snapshot,current,currentState).status !== 'current' ||
          currentState.sliceDoc() !== state.sliceDoc())
        throw Error('BUFFER_CHANGED_DURING_REFERENCE_COPY');
      await navigator.clipboard.writeText(formatBufferReference(snapshot,hex));
      setHint('Copied a session-only draft reference. For cross-restart references use Preserve draft.');
    } catch (error) {
      setProblem('Could not copy draft reference: '+String(error));
    } finally { setDraftReferencePending(false); }
  }

  async function checkExistingDraft(tab) {
    const key=tab.key;
    const identity=bufferIdentities.current.get(key);
    const lookup=(draftLookupVersion.current.get(key)||0)+1;
    draftLookupVersion.current.set(key,lookup);
    try {
      const record=await invoke('ide_draft_latest',{
        projectId:tab.projectId,relativePath:tab.relativePath
      });
      if (draftLookupVersion.current.get(key)!==lookup ||
          bufferIdentities.current.get(key)?.bufferId!==identity?.bufferId) return;
      if (record) {
        draftHeadByTab.current.set(key,record.checkpointId);
        setDraftRecoveryByTab(previous=>({...previous,[key]:record}));
        setHint('Preserved draft available for '+tab.relativePath+
          '. Recover it explicitly; saved source was not changed.');
      }
    } catch (error) {
      if (draftLookupVersion.current.get(key)===lookup) {
        setProblem('Draft recovery inspection unavailable: '+String(error));
      }
    }
  }

  async function preserveDraft(tab,copyPersistent=false) {
    if (!tab || draftInFlight.current.has(tab.key)) return;
    const identity=bufferIdentities.current.get(tab.key);
    const state=editorStates.current.get(tab.key);
    if (!identity || !state || state.sliceDoc().length>524288 ||
        !tabsRef.current.some(row=>row.key===tab.key) ||
        identity.projectId!==tab.projectId || identity.relativePath!==tab.relativePath) {
      setProblem('Draft is unavailable or exceeds private 512 KiB recovery limit.');
      return;
    }
    // Capture immutable editor revision before native async write.
    const text=state.sliceDoc();
    const input={
      projectId:identity.projectId,relativePath:identity.relativePath,
      bufferId:identity.bufferId,bufferRevision:identity.revision,
      baseDiskRevision:identity.savedRevision,text,
      expectedHead:draftHeadByTab.current.get(tab.key)||null,
    };
    draftInFlight.current.add(tab.key);
    if (copyPersistent) setDraftReferencePending(true);
    try {
      const saved=await invoke('ide_draft_checkpoint',{input});
      if (bufferIdentities.current.get(tab.key)?.bufferId !== identity.bufferId)
        return; // Old editor replaced: do not bind its checkpoint to new tab.
      draftHeadByTab.current.set(tab.key,saved.checkpointId);
      setDraftRecoveryByTab(previous=>({...previous,[tab.key]:saved}));
      draftAutoPreserve.current.add(tab.key);
      if (copyPersistent) {
        // A permanent reference names *exactly* the persisted immutable bytes.
        await navigator.clipboard.writeText(formatPersistentDraftReference(saved));
        setHint('Copied durable IDE-private draft reference. Project file was not saved.');
      } else {
        setHint('Draft checkpoint preserved privately; source file unchanged.');
      }
    } catch (error) {
      if (String(error).includes('DRAFT_HISTORY_FULL')) {
        draftAutoPreserve.current.delete(tab.key);
        setProblem('Private draft history is full. Earlier durable references are safe; '+
          'continuous checkpointing paused until you explicitly manage the recovery history.');
      } else {
        setProblem('Draft recovery checkpoint failed: '+String(error));
      }
    } finally {
      draftInFlight.current.delete(tab.key);
      if (copyPersistent) setDraftReferencePending(false);
      const current=bufferIdentities.current.get(tab.key);
      if (current?.bufferId===identity.bufferId &&
          current.revision>identity.revision &&
          draftAutoPreserve.current.has(tab.key))
        scheduleDraftPreserve(tab.key);
    }
  }

  function scheduleDraftPreserve(key) {
    const prior=draftTimers.current.get(key);
    if (prior) clearTimeout(prior);
    if (!draftAutoPreserve.current.has(key)) return;
    draftTimers.current.set(key,setTimeout(()=>{
      draftTimers.current.delete(key);
      const tab=tabsRef.current.find(row=>row.key===key);
      if (tab && tab.dirty && !draftInFlight.current.has(key))
        void preserveDraft(tab,false);
      else if (tab && tab.dirty && draftInFlight.current.has(key))
        scheduleDraftPreserve(key);
    },750));
  }

  async function restorePreservedDraft(tab,referenceOverride=null) {
    const record=referenceOverride || draftRecoveryByTab[tab?.key];
    if (!tab || !record || draftRecoveryBusy || savingKey ||
        savingKeyRef.current || workspaceLayoutOp.current ||
        workspaceTransitionRef.current ||
        activeProjectIdRef.current!==tab.projectId) return;
    const original=captureLiveEditorIntent(tab,
      bufferIdentities.current.get(tab.key),
      editorStates.current.get(tab.key),tab.projectId);
    if (original.status!=='current') {
      setProblem('Cannot recover: current editor identity or saved base is stale.');
      return;
    }
    let claimed;
    try {
      claimed=checkedPrivateDraftHead(record,tab.projectId,tab.relativePath);
      if (!claimed) throw Error('NO_PRIVATE_CHECKPOINT');
    } catch(error) {
      setProblem('Private draft identity is unavailable: '+String(error));
      return;
    }
    if (original.dirty && !window.confirm(
      'Replace this CURRENT unsaved editor draft with the private checkpoint? '+
      'Project files remain untouched. Preserve your newer edits first if needed.'))
      return;
    const token=Symbol('ide-int01d-explicit-private-draft-recovery');
    workspaceTransitionRef.current=token;
    const epoch=projectNavigationEpoch.current;
    setDraftRecoveryBusy(true);
    setProblem('');
    try {
      const [disk,recovered]=await Promise.all([
        invoke('ide_preview_source_file',{projectId:tab.projectId,
          relativePath:tab.relativePath}),
        invoke('ide_draft_load',{projectId:tab.projectId,
          relativePath:tab.relativePath,checkpointId:claimed.checkpointId}),
      ]);
      if (workspaceTransitionRef.current!==token ||
          projectNavigationEpoch.current!==epoch ||
          !stillCurrentEditorIntent(original,
            tabsRef.current.find(item=>item.key===tab.key),
            bufferIdentities.current.get(tab.key),
            editorStates.current.get(tab.key),activeProjectIdRef.current))
        throw Error('INT01D_EDITOR_CHANGED_DURING_PRIVATE_RECOVERY');
      if (disk?.projectId!==tab.projectId ||
          disk.relativePath!==tab.relativePath ||
          !/^[a-f0-9]{64}$/i.test(disk.revision||'') ||
          typeof disk.text!=='string' ||
          typeof recovered?.text!=='string') {
        throw Error('INT01D_UNVERIFIED_RECOVERY_SOURCE');
      }
      const checked=checkedPrivateDraftHead(recovered,tab.projectId,
        tab.relativePath);
      if (checked.checkpointId!==claimed.checkpointId ||
          checked.draftSha256!==claimed.draftSha256 ||
          checked.baseDiskRevision!==claimed.baseDiskRevision ||
          checked.bufferId!==claimed.bufferId ||
          checked.bufferRevision!==claimed.bufferRevision)
        throw Error('INT01D_PRIVATE_CHECKPOINT_CHANGED');
      if ((disk.revision!==original.savedRevision ||
          claimed.baseDiskRevision!==disk.revision) && !window.confirm(
        'The saved file or checkpoint base differs from this editor session. '+
        'Recover the checkpoint only as an UNSAVED buffer against the '+
        'current disk version? Review the differences before any Save.'))
        return;
      if (workspaceTransitionRef.current!==token ||
          projectNavigationEpoch.current!==epoch ||
          !stillCurrentEditorIntent(original,
            tabsRef.current.find(item=>item.key===tab.key),
            bufferIdentities.current.get(tab.key),
            editorStates.current.get(tab.key),activeProjectIdRef.current))
        throw Error('INT01D_EDITOR_CHANGED_AFTER_RECOVERY_CONSENT');
      if (lspSessionRef.current?.members?.[tab.key] &&
          !await closeLspDocument(tab.key))
        throw Error('LSP_DOCUMENT_CLEANUP_REQUIRED');
      if (workspaceTransitionRef.current!==token ||
          projectNavigationEpoch.current!==epoch ||
          !stillCurrentEditorIntent(original,
            tabsRef.current.find(item=>item.key===tab.key),
            bufferIdentities.current.get(tab.key),
            editorStates.current.get(tab.key),activeProjectIdRef.current))
        throw Error('INT01D_EDITOR_CHANGED_DURING_LSP_CLEANUP');
      // New editor session, new disk base identity, no project file write.
      clearPrivateDraftSession(tab.key);
      resetBufferIdentity(tab.key,tab.projectId,tab.relativePath,disk.revision);
      draftHeadByTab.current.set(tab.key,checked.checkpointId);
      draftAutoPreserve.current.add(tab.key);
      setDraftRecoveryByTab(previous=>({...previous,[tab.key]:checked}));
      editorStates.current.set(tab.key,createEditorState(
        recovered.text,tab.relativePath,
        (state,draft)=>onEditorChange(tab.key,state,draft),
        state=>onEditorSelection(tab.key,state)));
      setTabs(previous=>previous.map(row=>row.key===tab.key?{
        ...row,revision:disk.revision,savedText:disk.text,draft:recovered.text,
        dirty:recovered.text!==disk.text,generation:row.generation+1,
        bufferRevision:(row.bufferRevision??0)+1,
      }:row));
      setWorkspaceRecoveryReport(previous=>previous?.projectId===tab.projectId
        ? {...previous,rows:previous.rows.map(row=>row.key===tab.key
          ? {...row,draftStatus:'recovered_as_unsaved_editor'}:row)}
        : previous);
      setHint('Recovered private checkpoint into unsaved editor only. '+
        'Review the draft before explicitly saving.');
    } catch(error) {
      if (workspaceTransitionRef.current===token)
        setProblem('Unable to recover private draft: '+String(error));
    } finally {
      if (workspaceTransitionRef.current===token)
        workspaceTransitionRef.current=null;
      setDraftRecoveryBusy(false);
    }
  }

  function recoverDraftByReference(tab) {
    if (!tab || draftRecoveryBusy || savingKey) return;
    try {
      const parsed=parsePersistentDraftReference(persistentReferenceInput.trim());
      if (parsed.projectId!==tab.projectId || parsed.relativePath!==tab.relativePath)
        throw Error('REFERENCE_BELONGS_TO_DIFFERENT_PROJECT_OR_FILE');
      void restorePreservedDraft(tab,parsed);
    } catch(error) {
      setProblem('Cannot recover referenced draft: '+String(error));
    }
  }

  async function discardPreservedDraft(tab) {
    const record=draftRecoveryByTab[tab?.key];
    if (!tab || !record || draftRecoveryBusy ||
        !window.confirm('Delete private recovery history for this file? Project source will not change.'))
      return;
    setDraftRecoveryBusy(true);
    try {
      await invoke('ide_draft_discard',{projectId:tab.projectId,
        relativePath:tab.relativePath,expectedHead:record.checkpointId});
      draftHeadByTab.current.delete(tab.key);
      draftAutoPreserve.current.delete(tab.key);
      const timer=draftTimers.current.get(tab.key);
      if(timer)clearTimeout(timer);
      draftTimers.current.delete(tab.key);
      setDraftRecoveryByTab(previous=>({...previous,[tab.key]:null}));
      setHint('Deleted private recovery history only. Project source and current editor remain unchanged.');
    } catch(error){setProblem('Unable to discard private draft: '+String(error));}
    finally{setDraftRecoveryBusy(false);}
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

  async function inspectPackageManagers() {
    const projectId=activeProject?.projectId;
    if(!projectId) {
      setPackageInspection(null);
      return;
    }
    const ticket=++packageInspectionRequest.current;
    setPackageInspectionBusy(true);
    setProblem('');
    setPackageInspection(null);
    invalidatePackagePreview();
    try {
      // No package name, manager, command, path or permission is sent.
      const result=await invoke('ide_inspect_package_managers',{projectId});
      if(ticket !== packageInspectionRequest.current ||
         activeProjectIdRef.current!==projectId)return;
      if(!result || result.projectId!==projectId ||
         result.schemaVersion!==1 ||
         !Array.isArray(result.candidates) ||
         result.networkAccessed!==false ||
         result.processStarted!==false ||
         result.filesModified!==false ||
         result.resolutionSupported!==false ||
         result.applySupported!==false ||
         result.executionAuthorized!==false) {
        throw Error('IDE05_NONEXECUTING_NATIVE_INSPECTION_INVALID');
      }
      setPackageInspection(result);
      const available=result.candidates.find(candidate =>
        candidate.state==='candidate_needs_review');
      if(available) setPackageManagerChoice(available.managerId);
    }catch(error){
      if(ticket===packageInspectionRequest.current &&
         activeProjectIdRef.current===projectId)
        setProblem('Package manager inspection unavailable: '+String(error));
    }finally{
      if(ticket===packageInspectionRequest.current)
        setPackageInspectionBusy(false);
    }
  }

  function invalidatePackagePreview() {
    packagePreviewRequest.current += 1;
    setPackagePreview(null);
    setPackageRevalidated(null);
    setPackageRunResult(null);
    setPackagePreviewBusy(false);
  }

  async function previewPackageOperation() {
    const projectId=activeProject?.projectId;
    if(!projectId || packageInspection?.projectId!==projectId)return;
    const request={
      managerId: packageManagerChoice,
      operation: packageOperation,
      packageName: packageOperation==='sync' ? null : packageName.trim(),
      exactVersion: packageOperation==='add' ? packageExactVersion.trim() : null,
    };
    const ticket=++packagePreviewRequest.current;
    setPackagePreviewBusy(true);
    setProblem('');
    setPackagePreview(null);
    setPackageRevalidated(null);
    try {
      // Only fixed typed data, NOT a manager command, path or permission.
      const result=await invoke('ide_preview_package_operation',{projectId,request});
      if(ticket!==packagePreviewRequest.current ||
         activeProjectIdRef.current!==projectId)return;
      if(!result || result.projectId!==projectId ||
         result.schemaVersion!==1 ||
         result.request?.managerId!==request.managerId ||
         result.request?.operation!==request.operation ||
         result.evidenceFingerprint!==packageInspection.fingerprint ||
         !Array.isArray(result.proposedChanges) ||
         !Array.isArray(result.risks) ||
         !/^[a-f0-9]{64}$/.test(result.previewSha256||'') ||
         result.readyForApproval!==false ||
         result.approvalIssued!==false ||
         result.applySupported!==false ||
         result.processStarted!==false ||
         result.networkAccessed!==false ||
         result.filesModified!==false) {
        throw Error('IDE052_NOT_A_TRUSTED_NONEXECUTING_PREVIEW');
      }
      setPackagePreview(result);
    }catch(error){
      if(ticket===packagePreviewRequest.current &&
         activeProjectIdRef.current===projectId)
        setProblem('Package operation preview blocked: '+String(error));
    }finally{
      if(ticket===packagePreviewRequest.current)
        setPackagePreviewBusy(false);
    }
  }

  async function revalidatePackageOperation() {
    const projectId=activeProject?.projectId;
    const preview=packagePreview;
    if(!projectId || !preview || preview.projectId!==projectId)return;
    const ticket=++packagePreviewRequest.current;
    setPackagePreviewBusy(true);
    setPackageRevalidated(null);
    setProblem('');
    try {
      const result=await invoke('ide_revalidate_package_operation',{
        projectId, request: preview.request,
        expectedPreviewSha256: preview.previewSha256,
        expectedEvidenceFingerprint: preview.evidenceFingerprint,
      });
      if(ticket!==packagePreviewRequest.current ||
         activeProjectIdRef.current!==projectId)return;
      if(!result || result.projectId!==projectId ||
         result.schemaVersion!==1 ||
         result.previewSha256!==preview.previewSha256 ||
         result.evidenceFingerprint!==preview.evidenceFingerprint ||
         result.matchedFreshNativeEvidence!==true ||
         result.readyForApproval!==false ||
         result.authorizationIssued!==false ||
         result.processStarted!==false ||
         result.filesModified!==false) {
        throw Error('IDE052_REVALIDATION_REJECTED');
      }
      setPackageRevalidated(result);
    }catch(error){
      if(ticket===packagePreviewRequest.current &&
         activeProjectIdRef.current===projectId) {
        // A stale preview is not displayed as verified after failure.
        setPackageRevalidated(null);
        setPackagePreview(null);
        setProblem('Package operation changed or revalidation denied; inspect again: '+String(error));
      }
    }finally{
      if(ticket===packagePreviewRequest.current)
        setPackagePreviewBusy(false);
    }
  }

  async function runProtectedOfflineUvSync() {
    const projectId=activeProject?.projectId;
    const preview=packagePreview;
    const confirmed=packageRevalidated;
    const runtime=buildStatus?.multilangReady?.find(
      entry=>entry.profileId==='uv_sync_offline'
    );
    if(!projectId || preview?.projectId!==projectId ||
       preview.request?.managerId!=='uv' ||
       preview.request?.operation!=='sync' ||
       preview.readyForApproval!==false ||
       confirmed?.previewSha256!==preview.previewSha256 ||
       confirmed?.evidenceFingerprint!==preview.evidenceFingerprint ||
       confirmed?.matchedFreshNativeEvidence!==true ||
       confirmed?.authorizationIssued!==false ||
       !runtime?.ready ||
       buildStatus?.current || packageRunBusy || buildBusy)return;
    setPackageRunBusy(true);
    setPackageRunResult(null);
    setProblem('');
    try {
      // No package names, executable paths, flags or grants cross IPC.
      // Rust derives a NEW uv Sync request, sealed target and OS consent.
      const result=await invoke('ide_package_run_reviewed_uv',{
        projectId,
        expectedPreviewSha256:preview.previewSha256,
        expectedEvidenceFingerprint:preview.evidenceFingerprint,
      });
      if(activeProjectIdRef.current!==projectId)return;
      if(!result || result.projectId!==projectId ||
         result.recipeId!=='uv_sync_offline' ||
         !/^[a-f0-9]{64}$/.test(result.planSha256||'') ||
         !/^[a-f0-9]{64}$/.test(result.terminalReceiptSha256||'') ||
         result.installedAppAccepted!==false) {
        throw Error('IDE053_UNVERIFIED_PACKAGE_JOB_RESULT');
      }
      invalidatePackagePreview();
      setPackageRunResult(result);
      // Completed Job is not proof packages are in the registered project:
      // the entire private /output artifact remains separately auditable.
      setHint(result.packageOutputAudit?.recordHashesVerified === true &&
        result.packageOutputAudit?.licenseFilesVerified === true &&
        result.packageOutputAudit?.packageVersionsMatchLock === true &&
        result.packageOutputAudit?.approvedForProjectPromotion === false ?
        'Private Python artifact bytes, RECORD hashes, license files and locked versions were audited. Wheel origin and deployment remain unverified.' :
        'Package artifacts are unverified or blocked. Check the original native journal and private-output cleanup status; no project deployment occurred.');
    }catch(error){
      if(activeProjectIdRef.current===projectId)
        setProblem('Offline package operation denied or interrupted: '+String(error));
    }finally{
      setPackageRunBusy(false);
      if(activeProjectIdRef.current===projectId)
        await refreshProtectedBuildStatus();
    }
  }

  async function discardProtectedPackageOutput(entry) {
    const projectId=activeProject?.projectId;
    if(!projectId || entry?.projectId!==projectId ||
       !entry?.jobId || packageRunBusy || buildBusy ||
       buildStatus?.current ||
       !['verified_local_bytes_private_not_promoted',
         'original_package_job_requires_native_reconciliation',
         'private_output_cleanup_pending'].includes(entry.state))return;
    setPackageRunBusy(true);
    setProblem('');
    try {
      // Only the original native job ID is passed. No disk path, file
      // pattern, grant or manager argument crosses the WebView boundary.
      const result=await invoke('ide_package_discard_private_output',{
        projectId,jobId:entry.jobId,
      });
      if(activeProjectIdRef.current===projectId &&
         result==='IDE054_PRIVATE_PACKAGE_OUTPUT_DISCARDED_NO_PROJECT_MUTATION') {
        setHint('Private package environment discarded with an append-only cleanup receipt. Original project unchanged; no package manager was relaunched.');
        setPackageRunResult(null);
      }
    }catch(error){
      if(activeProjectIdRef.current===projectId)
        setProblem('Private package cleanup denied or remains pending: '+String(error));
    }finally{
      setPackageRunBusy(false);
      if(activeProjectIdRef.current===projectId)
        await refreshProtectedBuildStatus();
    }
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

  async function restoreReviewedPartialStageReceipt() {
    if (!activeProject?.projectId || reviewedCommitBusy) return;
    const projectId = activeProject.projectId;
    const ticket = gitHunkRequest.current;
    setReviewedCommitBusy(true);
    setProblem('');
    try {
      const restored = await invoke('ide_git_restore_reviewed_stage_receipt', { projectId });
      if (ticket === gitHunkRequest.current && restored.projectId === projectId) {
        setGitPartialStageReceipt(restored);
        setReviewedCommitPlan(null);
        setReviewedCommitReceipt(null);
        setHint('Previously approved partial stage was independently verified from the native journal; no commit has been authorized.');
      }
    } catch (error) {
      if (ticket === gitHunkRequest.current)
        setProblem('Unable to restore exact staged-hunk evidence: ' + String(error));
    } finally {
      if (ticket === gitHunkRequest.current) setReviewedCommitBusy(false);
    }
  }

  async function previewReviewedPartialCommit(mode) {
    if (!activeProject?.projectId || !gitPartialStageReceipt || reviewedCommitBusy ||
        gitPartialStageReceipt.projectId !== activeProject.projectId) return;
    const projectId = activeProject.projectId;
    const ticket = gitHunkRequest.current;
    setReviewedCommitPlan(null);
    setReviewedCommitReceipt(null);
    setReviewedCommitBusy(true);
    setProblem('');
    try {
      const review = await invoke('ide_git_preview_reviewed_commit', {
        stage: gitPartialStageReceipt, mode,
        message: reviewedCommitMessage,
        authorName: reviewedCommitAuthor,
        authorEmail: reviewedCommitEmail,
      });
      if (ticket === gitHunkRequest.current && review.stage.projectId === projectId)
        setReviewedCommitPlan(review);
    } catch (error) {
      if (ticket === gitHunkRequest.current)
        setProblem('Native commit or amendment review refused: ' + String(error));
    } finally {
      if (ticket === gitHunkRequest.current) setReviewedCommitBusy(false);
    }
  }

  async function submitApprovedReviewedCommit() {
    if (!activeProject?.projectId || !reviewedCommitPlan || reviewedCommitBusy ||
        reviewedCommitPlan.stage.projectId !== activeProject.projectId) return;
    const projectId = activeProject.projectId;
    const ticket = gitHunkRequest.current;
    setReviewedCommitBusy(true);
    setReviewedCommitReceipt(null);
    setProblem('');
    const command = reviewedCommitPlan.mode === 'amend'
      ? 'ide_git_amend_reviewed_commit' : 'ide_git_commit_selected_hunks';
    try {
      const receipt = await invoke(command, { expected: reviewedCommitPlan });
      if (ticket === gitHunkRequest.current && receipt.projectId === projectId) {
        setReviewedCommitReceipt(receipt);
        setReviewedCommitPlan(null);
        setGitPartialStageReceipt(null);
        setReviewedCommitMessage('');
        setHint(receipt.amendedExistingHistory
          ? 'Explicitly reviewed local amend completed. No force-push or hooks.'
          : 'Separately authorized local commit completed. No hooks or push.');
        void refreshGitStatus();
      }
    } catch (error) {
      if (ticket === gitHunkRequest.current) {
        setReviewedCommitPlan(null);
        setProblem('Local commit or explicit amend denied/interrupted: ' + String(error));
      }
    } finally {
      if (ticket === gitHunkRequest.current) setReviewedCommitBusy(false);
    }
  }

  async function cancelReviewedCommit() {
    if (!activeProject?.projectId || !reviewedCommitBusy) return;
    try {
      const result = await invoke('ide_git_cancel_reviewed_commit', {
        projectId: activeProject.projectId,
      });
      setHint('Commit cancellation: ' + result + '. Published refs are never automatically undone.');
    } catch (error) {
      setProblem('Commit cancellation refused at this phase: ' + String(error));
    }
  }

  async function inspectReviewedCommitRecovery() {
    if (!activeProject?.projectId || reviewedCommitRecoveryBusy) return;
    const projectId = activeProject.projectId;
    const ticket = gitHunkRequest.current;
    setReviewedCommitRecoveryBusy(true);
    setReviewedCommitRecovery(null);
    try {
      const state = await invoke('ide_git_inspect_commit_recovery', { projectId });
      if (ticket === gitHunkRequest.current && state.projectId === projectId)
        setReviewedCommitRecovery(state);
    } catch (error) {
      if (ticket === gitHunkRequest.current)
        setProblem('Commit journal inspection refused: ' + String(error));
    } finally {
      if (ticket === gitHunkRequest.current) setReviewedCommitRecoveryBusy(false);
    }
  }

  async function reconcileReviewedCommitReceipt() {
    if (!activeProject?.projectId || !reviewedCommitRecovery?.operatorReconciliationRequired ||
        reviewedCommitRecovery.gitIndexLockPresent || reviewedCommitRecoveryBusy) return;
    const projectId = activeProject.projectId;
    const ticket = gitHunkRequest.current;
    setReviewedCommitRecoveryBusy(true);
    try {
      const state = await invoke('ide_git_reconcile_commit_receipt', { projectId });
      if (ticket === gitHunkRequest.current && state.projectId === projectId)
        setReviewedCommitRecovery(state);
    } catch (error) {
      if (ticket === gitHunkRequest.current)
        setProblem('Commit journal reconciliation denied: ' + String(error));
    } finally {
      if (ticket === gitHunkRequest.current) setReviewedCommitRecoveryBusy(false);
    }
  }

  async function stageReviewedSelectedHunks() {
    if (!activeProject?.projectId || !gitHunkSelection || gitPartialStageBusy ||
        gitHunkSelection.plan.projectId !== activeProject.projectId ||
        gitHunkSelection.stageAuthorized || gitHunkSelection.operationPerformed) return;
    const projectId = activeProject.projectId;
    const ticket = gitHunkRequest.current;
    setGitPartialStageBusy(true);
    setGitPartialStageReceipt(null);
    setReviewedCommitPlan(null);
    setReviewedCommitReceipt(null);
    setProblem('');
    try {
      const receipt = await invoke('ide_git_stage_selected_hunks', {
        expected: gitHunkSelection,
      });
      if (ticket === gitHunkRequest.current && receipt.projectId === projectId) {
        setGitPartialStageReceipt(receipt);
        setGitHunkPlan(null);
        setGitHunkSelection(null);
        setGitSelectedHunkIds([]);
        setHint('The selected hunks were staged in the Git index. No commit was created.');
        void refreshGitStatus();
      }
    } catch (error) {
      if (ticket === gitHunkRequest.current) {
        setProblem('Protected partial staging denied or interrupted: ' + String(error));
        setGitHunkSelection(null);
      }
    } finally {
      if (ticket === gitHunkRequest.current) setGitPartialStageBusy(false);
    }
  }

  async function cancelProtectedPartialStage() {
    if (!activeProject?.projectId || !gitPartialStageBusy) return;
    try {
      const state = await invoke('ide_git_cancel_partial_stage', {
        projectId: activeProject.projectId,
      });
      setHint('Partial stage cancellation: ' + state + '. No published index is rolled back automatically.');
    } catch (error) {
      setProblem('Partial stage cannot be cancelled at this phase: ' + String(error));
    }
  }

  async function reconcilePartialStageReceipt() {
    if (!activeProject?.projectId || gitPartialRecoveryBusy ||
        !gitPartialRecovery?.operatorReconciliationRequired ||
        gitPartialRecovery.pendingGitLock) return;
    const projectId = activeProject.projectId;
    const ticket = gitHunkRequest.current;
    setGitPartialRecoveryBusy(true);
    setProblem('');
    try {
      const reconciled = await invoke('ide_git_reconcile_partial_stage', {projectId});
      if (ticket === gitHunkRequest.current && reconciled.projectId === projectId) {
        setGitPartialRecovery(reconciled);
        setHint('Independent native partial-stage journal reconciliation completed.');
      }
    } catch (error) {
      if (ticket === gitHunkRequest.current)
        setProblem('Native partial-stage reconciliation denied: ' + String(error));
    } finally {
      if (ticket === gitHunkRequest.current) setGitPartialRecoveryBusy(false);
    }
  }

  async function inspectPartialStageRecovery() {
    if (!activeProject?.projectId || gitPartialRecoveryBusy) return;
    const projectId = activeProject.projectId;
    const ticket = gitHunkRequest.current;
    setGitPartialRecoveryBusy(true);
    setGitPartialRecovery(null);
    try {
      const result = await invoke('ide_git_partial_stage_recovery', { projectId });
      if (ticket === gitHunkRequest.current && result.projectId === projectId)
        setGitPartialRecovery(result);
    } catch (error) {
      if (ticket === gitHunkRequest.current)
        setProblem('Native partial-stage journal inspection failed: ' + String(error));
    } finally {
      if (ticket === gitHunkRequest.current) setGitPartialRecoveryBusy(false);
    }
  }

  async function previewGitHunks() {
    if (!activeProject?.projectId || !gitHunkPath.trim() || gitHunkBusy) return;
    const projectId = activeProject.projectId;
    const ticket = ++gitHunkRequest.current;
    setGitHunkPlan(null);
    setGitHunkSelection(null);
    setGitSelectedHunkIds([]);
    setGitHunkBusy(true);
    setProblem('');
    try {
      const plan = await invoke('ide_git_preview_hunks', {
        projectId, relativePath: gitHunkPath.trim(),
      });
      if (ticket === gitHunkRequest.current && plan.projectId === projectId)
        setGitHunkPlan(plan);
    } catch (error) {
      if (ticket === gitHunkRequest.current)
        setProblem('Git hunk preview refused: ' + String(error));
    } finally {
      if (ticket === gitHunkRequest.current) setGitHunkBusy(false);
    }
  }

  async function confirmSelectedGitHunks() {
    if (!activeProject?.projectId || !gitHunkPlan ||
        gitHunkPlan.projectId !== activeProject.projectId ||
        gitSelectedHunkIds.length === 0 || gitHunkBusy) return;
    const projectId = activeProject.projectId;
    const ticket = ++gitHunkRequest.current;
    setGitHunkBusy(true);
    setGitHunkSelection(null);
    setProblem('');
    try {
      const selected = await invoke('ide_git_select_reviewed_hunks', {
        expected: gitHunkPlan, selectedIds: gitSelectedHunkIds,
      });
      if (ticket === gitHunkRequest.current &&
          selected.plan.projectId === projectId &&
          !selected.stageAuthorized && !selected.operationPerformed)
        setGitHunkSelection(selected);
    } catch (error) {
      if (ticket === gitHunkRequest.current) {
        setGitHunkPlan(null);
        setGitSelectedHunkIds([]);
        setProblem('Hunk plan changed or could not be verified: ' + String(error));
      }
    } finally {
      if (ticket === gitHunkRequest.current) setGitHunkBusy(false);
    }
  }

  async function previewGitBatch() {
    if (!activeProject || gitBatchBusy) return;
    const requested = gitBatchPaths.split(/[\n,]+/).map(x => x.trim()).filter(Boolean);
    if (!requested.length || requested.length > 8) {
      setProblem('Select between one and eight tracked relative files for a read-only batch review.');
      return;
    }
    const projectId = activeProject.projectId;
    const request = ++gitBatchRequest.current;
    setGitBatchBusy(true);
    setGitBatchReview(null);
    setProblem('');
    try {
      const preview = await invoke('ide_git_preview_batch', {
        projectId, relativePaths: requested,
      });
      if (request === gitBatchRequest.current && preview.projectId === projectId)
        setGitBatchReview(preview);
    } catch (error) {
      if (request === gitBatchRequest.current)
        setProblem('Read-only Git batch preview denied: ' + String(error));
    } finally { if (request === gitBatchRequest.current) setGitBatchBusy(false); }
  }

  async function revalidateGitBatch() {
    if (!activeProject || !gitBatchReview || gitBatchBusy ||
        gitBatchReview.projectId !== activeProject.projectId) return;
    const request = ++gitBatchRequest.current;
    setGitBatchBusy(true);
    setProblem('');
    try {
      const verified = await invoke('ide_git_verify_batch_review', {
        expected: gitBatchReview,
      });
      if (request === gitBatchRequest.current &&
          verified.projectId === activeProject.projectId)
        setGitBatchReview(verified);
    } catch (error) {
      if (request === gitBatchRequest.current) {
        setGitBatchReview(null);
        setProblem('Git batch review changed; inspect files again: ' + String(error));
      }
    } finally { if (request === gitBatchRequest.current) setGitBatchBusy(false); }
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

  async function reverifyNativeStepAfterRestart(run, index) {
    if (workflowRunBusy) return;
    setWorkflowRunBusy(true);
    setProblem('');
    try {
      const updated = await invoke('ide_workflow_verify_native_receipt', {
        workflowId: run.workflowId, step: index,
      });
      setWorkflowRuns(prev => prev.map(item =>
        item.workflowId === updated.workflowId ? updated : item));
      setHint('Completed protected step confirmed against native supervisor journal; no code rerun.');
    } catch (error) {
      setProblem('WO9 native restart receipt remains unverified: ' + String(error));
      await readWorkflowRuns();
    } finally {
      setWorkflowRunBusy(false);
    }
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

  async function previewLocalBranchOperation() {
    if (!activeProject?.projectId || branchReviewBusy ||
        (branchReviewKind !== 'stash' && !branchReviewTarget)) return;
    const projectId = activeProject.projectId;
    const operation = branchReviewKind;
    const selectedBranch = operation === 'stash' ? null : branchReviewTarget;
    const ticket = ++branchReviewRequest.current;
    replayPlanRequest.current += 1;
    setReplayPlan(null);
    setReplayPlanBusy(false);
    replayResultRequest.current += 1;
    setReplayResult(null);
    setReplayResultBusy(false);
    setReplayExecReceipt(null);
    mergeExecRequest.current += 1;
    setMergeExecReceipt(null);
    mergePlanRequest.current += 1;
    setMergePlan(null);
    setMergePlanBusy(false);
    setBranchReviewBusy(true);
    setBranchReview(null);
    setProblem('');
    try {
      const review = await invoke('ide_git_review_branch_operation', {
        projectId, operation, selectedBranch,
      });
      if (ticket !== branchReviewRequest.current) return;
      if (review?.schemaVersion !== 1 || review.projectId !== projectId ||
          review.operation !== operation || review.selectedLocalBranch !== selectedBranch ||
          review.operationAuthorized !== false || review.operationExecuted !== false ||
          review.executionRouteAvailable !== false || review.repositoryModified !== false ||
          review.gitProcessStarted !== false || review.remoteQueried !== false ||
          !Array.isArray(review.blockers) ||
          !/^[a-f0-9]{64}$/.test(review.evidenceSha256)) {
        throw Error('G06C_PREVIEW_UNTRUSTED_OR_MISMATCHED');
      }
      setBranchReview(review);
    } catch (error) {
      if (ticket === branchReviewRequest.current) {
        setBranchReview(null);
        setProblem('Could not review the exact local branch operation: ' + String(error));
      }
    } finally {
      if (ticket === branchReviewRequest.current) setBranchReviewBusy(false);
    }
  }

  async function previewRebaseCherryReplay() {
    if (!activeProject?.projectId || replayPlanBusy || branchReviewBusy ||
        !['rebase', 'cherry_pick'].includes(branchReviewKind) ||
        branchReview?.projectId !== activeProject.projectId ||
        branchReview?.operation !== branchReviewKind ||
        branchReview.selectedLocalBranch !== branchReviewTarget) return;
    const projectId = activeProject.projectId;
    const selectedBranch = branchReviewTarget;
    const operation = branchReviewKind === 'rebase'
      ? 'rebase_current_onto_local' : 'cherry_pick_local_tip';
    const upstreamSha = branchReview.evidenceSha256;
    const ticket = ++replayPlanRequest.current;
    replayResultRequest.current += 1;
    setReplayResult(null);
    setReplayResultBusy(false);
    setReplayExecReceipt(null);
    setReplayPlanBusy(true);
    setReplayPlan(null);
    setProblem('');
    try {
      const plan = await invoke('ide_git_review_rebase_cherry_pick', {
        projectId, selectedBranch, operation,
      });
      if (ticket !== replayPlanRequest.current) return;
      if (plan?.schemaVersion !== 1 || plan.projectId !== projectId ||
          plan.operation !== operation || plan.selectedLocalBranch !== selectedBranch ||
          plan.currentBranch !== branchReview.currentBranch ||
          plan.currentHead !== branchReview.currentHead ||
          plan.selectedTip !== branchReview.selectedTip ||
          plan.upstreamReviewSha256 !== upstreamSha ||
          plan.sourceRechecked !== true || plan.originalCommitsRetained !== true ||
          plan.branchRefUpdated !== false || plan.worktreeOrIndexModified !== false ||
          plan.gitHookExecuted !== false || plan.networkQueried !== false ||
          plan.processStarted !== false || plan.executionRouteAvailable !== false ||
          plan.oneUseOsApprovalGranted !== false || plan.resultingTreeVerified !== false ||
          plan.conflictResolutionPerformed !== false || plan.renameAnalysisPerformed !== false ||
          !Array.isArray(plan.blockers) || !Array.isArray(plan.replayCommits) ||
          plan.replayCommits.length > 16 ||
          !/^[a-f0-9]{64}$/.test(plan.evidenceSha256) ||
          !/^[a-f0-9]{64}$/.test(plan.originalIndexSha256)) {
        throw Error('G06C04_REPLAY_PREVIEW_STALE_OR_UNTRUSTED');
      }
      setReplayPlan(plan);
    } catch (error) {
      if (ticket === replayPlanRequest.current) {
        setReplayPlan(null);
        setProblem('Exact original-commit replay review refused: ' + String(error));
      }
    } finally {
      if (ticket === replayPlanRequest.current) setReplayPlanBusy(false);
    }
  }

  async function previewIsolatedReplayResult() {
    if (!activeProject?.projectId || replayResultBusy || replayExecBusy ||
        !replayPlan || !branchReviewTarget ||
        replayPlan.projectId !== activeProject.projectId ||
        replayPlan.selectedLocalBranch !== branchReviewTarget ||
        replayPlan.upstreamReviewSha256 !== branchReview?.evidenceSha256 ||
        !['rebase', 'cherry_pick'].includes(branchReviewKind)) return;
    const projectId = activeProject.projectId;
    const selectedBranch = branchReviewTarget;
    const operation = replayPlan.operation;
    const sourcePlan = replayPlan;
    const ticket = ++replayResultRequest.current;
    setReplayResultBusy(true);
    setReplayResult(null);
    setReplayExecReceipt(null);
    setProblem('');
    try {
      const result = await invoke('ide_git_preview_isolated_replay', {
        projectId, operation, selectedBranch,
      });
      if (ticket !== replayResultRequest.current) return;
      if (result?.schemaVersion !== 1 || result.projectId !== projectId ||
          result.operation !== operation || result.selectedLocalBranch !== selectedBranch ||
          result.currentHead !== sourcePlan.currentHead ||
          result.selectedTip !== sourcePlan.selectedTip ||
          result.replayPlanSha256 !== sourcePlan.evidenceSha256 ||
          result.originalIndexSha256 !== sourcePlan.originalIndexSha256 ||
          result.originalProjectUnchanged !== true ||
          result.quarantineOnlyResultObjects !== true ||
          result.oneUseOsAuthorized !== false ||
          result.projectRefModified !== false ||
          result.projectIndexOrWorktreeModified !== false ||
          result.projectOdbModified !== false ||
          result.noHooksOrExternalProcesses !== true ||
          result.remoteQueried !== false ||
          result.executionRouteAvailable !== false ||
          !Array.isArray(result.blockers) || !Array.isArray(result.resultSteps) ||
          !Array.isArray(result.conflictingPaths) ||
          !/^[a-f0-9]{64}$/.test(result.evidenceSha256)) {
        throw Error('G06C04B_RESULT_UNTRUSTED_OR_STALE');
      }
      setReplayResult(result);
    } catch (error) {
      if (ticket === replayResultRequest.current) {
        setReplayResult(null);
        setProblem('Isolated prospective Git replay unavailable: ' + String(error));
      }
    } finally {
      if (ticket === replayResultRequest.current) setReplayResultBusy(false);
    }
  }

  async function executeProtectedReplay() {
    if (!activeProject?.projectId || replayExecBusy || replayResultBusy ||
        !replayResult || !replayPlan ||
        replayResult.projectId !== activeProject.projectId ||
        replayResult.replayPlanSha256 !== replayPlan.evidenceSha256 ||
        replayResult.selectedLocalBranch !== branchReviewTarget ||
        replayResult.blockers.length !== 0 || replayResult.conflictCount !== 0 ||
        replayResult.candidateChangePaths < 1 || replayResult.candidateChangePaths > 80 ||
        !replayResult.prospectiveHead || !replayResult.prospectiveTree) return;
    const projectId = activeProject.projectId;
    const selectedBranch = branchReviewTarget;
    const original = replayResult;
    const cmd = original.operation === 'rebase_current_onto_local'
      ? 'ide_git_execute_rebase' : original.operation === 'cherry_pick_local_tip'
        ? 'ide_git_execute_cherry_pick' : null;
    if (!cmd) return;
    const ticket = ++replayResultRequest.current;
    setReplayExecBusy(true);
    setReplayExecReceipt(null);
    setProblem('');
    try {
      const receipt = await invoke(cmd, {
        projectId, selectedBranch, evidenceSha256: original.evidenceSha256,
      });
      if (ticket !== replayResultRequest.current) return;
      if (receipt?.schemaVersion !== 1 || receipt.projectId !== projectId ||
          receipt.operation !== original.operation ||
          receipt.originalHead !== original.currentHead ||
          receipt.publishedHead !== original.prospectiveHead ||
          receipt.resultingTree !== original.prospectiveTree ||
          receipt.resultSha256 !== original.evidenceSha256 ||
          receipt.originalObjectsPreserved !== true ||
          receipt.independentlyVerified !== true || receipt.exactRefCas !== true ||
          receipt.oneUseNativeOsConsent !== true || receipt.journalSynced !== true ||
          receipt.gitHooksExecuted !== false || receipt.remoteUpdated !== false ||
          (cmd === 'ide_git_execute_rebase' && !receipt.originalHistoryRef)) {
        throw Error('G06C04_EXECUTION_RECEIPT_UNTRUSTED');
      }
      await refreshGitStatus();
      setReplayExecReceipt(receipt);
      setHint('Native one-use OS-approved local replay verified. No remote branch changed.');
    } catch (error) {
      if (ticket === replayResultRequest.current) {
        setReplayExecReceipt(null);
        setProblem('Protected Git replay refused or interrupted: ' + String(error));
      }
    } finally {
      if (ticket === replayResultRequest.current) setReplayExecBusy(false);
    }
  }

  async function cancelProtectedReplay() {
    if (!activeProject?.projectId || !replayExecBusy) return;
    try {
      const result = await invoke('ide_git_cancel_protected_replay', {
        projectId: activeProject.projectId,
      });
      setHint('Native pre-publication cancellation: ' + result.replaceAll('_', ' '));
    } catch (error) {
      setProblem('Cancellation denied beyond object-publication boundary: ' + String(error));
    }
  }

  async function inspectReplayRecovery() {
    if (!activeProject?.projectId || replayRecoveryBusy) return;
    const projectId = activeProject.projectId;
    const ticket = ++replayResultRequest.current;
    setReplayRecoveryBusy(true);
    setReplayRecovery(null);
    try {
      const status = await invoke('ide_git_inspect_replay_recovery', { projectId });
      if (ticket !== replayResultRequest.current) return;
      if (status?.schemaVersion !== 1 || status.projectId !== projectId ||
          status.automaticRefRetry !== false ||
          status.automaticHistoryRewrite !== false ||
          status.automaticCheckout !== false ||
          status.automaticRollback !== false ||
          status.automaticForeignLockDeletion !== false) {
        throw Error('G06C04_RECOVERY_UNTRUSTED');
      }
      setReplayRecovery(status);
    } catch (error) {
      if (ticket === replayResultRequest.current) {
        setProblem('Protected replay recovery inspection refused: ' + String(error));
      }
    } finally {
      if (ticket === replayResultRequest.current) setReplayRecoveryBusy(false);
    }
  }

  async function reconcileReplayReceipt() {
    if (!activeProject?.projectId || replayRecoveryBusy ||
        replayRecovery?.projectId !== activeProject.projectId ||
        replayRecovery.state !== 'published_ref_requires_separate_receipt' ||
        replayRecovery.indexLockPresent || !replayRecovery.backupRefVerified) return;
    const projectId = activeProject.projectId;
    const ticket = ++replayResultRequest.current;
    setReplayRecoveryBusy(true);
    setProblem('');
    try {
      const result = await invoke('ide_git_reconcile_replay_receipt', { projectId });
      if (ticket !== replayResultRequest.current) return;
      if (result?.projectId !== projectId ||
          result.state !== 'verified_original_replay' ||
          result.operatorReconciliationRequired !== false ||
          result.automaticRefRetry !== false ||
          result.automaticHistoryRewrite !== false ||
          result.automaticRollback !== false ||
          result.automaticCheckout !== false ||
          result.automaticForeignLockDeletion !== false) {
        throw Error('G06C04_RECOVERY_RECEIPT_UNTRUSTED');
      }
      setReplayRecovery(result);
      setHint('Original and resulting Git history independently reverified; receipt only.');
    } catch (error) {
      if (ticket === replayResultRequest.current) {
        setProblem('Native recovery receipt denied or unverified: ' + String(error));
      }
    } finally {
      if (ticket === replayResultRequest.current) setReplayRecoveryBusy(false);
    }
  }

  async function previewExactLocalMergePlan() {
    if (!activeProject?.projectId || mergePlanBusy ||
        branchReviewKind !== 'merge' || !branchReviewTarget ||
        branchReview?.projectId !== activeProject.projectId ||
        branchReview?.operation !== 'merge' ||
        branchReview?.selectedLocalBranch !== branchReviewTarget) return;
    const projectId = activeProject.projectId;
    const selectedBranch = branchReviewTarget;
    const expectedUpstreamSha = branchReview.evidenceSha256;
    const ticket = ++mergePlanRequest.current;
    setMergePlan(null);
    setMergePlanBusy(true);
    setProblem('');
    try {
      const plan = await invoke('ide_git_preview_local_merge', { projectId, selectedBranch });
      if (ticket !== mergePlanRequest.current) return;
      if (plan?.schemaVersion !== 1 || plan.projectId !== projectId ||
          plan.selectedLocalBranch !== selectedBranch ||
          plan.upstreamReviewSha256 !== expectedUpstreamSha ||
          plan.currentHead !== branchReview.currentHead ||
          plan.selectedTip !== branchReview.selectedTip ||
          plan.sourceRechecked !== true ||
          plan.authorizationGranted !== false ||
          plan.branchRefUpdated !== false ||
          plan.indexOrWorktreeModified !== false ||
          plan.threeWayContentMergePerformed !== false ||
          plan.renameAnalysisPerformed !== false ||
          plan.hooksExecuted !== false ||
          plan.remoteQueried !== false ||
          plan.processStarted !== false ||
          plan.executionRouteAvailable !== false ||
          !Array.isArray(plan.blockers) || !Array.isArray(plan.visiblePaths) ||
          !/^[a-f0-9]{64}$/.test(plan.evidenceSha256) ||
          !/^[a-f0-9]{64}$/.test(plan.indexSha256)) {
        throw Error('G06C02_MERGE_PREVIEW_STALE_OR_UNTRUSTED');
      }
      setMergePlan(plan);
    } catch (error) {
      if (ticket === mergePlanRequest.current) {
        setMergePlan(null);
        setProblem('Conservative merge planning unavailable: ' + String(error));
      }
    } finally {
      if (ticket === mergePlanRequest.current) setMergePlanBusy(false);
    }
  }

  function selectConflictDraft(entry, source) {
    if (!nativeConflicts || nativeConflicts.projectId !== activeProject?.projectId ||
        !nativeConflicts.conflicts.some(row => row.relativePath === entry.relativePath)) return;
    const selected = source === 'ours' ? entry.ours :
      source === 'theirs' ? entry.theirs : null;
    // A truncated or binary preview can NEVER be accepted as whole-file text.
    if (selected && (selected.binaryOrNonUtf8 || selected.previewTruncated ||
        typeof selected.preview !== 'string')) {
      setProblem('G06D02_INCOMPLETE_OR_BINARY_BLOB_REQUIRES_FULL_REVIEW');
      return;
    }
    const draftText = selected ? selected.preview : '';
    setConflictDraft({
      projectId: nativeConflicts.projectId,
      currentHead: nativeConflicts.currentHead,
      indexSha256: nativeConflicts.indexSha256,
      evidenceSha256: nativeConflicts.evidenceSha256,
      relativePath: entry.relativePath,
      candidateText: draftText,
      origin: source
    });
    setConflictDraftMode(source);
    setConflictReview(null);
  }

  async function reviewNativeConflictCandidate() {
    if (!activeProject?.projectId || !nativeConflicts || !conflictDraft ||
        conflictReviewBusy || conflictApplyBusy) return;
    const snapshot = { ...conflictDraft };
    const ticket = ++conflictRequest.current;
    setConflictReviewBusy(true);
    setConflictReview(null);
    setProblem('');
    try {
      const reviewed = await invoke('ide_git_review_conflict_resolution', {
        projectId: snapshot.projectId, relativePath: snapshot.relativePath,
        inventorySha256: snapshot.evidenceSha256,
        indexSha256: snapshot.indexSha256, candidateText: snapshot.candidateText,
      });
      if (ticket !== conflictRequest.current) return;
      if (reviewed?.schemaVersion !== 1 ||
          reviewed.projectId !== snapshot.projectId ||
          reviewed.relativePath !== snapshot.relativePath ||
          reviewed.originalIndexSha256 !== snapshot.indexSha256 ||
          reviewed.inventoryEvidenceSha256 !== snapshot.evidenceSha256 ||
          reviewed.currentHead !== snapshot.currentHead ||
          reviewed.proposedBytes !== new TextEncoder().encode(snapshot.candidateText).length ||
          reviewed.sourceRechecked !== true ||
          reviewed.requiresSeparateNativeOsApproval !== true ||
          reviewed.authorityGranted !== false ||
          reviewed.automaticallyApplied !== false ||
          reviewed.indexOrWorktreeModified !== false ||
          reviewed.branchRefModified !== false ||
          !/^[a-f0-9]{64}$/.test(reviewed.reviewSha256) ||
          !/^[a-f0-9]{64}$/.test(reviewed.proposedContentSha256)) {
        throw Error('G06D03_REVIEW_EVIDENCE_UNTRUSTED');
      }
      setConflictReview({ ...reviewed, candidateText: snapshot.candidateText });
    } catch (error) {
      if (ticket === conflictRequest.current) {
        setProblem('Native reviewed resolution not available: ' + String(error));
      }
    } finally {
      if (ticket === conflictRequest.current) setConflictReviewBusy(false);
    }
  }

  async function applyNativeConflictResolution() {
    if (!activeProject?.projectId || !conflictReview || !conflictDraft ||
        conflictReview.projectId !== activeProject.projectId ||
        conflictReview.candidateText !== conflictDraft.candidateText ||
        conflictReview.relativePath !== conflictDraft.relativePath ||
        conflictReview.inventoryEvidenceSha256 !== nativeConflicts?.evidenceSha256 ||
        conflictApplyBusy || conflictReviewBusy) return;
    const ticket = ++conflictRequest.current;
    const reviewed = conflictReview;
    setConflictApplyBusy(true);
    setConflictApplyReceipt(null);
    setProblem('');
    try {
      const receipt = await invoke('ide_git_apply_conflict_resolution', {
        projectId: reviewed.projectId,
        relativePath: reviewed.relativePath,
        inventorySha256: reviewed.inventoryEvidenceSha256,
        indexSha256: reviewed.originalIndexSha256,
        candidateText: reviewed.candidateText,
        reviewSha256: reviewed.reviewSha256,
      });
      if (ticket !== conflictRequest.current) return;
      if (receipt?.schemaVersion !== 1 ||
          receipt.projectId !== reviewed.projectId ||
          receipt.relativePath !== reviewed.relativePath ||
          receipt.reviewSha256 !== reviewed.reviewSha256 ||
          receipt.proposedContentSha256 !== reviewed.proposedContentSha256 ||
          receipt.nativeOsConsentRequired !== true ||
          receipt.candidateReadbackVerified !== true ||
          receipt.targetConflictCleared !== true ||
          receipt.journalSynced !== true ||
          receipt.branchRefModified !== false ||
          receipt.commitCreated !== false || receipt.remoteContacted !== false) {
        throw Error('G06D04_NATIVE_RECEIPT_INVALID');
      }
      setConflictApplyReceipt(receipt);
      setConflictReview(null);
      setConflictDraft(null);
      setNativeConflicts(null); // Index changed; all old source evidence expires.
    } catch (error) {
      if (ticket === conflictRequest.current) {
        setProblem('Protected conflict resolution failed or requires repair: ' + String(error));
      }
    } finally {
      if (ticket === conflictRequest.current) setConflictApplyBusy(false);
    }
  }

  async function inspectNativeConflictRecovery() {
    if (!activeProject?.projectId || conflictRecoveryBusy || conflictApplyBusy) return;
    const projectId = activeProject.projectId;
    const ticket = ++conflictRequest.current;
    setConflictRecoveryBusy(true);
    setProblem('');
    try {
      const result = await invoke('ide_git_inspect_conflict_recovery', { projectId });
      if (ticket !== conflictRequest.current) return;
      if (result?.schemaVersion !== 1 || result.projectId !== projectId ||
          result.automaticRetry !== false || result.automaticRestore !== false ||
          result.automaticIndexWrite !== false || result.automaticRefUpdate !== false) {
        throw Error('G06D04_RECOVERY_EVIDENCE_INVALID');
      }
      setConflictRecovery(result);
    } catch (error) {
      if (ticket === conflictRequest.current) {
        setProblem('Native recovery inspection unavailable: ' + String(error));
      }
    } finally {
      if (ticket === conflictRequest.current) setConflictRecoveryBusy(false);
    }
  }

  async function reconcileNativeConflictReceipt() {
    if (!activeProject?.projectId || conflictRecoveryBusy || conflictApplyBusy ||
        !['resolved_requires_independent_receipt', 'unpublished_original_source_preserved']
          .includes(conflictRecovery?.state)) return;
    const projectId = activeProject.projectId;
    const ticket = ++conflictRequest.current;
    setConflictRecoveryBusy(true);
    setProblem('');
    try {
      const result = await invoke('ide_git_reconcile_conflict_receipt', { projectId });
      if (ticket !== conflictRequest.current) return;
      if (result?.schemaVersion !== 1 || result.projectId !== projectId ||
          !['verified_resolved_stage', 'unpublished_original_source_preserved']
            .includes(result.state) ||
          result.operatorReviewRequired !== (result.state !== 'verified_resolved_stage')) {
        throw Error('G06D04_RECOVERY_RECEIPT_INVALID');
      }
      setConflictRecovery(result);
    } catch (error) {
      if (ticket === conflictRequest.current) {
        setProblem('Receipt-only reconciliation denied or unavailable: ' + String(error));
      }
    } finally {
      if (ticket === conflictRequest.current) setConflictRecoveryBusy(false);
    }
  }

  async function inspectUnresolvedNativeConflicts() {
    if (!activeProject?.projectId || nativeConflictsBusy) return;
    const projectId = activeProject.projectId;
    const ticket = ++nativeConflictsRequest.current;
    setNativeConflictsBusy(true);
    setNativeConflicts(null);
    conflictRequest.current += 1;
    setConflictReview(null);
    setConflictReviewBusy(false);
    setConflictApplyReceipt(null);
    setConflictRecovery(null);
    setConflictDraft(null);
    setConflictDraftMode('manual');
    setProblem('');
    try {
      const result = await invoke('ide_git_inspect_native_conflicts', { projectId });
      if (ticket !== nativeConflictsRequest.current) return;
      if (result?.schemaVersion !== 1 || result.projectId !== projectId ||
          result.sourceRechecked !== true ||
          result.authorityGranted !== false || result.resolutionWritten !== false ||
          result.indexOrWorktreeModified !== false || result.branchRefModified !== false ||
          result.remoteContacted !== false || result.gitProcessStarted !== false ||
          !Array.isArray(result.conflicts) ||
          result.conflictCount !== result.conflicts.length ||
          !/^[a-f0-9]{64}$/.test(result.indexSha256) ||
          !/^[a-f0-9]{64}$/.test(result.evidenceSha256)) {
        throw Error('G06D01_NATIVE_CONFLICT_EVIDENCE_UNTRUSTED');
      }
      setConflictDraft(null);
      setConflictDraftMode('manual');
      setNativeConflicts(result);
    } catch (error) {
      if (ticket === nativeConflictsRequest.current) {
        setProblem('Read-only native Git conflict inspection unavailable: ' + String(error));
      }
    } finally {
      if (ticket === nativeConflictsRequest.current) setNativeConflictsBusy(false);
    }
  }

  async function verifyDivergentMergeResult() {
    if (!activeProject?.projectId || divergentBusy || divergentExecBusy ||
        branchReviewKind !== 'merge' || mergePlan?.classification !== 'divergent_merge_candidate' ||
        mergePlan?.projectId !== activeProject.projectId ||
        mergePlan?.selectedLocalBranch !== branchReviewTarget ||
        mergePlan?.upstreamReviewSha256 !== branchReview?.evidenceSha256) return;
    const projectId = activeProject.projectId;
    const selectedBranch = branchReviewTarget;
    const plan = mergePlan;
    const ticket = ++divergentRequest.current;
    setDivergentBusy(true);
    setDivergentResult(null);
    setDivergentReceipt(null);
    setProblem('');
    try {
      const candidate = await invoke('ide_git_verify_divergent_merge', {
        projectId, selectedBranch, evidenceSha256: plan.evidenceSha256,
      });
      if (ticket !== divergentRequest.current) return;
      if (candidate?.schemaVersion !== 1 || candidate.projectId !== projectId ||
          candidate.currentHead !== plan.currentHead ||
          candidate.selectedTip !== plan.selectedTip ||
          candidate.sourcePlanSha256 !== plan.evidenceSha256 ||
          candidate.firstParent !== plan.currentHead ||
          candidate.secondParent !== plan.selectedTip ||
          candidate.authorityGranted !== false ||
          candidate.sourceObjectsWritten !== false ||
          candidate.sourceIndexOrWorktreeWritten !== false ||
          !Array.isArray(candidate.conflicts) ||
          !/^[a-f0-9]{64}$/.test(candidate.resultSha256) ||
          !/^[a-f0-9]{64}$/.test(candidate.resultingIndexSha256)) {
        throw Error('G06C03B_DIVERGENT_RESULT_UNTRUSTED');
      }
      setDivergentResult(candidate);
    } catch (error) {
      if (ticket === divergentRequest.current) {
        setDivergentResult(null);
        setProblem('Isolated three-way local merge verification unavailable: ' + String(error));
      }
    } finally {
      if (ticket === divergentRequest.current) setDivergentBusy(false);
    }
  }

  async function executeProtectedDivergentMerge() {
    if (!activeProject?.projectId || divergentBusy || divergentExecBusy ||
        divergentResult?.projectId !== activeProject.projectId ||
        divergentResult.sourcePlanSha256 !== mergePlan?.evidenceSha256 ||
        divergentResult.firstParent !== mergePlan.currentHead ||
        divergentResult.secondParent !== mergePlan.selectedTip ||
        divergentResult.conflictFreePreview !== true ||
        divergentResult.conflictCount !== 0 || !divergentResult.prospectiveTree) return;
    const projectId = activeProject.projectId;
    const selectedBranch = branchReviewTarget;
    const candidate = divergentResult;
    const ticket = ++divergentRequest.current;
    setDivergentExecBusy(true);
    setDivergentReceipt(null);
    setProblem('');
    try {
      const receipt = await invoke('ide_git_execute_divergent_merge', {
        projectId, selectedBranch,
        evidenceSha256: candidate.sourcePlanSha256,
        resultSha256: candidate.resultSha256,
      });
      if (ticket !== divergentRequest.current) return;
      if (receipt?.schemaVersion !== 1 || receipt.projectId !== projectId ||
          receipt.firstParent !== candidate.firstParent ||
          receipt.secondParent !== candidate.secondParent ||
          receipt.resultingTree !== candidate.prospectiveTree ||
          receipt.reviewedResultSha256 !== candidate.resultSha256 ||
          receipt.parentCountVerified !== true ||
          receipt.originalBranchesPreserved !== true ||
          receipt.exactOldHeadCas !== true ||
          receipt.separatelyRequiredOneUseOsConsent !== true ||
          receipt.independentlyVerified !== true || receipt.journalSynced !== true ||
          receipt.hooksRun !== false || receipt.remoteMutated !== false) {
        throw Error('G06C03B_PUBLISHED_RECEIPT_UNTRUSTED');
      }
      await refreshGitStatus();
      setDivergentReceipt(receipt);
      setHint('OS-approved two-parent local merge verified; no remote branch changed.');
    } catch (error) {
      if (ticket === divergentRequest.current) {
        setProblem('Divergent merge denied, interrupted or unverified: ' + String(error));
      }
    } finally {
      if (ticket === divergentRequest.current) setDivergentExecBusy(false);
    }
  }

  async function cancelProtectedDivergentMerge() {
    if (!activeProject?.projectId || !divergentExecBusy) return;
    try {
      const result = await invoke('ide_git_cancel_protected_merge', {
        projectId: activeProject.projectId,
      });
      setHint('Native pre-publication cancellation: ' + result.replaceAll('_', ' '));
    } catch (error) {
      setProblem('Cancellation unavailable beyond object-publication boundary: ' + String(error));
    }
  }

  async function inspectDivergentRecovery() {
    if (!activeProject?.projectId || divergentRecoveryBusy) return;
    const projectId = activeProject.projectId;
    const ticket = ++divergentRequest.current;
    setDivergentRecoveryBusy(true);
    setDivergentRecovery(null);
    try {
      const state = await invoke('ide_git_inspect_divergent_recovery', { projectId });
      if (ticket !== divergentRequest.current) return;
      if (state?.schemaVersion !== 1 || state.projectId !== projectId ||
          state.automaticRetry !== false || state.automaticCheckout !== false ||
          state.automaticRefRewrite !== false || state.automaticRollback !== false ||
          state.automaticLockDeletion !== false) {
        throw Error('G06C03B_RECOVERY_UNTRUSTED');
      }
      setDivergentRecovery(state);
    } catch (error) {
      if (ticket === divergentRequest.current) {
        setProblem('Protected divergent merge restart inspection unavailable: ' + String(error));
      }
    } finally {
      if (ticket === divergentRequest.current) setDivergentRecoveryBusy(false);
    }
  }

  async function reconcileDivergentReceipt() {
    if (!activeProject?.projectId || divergentRecoveryBusy ||
        !divergentRecovery?.operatorReconciliationRequired ||
        divergentRecovery.foreignIndexLockPresent ||
        !['published_requires_independent_receipt',
          'unpublished_no_index_change_needs_review'].includes(divergentRecovery.state)) return;
    const projectId = activeProject.projectId;
    const ticket = ++divergentRequest.current;
    setDivergentRecoveryBusy(true);
    setProblem('');
    try {
      const state = await invoke('ide_git_reconcile_divergent_receipt', { projectId });
      if (ticket !== divergentRequest.current) return;
      if (state?.projectId !== projectId || state.operatorReconciliationRequired !== false ||
          state.automaticRetry !== false || state.automaticCheckout !== false ||
          state.automaticRefRewrite !== false || state.automaticRollback !== false ||
          state.automaticLockDeletion !== false) {
        throw Error('G06C03B_RECEIPT_NOT_VERIFIED');
      }
      setDivergentRecovery(state);
      setHint('Independent native two-parent merge receipt reconciled; no replay performed.');
    } catch (error) {
      if (ticket === divergentRequest.current) {
        setProblem('Receipt-only divergent recovery denied: ' + String(error));
      }
    } finally {
      if (ticket === divergentRequest.current) setDivergentRecoveryBusy(false);
    }
  }

  async function executeReviewedFastForward() {
    if (!activeProject?.projectId || mergeExecBusy || mergePlanBusy ||
        branchReviewKind !== 'merge' || mergePlan?.projectId !== activeProject.projectId ||
        mergePlan.classification !== 'fast_forward_candidate' ||
        mergePlan.blockers.length !== 0 ||
        mergePlan.changedPathCount < 1 || mergePlan.changedPathCount > 80 ||
        mergePlan.omittedPathCount !== 0 ||
        mergePlan.possibleConflictCount !== 0 ||
        mergePlan.selectedLocalBranch !== branchReviewTarget ||
        mergePlan.upstreamReviewSha256 !== branchReview?.evidenceSha256) return;
    const projectId = activeProject.projectId;
    const selectedBranch = branchReviewTarget;
    const original = mergePlan;
    const ticket = ++mergeExecRequest.current;
    setMergeExecBusy(true);
    setMergeExecReceipt(null);
    setProblem('');
    try {
      const receipt = await invoke('ide_git_execute_fast_forward', {
        projectId, selectedBranch, evidenceSha256: original.evidenceSha256,
      });
      if (ticket !== mergeExecRequest.current) return;
      if (receipt?.schemaVersion !== 1 || receipt.projectId !== projectId ||
          receipt.previousHead !== original.currentHead ||
          receipt.resultingHead !== original.selectedTip ||
          receipt.selectedTip !== original.selectedTip ||
          receipt.resultingTree !== original.selectedTree ||
          receipt.planSha256 !== original.evidenceSha256 ||
          receipt.worktreeAndIndexVerified !== true ||
          receipt.exactRefCasVerified !== true ||
          receipt.recoveryJournalVerified !== true ||
          receipt.originalHistoryRetained !== true ||
          receipt.oneUseOsApprovalRequired !== true ||
          receipt.hooksExecuted !== false || receipt.remoteUpdated !== false) {
        throw Error('G06C03_FAST_FORWARD_RECEIPT_UNTRUSTED');
      }
      await refreshGitStatus();
      setMergeExecReceipt(receipt);
      setHint('One-use OS-approved local fast-forward independently verified; remote unchanged.');
    } catch (error) {
      if (ticket === mergeExecRequest.current) {
        setMergeExecReceipt(null);
        setProblem('Protected fast-forward refused or interrupted: ' + String(error));
      }
    } finally {
      if (ticket === mergeExecRequest.current) setMergeExecBusy(false);
    }
  }

  async function cancelProtectedFastForward() {
    if (!activeProject?.projectId || !mergeExecBusy) return;
    try {
      const state = await invoke('ide_git_cancel_protected_merge', {
        projectId: activeProject.projectId,
      });
      setHint('Native cancellation: ' + state.replaceAll('_', ' '));
    } catch (error) {
      setProblem('Cannot cancel after the checkout commit point: ' + String(error));
    }
  }

  async function inspectProtectedMergeRecovery() {
    if (!activeProject?.projectId || mergeRecoveryBusy) return;
    const projectId = activeProject.projectId;
    const ticket = ++mergeExecRequest.current;
    setMergeRecoveryBusy(true);
    setMergeRecovery(null);
    try {
      const status = await invoke('ide_git_inspect_protected_merge_recovery', { projectId });
      if (ticket !== mergeExecRequest.current) return;
      if (status?.schemaVersion !== 1 || status.projectId !== projectId ||
          status.automaticRetry !== false ||
          status.automaticRefRewrite !== false ||
          status.automaticCheckout !== false ||
          status.automaticLockDeletion !== false) {
        throw Error('G06C03_RECOVERY_UNTRUSTED_RESPONSE');
      }
      setMergeRecovery(status);
    } catch (error) {
      if (ticket === mergeExecRequest.current) {
        setProblem('Protected merge recovery unavailable: ' + String(error));
      }
    } finally {
      if (ticket === mergeExecRequest.current) setMergeRecoveryBusy(false);
    }
  }

  async function reconcileProtectedMergeReceipt() {
    if (!activeProject?.projectId || !mergeRecovery?.operatorReconciliationRequired ||
        mergeRecovery.gitIndexLockPresent || mergeRecoveryBusy ||
        !['published_ref_requires_operator_receipt',
          'prepublication_no_index_change_pending_review'].includes(mergeRecovery.state)) return;
    const projectId = activeProject.projectId;
    const ticket = ++mergeExecRequest.current;
    setMergeRecoveryBusy(true);
    setProblem('');
    try {
      const result = await invoke('ide_git_reconcile_protected_merge_receipt', { projectId });
      if (ticket !== mergeExecRequest.current) return;
      if (result?.projectId !== projectId || result.operatorReconciliationRequired !== false ||
          result.automaticRetry !== false || result.automaticRefRewrite !== false ||
          result.automaticCheckout !== false || result.automaticLockDeletion !== false) {
        throw Error('G06C03_RECOVERY_RECEIPT_UNTRUSTED');
      }
      setMergeRecovery(result);
      setHint('Only the independently verified historical journal receipt was reconciled.');
    } catch (error) {
      if (ticket === mergeExecRequest.current) {
        setProblem('Recovery review was denied, interrupted, or unverified: ' + String(error));
      }
    } finally {
      if (ticket === mergeExecRequest.current) setMergeRecoveryBusy(false);
    }
  }

  async function refreshGitStatus() {
    replayResultRequest.current += 1;
    setReplayResult(null);
    setReplayResultBusy(false);
    setReplayExecBusy(false);
    setReplayExecReceipt(null);
    setReplayRecovery(null);
    setReplayRecoveryBusy(false);
    replayPlanRequest.current += 1;
    setReplayPlan(null);
    setReplayPlanBusy(false);
    mergeExecRequest.current += 1;
    setMergeExecReceipt(null);
    setMergeRecovery(null);
    setMergeExecBusy(false);
    setMergeRecoveryBusy(false);
    divergentRequest.current += 1;
    setDivergentResult(null);
    setDivergentBusy(false);
    setDivergentExecBusy(false);
    setDivergentReceipt(null);
    setDivergentRecovery(null);
    setDivergentRecoveryBusy(false);
    nativeConflictsRequest.current += 1;
    setNativeConflicts(null);
    conflictRequest.current += 1;
    setConflictReview(null);
    setConflictReviewBusy(false);
    setConflictApplyReceipt(null);
    setConflictRecovery(null);
    setConflictDraft(null);
    setConflictDraftMode('manual');
    setNativeConflictsBusy(false);
    branchReviewRequest.current += 1;
    setBranchReview(null);
    setBranchReviewBusy(false);
    mergePlanRequest.current += 1;
    setMergePlan(null);
    setMergePlanBusy(false);
    if (!activeProject?.projectId) {
      setGitStatus(null);
      setGitInventory(null);
      setGitRefsHealth(null);
      return;
    }
    const projectId = activeProject.projectId;
    const request = ++gitSnapshotRequest.current;
    setGitBusy(true);
    setGitStatus(null);
    setGitInventory(null);
    setGitRefsHealth(null);
    setGitFileHistory(null);
    setGitFileBusy(false);
    setGitDiff(null);
    setGitHistory(null);
    gitEvidenceRequest.current += 1;
    setGithubComparison(null); // A local refresh invalidates any prior live comparison.
    githubPullsRequest.current += 1;
    setGithubPulls(null);
    setGithubPullsBusy(false);
    exactCiRequest.current += 1;
    setExactCi(null);
    setExactCiBusy(false);
    setProblem('');
    try {
      const [status, inventory, refs] = await Promise.all([
        invoke('ide_read_git_status', { projectId }),
        invoke('ide_read_git_repository_inventory', { projectId }),
        invoke('ide_read_git_refs_health', { projectId }),
      ]);
      if (request !== gitSnapshotRequest.current) return;
      if (inventory.projectId !== projectId || refs.projectId !== projectId || activeProject?.projectId !== projectId)
        throw Error('GIT_PROJECT_CONTEXT_CHANGED');
      setGitStatus(status);
      setGitInventory(inventory);
      setGitRefsHealth(refs);
    } catch (error) {
      if (request === gitSnapshotRequest.current) {
        setGitStatus(null);
        setGitInventory(null);
        setGitRefsHealth(null);
        setProblem('Could not inspect source control: ' + String(error));
      }
    } finally { if (request === gitSnapshotRequest.current) setGitBusy(false); }
  }

  async function loadGitFileHistory() {
    if (!activeProject?.projectId || !gitFilePath.trim() || gitFileBusy) return;
    const projectId = activeProject.projectId;
    const request = gitSnapshotRequest.current;
    setGitFileBusy(true);
    setGitFileHistory(null);
    try {
      const history = await invoke('ide_read_git_file_history', {
        projectId, relativePath: gitFilePath.trim(),
      });
      if (request !== gitSnapshotRequest.current || history.projectId !== projectId) return;
      setGitFileHistory(history);
    } catch (error) {
      if (request === gitSnapshotRequest.current)
        setProblem('File blame/history unavailable: ' + String(error));
    } finally { if (request === gitSnapshotRequest.current) setGitFileBusy(false); }
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

  async function inspectGithubPulls() {
    if (!activeProject?.projectId || githubPullsBusy || gitBusy) return;
    const projectId = activeProject.projectId;
    const ticket = ++githubPullsRequest.current;
    setGithubPullsBusy(true);
    setGithubPulls(null);
    setProblem('');
    try {
      const result = await invoke('ide_read_github_pull_requests', { projectId });
      if (ticket !== githubPullsRequest.current ||
          activeProjectIdRef.current !== projectId) return;
      if (result?.schemaVersion !== 1 || result.projectId !== projectId ||
          result.readOnly !== true || result.credentialsUsed !== false ||
          result.remoteRefModified !== false || result.reviewSubmitted !== false ||
          result.gitProcessStarted !== false || result.taskCompletionVerified !== false ||
          !Array.isArray(result.pulls) || result.pulls.length > 20 ||
          typeof result.state !== 'string') {
        throw Error('G06E01_UNTRUSTED_PR_INSPECTION');
      }
      if (result.pulls.length > 0 &&
          (result.sourceRechecked !== true ||
            !/^[a-f0-9]{40}$/i.test(result.localHead || '') ||
            !/^[a-z0-9_.-]+\/[a-z0-9_.-]+$/i.test(result.repository || '') ||
            !result.pulls.every(pr =>
              Number.isSafeInteger(pr.number) && pr.number > 0 &&
              typeof pr.title === 'string' && pr.title.length > 0 &&
              pr.state === 'open' && typeof pr.draft === 'boolean' &&
              /^[a-f0-9]{40}$/i.test(pr.headSha || '') &&
              /^[a-f0-9]{40}$/i.test(pr.baseSha || '') &&
              pr.headBranch === result.currentBranch &&
              pr.url === `https://github.com/${result.repository}/pull/${pr.number}` &&
              pr.reviewApprovalVerified === false && pr.checksVerified === false &&
              pr.operationAuthorized === false))) {
        throw Error('G06E01_PR_SOURCE_MISMATCH');
      }
      setGithubPulls(result);
    } catch (error) {
      if (ticket === githubPullsRequest.current &&
          activeProjectIdRef.current === projectId) {
        setProblem('Could not verify public pull-request metadata: ' + String(error));
      }
    } finally {
      if (ticket === githubPullsRequest.current) setGithubPullsBusy(false);
    }
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

  async function inspectDebugTarget() {
    const projectId=activeProjectIdRef.current;
    const profile=projectRunProfiles?.profiles?.find(p=>p.id==='rust_build');
    const sha=profile?.sha;
    if(!projectId||debugTargetBusy||buildBusy||!sha||
       projectRunProfiles.projectId!==projectId)return;
    const request=++debugTargetRequest.current;
    setDebugTargetBusy(true);
    setProblem('');
    try {
      const raw=await invoke('ide_review_project_debug_target',
        {projectId,expectedManifestSha256:sha});
      if(request!==debugTargetRequest.current||
         activeProjectIdRef.current!==projectId)return;
      const result=safeDebugTargetReview(raw,projectId,sha);
      setDebugTargetReview(result);
      setHint('Reviewed project source provenance only. No executable or debugger grant exists.');
    }catch(error){
      if(request===debugTargetRequest.current&&
         activeProjectIdRef.current===projectId){
        setDebugTargetReview(null);
        setProblem('Project debugger review denied: '+String(error));
      }
    }finally{
      if(request===debugTargetRequest.current)setDebugTargetBusy(false);
    }
  }

  async function inspectConventionalTests() {
    const projectId=activeProjectIdRef.current;
    if(!projectId || testDiscoveryBusy || buildBusy ||
       explorerIoTicket.current || workspaceLayoutOp.current) return;
    const ticket=++testDiscoveryTicket.current;
    setTestDiscoveryBusy(true);
    setProblem('');
    try {
      const raw=await invoke('ide_discover_conventional_tests',{projectId});
      if(ticket!==testDiscoveryTicket.current ||
         activeProjectIdRef.current!==projectId)return;
      const result=safeConventionalTestDiscovery(raw,projectId);
      setTestDiscovery(result);
      setHint('Inspected '+result.files.length+
        ' conventional test filenames without executing project code.');
    } catch(error) {
      if(ticket===testDiscoveryTicket.current &&
         activeProjectIdRef.current===projectId){
        setTestDiscovery(null);
        setProblem('Test discovery unavailable: '+String(error));
      }
    } finally {
      if(ticket===testDiscoveryTicket.current)setTestDiscoveryBusy(false);
    }
  }

  async function inspectProjectRunProfiles() {
    const projectId=activeProjectIdRef.current;
    if(!projectId || profileDiscoveryBusy || buildBusy ||
       explorerIoTicket.current || workspaceLayoutOp.current) return;
    const request=++profileDiscoveryRequest.current;
    adapterReviewRequest.current += 1;
    setAdapterReview(null);
    setAdapterReviewBusy(false);
    setProfileDiscoveryBusy(true);
    setProblem('');
    try {
      const observed=await invoke('ide_discover_project_run_profiles',{projectId});
      if(request!==profileDiscoveryRequest.current ||
         activeProjectIdRef.current!==projectId) return;
      const checked=safeProjectRunProfileDiscovery(observed,projectId);
      setProjectRunProfiles(checked);
      setHint('Inspected '+checked.profiles.length+
        ' fixed build/test profiles without executing project code.');
    } catch(error) {
      if(request===profileDiscoveryRequest.current &&
         activeProjectIdRef.current===projectId) {
        setProjectRunProfiles(null);
        setProblem('Could not inspect project run profiles: '+String(error));
      }
    } finally {
      if(request===profileDiscoveryRequest.current)
        setProfileDiscoveryBusy(false);
    }
  }

  async function inspectReviewedMultilangAdapter(profileId) {
    const projectId=activeProjectIdRef.current;
    if (!projectId || adapterReviewBusy || profileDiscoveryBusy ||
        buildBusy || explorerIoTicket.current || workspaceLayoutOp.current)
      return;
    let requestPlan;
    try {
      requestPlan=planReviewedAdapterRequest(
        projectRunProfiles,projectId,profileId);
    } catch(error) {
      setProblem('Cannot review adapter: '+String(error));
      return;
    }
    const request=++adapterReviewRequest.current;
    const discovery=profileDiscoveryRequest.current;
    setAdapterReviewBusy(true);
    setAdapterReview(null);
    setProblem('');
    try {
      const result=await invoke('ide_review_multilang_adapter',requestPlan);
      if (request!==adapterReviewRequest.current ||
          discovery!==profileDiscoveryRequest.current ||
          activeProjectIdRef.current!==projectId) return;
      const checked=checkedReviewedAdapterContract(result,requestPlan);
      setAdapterReview(checked);
      setHint('Native contract reviewed for '+profileId+
        '. No language worker, source snapshot or execution approval was issued.');
    } catch(error) {
      if (request===adapterReviewRequest.current &&
          activeProjectIdRef.current===projectId)
        setProblem('Native adapter review denied: '+String(error));
    } finally {
      if (request===adapterReviewRequest.current)
        setAdapterReviewBusy(false);
    }
  }

  function chooseProjectRunProfile(profileId) {
    const projectId=activeProjectIdRef.current;
    try {
      const selected=selectExistingProtectedRecipe(
        projectRunProfiles,profileId,projectId);
      // An immutable recipe selector is NOT an OS confirmation or grant.
      // Existing startProtectedBuild still gates with pinned release
      // resources, a frozen input snapshot and fresh native OS approval.
      setIntentRecipe(selected);
      setHint('Selected existing protected Cargo recipe '+selected+
        '. No process started; native approval remains required.');
    } catch(error) { setProblem('Profile is not runnable: '+String(error)); }
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
        buildStatus.current || !['rust_check', 'rust_build', 'rust_tests', 'lu_ide_host_tests'].includes(intentRecipe)) return;
    setBuildBusy(true);
    setBuildResult(null);
    nativeJobOutputRef.current=null;
    nativeOutputCursor.current={projectId:null,jobId:null,sequence:0};
    setNativeJobOutput(null);
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
      // One final drain after the native terminal receipt; this never
      // substitutes output text for completed/cancelled state.
      try {
        // Four bounded pages cover the native 256-chunk rolling window.
        // Do not mark a worker successful based on this advisory text.
        for(let n=0;n<4;n++) {
          const cursor=nativeOutputCursor.current;
          const after=cursor.projectId===projectId &&
            cursor.jobId===outcome.jobId ? cursor.sequence : 0;
          const page=await invoke('ide_build_read_output',
            {projectId,jobId:outcome.jobId,after});
          if (activeProjectIdRef.current!==projectId) break;
          const next=acceptNativeOutput(
            cursor.projectId===projectId && cursor.jobId===outcome.jobId
              ? nativeJobOutputRef.current : null,
            projectId,outcome.jobId,page);
          if(!next || next.lastSequence===after) break;
          nativeJobOutputRef.current=next;
          nativeOutputCursor.current={projectId,jobId:outcome.jobId,
            sequence:next.lastSequence};
          setNativeJobOutput(next);
          if(next.lastSequence===page.lastSequence) break;
        }
      } catch {
        // Terminal receipt remains authoritative if the advisory log fails.
      }
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

  async function runProtectedProjectDebugger() {
    const projectId=activeProject?.projectId;
    const witness=safeDebugSymbolEvidence(buildResult,projectId);
    if(!projectId||!witness||!debugStatus?.projectDebugSupported||
       !!debugStatus.current||debugBusy||buildBusy)return;
    setDebugBusy(true);
    setProblem('');
    setDebugResult(null);
    try {
      // Only projectId reaches Tauri. All build receipts, binary files,
      // debugger args and authorizations are resolved by the native host.
      const result=await invoke('ide_debug_run_project',{projectId});
      if(activeProjectIdRef.current!==projectId)return;
      if(!checkedProjectDebugWitness(result,buildResult,projectId))
        throw Error('DEV044_PROJECT_DEBUG_RECEIPT_DENIED');
      setDebugResult(result);
      setHint('Exact protected project debugger returned a native stop, stack and OS tree cleanup receipt.');
    }catch(error){
      if(activeProjectIdRef.current===projectId)
        setProblem('Project debugger denied or failed: '+String(error));
    }finally{
      setDebugBusy(false);
      if(activeProjectIdRef.current===projectId)
        invoke('ide_debug_status',{projectId}).then(setDebugStatus).catch(()=>{});
    }
  }

  // Fixed-choice request only: Rust independently picks the immutable target,
  // authenticates LLDB's native stop and asks via a separate OS-native dialog.
  // A WebView action here is NOT human consent and carries no DAP arguments.
  async function runNativeDebuggerAction(kind) {
    const projectId=activeProject?.projectId;
    const witness=safeDebugSymbolEvidence(buildResult,projectId);
    if(!projectId || !witness || !debugStatus?.projectDebugSupported ||
       !debugStatus?.operationControlSupported ||
       !!debugStatus.current || debugBusy || buildBusy)return;
    const command=kind==='stepOver'?'ide_debug_step_project':
      kind==='sameLineBreakpoint'?'ide_debug_breakpoint_project':
      kind==='inspectVariables'?'ide_debug_inspect_project_variables':null;
    if(!command)return;
    setDebugBusy(true);
    setProblem('');
    setDebugResult(null);
    try {
      const result=await invoke(command,{projectId});
      if(activeProjectIdRef.current!==projectId)return;
      if(!checkedProjectDebugWitness(result,buildResult,projectId))
        throw Error('DEV046D_PROJECT_RECEIPT_DENIED');
      if(kind==='stepOver' &&
         (result.operationOutcomeCode!=='native_dialog_step_verified' ||
          !/^[a-f0-9]{64}$/i.test(result.operationReceiptSha256||'')))
        throw Error('DEV046D_STEP_REQUIRES_NATIVE_VERIFIED_RESULT');
      if(kind==='sameLineBreakpoint' &&
         (result.operationOutcomeCode!=='native_dialog_bp_uncertain' ||
          !/^[a-f0-9]{64}$/i.test(result.operationReceiptSha256||'')))
        throw Error('DEV046D_BREAKPOINT_RESULT_MUST_BE_UNCERTAIN');
      if(kind==='inspectVariables') {
        if(result.operationOutcomeCode!=='native_dialog_variables_redacted_uncertain' ||
           !/^[a-f0-9]{64}$/i.test(result.operationReceiptSha256||'') ||
           !Number.isSafeInteger(result.inspectedFrameId) ||
           result.inspectedFrameId<=0 ||
           !Array.isArray(result.redactedVariables) ||
           result.redactedVariables.length>16 ||
           result.redactedVariables.some(entry=>
             !entry || entry.valueRedacted!==true ||
             !/^[A-Za-z0-9_]{1,64}$/.test(entry.name||'') ||
             (entry.declaredType!=null && (
               typeof entry.declaredType!=='string' ||
               entry.declaredType.length>64)))) {
          throw Error('IDE_G04D_VARIABLES_UNREVIEWED_OR_RAW_VALUES_DENIED');
        }
      }
      setDebugResult(result);
      setHint(kind==='stepOver'?
        'Native dialog approved exactly one step; independent OS/PDB stop and terminal journal were verified in this development build. Not signed installed acceptance.':
        kind==='inspectVariables'?
        'One fresh native dialog approved read-only local variable names/types. All values are redacted, and adapter assertions remain unverified. No installed release acceptance.':
        'Native dialog approved a same-source-line breakpoint; adapter acknowledgment is not an independently verified breakpoint hit. Result remains uncertain.');
    }catch(error){
      if(activeProjectIdRef.current===projectId)
        setProblem('Native debugger action denied or failed: '+String(error));
    }finally{
      setDebugBusy(false);
      if(activeProjectIdRef.current===projectId)
        invoke('ide_debug_status',{projectId}).then(setDebugStatus).catch(()=>{});
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

  async function loadEditorTaskCatalog() {
    const projectId=activeProjectIdRef.current;
    if (!projectId || editorTaskBusy || savingKey || explorerIoTicket.current)
      return;
    const request=++editorTaskRequest.current;
    setEditorTaskBusy(true);
    setProblem('');
    try {
      const result=await invoke('ide_read_project_context',{projectId});
      if (request!==editorTaskRequest.current ||
          activeProjectIdRef.current!==projectId) return;
      if (result?.projectId!==projectId || !Array.isArray(result.tasks))
        throw Error('C7_TASK_CATALOG_INVALID');
      const tasks=result.tasks.filter(t=>typeof t.taskId==='string').slice(0,64);
      setEditorTaskCatalog({projectId,tasks});
      setEditorTaskId(previous=>tasks.some(t=>t.taskId===previous)
        ? previous : tasks[0]?.taskId??'');
      setHint('Loaded '+tasks.length+
        ' existing native development tasks. No task was changed.');
    } catch(error) {
      if (request===editorTaskRequest.current &&
          activeProjectIdRef.current===projectId)
        setProblem('Could not load development tasks: '+String(error));
    } finally {
      if (request===editorTaskRequest.current)setEditorTaskBusy(false);
    }
  }

  async function linkActiveSourceToTask(tab) {
    const projectId=activeProjectIdRef.current;
    if (!tab || !editorTaskCatalog || editorTaskCatalog.projectId!==projectId ||
        editorTaskBusy || taskBusy || savingKey || explorerPending ||
        explorerIoTicket.current || workspaceLayoutOp.current || mergeBusy)
      return;
    const state=editorStates.current.get(tab.key);
    let plan;
    try {
      plan=planTaskSourceLink(editorTaskCatalog.tasks,editorTaskId,
        tab,state?.sliceDoc(),projectId);
    } catch(error){setProblem('Cannot link source to task: '+String(error));return;}
    if (!window.confirm('Link the current SAVED disk revision of '+tab.relativePath+
      ' to task '+editorTaskId+'? '+
      (plan.hasUnrecordedDraft
        ? 'Your unsaved editor edits are NOT included. '
        : '')+
      'This records a source observation, not task completion or CI approval.'))
      return;
    const token=++editorTaskRequest.current;
    setEditorTaskBusy(true);
    setProblem('');
    try {
      const result=await invoke('ide_link_task_source',plan.request);
      if (token!==editorTaskRequest.current ||
          activeProjectIdRef.current!==projectId) return;
      if(result?.taskId!==editorTaskId ||
          result.updateEventId!==plan.boundUpdateEventId ||
          !result.sourceLinks?.some(link=>
            link.relativePath===tab.relativePath &&
            link.savedRevision===plan.request.expectedSavedRevision))
        throw Error('C7_LINK_RESULT_UNTRUSTED');
      setEditorTaskCatalog(previous=>previous?.projectId===projectId
        ? {...previous,tasks:previous.tasks.map(task=>
          task.taskId===result.taskId?result:task)}:previous);
      setHint('Host recorded the saved file revision for '+result.taskId+
        '. Unsaved editor work and task acceptance remain unverified.');
      // If Continuity is already visible, force a fresh native read later.
      setProjectContext(previous=>previous?.projectId===projectId
        ? {...previous,tasks:previous.tasks.map(task=>
          task.taskId===result.taskId?result:task)}:previous);
    } catch(error){
      if(token===editorTaskRequest.current &&
          activeProjectIdRef.current===projectId)
        setProblem('Could not link source file: '+String(error)+
          '. Refresh tasks if the task revision changed.');
    } finally {
      if(token===editorTaskRequest.current)setEditorTaskBusy(false);
    }
  }

  async function recheckLinkedTaskSource(task,link) {
    const projectId=activeProjectIdRef.current;
    if (!projectId || !task || !link || !projectContext ||
        projectContext.projectId!==projectId || explorerIoTicket.current)
      return;
    const token=++taskSourceCheckRequest.current;
    const key=task.taskId+':'+link.relativePath;
    setTaskSourceChecks(previous=>({...previous,[key]:{state:'checking'}}));
    try {
      const preview=await invoke('ide_preview_source_file',{
        projectId,relativePath:link.relativePath,
      });
      if(token!==taskSourceCheckRequest.current ||
          activeProjectIdRef.current!==projectId) return;
      const currentTask=projectContext.tasks.find(t=>t.taskId===task.taskId);
      const currentLink=currentTask?.sourceLinks?.find(item=>
        item.relativePath===link.relativePath);
      if (!currentTask || !currentLink ||
          currentLink.savedRevision!==link.savedRevision ||
          currentTask.updateEventId!==task.updateEventId)
        throw Error('C7_TASK_LINK_CHANGED');
      const result=checkedSourceReobservation(link,currentTask,projectId,preview);
      setTaskSourceChecks(previous=>({...previous,[key]:{
        state:result.matchesRecordedSavedRevision?'matched_disk':'changed_disk',
        diskRevision:result.observedDiskRevision,
        taskUpdateCurrent:result.linkedToCurrentTaskUpdate,
      }}));
    } catch(error) {
      if(token===taskSourceCheckRequest.current &&
          activeProjectIdRef.current===projectId)
        setTaskSourceChecks(previous=>({...previous,[key]:{
          state:'unavailable_or_stale',error:String(error).slice(0,100),
        }}));
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
    if (inboxBusy || pending || savingKey || savingKeyRef.current ||
        explorerPending || explorerIoTicket.current ||
        workspaceLayoutOp.current || workspaceTransitionRef.current) return;
    const project=projects.find(entry=>entry.projectId===item.projectId && entry.active);
    if (!project) {
      setProblem('Open this registered project before staging its proposed edits.');
      return;
    }
    // Confirm against CodeMirror rather than a delayed React dirty flag.
    const witness=projectEditorWitness(item.projectId);
    if (witness.some(entry=>entry.intent.status!=='current' ||
        entry.intent.dirty) &&
        !window.confirm('Staging will replace live unsaved drafts in this project. '+
          'Continue? No project files will be saved or deleted.')) return;
    const token=Symbol('ide-int01c-proposal-stage');
    workspaceTransitionRef.current=token;
    const originalProjectId=activeProjectIdRef.current;
    const originalEpoch=projectNavigationEpoch.current;
    setInboxBusy(true);
    setProblem('');
    try {
      const approved=await invoke('ide_stage_proposal',{
        inboxId:item.inboxId,expectedDigest:item.digest,
      });
      if (workspaceTransitionRef.current!==token ||
          activeProjectIdRef.current!==originalProjectId ||
          projectNavigationEpoch.current!==originalEpoch) return;
      if (!Array.isArray(approved?.edits) || approved.edits.length===0 ||
          approved.edits.some(edit=>!verifiedRelativeSourcePath(edit.relativePath) ||
            typeof edit.proposedText!=='string'))
        throw Error('UNTRUSTED_PROPOSAL_SOURCE');
      // No partial staging when one source changed on disk.
      const originals=await Promise.all(approved.edits.map(edit=>
        invoke('ide_preview_source_file',{
          projectId:item.projectId,relativePath:edit.relativePath,
        })
      ));
      for (let index=0;index<originals.length;index++) {
        if (originals[index]?.projectId!==item.projectId ||
            originals[index]?.relativePath!==approved.edits[index].relativePath ||
            originals[index]?.revision!==approved.edits[index].expectedRevision)
          throw Error('PROPOSAL_SOURCE_CHANGED_ON_DISK');
      }
      const rootRaw=await invoke('ide_list_source_directory_page',{
        projectId:item.projectId,relativeDir:'',after:null,
      });
      const rootListing=checkedDirectoryPage(rootRaw,item.projectId,'',null);
      if (rootListing?.projectId!==item.projectId ||
          rootListing?.relativeDir!=='' || !Array.isArray(rootListing.entries))
        throw Error('PROPOSAL_PROJECT_LISTING_UNTRUSTED');
      if (workspaceTransitionRef.current!==token ||
          activeProjectIdRef.current!==originalProjectId ||
          projectNavigationEpoch.current!==originalEpoch ||
          projectEditorWitnessChanged(item.projectId,witness))
        throw Error('PROPOSAL_TARGET_EDITOR_CHANGED_DURING_REVIEW');
      const incoming=approved.edits.map(edit=>keyFor(item.projectId,edit.relativePath));
      if (new Set(incoming).size!==incoming.length)
        throw Error('PROPOSAL_DUPLICATE_SOURCE_PATH');
      const admission=admitOpenTabs(tabsRef.current,item.projectId,incoming);
      if (!admission.allowed)
        throw Error('OPEN_TAB_BUDGET_EXCEEDED: close unused tabs before staging.');
      // Only now can this edit replace the target project's in-memory buffers.
      const stagedTabs=approved.edits.map((edit,index)=>{
        const key=keyFor(item.projectId,edit.relativePath);
        const previous=tabsRef.current.find(tab=>tab.key===key);
        clearPrivateDraftSession(key);
        resetBufferIdentity(key,item.projectId,edit.relativePath,edit.expectedRevision);
        editorStates.current.set(key,createEditorState(
          edit.proposedText,edit.relativePath,
          (state,draft)=>onEditorChange(key,state,draft),
          state=>onEditorSelection(key,state)
        ));
        return {key,projectId:item.projectId,relativePath:edit.relativePath,
          revision:edit.expectedRevision,savedText:originals[index].text,
          draft:edit.proposedText,
          dirty:edit.proposedText!==originals[index].text,
          generation:(previous?.generation??0)+1,
          bufferRevision:(previous?.bufferRevision??0)+1};
      });
      const stagedKeys=new Set(stagedTabs.map(tab=>tab.key));
      const nextTabs=[...tabsRef.current.filter(tab=>!stagedKeys.has(tab.key)),
        ...stagedTabs];
      const pane=paneSnapshotRef.current;
      if (originalProjectId && originalProjectId!==item.projectId) {
        const leaving=captureProjectView(tabsRef.current,originalProjectId,
          pane.primaryKey,pane.secondaryKey,pane.splitOpen,pane.tabWindowStart);
        if (leaving) projectEditorViews.current.set(originalProjectId,leaving);
        setSplitEditorOpen(false);
        setSecondaryTabKey(null);
        setTabWindowStart(0);
      } else {
        const selected=planProjectPaneSelection(nextTabs,item.projectId,
          pane.primaryKey,pane.secondaryKey,pane.splitOpen,stagedTabs[0].key);
        if (!selected.allowed) throw Error('PROPOSAL_PANE_SELECTION_UNAVAILABLE');
        setSecondaryTabKey(selected.secondaryKey);
      }
      projectNavigationEpoch.current+=1;
      setTabs(previous=>[...previous.filter(tab=>!stagedKeys.has(tab.key)),
        ...stagedTabs]);
      setActiveProject(project);
      setListing(rootListing);
      setDirectory('');
      setSection('explorer');
      setActiveTabKey(stagedTabs[0].key);
      setProposalDetail(null);
      setHint('Proposal staged only as editor drafts. Review and Save each file yourself.');
    } catch(error) {
      if (workspaceTransitionRef.current===token)
        setProblem('Could not stage proposal without losing editor state: '+String(error));
    } finally {
      if (workspaceTransitionRef.current===token) workspaceTransitionRef.current=null;
      setInboxBusy(false);
    }
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

  // A project close/unregister may complete after the user typed in a pane.
  // Capture ALL live editor identities before native IPC, not React dirty flags.
  function projectEditorWitness(projectId) {
    return tabsRef.current.filter(tab=>tab.projectId===projectId)
      .map(tab=>({key:tab.key,intent:captureLiveEditorIntent(tab,
        bufferIdentities.current.get(tab.key),
        editorStates.current.get(tab.key),projectId)}));
  }
  function projectEditorWitnessChanged(projectId, before) {
    const current=tabsRef.current.filter(tab=>tab.projectId===projectId);
    if (current.length!==before.length) return true;
    return before.some(entry=>!stillCurrentEditorIntent(entry.intent,
      current.find(tab=>tab.key===entry.key),
      bufferIdentities.current.get(entry.key),
      editorStates.current.get(entry.key),projectId));
  }
  function discardProjectTabs(projectId) {
    // Only invoked after the native close succeeds AND the editor witness
    // remains unchanged. No source file is ever deleted by this operation.
    projectEditorViews.current.delete(projectId);
    setReview(previous => previous?.projectId === projectId ? null : previous);
    const affected = tabsRef.current.filter(tab => tab.projectId === projectId);
    affected.forEach(tab => {
      editorStates.current.delete(tab.key);
      clearPrivateDraftSession(tab.key);
    });
    setTabs(previous => previous.filter(tab => tab.projectId !== projectId));
    if (activeProjectIdRef.current === projectId) {
      setSecondaryTabKey(null);
      setSplitEditorOpen(false);
    }
    if (activeTabKey && affected.some(tab => tab.key === activeTabKey)) {
      setActiveTabKey(null);
    }
  }

  async function operation(command, args) {
    if (pending || savingKey || savingKeyRef.current || mergeBusy ||
        mergeApplyRequest || inboxBusy || workspaceTransitionRef.current ||
        explorerPending || explorerIoTicket.current ||
        workspaceLayoutOp.current) return;
    const closing=command==='ide_close_project' ||
      command==='ide_unregister_project';
    const witness=closing ? projectEditorWitness(args.projectId) : null;
    if (command==='ide_close_project' &&
        witness.some(entry=>entry.intent.status!=='current' || entry.intent.dirty) &&
        !window.confirm('Discard the current unsaved editor buffers in this project?'))
      return;
    setPending(true);
    setProblem('');
    setHint('');
    try {
      const next = await invoke(command, args);
      setProjects(next);
      if (closing) {
        if (projectEditorWitnessChanged(args.projectId,witness)) {
          // The native project lifecycle already completed; retain the live
          // in-memory drafts instead of deleting edits made during that IPC.
          setProblem('Project was closed or unregistered, but editor buffers changed '+
            'during the operation. Live drafts were kept in memory; reopen or '+
            'register the project again before saving or dismissing these tabs.');
        } else {
          discardProjectTabs(args.projectId);
        }
      }
      if (command === 'ide_choose_project') {
        setHint('Project selection complete. No scripts or project tools were executed.');
      }
      if (closing && activeProject?.projectId === args?.projectId) {
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
    if (pending || savingKey || savingKeyRef.current || explorerPending ||
        workspaceLayoutOp.current) return;
    const witness=projectEditorWitness(project.projectId);
    const hasDirty=witness.some(entry=>entry.intent.status!=='current' ||
      entry.intent.dirty);
    const message = 'Remove "' + project.displayName + '" from Lu IDE?\\n\\n' +
      'The original project folder and ALL files on disk will be kept. ' +
      'Only the IDE registration will be removed.\\n\\n' +
      'You can add the folder again using Open folder.' +
      (hasDirty ? '\\n\\nWARNING: Unsaved live editor changes will be discarded unless ' +
        'they were preserved separately as private draft checkpoints.' : '');
    if (window.confirm(message)) {
      void operation('ide_unregister_project', { projectId: project.projectId });
    }
  }

  function beginExplorerIo() {
    if (explorerIoTicket.current) return null;
    const ticket = Symbol('explorer-read');
    explorerIoTicket.current = ticket;
    setExplorerPending(true);
    return ticket;
  }
  function finishExplorerIo(ticket) {
    if (explorerIoTicket.current !== ticket) return;
    explorerIoTicket.current = null;
    setExplorerPending(false);
  }

  async function openDirectory(project, path) {
    if (explorerPending || mergeBusy || mergeApplyRequest ||
        workspaceLayoutOp.current || workspaceTransitionRef.current || !project?.active ||
        !projects.some(row=>row.projectId===project.projectId && row.active)) return;
    const ticket=beginExplorerIo();
    if (!ticket) return;
    const token=Symbol('ide-int01c-project-directory');
    workspaceTransitionRef.current=token;
    const request={epoch:++projectNavigationEpoch.current,
      fromProjectId:activeProjectIdRef.current,
      toProjectId:project.projectId,relativeDir:path};
    setProblem('');
    try {
      const raw=await invoke('ide_list_source_directory_page',{
        projectId:project.projectId,relativeDir:path,after:null,
      });
      const result=checkedDirectoryPage(raw,project.projectId,path,null);
      if (workspaceTransitionRef.current!==token ||
          explorerIoTicket.current!==ticket ||
          !mayCommitProjectNavigation(request,result,{
            epoch:projectNavigationEpoch.current,
            liveProjectId:activeProjectIdRef.current,
            requestedProjectId:project.projectId,
          })) return;
      if (request.fromProjectId!==project.projectId) {
        const pane=paneSnapshotRef.current;
        const selection=planProjectSwitchView(tabsRef.current,
          request.fromProjectId,project.projectId,
          pane.primaryKey,pane.secondaryKey,pane.splitOpen,
          pane.tabWindowStart,projectEditorViews.current.get(project.projectId));
        if (!selection.allowed)
          throw Error('PROJECT_VIEW_RECONCILIATION_REFUSED');
        if (selection.leaving)
          projectEditorViews.current.set(request.fromProjectId,selection.leaving);
        setActiveTabKey(selection.primaryKey);
        setSecondaryTabKey(selection.secondaryKey);
        setSplitEditorOpen(selection.splitOpen);
        setTabWindowStart(selection.tabWindowStart);
      }
      setActiveProject(project);
      setDirectory(path);
      setListing(result);
      setSection('explorer');
      if (request.fromProjectId!==project.projectId) {
        setHint('Opened '+project.displayName+' with its own source tabs and pane layout. '+
          'Other projects and unsaved editor drafts remain available.');
      }
    } catch (error) {
      if (workspaceTransitionRef.current===token &&
          explorerIoTicket.current===ticket)
        setProblem('Project directory could not be opened: '+String(error));
    } finally {
      if (workspaceTransitionRef.current===token)
        workspaceTransitionRef.current=null;
      finishExplorerIo(ticket);
    }
  }

  function liveLspSourceForTab(tab) {
    return tab ? captureLiveLspSource(tab,
      bufferIdentities.current.get(tab.key),
      editorStates.current.get(tab.key),
      activeProjectIdRef.current) : null;
  }
  function isCurrentLspReply(source,response,epoch) {
    if (epoch!==lspLifecycleEpoch.current ||
        !lspNativeReplyMatches(source,response) ||
        (lspSessionRef.current &&
          response.providerId!==lspSessionRef.current.providerId)) return false;
    const latest=tabsRef.current.find(tab=>tab.key===source.key);
    return stillCurrentLspSource(source,latest,
      bufferIdentities.current.get(source.key),
      editorStates.current.get(source.key),
      activeProjectIdRef.current);
  }
  async function startLspSession() {
    const tab=activeTab;
    if (!tab || !lspAvailableForTab || lspSessionRef.current ||
        lspBusyRef.current || workspaceTransitionRef.current) return;
    const source=liveLspSourceForTab(tab);
    const request=lspRequestForSource(source);
    if (!request) {
      setProblem('Cannot start language service for a stale or oversized editor buffer.');
      return;
    }
    const epoch=++lspLifecycleEpoch.current;
    lspBusyRef.current=true;
    setLspBusy(true);
    setProblem('');
    let nativeStarted=false;
    try {
      const response=await invoke('ide_lsp_start_session',{request});
      nativeStarted=true;
      if (!isCurrentLspReply(source,response,epoch)) {
        // Native consent applies to the original tab snapshot only.
        // Do not publish an orphaned process or a stale diagnostic.
        await invoke('ide_lsp_stop_session');
        nativeStarted=false;
        throw Error('LSP_SOURCE_CHANGED_DURING_NATIVE_CONSENT');
      }
      const firstMember=sessionMemberForSource(source);
      const session=Object.freeze({
        projectId:source.projectId,relativePath:source.relativePath,
        languageId:source.languageId,bufferId:source.bufferId,
        savedRevision:source.savedRevision,key:source.key,
        providerId:response.providerId,
        members:Object.freeze({[source.key]:firstMember}),
      });
      lspSessionRef.current=session;
      setLspSession(session);
      setLspNativeDiagnostics(previous=>({...previous,[source.key]:response}));
      setHint('Protected '+source.languageId+' language session started for '+
        source.relativePath+'.');
    } catch(error) {
      if (nativeStarted) {
        try { await invoke('ide_lsp_stop_session'); }
        catch(cleanupError) {
          // Do not make an unsupported claim of native worker cleanup.
          lspSessionRef.current=Object.freeze({
            projectId:source.projectId,relativePath:source.relativePath,
            languageId:source.languageId,bufferId:source.bufferId,
            savedRevision:source.savedRevision,key:source.key,
            providerId:'cleanup_unverified',cleanupRequired:true,
            members:Object.freeze({}),
          });
          setLspSession(lspSessionRef.current);
          setProblem('Language worker cleanup requires manual retry: '+
            String(cleanupError));
          return;
        }
      }
      setProblem('Language service was not started: '+String(error));
    } finally {
      lspBusyRef.current=false;
      setLspBusy(false);
    }
  }

  async function addLspDocument() {
    const tab=activeTab;
    const session=lspSessionRef.current;
    if (!tab || !session || !lspMayAddCurrentFile ||
        lspBusyRef.current || workspaceTransitionRef.current ||
        activeProjectIdRef.current!==session.projectId) return false;
    const source=liveLspSourceForTab(tab);
    const member=sessionMemberForSource(source);
    const request=lspRequestForSource(source);
    if (!member || !request || !providerServesLanguage(session.providerId,source.languageId)) {
      setProblem('Cannot analyze a stale editor or an unsupported language in this session.');
      return false;
    }
    const epoch=lspLifecycleEpoch.current;
    ++lspFeatureSerial.current;
    ++lspSyncSerial.current;
    lspBusyRef.current=true;
    setLspBusy(true);
    let opened=false;
    try {
      const response=await invoke('ide_lsp_open_document',{request});
      opened=true;
      if (!isCurrentLspReply(source,response,epoch) ||
          lspSessionRef.current!==session) {
        throw Error('LSP_DOCUMENT_CHANGED_DURING_NATIVE_ADMISSION');
      }
      const revised=Object.freeze({...session,members:Object.freeze({
        ...session.members,[source.key]:member})});
      lspSessionRef.current=revised;
      setLspSession(revised);
      setLspNativeDiagnostics(previous=>({...previous,[source.key]:response}));
      setHint('Added '+source.relativePath+' to the existing protected language session.');
      return true;
    } catch(error) {
      if (opened) {
        try {
          await invoke('ide_lsp_close_document',{request:sessionCloseRequest(member)});
          opened=false;
        } catch(cleanupError) {
          const uncertain=Object.freeze({...session,cleanupRequired:true});
          lspSessionRef.current=uncertain;
          setLspSession(uncertain);
          setProblem('Protected worker needs cleanup after file admission: '+
            String(cleanupError));
          return false;
        }
      }
      setProblem('Could not add this file to the protected language session: '+String(error));
      return false;
    } finally {
      lspBusyRef.current=false;
      setLspBusy(false);
    }
  }
  async function closeLspDocument(key) {
    const session=lspSessionRef.current;
    const member=session?.members?.[key];
    if (!member) return true;
    if (lspBusyRef.current || session.cleanupRequired) {
      setProblem('Finish or stop the protected language operation before closing this file.');
      return false;
    }
    const request=sessionCloseRequest(member);
    if (!request) return false;
    lspBusyRef.current=true;
    const epoch=++lspLifecycleEpoch.current;
    ++lspFeatureSerial.current;
    ++lspSyncSerial.current;
    setLspBusy(true);
    try {
      await invoke('ide_lsp_close_document',{request});
      if (epoch!==lspLifecycleEpoch.current) return false;
      const current=lspSessionRef.current;
      if (!current || current.projectId!==member.projectId) return false;
      const members={...current.members};
      delete members[key];
      const revised=Object.freeze({...current,members:Object.freeze(members)});
      lspSessionRef.current=revised;
      setLspSession(revised);
      setLspNativeDiagnostics(previous=>{
        const next={...previous}; delete next[key]; return next;
      });
      setLspCompletions(null);
      setLspHover(null);
      return true;
    } catch(error) {
      // Do not assume a failed didClose released the native source or worker.
      const uncertain=Object.freeze({...session,cleanupRequired:true});
      lspSessionRef.current=uncertain;
      setLspSession(uncertain);
      setProblem('Protected document close was not confirmed. Stop the worker: '+String(error));
      return false;
    } finally {
      lspBusyRef.current=false;
      setLspBusy(false);
    }
  }
  async function stopLspSession() {
    if (!lspSessionRef.current || lspBusyRef.current) return false;
    lspBusyRef.current=true;
    const epoch=++lspLifecycleEpoch.current;
    ++lspSyncSerial.current;
    ++lspFeatureSerial.current;
    setLspBusy(true);
    try {
      await invoke('ide_lsp_stop_session');
      if (epoch!==lspLifecycleEpoch.current) return false;
      lspSessionRef.current=null;
      setLspSession(null);
      setLspNativeDiagnostics({});
      setLspCompletions(null);
      setLspHover(null);
      setLspJumpRequest(null);
      setHint('Protected language-server session stopped.');
      return true;
    } catch(error) {
      // A failed stop is not proof the worker exited; retain the ability to
      // retry, and do not start a second native worker over the old one.
      setProblem('Language-server cleanup is unverified. Retry Stop: '+String(error));
      return false;
    } finally {
      lspBusyRef.current=false;
      setLspBusy(false);
    }
  }
  useEffect(() => {
    const session=lspSessionRef.current;
    if (!session || lspBusyRef.current || session.cleanupRequired) return;
    if (session.projectId!==activeProject?.projectId) {
      void stopLspSession();
      return;
    }
    // The session is project-owned, not tab-owned: switching between tabs
    // never stops the OS worker. A replaced, saved or closed member does
    // lose its own document identity and must be detached.
    const invalid=Object.values(session.members||{}).find(member=>{
      const tab=tabsRef.current.find(item=>item.key===member.key);
      return !sourceMatchesSession(liveLspSourceForTab(tab),session);
    });
    if (invalid) void closeLspDocument(invalid.key);
  },[lspSession,tabs,activeProject?.projectId,lspBusy]);
  useEffect(() => {
    const session=lspSessionRef.current;
    const source=liveLspSourceForTab(activeTab);
    if (!sourceMatchesSession(source,session) ||
        lspBusyRef.current || session?.cleanupRequired) return;
    const epoch=lspLifecycleEpoch.current;
    const serial=++lspSyncSerial.current;
    const timer=setTimeout(async()=>{
      if (epoch!==lspLifecycleEpoch.current ||
          serial!==lspSyncSerial.current ||
          !sourceMatchesSession(liveLspSourceForTab(
            tabsRef.current.find(tab=>tab.key===source.key)),lspSessionRef.current))
        return;
      const request=lspRequestForSource(source);
      try {
        const changed=await invoke('ide_lsp_change_document',{request});
        if (!isCurrentLspReply(source,changed,epoch) ||
            serial!==lspSyncSerial.current) return;
        const result=await invoke('ide_lsp_read_diagnostics',{request});
        if (!isCurrentLspReply(source,result,epoch) ||
            serial!==lspSyncSerial.current ||
            !sourceMatchesSession(source,lspSessionRef.current)) return;
        setLspNativeDiagnostics(previous=>({...previous,[source.key]:result}));
      } catch(error) {
        if (epoch!==lspLifecycleEpoch.current ||
            serial!==lspSyncSerial.current) return;
        if (String(error).includes('LSP_STALE') ||
            String(error).includes('LSP_BUFFER_REVISION') ||
            String(error).includes('LSP_SOURCE_SCOPE_MISMATCH')) return;
        setProblem('Language service could not synchronize this editor: '+String(error));
      }
    },400);
    return ()=>clearTimeout(timer);
  },[lspSession,activeTab?.key,activeTab?.bufferRevision,
    activeTab?.revision,activeTab?.generation]);

  async function navigateProtectedCrossFileDefinition(source,response,epoch,serial) {
    const fromTab=tabsRef.current.find(tab=>tab.key===source.key);
    const proposed=visibleCrossFileDefinition({...fromTab,
      draft:source.text,bufferRevision:source.bufferRevision},response);
    if (!proposed) return false;
    if (explorerIoTicket.current || workspaceTransitionRef.current ||
        mergeBusy || mergeApplyRequest) {
      setProblem('Finish the current editor operation before navigating to a definition.');
      return true;
    }
    const projectId=source.projectId;
    const key=keyFor(projectId,proposed.relativePath);
    if (!admitOpenTabs(tabsRef.current,projectId,[key]).allowed) {
      setProblem('Open-file budget reached. Close an unused tab before Go to Definition.');
      return true;
    }
    const ticket=beginExplorerIo();
    if (!ticket) return true;
    const navEpoch=projectNavigationEpoch.current;
    try {
      // The server's URI never reaches this IPC. Native has already bound
      // the target to this registered project and the sealed worker snapshot.
      // Re-read exact saved bytes to close disk-change and project-switch races.
      const opened=await invoke('ide_preview_source_file',{
        projectId,relativePath:proposed.relativePath,
      });
      if (explorerIoTicket.current!==ticket ||
          projectNavigationEpoch.current!==navEpoch ||
          activeProjectIdRef.current!==projectId ||
          serial!==lspFeatureSerial.current ||
          !isCurrentLspReply(source,response,epoch) ||
          !sourceMatchesSession(source,lspSessionRef.current)) return true;
      if (opened.projectId!==projectId ||
          opened.relativePath!==proposed.relativePath ||
          opened.revision!==proposed.diskSha256)
        throw Error('DEFINITION_TARGET_CHANGED_ON_DISK');
      const from=lspPositionOffset(opened.text,proposed.start);
      const to=lspPositionOffset(opened.text,proposed.end);
      if (from===null || to===null || from>to)
        throw Error('DEFINITION_RANGE_OUTSIDE_REGISTERED_SOURCE');
      const existing=tabsRef.current.find(tab=>tab.key===key);
      const admission=lateSourceOpenDecision(tabsRef.current,projectId,
        proposed.relativePath,opened,activeProjectIdRef.current,key);
      if (admission.action==='discard_stale') return true;
      if (admission.action==='deny') throw Error('DEFINITION_OPEN_FILE_BUDGET');
      const live=existing ? captureLiveEditorIntent(existing,
        bufferIdentities.current.get(key),editorStates.current.get(key),projectId)
        : null;
      // Saved-file UTF-16 coordinates are never applied to an unsaved tab,
      // even when React's dirty flag has not caught up with CodeMirror.
      if (existing && (live?.status!=='current' || live.dirty ||
          live.text!==opened.text || existing.revision!==opened.revision ||
          existing.savedText!==opened.text)) {
        setProblem('Definition refers to saved source, but the target tab has '+
          'unsaved or newer edits. Its draft and Undo history were preserved.');
        return true;
      }
      const editorState=existing ? editorStates.current.get(key)
        : createEditorState(opened.text,opened.relativePath,
            (state,draft)=>onEditorChange(key,state,draft),
            state=>onEditorSelection(key,state));
      if (!editorState || editorState.sliceDoc()!==opened.text ||
          to>editorState.doc.length) throw Error('DEFINITION_EDITOR_CHANGED');
      if (!existing) {
        clearPrivateDraftSession(key);
        resetBufferIdentity(key,projectId,opened.relativePath,opened.revision);
        editorStates.current.set(key,editorState);
        setTabs(previous=>[...previous,{
          key,projectId,relativePath:opened.relativePath,
          revision:opened.revision,savedText:opened.text,draft:opened.text,
          dirty:false,generation:0,bufferRevision:0,
        }]);
      }
      if (existing) selectPrimaryEditorTab(key);
      else setActiveTabKey(key);
      setSection('explorer');
      setLspJumpRequest({from,to,tabIdentity:key+':'+(existing?.generation||0),
        nonce:++problemJumpNonce.current});
      if (!existing) void checkExistingDraft({key,projectId,
        relativePath:opened.relativePath});
      setHint('Opened verified definition in '+proposed.relativePath+'.');
    } catch(error) {
      if (activeProjectIdRef.current===projectId &&
          serial===lspFeatureSerial.current)
        setProblem('Definition navigation refused: '+String(error));
    } finally { finishExplorerIo(ticket); }
    return true;
  }

  async function requestLspFeature(kind) {
    const tab=activeTab;
    if (!tab || !['completion','hover','definition','diagnostics'].includes(kind) ||
        lspBusyRef.current) return;
    const source=liveLspSourceForTab(tab);
    if (!sourceMatchesSession(source,lspSessionRef.current) ||
        lspSessionRef.current?.cleanupRequired) return;
    if (!languageMethodAvailable(source.languageId,kind,lspWorker) ||
        (source.languageId==='python' && kind!=='diagnostics' &&
          lspSessionRef.current?.providerId!=='pyright')) {
      setHint('The protected '+source.languageId+' provider does not offer '+
        kind+' for this file. No unapproved language server will start.');
      return;
    }
    const position=cursorPositions[source.key] || {line:0,character:0};
    const request=lspRequestForSource(source,kind,position);
    if (!request) {
      setProblem('Language request rejected: editor cursor or source changed.');
      return;
    }
    const epoch=lspLifecycleEpoch.current;
    const serial=++lspFeatureSerial.current;
    // Cross-file semantics must observe *every* attached CodeMirror buffer
    // at its exact revision, not just the currently selected editor.
    // This is a read-only sync through the already supervised, mutex-owned
    // worker. The source checks after every awaited IPC catch later edits.
    const session=lspSessionRef.current;
    const sources=Object.values(session.members||{}).map(member=>{
      const tab=tabsRef.current.find(item=>item.key===member.key);
      return liveLspSourceForTab(tab);
    });
    if (sources.some(item=>!sourceMatchesSession(item,session))) {
      setProblem('An analyzed file was replaced or closed. Refresh the language session.');
      return;
    }
    try {
      for (const item of sources) {
        const sync=lspRequestForSource(item);
        if (!sync) throw Error('LSP_MEMBER_SOURCE_UNAVAILABLE');
        const synced=await invoke('ide_lsp_change_document',{request:sync});
        if (serial!==lspFeatureSerial.current ||
            !isCurrentLspReply(item,synced,epoch) ||
            !sourceMatchesSession(item,lspSessionRef.current)) return;
      }
      // The worker never answers a multi-file feature against drafts changed
      // during the synchronization sequence.
      if (sources.some(item=>{
        const tab=tabsRef.current.find(candidate=>candidate.key===item.key);
        return !stillCurrentLspSource(item,tab,
          bufferIdentities.current.get(item.key),
          editorStates.current.get(item.key),activeProjectIdRef.current);
      })) return;
      const response=await invoke(kind==='diagnostics' ?
        'ide_lsp_read_diagnostics':'ide_lsp_feature',{request});
      if (serial!==lspFeatureSerial.current ||
          !isCurrentLspReply(source,response,epoch) ||
          !sourceMatchesSession(source,lspSessionRef.current) ||
          sources.some(item=>{
            const tab=tabsRef.current.find(candidate=>candidate.key===item.key);
            return !stillCurrentLspSource(item,tab,
              bufferIdentities.current.get(item.key),
              editorStates.current.get(item.key),activeProjectIdRef.current);
          })) return;
      if (kind==='diagnostics') {
        setLspNativeDiagnostics(previous=>({...previous,[source.key]:response}));
      } else if (kind==='completion') {
        setLspCompletions({tabKey:source.key,source,response});
      } else if (kind==='hover') {
        setLspHover({tabKey:source.key,source,response});
      } else if (kind==='definition') {
        const latest=tabsRef.current.find(item=>item.key===source.key);
        const target=visibleDefinition({...latest,draft:source.text,
          bufferRevision:source.bufferRevision},response);
        if (target) {
          setLspJumpRequest({...target,
            tabIdentity:latest.key+':'+latest.generation,
            nonce:++problemJumpNonce.current});
        } else if (!await navigateProtectedCrossFileDefinition(
          source,response,epoch,serial)) {
          setHint('No verified definition found in this registered project.');
        }
      }
    } catch(error) {
      if (epoch!==lspLifecycleEpoch.current ||
          serial!==lspFeatureSerial.current) return;
      setProblem('Protected language service unavailable: '+String(error));
    }
  }
  function applyLspCompletion(item) {
    const tab=activeTab;
    const source=lspCompletions?.source;
    if (!tab || !source || !sourceMatchesSession(source,lspSessionRef.current) ||
        !stillCurrentLspSource(source,tab,
          bufferIdentities.current.get(tab.key),
          editorStates.current.get(tab.key),activeProjectIdRef.current) ||
        !activeLspCompletions.some(choice=>choice===item)) return;
    setLspCompletionApplyRequest({
      tabIdentity:tab.key+':'+tab.generation,
      expectedDraft:source.text,insertText:item.insertText,
      replaceFrom:item.replaceFrom,replaceTo:item.replaceTo,
      nonce:++problemJumpNonce.current,
    });
    setLspCompletions(null);
  }

  function onEditorChange(key, state, draft) {
    editorStates.current.set(key, state);
    // Editing invalidates any prior merge agreement immediately. Applying
    // an approved merge itself also clears the old review, not its callback.
    setMergeReview(previous=>previous?.tabKey===key ? null : previous);
    // Any earlier external comparison described a different editor revision.
    setDiskComparison(previous=>previous?.tabKey===key ? null : previous);
    setDiskAuditReport(previous=>previous ? {
      ...previous,
      rows:previous.rows.map(row=>row.key===key ?
        {...row,status:'stale',reason:'Draft edited since inspection'} : row),
    } : previous);
    const previousBuffer = bufferIdentities.current.get(key);
    if (previousBuffer) {
      bufferIdentities.current.set(key,advanceBufferIdentity(previousBuffer));
      scheduleDraftPreserve(key);
    }
    setReview(previous => previous?.tabKey === key ? null : previous);
    setTabs(previous => previous.map(tab =>
      tab.key === key ? { ...tab, draft, dirty: draft !== tab.savedText,
        bufferRevision: (tab.bufferRevision ?? 0) + 1 } : tab));
    // Native diagnostics and feature results describe the previous exact
    // CodeMirror revision. Hide them immediately, before React catches up.
    setLspNativeDiagnostics(previous=>{
      if(!Object.prototype.hasOwnProperty.call(previous,key)) return previous;
      const next={...previous};delete next[key];return next;
    });
    ++lspFeatureSerial.current;
    setLspCompletions(null);
    setLspHover(null);
  }

  async function runProjectSearch() {
    const projectId = activeProject?.projectId;
    if (!projectId || !projectSearchQuery.trim() || projectSearchBusy) return;
    const generation = ++projectSearchGeneration.current;
    setProjectSearchBusy(true);
    setProjectSearchResults(null);
    setProblem('');
    try {
      const results = await invoke('ide_find_in_project', {
        projectId, query: projectSearchQuery, caseSensitive: projectSearchCase,
      });
      // Project/session changes supersede this response, never reassign it.
      if (generation !== projectSearchGeneration.current ||
          activeProjectIdRef.current !== projectId) return;
      if (results.projectId !== projectId || !Array.isArray(results.matches))
        throw new Error('PROJECT_SEARCH_RESULT_NOT_SCOPED');
      setProjectSearchResults(results);
    } catch (error) {
      if (generation === projectSearchGeneration.current &&
          activeProjectIdRef.current === projectId)
        setProblem('Could not search project: ' + String(error));
    } finally {
      if (generation === projectSearchGeneration.current)
        setProjectSearchBusy(false);
    }
  }

  async function openProjectSearchMatch(match) {
    const projectId = activeProject?.projectId;
    if (!projectId || explorerPending || projectSearchBusy ||
        workspaceTransitionRef.current ||
        projectSearchResults?.projectId !== projectId) return;
    const generation = projectSearchGeneration.current;
    const navigationEpoch=projectNavigationEpoch.current;
    const key = keyFor(projectId, match.relativePath);
    if (!admitOpenTabs(tabsRef.current,projectId,[key]).allowed) {
      setProblem('Open-file limit ('+MAX_OPEN_TABS_PER_PROJECT+
        ') reached. Close an unused tab; saved-source search is unchanged.');
      return;
    }
    const ticket = beginExplorerIo();
    if (!ticket) return;
    setProblem('');
    try {
      // Never trust a search hit alone: re-read and rehash saved disk bytes.
      const opened = await invoke('ide_preview_source_file', {
        projectId, relativePath: match.relativePath,
      });
      if (explorerIoTicket.current !== ticket ||
          activeProjectIdRef.current !== projectId ||
          navigationEpoch!==projectNavigationEpoch.current ||
          generation !== projectSearchGeneration.current)
        return; // A different project, read or search took over.
      if (opened.projectId !== projectId || opened.relativePath !== match.relativePath ||
          opened.revision !== match.revision)
        throw new Error('FILE_CHANGED_ON_DISK: search again');
      const existing = tabsRef.current.find(tab => tab.key === key);
      const admission = lateSourceOpenDecision(tabsRef.current,projectId,
        match.relativePath,opened,activeProjectIdRef.current,key);
      if (admission.action === 'discard_stale') return;
      if (admission.action === 'deny') {
        setProblem('Open-file limit reached while loading this result; existing drafts preserved.');
        return;
      }
      // Search offsets refer to the persisted disk snapshot. A newer live
      // CodeMirror edit can precede React's dirty flag: never apply saved
      // coordinates to that different in-memory buffer.
      const existingLive = existing ? captureLiveEditorIntent(existing,
        bufferIdentities.current.get(key),editorStates.current.get(key),projectId)
        : null;
      if (existing && (existingLive.status !== 'current' || existingLive.dirty ||
                       existing.revision !== opened.revision ||
                       existing.savedText !== opened.text)) {
        selectPrimaryEditorTab(key);
        setSection('explorer');
        setProblem('Search refers to saved disk bytes. The existing editor draft differs. ' +
          'No draft was changed and no stale coordinates were applied.');
        return;
      }
      const state = existing
        ? editorStates.current.get(key)
        : createEditorState(opened.text, opened.relativePath,
            (editorState, draft) => onEditorChange(key, editorState, draft),
            state => onEditorSelection(key,state));
      if (!state) throw new Error('EDITOR_STATE_UNAVAILABLE');
      const line = state.doc.line(match.line);
      const from = line.from + match.columnUtf16 - 1;
      const to = from + match.matchLengthUtf16;
      if (from < line.from || to > line.to || to <= from ||
          state.doc.sliceString(from, to).length !== match.matchLengthUtf16)
        throw new Error('SEARCH_RANGE_UNVERIFIABLE');
      if (!existing) {
        clearPrivateDraftSession(key);
        resetBufferIdentity(key,projectId,opened.relativePath,opened.revision);
        editorStates.current.set(key, state);
        setTabs(previous => [...previous, {
          key, projectId, relativePath: opened.relativePath,
          revision: opened.revision, savedText: opened.text,
          draft: opened.text, dirty: false, generation: 0,
          bufferRevision: 0,
        }]);
      }
      if (existing) selectPrimaryEditorTab(key);
      else setActiveTabKey(key);
      if (!existing) void checkExistingDraft({key,projectId,
        relativePath:opened.relativePath});
      setFindRequest({ tabKey: key, from, to,
        nonce: ++projectSearchOpenNonce.current });
      setSection('explorer');
    } catch (error) {
      if (activeProjectIdRef.current === projectId &&
          generation === projectSearchGeneration.current)
        setProblem('Cannot open that saved search hit: ' + String(error));
    } finally {
      finishExplorerIo(ticket);
    }
  }

  useEffect(()=>{
    // Ctrl/Cmd+P is navigation, not printing and not a build command.
    // Focus the exact active project's filename finder without running it.
    const onQuickOpenShortcut=event=>{
      if ((event.ctrlKey||event.metaKey)&&!event.shiftKey&&!event.altKey &&
          event.key?.toLowerCase()==='p' && activeProjectIdRef.current) {
        event.preventDefault();
        event.stopPropagation();
        setSection('explorer');
        setQuickOpenFocusRequest(previous=>previous+1);
      }
    };
    window.addEventListener('keydown',onQuickOpenShortcut,true);
    return ()=>window.removeEventListener('keydown',onQuickOpenShortcut,true);
  },[]);
  useEffect(()=>{
    if (section==='explorer'&&quickOpenFocusRequest>0)
      quickOpenInputRef.current?.focus();
  },[section,quickOpenFocusRequest]);

  async function searchQuickOpen() {
    const projectId=activeProjectIdRef.current;
    const query=quickOpenQuery.trim();
    if (!projectId || !query || query.length>128 ||
        quickOpenBusyRef.current || explorerIoTicket.current ||
        workspaceTransitionRef.current || workspaceLayoutOp.current) return;
    const serial=++quickOpenSerial.current;
    const epoch=projectNavigationEpoch.current;
    quickOpenBusyRef.current=true;
    setQuickOpenBusy(true);
    setQuickOpenResults(null);
    setProblem('');
    try {
      const raw=await invoke('ide_quick_open_source_paths',{projectId,query});
      if (serial!==quickOpenSerial.current ||
          activeProjectIdRef.current!==projectId ||
          epoch!==projectNavigationEpoch.current) return;
      const result=checkedQuickOpenResult(raw,projectId,query);
      setQuickOpenResults(result);
      if (result.truncated)
        setHint('Quick Open reached a bounded scan or result limit. Narrow the filename; results are partial.');
    } catch(error) {
      if (serial===quickOpenSerial.current &&
          activeProjectIdRef.current===projectId)
        setProblem('Could not search project filenames: '+String(error));
    } finally {
      if (serial===quickOpenSerial.current) {
        quickOpenBusyRef.current=false;
        setQuickOpenBusy(false);
      }
    }
  }

  async function openQuickOpenMatch(hit) {
    if (!activeProject?.projectId || !quickOpenResults ||
        quickOpenResults.projectId!==activeProject.projectId ||
        !quickOpenResults.matches.some(entry=>entry.relativePath===hit?.relativePath) ||
        hit.size>512*1024 ||
        explorerIoTicket.current || workspaceTransitionRef.current) return;
    // This is just a source-relative filename suggestion. Reuse the exact
    // native preview + late-admission path; no filename search grants read/write.
    await selectEntry({kind:'file',relativePath:hit.relativePath,
      name:hit.relativePath.split('/').pop()});
  }

  async function loadNextSourceDirectoryPage() {
    const projectId=activeProjectIdRef.current;
    const current=listing;
    if (!projectId || !current || current.projectId!==projectId ||
        !current.nextAfter || directoryPageBusy || explorerIoTicket.current ||
        workspaceTransitionRef.current || workspaceLayoutOp.current) return;
    const ticket=beginExplorerIo();
    if (!ticket) return;
    const epoch=projectNavigationEpoch.current;
    const after=current.nextAfter;
    setDirectoryPageBusy(true);
    setProblem('');
    try {
      const raw=await invoke('ide_list_source_directory_page',{
        projectId,relativeDir:current.relativeDir,after,
      });
      if (explorerIoTicket.current!==ticket ||
          epoch!==projectNavigationEpoch.current ||
          activeProjectIdRef.current!==projectId) return;
      const page=checkedDirectoryPage(raw,projectId,current.relativeDir,after);
      if (!page.entries.length)
        throw Error('INT01E_EMPTY_DIRECTORY_PAGE');
      setListing(previous=>{
        if (previous?.projectId!==projectId ||
            previous.relativeDir!==current.relativeDir ||
            previous.nextAfter!==after) return previous;
        return {...previous,
          entries:[...previous.entries,...page.entries],
          nextAfter:page.nextAfter,truncated:page.truncated,
          scanLimited:page.scanLimited,
          entriesExamined:page.entriesExamined};
      });
      setVisibleTreeCount(previous=>previous+page.entries.length);
    } catch(error) {
      if (explorerIoTicket.current===ticket)
        setProblem('Additional directory entries unavailable: '+String(error));
    } finally {
      setDirectoryPageBusy(false);
      finishExplorerIo(ticket);
    }
  }

  async function openRelativeSourcePath() {
    const path=verifiedRelativeSourcePath(relativeOpenPath);
    if (!activeProject || !path) {
      setProblem('Enter a valid project-relative source file path. '+
        'No absolute or parent-relative paths are allowed.');
      return;
    }
    // Uses the existing native picked-directory read, even if this file was
    // absent from the 400-entry partial directory listing. No ambient paths.
    await selectEntry({kind:'file',relativePath:path,name:path.split('/').pop()});
  }

  async function selectEntry(entry) {
    if (!activeProject || explorerPending || explorerIoTicket.current ||
        workspaceTransitionRef.current) return;
    if (entry.kind === 'directory') {
      await openDirectory(activeProject, entry.relativePath);
    } else if (entry.kind === 'file') {
      const projectId = activeProject.projectId;
      const key = keyFor(projectId, entry.relativePath);
      if (tabsRef.current.some(tab => tab.key === key)) {
        selectPrimaryEditorTab(key);
        return;
      }
      const tabAdmission=admitOpenTabs(tabsRef.current,projectId,[key]);
      if (!tabAdmission.allowed) {
        setProblem('Open-file budget ('+MAX_OPEN_TABS_PER_PROJECT+
          ') reached. Close an unused file to preserve existing drafts and Undo.');
        return;
      }
      const ticket = beginExplorerIo();
      if (!ticket) return;
      const navigationEpoch=projectNavigationEpoch.current;
      setProblem('');
      try {
        const opened = await invoke('ide_preview_source_file', {
          projectId, relativePath: entry.relativePath,
        });
        if (explorerIoTicket.current !== ticket ||
            navigationEpoch!==projectNavigationEpoch.current) return;
        const decision = lateSourceOpenDecision(tabsRef.current,projectId,
          entry.relativePath,opened,activeProjectIdRef.current,key);
        if (decision.action === 'discard_stale') return;
        if (decision.action === 'select_existing') {
          selectPrimaryEditorTab(key); // Keep pane ownership and its Undo state.
          return;
        }
        if (decision.action === 'deny') {
          setProblem('Open-file limit reached during read. No draft or Undo history was evicted.');
          return;
        }
        const newTab = {
          key, projectId: opened.projectId,
          relativePath: opened.relativePath, revision: opened.revision,
          savedText: opened.text, draft: opened.text,
          dirty: false, generation: 0, bufferRevision: 0,
        };
        clearPrivateDraftSession(key);
        resetBufferIdentity(key,opened.projectId,opened.relativePath,opened.revision);
        editorStates.current.set(key, createEditorState(
          opened.text, opened.relativePath,
          (state, draft) => onEditorChange(key, state, draft),
          state => onEditorSelection(key,state)
        ));
        setTabs(previous => [...previous, newTab]);
        setActiveTabKey(key);
        void checkExistingDraft(newTab);
      } catch (error) {
        if (explorerIoTicket.current === ticket) setProblem(String(error));
      } finally { finishExplorerIo(ticket); }
    }
  }

  async function rememberWorkspaceForRestart() {
    const projectId=activeProjectIdRef.current;
    if (!projectId || mergeBusy || mergeApplyRequest ||
        workspaceLayoutOp.current || workspaceTransitionRef.current ||
        explorerPending || explorerIoTicket.current ||
        savingKey || savingKeyRef.current || draftRecoveryBusy) return;
    let layout;
    try {
      layout=rememberedWorkspaceFromTabs(tabsRef.current,projectId,
        activeTabKey,secondaryTabKey,splitEditorOpen,tabWindowStart,
        key=>editorStates.current.get(key)?.sliceDoc());
    } catch(error) {
      setProblem('Cannot remember workspace: '+String(error));
      return;
    }
    if (layout.files.some(file=>file.wasDirty) && !window.confirm(
      'Remember file tabs and pane layout only? Unsaved edits are NOT stored in '+ 
      'this layout. Use Preserve draft for private recovery before a restart.'))
      return;
    const token=Symbol('b9-remember');
    workspaceLayoutOp.current=token;
    setWorkspaceLayoutBusy(true);
    setProblem('');
    try {
      const stored=await invoke('ide_workspace_layout_save',{layout});
      if (workspaceLayoutOp.current!==token ||
          activeProjectIdRef.current!==projectId) return;
      workspaceLayoutReadVersion.current++;
      setRememberedLayout(checkedRememberedWorkspace(stored,projectId));
      setWorkspaceRecoveryReport(null);
      setHint('Remembered '+stored.files.length+
        ' file paths and pane positions privately. No source or unsaved text was saved.');
    } catch(error) {
      if (workspaceLayoutOp.current===token)
        setProblem('Could not remember workspace: '+String(error));
    } finally {
      if (workspaceLayoutOp.current===token) {
        workspaceLayoutOp.current=null;
        setWorkspaceLayoutBusy(false);
      }
    }
  }

  async function forgetRememberedWorkspace() {
    const projectId=activeProjectIdRef.current;
    if (!projectId || workspaceLayoutOp.current ||
        workspaceTransitionRef.current || !rememberedLayout ||
        !window.confirm('Forget the saved tab layout? Source files and B2 private drafts stay intact.'))
      return;
    const token=Symbol('b9-forget');
    workspaceLayoutOp.current=token;
    setWorkspaceLayoutBusy(true);
    try {
      await invoke('ide_workspace_layout_clear',{projectId});
      if (workspaceLayoutOp.current===token &&
          activeProjectIdRef.current===projectId) {
        workspaceLayoutReadVersion.current++;
        setRememberedLayout(null);
        setWorkspaceRecoveryReport(null);
        setHint('Forgot saved tab navigation only. Editor drafts and project files were kept.');
      }
    } catch(error) {
      if (workspaceLayoutOp.current===token)
        setProblem('Cannot forget remembered layout: '+String(error));
    } finally {
      if (workspaceLayoutOp.current===token) {
        workspaceLayoutOp.current=null;
        setWorkspaceLayoutBusy(false);
      }
    }
  }

  async function resumeRememberedWorkspace() {
    const projectId=activeProjectIdRef.current;
    if (!projectId || !rememberedLayout || mergeBusy || mergeApplyRequest ||
        workspaceLayoutOp.current || workspaceTransitionRef.current ||
        explorerPending || explorerIoTicket.current || savingKey ||
        savingKeyRef.current || draftRecoveryBusy || reviewPending || inboxBusy) return;
    let layout;
    try { layout=checkedRememberedWorkspace(rememberedLayout,projectId); }
    catch(error) {
      setProblem('Remembered workspace cannot be trusted: '+String(error));
      return;
    }
    // Persisted navigation contains no source/drafts. Reopening even the
    // saved version of a formerly dirty file needs an explicit decision.
    if (layout.files.some(file=>file.wasDirty) && !window.confirm(
      'This saved layout included unsaved editor files. Reopen only their '+
      'current SAVED disk versions and separately inspect private draft '+
      'checkpoints? Existing unsaved editor buffers will not be replaced.'))
      return;
    const ticket=beginExplorerIo();
    if (!ticket) return;
    const token=Symbol('ide-int01d-restart-reconciliation');
    workspaceLayoutOp.current=token;
    workspaceTransitionRef.current=token;
    const originalEpoch=projectNavigationEpoch.current;
    const initialWitness=projectEditorWitness(projectId);
    setWorkspaceLayoutBusy(true);
    setWorkspaceRecoveryReport(null);
    setProblem('');
    const observations=[];
    try {
      // Each path is verified through native registered-project source I/O.
      // An unreadable source is reported rather than recreated from a draft.
      for (const entry of layout.files) {
        let preview=null, draft=null, draftUnavailable=false;
        try {
          preview=await invoke('ide_preview_source_file',{
            projectId,relativePath:entry.relativePath,
          });
        } catch {
          // Deletion, permission denial, binary/large file, etc. are not
          // permission to repair or write any file.
        }
        if (workspaceLayoutOp.current!==token ||
            workspaceTransitionRef.current!==token ||
            explorerIoTicket.current!==ticket ||
            projectNavigationEpoch.current!==originalEpoch ||
            activeProjectIdRef.current!==projectId) return;
        if (preview?.projectId===projectId &&
            preview.relativePath===entry.relativePath &&
            typeof preview.text==='string' &&
            /^[a-f0-9]{64}$/i.test(preview.revision||'')) {
          try {
            const result=await invoke('ide_draft_latest',{
              projectId,relativePath:entry.relativePath,
            });
            draft=checkedPrivateDraftHead(result,projectId,entry.relativePath);
          } catch {
            draftUnavailable=true;
          }
        }
        if (workspaceLayoutOp.current!==token ||
            workspaceTransitionRef.current!==token ||
            explorerIoTicket.current!==ticket ||
            projectNavigationEpoch.current!==originalEpoch ||
            activeProjectIdRef.current!==projectId) return;
        observations.push({
          relativePath:entry.relativePath,preview,draft,draftUnavailable,
        });
      }
      const plan=planRestartReconciliation(layout,observations,tabsRef.current,
        key=>bufferIdentities.current.get(key),
        key=>editorStates.current.get(key));
      // Before any UI tab admission, verify that all preexisting live source
      // buffers are still the same. Another transaction cannot silently win.
      if (projectEditorWitnessChanged(projectId,initialWitness))
        throw Error('INT01D_EDITOR_CHANGED_DURING_RESTART_RECONCILIATION');
      const freshPreviews=plan.toOpen;
      if (!admitOpenTabs(tabsRef.current,projectId,
          freshPreviews.map(file=>keyFor(projectId,file.relativePath))).allowed)
        throw Error('INT01D_TAB_BUDGET: close unused tabs before reopening saved files');
      // Show disk drift BEFORE opening anything; the user can decline while
      // keeping the source-free summary available for a later decision.
      if (plan.summary.changedOnDisk && !window.confirm(
        plan.summary.changedOnDisk+' remembered file(s) changed on disk. '+
        'Open their CURRENT saved versions without restoring or overwriting '+
        'any old draft? No project files will be modified.')) {
        setWorkspaceRecoveryReport({projectId,rows:plan.rows,
          summary:plan.summary,restoredCount:0,action:'inspection_only'});
        setHint('Inspected remembered files. No editor tabs were reopened.');
        return;
      }
      if (workspaceLayoutOp.current!==token ||
          workspaceTransitionRef.current!==token ||
          explorerIoTicket.current!==ticket ||
          projectNavigationEpoch.current!==originalEpoch ||
          activeProjectIdRef.current!==projectId ||
          projectEditorWitnessChanged(projectId,initialWitness)) return;
      // A file may change after the first preview. Re-read every proposed
      // new editor file immediately before accepting its saved snapshot.
      for (const opened of freshPreviews) {
        const latest=await invoke('ide_preview_source_file',{
          projectId,relativePath:opened.relativePath,
        });
        if (workspaceLayoutOp.current!==token ||
            workspaceTransitionRef.current!==token ||
            explorerIoTicket.current!==ticket ||
            projectNavigationEpoch.current!==originalEpoch ||
            activeProjectIdRef.current!==projectId ||
            projectEditorWitnessChanged(projectId,initialWitness)) return;
        if (latest?.projectId!==projectId ||
            latest.relativePath!==opened.relativePath ||
            latest.revision!==opened.revision || latest.text!==opened.text)
          throw Error('INT01D_SOURCE_CHANGED_DURING_RESTART');
      }
      if (projectEditorWitnessChanged(projectId,initialWitness))
        throw Error('INT01D_EDITOR_CHANGED_DURING_RESTART_RECONCILIATION');
      // Create only verified clean CodeMirror states. The separate B2 vault
      // remains unchanged; recovery below is an explicit second user action.
      const freshTabs=freshPreviews.map(opened=>{
        const key=keyFor(projectId,opened.relativePath);
        const tab={key,projectId,relativePath:opened.relativePath,
          revision:opened.revision,savedText:opened.text,draft:opened.text,
          dirty:false,generation:0,bufferRevision:0};
        clearPrivateDraftSession(key);
        resetBufferIdentity(key,projectId,opened.relativePath,opened.revision);
        editorStates.current.set(key,createEditorState(
          opened.text,opened.relativePath,
          (state,draft)=>onEditorChange(key,state,draft),
          state=>onEditorSelection(key,state)));
        return tab;
      });
      const visible=[...tabsRef.current,...freshTabs];
      const rememberedView={
        schemaVersion:1,projectId,
        primaryKey:layout.primaryPath?keyFor(projectId,layout.primaryPath):null,
        secondaryKey:layout.secondaryPath?keyFor(projectId,layout.secondaryPath):null,
        splitOpen:layout.splitOpen,tabWindowStart:layout.tabWindowStart,
      };
      const focus=restoreProjectView(visible,projectId,rememberedView);
      projectEditorViews.current.set(projectId,rememberedView);
      if (freshTabs.length) setTabs(previous=>{
        const keys=new Set(previous.map(tab=>tab.key));
        return [...previous,...freshTabs.filter(tab=>!keys.has(tab.key))];
      });
      // Metadata only: do not copy checkpoint text into the report or editor.
      // A later explicit Recover action independently loads verified bytes.
      const metadata={};
      for (const observation of observations) {
        if (!observation.draft) continue;
        const key=keyFor(projectId,observation.relativePath);
        const activeHead=draftHeadByTab.current.get(key);
        if (activeHead && activeHead!==observation.draft.checkpointId)
          continue; // Another live draft checkpoint superseded this lookup.
        draftHeadByTab.current.set(key,observation.draft.checkpointId);
        metadata[key]=observation.draft;
      }
      if (Object.keys(metadata).length)
        setDraftRecoveryByTab(previous=>({...previous,...metadata}));
      setActiveTabKey(focus.primaryKey);
      setSecondaryTabKey(focus.secondaryKey);
      setSplitEditorOpen(focus.splitOpen);
      setTabWindowStart(focus.tabWindowStart);
      setSection('explorer');
      projectNavigationEpoch.current+=1;
      setWorkspaceRecoveryReport({projectId,rows:plan.rows,
        summary:plan.summary,restoredCount:freshTabs.length,action:'reopened_saved'});
      setHint('Reopened '+freshTabs.length+' verified saved files. '+
        plan.summary.changedOnDisk+' changed on disk, '+
        plan.summary.missing+' unavailable, '+
        plan.summary.checkpoints+' private checkpoints found. '+
        'Unsaved recovery requires a separate explicit action.');
    } catch(error) {
      if (workspaceLayoutOp.current===token)
        setProblem('Saved workspace was not restored: '+String(error));
    } finally {
      if (workspaceLayoutOp.current===token) workspaceLayoutOp.current=null;
      if (workspaceTransitionRef.current===token)
        workspaceTransitionRef.current=null;
      setWorkspaceLayoutBusy(false);
      finishExplorerIo(ticket);
    }
  }

  async function inspectOpenTabDiskRevisions() {
    const projectId=activeProjectIdRef.current;
    if (!projectId || mergeBusy || mergeApplyRequest ||
        workspaceLayoutOp.current || explorerIoTicket.current ||
        savingKey || draftRecoveryBusy || reviewPending || inboxBusy) return;
    let targets;
    try { targets=diskAuditTargets(tabsRef.current,projectId); }
    catch(error) {
      setProblem('Cannot inspect open tabs: '+String(error));
      return;
    }
    if (!targets.length) {
      setHint('Open a source file before checking external disk changes.');
      return;
    }
    const ticket=beginExplorerIo();
    if (!ticket) return;
    const serial=++diskAuditSerial.current;
    setDiskAuditBusy(true);
    setDiskAuditReport(null);
    setDiskComparison(null);
    setProblem('');
    const rows=[];
    try {
      // This is an explicit, bounded read of already-registered source tabs.
      // Never poll background files and never auto-reload a local draft.
      for (const target of targets) {
        const originalState=editorStates.current.get(target.key);
        let preview=null,unavailable=false;
        try {
          preview=await invoke('ide_preview_source_file',{
            projectId,relativePath:target.relativePath,
          });
        } catch {
          unavailable=true;
        }
        if (serial!==diskAuditSerial.current ||
            explorerIoTicket.current!==ticket ||
            activeProjectIdRef.current!==projectId) return;
        const current=tabsRef.current.find(tab=>tab.key===target.key);
        const state=editorStates.current.get(target.key);
        const sameState=!!originalState && originalState===state;
        const result=classifyDiskAudit(target,current,preview,projectId,sameState);
        rows.push({
          ...target,
          status:unavailable && result.status!=='stale'
            ? 'unavailable' : result.status,
          diskRevision:result.diskRevision,
          liveUnsaved:!!current && !!state &&
            (state.sliceDoc()!==current.savedText),
        });
      }
      if (serial!==diskAuditSerial.current ||
          activeProjectIdRef.current!==projectId) return;
      setDiskAuditReport({projectId,rows});
      const changed=rows.filter(row=>row.status==='changed').length;
      const unavailable=rows.filter(row=>row.status==='unavailable').length;
      setHint('Checked '+rows.length+' currently open files. Changed on disk: '+
        changed+'. Unavailable: '+unavailable+
        '. No source or editor drafts were modified.');
    } catch(error) {
      if (serial===diskAuditSerial.current)
        setProblem('Read-only disk check failed: '+String(error));
    } finally {
      if (serial===diskAuditSerial.current) setDiskAuditBusy(false);
      finishExplorerIo(ticket);
    }
  }

  async function compareEditorWithDisk(tab) {
    const projectId=activeProjectIdRef.current;
    if (!tab || tab.projectId!==projectId || mergeBusy || mergeApplyRequest ||
        workspaceLayoutOp.current || explorerIoTicket.current ||
        savingKey || draftRecoveryBusy || reviewPending) return;
    let target;
    try { [target]=diskAuditTargets([tab],projectId); }
    catch(error) {setProblem('Cannot compare disk revision: '+String(error));return;}
    const originalState=editorStates.current.get(tab.key);
    if (!originalState) {
      setProblem('Current editor state is unavailable for comparison.');
      return;
    }
    const ticket=beginExplorerIo();
    if (!ticket) return;
    const serial=++diskAuditSerial.current;
    setDiskAuditBusy(true);
    setDiskComparison(null);
    setProblem('');
    try {
      const preview=await invoke('ide_preview_source_file',{
        projectId,relativePath:tab.relativePath,
      });
      if (explorerIoTicket.current!==ticket ||
          serial!==diskAuditSerial.current ||
          activeProjectIdRef.current!==projectId) return;
      const current=tabsRef.current.find(row=>row.key===tab.key);
      const state=editorStates.current.get(tab.key);
      const outcome=classifyDiskAudit(target,current,preview,projectId,
        state===originalState);
      if (outcome.status==='stale' ||
          outcome.status==='invalid_native_result' || !state)
        throw Error('B10_EDITOR_OR_DISK_CONTEXT_CHANGED');
      const summary=summarizeExternalDifference(
        current.savedText,state.sliceDoc(),preview.text);
      // Keep only bounded comparison excerpts, not the complete disk file.
      setDiskComparison({
        projectId,tabKey:tab.key,bufferRevision:current.bufferRevision,
        generation:current.generation,baseRevision:current.revision,
        diskRevision:outcome.diskRevision,status:outcome.status,summary,
      });
      setHint('Read-only disk comparison for '+tab.relativePath+
        '. Review the excerpts; Reload and Save remain separate explicit actions.');
    } catch(error) {
      if (serial===diskAuditSerial.current)
        setProblem('Cannot compare with current disk source: '+String(error));
    } finally {
      if (serial===diskAuditSerial.current) setDiskAuditBusy(false);
      finishExplorerIo(ticket);
    }
  }

  async function prepareExternalMerge(tab) {
    const projectId=activeProjectIdRef.current;
    if (!tab || tab.projectId!==projectId ||
        selectedMergeTabRef.current!==tab.key || !editorVisibleRef.current ||
        mergeBusy || workspaceLayoutOp.current || explorerIoTicket.current ||
        savingKey || reviewPending || draftRecoveryBusy || inboxBusy) return;
    const state=editorStates.current.get(tab.key);
    if (!state) {
      setProblem('Open the source editor before reviewing external changes.');
      return;
    }
    const serial=++mergeSerial.current;
    const ticket=beginExplorerIo();
    if (!ticket) return;
    setMergeBusy(true);
    setMergeReview(null);
    setMergeChoices({});
    setProblem('');
    try {
      const preview=await invoke('ide_preview_source_file',{
        projectId,relativePath:tab.relativePath,
      });
      const current=tabsRef.current.find(row=>row.key===tab.key);
      const stillCurrent=serial===mergeSerial.current &&
        explorerIoTicket.current===ticket &&
        selectedMergeTabRef.current===tab.key &&
        activeProjectIdRef.current===projectId &&
        editorVisibleRef.current &&
        !!current && current.savedText===tab.savedText &&
        current.draft===tab.draft && current.revision===tab.revision &&
        current.bufferRevision===tab.bufferRevision &&
        current.generation===tab.generation &&
        editorStates.current.get(tab.key)===state &&
        state.sliceDoc()===current.draft &&
        preview?.projectId===projectId &&
        preview?.relativePath===tab.relativePath &&
        typeof preview?.text==='string' &&
        /^[a-f0-9]{64}$/.test(preview?.revision);
      if (!stillCurrent)
        throw Error('B11_REVIEW_CONTEXT_CHANGED');
      if (preview.revision===current.revision) {
        setHint('The saved disk revision has not changed. No external merge is needed.');
        return;
      }
      const plan=planThreeWayMerge(current.savedText,state.sliceDoc(),preview.text);
      const review={
        projectId,tabKey:tab.key,relativePath:tab.relativePath,
        savedRevision:current.revision,diskRevision:preview.revision,
        generation:current.generation,bufferRevision:current.bufferRevision,
        savedText:current.savedText,draftText:state.sliceDoc(),
        diskText:preview.text,editorState:state,plan,
      };
      setMergeReview(review);
      setHint('Review '+plan.conflicts.length+' overlapping conflict(s). '+
        'Unrelated line edits are combined. Nothing has been applied or saved.');
    } catch(error) {
      if (serial===mergeSerial.current)
        setProblem('Cannot prepare a verified external merge: '+String(error));
    } finally {
      if (serial===mergeSerial.current) setMergeBusy(false);
      finishExplorerIo(ticket);
    }
  }

  async function applyReviewedExternalMerge() {
    const review=mergeReview;
    const projectId=activeProjectIdRef.current;
    if (!review || mergeBusy || mergeApplyRequest ||
        selectedMergeTabRef.current!==review.tabKey || !editorVisibleRef.current ||
        projectId!==review.projectId ||
        workspaceLayoutOp.current || explorerIoTicket.current ||
        savingKey || reviewPending || draftRecoveryBusy) return;
    let result;
    try { result=renderReviewedMerge(review.plan,mergeChoices); }
    catch(error) {setProblem('Invalid reviewed merge: '+String(error));return;}
    if (!result.ready) {
      setProblem('Choose which version to keep for '+result.missing+' before applying.');
      return;
    }
    if (!window.confirm(
      'Apply the reviewed result ONLY to this unsaved editor buffer? '+
      'Disk files remain unchanged. Undo can restore the earlier draft; '+
      'Save will still require a separate explicit action.')) return;
    const serial=++mergeSerial.current;
    const ticket=beginExplorerIo();
    if (!ticket) return;
    setMergeBusy(true);
    setProblem('');
    try {
      // Revalidate the disk snapshot when the user approves, NOT just when
      // the preview was prepared. A subsequent external write will still be
      // fenced by the original native Save expected-revision CAS.
      const latest=await invoke('ide_preview_source_file',{
        projectId,relativePath:review.relativePath,
      });
      const tab=tabsRef.current.find(item=>item.key===review.tabKey);
      const state=editorStates.current.get(review.tabKey);
      if (serial!==mergeSerial.current ||
          explorerIoTicket.current!==ticket ||
          selectedMergeTabRef.current!==review.tabKey ||
          !editorVisibleRef.current ||
          !mergeCanApply(review,tab,state,activeProjectIdRef.current,latest))
        throw Error('B11_RECHECK_FAILED: source or draft changed; review again');
      const nonce=++mergeNonce.current;
      setMergeApplyRequest({
        nonce,tabIdentity:tab.key+':'+tab.generation,
        projectId,tabKey:tab.key,relativePath:tab.relativePath,
        generation:tab.generation,savedRevision:tab.revision,
        expectedDraft:review.draftText,expectedEditorState:state,
        mergedText:result.text,
        diskText:latest.text,diskRevision:latest.revision,
      });
    } catch(error) {
      if (serial===mergeSerial.current) {
        setMergeBusy(false);
        setProblem('Merge not applied: '+String(error));
      }
    } finally { finishExplorerIo(ticket); }
  }

  function finishReviewedExternalMerge(nonce,applied,reason=null) {
    const req=mergeApplyRequest;
    if (!req || req.nonce!==nonce) return;
    setMergeApplyRequest(null);
    setMergeBusy(false);
    if (!applied) {
      setProblem(reason==='B11_LINE_ENDING_RELOAD_REQUIRED'
        ? 'The disk or merged file uses a different newline format from the active CodeMirror state. No merge applied; preserve your draft and explicitly reload before reconciling line endings.'
        : 'The editor changed before the merge transaction. No source rebase was made.');
      return;
    }
    const tab=tabsRef.current.find(item=>item.key===req.tabKey);
    const state=editorStates.current.get(req.tabKey);
    if (!tab || !state ||
        activeProjectIdRef.current!==req.projectId ||
        selectedMergeTabRef.current!==req.tabKey ||
        tab.projectId!==req.projectId ||
        tab.relativePath!==req.relativePath ||
        tab.generation!==req.generation ||
        tab.revision!==req.savedRevision ||
        state.sliceDoc()!==req.mergedText) {
      setProblem('Editor identity changed while applying merge. Recheck the source before Save.');
      return;
    }
    // Old private checkpoints stay on disk, but their session buffer
    // identity must not silently become the new disk revision's authority.
    clearPrivateDraftSession(req.tabKey);
    resetBufferIdentity(req.tabKey,req.projectId,req.relativePath,req.diskRevision);
    setTabs(previous=>previous.map(item=>
      item.key===req.tabKey && item.generation===req.generation &&
      item.revision===req.savedRevision
        ? {...item,savedText:req.diskText,revision:req.diskRevision,
            dirty:item.draft!==req.diskText}
        : item));
    setMergeReview(null);
    setMergeChoices({});
    setDiskComparison(null);
    setDiskAuditReport(null);
    setReview(null);
    if (lspSessionRef.current?.members?.[req.tabKey])
      void closeLspDocument(req.tabKey);
    setHint('Applied reviewed merge to the editor only. '+
      'Original draft remains in CodeMirror Undo. Source Save is still explicit.');
  }

  function closeCleanBackgroundTabs() {
    const projectId=activeProjectIdRef.current;
    if (!projectId || savingKey || savingKeyRef.current ||
        mergeBusy || mergeApplyRequest || workspaceTransitionRef.current ||
        explorerPending || explorerIoTicket.current ||
        workspaceLayoutOp.current || draftRecoveryBusy || reviewPending || inboxBusy) return;
    // An editor transaction can be newer than the last React tab render.
    // The pure planner refuses to close a tab unless its actual CodeMirror
    // text still exactly matches the saved disk-base text.
    const protectedKeys=new Set([activeTabKey,secondaryTabKey,review?.tabKey]
      .filter(Boolean));
    for (const key of draftInFlight.current) protectedKeys.add(key);
    for (const key of draftTimers.current.keys()) protectedKeys.add(key);
    for (const tab of tabsRef.current) {
      if (lspSession?.members?.[tab.key]) protectedKeys.add(tab.key);
    }
    const plan=planCleanBackgroundTabClosure(tabsRef.current,projectId,
      protectedKeys,key=>editorStates.current.get(key)?.sliceDoc());
    if (!plan.closeKeys.length) {
      setHint('No verified clean background tabs to close. Unsaved and active panes were kept.');
      return;
    }
    const closing=new Set(plan.closeKeys);
    // Single synchronous user action, no native IPC, no filesystem writes,
    // and no await between the live-state witnesses and memory reclamation.
    for (const key of closing) {
      editorStates.current.delete(key);
      clearPrivateDraftSession(key);
    }
    setTabs(previous=>previous.filter(tab=>!closing.has(tab.key)));
    setHint('Closed '+closing.size+' clean background tab(s). Both editor panes, '+
      'unsaved drafts and project files were preserved.');
  }

  async function closeTab(key) {
    const selected = tabsRef.current.find(tab => tab.key === key);
    if (!selected) return;
    if (savingKey === key || savingKeyRef.current === key ||
        mergeBusy || mergeApplyRequest || workspaceLayoutOp.current ||
        workspaceTransitionRef.current) return;
    // The render's selected.dirty can be stale for the latest CodeMirror
    // transaction. Unknown identity requires confirmation, never silent loss.
    const live = captureLiveEditorIntent(selected,
      bufferIdentities.current.get(key),editorStates.current.get(key),
      selected.projectId);
    if ((live.status !== 'current' || live.dirty) && !window.confirm(
      'Discard unsaved changes in ' + selected.relativePath + '?')) return;
    if (lspSessionRef.current?.members?.[key] &&
        !await closeLspDocument(key)) return;
    // Closing a native document can await a worker round trip. An editor
    // changed during that wait may no longer inherit the old discard consent.
    if (live.status!=='current' ||
        activeProjectIdRef.current!==selected.projectId ||
        !stillCurrentEditorIntent(live,
          tabsRef.current.find(item=>item.key===key),
          bufferIdentities.current.get(key),editorStates.current.get(key),
          activeProjectIdRef.current)) {
      setProblem('File changed during analysis cleanup. Close was cancelled; draft preserved.');
      return;
    }
    editorStates.current.delete(key);
    setMergeReview(previous=>previous?.tabKey===key ? null : previous);
    if (secondaryTabKey === key) setSecondaryTabKey(null);
    clearPrivateDraftSession(key);
    setReview(previous => previous?.tabKey === key ? null : previous);
    const other = tabsRef.current.filter(tab => tab.key !== key);
    setTabs(previous=>previous.filter(tab=>tab.key!==key));
    if (activeTabKey === key) {
      const candidates = other.filter(tab => tab.projectId === selected.projectId);
      setActiveTabKey(candidates.length ? candidates[candidates.length - 1].key : null);
    }
  }

  async function reloadTab(tab) {
    if (!tab || savingKey || savingKeyRef.current || reviewPending ||
        mergeBusy || mergeApplyRequest || workspaceLayoutOp.current ||
        workspaceTransitionRef.current) return;
    const original = captureLiveEditorIntent(tab,
      bufferIdentities.current.get(tab.key),editorStates.current.get(tab.key),
      activeProjectIdRef.current);
    if (original.status !== 'current') {
      setProblem('Cannot Reload: the live editor or original file base is unavailable.');
      return;
    }
    if (original.dirty && !window.confirm(
      'Discard your live unsaved edits and reload from disk?')) return;
    // A confirmation cannot authorize deleting edits made while an LSP stop
    // or filesystem read was in flight.
    if (lspSessionRef.current?.members?.[tab.key] &&
        !await closeLspDocument(tab.key)) return;
    if (!stillCurrentEditorIntent(original,
        tabsRef.current.find(item=>item.key===tab.key),
        bufferIdentities.current.get(tab.key),editorStates.current.get(tab.key),
        activeProjectIdRef.current)) {
      setProblem('Reload stopped: editor changed after confirmation.');
      return;
    }
    const ticket = beginExplorerIo();
    if (!ticket) return;
    const editorStateAtStart=editorStates.current.get(tab.key);
    setReview(null);
    setProblem('');
    try {
      const opened = await invoke('ide_preview_source_file', {
        projectId: tab.projectId, relativePath: tab.relativePath,
      });
      const latestTab = tabsRef.current.find(item => item.key === tab.key);
      if (explorerIoTicket.current !== ticket ||
          editorStates.current.get(tab.key) !== editorStateAtStart ||
          !stillCurrentEditorIntent(original,latestTab,
            bufferIdentities.current.get(tab.key),
            editorStates.current.get(tab.key),activeProjectIdRef.current) ||
          !lateReloadCanCommit(tab,latestTab,opened,activeProjectIdRef.current)) {
        throw new Error('Reload stopped: project, source or draft changed during native read.');
      }
      setMergeReview(previous=>previous?.tabKey===tab.key ? null : previous);
      clearPrivateDraftSession(tab.key);
      resetBufferIdentity(tab.key,opened.projectId,opened.relativePath,opened.revision);
      editorStates.current.set(tab.key, createEditorState(
        opened.text, opened.relativePath,
        (state, draft) => onEditorChange(tab.key, state, draft),
        state => onEditorSelection(tab.key,state)
      ));
      void checkExistingDraft({...tab,revision:opened.revision,
        savedText:opened.text,draft:opened.text,dirty:false});
      setTabs(previous => previous.map(item => item.key === tab.key ? {
        ...item, revision: opened.revision, savedText: opened.text,
        draft: opened.text, dirty: false, generation: item.generation + 1,
        bufferRevision: (item.bufferRevision ?? 0) + 1,
      } : item));
      setHint('Reloaded ' + tab.relativePath + ' from disk.');
    } catch (error) {
      if (explorerIoTicket.current === ticket) setProblem(String(error));
    } finally { finishExplorerIo(ticket); }
  }

  async function saveTab(tab, expectedReviewedIntent=null) {
    if (!tab || savingKey || savingKeyRef.current || mergeBusy ||
        mergeApplyRequest || explorerPending || explorerIoTicket.current ||
        workspaceLayoutOp.current || workspaceTransitionRef.current) return false;
    // CodeMirror is authoritative. The React draft/dirty flag can lag the
    // current edit transaction, so never save tab.draft from a stale render.
    const started = captureLiveEditorIntent(tab,
      bufferIdentities.current.get(tab.key), editorStates.current.get(tab.key),
      activeProjectIdRef.current);
    if (started.status !== 'current') {
      setProblem('Cannot Save: editor identity or saved file revision is stale.');
      return false;
    }
    if (!started.dirty) {
      setHint('No unsaved changes in the current editor buffer.');
      return false;
    }
    if (expectedReviewedIntent && !stillCurrentEditorIntent(expectedReviewedIntent,
        tab, bufferIdentities.current.get(tab.key),
        editorStates.current.get(tab.key), activeProjectIdRef.current)) {
      setReview(null);
      setProblem('Reviewed source changed. Review the live editor text again before saving.');
      return false;
    }
    const saveStartedBuffer=bufferIdentities.current.get(tab.key);
    savingKeyRef.current=tab.key;
    setSavingKey(tab.key);
    setProblem('');
    setHint('');
    try {
      const saved = await invoke('ide_save_source_file', {
        projectId: started.projectId,
        relativePath: started.relativePath,
        expectedRevision: started.savedRevision,
        text: started.text,
      });
      if (saved?.projectId !== started.projectId ||
          saved?.relativePath !== started.relativePath ||
          !/^[a-f0-9]{64}$/i.test(saved.revision || '') ||
          saved.text !== started.text) {
        throw Error('SAVE_RECEIPT_SOURCE_MISMATCH');
      }
      // Saving does NOT authorize rebasing a replaced editor instance.
      const currentBuffer=bufferIdentities.current.get(tab.key);
      const rebased=rebaseBufferAfterSave(currentBuffer,saveStartedBuffer,saved.revision);
      if (rebased.status!=='rebased') {
        setHint('Original editor buffer was saved, but the newer or reopened editor was untouched.');
        return false;
      }
      bufferIdentities.current.set(tab.key,rebased.identity);
      setMergeReview(previous=>previous?.tabKey===tab.key ? null : previous);
      setReview(previous=>previous?.tabKey===tab.key ? null : previous);
      // The live CodeMirror buffer may have advanced while native Save ran.
      // Keep those newer edits dirty, and rebase only this exact saved base.
      setTabs(previous => previous.map(item => {
        if (item.key!==tab.key) return item;
        return reconcileSavedEditorTab(item,started,saved.revision,
          bufferIdentities.current.get(tab.key),
          editorStates.current.get(tab.key)) || item;
      }));
      if (lspSessionRef.current?.members?.[tab.key] &&
          !await closeLspDocument(tab.key)) {
        // The Save receipt is durable even if LSP cleanup is uncertain.
        return true;
      }
      setHint('Saved ' + tab.relativePath + ' to disk. Any edits made during Save remain unsaved.');
      return true;
    } catch (error) {
      if (String(error).includes('FILE_CHANGED_ON_DISK')) {
        setProblem('File changed on disk. Your live unsaved editor buffer was preserved. ' +
          'Compare the changes before reloading or saving.');
      } else {
        setProblem('Save failed: '+String(error));
      }
    } finally {
      if (savingKeyRef.current===tab.key) savingKeyRef.current=null;
      setSavingKey(null);
    }
    return false;
  }

  async function reviewTab(tab) {
    if (!tab || reviewPending || savingKey || savingKeyRef.current ||
        mergeBusy || mergeApplyRequest || workspaceTransitionRef.current ||
        explorerPending || explorerIoTicket.current) return;
    const intent=captureLiveEditorIntent(tab,
      bufferIdentities.current.get(tab.key),editorStates.current.get(tab.key),
      activeProjectIdRef.current);
    if (intent.status!=='current' || !intent.dirty) {
      setProblem('No current unsaved editor change is available for review.');
      return;
    }
    setReviewPending(true);
    setProblem('');
    try {
      const proposal = await invoke('ide_review_source_change', {
        projectId: intent.projectId,
        relativePath: intent.relativePath,
        expectedRevision: intent.savedRevision,
        proposedText: intent.text,
      });
      if (!stillCurrentEditorIntent(intent,
          tabsRef.current.find(item=>item.key===intent.key),
          bufferIdentities.current.get(intent.key),
          editorStates.current.get(intent.key),activeProjectIdRef.current) ||
          proposal.projectId!==intent.projectId ||
          proposal.relativePath!==intent.relativePath ||
          proposal.expectedRevision!==intent.savedRevision) {
        throw Error('REVIEW_SOURCE_CHANGED_DURING_NATIVE_READ');
      }
      setReview({ ...proposal, tabKey:intent.key,
        draft:intent.text, liveIntent:intent });
    } catch (error) {
      setReview(null);
      setProblem(String(error).includes('FILE_CHANGED_ON_DISK')
        ? 'File changed externally. Your editor draft is preserved.'
        : 'Cannot review editor changes: '+String(error));
    } finally { setReviewPending(false); }
  }

  async function saveReviewedChange() {
    if (!review || review.truncated || savingKey || savingKeyRef.current ||
        reviewPending) return;
    const tab = tabsRef.current.find(item => item.key === review.tabKey);
    if (!tab || tab.revision !== review.expectedRevision ||
        tab.projectId !== review.projectId ||
        !stillCurrentEditorIntent(review.liveIntent,tab,
          bufferIdentities.current.get(review.tabKey),
          editorStates.current.get(review.tabKey),activeProjectIdRef.current)) {
      setReview(null);
      setProblem('The reviewed edit changed. Review the current live buffer before saving.');
      return;
    }
    const saved = await saveTab(tab, review.liveIntent);
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
      } else if ((key==='1'||key==='2') && section==='explorer' &&
          !event.altKey && !event.shiftKey) {
        event.preventDefault();
        requestEditorFocus(key==='2'?'secondary':'primary');
      } else if (key === 's') {
        event.preventDefault();
        // Toolbars are not editor panes. Preserve the most recently focused
        // source pane instead of silently defaulting Save to primary.
        const closest = event.target?.closest?.('[data-ide-editor-pane]')
          ?.getAttribute('data-ide-editor-pane');
        const pane = closest || focusedEditorPaneRef.current;
        const target = canSaveFocusedPane(pane,activeTab,secondaryTab);
        if (target && !savingKeyRef.current && !workspaceTransitionRef.current)
          void saveTab(target);
      }
    };
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, [pending, explorerPending, savingKey, activeTab, secondaryTab, tabs, section,
    splitEditorOpen, activeProject?.projectId]);

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
          disabled={item.pending || mergeBusy || !!mergeApplyRequest ||
            ((item.id === 'explorer' || item.id === 'project-search') && !activeProject)}
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
          <h1 aria-label={isJobs ? 'Execution requests' : undefined}>{isProjectSearch ? 'Find in project' : isJobs ? 'Execution requests' : isPackages ? 'Packages & workflows' : isSource ? 'Source control' : isContinuity ? 'Project continuity' : isCapabilities ? 'IDE capabilities' : isLanguages ? 'Language services' : isProposals ? 'Coding proposals' : isExplorer ? (activeProject?.displayName || 'Source editor') : 'Workspace projects'}</h1>
          <p>{isProjectSearch ? 'Saved-source search · Revision-checked hits · No writes' :
            isJobs ? 'Historical review intents · Protected native Run/Test · Explicit cancellation' :
            isPackages ? 'Local package manager inspection · Multi-project release review · No install permission' :
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
            onClick={() => isJobs ? (refreshIntents(), refreshProtectedBuildStatus()) : isPackages ? (inspectPackageManagers(), readWorkflowRuns(), workflowInspection && previewMultiProjectWorkflow()) : isSource ? refreshGitStatus() : isContinuity ? refreshContext() : isCapabilities ? refreshCapabilities() : isLanguages ? Promise.all([
                invoke('ide_list_language_servers'),
                invoke('ide_lsp_worker_status'),
              ]).then(([servers,worker])=>{
                setLanguageServers(Array.isArray(servers)?servers.slice(0,16):[]);
                setLspWorker(worker && typeof worker.ready==='boolean'
                  ? worker : {ready:false,supportedLanguages:[],semanticLanguages:[],
                    detail:'Native language-service status unavailable.'});
              }).catch(()=>setLspWorker({ready:false,
                supportedLanguages:[],semanticLanguages:[],
                detail:'Native language-service status unavailable.'})) : isProposals ? refreshProposals() : isExplorer && activeProject ? openDirectory(activeProject, directory) : refresh()}>
            ↻ Refresh</button>
          <button className="primaryBtn" disabled={pending || explorerPending || !!savingKey}
            onClick={() => operation('ide_choose_project')}>+ Open folder</button>
        </div>
      </header>
      {problem && <div className="errorMsg appError" role="alert"
        aria-label={problem.startsWith('File changed on disk.')
          ? 'File changed on disk. Unsaved edits preserved' : undefined}
        aria-describedby="ide-visible-error-message">
        <span id="ide-visible-error-message">{problem}</span>
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
            {activeProject && <div className="ideJobsSourceReturn">
              <button type="button" className="ghostBtn"
                onClick={()=>showProjectProblems()}>
                Return to source Problems
              </button>
              <button type="button" className="ghostBtn"
                onClick={()=>requestEditorFocus('primary')}>
                Return to first editor
              </button>
              <p>Jobs are scoped to the selected project. Their progress does
                not imply successful builds, cleanup, or permission to execute.
                Source diagnostics remain separately verified.</p>
            </div>}
            <section className="continuityDocument" aria-label="DEV-03 project build and test profiles">
              <div className="capabilityHeading">
                <h3>DEV-03 · Project-specific build/test discovery</h3>
                <button type="button" className="ghostBtn"
                  disabled={!activeProject || profileDiscoveryBusy || buildBusy}
                  onClick={()=>void inspectProjectRunProfiles()}>
                  {profileDiscoveryBusy ? 'Inspecting…' : 'Inspect project profiles'}
                </button>
              </div>
              <p>Native read-only inspection of fixed manifest conventions and
                conventional files in tests/. This is not exhaustive test
                discovery, does not execute scripts, and does not validate
                package dependencies or approve a worker.</p>
              {belongsToSelectedProject(activeProject, projectRunProfiles) &&
                <div className="dev03ProfileGrid">
                  {projectRunProfiles.profiles.map(profile=>
                    <article className="dev03ProfileCard" key={profile.id}>
                      <div className="capabilityHeading">
                        <strong>{profile.title}</strong>
                        <small>{profile.operation}</small>
                      </div>
                      <p><code>{profile.manifest}</code> ·
                        {' '}{profile.manifestState.replaceAll('_',' ')}</p>
                      {profile.sha &&
                        <small>Saved source SHA-256:
                          {' '}{profile.sha.slice(0,16)}…</small>}
                      <p>Conventional test file candidates:
                        {' '}{profile.testCandidates} ·
                        {' '}{profile.scanState.replaceAll('_',' ')}</p>
                      {!!profile.hints.length &&
                        <p>Example paths:
                          {' '}{profile.hints.slice(0,4).join(', ')}
                          {profile.hints.length>4 && ' …'}</p>}
                      {profile.existingRecipe
                        ? <button type="button" className="ghostBtn"
                            disabled={profile.manifestState!==
                              'saved_file_sha_observed_not_validated' ||
                              !!buildStatus?.current || buildBusy}
                            onClick={()=>chooseProjectRunProfile(profile.id)}>
                            Select existing protected Cargo recipe
                          </button>
                        : <div className="dev03AdapterReview">
                            <p role="status">No reviewed native worker adapter.
                              Execution unavailable; project scripts are untrusted.</p>
                            <button type="button" className="ghostBtn"
                              disabled={adapterReviewBusy || profileDiscoveryBusy ||
                                !profile.sha || buildBusy}
                              onClick={()=>void inspectReviewedMultilangAdapter(profile.id)}>
                              {adapterReviewBusy ? 'Reviewing…' :
                                'Inspect fixed native adapter contract'}
                            </button>
                            {belongsToSelectedProject(activeProject, adapterReview) &&
                              adapterReview.profileId===profile.id &&
                              adapterReview.manifestSha===profile.sha &&
                              <section className="dev03AdapterReceipt"
                                aria-label={'Non-executable adapter review '+profile.id}>
                                <strong>Contract only — execution blocked</strong>
                                <p>Clean Git HEAD:
                                  {' '}{adapterReview.cleanGitHead.slice(0,12)}…</p>
                                <p>Review fingerprint:
                                  {' '}{adapterReview.fingerprint.slice(0,16)}…</p>
                                <p>Unverified tool bundle:
                                  {' '}{adapterReview.toolchainBundleId}</p>
                                <p>Fixed candidate worker operations
                                  (none executed):</p>
                                {adapterReview.steps.map(step=>
                                  <p key={step.stepId}><code>{step.toolId}
                                    {' '}{step.fixedArgs.join(' ')}</code></p>)}
                                <p>Source snapshot not sealed; OS worker not
                                  available; installed acceptance not established.</p>
                              </section>}
                          </div>}
                    </article>)}
                </div>}
              <p>The existing protected Cargo runner still independently
                verifies its own source snapshot, toolchain and OS admission.
                Selecting a recipe here never starts work.</p>
            </section>
            <section className="continuityDocument" aria-label="DEV-03.4 bounded test discovery">
              <div className="capabilityHeading">
                <h3>DEV-03.4 · Conventional test files</h3>
                <button type="button" className="ghostBtn"
                  disabled={!activeProject||testDiscoveryBusy||buildBusy}
                  onClick={()=>void inspectConventionalTests()}>
                  {testDiscoveryBusy?'Inspecting…':'Discover test files'}
                </button>
              </div>
              <p>Native fixed-directory metadata inspection only. Filename
                matches are hints, not individual runnable tests. Nested,
                generated and framework-discovered tests may be omitted.
                Nothing here grants execution or runs a project script.</p>
              {belongsToSelectedProject(activeProject, testDiscovery) &&
                <>
                  <p role="status" aria-label="Conventional test discovery incomplete by design">
                    {testDiscovery.files.length} filenames found
                    {testDiscovery.truncated?' · results truncated':''}
                    {' · discovery incomplete by design'}
                  </p>
                  {testDiscovery.files.length>0 &&
                    <ul>{testDiscovery.files.map(file=>
                      <li key={file.path}>
                        <code>{file.path}</code> · {file.language}
                        {file.recipe?' · Candidate '+file.recipe:' · No approved adapter'}
                        {' · Not executable from discovery'}
                      </li>)}</ul>}
                  {testDiscovery.files.length===0 &&
                    <p>No conventional test filenames found in the fixed
                      scan; this does not prove that the project has no tests.</p>}
                </>}
            </section>
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
                  !['rust_check','rust_build','rust_tests','lu_ide_host_tests'].includes(intentRecipe)}
                onClick={() => startProtectedBuild()}>
                {buildBusy ? 'Protected job in progress…' : 'Request protected Run / Test'}
              </button>
              {belongsToSelectedProject(activeProject, buildStatus?.current) && <>
                <p role="status">Active protected job: {buildStatus.current.recipeId} ·
                  {buildStatus.current.jobId || 'Preparing frozen snapshot'}
                  {buildStatus.current.cancellationRequested ? ' · cancellation requested' : ''}</p>
                <button type="button" className="ghostBtn"
                  disabled={buildCancelBusy || buildStatus.current.cancellationRequested}
                  onClick={() => cancelProtectedBuild()}>
                  {buildCancelBusy ? 'Requesting cancellation…' : 'Cancel and verify process-tree termination'}
                </button>
              </>}
              <section className="continuityDocument"
                aria-label="Protected worker live stdout and stderr">
                <h4>DEV-03.3B · Live protected worker output</h4>
                <p>Bounded untrusted process text from the native worker.
                  Both Linux and Windows preserve stdout/stderr identity; Windows
                  uses separate protected inherited file handles. Live output is not
                  a completed-job receipt or proof of kernel cleanup.</p>
                {belongsToSelectedProject(activeProject, nativeJobOutput) &&
                  nativeJobOutput.chunks.length > 0
                  ? <>
                    {nativeJobOutput.truncated &&
                      <p role="status">Earlier output truncated to the bounded
                        in-memory window.</p>}
                    <pre aria-label="Untrusted worker output"
                      style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere',
                        maxHeight:'20rem',overflow:'auto'}}>
                      {nativeJobOutput.chunks.map(chunk=>
                        '['+chunk.stream+'] '+chunk.text).join('')}
                    </pre>
                  </>
                  : <p>No native worker output has been observed for this project.</p>}
              </section>
              <section aria-label="DEV-03.3 live native job phase timeline"
                className="continuityDocument">
                <h4>DEV-03.3 · Live protected job progress</h4>
                <p>Read-only native broker phases, refreshed while Jobs is open.
                  Output is displayed separately above. A progress entry
                  does not prove kernel cleanup, successful output, or release acceptance.</p>
                {visibleJobProgress(jobProgressEvents, activeProject?.projectId)
                  .length === 0
                  ? <p>No verified native job phase observations for this project.</p>
                  : <ol aria-live="polite">
                    {visibleJobProgress(jobProgressEvents, activeProject?.projectId)
                      .map((event, index) =>
                        <li key={event.jobId + '-' + event.ordinal + '-' + index}>
                          <code>{event.recipeId}</code> ·
                          <code>{event.jobId}</code> ·
                          {event.phase.replaceAll('_',' ')}
                          {event.cancellationRequested ? ' · cancel requested' : ''}
                        </li>)}
                  </ol>}
              </section>
              <section className="continuityDocument"
                aria-label="DEV-03.5 verified protected job lifecycle">
                <h4>DEV-03.5 · Protected job lifecycle</h4>
                <p>Lifecycle is projected only from the native durable
                  journal, current worker identity and matching native
                  result. Output and test filenames are never proof of
                  execution, OS cleanup or installed acceptance.</p>
                {projectJobLifecycle(buildStatus,activeProject?.projectId,
                  nativeJobOutput).jobs.length===0
                  ? <p role="status"
                      aria-label="No matching protected job journal entries observed">
                      No matching protected job journal entries observed.</p>
                  : <ol aria-live="polite">
                    {projectJobLifecycle(buildStatus,activeProject?.projectId,
                      nativeJobOutput).jobs.slice(-12).map(job=>
                      <li key={job.jobId}>
                        <code>{job.jobId}</code> ·
                        {job.state.replaceAll('_',' ')} ·
                        {job.nativeResult
                          ? 'native result matches durable receipt'
                          : 'no fresh native execution result proved'}
                        {job.outputObserved && ' · bounded untrusted output observed'}
                        {job.outputTruncated && ' · output truncated'}
                        {job.requiresReconciliation && ' · OS reconciliation required'}
                        {' · Installed acceptance not established'}
                      </li>)}
                  </ol>}
              </section>
              {belongsToSelectedProject(activeProject, buildResult) && <div role="status">
                <p>Last worker result: <strong>{buildResult.phase}</strong> · Job {buildResult.jobId}</p>
                <p>Native exit: {String(buildResult.nativeExitCode ?? 'unknown')} ·
                  Fixed OS error category: {buildResult.nativeErrorCategory || 'not reported'} · Timed out:
                  {buildResult.timedOut ? 'yes' : 'no'} · Reconciliation:
                  {buildResult.requiresReconciliation ? 'required' : 'not indicated'}</p>
                <p>Source: <code>{buildResult.inputSnapshotSha256}</code></p>
                <p>Toolchain: <code>{buildResult.toolchainSha256}</code></p>
                <p>Worker output seal: {buildResult.outputSha256 || 'not independently published'}</p>
                {safeDebugBinaryEvidence(buildResult,activeProject?.projectId) &&
                  <section aria-label="DEV-04.2 protected native build binary witness">
                    <h4>DEV-04.2 · Native build-to-binary provenance</h4>
                    <p>One candidate rehashed after the protected worker exited,
                      with exact source snapshot, native terminal receipt and
                      toolchain binding. Executable and symbol closure remain
                      unapproved; this does not enable debugging or launch.</p>
                    <p>Candidate: <code>{safeDebugBinaryEvidence(buildResult,
                      activeProject?.projectId).relativePath}</code></p>
                    <p>Binary SHA-256: <code>{safeDebugBinaryEvidence(buildResult,
                      activeProject?.projectId).binarySha}</code></p>
                    <p>Terminal receipt SHA-256: <code>{safeDebugBinaryEvidence(
                      buildResult,activeProject?.projectId).terminalReceiptSha}</code></p>
                    <p><strong>Debug execution: not authorized.</strong></p>
                  </section>}
                {safeDebugSymbolEvidence(buildResult,activeProject?.projectId) &&
                  <section aria-label="DEV-04.3 structurally matched debug symbols">
                    <h4>DEV-04.3 · Debug-symbol verification</h4>
                    <p>Native structural verification bound to the completed
                      protected build's executable SHA-256, output tree and
                      terminal receipt. Debugger execution and source-line
                      mapping are not yet authorized or independently proven.</p>
                    <p>Executable format:
                      {' '}{safeDebugSymbolEvidence(buildResult,
                        activeProject?.projectId).format}</p>
                    <p>Debug information:
                      {' '}{safeDebugSymbolEvidence(buildResult,
                        activeProject?.projectId).kind}</p>
                    {safeDebugSymbolEvidence(buildResult,
                      activeProject?.projectId).symbolsPath &&
                      <p>Matching symbol artifact: <code>{
                        safeDebugSymbolEvidence(buildResult,
                          activeProject?.projectId).symbolsPath}</code></p>}
                    <p>Symbol bytes SHA-256: <code>{safeDebugSymbolEvidence(
                      buildResult,activeProject?.projectId).symbolsSha}</code></p>
                    <p><strong>Structural checks only. No full DWARF DIE/CodeView
                      source mapping or project debugger launch.</strong></p>
                  </section>}
                {buildResult.recipeId==='rust_build' &&
                  safeDebugBinaryEvidence(buildResult,activeProject?.projectId) &&
                  !safeDebugSymbolEvidence(buildResult,activeProject?.projectId) &&
                  <p>DEV-04.3: No structurally matching debug information
                    was verified for this executable. Project debugging
                    remains unavailable.</p>}
                {buildResult.recipeId==='rust_build' &&
                  !safeDebugBinaryEvidence(buildResult,activeProject?.projectId) &&
                  <p>DEV-04.2: no uniquely attested executable from this build.
                    A completed Cargo build may produce a library or multiple
                    targets; the debugger remains unavailable.</p>}

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
            <section className="continuityDocument" aria-label="DEV-04.1 project debug provenance review">
              <div className="capabilityHeading">
                <h3>DEV-04.1 · Project debugger target preparation</h3>
                <button type="button" className="ghostBtn"
                  disabled={!activeProject||debugTargetBusy||buildBusy||
                    !belongsToSelectedProject(activeProject, projectRunProfiles)||
                    !projectRunProfiles.profiles.some(p=>p.id==='rust_build'&&p.sha)}
                  onClick={()=>void inspectDebugTarget()}>
                  {debugTargetBusy?'Reviewing…':'Review Rust target provenance'}
                </button>
              </div>
              <p>Read-only source review, not project debugging. A Cargo
                manifest and clean Git state cannot attest a compiled executable,
                debug symbols or a safe execution target. Project debugger
                launch remains unavailable pending independently verified
                build-to-binary provenance and native OS authorization.</p>
              {belongsToSelectedProject(activeProject, debugTargetReview) &&
                <div role="status">
                  <p>Clean source HEAD:
                    {' '}<code>{debugTargetReview.cleanGitHead}</code></p>
                  <p>Saved Cargo manifest SHA:
                    {' '}<code>{debugTargetReview.manifestSha}</code></p>
                  <p>Native review fingerprint:
                    {' '}<code>{debugTargetReview.fingerprint}</code></p>
                  <p><strong>No attested executable; no debugger approval;
                    no worker launched.</strong></p>
                </div>}
            </section>
            <div className="continuityDocument jobIntentComposer" aria-label="Protected native debugger self-test">
              <h3>Protected debugger (LLDB-DAP)</h3>
              <section aria-label="DEV-04.4 protected project debugger">
                <h4>DEV-04.4 · Debug exact completed project build</h4>
                <p>A fresh human approval and a new protected Job/cgroup
                  are required. The host selects only its last private,
                  independently rehashed Cargo executable and matched
                  debug symbols. Source, build receipt, and target digest
                  must still match. No arbitrary executable, attach,
                  shell, expressions, terminal, or debugger arguments.</p>
                <button type="button" className="ghostBtn"
                  disabled={!activeProject||!debugStatus?.projectDebugSupported||
                    !!debugStatus.current||debugBusy||buildBusy||
                    !safeDebugSymbolEvidence(buildResult,activeProject?.projectId)}
                  onClick={()=>void runProtectedProjectDebugger()}>
                  {debugBusy?'Protected project debugger running…':
                    'Debug last verified project build'}
                </button>
                {!debugStatus?.projectDebugSupported &&
                  <p>Project debugger release feature and native adapter
                    are not independently installed/approved on this desktop.
                    The default build remains nonexecuting.</p>}
                {debugStatus?.operationControlSupported &&
                  <details className="jobOperationControls" aria-label="Developer-only native debugger action approval">
                    <summary>Advanced debugger actions (developer build only)</summary>
                    <p><strong>DEV-04.6D development mode only.</strong> Each action creates
                      a new sealed, OS-protected debugging session with separate launch
                      approval and a second native modal bound to the verified stop.
                      It is not signed installed release acceptance.</p>
                    <button type="button" className="ghostBtn"
                      disabled={!activeProject||!debugStatus?.projectDebugSupported||
                        !!debugStatus.current||debugBusy||buildBusy||
                        !safeDebugSymbolEvidence(buildResult,activeProject?.projectId)}
                      onClick={()=>void runNativeDebuggerAction('stepOver')}>
                      Step over verified source line (native approval)
                    </button>
                    <button type="button" className="ghostBtn"
                      disabled={!activeProject||!debugStatus?.projectDebugSupported||
                        !!debugStatus.current||debugBusy||buildBusy||
                        !safeDebugSymbolEvidence(buildResult,activeProject?.projectId)}
                      onClick={()=>void runNativeDebuggerAction('sameLineBreakpoint')}>
                      Request verified-line breakpoint (native approval)
                    </button>
                    <button type="button" className="ghostBtn"
                      disabled={!activeProject||!debugStatus?.projectDebugSupported||
                        !!debugStatus.current||debugBusy||buildBusy||
                        !safeDebugSymbolEvidence(buildResult,activeProject?.projectId)}
                      onClick={()=>void runNativeDebuggerAction('inspectVariables')}>
                      Inspect redacted local names/types (native approval)
                    </button>
                    <p>Breakpoint acknowledgement and adapter-reported variable names/types
                      remain unverified. Variable values, evaluate, child expansion, attach
                      and arbitrary paths are unavailable.</p>
                  </details>}
              </section>
              <p>The fixed self-test remains available separately. Project debugging
                requires the distinct opt-in native-project-debug feature,
                a sealed protected build and an independently pinned installed LLDB.
                Arbitrary attach, debugger scripts and terminal commands remain denied.</p>
              <p>Debugger: <strong>{debugStatus?.ready
                ? 'Pinned debugger bundle observed; OS containment checked per run'
                : (debugStatus?.code || 'Checking approved debugger assets…')}</strong></p>
              <p>{debugStatus?.description || 'An unpinned adapter cannot execute.'}</p>
              <button type="button" className="ghostBtn"
                disabled={!activeProject || !debugStatus?.ready || !!debugStatus.current || debugBusy}
                onClick={() => runProtectedDebuggerFixture()}>
                {debugBusy ? 'Inspecting fixed native debuggee…' : 'Run protected debugger self-test'}
              </button>
              {belongsToSelectedProject(activeProject, debugStatus?.current) && <>
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
              {belongsToSelectedProject(activeProject, debugResult) && <div role="status">
                <p>{debugResult.target==='one_exact_private_project_build_in_protected_job'
                  ? 'Protected project LLDB session' : 'Native LLDB self-test'}:
                  <strong> {debugResult.terminalPhase}</strong></p>
                {debugResult.target==='one_exact_private_project_build_in_protected_job' &&
                  <>
                    <p>Originating protected build:
                      {' '}<code>{debugResult.originBuildJobId}</code></p>
                    <p>Exact build terminal receipt:
                      {' '}<code>{debugResult.originBuildReceiptSha256}</code></p>
                    <p>Project debugger D3 target binding:
                      {' '}<code>{debugResult.targetBindingSha256}</code></p>
                  </>}
                <p>Stopped thread: {debugResult.stoppedThread ?? 'none'} · Thread inspection:
                  {debugResult.threadsResponseObserved ? 'observed' : 'not proven'} · Stack:
                  {debugResult.stackResponseObserved ? 'observed' : 'not proven'}</p>
                <p>Kernel tree cleanup: {debugResult.verifiedOsTreeReleaseByWorker
                  ? 'reported by protected supervisor' : 'not verified'}.</p>
                {debugResult.operationOutcomeCode==='native_dialog_variables_redacted_uncertain' &&
                  Array.isArray(debugResult.redactedVariables) &&
                  debugResult.redactedVariables.length<=16 && <>
                  <p>Read-only frame: <code>{debugResult.inspectedFrameId}</code>.
                    Native consent was one-use. All variable values are redacted;
                    the names and types below remain adapter assertions.</p>
                  <ul>{debugResult.redactedVariables
                    .filter(entry=>entry.valueRedacted===true)
                    .map((entry,index)=>
                      <li key={index}><code>{entry.name}</code>
                        {' '}· {entry.declaredType||'type unavailable'} · value redacted</li>)}
                  </ul>
                </>}
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
                <option value="rust_build">Rust project build</option>
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
        : isProjectSearch
        ? <section className="workspaceContent projectSearchWorkspace" aria-label="Find in project">
            <div className="sectionHeading">
              <h2>Find in saved project files</h2>
              <span>Read-only · Native registered project only</span>
            </div>
            <p className="capabilityNotice">Search a bounded set of UTF-8 files saved on disk.
              Unsaved drafts are never searched or overwritten. Generated, binary or large
              files may be excluded, and a partial scan must not be treated as a complete result.</p>
            <form className="projectSearchControls" onSubmit={event => {
              event.preventDefault(); void runProjectSearch();
            }}>
              <input aria-label="Search project text" placeholder="Literal text to find"
                value={projectSearchQuery} maxLength={128}
                onChange={event => {
                  setProjectSearchQuery(event.target.value);
                  setProjectSearchResults(null);
                  projectSearchGeneration.current += 1;
                  setProjectSearchBusy(false);
                }}/>
              <label><input type="checkbox" checked={projectSearchCase}
                onChange={event => {
                  setProjectSearchCase(event.target.checked);
                  setProjectSearchResults(null);
                  projectSearchGeneration.current += 1;
                  setProjectSearchBusy(false);
                }}/> Match case</label>
              <button className="primaryBtn" type="submit"
                disabled={!activeProject || !projectSearchQuery.trim() || projectSearchBusy}>
                {projectSearchBusy ? 'Searching…' : 'Search project'}
              </button>
            </form>
            {projectSearchBusy && <p role="status">Searching saved project sources…</p>}
            {projectSearchResults && projectSearchResults.projectId === activeProject?.projectId && <>
              <p className="projectSearchSummary">
                {projectSearchResults.matches.length} match(es) ·
                {' '}{projectSearchResults.filesScanned} files scanned ·
                {' '}{projectSearchResults.bytesScanned.toLocaleString()} bytes read
                {projectSearchResults.skippedFiles > 0 &&
                  ' · ' + projectSearchResults.skippedFiles + ' skipped files'}
              </p>
              {projectSearchResults.truncated && <p role="status" className="reviewWarning">
                Search limits reached: results are incomplete. Narrow the search scope.
              </p>}
              {projectSearchResults.matches.length === 0 && <p>
                No matching saved source found in the inspected scope.</p>}
              <div className="projectSearchResults">
                {projectSearchResults.matches.map((hit, index) =>
                  <button type="button" className="projectSearchHit"
                    key={hit.relativePath + ':' + hit.fromByte + ':' + index}
                    disabled={explorerPending}
                    onClick={() => void openProjectSearchMatch(hit)}>
                    <strong>{hit.relativePath}:{hit.line}:{hit.columnUtf16}</strong>
                    <code>{hit.preview}</code>
                    <small>Saved SHA-256 {hit.revision.slice(0, 12)}… ·
                      UTF-8 bytes {hit.fromByte}–{hit.toByte}</small>
                  </button>)}
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
            <div className="continuityDocument" aria-label="IDE-05.1 read-only package manager discovery">
              <h2>Package managers &amp; lockfiles</h2>
              <p>Inspect saved Python uv and C/C++ vcpkg/Conan manifests
                inside the active OS-picked project. This is read-only:
                no package tool is run, no network lookup or dependency install
                occurs, and the digest is never an execution grant.</p>
              <button type="button" className="ghostBtn"
                disabled={!activeProject || packageInspectionBusy}
                onClick={() => void inspectPackageManagers()}>
                {packageInspectionBusy ? 'Inspecting manifest evidence…' :
                  'Inspect project package managers'}
              </button>
              {belongsToSelectedProject(activeProject, packageInspection) &&
                <div role="status" className="continuityEvents">
                  <p>Revision fingerprint: <code>{packageInspection.fingerprint}</code></p>
                  <p>Snapshot: {packageInspection.snapshotConsistency.replaceAll('_',' ')}.
                    Resolution and Apply: disabled. No process, network or file changes.</p>
                  {packageInspection.blockers.length>0 &&
                    <p>Blockers: <strong>{packageInspection.blockers.join(', ')}</strong></p>}
                  {packageInspection.candidates.map(candidate=>
                    <article className="continuityEvent" key={candidate.managerId}>
                      <strong>{candidate.managerId} · {candidate.ecosystem}</strong>
                      <p>Status: {candidate.state.replaceAll('_',' ')}.</p>
                      {candidate.executableManifest &&
                        <p>Conan Python recipes are untrusted executable code.
                          They were NOT evaluated.</p>}
                      {packageInspection.evidence.filter(f=>
                        candidate.evidencePaths.includes(f.relativePath)).map(file=>
                        <p key={file.relativePath}>
                          <code>{file.relativePath}</code> · {file.state.replaceAll('_',' ')}
                          {file.sha256 && <> · SHA-256 <code>{file.sha256}</code></>}
                        </p>)}
                    </article>)}
                </div>}
            </div>
              {belongsToSelectedProject(activeProject, packageInspection) &&
                <section aria-label="IDE-05.2 verified package operation preview">
                  <h3>Review a proposed dependency operation</h3>
                  <p>The host fixes the recognized manager to this project and
                    rechecks exact manifest/lock SHA-256 evidence. No packages
                    are resolved, installed, upgraded or removed here.</p>
                  <label htmlFor="ide052Manager">Detected manager</label>
                  <select id="ide052Manager" value={packageManagerChoice}
                    disabled={packagePreviewBusy}
                    onChange={event=>{
                      setPackageManagerChoice(event.target.value);
                      invalidatePackagePreview();
                    }}>
                    {packageInspection.candidates.map(candidate=>
                      <option key={candidate.managerId} value={candidate.managerId}
                        disabled={candidate.state!=='candidate_needs_review'}>
                        {candidate.managerId} · {candidate.state.replaceAll('_',' ')}
                      </option>)}
                  </select>
                  <label htmlFor="ide052Operation">Proposed operation</label>
                  <select id="ide052Operation" value={packageOperation}
                    disabled={packagePreviewBusy}
                    onChange={event=>{
                      setPackageOperation(event.target.value);
                      invalidatePackagePreview();
                    }}>
                    <option value="add">Add direct dependency</option>
                    <option value="remove">Remove direct dependency</option>
                    <option value="sync">Review environment synchronization</option>
                  </select>
                  {packageOperation!=='sync' && <>
                    <label htmlFor="ide052Package">Dependency name (literal only)</label>
                    <input id="ide052Package" type="text" value={packageName}
                      maxLength={96} placeholder="example-lib"
                      disabled={packagePreviewBusy}
                      onChange={event=>{
                        setPackageName(event.target.value);
                        invalidatePackagePreview();
                      }} />
                  </>}
                  {packageOperation==='add' && <>
                    <label htmlFor="ide052Version">Requested exact version</label>
                    <input id="ide052Version" type="text" value={packageExactVersion}
                      maxLength={64} placeholder="1.2.3"
                      disabled={packagePreviewBusy}
                      onChange={event=>{
                        setPackageExactVersion(event.target.value);
                        invalidatePackagePreview();
                      }} />
                  </>}
                  <button type="button" className="ghostBtn"
                    disabled={packagePreviewBusy ||
                      !packageInspection.candidates.some(candidate =>
                        candidate.managerId===packageManagerChoice &&
                        candidate.state==='candidate_needs_review') ||
                      (packageOperation!=='sync'&&!packageName.trim()) ||
                      (packageOperation==='add'&&!packageExactVersion.trim())}
                    onClick={()=>void previewPackageOperation()}>
                    {packagePreviewBusy?'Inspecting source evidence…':
                      'Preview operation without executing'}
                  </button>
                  {belongsToSelectedProject(activeProject, packagePreview) &&
                    <article className="continuityEvent">
                      <strong>Static review — {packagePreview.request.managerId}
                        {' · '}{packagePreview.request.operation}</strong>
                      <p>Evidence: <code>{packagePreview.evidenceFingerprint}</code></p>
                      <p>Review SHA: <code>{packagePreview.previewSha256}</code></p>
                      <p>Dependency resolution: {packagePreview.dependencyResolution.replaceAll('_',' ')}.
                        This is an intent preview, NOT an exact file diff.</p>
                      <h4>Proposed changes (not applied)</h4>
                      {packagePreview.proposedChanges.map((change,index)=>
                        <p key={change.targetRelativePath+index}>
                          <code>{change.targetRelativePath}</code> ·
                          {' '}{change.intendedAction.replaceAll('_',' ')}
                          {change.requestedDeclaration &&
                            <> · <code>{change.requestedDeclaration}</code></>}
                          {' '}· direct declaration:
                          {' '}{change.observedDirectDeclaration.replaceAll('_',' ')}
                          {change.currentlyObservedSha256 &&
                            <> · observed SHA <code>{change.currentlyObservedSha256}</code></>}
                          {' '}· final bytes/versions unknown
                        </p>)}
                      <h4>Risks and review blockers</h4>
                      {packagePreview.risks.map(risk=>
                        <p key={risk}>{risk.replaceAll('_',' ')}</p>)}
                      {packagePreview.blockers.length>0 &&
                        <p role="status">Blocked: {packagePreview.blockers.join(', ')}</p>}
                      <button className="ghostBtn" type="button"
                        disabled={packagePreviewBusy}
                        onClick={()=>void revalidatePackageOperation()}>
                        Reinspect native manifests and verify exact preview
                      </button>
                      {packageRevalidated?.previewSha256===packagePreview.previewSha256 &&
                        <p role="status"><strong>Fresh native evidence matched.</strong>
                          {' '}Approval is still disabled; this result is not an
                          execution token and cannot bypass future OS consent.</p>}
                      <p><strong>General Install/Apply unavailable.</strong> This
                        review is not authority. A fixed offline uv Sync
                        is separately feature-gated and still requires native
                        revalidation, new approval and verified OS custody.</p>
                      {packagePreview.request.managerId==='uv' &&
                        packagePreview.request.operation==='sync' &&
                        <div className="continuityDocument"
                          aria-label="IDE-05.3 native approved offline package Sync">
                          <h4>Optional protected offline uv Sync</h4>
                          <p>The hash-pinned uv and Python runtime, protected OS
                            Job/cgroup, no-network sandbox and private output
                            must be verified. A <strong>new native human
                            approval</strong> is mandatory for each execution.
                            Add/Remove, Conan, vcpkg and online resolution remain disabled.</p>
                          <p>Native uv runtime: {buildStatus?.multilangReady?.find(
                            entry=>entry.profileId==='uv_sync_offline')?.ready ?
                            'Verified in this installed runtime' :
                            'Unavailable or not release-pinned'}</p>
                          <button className="primaryBtn" type="button"
                            disabled={packageRunBusy || buildBusy || buildStatus?.current ||
                              packageRevalidated?.previewSha256!==packagePreview.previewSha256 ||
                              packageRevalidated?.authorizationIssued!==false ||
                              !buildStatus?.multilangReady?.some(
                                entry=>entry.profileId==='uv_sync_offline' && entry.ready)}
                            onClick={()=>void runProtectedOfflineUvSync()}>
                            {packageRunBusy?'Native worker running…':
                              'Request fresh OS approval for sealed offline Sync'}
                          </button>
                        </div>}

                    </article>}
                </section>}
              {belongsToSelectedProject(activeProject, buildStatus?.current) &&
                buildStatus.current.recipeId==='uv_sync_offline' &&
                <article className="continuityEvent" role="status">
                  <strong>Active protected offline uv Sync</strong>
                  <p>Native job: <code>{buildStatus.current.jobId || 'Preparing sealed job'}</code></p>
                  <button type="button" className="ghostBtn"
                    disabled={buildCancelBusy}
                    onClick={()=>void cancelProtectedBuild()}>
                    Request cancellation and verified OS-tree cleanup
                  </button>
                </article>}
              {buildStatus?.quarantined?.filter(job=>job.projectId===activeProject?.projectId)
                .map(job=><article className="continuityEvent" key={job.jobId}>
                  <strong>Interrupted protected job — OS cleanup required</strong>
                  <p>Job: <code>{job.jobId}</code>. No automatic replay or
                    package retry after restart.</p>
                  <button type="button" className="ghostBtn" disabled={buildBusy}
                    onClick={()=>void reconcileProtectedBuild(job)}>
                    Ask for separate native OS recovery approval
                  </button>
                </article>)}
              {belongsToSelectedProject(activeProject, packageRunResult) &&
                <article className="continuityEvent" role="status">
                  <strong>Last protected uv Sync</strong>
                  <p>Job: <code>{packageRunResult.jobId}</code> ·
                    Phase: {packageRunResult.phase} · Native exit:
                    {String(packageRunResult.nativeExitCode)}</p>
                  <p>Terminal receipt:
                    {' '}<code>{packageRunResult.terminalReceiptSha256}</code></p>
                  <p>Private artifact evidence:
                    {' '}{packageRunResult.artifactEvidenceState.replaceAll('_',' ')}
                    · Digest: {packageRunResult.artifactManifestSha256 || 'Unverified'}</p>
                  <p>IDE-05.4 package audit:
                    {' '}{packageRunResult.packageAuditState?.replaceAll('_',' ') || 'not available'}</p>
                  <p>Original project manifest/lock unchanged after worker:
                    {' '}{String(packageRunResult.postrunOriginalManifestLockUnchanged===true)}
                    {' '}· Project package deployment: not performed</p>
                  {packageRunResult.packageOutputAudit &&
                    <div className="continuityDocument">
                      <p>Local RECORD bytes verified:
                        {' '}{String(packageRunResult.packageOutputAudit.recordHashesVerified)}
                        {' '}· License files verified:
                        {' '}{String(packageRunResult.packageOutputAudit.licenseFilesVerified)}
                        {' '}· Locked package versions:
                        {' '}{String(packageRunResult.packageOutputAudit.packageVersionsMatchLock)}</p>
                      <p>Wheel-archive origin verified:
                        {' '}{String(packageRunResult.packageOutputAudit.wheelArchivesVerifiedAgainstLock)}
                        {' '}· Project promotion permitted:
                        {' '}{String(packageRunResult.packageOutputAudit.approvedForProjectPromotion)}</p>
                      <p>Evidence digest:
                        {' '}<code>{packageRunResult.packageOutputAudit.digestSha256}</code></p>
                      {packageRunResult.packageOutputAudit.installed.map(dist=>
                        <article className="continuityEvent" key={dist.name}>
                          <strong>{dist.name} · {dist.version}</strong>
                          <p>License declaration: {dist.licenseExpression}
                            {' '}· Hashed license files: {dist.verifiedLicenseFileCount}
                            {' '}· Manifest-listed files: {dist.recordedFileCount}</p>
                          <p>Origin declaration wheel hashes: {dist.lockRegistryWheelHashCount}
                            {' '}· Actual wheel archive verified:
                            {' '}{String(dist.actualWheelArchiveSha256Verified)}</p>
                        </article>)}
                    </div>}
                  <p>No packages were installed into the registered project;
                    a completed worker is not signed installed acceptance.</p>
                </article>}
              {Array.isArray(buildStatus?.packageAudits) &&
                buildStatus.packageAudits.filter(row=>
                  row.projectId===activeProject?.projectId).map(row=>
                  <article className="continuityEvent" key={row.jobId}
                    aria-label="Durable package audit and disposal receipt">
                    <strong>Durable private package status:
                      {' '}{row.state.replaceAll('_',' ')}</strong>
                    <p>Job: <code>{row.jobId}</code> · Plan:
                      {' '}<code>{row.planSha256}</code></p>
                    <p>Original terminal:
                      {' '}<code>{row.terminalReceiptSha256}</code></p>
                    <p>Private tree SHA-256:
                      {' '}<code>{row.outputTreeSha256}</code></p>
                    {row.auditSha256 && <p>Local artifact audit:
                      {' '}<code>{row.auditSha256}</code></p>}
                    <p>May reexecute: {String(row.mayExecuteOrRetryWorker)}
                      {' '}· Installed into project: {String(row.installedIntoProject)}</p>
                    {['verified_local_bytes_private_not_promoted',
                      'original_package_job_requires_native_reconciliation',
                      'private_output_cleanup_pending'].includes(row.state) &&
                      <button className="ghostBtn" type="button"
                        disabled={packageRunBusy || buildBusy || !!buildStatus?.current}
                        onClick={()=>void discardProtectedPackageOutput(row)}>
                        {row.requiresExplicitPrivateCleanup ?
                          'Resume verified private cleanup with fresh OS consent' :
                          row.state==='original_package_job_requires_native_reconciliation' ?
                            'After OS terminal verification, discard unverified private output' :
                            'Request OS approval to discard this private environment'}
                      </button>}
                    {row.state==='private_output_audit_blocked' &&
                      <p role="status">Private output blocked: no promotion or
                        automatic destructive recovery. Inspect protected
                        native evidence before OS-managed disposal.</p>}
                  </article>)}
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
                    {task.state === 'native_receipt_unverified' &&
                      <div role="status">
                        <p>Host restarted. This completed step remains blocked until its
                          exact WO6 protected supervisor job/plan is independently
                          confirmed. No project code is rerun.</p>
                        <button type="button" className="ghostBtn"
                          disabled={workflowRunBusy}
                          onClick={() => reverifyNativeStepAfterRestart(run,index)}>
                          Verify stored native completion against OS journal
                        </button>
                      </div>}
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
            {activeProject && <div className="continuityDocument" aria-label="Read-only reviewed hunk selection">
              <h3>Advanced staging · select exact diff hunks (review only)</h3>
              <p>Inspect local changes for one tracked text file. Every hunk is bound to
                the registered project, clean Git index, original blob, source digest, and HEAD.
                Selecting hunks does not stage, apply, commit, or authorize a Git operation.</p>
              <label htmlFor="g06b-hunk-path">Tracked project-relative text file</label>
              <input id="g06b-hunk-path" type="text" maxLength={240}
                placeholder="src/main.rs" value={gitHunkPath} disabled={gitHunkBusy}
                onChange={event => {
                  setGitHunkPath(event.target.value);
                  setGitHunkPlan(null); setGitSelectedHunkIds([]); setGitHunkSelection(null);
                  gitHunkRequest.current += 1;
                }} />
              <button type="button" className="ghostBtn"
                disabled={!gitHunkPath.trim() || gitHunkBusy}
                onClick={() => previewGitHunks()}>
                {gitHunkBusy ? 'Checking native diff…' : 'Review exact hunks (no stage)'}
              </button>
              {gitHunkPlan && gitHunkPlan.projectId === activeProject.projectId && <div>
                <p>HEAD: <code>{gitHunkPlan.headSha}</code>
                  {' · '}Index: <code>{gitHunkPlan.indexBlobOid}</code></p>
                <p>Source SHA-256: <code>{gitHunkPlan.sourceSha256}</code>
                  {' · '}Hunks: {gitHunkPlan.hunks.length}</p>
                {gitHunkPlan.hunks.length === 0 && <p>No unstaged text hunks were detected.</p>}
                {gitHunkPlan.hunks.map(hunk => <article className="continuityEvent" key={hunk.id}>
                  <label>
                    <input type="checkbox" disabled={gitHunkBusy}
                      checked={gitSelectedHunkIds.includes(hunk.id)}
                      onChange={event => {
                        setGitSelectedHunkIds(previous => event.target.checked
                          ? [...previous, hunk.id] : previous.filter(id => id !== hunk.id));
                        setGitHunkSelection(null);
                      }} />
                    {' '}Select hunk {hunk.position + 1}: <code>{hunk.id.slice(0, 14)}</code>
                  </label>
                  <p>Old lines {hunk.oldStart} (+{hunk.oldLines}) ·
                    New lines {hunk.newStart} (+{hunk.newLines})</p>
                  <pre className="sourcePreview" style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>
                    {hunk.header + hunk.lines.map(line => line.origin + line.text).join('')}
                  </pre>
                </article>)}
                <button type="button" className="ghostBtn"
                  disabled={gitHunkBusy || gitSelectedHunkIds.length === 0}
                  onClick={() => confirmSelectedGitHunks()}>
                  Reverify selected hunks (no stage)
                </button>
                {gitHunkSelection && <div>
                  <p role="status">Reviewed {gitHunkSelection.selectedIds.length} hunk(s).
                    Selection SHA-256: <code>{gitHunkSelection.selectionSha256}</code>
                  </p>
                  <button type="button" className="ghostBtn"
                    disabled={gitPartialStageBusy || gitHunkBusy}
                    onClick={() => stageReviewedSelectedHunks()}>
                    Request native one-use OS approval to stage selected hunks
                  </button>
                </div>}
                {gitPartialStageBusy && <div role="status">
                  Native approval or partial index transaction in progress.
                  <button type="button" className="ghostBtn"
                    onClick={() => cancelProtectedPartialStage()}>
                    Request cancellation before index publication
                  </button>
                </div>}
                <p>Review alone grants no staging or commit authority.
                  Only the native OS approval dialog may authorize a one-use index update.</p>
              </div>}
            </div>}
            {activeProject && <div className="continuityDocument" aria-label="Separately authorized reviewed commit or amendment">
              <h3>Reviewed staged-hunk commit · one-use OS approval</h3>
              <p>Staging hunks does not create a commit. Select a new commit or,
                separately, an explicit rewrite of the latest local commit.
                A valid original native staging receipt is required; the host
                independently checks the private journal, exact index, staged
                tree, original source, HEAD and branch before showing any OS dialog.</p>
              {!gitPartialStageReceipt && <button type="button" className="ghostBtn"
                disabled={reviewedCommitBusy}
                onClick={() => restoreReviewedPartialStageReceipt()}>
                Restore previously verified staged-hunk receipt (read-only)
              </button>}
              <label htmlFor="g06b-commit-message">Commit message</label>
              <textarea id="g06b-commit-message" rows={2} maxLength={4096}
                value={reviewedCommitMessage} disabled={reviewedCommitBusy}
                onChange={event => {
                  setReviewedCommitMessage(event.target.value);
                  setReviewedCommitPlan(null);
                }} />
              <label htmlFor="g06b-author-name">Author name</label>
              <input id="g06b-author-name" type="text" maxLength={160}
                value={reviewedCommitAuthor} disabled={reviewedCommitBusy}
                onChange={event => {
                  setReviewedCommitAuthor(event.target.value);
                  setReviewedCommitPlan(null);
                }} />
              <label htmlFor="g06b-author-email">Author email</label>
              <input id="g06b-author-email" type="email" maxLength={160}
                value={reviewedCommitEmail} disabled={reviewedCommitBusy}
                onChange={event => {
                  setReviewedCommitEmail(event.target.value);
                  setReviewedCommitPlan(null);
                }} />
              <div className="actionRow">
                <button type="button" className="ghostBtn"
                  disabled={!gitPartialStageReceipt || reviewedCommitBusy ||
                    !reviewedCommitMessage.trim() || !reviewedCommitAuthor.trim() ||
                    !reviewedCommitEmail.trim()}
                  onClick={() => previewReviewedPartialCommit('create')}>
                  Review new local commit (no commit yet)
                </button>
                <button type="button" className="ghostBtn"
                  disabled={!gitPartialStageReceipt || reviewedCommitBusy ||
                    !reviewedCommitMessage.trim() || !reviewedCommitAuthor.trim() ||
                    !reviewedCommitEmail.trim()}
                  onClick={() => previewReviewedPartialCommit('amend')}>
                  Review replacement of latest local commit
                </button>
              </div>
              {reviewedCommitPlan && reviewedCommitPlan.stage.projectId === activeProject.projectId &&
                <div>
                  <p role="status">Independent native review: <strong>
                    {reviewedCommitPlan.mode === 'amend' ? 'HISTORY REWRITE' : 'New local commit'}
                  </strong></p>
                  {reviewedCommitPlan.mode === 'amend' && <p role="alert">
                    Amendment replaces the current local HEAD commit. The previous commit
                    will no longer be on this branch. The host refuses an upstream
                    whose publication status cannot be verified; there is no push.
                    Amendment requires two separate OS confirmations.
                  </p>}
                  <p>Branch: <code>{reviewedCommitPlan.branchRef}</code></p>
                  <p>Previous HEAD: <code>{reviewedCommitPlan.previousHead}</code></p>
                  <p>New parent: <code>{reviewedCommitPlan.resultingParent}</code></p>
                  <p>Commit tree: <strong>Materialized only after separate native OS approval</strong></p>
                  <p>Index SHA-256: <code>{reviewedCommitPlan.indexSha256}</code></p>
                  <p>Review SHA-256: <code>{reviewedCommitPlan.planSha256}</code></p>
                  <p>No commit authority follows from preview; the native OS dialog decides.</p>
                  <button type="button" className="ghostBtn"
                    disabled={reviewedCommitBusy}
                    onClick={() => submitApprovedReviewedCommit()}>
                    {reviewedCommitPlan.mode === 'amend'
                      ? 'Request separate OS authorization to amend local HEAD'
                      : 'Request separate OS authorization to create commit'}
                  </button>
                </div>}
              {reviewedCommitBusy && <div role="status">
                Native commit review or OS approval in progress.
                <button type="button" className="ghostBtn"
                  onClick={() => cancelReviewedCommit()}>
                  Request cancellation before branch update
                </button>
              </div>}
              {reviewedCommitReceipt &&
                reviewedCommitReceipt.projectId === activeProject.projectId &&
                <div role="status">
                  <p>Native Git operation independently verified.
                    {reviewedCommitReceipt.amendedExistingHistory
                      ? ' The latest local commit was explicitly replaced.' : ' A new local commit was created.'}
                  </p>
                  <p>Commit: <code>{reviewedCommitReceipt.commitOid}</code></p>
                  <p>Parent: <code>{reviewedCommitReceipt.resultingParent}</code></p>
                  <p>Committed tree: <code>{reviewedCommitReceipt.committedTreeOid}</code></p>
                  <p>Index: <code>{reviewedCommitReceipt.indexSha256}</code></p>
                  <p>Journal verified: {String(reviewedCommitReceipt.journalVerified)}
                    {' · '}Hooks: {String(reviewedCommitReceipt.gitHooksExecuted)}
                    {' · '}Remote updated: {String(reviewedCommitReceipt.remoteUpdated)}</p>
                </div>}
              <h4>Durable local commit recovery</h4>
              <button type="button" className="ghostBtn"
                disabled={reviewedCommitRecoveryBusy}
                onClick={() => inspectReviewedCommitRecovery()}>
                Inspect native commit journal
              </button>
              {reviewedCommitRecovery && reviewedCommitRecovery.projectId === activeProject.projectId &&
                <div>
                  <p>Journal: <strong>{reviewedCommitRecovery.state.replaceAll('_', ' ')}</strong>
                    {' · '}Last phase: {reviewedCommitRecovery.lastPhase || 'none'}</p>
                  <p>Git index lock: {String(reviewedCommitRecovery.gitIndexLockPresent)}</p>
                  {reviewedCommitRecovery.operatorReconciliationRequired && <div>
                    <p role="alert">Interrupted or ambiguous Git ref update. No automatic
                      retry, history rewrite, or lock deletion is authorized.</p>
                    {!reviewedCommitRecovery.gitIndexLockPresent &&
                      <button type="button" className="ghostBtn"
                        disabled={reviewedCommitRecoveryBusy}
                        onClick={() => reconcileReviewedCommitReceipt()}>
                        Request OS approval to independently reconcile receipt
                      </button>}
                  </div>}
                </div>}
            </div>}
            {activeProject && <div className="continuityDocument" aria-label="Native partial Git staging recovery and receipts">
              <h3>Protected partial staging · journal and recovery</h3>
              <button type="button" className="ghostBtn"
                disabled={gitPartialRecoveryBusy}
                onClick={() => inspectPartialStageRecovery()}>
                {gitPartialRecoveryBusy ? 'Inspecting recovery…' : 'Inspect native Git recovery journal'}
              </button>
              {gitPartialStageReceipt && gitPartialStageReceipt.projectId === activeProject.projectId && <div role="status">
                <p>Selected hunk index published and verified. No commit created.</p>
                <p>Staged blob: <code>{gitPartialStageReceipt.stagedBlobOid}</code>
                  {' · '}SHA-256: <code>{gitPartialStageReceipt.stagedBlobSha256}</code></p>
                <p>Index before: <code>{gitPartialStageReceipt.oldIndexSha256}</code></p>
                <p>Index after: <code>{gitPartialStageReceipt.newIndexSha256}</code></p>
                <p>Durable journal: {String(gitPartialStageReceipt.durableJournalVerified)}
                  {' · '}Source modified: {String(gitPartialStageReceipt.sourceFileModified)}</p>
              </div>}
              {gitPartialRecovery && gitPartialRecovery.projectId === activeProject.projectId && <div>
                <p>Journal state: <strong>{gitPartialRecovery.state.replaceAll('_', ' ')}</strong></p>
                <p>Last phase: {gitPartialRecovery.lastPhase || 'none'}
                  {' · '}Existing Git lock: {String(gitPartialRecovery.pendingGitLock)}</p>
                {gitPartialRecovery.operatorReconciliationRequired
                  ? <div>
                    <p role="alert">Interrupted or ambiguous Git index operation.
                      Original Git metadata must be reviewed manually.
                      No automatic rollback or Git lock removal is authorized.</p>
                    {!gitPartialRecovery.pendingGitLock && <button type="button"
                      className="ghostBtn" disabled={gitPartialRecoveryBusy}
                      onClick={() => reconcilePartialStageReceipt()}>
                      Ask OS permission to independently reconcile the journal receipt
                    </button>}
                  </div>
                  : <p>No unfinished native partial-stage receipt detected at inspection time.</p>}
              </div>}
            </div>}
            {activeProject && <div className="continuityDocument" aria-label="Reviewed Git batch planning only">
              <h3>Advanced staging · multi-file review preview</h3>
              <p>Choose up to eight already tracked regular files, separated by newlines or commas.
                This compares exact current source SHA, HEAD and index for a future independently approved
                multi-file stage. Preview and revalidation do not modify Git, stage files, or grant commit rights.</p>
              <label htmlFor="g06b-batch-paths">Tracked project-relative files</label>
              <textarea id="g06b-batch-paths" value={gitBatchPaths} rows={3}
                disabled={gitBatchBusy} placeholder="src/main.rs&#10;src/lib.rs"
                onChange={event => { setGitBatchPaths(event.target.value); setGitBatchReview(null); }} />
              <button type="button" className="ghostBtn" disabled={gitBatchBusy || !gitBatchPaths.trim()}
                onClick={() => previewGitBatch()}>Preview selected files (no stage)</button>
              {gitBatchReview && gitBatchReview.projectId === activeProject.projectId && <div>
                <p>Exact HEAD: <code>{gitBatchReview.headSha}</code>
                  {' · '}Files: {gitBatchReview.files.length}
                  {' · '}Total bytes: {gitBatchReview.totalSourceBytes}</p>
                <p>Index: <code>{gitBatchReview.indexFingerprint}</code></p>
                {gitBatchReview.files.map(item => <article key={item.relativePath} className="continuityEvent">
                  <strong>{item.relativePath}</strong>
                  <p>SHA256: <code>{item.worktreeSha256}</code> · {item.fileSize} bytes</p>
                </article>)}
                <button type="button" className="ghostBtn" disabled={gitBatchBusy}
                  onClick={() => revalidateGitBatch()}>Revalidate exact source (no stage)</button>
                <p>Stage authorized: <strong>No</strong>. Commit authorized: <strong>No</strong>.
                  This is a review plan, not an executable grant.</p>
              </div>}
            </div>}
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

            {activeProject && <div className="continuityDocument"
              aria-label="G06C local Git branch operation review only">
              <h3>Branch operations · review before any execution</h3>
              <p>Inspect an exact local branch tip and Git ancestry for merge, rebase,
                cherry-pick or revert, or assess whether a local stash would affect
                unsaved work. This stage is read-only; it cannot checkout, stash,
                run hooks, fetch, rewrite history or authorize an operation.</p>
              <label htmlFor="g06c-operation-kind">Operation to review</label>
              <select id="g06c-operation-kind" value={branchReviewKind}
                disabled={branchReviewBusy}
                onChange={event => {
                  branchReviewRequest.current += 1;
                  setBranchReviewKind(event.target.value);
                  replayResultRequest.current += 1;
                  setReplayResult(null);
                  setReplayExecReceipt(null);
                  replayPlanRequest.current += 1;
                  setReplayPlan(null);
                  setReplayPlanBusy(false);
                  mergeExecRequest.current += 1;
                  setMergeExecReceipt(null);
                  setBranchReview(null);
                  setBranchReviewBusy(false);
                  mergePlanRequest.current += 1;
                  setMergePlan(null);
                  setMergePlanBusy(false);
                }}>
                <option value="merge">Merge local branch</option>
                <option value="rebase">Rebase onto local branch</option>
                <option value="cherry_pick">Cherry-pick local branch tip</option>
                <option value="revert">Revert local branch tip</option>
                <option value="stash">Review local stash impact</option>
              </select>
              {branchReviewKind !== 'stash' && <>
                <label htmlFor="g06c-selected-local-branch">Existing local branch</label>
                <select id="g06c-selected-local-branch" value={branchReviewTarget}
                  disabled={branchReviewBusy || !gitInventory ||
                    gitInventory.projectId !== activeProject.projectId}
                  onChange={event => {
                    branchReviewRequest.current += 1;
                    setBranchReviewTarget(event.target.value);
                    replayResultRequest.current += 1;
                    setReplayResult(null);
                    setReplayExecReceipt(null);
                    replayPlanRequest.current += 1;
                    setReplayPlan(null);
                    setReplayPlanBusy(false);
                    mergeExecRequest.current += 1;
                    setMergeExecReceipt(null);
                    setBranchReview(null);
                    setBranchReviewBusy(false);
                    mergePlanRequest.current += 1;
                    setMergePlan(null);
                    setMergePlanBusy(false);
                  }}>
                  <option value="">Choose a local branch</option>
                  {(gitInventory?.projectId === activeProject.projectId
                    ? gitInventory.branches : []).filter(entry => entry.tipSha)
                    .map(entry => <option key={entry.name} value={entry.name}>
                      {entry.name}{entry.current ? ' · current (blocked)' : ''}
                    </option>)}
                </select>
              </>}
              <button type="button" className="ghostBtn"
                disabled={branchReviewBusy || gitBusy ||
                  (branchReviewKind !== 'stash' && !branchReviewTarget)}
                onClick={() => void previewLocalBranchOperation()}>
                {branchReviewBusy ? 'Inspecting native Git graph…' :
                  'Inspect operation (no changes)'}
              </button>
              {branchReview && branchReview.projectId === activeProject.projectId &&
                <article className="continuityEvent" role="status">
                  <strong>{branchReview.operation.replaceAll('_',' ')}
                    {' · '}{branchReview.relationship.replaceAll('_',' ')}</strong>
                  <p>Current: <code>{branchReview.currentBranch}</code>
                    {' · '}HEAD: <code>{branchReview.currentHead}</code></p>
                  {branchReview.selectedLocalBranch && <p>
                    Local branch: <code>{branchReview.selectedLocalBranch}</code>
                    {' · '}Tip: <code>{branchReview.selectedTip}</code></p>}
                  {branchReview.mergeBase &&
                    <p>Shared ancestor: <code>{branchReview.mergeBase}</code></p>}
                  {branchReview.currentOnlyCommits !== null &&
                    <p>Current-only commits: {branchReview.currentOnlyCommits}
                      {' · '}Selected-only commits: {branchReview.selectedOnlyCommits}</p>}
                  <p>Git state: {branchReview.gitState} · Index conflicts:
                    {' '}{String(branchReview.indexConflicts)} · Index locked:
                    {' '}{String(branchReview.indexLockPresent)}</p>
                  <p>Changed paths: {branchReview.dirtyEntries}
                    {branchReview.dirtyTruncated ? '+' : ''}
                    {' · '}Untracked present: {String(branchReview.untrackedPresent)}</p>
                  {branchReview.blockers.length > 0
                    ? <p role="alert">Safety blockers:
                        {' '}{branchReview.blockers.join(', ').replaceAll('_', ' ')}</p>
                    : <p>No static safety blockers were observed. This does not
                        authorize a branch operation or prove it can succeed.</p>}
                  <p>Evidence: <code>{branchReview.evidenceSha256}</code></p>
                  <p>Authorization: none · Execution route: unavailable · No Git
                    writes, process launch or network access.</p>
                </article>}
              {['rebase', 'cherry_pick'].includes(branchReviewKind) &&
                branchReview?.projectId === activeProject.projectId &&
                branchReview.operation === branchReviewKind &&
                branchReview.selectedLocalBranch === branchReviewTarget &&
                <div aria-label="G06C04 original rebase cherry-pick history review no execution">
                  <h4>Commit replay · original-history review only</h4>
                  <p>{branchReviewKind === 'rebase'
                    ? 'Inspect the current branch commits that would be replayed on the selected local branch. Rebase rewrites commit identity and may invalidate shared history.'
                    : 'Inspect ONLY the existing selected local branch tip and its parent. Cherry-picking creates a new commit rather than moving the source branch.'}
                    {' '}No resulting trees are built. Conflict and rename analysis remain
                    incomplete; even an empty overlap list is not approval to execute.</p>
                  <button type="button" className="ghostBtn"
                    disabled={replayPlanBusy || branchReviewBusy}
                    onClick={() => void previewRebaseCherryReplay()}>
                    {replayPlanBusy ? 'Reading native original commits…' :
                      'Review exact replay commits (no Git changes)'}
                  </button>
                  {replayPlan?.projectId === activeProject.projectId &&
                    replayPlan.selectedLocalBranch === branchReviewTarget &&
                    replayPlan.upstreamReviewSha256 === branchReview.evidenceSha256 &&
                    <article className="continuityEvent" role="status">
                      <strong>{replayPlan.operation.replaceAll('_', ' ')}</strong>
                      <p>Current HEAD: <code>{replayPlan.currentHead}</code>
                        {' · '}Target: <code>{replayPlan.selectedTip}</code></p>
                      <p>Unique merge base: <code>{replayPlan.mergeBase}</code></p>
                      <p>Original index SHA-256: <code>{replayPlan.originalIndexSha256}</code></p>
                      <p>Original replay commits: {replayPlan.replayCommits.length}
                        {' · '}Changed paths: {replayPlan.changedPathCount}
                        {' · '}Potential path overlap: {replayPlan.possibleOverlapCount}</p>
                      <p>History rewrite: {String(replayPlan.historyRewrite)}
                        {' · '}Original commits retained: {String(replayPlan.originalCommitsRetained)}</p>
                      {replayPlan.blockers.length > 0 &&
                        <p role="alert">Blockers: {replayPlan.blockers.join(', ').replaceAll('_', ' ')}</p>}
                      <div className="continuityEvents">
                        {replayPlan.replayCommits.map(item => <article className="continuityEvent"
                          key={item.originalCommit}>
                          <strong>Commit {item.ordinal}: <code>{item.originalCommit}</code></strong>
                          <p>{item.commitMessagePreview}</p>
                          <p>Parent: <code>{item.originalParent}</code>
                            {' · '}Tree: <code>{item.originalTree}</code></p>
                          <p>Paths: {item.visiblePaths.join(', ') || 'no paths'}
                            {item.omittedPathCount > 0 ? ' · additional paths hidden' : ''}
                            {' · '}Unsupported deltas: {item.unsupportedDeltaCount}</p>
                          <p>Original path ledger SHA-256: <code>{item.pathLedgerSha256}</code></p>
                        </article>)}
                      </div>
                      <p>Evidence digest: <code>{replayPlan.evidenceSha256}</code></p>
                      <p>No Git ref, index, worktree or remote mutation. No model or
                        WebView authorization; no execution route for rebase or cherry-pick.</p>
                    </article>}
                </div>}
                  {replayPlan && replayPlan.projectId === activeProject.projectId &&
                    replayPlan.upstreamReviewSha256 === branchReview.evidenceSha256 &&
                    <div aria-label="G06C04 isolated real replay result and separate OS authorization">
                      <h4>Isolated resulting trees · actual Git conflict inspection</h4>
                      <p>Replays original commits into a temporary bare Git repository.
                        Resulting Git commit/tree IDs can be verified without modifying the
                        project. Conflict findings are conservative; external merge drivers,
                        hooks and remote operations are excluded.</p>
                      <button type="button" className="ghostBtn"
                        disabled={replayResultBusy || replayExecBusy}
                        onClick={() => void previewIsolatedReplayResult()}>
                        {replayResultBusy ? 'Testing isolated Git replay…' :
                          'Verify prospective result in quarantine (no project writes)'}
                      </button>
                      {replayResult && replayResult.projectId === activeProject.projectId &&
                        replayResult.replayPlanSha256 === replayPlan.evidenceSha256 &&
                        <article className="continuityEvent" role="status">
                          <p>Predicted HEAD: <code>{replayResult.prospectiveHead || 'not verified'}</code></p>
                          <p>Predicted tree: <code>{replayResult.prospectiveTree || 'not verified'}</code></p>
                          <p>Result commits: {replayResult.resultSteps.length}
                            {' · '}Files changed: {replayResult.candidateChangePaths}
                            {' · '}Actual conflicts: {replayResult.conflictCount}</p>
                          {replayResult.resultSteps.map(step =>
                            <p key={step.originalCommit + step.ordinal}>
                              Original: <code>{step.originalCommit}</code>
                              {' → '}New: <code>{step.resultingCommit}</code>
                              {' · '}Tree: <code>{step.resultingTree}</code>
                            </p>)}
                          {replayResult.conflictingPaths.length > 0 &&
                            <p role="alert">Conflicting paths:
                              {' '}{replayResult.conflictingPaths.join(', ')}</p>}
                          {replayResult.blockers.length > 0 &&
                            <p role="alert">Publication blockers:
                              {' '}{replayResult.blockers.join(', ').replaceAll('_', ' ')}</p>}
                          <p>Review SHA-256: <code>{replayResult.evidenceSha256}</code></p>
                          <p>Quarantine only: {String(replayResult.quarantineOnlyResultObjects)}.
                            Original project object database, refs, index and worktree untouched
                            by this review. No approval is issued by the model or WebView.</p>
                          {replayResult.blockers.length === 0 &&
                            replayResult.conflictCount === 0 &&
                            replayResult.candidateChangePaths > 0 &&
                            replayResult.candidateChangePaths <= 80 &&
                            replayResult.prospectiveHead && replayResult.prospectiveTree &&
                            <div aria-label="G06C04 distinct one-use OS replay publication">
                              <p role="alert">{replayResult.operation === 'rebase_current_onto_local'
                                ? 'Rebase REWRITES the current local branch history. The original tip will be preserved under a durable recovery ref before publication.'
                                : 'Cherry-pick appends a new local commit without moving the selected source branch.'}
                                {' '}The native OS dialog will request separate one-use approval.
                                Checkout precedes exact ref CAS; interruption may need manual repair.</p>
                              <button type="button" className="primaryBtn"
                                disabled={replayExecBusy || replayResultBusy}
                                onClick={() => void executeProtectedReplay()}>
                                {replayExecBusy ? 'Native operator consent / replay in progress…' :
                                  replayResult.operation === 'rebase_current_onto_local'
                                    ? 'Request separate OS permission for exact history rewrite'
                                    : 'Request OS permission for exact cherry-pick'}
                              </button>
                              {replayExecBusy &&
                                <button type="button" className="ghostBtn"
                                  onClick={() => void cancelProtectedReplay()}>
                                  Request cancellation before object publication
                                </button>}
                            </div>}
                        </article>}
                    </div>}
              {branchReviewKind === 'merge' &&
                branchReview?.projectId === activeProject.projectId &&
                branchReview.selectedLocalBranch === branchReviewTarget && <div
                  aria-label="G06C02 reviewed local merge plan no execution">
                  <h4>Reviewed merge planning · exact source trees</h4>
                  <p>Compare committed Git trees and the saved index without making a merge.
                    This is a conservative path-level overlap assessment, not a completed
                    three-way content merge or a promise of conflict-free execution.</p>
                  <button type="button" className="ghostBtn"
                    disabled={mergePlanBusy || branchReviewBusy}
                    onClick={() => void previewExactLocalMergePlan()}>
                    {mergePlanBusy ? 'Checking tree and ref evidence…' :
                      'Review possible merge and conflicts (no write)'}
                  </button>
                  {mergePlan && mergePlan.projectId === activeProject.projectId &&
                    mergePlan.upstreamReviewSha256 === branchReview.evidenceSha256 &&
                    <article className="continuityEvent" role="status">
                      <strong>{mergePlan.classification.replaceAll('_', ' ')}</strong>
                      <p>Current HEAD: <code>{mergePlan.currentHead}</code>
                        {' · '}Current tree: <code>{mergePlan.currentTree}</code></p>
                      <p>Selected tip: <code>{mergePlan.selectedTip}</code>
                        {' · '}Selected tree: <code>{mergePlan.selectedTree}</code></p>
                      <p>Unique merge base: {mergePlan.mergeBaseCount === 1
                        ? <code>{mergePlan.mergeBaseCommit}</code>
                        : 'not verified'}
                        {' · '}Base tree: <code>{mergePlan.mergeBaseTree || 'not available'}</code></p>
                      <p>Index SHA-256: <code>{mergePlan.indexSha256}</code></p>
                      <p>Changed paths: {mergePlan.changedPathCount}
                        {' · '}Potential conflicts: {mergePlan.possibleConflictCount}
                        {' · '}Hidden paths: {mergePlan.omittedPathCount}</p>
                      <p>Review: {mergePlan.conflictReview.replaceAll('_', ' ')}</p>
                      {mergePlan.blockers.length > 0 &&
                        <p role="alert">Blockers: {mergePlan.blockers.join(', ').replaceAll('_', ' ')}</p>}
                      <div className="continuityEvents">
                        {mergePlan.visiblePaths.map(item => <article
                          className="continuityEvent" key={item.relativePath}>
                          <strong>{item.relativePath}</strong>
                          <p>{item.classification.replaceAll('_', ' ')}
                            {' · '}Manual review: {String(item.requiresManualResolution)}</p>
                          <p>Base: <code>{item.baseOid || 'absent'}</code>
                            {' · '}Current: <code>{item.currentOid || 'absent'}</code>
                            {' · '}Selected: <code>{item.selectedOid || 'absent'}</code></p>
                        </article>)}
                      </div>
                  {mergePlan.classification === 'fast_forward_candidate' &&
                    mergePlan.blockers.length === 0 &&
                    mergePlan.changedPathCount > 0 &&
                    mergePlan.changedPathCount <= 80 &&
                    mergePlan.omittedPathCount === 0 &&
                    mergePlan.possibleConflictCount === 0 &&
                    <div aria-label="G06C03 OS protected fast-forward execution">
                      <p>Fast-forward candidate only: execution requires a NEW,
                        one-use native OS confirmation. Local working files and
                        index change before the exact branch-ref update; interruption
                        may leave a worktree requiring manual review. This review
                        digest alone never grants execution.</p>
                      <button type="button" className="primaryBtn"
                        disabled={mergeExecBusy || mergePlanBusy}
                        onClick={() => void executeReviewedFastForward()}>
                        Request OS permission to fast-forward this exact local branch
                      </button>
                      {mergeExecBusy && <div role="status">
                        Native approval/checkout in progress. Cancellation is possible
                        only before initial worktree mutation.
                        <button type="button" className="ghostBtn"
                          onClick={() => void cancelProtectedFastForward()}>
                          Request pre-checkout cancellation
                        </button>
                      </div>}
                    </div>}
                  {mergePlan.classification === 'divergent_merge_candidate' &&
                    <div aria-label="G06C03B quarantined real divergent merge">
                      <h4>Verified two-parent divergent merge · native Git content</h4>
                      <p>The earlier path-overlap plan is NOT an execution grant.
                        This separate check computes the actual three-way result in an
                        isolated bare Git object store and independently verifies its
                        unique base, both parents, output tree and conflict paths.</p>
                      <button type="button" className="ghostBtn"
                        disabled={divergentBusy || divergentExecBusy}
                        onClick={() => void verifyDivergentMergeResult()}>
                        {divergentBusy ? 'Testing actual content conflicts…' :
                          'Verify divergent result in quarantine (read-only)'}
                      </button>
                      {divergentResult && divergentResult.projectId === activeProject.projectId &&
                        divergentResult.sourcePlanSha256 === mergePlan.evidenceSha256 &&
                        <article className="continuityEvent" role="status">
                          <p>First parent: <code>{divergentResult.firstParent}</code></p>
                          <p>Second parent: <code>{divergentResult.secondParent}</code></p>
                          <p>Quarantined prospective tree:
                            {' '}<code>{divergentResult.prospectiveTree || 'no conflict-free tree'}</code></p>
                          <p>Actual Git conflicts: {divergentResult.conflictCount}
                            {' · '}Result entries: {divergentResult.proposedEntryCount}</p>
                          {divergentResult.conflictCount > 0 &&
                            <p role="alert">Execution denied. Conflicting paths:
                              {' '}{divergentResult.conflicts.join(', ')}</p>}
                          <p>Native result evidence:
                            {' '}<code>{divergentResult.resultSha256}</code></p>
                          <p>No authorization was created by this preview.
                            Conflicts require manual Git resolution outside this operation.</p>
                          {divergentResult.conflictFreePreview === true &&
                            divergentResult.conflictCount === 0 &&
                            divergentResult.prospectiveTree &&
                            <div aria-label="G06C03B separate native OS two-parent merge consent">
                              <p role="alert">Publication creates a NEW two-parent local
                                merge commit. The OS must grant a distinct one-use approval.
                                The source branch is retained. Checkout and ref publication
                                use a durable journal; interruption may require manual repair.</p>
                              <button type="button" className="primaryBtn"
                                disabled={divergentExecBusy || divergentBusy}
                                onClick={() => void executeProtectedDivergentMerge()}>
                                {divergentExecBusy ? 'Awaiting native approval / publishing…' :
                                  'Request separate OS approval for exact two-parent merge'}
                              </button>
                              {divergentExecBusy &&
                                <button type="button" className="ghostBtn"
                                  onClick={() => void cancelProtectedDivergentMerge()}>
                                  Request cancellation before object publication
                                </button>}
                            </div>}
                        </article>}
                    </div>}
                      <p>Evidence digest: <code>{mergePlan.evidenceSha256}</code></p>
                      <p>Source rechecked: {String(mergePlan.sourceRechecked)}.
                        Authorization: none. Execution route: unavailable.
                        No ref/index/worktree mutation, hooks, merge drivers,
                        process launch, or network operations.</p>
                    </article>}
                </div>}
            </div>}
            {activeProject && <div className="continuityDocument"
              aria-label="G06C04 protected rebase cherry-pick original history restart recovery">
              <h3>Rebase / Cherry-pick · historical receipts and restart recovery</h3>
              <p>No automatic ref retry, history rewrite, checkout, rollback or foreign index
                lock deletion. Interrupted operations require manual review.
                Physical installed human consent and restart acceptance remain deferred.</p>
              <button type="button" className="ghostBtn"
                disabled={replayRecoveryBusy || replayExecBusy}
                onClick={() => void inspectReplayRecovery()}>
                {replayRecoveryBusy ? 'Checking original Git history…' :
                  'Inspect protected replay journal and original history'}
              </button>
              {replayExecReceipt?.projectId === activeProject.projectId &&
                <article className="continuityEvent" role="status">
                  <strong>Protected original replay verified · {replayExecReceipt.operation.replaceAll('_',' ')}</strong>
                  <p>Original HEAD: <code>{replayExecReceipt.originalHead}</code></p>
                  <p>Published HEAD: <code>{replayExecReceipt.publishedHead}</code></p>
                  <p>Original-history recovery ref:
                    {' '}<code>{replayExecReceipt.originalHistoryRef || 'parent retained'}</code></p>
                  <p>Result tree: <code>{replayExecReceipt.resultingTree}</code></p>
                  <p>Original object history preserved. No push or hooks executed.</p>
                </article>}
              {replayRecovery?.projectId === activeProject.projectId &&
                <article className="continuityEvent" role="status">
                  <strong>{replayRecovery.state.replaceAll('_',' ')}</strong>
                  <p>Journal: {replayRecovery.lastPhase}
                    {' · '}Lock present: {String(replayRecovery.indexLockPresent)}
                    {' · '}Original history verified: {String(replayRecovery.backupRefVerified)}</p>
                  <p>HEAD original: {String(replayRecovery.branchMatchesOriginal)}
                    {' · '}HEAD candidate: {String(replayRecovery.branchMatchesCandidate)}</p>
                  {replayRecovery.operatorReconciliationRequired &&
                    <p role="alert">Manual review required. No automatic replay, rollback,
                      index lock removal or branch rewrite will run.</p>}
                  {replayRecovery.state === 'published_ref_requires_separate_receipt' &&
                    !replayRecovery.indexLockPresent && replayRecovery.backupRefVerified &&
                    <button type="button" className="ghostBtn" disabled={replayRecoveryBusy}
                      onClick={() => void reconcileReplayReceipt()}>
                      Ask OS permission to verify historical receipt only
                    </button>}
                </article>}
            </div>}
            {activeProject && <div className="continuityDocument"
              aria-label="G06D01 native unresolved three-way conflict inspection">
              <h3>Conflict resolution · exact unresolved Git index stages</h3>
              <p>Read-only inspection of genuine stage 1 (base), stage 2 (ours),
                and stage 3 (theirs) Git entries. Source blob identities and
                bounded previews are shown without modifying files, staging,
                creating a commit, or granting resolution authority.</p>
              <button type="button" className="ghostBtn"
                disabled={nativeConflictsBusy}
                onClick={() => void inspectUnresolvedNativeConflicts()}>
                {nativeConflictsBusy ? 'Inspecting original index objects…' :
                  'Inspect unresolved Git conflicts (read only)'}
              </button>
              {nativeConflicts?.projectId === activeProject.projectId &&
                <article role="status" className="continuityEvent">
                  <p>Branch: {nativeConflicts.currentBranch}
                    {' · '}Unresolved files: {nativeConflicts.conflictCount}</p>
                  <p>Index SHA-256: <code>{nativeConflicts.indexSha256}</code></p>
                  <p>Review SHA-256: <code>{nativeConflicts.evidenceSha256}</code></p>
                  {nativeConflicts.conflicts.map(entry =>
                    <article className="continuityEvent" key={entry.relativePath}>
                      <strong>{entry.relativePath}</strong>
                      <p>Classification: {entry.classification.replaceAll('_',' ')}
                        {' · '}Human resolution required</p>
                      {[['Base · stage 1', entry.ancestor],
                        ['Ours · stage 2', entry.ours],
                        ['Theirs · stage 3', entry.theirs]].map(([name, value]) =>
                          <div key={name}>
                            <p>{name}: {value ? <code>{value.oid}</code> : 'absent'}</p>
                            {value?.binaryOrNonUtf8 &&
                              <p>Binary or non-UTF8 data: preview withheld</p>}
                            {value?.preview != null &&
                              <pre className="continuityEvent">{value.preview}
                                {value.previewTruncated ? ' [truncated]' : ''}</pre>}
                          </div>)}
                    </article>)}
                  <div aria-label="G06D02 offline three-way conflict draft editor">
                    <h4>Review a proposed resolution · local draft only</h4>
                    <p>A proposal is never written to disk. Only a complete, readable
                      stage preview may be copied. Truncated/binary stages are blocked;
                      manual drafts start empty. A new inspection invalidates the draft.</p>
                    {nativeConflicts.conflicts.map(entry =>
                      <div key={'draft-' + entry.relativePath}>
                        <strong>{entry.relativePath}</strong>
                        <div>
                          <button type="button" className="ghostBtn"
                            disabled={!entry.ours || entry.ours.binaryOrNonUtf8 ||
                              entry.ours.previewTruncated || entry.ours.preview == null}
                            onClick={() => selectConflictDraft(entry, 'ours')}>
                            Draft from complete ours
                          </button>
                          <button type="button" className="ghostBtn"
                            disabled={!entry.theirs || entry.theirs.binaryOrNonUtf8 ||
                              entry.theirs.previewTruncated || entry.theirs.preview == null}
                            onClick={() => selectConflictDraft(entry, 'theirs')}>
                            Draft from complete theirs
                          </button>
                          <button type="button" className="ghostBtn"
                            onClick={() => selectConflictDraft(entry, 'manual')}>
                            Draft manually from empty
                          </button>
                        </div>
                      </div>)}
                    {conflictDraft?.projectId === activeProject.projectId &&
                      conflictDraft.indexSha256 === nativeConflicts.indexSha256 &&
                      conflictDraft.evidenceSha256 === nativeConflicts.evidenceSha256 &&
                      nativeConflicts.conflicts.some(
                        e => e.relativePath === conflictDraft.relativePath) &&
                      <article className="continuityEvent" role="status">
                        <p>Unsaved resolution candidate: <strong>{conflictDraft.relativePath}</strong>
                          {' · '}Basis: {conflictDraftMode}</p>
                        <label htmlFor="g06d02-proposed-text">
                          Proposed full replacement text (not applied)
                        </label>
                        <textarea id="g06d02-proposed-text" rows={12}
                          value={conflictDraft.candidateText}
                          maxLength={131072}
                          spellCheck={false}
                          onChange={e => {
                            setConflictReview(null);
                            setConflictDraft(old =>
                              old ? { ...old, candidateText: e.target.value } : null);
                          }} />
                        <p>Bound to Git HEAD <code>{conflictDraft.currentHead}</code>
                          {' · '}Original index <code>{conflictDraft.indexSha256}</code></p>
                        <button type="button" className="ghostBtn"
                          disabled={conflictReviewBusy || conflictApplyBusy ||
                            conflictDraft.candidateText.length > 131072}
                          onClick={() => void reviewNativeConflictCandidate()}>
                          {conflictReviewBusy ? 'Verifying source and proposal…' :
                            'Review exact replacement with native Git evidence'}
                        </button>
                        {conflictReview?.projectId === activeProject.projectId &&
                          conflictReview.candidateText === conflictDraft.candidateText &&
                          conflictReview.relativePath === conflictDraft.relativePath &&
                          conflictReview.originalIndexSha256 === nativeConflicts.indexSha256 &&
                          conflictReview.inventoryEvidenceSha256 === nativeConflicts.evidenceSha256 &&
                          <article className="continuityEvent"
                            aria-label="G06D03 reviewed resolution proposal">
                            <strong>Native reviewed proposal · no authority</strong>
                            <p>File: {conflictReview.relativePath}
                              {' · '}Exact bytes: {conflictReview.proposedBytes}
                              {' · '}Mode: {conflictReview.fileMode.toString(8)}</p>
                            <p>Proposed text SHA-256:
                              <code>{conflictReview.proposedContentSha256}</code></p>
                            <p>Source review SHA-256:
                              <code>{conflictReview.reviewSha256}</code></p>
                            <p>Original file SHA-256:
                              <code>{conflictReview.originalWorktreeSha256}</code></p>
                            <p role="alert">This digest is NOT permission. Pressing Apply
                              requests separate native OS human consent, then verifies
                              the original HEAD, index, three Git stages and worktree again.
                              An interrupted file/index update may require manual repair.</p>
                            <button type="button" className="primaryBtn"
                              disabled={conflictApplyBusy || conflictReviewBusy}
                              onClick={() => void applyNativeConflictResolution()}>
                              {conflictApplyBusy ? 'Native approval / resolution in progress…' :
                                'Request OS permission to apply this exact resolution'}
                            </button>
                            {conflictApplyBusy &&
                              <button type="button" className="ghostBtn"
                                onClick={() => void invoke('ide_git_cancel_protected_merge', {
                                  projectId: activeProject.projectId,
                                }).catch(e => setProblem(String(e)))}>
                                Request cancellation before mutation
                              </button>}
                          </article>}
                        <p role="alert">Uncommitted local editor draft. No index stage,
                          file replacement, branch publication, or authority has been granted.
                          Closing/restarting this view discards the draft.</p>
                        <button type="button" className="ghostBtn"
                          onClick={() => setConflictDraft(null)}>
                          Discard local resolution draft
                        </button>
                      </article>}
                  </div>
                  <p>No resolution applied. No checkout, commit, ref update or remote contact.</p>
                </article>}
            </div>}
            {activeProject && <div className="continuityDocument"
              aria-label="G06D04 protected resolution journal and restart recovery">
              <h3>Conflict resolution · durable restart recovery</h3>
              <p>Read-only inspection of app-private journal, original Git HEAD,
                index stages, resolved blob and original worktree. Partial writes
                require manual repair; there is no automatic retry or rollback.</p>
              <button type="button" className="ghostBtn"
                disabled={conflictRecoveryBusy || conflictApplyBusy}
                onClick={() => void inspectNativeConflictRecovery()}>
                {conflictRecoveryBusy ? 'Checking journal and original source…' :
                  'Inspect protected conflict-resolution recovery'}
              </button>
              {conflictApplyReceipt?.projectId === activeProject.projectId &&
                <article role="status" className="continuityEvent">
                  <strong>Native resolution applied and independently read back</strong>
                  <p>Path: {conflictApplyReceipt.relativePath}
                    {' · '}Resolved blob: <code>{conflictApplyReceipt.candidateBlobOid}</code></p>
                  <p>Final index: <code>{conflictApplyReceipt.resultingIndexSha256}</code></p>
                  <p>Original HEAD preserved: <code>{conflictApplyReceipt.finalHead}</code></p>
                </article>}
              {conflictRecovery?.projectId === activeProject.projectId &&
                <article role="status" className="continuityEvent">
                  <p>State: <strong>{conflictRecovery.state.replaceAll('_', ' ')}</strong></p>
                  <p>Path: {conflictRecovery.relativePath || 'no operation'}
                    {' · '}Phase: {conflictRecovery.phase || 'none'}</p>
                  <p>Original Git HEAD matches: {String(conflictRecovery.originalHeadMatches)}
                    {' · '}Staged resolution matches: {String(conflictRecovery.candidateIndexEntryMatches)}
                    {' · '}Target file matches proposal: {String(conflictRecovery.proposedWorktreeMatches)}</p>
                  {conflictRecovery.state === 'partial_or_drifted_manual_repair' &&
                    <p role="alert">Interrupted or changed source: manual Git repair required.
                      No automatic restore, index staging, checkout or ref update.</p>}
                  {['resolved_requires_independent_receipt',
                    'unpublished_original_source_preserved'].includes(conflictRecovery.state) &&
                    <button type="button" className="ghostBtn"
                      disabled={conflictRecoveryBusy}
                      onClick={() => void reconcileNativeConflictReceipt()}>
                      Request separate native OS receipt-only reconciliation
                    </button>}
                </article>}
            </div>}
            {activeProject && <div className="continuityDocument"
              aria-label="G06C03B two-parent divergent merge durable recovery">
              <h3>Divergent merge · two-parent custody and restart recovery</h3>
              <p>Exact Git parent and tree readback. No automatic merge retry, ref rewrite,
                checkout, rollback or deletion of another process's Git lock.
                Physical-human OS consent and installed acceptance remain deferred.</p>
              <button type="button" className="ghostBtn"
                disabled={divergentRecoveryBusy || divergentExecBusy}
                onClick={() => void inspectDivergentRecovery()}>
                {divergentRecoveryBusy ? 'Re-reading native Git journal…' :
                  'Inspect divergent merge journal and interrupted state'}
              </button>
              {divergentReceipt?.projectId === activeProject.projectId &&
                <article className="continuityEvent" role="status">
                  <strong>Two-parent protected merge independently verified</strong>
                  <p>First parent: <code>{divergentReceipt.firstParent}</code></p>
                  <p>Second parent: <code>{divergentReceipt.secondParent}</code></p>
                  <p>Published merge commit:
                    {' '}<code>{divergentReceipt.publishedMergeCommit}</code></p>
                  <p>Result tree: <code>{divergentReceipt.resultingTree}</code></p>
                  <p>Operation: <code>{divergentReceipt.operationId}</code></p>
                </article>}
              {divergentRecovery?.projectId === activeProject.projectId &&
                <article className="continuityEvent" role="status">
                  <strong>{divergentRecovery.state.replaceAll('_', ' ')}</strong>
                  <p>Journal phase: {divergentRecovery.phase}
                    {' · '}Foreign lock: {String(divergentRecovery.foreignIndexLockPresent)}</p>
                  <p>Original HEAD: {String(divergentRecovery.originalHeadMatches)}
                    {' · '}Candidate HEAD: {String(divergentRecovery.candidateHeadMatches)}
                    {' · '}Selected branch preserved:
                    {' '}{String(divergentRecovery.selectedBranchPreserved)}</p>
                  {divergentRecovery.operatorReconciliationRequired &&
                    <p role="alert">Operator review required. No unattended mutation is allowed.</p>}
                  {['published_requires_independent_receipt',
                    'unpublished_no_index_change_needs_review'].includes(divergentRecovery.state) &&
                    !divergentRecovery.foreignIndexLockPresent &&
                    <button type="button" className="ghostBtn"
                      disabled={divergentRecoveryBusy}
                      onClick={() => void reconcileDivergentReceipt()}>
                      Request distinct OS consent for receipt-only reconciliation
                    </button>}
                </article>}
            </div>}
            {activeProject && <div className="continuityDocument"
              aria-label="G06C03 native protected merge and restart recovery">
              <h3>Protected merge · installed recovery inspection</h3>
              <p>Read-only inspection of durable native merge journal and exact local
                Git state. No automatic checkout, branch rewrite, rollback or lock deletion.
                Physical operator and installed application acceptance remain deferred.</p>
              <button type="button" className="ghostBtn"
                disabled={mergeRecoveryBusy || mergeExecBusy}
                onClick={() => void inspectProtectedMergeRecovery()}>
                {mergeRecoveryBusy ? 'Inspecting durable Git evidence…' :
                  'Inspect interrupted merge / restart receipt'}
              </button>
              {mergeExecReceipt && mergeExecReceipt.projectId === activeProject.projectId &&
                <article className="continuityEvent" role="status">
                  <strong>Fast-forward independently verified</strong>
                  <p>Previous HEAD: <code>{mergeExecReceipt.previousHead}</code></p>
                  <p>Current HEAD: <code>{mergeExecReceipt.resultingHead}</code></p>
                  <p>Tree: <code>{mergeExecReceipt.resultingTree}</code></p>
                  <p>Native operation: <code>{mergeExecReceipt.operationId}</code></p>
                  <p>Source plan: <code>{mergeExecReceipt.planSha256}</code></p>
                  <p>Original Git history preserved. No network request or Git hook.</p>
                </article>}
              {mergeRecovery && mergeRecovery.projectId === activeProject.projectId &&
                <article className="continuityEvent" role="status">
                  <strong>{mergeRecovery.state.replaceAll('_', ' ')}</strong>
                  <p>Journal phase: {mergeRecovery.lastPhase || 'none'}
                    {' · '}Index locked: {String(mergeRecovery.gitIndexLockPresent)}
                    {' · '}Worktree vs index: {String(mergeRecovery.worktreeMatchesCurrentIndex)}</p>
                  <p>Original HEAD: {String(mergeRecovery.branchMatchesOriginal)}
                    {' · '}Selected HEAD: {String(mergeRecovery.branchMatchesSelected)}</p>
                  {mergeRecovery.operatorReconciliationRequired &&
                    <p role="alert">Operator review required. Uncertain effects must
                      be repaired separately; Lu will not retry the merge or
                      delete a foreign lock.</p>}
                  {mergeRecovery.operatorReconciliationRequired &&
                    !mergeRecovery.gitIndexLockPresent &&
                    ['published_ref_requires_operator_receipt',
                     'prepublication_no_index_change_pending_review']
                      .includes(mergeRecovery.state) &&
                    <button type="button" className="ghostBtn"
                      disabled={mergeRecoveryBusy}
                      onClick={() => void reconcileProtectedMergeReceipt()}>
                      Ask OS consent to verify the historical receipt only
                    </button>}
                </article>}
            </div>}
            {gitInventory && gitInventory.projectId === activeProject?.projectId &&
              <div className="continuityDocument" aria-label="Local branches and linked worktrees">
                <h3>Branches and linked worktrees · read-only</h3>
                <p>Repository: {gitInventory.repositoryKind.replaceAll('_', ' ')}
                  {' · '}HEAD: {gitInventory.headState.replaceAll('_', ' ')}
                  {' · '}Inventory: {gitInventory.state.replaceAll('_', ' ')}</p>
                <h4>Local branches ({gitInventory.branches.length}{gitInventory.branchesTruncated ? '+' : ''})</h4>
                {gitInventory.branches.length === 0 && <p>No verified branch names available.</p>}
                <div className="continuityEvents">
                  {gitInventory.branches.map(item => <article className="continuityEvent" key={item.name}>
                    <strong>{item.name}{item.current ? ' · current' : ''}</strong>
                    <p>Local tip: <code>{item.tipSha || 'unresolved'}</code></p>
                    <p>Stored upstream: {item.upstreamName || 'not verified'}
                      {' · '}{item.upstreamState.replaceAll('_', ' ')}</p>
                    {item.ahead !== null && <p>Ahead: {item.ahead} · Behind: {item.behind}</p>}
                  </article>)}
                </div>
                {gitInventory.branchesTruncated && <p role="status">Branch inventory is capped at 96 entries.</p>}
                <h4>Linked worktrees ({gitInventory.worktrees.length}{gitInventory.worktreesTruncated ? '+' : ''})</h4>
                {gitInventory.worktrees.length === 0 && <p>No linked worktrees observed in repository metadata.</p>}
                <div className="continuityEvents">
                  {gitInventory.worktrees.map(item => <article className="continuityEvent" key={item.name}>
                    <strong>{item.name}{item.currentProject ? ' · current project' : ''}</strong>
                    <p>{item.administrativeState.replaceAll('_', ' ')}
                      {' · '}{item.pathState.replaceAll('_', ' ')}</p>
                    <p>Not registered for access; external path hidden. Locked state: {item.lockedState.replaceAll('_', ' ')}.</p>
                  </article>)}
                </div>
                {gitInventory.worktreesTruncated && <p role="status">Worktree inventory is capped at 48 entries.</p>}
                <p>No branch changes, worktree access grants, Git processes or network requests are made by this inventory.</p>
              </div>}
            {gitRefsHealth && gitRefsHealth.projectId === activeProject?.projectId &&
              <div className="continuityDocument" aria-label="Local Git refs and repository health">
                <h3>References, remotes and repository health · read-only</h3>
                <p>Repository state: {gitRefsHealth.health.repositoryState}
                  {' · '}Index conflicts: {String(gitRefsHealth.health.indexConflicts)}
                  {' · '}Shallow: {String(gitRefsHealth.health.shallow)}
                  {' · '}Detached: {String(gitRefsHealth.health.detachedHead)}</p>
                <p>Remote aliases (URLs and credentials hidden):
                  {' '}{gitRefsHealth.remoteAliases.join(', ') || 'none observed'}</p>
                <p>Local/tracking/tag refs ({gitRefsHealth.localRefs.length}{gitRefsHealth.refsTruncated ? '+' : ''}):</p>
                <div className="continuityEvents">
                  {gitRefsHealth.localRefs.map(item => <article className="continuityEvent" key={item.kind + item.name}>
                    <strong>{item.name}</strong>
                    <p>{item.kind.replaceAll('_', ' ')} · SHA: <code>{item.resolvedSha || 'unresolved'}</code></p>
                  </article>)}
                </div>
                {(gitRefsHealth.refsTruncated || gitRefsHealth.remotesTruncated) &&
                  <p role="status">Git ref/remote evidence is truncated.</p>}
                <p>Remote references are locally stored; no live fetch or remote verification was performed.</p>
              </div>}
            {activeProject && <div className="continuityDocument" aria-label="Committed file history and blame">
              <h3>File history and blame · local Git objects</h3>
              <label htmlFor="g06a-file-history-path">Tracked project-relative file</label>
              <input id="g06a-file-history-path" type="text" value={gitFilePath}
                placeholder="src/main.rs" maxLength={240} disabled={gitFileBusy}
                onChange={event => { setGitFilePath(event.target.value); setGitFileHistory(null); }} />
              <button type="button" className="ghostBtn"
                disabled={gitFileBusy || !gitFilePath.trim()}
                onClick={() => loadGitFileHistory()}>
                {gitFileBusy ? 'Inspecting…' : 'Inspect committed file history'}
              </button>
              {gitFileHistory && gitFileHistory.projectId === activeProject.projectId &&
                <div>
                  <p>HEAD: <code>{gitFileHistory.headSha}</code>
                    {' · '} {gitFileHistory.blame.length} blame hunk(s)</p>
                  {gitFileHistory.blame.map((entry, idx) =>
                    <p key={idx}>Lines {entry.finalStartLine}–{entry.finalStartLine + entry.lineCount - 1}
                      {' · '}<code>{entry.finalCommitSha.slice(0, 12)}</code></p>)}
                  <h4>First-parent path history</h4>
                  {gitFileHistory.commits.map(entry =>
                    <p key={entry.commitSha}><code>{entry.commitSha.slice(0, 12)}</code>
                      {' · '}{entry.summary}</p>)}
                  {(gitFileHistory.blameTruncated || gitFileHistory.historyTruncated) &&
                    <p role="status">File history or blame has been truncated.</p>}
                </div>}
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
                <h3>GitHub pull requests · read-only source review</h3>
                <button type="button" className="ghostBtn"
                  disabled={!activeProject || githubPullsBusy || gitBusy}
                  onClick={() => void inspectGithubPulls()}>
                  {githubPullsBusy ? 'Checking public pull requests…' :
                    'Inspect open PRs for this exact local branch'}
                </button>
              </div>
              <p>Human-triggered, public GitHub metadata only. No credentials,
                remote publication, review submissions or trusted execution rights.
                Pull request metadata may become stale after this check.</p>
              {githubPulls?.projectId === activeProject?.projectId &&
                <div className="continuityDocument"
                  aria-label="G06E01 exact origin read-only pull-request inspection">
                  <h4>PR inspection: {githubPulls.state.replaceAll('_', ' ')}</h4>
                  <p>Repository: {githubPulls.repository || 'unavailable'}
                    {' · '}Branch: {githubPulls.currentBranch || 'unavailable'}</p>
                  <p>Local HEAD: <code>{githubPulls.localHead || 'unavailable'}</code></p>
                  {githubPulls.errorCode && <p role="status">
                    Evidence unavailable: {githubPulls.errorCode}</p>}
                  {githubPulls.listingPartial && <p role="alert">
                    Partial result page: not a complete PR inventory</p>}
                  {githubPulls.pulls.map(pr =>
                    <article key={pr.number} className="continuityEvent">
                      <strong>{pr.title}</strong>
                      <p>PR #{pr.number}
                        {' · '}{pr.draft ? 'Draft' : 'Open'}
                        {' · '}Base: {pr.baseBranch}</p>
                      <p>Remote PR HEAD: <code>{pr.headSha}</code>
                        {' · '}Local comparison: {pr.localVsPrHead.replaceAll('_', ' ')}</p>
                      <p>Approval and CI: not independently verified.
                        Review is not permission to execute, publish or merge.</p>
                      <p><a href={pr.url} target="_blank"
                        rel="noreferrer">Open this pull request on GitHub</a></p>
                    </article>)}
                </div>}
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
                  <div className="taskLinkedSources" aria-label={'Linked source files for '+task.taskId}>
                    <strong>Linked saved-source observations · {task.sourceLinks?.length??0}</strong>
                    <p>Recorded native source revisions are historical. Open editor
                      identity is not proof of unchanged disk; recheck explicitly.
                      An edited task leaves prior links attached as history.</p>
                    {(task.sourceLinks??[]).map(link=>{
                      const tab=tabs.find(t=>t.projectId===activeProject?.projectId &&
                        t.relativePath===link.relativePath);
                      const state=tab?editorStates.current.get(tab.key):null;
                      const display=sourceLinkDisplay(link,task,tab,activeProject?.projectId);
                      const key=task.taskId+':'+link.relativePath;
                      const checked=taskSourceChecks[key];
                      return <div className="taskLinkedSourceRow" key={link.relativePath}>
                        <code>{link.relativePath}</code>
                        <small>Recorded SHA {link.savedRevision.slice(0,12)}…</small>
                        <span>{display.replaceAll('_',' ')}</span>
                        {tab && state?.sliceDoc()!==tab.savedText &&
                          <span>Unsaved editor changes not linked</span>}
                        <button type="button" className="ghostBtn"
                          aria-label={'Recheck linked source '+link.relativePath}
                          disabled={contextBusy || editorTaskBusy ||
                            checked?.state==='checking'}
                          onClick={()=>void recheckLinkedTaskSource(task,link)}>
                          {checked?.state==='checking'?'Checking…':'Recheck disk'}
                        </button>
                        {checked && checked.state!=='checking' &&
                          <small role="status">Last explicit recheck:
                            {' '}{checked.state.replaceAll('_',' ')}
                          </small>}
                      </div>;
                    })}
                  </div>
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
              <strong>{lspSession
                ? 'A protected language session is '+(lspBusy?'changing state':'active')+
                    ' for '+Object.keys(lspSession.members||{}).length+' project file(s).'
                : lspWorker?.ready
                  ? 'Reviewed local server assets are available; none started automatically.'
                  : 'Language servers are not running.'}</strong>
              {' '}{lspWorker?.detail || 'Worker state unavailable.'}
              {' '}Server discovery does not grant execution or install packages.
            </p>
            {lspSession && <div className="capabilityNotice">
              <p role={lspSession.cleanupRequired?'alert':'status'}>
                Project: {lspSession.projectId}
                {' · '}Files: {Object.keys(lspSession.members||{}).length}
                {' · '}Provider: {lspSession.providerId}
                {lspSession.cleanupRequired &&
                  ' · Native worker cleanup not independently confirmed'}
              </p>
              <button type="button" className="ghostBtn"
                disabled={lspBusy}
                onClick={()=>void stopLspSession()}>
                {lspBusy?'Stopping worker…':lspSession.cleanupRequired
                  ? 'Retry protected worker cleanup'
                  : 'Stop protected language session'}
              </button>
            </div>}
            <p className="capabilityNotice">
              Native verified semantic languages:
              {' '}{lspWorker?.semanticLanguages?.length
                ? lspWorker.semanticLanguages.join(', ')
                : 'none presently observed'}.
              {' '}Python semantic features require the separately pinned
              offline Pyright bundle. Ruff alone provides diagnostics only.
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
                  <dt>Availability</dt><dd>{
                    lspWorker?.ready && Array.isArray(lspWorker.supportedLanguages) &&
                    server.languageIds?.some(id=>lspWorker.supportedLanguages.includes(id)) &&
                    (server.id!=='pyright' ||
                      lspWorker.semanticLanguages?.includes('python')) &&
                    (server.id!=='ruff' ||
                      !lspWorker.semanticLanguages?.includes('python'))
                      ? 'Verified as available by native host; explicit OS approval required'
                      : 'Catalogue entry only · Installed executable not verified'
                  }</dd>
                  <dt>Source</dt><dd><a href={server.upstream} target="_blank"
                    rel="noreferrer">Publisher documentation</a></dd>
                </dl>
              </article>)}
            </div>
            <p className="capabilityNotice">
              Installer candidates: native rust-analyzer, clangd and Ruff;
              Pyright must be supplied as an independently reviewed, fully offline
              executable bundle and exact installer pin; user Node/Python runtimes
              and package downloads are not accepted. No binary is downloaded by the current alpha.
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
                      aria-label={'Edit project ' + project.displayName}
                      disabled={explorerPending || pending || workspaceLayoutBusy}
                      onClick={() => openDirectory(project, '')}>Edit</button>}
                    {project.active
                      ? <button className="ghostBtn" disabled={pending || !!savingKey ||
                          workspaceLayoutBusy}
                          onClick={() => operation('ide_close_project',
                            {projectId: project.projectId})}>Close</button>
                      : <button className="ghostBtn" disabled={pending || !project.available}
                          aria-label={'Reopen project ' + project.displayName}
                          onClick={() => operation('ide_open_project',
                            {projectId: project.projectId})}>Reopen</button>}
                    <button className="removeProjectBtn"
                      disabled={pending || explorerPending || !!savingKey ||
                        workspaceLayoutBusy}
                      aria-label={'Remove ' + project.displayName + ' from IDE registration'}
                      onClick={() => confirmUnregister(project)}>Remove from IDE</button>
                  </div>
                </article>)}
              </div>}
          <div className="infoNotice"><strong>Human-owned editing only.</strong>
            Saving is explicit and revision-checked. This alpha does not execute scripts,
            install packages, or run a language server.</div>
        </section>
        : <section className="sourceWorkspace" aria-label="Source explorer and editor"
          aria-busy={explorerPending || workspaceLayoutBusy || draftRecoveryBusy}>
          <div className="sourceTree">
            <div className="sourceTreeHeader">
              <strong>Files</strong><span>Project</span>
            </div>
            <form className="sourceGoToFile" onSubmit={event=>{
              event.preventDefault(); void openRelativeSourcePath();
            }}>
              <input aria-label="Open project-relative file" spellCheck={false}
                maxLength={1024} value={relativeOpenPath}
                placeholder="Open src/path/file.rs"
                onChange={event=>setRelativeOpenPath(event.target.value)}/>
              <button type="submit" disabled={!activeProject || !relativeOpenPath.trim() ||
                explorerPending || !!savingKey}>Open file</button>
            </form>
            <form className="sourceGoToFile" onSubmit={event=>{
              event.preventDefault(); void searchQuickOpen();
            }}>
              <input ref={quickOpenInputRef}
                aria-label="Quick Open filename search" spellCheck={false}
                maxLength={128} value={quickOpenQuery}
                onKeyDown={event=>{
                  if(event.key==='Escape'){
                    quickOpenSerial.current+=1;
                    quickOpenBusyRef.current=false;
                    setQuickOpenBusy(false);
                    setQuickOpenResults(null);
                    quickOpenInputRef.current?.blur();
                  }
                }}
                placeholder="Quick Open: filename or path"
                onChange={event=>{
                  quickOpenSerial.current+=1;
                  quickOpenBusyRef.current=false;
                  setQuickOpenBusy(false);
                  setQuickOpenQuery(event.target.value);
                  setQuickOpenResults(null);
                }}/>
              <button type="submit"
                disabled={!activeProject || !quickOpenQuery.trim() ||
                  quickOpenBusy || explorerPending || !!savingKey ||
                  !!workspaceTransitionRef.current}>
                {quickOpenBusy ? 'Finding files…' : 'Find files'}
              </button>
            </form>
            {quickOpenResults?.projectId===activeProject?.projectId &&
              <section className="sourceQuickOpenResults"
                aria-label="Quick Open filename results">
                <p role="status">{quickOpenResults.matches.length} filename match(es)
                  {' · '}{quickOpenResults.entriesExamined} entries examined
                  {' · '}{quickOpenResults.directoriesExamined} directories.
                  {quickOpenResults.truncated &&
                    ' Partial scan/results; narrow the filename for better coverage.'}
                </p>
                {quickOpenResults.matches.map(hit=>
                  <button type="button" className="fileRow"
                    key={hit.relativePath} title={hit.relativePath}
                    disabled={explorerPending || !!workspaceTransitionRef.current ||
                      hit.size>512*1024}
                    onClick={()=>void openQuickOpenMatch(hit)}>
                    <span aria-hidden="true" className="fileGlyph">⌕</span>
                    <span className="fileName">{hit.relativePath}</span>
                    {hit.size>512*1024 ? ' · exceeds current editor admission limit' : ''}
                  </button>)}
                {projectTabCount>=MAX_OPEN_TABS_PER_PROJECT &&
                  <p className="treeCaution">Open-tab budget reached.
                    Close verified clean background tabs below or close a file
                    before opening another. No unsaved edit is evicted automatically.</p>}
              </section>}
            <div className="sourcePath" title={directory || '/'}>
              <button type="button" className="pathLink" disabled={explorerPending}
                onClick={() => activeProject && openDirectory(activeProject, '')}>
                {activeProject?.displayName}
              </button>
              {directory && <span> / {directory}</span>}
            </div>
            {directory && <button type="button" className="fileRow backRow"
              disabled={explorerPending} onClick={() => openDirectory(activeProject, parentPath)}>↰ ..</button>}
            {visibleTree.visible.map(entry =>
              <button type="button" key={entry.relativePath}
                className={'fileRow' + (activeTab?.relativePath === entry.relativePath ? ' active' : '')}
                disabled={explorerPending || (entry.kind !== 'directory' && entry.kind !== 'file')}
                title={entry.relativePath}
                onClick={() => selectEntry(entry)}>
                <span aria-hidden="true" className="fileGlyph">{entry.kind === 'directory' ? '▸' :
                  entry.kind === 'file' ? '▤' : '⤷'}</span>
                <span className="fileName">{entry.name}</span>
              </button>)}
            {visibleTree.remaining>0 && <button type="button"
              className="fileRow treeLoadMore" disabled={explorerPending}
              onClick={()=>setVisibleTreeCount(previous=>previous+TREE_ENTRY_PAGE)}>
              Show next {Math.min(TREE_ENTRY_PAGE,visibleTree.remaining)} entries
              · {visibleTree.total} loaded from native directory
            </button>}
            {listing?.nextAfter && <button type="button"
              className="fileRow treeLoadMore"
              disabled={explorerPending || directoryPageBusy ||
                !!workspaceTransitionRef.current}
              onClick={()=>void loadNextSourceDirectoryPage()}>
              {directoryPageBusy ? 'Reading another page…' :
                'Load next 120 names from this directory'}
            </button>}
            {listing?.truncated && <p className="treeCaution">
              {listing.scanLimited
                ? 'This directory exceeds the bounded native scan. Its list is partial; use Quick Open or a project-relative path.'
                : 'More directory entries are available. Load another native page to continue.'}
              {' '}Folder entries may change between pages; refresh when needed.
            </p>}
            {!explorerPending && listing && listing.entries.length === 0 &&
              <p className="treeCaution">Empty directory.</p>}
          </div>
          <div className="sourcePreview" role="region" aria-label="Source code editor">
            <p className="sourceWorkspaceStatus" role="status" aria-live="polite">
              {explorerPending ? 'Opening project files…' :
                workspaceLayoutBusy ? 'Restoring saved workspace navigation…' :
                draftRecoveryBusy ? 'Reviewing private draft recovery…' :
                activeProject
                  ? 'Project: '+activeProject.displayName+
                    ' · '+projectTabs.length+' open files · '+
                    (splitEditorOpen ? 'two editor panes' : 'one editor pane')
                  : 'Select a registered project to open source files'}
            </p>
            <div className="editorLayoutToolbar" role="toolbar"
              aria-label="Source editor navigation and workspace actions">
              <button type="button" className="ghostBtn"
                aria-label="Focus primary editor (Ctrl+1)"
                disabled={!activeTab || explorerPending}
                onClick={()=>requestEditorFocus('primary')}>
                Focus first editor · Ctrl+1
              </button>
              <button type="button" className="ghostBtn"
                aria-label="Focus secondary editor (Ctrl+2)"
                disabled={!splitEditorOpen || !secondaryTab || explorerPending}
                onClick={()=>requestEditorFocus('secondary')}>
                Focus second editor · Ctrl+2
              </button>
              <button type="button" className="ghostBtn"
                aria-controls="projectProblemsPanel"
                aria-expanded={projectProblemsOpen}
                disabled={!activeProject}
                onClick={()=>showProjectProblems()}>
                Problems · {problemsForProject.errors+problemsForProject.warnings+
                  problemsForProject.infos}
              </button>
              <button type="button" className="ghostBtn"
                disabled={!activeProject}
                onClick={()=>{setSection('tasks');void refreshIntents();}}>
                Jobs · {currentJobEvents.length
                  ? currentJobEvents.length+' updates' : 'view progress'}
              </button>
              <button type="button" className="ghostBtn"
                aria-label="Check open tabs for external disk changes"
                disabled={!activeProject || !projectTabs.length || diskAuditBusy ||
                  explorerPending || !!savingKey || workspaceLayoutBusy}
                onClick={()=>void inspectOpenTabDiskRevisions()}>
                {diskAuditBusy ? 'Checking disk…' : 'Check disk changes'}
              </button>
              <button type="button" className="ghostBtn"
                aria-label="Remember open tabs for restart"
                disabled={!activeProject || !projectTabs.length || workspaceLayoutBusy ||
                  explorerPending || !!savingKey}
                onClick={()=>void rememberWorkspaceForRestart()}>
                {workspaceLayoutBusy ? 'Working…' : 'Remember tabs'}
              </button>
              <button type="button" className="ghostBtn"
                aria-label="Restore remembered saved tabs"
                disabled={!rememberedLayout || workspaceLayoutBusy || explorerPending ||
                  !!savingKey}
                onClick={()=>void resumeRememberedWorkspace()}>
                Restore remembered
              </button>
              <button type="button" className="ghostBtn"
                aria-label="Forget remembered tab layout"
                disabled={!rememberedLayout || workspaceLayoutBusy || explorerPending}
                onClick={()=>void forgetRememberedWorkspace()}>
                Forget layout
              </button>
              <button type="button" className="ghostBtn" aria-label="Toggle split editor"
                aria-pressed={splitEditorOpen} disabled={!activeTab}
                onClick={toggleSplitEditor}>
                {splitEditorOpen ? 'Close split' : 'Split editor'}
              </button>
              <span role="status" className="editorWorkingSetNotice">
                {workingSetNotice}
              </span>
              <button type="button" className="ghostBtn"
                aria-label="Close clean background tabs"
                disabled={!activeProject || projectTabs.length<2 || !!savingKey ||
                  explorerPending || !!explorerIoTicket.current ||
                  draftRecoveryBusy || reviewPending || inboxBusy}
                onClick={closeCleanBackgroundTabs}>
                Close clean tabs
              </button>
              {splitEditorOpen && <>
                <span className="editorFocusLabel" role="status">
                  Focused: {focusedEditorPane==='secondary' && secondaryTab
                    ? 'second editor' : 'first editor'}
                </span>
                <label htmlFor="secondary-editor-select">Second editor</label>
                <select id="secondary-editor-select" aria-label="Second editor file"
                  value={secondaryTab?.key || ''}
                  onChange={event=>setSecondaryTabKey(event.target.value || null)}>
                  <option value="">Choose an open file</option>
                  {projectTabs.filter(tab=>tab.key!==activeTabKey).map(tab=>
                    <option key={tab.key} value={tab.key}>{tab.relativePath}</option>)}
                </select>
                <span>Two distinct buffers · saves remain explicit</span>
              </>}
            </div>
            <p className="treeCaution" aria-live="polite">
              {rememberedLayout
                ? 'Remembered '+rememberedLayout.files.length+
                  ' relative file paths privately. Restore explicitly rechecks saved disk files.'
                : 'No remembered tab layout. Remember tabs explicitly for restart.'}
              {' '}Never stores unsaved code or Undo. Use Preserve draft for private recovery.
            </p>
            {belongsToSelectedProject(activeProject,workspaceRecoveryReport) &&
              <section className="diskAuditPanel"
                aria-label="Restarted workspace and private draft recovery">
                <div className="diskAuditHeading">
                  <strong>Workspace restart reconciliation</strong>
                  <button type="button" className="ghostBtn"
                    aria-label="Dismiss workspace recovery report"
                    onClick={()=>setWorkspaceRecoveryReport(null)}>
                    Dismiss report
                  </button>
                </div>
                <p role="status">
                  {workspaceRecoveryReport.restoredCount} saved file(s) reopened.
                  {' '}{workspaceRecoveryReport.summary.changedOnDisk} changed on disk;
                  {' '}{workspaceRecoveryReport.summary.missing} unavailable;
                  {' '}{workspaceRecoveryReport.summary.existingEditorsPreserved} existing
                  editor(s) preserved;
                  {' '}{workspaceRecoveryReport.summary.checkpoints} private
                  checkpoint(s) found.
                </p>
                <p>Only registered, currently readable source files were reopened.
                  No project file was written, no older editor draft was applied,
                  and no language server, build or debugger was restarted.
                  The following is a read-only reconciliation from this session.</p>
                {workspaceRecoveryReport.action==='inspection_only' &&
                  <p role="alert">Disk changes were not accepted. No tabs were
                    reopened. Choose Restore remembered again if you want to
                    review the current saved files.</p>}
                <div className="diskAuditRows">
                  {workspaceRecoveryReport.rows.map(row=>{
                    const opened=projectTabs.find(tab=>tab.key===row.key);
                    const availableDraft=draftRecoveryByTab[row.key];
                    const canRecover=!!opened && !!row.checkpointId &&
                      availableDraft?.checkpointId===row.checkpointId &&
                      row.draftStatus!=='recovered_as_unsaved_editor';
                    return <div key={row.key} className="diskAuditRow">
                      <code>{row.relativePath}</code>
                      <strong>{row.status.replaceAll('_',' ')}</strong>
                      {row.diskChanged && <span role="status">
                        Saved disk revision changed since the remembered layout.
                      </span>}
                      {row.wasDirtyBeforeRestart && <span>
                        File had unsaved changes before restart.
                      </span>}
                      <span>Private draft:
                        {' '}{row.draftStatus.replaceAll('_',' ')}
                      </span>
                      {row.wasDirtyBeforeRestart && !row.checkpointId &&
                        <span role="status">No recoverable private checkpoint
                          confirmed. The remembered layout never stored unsaved text.
                        </span>}
                      {opened && <button type="button" className="ghostBtn"
                        disabled={workspaceLayoutBusy || draftRecoveryBusy}
                        onClick={()=>selectPrimaryEditorTab(opened.key)}>
                        Show editor
                      </button>}
                      {canRecover && <button type="button" className="ghostBtn"
                        disabled={workspaceLayoutBusy || draftRecoveryBusy ||
                          !!savingKey || !!workspaceTransitionRef.current}
                        onClick={()=>void restorePreservedDraft(opened,availableDraft)}>
                        Recover checkpoint to unsaved editor
                      </button>}
                      {!opened && row.checkpointId && <span role="alert">
                        Original source cannot be reopened. No automatic
                        reconstruction from its private draft is permitted.
                      </span>}
                    </div>;
                  })}
                </div>
              </section>}
            {belongsToSelectedProject(activeProject, diskAuditReport) &&
              <section className="diskAuditPanel" aria-label="External file changes">
                <div className="diskAuditHeading">
                  <strong>External file revision check</strong>
                  <button type="button" className="ghostBtn"
                    aria-label="Dismiss external disk report"
                    onClick={()=>setDiskAuditReport(null)}>Dismiss</button>
                </div>
                <p role="status" aria-live="polite">
                  {diskAuditCounts.changed} changed on disk ·
                  {' '}{diskAuditCounts.current} unchanged ·
                  {' '}{diskAuditCounts.unavailable} unavailable ·
                  {' '}{diskAuditCounts.stale} stale or unverified ·
                  {' '}{diskAuditCounts.unsaved} with unsaved edits.
                </p>
                <p>Read-only inspection of {diskAuditReport.rows.length} open files.
                  Disk files may change after inspection; compare again before acting.
                  No automatic reload, merge, or Save.</p>
                <div className="diskAuditRows">
                  {diskAuditReport.rows.map(row=>
                    <div className="diskAuditRow" key={row.key}>
                      <code>{row.relativePath}</code>
                      <strong>{row.status.replaceAll('_',' ')}</strong>
                      {row.liveUnsaved && <span> · Unsaved editor retained</span>}
                      {row.status==='changed' && row.liveUnsaved &&
                        <span role="alert">Disk and unsaved editor both differ.
                          Compare before reloading or saving.</span>}
                      {row.status==='unavailable' &&
                        <span role="status">Source could not be read; no
                          automatic restore or replacement.</span>}
                      {row.status==='stale' &&
                        <span role="status">Editor identity changed during
                          inspection; refresh before deciding.</span>}
                      {row.status==='changed' &&
                        <button className="ghostBtn" type="button"
                          disabled={diskAuditBusy || explorerPending}
                          onClick={()=>{
                            const live=tabsRef.current.find(tab=>tab.key===row.key);
                            if (live) void compareEditorWithDisk(live);
                          }}>Compare disk</button>}
                    </div>
                  )}
                </div>
              </section>}
            <div className="editorTabs" role="tablist" aria-label="Open source files"
              aria-orientation="horizontal" onKeyDown={selectKeyboardTab}>
              <button type="button" className="editorTabPage"
                aria-label="Earlier open source tabs" disabled={!visibleEditorTabs.hiddenBefore}
                onClick={()=>setTabWindowStart(previous=>
                  pageTabWindow(projectTabs,previous,-1))}>‹</button>
              <span className="editorTabCount" aria-live="polite">
                {visibleEditorTabs.count===0 ? 'No files' :
                  (visibleEditorTabs.start+1)+'–'+visibleEditorTabs.end+' of '+
                   visibleEditorTabs.count} · max {MAX_OPEN_TABS_PER_PROJECT}
              </span>
              <select className="editorTabJump" aria-label="Jump to open file"
                value={belongsToSelectedProject(activeProject, activeTab) ?
                  activeTab.key : ''}
                onChange={event=>selectPrimaryEditorTab(event.target.value)}>
                <option value="">Open tabs</option>
                {projectTabs.map(tab=><option key={tab.key} value={tab.key}>
                  {tab.dirty?'● ':''}{tab.relativePath}</option>)}
              </select>
              {visibleEditorTabs.visible.map(tab => <div className={'editorTab' +
                (tab.key === activeTabKey ? ' selected' : '')} key={tab.key}>
                <button type="button" role="tab" aria-selected={tab.key === activeTabKey}
                  data-ide-tab-key={tab.key}
                  tabIndex={tab.key===activeTabKey?0:-1}
                  aria-label={(tab.dirty ? 'Unsaved source tab ' : 'Saved source tab ') +
                    tab.relativePath}
                  className="editorTabSelect" title={tab.relativePath}
                  onClick={() => selectPrimaryEditorTab(tab.key)}>
                  <span>{leafFor(tab.relativePath)}</span>
                  {tab.dirty && <span className="unsavedDot" title="Unsaved changes">●</span>}
                </button>
                <button type="button" className="editorTabClose"
                  disabled={savingKey === tab.key} title={'Close ' + tab.relativePath}
                  aria-label={'Close ' + tab.relativePath} onClick={() => closeTab(tab.key)}>×</button>
              </div>)}
              <button type="button" className="editorTabPage"
                aria-label="Later open source tabs" disabled={!visibleEditorTabs.hiddenAfter}
                onClick={()=>setTabWindowStart(previous=>
                  pageTabWindow(projectTabs,previous,1))}>›</button>
            </div>
            {activeTab && activeTab.projectId === activeProject?.projectId ? <>
              <section className="taskSourceLinkBar" aria-label="Associate source with development task">
                <strong>C7 · Task evidence</strong>
                <button type="button" className="ghostBtn"
                  disabled={editorTaskBusy || taskBusy || !!savingKey || explorerPending}
                  onClick={()=>void loadEditorTaskCatalog()}>
                  {editorTaskBusy ? 'Working…' : 'Load C6 tasks'}
                </button>
                {belongsToSelectedProject(activeProject, editorTaskCatalog) &&
                  editorTaskCatalog.tasks.length>0 && <>
                    <label htmlFor="c7EditorTaskChoice">Task</label>
                    <select id="c7EditorTaskChoice" value={editorTaskId}
                      disabled={editorTaskBusy}
                      onChange={event=>setEditorTaskId(event.target.value)}>
                      {editorTaskCatalog.tasks.map(task=><option key={task.taskId}
                        value={task.taskId}>{task.taskId} · {task.title}</option>)}
                    </select>
                    <button type="button" className="ghostBtn"
                      aria-label="Link saved source revision to selected task"
                      disabled={editorTaskBusy || taskBusy || !!savingKey ||
                        explorerPending || !editorTaskId}
                      onClick={()=>void linkActiveSourceToTask(activeTab)}>
                      Link saved file to task
                    </button>
                  </>}
                {belongsToSelectedProject(activeProject, editorTaskCatalog) &&
                  !editorTaskCatalog.tasks.length &&
                  <span>Create a development task in Continuity first.</span>}
                <small>Host-verified saved SHA only. Draft bytes, test results,
                  and task acceptance are never inferred from an editor tab.</small>
              </section>
              <div className="sourcePreviewHeader">
                <strong>{activeTab.relativePath}</strong>
                <span className="editorToolbar">
                  <span role="status"
                    aria-label={(activeTab.dirty
                      ? 'Unsaved editor draft ' : 'Saved editor source ') +
                      activeTab.relativePath}>{activeTab.draft.length.toLocaleString()} chars ·
                    {activeTab.dirty ? ' Unsaved' : ' Saved'}
                    {isLargeSourceEditor(activeTab.draft) ? ' · Lightweight editor' : ''}
                  </span>
                  <button className="ghostBtn editorToolButton"
                    disabled={referencePending || explorerPending || !!savingKey}
                    title="Copy a reference to the current saved file and cursor line"
                    onClick={() => copySourceReference(activeTab)}>
                    {referencePending ? 'Checking…' : 'Copy source reference'}
                  </button>
                  <button className="ghostBtn editorToolButton"
                    disabled={draftReferencePending || draftRecoveryBusy || explorerPending || !!savingKey}
                    title="Copy an exact volatile CodeMirror buffer range; does not save source"
                    onClick={() => void copyDraftReference(activeTab)}>
                    {draftReferencePending ? 'Checking…' : 'Copy draft reference'}
                  </button>
                  <button className="ghostBtn editorToolButton"
                    disabled={draftReferencePending || draftRecoveryBusy || explorerPending || !!savingKey}
                    title="Preserve complete draft under IDE-private app data and copy a durable immutable reference"
                    onClick={() => void preserveDraft(activeTab,true)}>
                    {draftReferencePending ? 'Preserving…' : 'Preserve draft + copy durable reference'}
                  </button>
                  {draftRecoveryByTab[activeTab.key] && <>
                    <button className="ghostBtn editorToolButton"
                      disabled={draftRecoveryBusy || !!savingKey}
                      title="Explicitly restore verified private checkpoint into unsaved editor only"
                      onClick={() => void restorePreservedDraft(activeTab)}>
                      {draftRecoveryBusy ? 'Checking recovery…' : 'Recover preserved draft'}
                    </button>
                    <button className="ghostBtn editorToolButton"
                      disabled={draftRecoveryBusy || !!savingKey}
                      title="Delete IDE-private recovery history only; no project source deletion"
                      onClick={() => void discardPreservedDraft(activeTab)}>
                      Discard private recovery
                    </button>
                  </>}
                  {lspAvailableForTab && !lspSession && <button type="button"
                    className="ghostBtn editorToolButton" disabled={lspBusy}
                    title={'Require native human consent, then start isolated '+lspProviderForTab}
                    onClick={() => void startLspSession()}>
                    {lspBusy ? 'Starting…' : 'Start '+lspProviderForTab}</button>}
                  {lspMayAddCurrentFile && <button type="button"
                    className="ghostBtn editorToolButton" disabled={lspBusy}
                    title="Analyze this open file within the approved read-only project snapshot"
                    onClick={() => void addLspDocument()}>
                    {lspBusy ? 'Adding…' : 'Analyze this file'}</button>}
                  {!!lspSession && <button type="button"
                    className="ghostBtn editorToolButton" disabled={lspBusy}
                    onClick={() => void (lspRunningForTab ?
                      closeLspDocument(activeTab.key) : stopLspSession())}>
                    {lspBusy ? 'Updating…' : lspSession.cleanupRequired
                      ? 'Retry protected worker cleanup' :
                      lspRunningForTab ? 'Close file analysis' :
                      'Stop previous language session'}</button>}
                  <button type="button" className="ghostBtn editorToolButton"
                    disabled={!lspRunningForTab || lspBusy || !lspSupportsNavigation}
                    title={lspRunningForTab ? 'Ask protected '+lspProviderForTab+' for completions' :
                      'Start the approved protected language server for this file first'}
                    onClick={() => void requestLspFeature('completion')}>Complete</button>
                  <button type="button" className="ghostBtn editorToolButton"
                    disabled={!lspRunningForTab || lspBusy || !lspSupportsNavigation}
                    onClick={() => void requestLspFeature('hover')}>Hover</button>
                  <button type="button" className="ghostBtn editorToolButton"
                    disabled={!lspRunningForTab || lspBusy || !lspSupportsNavigation}
                    onClick={() => void requestLspFeature('definition')}>Definition</button>
                  {lspCapabilities && <span className="lspLanguageCapabilityNotice"
                    title={lspCapabilities.status}
                    aria-label={'Language server capabilities: ' + lspCapabilities.status}>
                    {currentLanguageId === 'python' &&
                      (lspSession?.providerId==='ruff' ||
                        !lspWorker?.semanticLanguages?.includes('python')) ?
                      'Ruff · diagnostics only (Pyright bundle not active)' :
                      lspProviderForTab + ' · diagnostics / completion / hover / definition'}
                  </span>}
                  <button type="button" className="ghostBtn editorToolButton"
                    disabled={!lspRunningForTab || lspBusy}
                    onClick={() => void requestLspFeature('diagnostics')}>Problems</button>
                  <button type="button" className="ghostBtn editorToolButton"
                    aria-expanded={projectProblemsOpen}
                    aria-controls="projectProblemsPanel"
                    title="Read-only diagnostics from exact-source authorized open tabs"
                    onClick={() => setProjectProblemsOpen(value => !value)}>
                    All problems{problemsForProject.total ? ' · '+problemsForProject.total : ''}
                  </button>
                  <button className="ghostBtn editorToolButton" aria-label="Find in file"
                    onClick={() => { setFindOpen(true); setFindIndex(-1); }}>Find · Ctrl+F</button>
                  <button type="button" className="ghostBtn editorToolButton"
                    aria-label="Review external disk changes for merge"
                    disabled={explorerPending || !!savingKey || mergeBusy ||
                      !!mergeApplyRequest || workspaceLayoutBusy}
                    onClick={()=>void prepareExternalMerge(activeTab)}>
                    {mergeBusy ? 'Rechecking…' : 'Review external merge'}
                  </button>
                  <button type="button" className="ghostBtn editorToolButton"
                    aria-label="Compare current editor with saved disk"
                    disabled={explorerPending || !!savingKey || diskAuditBusy}
                    onClick={()=>void compareEditorWithDisk(activeTab)}>
                    Compare with disk
                  </button>
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
              {belongsToSelectedProject(activeProject, diskComparison) &&
                diskComparison.tabKey===activeTab.key &&
                diskComparison.bufferRevision===activeTab.bufferRevision &&
                diskComparison.generation===activeTab.generation &&
                diskComparison.baseRevision===activeTab.revision &&
                <section className="diskComparisonPanel" aria-label="Read-only external source comparison">
                  <div className="diskAuditHeading">
                    <strong>{diskComparison.status==='current'
                      ? 'Saved disk revision matches editor base'
                      : 'Saved disk revision changed outside Lu IDE'}</strong>
                    <button type="button" className="ghostBtn"
                      onClick={()=>setDiskComparison(null)}>Close comparison</button>
                  </div>
                  <p>Source base <code>{diskComparison.baseRevision.slice(0,12)}…</code>
                    {' · '}Current disk <code>{diskComparison.diskRevision.slice(0,12)}…</code>.
                    This is a bounded first-difference excerpt, not a complete diff or merge.
                    Neither view modifies project files or unsaved drafts.</p>
                  {[
                    ['Earlier saved base vs current disk',diskComparison.summary.savedVersusDisk],
                    ['Live editor draft vs current disk',diskComparison.summary.draftVersusDisk],
                  ].map(([label,region])=>
                    <div className="diskComparisonBlock" key={label}>
                      <strong>{label}</strong>
                      {region.identical
                        ? <p>Contents match exactly.</p>
                        : region.lineEndingOnlyDifference
                          ? <p>Logical lines match, but newline bytes differ
                              (for example LF versus CRLF). No text was modified.</p>
                          : <>
                            <p>First difference at line {region.firstDifferentLine}.
                              {' '}Earlier side: {region.leftChangedLines} changed lines;
                              disk side: {region.rightChangedLines} changed lines.</p>
                            <div className="diskCompareColumns">
                              <div><strong>Editor side · line {region.left.startLine}</strong>
                                <pre>{region.left.lines.join('\n')||'(empty region)'}</pre>
                                {(region.left.moreLines>0 || region.left.clippedLines>0) &&
                                  <small>Additional or clipped lines not shown</small>}
                              </div>
                              <div><strong>Current disk · line {region.right.startLine}</strong>
                                <pre>{region.right.lines.join('\n')||'(empty region)'}</pre>
                                {(region.right.moreLines>0 || region.right.clippedLines>0) &&
                                  <small>Additional or clipped lines not shown</small>}
                              </div>
                            </div>
                          </>}
                    </div>)}
                  <p>Existing Reload and Save commands retain their own confirmation,
                    source revision checks and draft-protection rules.</p>
                </section>}
              {belongsToSelectedProject(activeProject, mergeReview) &&
                mergeReview.tabKey===activeTab.key &&
                mergeReview.generation===activeTab.generation &&
                mergeReview.savedRevision===activeTab.revision &&
                mergeReview.bufferRevision===activeTab.bufferRevision &&
                <section className="mergeReviewPanel"
                  aria-label="Reviewed external three-way merge" aria-live="polite">
                  <div className="diskAuditHeading">
                    <strong>Three-way merge · {mergeReview.plan.conflicts.length} conflict(s)</strong>
                    <button type="button" className="ghostBtn"
                      disabled={mergeBusy} onClick={()=>{
                        setMergeReview(null);setMergeChoices({});
                      }}>Dismiss merge</button>
                  </div>
                  <p>Base <code>{mergeReview.savedRevision.slice(0,12)}…</code>
                    {' · '}Current disk <code>{mergeReview.diskRevision.slice(0,12)}…</code>.
                    Distinct line edits combine. Overlapping changes require a
                    choice. No project write, automatic conflict decision or disk Save.</p>
                  {mergeReview.plan.conflicts.map(conflict=>
                    <fieldset className="mergeConflict" key={conflict.id}>
                      <legend>{conflict.id} · original line {conflict.firstBaseLine}</legend>
                      <div className="mergeConflictChoices">
                        {[
                          ['draft','Keep my editor change',conflict.draftText],
                          ['disk','Keep current disk change',conflict.diskText],
                          ['base','Keep earlier saved base',conflict.baseText],
                        ].map(([value,label,content])=>{
                          const excerpt=conflictExcerpt(content);
                          return <label className="mergeConflictChoice" key={value}>
                            <span><input type="radio" name={'b11-'+conflict.id}
                              checked={mergeChoices[conflict.id]===value}
                              disabled={mergeBusy} onChange={()=>
                                setMergeChoices(previous=>({...previous,
                                  [conflict.id]:value}))}/>
                              {label}</span>
                            <pre>{excerpt.lines.join('')||'(empty selection)'}</pre>
                            {(excerpt.extraLines>0 || excerpt.clipped) &&
                              <small>Only the first {excerpt.lines.length}
                                {' '}bounded lines are shown</small>}
                          </label>;
                        })}
                      </div>
                    </fieldset>)}
                  <p>Choose every conflict before applying. Only a bounded,
                    user-reviewed editor transaction is permitted, and Undo
                    remains available afterward. Your pre-merge private draft
                    checkpoints are not deleted.</p>
                  <button type="button" className="primaryBtn"
                    aria-label="Apply reviewed merge to unsaved editor only"
                    disabled={mergeBusy || !!mergeApplyRequest || explorerPending ||
                      mergeReview.plan.conflicts.some(c=>!mergeChoices[c.id])}
                    onClick={()=>void applyReviewedExternalMerge()}>
                    {mergeBusy ? 'Revalidating…' :
                      'Apply to unsaved editor · no disk Save'}
                  </button>
                </section>}
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
              <div className={'splitEditorGrid' + (splitEditorOpen ? ' enabled' : ' single')}>
                <div className={'splitPrimaryPane' +
                    (focusedEditorPane==='primary'?' focused':'')}
                  data-ide-editor-pane="primary" role="region"
                  aria-label={'First editor, '+activeTab.relativePath}>
              <CodeEditor
                pane="primary"
                projectId={activeTab.projectId}
                label={'First editor: '+activeTab.relativePath}
                focusRequest={editorFocusRequest}
                onFocused={onEditorFocused}
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
                mergeApplyRequest={mergeApplyRequest?.tabKey===activeTab.key
                  ? mergeApplyRequest : null}
                onMergeApplied={(nonce,applied,reason)=>
                  finishReviewedExternalMerge(nonce,applied,reason)}
                key={activeTab.key + ':' + activeTab.generation}
                tabIdentity={activeTab.key + ':' + activeTab.generation}
                editorState={editorStates.current.get(activeTab.key)}
                onCursorLine={line => setCursorByTab(previous =>
                  previous[activeTab.key] === line ? previous :
                    { ...previous, [activeTab.key]: line })}
              />

                </div>
                {splitEditorOpen && <div
                  className={'splitSecondaryPane' +
                    (focusedEditorPane==='secondary'?' focused':'')}
                  data-ide-editor-pane="secondary" role="region"
                  aria-label={secondaryTab
                    ? 'Second editor, '+secondaryTab.relativePath
                    : 'Second editor, select a file'}>
                  {secondaryTab ? <>
                    <div className="splitSecondaryHeader">
                      <strong title={secondaryTab.relativePath}>{secondaryTab.relativePath}</strong>
                      <span>{secondaryTab.dirty ? 'Unsaved draft' : 'Saved source'}</span>
                      <button className="ghostBtn" type="button"
                        onClick={() => selectPrimaryEditorTab(secondaryTab.key)}>
                        Make primary
                      </button>
                      <button className="ghostBtn" type="button"
                        disabled={!secondaryTab.dirty || !!savingKey || explorerPending}
                        onClick={() => void saveTab(secondaryTab)}>
                        Save second file
                      </button>
                    </div>
                    <CodeEditor
                      pane="secondary"
                      projectId={secondaryTab.projectId}
                      label={'Second editor: '+secondaryTab.relativePath}
                      focusRequest={editorFocusRequest}
                      onFocused={onEditorFocused}
                      key={secondaryTab.key + ':' + secondaryTab.generation + ':secondary'}
                      tabIdentity={secondaryTab.key + ':' + secondaryTab.generation}
                      editorState={editorStates.current.get(secondaryTab.key)}
                      diagnostics={secondaryLspDiagnostics}
                      onCursorLine={line=>setCursorByTab(previous =>
                        previous[secondaryTab.key]===line ? previous :
                          {...previous,[secondaryTab.key]:line})}
                      onCursorPosition={position=>setCursorPositions(previous=>{
                        const prior=previous[secondaryTab.key];
                        if(prior?.line===position.line && prior?.character===position.character)
                          return previous;
                        return {...previous,[secondaryTab.key]:position};
                      })}
                    />
                  </> : <div className="splitSecondaryPlaceholder">
                    <p>Open a second file and select it above to compare and edit side by side.</p>
                    <p>No duplicate buffer is created. Unsaved edits remain in their original tabs.</p>
                  </div>}
                </div>}
              </div>
              {projectProblemsOpen && <section id="projectProblemsPanel"
                aria-label="Open-project verified language-server problems"
                className="lspProjectProblems" tabIndex={-1}
                ref={projectProblemsPanelRef}>
                <div className="lspProjectProblemsHeading">
                  <strong>Problems across open files</strong>
                  <button className="ghostBtn" type="button"
                    onClick={() => setProjectProblemsOpen(false)}>Close problems</button>
                </div>
                <div className="lspProjectProblemsFilters">
                  <label htmlFor="projectProblemSeverity">Severity</label>
                  <select id="projectProblemSeverity" value={projectProblemSeverity}
                    onChange={event => setProjectProblemSeverity(event.target.value)}>
                    <option value="all">All</option><option value="error">Errors</option>
                    <option value="warning">Warnings</option><option value="info">Info</option>
                  </select>
                  <label htmlFor="projectProblemSearch">Filter verified problems</label>
                  <input id="projectProblemSearch" type="search" maxLength={128}
                    value={projectProblemSearch}
                    onChange={event => setProjectProblemSearch(event.target.value)}
                    placeholder="Filename or diagnostic message"/>
                </div>
                <p className="lspProblemCounts" role="status">
                  {problemsForProject.errors} errors · {problemsForProject.warnings} warnings ·
                  {' '}{problemsForProject.infos} info ·
                  {' '}{problemsForProject.verifiedTabs} source-verified open tabs
                  {problemsForProject.staleTabs > 0 &&
                    ' · '+problemsForProject.staleTabs+' stale responses hidden'}
                </p>
                {problemsForProject.capacityLimited && <p className="lspProblemCaution">
                  Partial results: open tab or problem display limit reached.
                </p>}
                {problemsForProject.verifiedTabs === 0 && <p className="lspProblemCaution">
                  No verified language-server diagnostics yet. An empty list is not
                  proof the project has no errors; protected language services
                  must be authorized and running.
                </p>}
                {problemsForProject.verifiedTabs > 0 &&
                  problemsForProject.items.length === 0 && <p>
                    No source-verified problems match this filter.
                  </p>}
                <div className="lspProjectProblemsResults">
                  {problemsForProject.items.map((item, index) =>
                    <button type="button" key={item.tabKey+':'+item.from+':'+index}
                      className="lspProjectProblem"
                      onClick={() => navigateProjectProblem(item)}
                      aria-label={item.kind+' in '+item.relativePath+' line '+item.line}>
                      <span>{item.kind} · {item.relativePath}:{item.line}</span>
                      <span>{item.message}</span>
                    </button>)}
                </div>
              </section>}
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
              <div className="draftRecoveryReferenceBar">
                <label htmlFor="b2-persistent-draft-reference">Recover exact private draft reference</label>
                <input id="b2-persistent-draft-reference"
                  aria-label="Persistent draft reference" spellCheck={false}
                  maxLength={2048} value={persistentReferenceInput}
                  placeholder="Paste ide-draft:v1 reference for this file"
                  onChange={event=>setPersistentReferenceInput(event.target.value)}/>
                <button className="ghostBtn" disabled={!persistentReferenceInput.trim() ||
                  draftRecoveryBusy || !!savingKey}
                  onClick={()=>recoverDraftByReference(activeTab)}>Recover by reference</button>
              </div>
              <div className="editorInfoBar">
                Draft revision {bufferIdentities.current.get(activeTab.key)?.revision ?? 0}
                {' · '}{draftAutoPreserve.current.has(activeTab.key)
                  ? 'Private draft checkpointing enabled after explicit Preserve · '
                  : 'Private recovery is opt-in · '}
                {draftRecoveryByTab[activeTab.key]
                  ? 'Preserved checkpoint available · ' : ''}
                {lspRunningForTab ? 'Protected '+lspProviderForTab+' running · approved session · ' :
                  'Language intelligence requires protected worker and native consent · '}
                Project files change only on explicit Save.
                Preserved drafts contain source text in private local app data; they are not encrypted.
                External source changes cause a revision conflict.
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
