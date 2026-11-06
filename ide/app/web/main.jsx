import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { CodeEditor, createEditorState } from './code_editor.jsx';
import { locateMatches, planReplacement } from './find_replace.js';
import { buildDevelopmentCheckpoint } from './development_checkpoint.js';

const sections = [
  { id: 'projects', title: 'Projects', glyph: '▣' },
  { id: 'explorer', title: 'Explorer', glyph: '▤' },
  { id: 'proposals', title: 'Proposals', glyph: '⇄' },
  { id: 'capabilities', title: 'Capabilities', glyph: '◇' },
  { id: 'continuity', title: 'Continuity', glyph: '▦' },
  { id: 'packages', title: 'Packages', glyph: '⬡', pending: true },
  { id: 'source', title: 'Source control', glyph: '⑂' },
  { id: 'tasks', title: 'Build & tests', glyph: '▶', pending: true },
];

const keyFor = (projectId, path) => projectId + ':' + path;
const leafFor = path => path.split('/').pop() || path;

function App() {
  const [section, setSection] = useState('projects');
  const [projects, setProjects] = useState([]);
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
  const [capabilityBusy, setCapabilityBusy] = useState(false);
  const [projectContext, setProjectContext] = useState(null);
  const [contextBusy, setContextBusy] = useState(false);
  const [checkpointBusy, setCheckpointBusy] = useState(false);
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
  const [snapshotBusy, setSnapshotBusy] = useState(false);
  const [resourceProbe, setResourceProbe] = useState(null);
  const [resourceProbeBusy, setResourceProbeBusy] = useState(false);
  const [inventoryComparison, setInventoryComparison] = useState(null);
  // CodeMirror EditorState is retained for each tab, including its undo stack.
  const editorStates = useRef(new Map());
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
    setProjectContext(null);
    setInventoryComparison(null);
    setGitStatus(null);
    setGithubComparison(null);
  }, [activeProject?.projectId]);

  const activeTab = tabs.find(tab => tab.key === activeTabKey) || null;
  const projectTabs = tabs.filter(tab => tab.projectId === activeProject?.projectId);
  const anyDirty = tabs.some(tab => tab.dirty);
  const isExplorer = section === 'explorer';
  const isProposals = section === 'proposals';
  const isCapabilities = section === 'capabilities';
  const isContinuity = section === 'continuity';
  const isSource = section === 'source';
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
        new Date().toISOString());
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

  function onEditorChange(key, state, draft) {
    editorStates.current.set(key, state);
    setReview(previous => previous?.tabKey === key ? null : previous);
    setTabs(previous => previous.map(tab =>
      tab.key === key ? { ...tab, draft, dirty: draft !== tab.savedText } : tab));
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
          dirty: false, generation: 0,
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
          onClick={() => { setSection(item.id); if (item.id === 'proposals') void refreshProposals(); if (item.id === 'capabilities') void refreshCapabilities(); if (item.id === 'continuity') void refreshContext(); if (item.id === 'source') void refreshGitStatus(); }}>
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
          <h1>{isSource ? 'Source control' : isContinuity ? 'Project continuity' : isCapabilities ? 'IDE capabilities' : isProposals ? 'Coding proposals' : isExplorer ? (activeProject?.displayName || 'Source editor') : 'Workspace projects'}</h1>
          <p>{isSource ? 'Read-only local Git status · No staging, commits, fetch or push' :
            isContinuity ? 'Development history · Revisioned sources · Explicit unknowns' :
            isCapabilities ? 'Host-published operations · Descriptions only · Never permission grants' :
            isProposals ? 'Data-only review inbox · No automatic changes · Lu disconnected' :
            isExplorer ? 'Tabbed code editor · Explicit, revision-checked saves · No code execution' :
            'Choose projects without executing their code.'}</p>
        </div>
        <div className="ideControls">
          {isExplorer && <button className="ghostBtn" onClick={() => setSection('projects')}>
            All projects</button>}
          <button className="ghostBtn" disabled={pending || explorerPending || !!savingKey}
            onClick={() => isSource ? refreshGitStatus() : isContinuity ? refreshContext() : isCapabilities ? refreshCapabilities() : isProposals ? refreshProposals() : isExplorer && activeProject ? openDirectory(activeProject, directory) : refresh()}>
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
      {isProposals
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
        : isSource
        ? <section className="workspaceContent continuityWorkspace" aria-label="Read-only source control">
            <div className="sectionHeading">
              <h2>Local Git worktree</h2>
              <span>{gitStatus ? gitStatus.worktree.replaceAll('_', ' ') : 'Not inspected'}</span>
            </div>
            <p className="capabilityNotice">
              Local status is read directly from the registered project.
              The IDE does not run Git, stage changes, fetch from GitHub, or modify files.
              Tracking information is locally stored and may be stale.
            </p>
            {!activeProject && <p>Open a project to inspect its source control.</p>}
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
              <div className="editorInfoBar">Edits stay in memory until Save.
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
