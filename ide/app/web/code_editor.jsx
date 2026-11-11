import React, { useEffect, useLayoutEffect, useRef } from 'react';
import { EditorView, basicSetup } from 'codemirror';
import { EditorState, StateEffect, StateField } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import { oneDark } from '@codemirror/theme-one-dark';
import { javascript } from '@codemirror/lang-javascript';
import { python } from '@codemirror/lang-python';
import { rust } from '@codemirror/lang-rust';
import { json } from '@codemirror/lang-json';
import { markdown } from '@codemirror/lang-markdown';

// Decorations are supplied only for the exact current source revision.
// The editor keeps these as display-only marks. Diagnostics never write data.
const setLspDiagnostics = StateEffect.define();
const lspDiagnosticField = StateField.define({
  create: () => Decoration.none,
  update(value, transaction) {
    value = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (effect.is(setLspDiagnostics)) {
        const spans = (Array.isArray(effect.value) ? effect.value : [])
          .filter(item => Number.isInteger(item.from) && Number.isInteger(item.to) &&
            item.from >= 0 && item.from < item.to &&
            item.to <= transaction.state.doc.length)
          .slice(0, 200)
          .map(item => Decoration.mark({
            class: item.severity === 1 ? 'cm-lsp-error' : item.severity === 2 ?
              'cm-lsp-warning' : 'cm-lsp-note'
          }).range(item.from, item.to));
        return Decoration.set(spans, true);
      }
    }
    return value;
  },
  provide: field => EditorView.decorations.from(field),
});
const EMPTY_DIAGNOSTICS = [];

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
export const LARGE_SOURCE_LIGHTWEIGHT_THRESHOLD = 128 * 1024;
export function isLargeSourceEditor(text) {
  return typeof text === 'string' &&
    text.length > LARGE_SOURCE_LIGHTWEIGHT_THRESHOLD;
}
export function createEditorState(text, filename, onChange, onSelectionChange) {
  // CodeMirror already retains an immutable line tree; for the largest
  // accepted native files, do not eagerly construct an expensive language
  // parse tree or wrap extremely long lines. Editing/Undo still work.
  const lightweight = isLargeSourceEditor(text);
  const syntax = lightweight ? null : languageExtension(filename);
  // Keep original uniform CRLF/CR source bytes when editing and checkpointing.
  const withoutCrLf = text.replace(/\r\n/g, '');
  const separator = text.includes('\r\n') && !/[\r\n]/.test(withoutCrLf)
    ? '\r\n' : text.includes('\r') && !text.includes('\n') ? '\r' : '\n';
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
      lspDiagnosticField,
      ...(!lightweight ? [EditorView.lineWrapping] : []),
      // Native WebView accessibility: expose the actual editable surface to
      // keyboard and screen-reader users and black-box installed OS tests.
      EditorView.contentAttributes.of({ 'aria-label': 'Source editor text',
        'aria-multiline': 'true', role: 'textbox' }),
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
          onSelectionChange?.(update.state);
        }
      }),
      ...(syntax ? [syntax] : []),
    ],
  });
}

export function CodeEditor({ editorState, tabIdentity, findRequest, replaceRequest,
  mergeApplyRequest, onMergeApplied,
  diagnostics = EMPTY_DIAGNOSTICS, jumpRequest, completionApplyRequest,
  onCursorLine, onCursorPosition, onReplaceApplied, onCompletionApplied,
  pane = 'primary', projectId, label, onFocused, focusRequest }) {
  const container = useRef(null);
  const viewRef = useRef(null);
  const onFocusedRef = useRef(onFocused);
  onFocusedRef.current = onFocused;
  const cursorCallback = useRef(onCursorLine);
  cursorCallback.current = onCursorLine;
  const replaceCallback = useRef(onReplaceApplied);
  replaceCallback.current = onReplaceApplied;
  const cursorPositionCallback = useRef(onCursorPosition);
  cursorPositionCallback.current = onCursorPosition;
  const completionCallback = useRef(onCompletionApplied);
  completionCallback.current = onCompletionApplied;
  const mergeCallback = useRef(onMergeApplied);
  mergeCallback.current = onMergeApplied;
  useLayoutEffect(() => {
    if (!container.current) return;
    const view = new EditorView({
      state: editorState,
      parent: container.current,
    });
    viewRef.current = view;
    if (typeof label === 'string' && label.length<=1100) {
      view.contentDOM.setAttribute('aria-label',label);
    }
    const reportFocus=()=>onFocusedRef.current?.(pane,tabIdentity);
    view.dom.addEventListener('focusin',reportFocus);
    const reportLine = () => {
      const head = view.state.selection.main.head;
      const line = view.state.doc.lineAt(head);
      cursorCallback.current?.(line.number);
      cursorPositionCallback.current?.({ line: line.number - 1,
        character: head - line.from, offset: head });
    };
    view.dom.addEventListener('keyup', reportLine);
    view.dom.addEventListener('mouseup', reportLine);
    reportLine();
    return () => {
      view.dom.removeEventListener('focusin',reportFocus);
      view.dom.removeEventListener('keyup', reportLine);
      view.dom.removeEventListener('mouseup', reportLine);
      viewRef.current = null;
      view.destroy();
    };
    // tabIdentity is the mount boundary; typing does not recreate EditorView.
  }, [tabIdentity]);
  useEffect(() => {
    const view = viewRef.current;
    // A focus command is an explicit current-project navigation decision.
    // Never steal focus from another project or a different tab identity.
    if (!view || !focusRequest ||
        focusRequest.pane!==pane ||
        focusRequest.projectId!==projectId ||
        focusRequest.tabIdentity!==tabIdentity) return;
    view.focus();
  },[focusRequest,tabIdentity,pane,projectId]);
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

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({ effects: setLspDiagnostics.of(diagnostics) });
  }, [diagnostics, tabIdentity]);
  useEffect(() => {
    const view=viewRef.current;
    if (!view || !mergeApplyRequest ||
        mergeApplyRequest.tabIdentity!==tabIdentity) return;
    const fail=()=>mergeCallback.current?.(mergeApplyRequest.nonce,false);
    const req=mergeApplyRequest;
    // The exact immutable EditorState identity is mandatory. An edit or
    // selection update since human review invalidates this request.
    if (view.state!==req.expectedEditorState ||
        view.state.sliceDoc()!==req.expectedDraft ||
        typeof req.mergedText!=='string' || req.mergedText.length>128*1024 ||
        typeof req.diskText!=='string') {
      fail();return;
    }
    // This immutable EditorState was created with an explicit newline facet.
    // A full-document transaction may normalize incoming line endings to
    // that facet. Refuse a disk/merge newline-style mismatch rather than
    // silently changing exact source bytes or pretending a safe rebase.
    const currentSeparator=view.state.facet(EditorState.lineSeparator) || '\n';
    const breaksMatch=text=>[...text.matchAll(/\r\n|\n|\r/g)]
      .every(match=>match[0]===currentSeparator);
    if (!breaksMatch(req.mergedText) || !breaksMatch(req.diskText)) {
      mergeCallback.current?.(req.nonce,false,'B11_LINE_ENDING_RELOAD_REQUIRED');
      return;
    }
    try {
      // One undoable CodeMirror transaction; DO NOT createEditorState or
      // touch project files. Rebased disk metadata is owned by the parent
      // only after a positive exact-document callback.
      if (req.mergedText!==req.expectedDraft) {
        view.dispatch({changes:{from:0,to:view.state.doc.length,
          insert:req.mergedText},scrollIntoView:false});
      }
      if (view.state.sliceDoc()!==req.mergedText) {fail();return;}
      mergeCallback.current?.(req.nonce,true);
      view.focus();
    } catch {fail();}
  },[mergeApplyRequest,tabIdentity]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view || !jumpRequest || jumpRequest.tabIdentity !== tabIdentity) return;
    if (!Number.isInteger(jumpRequest.from) || !Number.isInteger(jumpRequest.to) ||
      jumpRequest.from < 0 || jumpRequest.to < jumpRequest.from ||
      jumpRequest.to > view.state.doc.length) return;
    view.dispatch({selection: {anchor: jumpRequest.from, head: jumpRequest.to},
      scrollIntoView: true});
    view.focus();
  }, [jumpRequest, tabIdentity]);
  useEffect(() => {
    const view = viewRef.current;
    if (!view || !completionApplyRequest ||
        completionApplyRequest.tabIdentity !== tabIdentity) return;
    const fail = () => completionCallback.current?.(completionApplyRequest.nonce, false);
    // The server's insertText is a suggestion, never a command or auto-edit.
    // Apply only after a direct human choice and exact unchanged draft.
    const insert = completionApplyRequest.insertText;
    if (typeof insert !== 'string' || insert.length > 512 ||
        view.state.sliceDoc() !== completionApplyRequest.expectedDraft) {
      fail();
      return;
    }
    const caret = view.state.selection.main.head;
    const line = view.state.doc.lineAt(caret);
    const prefix = view.state.sliceDoc(line.from, caret).match(/[a-zA-Z_$][a-zA-Z0-9_$]*$/);
    const ranged = Number.isInteger(completionApplyRequest.replaceFrom) &&
      Number.isInteger(completionApplyRequest.replaceTo);
    const from = ranged ? completionApplyRequest.replaceFrom :
      caret - (prefix?.[0]?.length || 0);
    const to = ranged ? completionApplyRequest.replaceTo : caret;
    if (from < line.from || from > to || to !== caret || to > view.state.doc.length) {
      fail(); return;
    }
    view.dispatch({changes: {from, to, insert},
      selection: {anchor: from + insert.length}, scrollIntoView: true});
    completionCallback.current?.(completionApplyRequest.nonce, true);
    view.focus();
  }, [completionApplyRequest, tabIdentity]);

  return <div ref={container} className="codeEditorHost" aria-label="Source code editor" />;
}
