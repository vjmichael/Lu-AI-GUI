import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';

const sections = [
  { id: 'projects', title: 'Projects', glyph: '▣' },
  { id: 'explorer', title: 'Explorer', glyph: '▤' },
  { id: 'packages', title: 'Packages', glyph: '⬡', pending: true },
  { id: 'source', title: 'Source control', glyph: '⑂', pending: true },
  { id: 'tasks', title: 'Build & tests', glyph: '▶', pending: true },
];

function App() {
  const [section, setSection] = useState('projects');
  const [projects, setProjects] = useState([]);
  const [pending, setPending] = useState(false);
  const [problem, setProblem] = useState('');
  const [hint, setHint] = useState('');
  const [activeProject, setActiveProject] = useState(null);
  const [directory, setDirectory] = useState('');
  const [listing, setListing] = useState(null);
  const [preview, setPreview] = useState(null);
  const [explorerPending, setExplorerPending] = useState(false);

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
        setPreview(null);
        setSection('projects');
      }
    } catch (error) { setProblem(String(error)); }
    finally { setPending(false); }
  }
  useEffect(() => { void refresh(); }, []);

  async function operation(command, args) {
    if (pending) return;
    setPending(true);
    setProblem('');
    setHint('');
    try {
      const next = await invoke(command, args);
      setProjects(next);
      if (command === 'ide_choose_project') {
        setHint('Project selection complete. No scripts or project tools were executed.');
      }
      if (command === 'ide_close_project' && activeProject?.projectId === args?.projectId) {
        setActiveProject(null);
        setPreview(null);
        setListing(null);
        setSection('projects');
      }
    } catch (error) { setProblem(String(error)); }
    finally { setPending(false); }
  }

  async function openDirectory(project, path) {
    setExplorerPending(true);
    setProblem('');
    try {
      const result = await invoke('ide_list_source_directory', {
        projectId: project.projectId, relativeDir: path
      });
      setActiveProject(project);
      setDirectory(path);
      setListing(result);
      setPreview(null);
      setSection('explorer');
    } catch (error) { setProblem(String(error)); }
    finally { setExplorerPending(false); }
  }

  async function selectEntry(entry) {
    if (!activeProject || explorerPending) return;
    if (entry.kind === 'directory') {
      await openDirectory(activeProject, entry.relativePath);
    } else if (entry.kind === 'file') {
      setExplorerPending(true);
      setProblem('');
      try {
        const result = await invoke('ide_preview_source_file', {
          projectId: activeProject.projectId,
          relativePath: entry.relativePath,
        });
        setPreview(result);
      } catch (error) {
        setPreview(null);
        setProblem(String(error));
      } finally { setExplorerPending(false); }
    }
  }

  useEffect(() => {
    const shortcut = event => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'o') {
        event.preventDefault();
        if (!pending && !explorerPending) void operation('ide_choose_project');
      }
    };
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, [pending, explorerPending]);

  const isExplorer = section === 'explorer';
  const parentPath = directory.split('/').slice(0, -1).join('/');

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
          <h1>{isExplorer ? (activeProject?.displayName || 'Source explorer') : 'Workspace projects'}</h1>
          <p>{isExplorer ? 'Read-only project browsing · No executable workers' : 'Choose projects without executing their code.'}</p>
        </div>
        <div className="ideControls">
          {isExplorer && <button className="ghostBtn" onClick={() => setSection('projects')}>
            All projects</button>}
          <button className="ghostBtn" disabled={pending || explorerPending}
            onClick={() => isExplorer && activeProject ? openDirectory(activeProject, directory) : refresh()}>
            ↻ Refresh</button>
          <button className="primaryBtn" disabled={pending || explorerPending}
            onClick={() => operation('ide_choose_project')}>+ Open folder</button>
        </div>
      </header>
      {problem && <div className="errorMsg appError" role="alert">{problem}
        <button type="button" onClick={() => setProblem('')}>Dismiss</button></div>}
      {hint && <p className="successMsg appMessage" role="status">{hint}</p>}
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
                    <p>{project.available ? (project.active ? 'Open · read-only browsing available' : 'Closed · ready to reopen') : 'Directory unavailable'}</p>
                    <small>{project.projectId.slice(0, 12)}…</small>
                  </div>
                  <div className="projectActions">
                    {project.active && <button className="primaryBtn"
                      disabled={explorerPending || pending}
                      onClick={() => openDirectory(project, '')}>Explore</button>}
                    {project.active
                      ? <button className="ghostBtn" disabled={pending}
                          onClick={() => operation('ide_close_project', {projectId: project.projectId})}>Close</button>
                      : <button className="ghostBtn" disabled={pending || !project.available}
                          onClick={() => operation('ide_open_project', {projectId: project.projectId})}>Reopen</button>}
                  </div>
                </article>)}
              </div>}
          <div className="infoNotice"><strong>Execution restricted.</strong>
            This alpha supports scoped read-only source previews only. It has no terminal,
            debugger, package installer or executable extension loader.</div>
        </section>
        : <section className="sourceWorkspace" aria-label="Read-only source explorer">
            <div className="sourceTree">
              <div className="sourceTreeHeader">
                <strong>Files</strong><span>Read-only</span>
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
                  className={'fileRow' + (preview?.relativePath === entry.relativePath ? ' active' : '')}
                  disabled={explorerPending || (entry.kind !== 'directory' && entry.kind !== 'file')}
                  title={entry.relativePath}
                  onClick={() => selectEntry(entry)}>
                  <span aria-hidden="true" className="fileGlyph">{entry.kind === 'directory' ? '▸' : entry.kind === 'file' ? '▤' : '⤷'}</span>
                  <span className="fileName">{entry.name}</span>
                </button>)}
              {listing?.truncated && <p className="treeCaution">Large directory: showing first 400 entries.</p>}
              {!explorerPending && listing && listing.entries.length === 0 &&
                <p className="treeCaution">Empty directory.</p>}
            </div>
            <div className="sourcePreview" role="region" aria-label="Source file preview">
              {preview ? <>
                <div className="sourcePreviewHeader"><strong>{preview.relativePath}</strong>
                  <span>{preview.byteLength.toLocaleString()} bytes · Preview only</span></div>
                <pre className="sourceCode"><code>{preview.text}</code></pre>
              </> : <div className="sourcePreviewEmpty">
                <div className="emptySymbol">⌘</div>
                <h3>Select a source file</h3>
                <p>Browse folders on the left. Files open as escaped, read-only text;
                  nothing is executed, indexed by an external worker or sent to Lu.</p>
                {explorerPending && <p role="status">Loading…</p>}
              </div>}
            </div>
          </section>}
      <footer className="ideStatusBar"><span>● Local standalone application</span>
        <span>{pending || explorerPending ? 'Working…' : 'Ready'} · Ctrl+O to select a folder</span>
      </footer>
    </main>
  </div>;
}
createRoot(document.getElementById('root')).render(<App />);
