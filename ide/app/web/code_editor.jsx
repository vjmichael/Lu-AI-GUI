import React, { useEffect, useLayoutEffect, useRef } from 'react';
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
export function createEditorState(text, filename, onChange, onSelectionChange) {
  const syntax = languageExtension(filename);
  // Keep uniform CRLF/CR files byte-for-byte stable across opening and Save.
  // CodeMirror otherwise normalizes all line endings to LF by default.
  const withoutCrLf = text.replace(/\r\n/g, '');
  const separator = text.includes('\r\n') && !/[\r\n]/.test(withoutCrLf)
    ? '\r\n'
    : text.includes('\r') && !text.includes('\n') ? '\r' : '\n';
  // Tauri rewrites the nonce on this bundled <style> element and its CSP
  // policy for each WebView. CodeMirror's dynamic StyleModule uses that same
  // nonce rather than relaxing script-src or enabling unsafe-inline CSS.
  const nonceCarrier = document.getElementById('lu-ide-editor-style-nonce');
  const styleNonce = nonceCarrier?.nonce || '';
  return EditorState.create({
    doc: text,
    extensions: [
      basicSetup,
      EditorState.lineSeparator.of(separator),
      ...(styleNonce ? [EditorView.cspNonce.of(styleNonce)] : []),
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
          onChange(update.state, update.state.sliceDoc());
        } else if (update.selectionSet) {
          // Keep exact selection/caret for session-only source references
          // without recreating the parent editor state or changing the draft.
          onSelectionChange?.(update.state);
        }
      }),
      ...(syntax ? [syntax] : []),
    ],
  });
}

export function CodeEditor({ editorState, tabIdentity, findRequest, replaceRequest, onCursorLine, onReplaceApplied }) {
  const container = useRef(null);
  const viewRef = useRef(null);
  const cursorCallback = useRef(onCursorLine);
  cursorCallback.current = onCursorLine;
  const replaceCallback = useRef(onReplaceApplied);
  replaceCallback.current = onReplaceApplied;
  useLayoutEffect(() => {
    if (!container.current) return;
    const view = new EditorView({
      state: editorState,
      parent: container.current,
    });
    viewRef.current = view;
    const reportLine = () => {
      cursorCallback.current?.(view.state.doc.lineAt(view.state.selection.main.head).number);
    };
    view.dom.addEventListener('keyup', reportLine);
    view.dom.addEventListener('mouseup', reportLine);
    reportLine();
    return () => {
      view.dom.removeEventListener('keyup', reportLine);
      view.dom.removeEventListener('mouseup', reportLine);
      viewRef.current = null;
      view.destroy();
    };
    // tabIdentity is the mount boundary; typing does not recreate EditorView.
  }, [tabIdentity]);
  useEffect(() => {
    const view = viewRef.current;
    if (!view || !findRequest || findRequest.to > view.state.doc.length) return;
    view.dispatch({ selection: { anchor: findRequest.from, head: findRequest.to },
      scrollIntoView: true });
    view.focus();
  }, [findRequest, tabIdentity]);
  useEffect(() => {
    const view = viewRef.current;
    if (!view || !replaceRequest || replaceRequest.tabIdentity !== tabIdentity) return;
    // Reject plans computed for a different editor document, including stale
    // asynchronous/UI-state requests. Never reconstruct the EditorState here:
    // dispatching one transaction preserves CodeMirror undo/redo history.
    if (view.state.sliceDoc() !== replaceRequest.expectedDraft) {
      replaceCallback.current?.(replaceRequest.nonce, false);
      return;
    }
    const changes = replaceRequest.changes;
    if (!changes?.length || changes.some(change =>
      !Number.isInteger(change.from) || !Number.isInteger(change.to) ||
      change.from < 0 || change.to > view.state.doc.length ||
      change.from > change.to || typeof change.insert !== 'string'
    )) return;
    view.dispatch({ changes, scrollIntoView: true });
    replaceCallback.current?.(replaceRequest.nonce, true);
    view.focus();
  }, [replaceRequest, tabIdentity]);

  return <div ref={container} className="codeEditorHost" aria-label="Source code editor" />;
}
