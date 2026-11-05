import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

/**
 * IDE-01B project registration UI. This is intentionally *not* a code runner,
 * file editor, package installer or tool authorization surface.
 */
export function IdeWorkspacePanel({ open, onClose }) {
  const [projects, setProjects] = useState([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');

  useEffect(() => {
    if (!open) return;
    let current = true;
    setBusy(true);
    setError('');
    setMessage('');
    invoke('lu_ide_projects').then(list => {
      if (current) setProjects(list);
    }).catch(err => {
      if (current) setError(String(err));
    }).finally(() => {
      if (current) setBusy(false);
    });
    return () => { current = false; };
  }, [open]);

  async function operate(command, args = undefined) {
    if (busy) return;
    setBusy(true);
    setError('');
    setMessage('');
    try {
      // The only argument passed to this native API is a registered,
      // host-issued project ID. Folder selection is an OS-native dialog.
      const list = await invoke(command, args);
      setProjects(list);
      if (command === 'lu_ide_select_project') {
        setMessage('Directory selection completed. No project code was run.');
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  if (!open) return null;
  return <div className="settingsBackdrop" role="presentation" onClick={onClose}>
    <div className="settingsPane ideWorkspacePane" role="dialog" aria-modal="true"
      aria-label="IDE Workspace projects" onClick={event => event.stopPropagation()}>
      <header>
        <div>
          <h2>IDE Workspace</h2>
          <small>Project registration · Development foundation</small>
        </div>
        <button className="quietIcon" type="button" onClick={onClose}
          aria-label="Close IDE workspace">×</button>
      </header>
      <p className="ideSafetyNotice">Projects are not executed or indexed with executable
        tools when registered. Language servers, terminals, builds, debugging,
        dependency installation and AI code edits are disabled in this phase.</p>
      <button type="button" className="primaryButton ideAddProject" disabled={busy}
        onClick={() => operate('lu_ide_select_project')}>
        {busy ? 'Working…' : '+ Choose project folder'}
      </button>
      {error && <div className="error" role="alert">
        <span>{error}</span><button type="button" onClick={() => setError('')}>Dismiss</button>
      </div>}
      {message && <p className="ideStatusMessage" role="status">{message}</p>}
      <div className="ideProjectList" aria-label="Registered projects">
        {projects.map(project => <div className="ideProjectRow" key={project.projectId}>
          <div className="ideProjectIdentity">
            <strong title={project.displayName}>{project.displayName}</strong>
            <small>{project.active ? 'Registered · active' : 'Registered · closed'}
              {!project.available ? ' · directory unavailable' : ''}</small>
          </div>
          {project.active
            ? <button type="button" className="subtleButton" disabled={busy}
                onClick={() => operate('lu_ide_close_project', { projectId: project.projectId })}>
                Close
              </button>
            : <button type="button" className="subtleButton"
                disabled={busy || !project.available}
                onClick={() => operate('lu_ide_open_project', { projectId: project.projectId })}>
                Reopen
              </button>}
        </div>)}
        {!busy && projects.length === 0 &&
          <p className="asideHelp">No projects registered. Choose a folder to create
            a host-owned project reference.</p>}
      </div>
      <p className="settingsNotice">This panel is an IDE lifecycle prototype, not yet
        the full IDE editor. Project paths remain in the native host.</p>
    </div>
  </div>;
}
