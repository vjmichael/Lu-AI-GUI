import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';

const initialSections = [
  { id: 'projects', title: 'Projects', glyph: '▣' },
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

  async function refresh() {
    setPending(true);
    setProblem('');
    try { setProjects(await invoke('ide_list_projects')); }
    catch (error) { setProblem(String(error)); }
    finally { setPending(false); }
  }

  useEffect(() => { void refresh(); }, []);

  async function operation(command, args) {
    setPending(true);
    setProblem('');
    setHint('');
    try {
      const next = await invoke(command, args);
      setProjects(next);
      if (command === 'ide_choose_project') {
        setHint('Project selection complete. No project scripts or tools were executed.');
      }
    } catch (error) {
      setProblem(String(error));
    } finally {
      setPending(false);
    }
  }

  useEffect(() => {
    const shortcut = event => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'o') {
        event.preventDefault();
        if (!pending) void operation('ide_choose_project');
      }
    };
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, [pending]);

  return <div className="ideShell">
    <aside className="ideRail">
      <div className="ideBrand"><div className="brandSymbol">L</div>
        <span><strong>Lu IDE</strong><small>Standalone alpha</small></span>
      </div>
      <nav aria-label="IDE sections" className="ideNav">
        {initialSections.map(item => <button key={item.id} type="button"
          className={'navItem' + (section === item.id ? ' selected' : '')}
          disabled={item.pending}
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
          <h1>Workspace projects</h1>
          <p>Choose projects without executing their code.</p></div>
        <div className="ideControls">
          <button className="ghostBtn" disabled={pending} onClick={() => void refresh()}>↻ Refresh</button>
          <button className="primaryBtn" disabled={pending} onClick={() => operation('ide_choose_project')}>
            + Open folder
          </button>
        </div>
      </header>
      <section aria-label="Projects" className="workspaceContent">
        {problem && <div className="errorMsg" role="alert">{problem}
          <button type="button" onClick={() => setProblem('')}>Dismiss</button></div>}
        {hint && <p className="successMsg" role="status">{hint}</p>}
        <div className="sectionHeading"><h2>Registered projects</h2>
          <span>{projects.length} project{projects.length === 1 ? '' : 's'}</span></div>
        {projects.length === 0
          ? <div className="emptyState"><div className="emptySymbol">⌘</div>
              <h3>No projects yet</h3>
              <p>Start by choosing a folder through the system dialog. Project scripts,
                package installations, and language servers stay disabled.</p>
              <button className="primaryBtn" disabled={pending}
                onClick={() => operation('ide_choose_project')}>Choose a folder</button>
            </div>
          : <div className="projectGrid">
              {projects.map(project => <article className="projectCard" key={project.projectId}>
                <div className="projectIcon" aria-hidden="true">▤</div>
                <div className="projectDetails">
                  <h3>{project.displayName}</h3>
                  <p>{project.available ? (project.active ? 'Open · metadata only' : 'Closed · ready to reopen') : 'Directory unavailable'}</p>
                  <small>{project.projectId.slice(0, 12)}…</small>
                </div>
                <div className="projectActions">{project.active
                  ? <button className="ghostBtn" disabled={pending} onClick={() =>
                    operation('ide_close_project', {projectId: project.projectId})}>Close</button>
                  : <button className="ghostBtn" disabled={pending || !project.available}
                    onClick={() => operation('ide_open_project', {projectId: project.projectId})}>Reopen</button>}
                </div>
              </article>)}
            </div>}
        <div className="infoNotice"><strong>Execution restricted.</strong>
          This alpha has no terminal, debugger, package installer, or executable extension loader.
          Registering a project only saves its location inside the native desktop host.</div>
      </section>
      <footer className="ideStatusBar"><span>● Local standalone application</span>
        <span>{pending ? 'Working…' : 'Ready'} · Ctrl+O to select a folder</span>
      </footer>
    </main>
  </div>;
}

createRoot(document.getElementById('root')).render(<App />);
