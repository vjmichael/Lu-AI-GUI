import React, { useLayoutEffect, useRef } from 'react';
import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { oneDark } from '@codemirror/theme-one-dark';
import { javascript } from '@codemirror/lang-javascript';
import { python } from '@codemirror/lang-python';
import { rust } from '@codemirror/lang-rust';
import { json } from '@codemirror/lang-json';
import { markdown } from '@codemirror/lang-markdown';

function languageExtension(filename) {
  const ext = filename.toLowerCase().split('.').pop();
  switch (ext) {
    case 'js':
    case 'jsx':
    case 'mjs':
    case 'cjs':
      return javascript({ jsx: ext === 'jsx' });
    case 'ts':
    case 'tsx':
      return javascript({ typescript: true, jsx: ext === 'tsx' });
    case 'py':
      return python();
    case 'rs':
      return rust();
    case 'json':
      return json();
    case 'md':
    case 'markdown':
      return markdown();
    default:
      return null;
  }
}

/**
 * One EditorState per open tab, owned by the parent. This retains CodeMirror
 * undo history, cursor, and selection when the user switches tabs.
 * The state is never sent across the Tauri IPC boundary automatically.
 */
export function createEditorState(text, filename, onChange) {
  const syntax = languageExtension(filename);
  return EditorState.create({
    doc: text,
    extensions: [
      basicSetup,
      oneDark,
      EditorView.lineWrapping,
      EditorView.theme({
        '&': { height: '100%', fontSize: '13px' },
        '.cm-scroller': { fontFamily: 'Consolas, "Cascadia Code", ui-monospace, monospace' },
        '.cm-content': { padding: '16px 0' },
        '.cm-gutters': { borderRight: '1px solid #33455e' },
      }, { dark: true }),
      EditorView.updateListener.of(update => {
        if (update.docChanged) {
          onChange(update.state, update.state.doc.toString());
        }
      }),
      ...(syntax ? [syntax] : []),
    ],
  });
}

export function CodeEditor({ editorState, tabIdentity }) {
  const container = useRef(null);
  useLayoutEffect(() => {
    if (!container.current) return;
    const view = new EditorView({
      state: editorState,
      parent: container.current,
    });
    return () => view.destroy();
    // tabIdentity is the mount boundary; typing does not recreate EditorView.
  }, [tabIdentity]);
  return <div ref={container} className="codeEditorHost" aria-label="Source code editor" />;
}
