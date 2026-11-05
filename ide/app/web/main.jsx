import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { CodeEditor, createEditorState } from './code_editor.jsx';

const sections = [
  { id: 'projects', title: 'Projects', glyph: '▣' },
  { id: 'explorer', title: 'Explorer', glyph: '▤' },
  { id: 'packages', title: 'Packages', glyph: '⬡', pending: true },
  { id: 'source', title: 'Source control', glyph: '⑂', pending: true },
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
  const [activeProject, setActiveProject] = useState(null);
  const [directory, setDirectory] = useState('');
  const [listing, setListing] = useState(null);
  const [explorerPending, setExplorerPending] = useState(false);
  const [tabs, setTabs] = useState([]);
  const [activeTabKey, setActiveTabKey] = useState(null);
  const [savingKey, setSavingKey] = useState(null);
  const [review, setReview] = useState(null);
  const [reviewPending, setReviewPending] = useState(false);
  // CodeMirror EditorState is retained for each tab, including its undo stack.
  const editorStates = useRef(new Map());

  const activeTab = tabs.find(tab => tab.key === activeTabKey) || null;
  const projectTabs = tabs.filter(tab => tab.projectId === activeProject?.projectId);
  const anyDirty = tabs.some(tab => tab.dirty);
  const isExplorer = section === 'explorer';
  const parentPath = directory.split('/').slice(0, -1).join('/');

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
      } else if (key === 's') {
        event.preventDefault();
        if (activeTab) void saveTab(activeTab);
      }
    };
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, [pending, explorerPending, savingKey, activeTab, tabs]);

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
          onClick={() => setSection(item.id)}>
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
          <h1>{isExplorer ? (activeProject?.displayName || 'Source editor') : 'Workspace projects'}</h1>
          <p>{isExplorer ? 'Tabbed code editor · Explicit, revision-checked saves · No code execution' :
            'Choose projects without executing their code.'}</p>
        </div>
        <div className="ideControls">
          {isExplorer && <button className="ghostBtn" onClick={() => setSection('projects')}>
            All projects</button>}
          <button className="ghostBtn" disabled={pending || explorerPending || !!savingKey}
            onClick={() => isExplorer && activeProject ? openDirectory(activeProject, directory) : refresh()}>
            ↻ Refresh</button>
          <button className="primaryBtn" disabled={pending || explorerPending || !!savingKey}
            onClick={() => operation('ide_choose_project')}>+ Open folder</button>
        </div>
      </header>
      {problem && <div className="errorMsg appError" role="alert">{problem}
        <button type="button" onClick={() => setProblem('')}>Dismiss</button></div>}
      {hint && <p className={'successMsg appMessage' +
        (hint.startsWith('Saved ') ? ' saveConfirmation' : '')} role="status">{hint}</p>}
      {!isExplorer
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
              <CodeEditor
                key={activeTab.key + ':' + activeTab.generation}
                tabIdentity={activeTab.key + ':' + activeTab.generation}
                editorState={editorStates.current.get(activeTab.key)}
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
      <footer className="ideStatusBar"><span>● Local standalone application</span>
        <span>{savingKey ? 'Saving…' : pending || explorerPending ? 'Working…' :
          anyDirty ? '● Unsaved changes' : 'Ready'} · Ctrl+O folder · Ctrl+S save</span>
      </footer>
    </main>
  </div>;
}

createRoot(document.getElementById('root')).render(<App />);
