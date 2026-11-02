import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

function App() {
  const [models, setModels] = useState([]);
  const [selectedModel, setSelectedModel] = useState('');
  const [runtime, setRuntime] = useState({ phase: 'Idle', modelRef: null, sessionId: null });
  const [chats, setChats] = useState([]);
  const [folders, setFolders] = useState([]);
  const [chatId, setChatId] = useState(null);
  const [history, setHistory] = useState([]);
  const [prompt, setPrompt] = useState('');
  const [images, setImages] = useState([]);
  const [stream, setStream] = useState('');
  const [working, setWorking] = useState(false);
  const [generationId, setGenerationId] = useState(null);
  const [error, setError] = useState('');
  const [activity, setActivity] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [folderDraft, setFolderDraft] = useState(false);
  const [folderName, setFolderName] = useState('');
  const [expandedFolders, setExpandedFolders] = useState([]);
  const [composerHeight, setComposerHeight] = useState(130);
  const [composerCollapsed, setComposerCollapsed] = useState(false);
  const endRef = useRef(null);
  const activeGeneration = useRef(null);
  const requestedChat = useRef(null);
  const dragSize = useRef(null);
  const selected = models.find(m => m.modelRef === selectedModel);
  const loadedModel = models.find(m => m.modelRef === runtime.modelRef);
  const loaded = runtime.phase === 'Ready' || runtime.phase === 'Generating';
  const busy = working || runtime.phase === 'Generating';
  const activeChat = chats.find(c => c.chatId === chatId);

  async function refreshCatalog() {
    const [knownModels, status] = await Promise.all([invoke('lu_list_models'), invoke('lu_status')]);
    setModels(knownModels);
    setRuntime(status);
    setSelectedModel(old => old && knownModels.some(m => m.modelRef === old) ? old : status.modelRef || knownModels[0]?.modelRef || '');
  }
  async function refreshChats() {
    const list = await invoke('lu_chats');
    setChats(list.chats);
    setFolders(list.folders);
    return list.chats;
  }
  async function switchChat(id) {
    if (busy) return;
    requestedChat.current = id;
    const entries = await invoke('lu_history', {chatId: id});
    if (requestedChat.current !== id) return;
    setChatId(id);
    setHistory(entries);
    setStream('');
    setError('');
  }
  useEffect(() => {
    let alive = true;
    let stopStart = () => {}, stopToken = () => {};
    (async () => {
      try {
        const [list] = await Promise.all([refreshChats(), refreshCatalog()]);
        if (alive && list.length) await switchChat(list[0].chatId);
      } catch (error) { if (alive) setError(String(error)); }
    })();
    listen('lu-generation-start', e => {
      if (!alive) return;
      activeGeneration.current = e.payload;
      setGenerationId(e.payload);
      setStream('');
    }).then(fn => { if (alive) stopStart = fn; else fn(); });
    listen('lu-token', e => {
      if (alive && e.payload?.generationId === activeGeneration.current) {
        setStream(previous => previous + e.payload.text);
      }
    }).then(fn => { if (alive) stopToken = fn; else fn(); });
    return () => { alive = false; stopStart(); stopToken(); };
  }, []);
  useEffect(() => { endRef.current?.scrollIntoView({behavior: 'smooth'}); }, [history, stream, chatId]);

  async function action(label, call) {
    setError(''); setActivity(label); setWorking(true);
    try { return await call(); }
    catch (error) { setError(String(error)); return undefined; }
    finally { setWorking(false); setActivity(''); }
  }
  async function newChat() {
    await action('Creating chat', async () => {
      const created = await invoke('lu_new_chat');
      const list = await refreshChats();
      if (list.some(c => c.chatId === created.chatId)) await switchChat(created.chatId);
    });
  }
  async function favoriteChat(chat) {
    await action('Updating favorite', async () => {
      await invoke('lu_favorite_chat', {chatId: chat.chatId, favorite: !chat.favorite});
      await refreshChats();
    });
  }
  async function moveChat(chat, folderId) {
    await action('Moving conversation', async () => {
      await invoke('lu_move_chat', {chatId: chat.chatId, folderId});
      await refreshChats();
    });
  }
  async function addFolder(e) {
    e.preventDefault();
    if (!folderName.trim()) return;
    await action('Creating folder', async () => {
      const created = await invoke('lu_new_folder', {name: folderName.trim()});
      setFolderDraft(false); setFolderName('');
      setExpandedFolders(p => [...p, created.folderId]);
      await refreshChats();
    });
  }
  function importModel() {
    action('Importing GGUF', async () => {
      const model = await invoke('lu_import_model');
      await refreshCatalog();
      if (model) setSelectedModel(model.modelRef);
    });
  }
  function attachProjector() {
    action('Attaching MMProj', async () => {
      await invoke('lu_attach_projector', {modelRef: selectedModel});
      await refreshCatalog();
    });
  }
  function rescanModels() {
    action('Scanning models folder', async () => {
      const count = await invoke('lu_rescan_models');
      await refreshCatalog();
      setActivity(count ? `Found ${count} new model${count === 1 ? '' : 's'}` : 'No new GGUF files');
    });
  }
  function loadModel() {
    if (!selectedModel || busy || loaded) return;
    setModelMenuOpen(false);
    action('Loading GGUF and projector', async () => {
      await invoke('lu_load_model', {modelRef: selectedModel});
      await refreshCatalog();
    });
  }
  function unloadModel() {
    action('Unloading model', async () => {
      await invoke('lu_unload_model', {sessionId: runtime.sessionId});
      await refreshCatalog();
    });
  }
  function pickImage() {
    action('Choosing image', async () => {
      const image = await invoke('lu_pick_image');
      if (image) setImages(prev => prev.some(p => p.imageRef === image.imageRef) ? prev : [...prev, image]);
    });
  }
  function removeImage(imageRef) {
    setImages(current => current.filter(i => i.imageRef !== imageRef));
    invoke('lu_release_image', {imageRef}).catch(error => setError(String(error)));
  }
  function cancel() {
    if (!generationId || !runtime.sessionId) return;
    invoke('lu_cancel', {sessionId: runtime.sessionId, generationId})
      .catch(error => setError(String(error)));
    // Cancellation is intentional; partial streamed text is not a final answer.
    setStream('');
  }
  async function send(e) {
    e.preventDefault();
    if (busy || !runtime.sessionId || !prompt.trim()) return;
    const input = prompt.trim();
    const refs = images.map(i => i.imageRef);
    let activeId = chatId;
    if (!activeId) {
      const created = await invoke('lu_new_chat').catch(error => { setError(String(error)); return null; });
      if (!created) return;
      activeId = created.chatId;
      setChatId(activeId);
    }
    let completed = false;
    setStream('');
    setWorking(true); setActivity('Generating response'); setError('');
    try {
      const output = await invoke('lu_generate', {
        sessionId: runtime.sessionId, chatId: activeId,
        prompt: input, imageRefs: refs, maxNewTokens: 512
      });
      if (typeof output !== 'string') throw Error('Lu did not return a text response');
      completed = true;
    } catch (error) {
      // A user-requested cancellation is not displayed as a red error.
      if (!/Cancelled|Canceled/i.test(String(error))) setError(String(error));
    } finally {
      setWorking(false); setActivity('');
      activeGeneration.current = null;
      setGenerationId(null);
      setStream('');
      await refreshChats().catch(error => setError(String(error)));
      await switchChatWhenSafe(activeId);
      await refreshCatalog().catch(error => setError(String(error)));
    }
    if (completed) {
      setPrompt(''); setImages([]);
      for (const imageRef of refs) {
        // Response was committed before releasing host-side images. A release
        // failure is diagnostic, never a lost response or retry request.
        await invoke('lu_release_image', {imageRef}).catch(() => {});
      }
    }
  }
  async function switchChatWhenSafe(id) {
    requestedChat.current = id;
    const entries = await invoke('lu_history', {chatId:id});
    if (requestedChat.current === id) { setChatId(id); setHistory(entries); }
  }
  function beginComposerDrag(event) {
    event.preventDefault();
    dragSize.current = {y: event.clientY, height: composerHeight};
    const onMove = e => setComposerHeight(Math.max(100, Math.min(460, dragSize.current.height + dragSize.current.y - e.clientY)));
    const onUp = () => { window.removeEventListener('pointermove', onMove); window.removeEventListener('pointerup', onUp); dragSize.current = null; };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
  const selectModelText = loadedModel?.filename ?? selected?.filename ?? 'Choose a model';
  const renderChat = chat => <div key={chat.chatId} className={'chatRow ' + (chatId === chat.chatId ? 'active' : '')}>
    <button className="chatName" title={chat.title} onClick={() => switchChat(chat.chatId)} disabled={busy}>{chat.title}</button>
    <button className={'favoriteAction ' + (chat.favorite ? 'isFavorite' : '')} title={chat.favorite ? 'Remove favorite' : 'Favorite'}
      onClick={() => favoriteChat(chat)} disabled={busy} aria-label={chat.favorite ? 'Unfavorite chat' : 'Favorite chat'}>{chat.favorite ? '★' : '☆'}</button>
    <select className="chatMove" title="Move conversation to a folder" disabled={busy} value={chat.folderId || ''}
      onChange={e => moveChat(chat,e.target.value || null)} aria-label={'Move ' + chat.title}>
      <option value="">—</option>{folders.map(f => <option key={f.folderId} value={f.folderId}>{f.name}</option>)}
    </select>
  </div>;
  return <div className="shell">
    <aside className="sidebar">
      <div className="brand"><div className="brandMark">L</div><div><strong>Lu</strong><small>Independent desktop</small></div></div>
      <button className="newChatButton" onClick={newChat} disabled={busy}><span>＋</span> New Chat</button>
      <div className="chatSidebarContent">
        {chats.some(c => c.favorite) && <section className="chatSection"><h3>FAVORITES</h3>
          {chats.filter(c => c.favorite).map(renderChat)}</section>}
        <section className="chatSection"><div className="sectionHeading"><h3>FOLDERS</h3>
          <button className="quietIcon" onClick={() => setFolderDraft(p => !p)} title="New folder" aria-label="New folder">+</button></div>
          {folderDraft && <form className="folderForm" onSubmit={addFolder}>
            <input autoFocus value={folderName} maxLength={100} placeholder="Folder name" onChange={e => setFolderName(e.target.value)}/>
            <button type="submit" disabled={!folderName.trim()}>Create</button></form>}
          {folders.map(folder => <div className="folderBlock" key={folder.folderId}>
            <button className="folderToggle" onClick={() => setExpandedFolders(p => p.includes(folder.folderId) ? p.filter(v => v !== folder.folderId) : [...p, folder.folderId])}>
              <span>{expandedFolders.includes(folder.folderId) ? '▾' : '▸'}</span> ▰ {folder.name} <small>{chats.filter(c => c.folderId === folder.folderId).length}</small></button>
            {expandedFolders.includes(folder.folderId) && <div className="folderChats">{chats.filter(c => c.folderId === folder.folderId).map(renderChat)}</div>}
          </div>)}
        </section>
        <section className="chatSection"><h3>RECENT CHATS</h3>{chats.filter(c => !c.folderId).map(renderChat)}
          {!chats.length && <p className="asideHelp">Your conversations will appear here. Models are never loaded automatically.</p>}
        </section>
      </div>
      <div className="sidebarFooter">
        <div className="runtimeStatus"><span className="statusDot" data-ready={loaded}></span><strong>{loaded ? 'Model loaded' : working && activity.startsWith('Loading') ? 'Loading model…' : 'No model loaded'}</strong></div>
        <small>{loaded ? loadedModel?.filename : 'Select a model from the chat header'}</small>
        <button className="settingsButton" onClick={() => setSettingsOpen(true)}>⚙ Settings</button>
      </div>
    </aside>
    <main className="workspace">
      <header className="topbar">
        <div className="chatTitle"><h1>{activeChat?.title && activeChat.title !== 'New chat' ? activeChat.title : 'Chat'}</h1>
          <p>{loaded ? 'Ready to chat · local model' : 'Choose a local GGUF to start'}</p></div>
        <div className="headerControls">
          {working && activity.startsWith('Loading') && <span className="busyHint"><span className="spinner"></span> Loading model…</span>}
          <div className="modelSwitcherWrap">
            <button className="modelSwitcher" onClick={() => setModelMenuOpen(v => !v)} aria-expanded={modelMenuOpen} title="Quick model selection">
              <span className="modelText">{selectModelText}</span> <span>⌄</span></button>
            {modelMenuOpen && <div className="modelMenu">
              <div className="modelMenuHeading">LOCAL GGUF MODELS <small>Selection does not load</small></div>
              {models.map(m => <button className="quickModel" key={m.modelRef} onClick={() => { setSelectedModel(m.modelRef); setModelMenuOpen(false); }}>
                <span>{m.filename}<small>{m.projectorAttached ? 'GGUF + MMProj' : 'GGUF'}{!m.available ? ' · missing file' : ''}</small></span>
                {runtime.modelRef === m.modelRef && loaded ? <span className="readyMark">● Loaded</span> : selectedModel === m.modelRef ? '✓' : ''}
              </button>)}
              {!models.length && <div className="asideHelp">Put GGUF files in Lu's models directory or import them from Settings.</div>}
              <div className="modelMenuActions">
                <button className="primaryButton" disabled={busy || loaded || !selected?.available} onClick={loadModel}>Load selected</button>
                <button className="subtleButton" onClick={() => {setModelMenuOpen(false);setSettingsOpen(true);}}>Manage models</button>
              </div>
              {loaded && <p className="modelMenuNote">Unload the active model in Settings before changing models.</p>}
            </div>}
          </div>
          <button className="quietSettings" title="Settings" onClick={() => setSettingsOpen(true)}>⚙</button>
        </div>
      </header>
      <div className="messages" role="log" aria-live="polite">
        {!history.length && <div className="welcome"><div className="welcomeSymbol">L</div>
          <h2>What can I help you build?</h2>
          <p>Local conversations are saved separately. Start a new chat or choose one from your history.</p></div>}
        {history.map((entry,i) => <div key={i} className={'message ' + (entry.role === 'user' ? 'user' : 'assistant')}>
          <span className="messageRole">{entry.role === 'user' ? 'You' : 'Lu'}{entry.interrupted ? ' · interrupted' : ''}</span>
          <p>{entry.text}</p></div>)}
        {stream && <div className="message assistant streaming"><span className="messageRole">Lu · generating</span><p>{stream}</p></div>}
        <div ref={endRef}></div>
      </div>
      <div className="composerArea">
        <div className="composerResizer" onPointerDown={beginComposerDrag} role="separator" aria-label="Resize composer" aria-orientation="horizontal">
          <span className="composerHandle">···</span>
          <button type="button" className="collapseComposer" onPointerDown={e => e.stopPropagation()} onClick={() => setComposerCollapsed(v => !v)}>{composerCollapsed ? 'Expand' : 'Collapse'} composer</button>
        </div>
        {error && <div className="error" role="alert"><span>{error}</span><button onClick={() => setError('')}>Dismiss</button></div>}
        {!composerCollapsed && <form className="composer" onSubmit={send}>
          {images.length > 0 && <div className="attachments">{images.map(im => <span key={im.imageRef} className="attachment">{im.mimeType}<button type="button" onClick={() => removeImage(im.imageRef)} aria-label="Remove image">×</button></span>)}</div>}
          <textarea value={prompt} onChange={e => setPrompt(e.target.value)} style={{height:composerHeight}} maxLength={16384}
            placeholder={loaded ? 'Message Lu…' : 'Load a GGUF model first…'} disabled={!loaded || busy}
            onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); e.currentTarget.form?.requestSubmit(); } }}/>
          <div className="composerFooter"><button type="button" className="subtleButton" disabled={busy || !loaded || !loadedModel?.projectorAttached || images.length >= 4} onClick={pickImage}>+ Image</button>
            <span className="composerHelp">Enter to send · Shift+Enter for newline</span>
            {busy && generationId ? <button className="dangerButton" type="button" onClick={cancel}>Cancel</button>
              : <button className="primaryButton" type="submit" disabled={busy || !loaded || !prompt.trim()}>Send</button>}
          </div></form>}
      </div>
    </main>
    {settingsOpen && <div className="settingsBackdrop" role="presentation" onClick={() => setSettingsOpen(false)}>
      <div className="settingsPane" role="dialog" aria-label="Lu settings" aria-modal="true" onClick={e => e.stopPropagation()}>
        <header><div><h2>Settings</h2><small>Local model management</small></div><button className="quietIcon" onClick={() => setSettingsOpen(false)} aria-label="Close settings">×</button></header>
        <h3>Models</h3><p>GGUF files in Lu's models folder are discovered at startup, but <strong>never loaded automatically</strong>.</p>
        <button className="subtleButton full" onClick={rescanModels} disabled={busy || loaded}>Rescan models folder</button>
        <button className="subtleButton full" onClick={importModel} disabled={busy}>Import GGUF from another location</button>
        <div className="settingsModelList">{models.map(m => <button key={m.modelRef} className={'settingsModel '+(selectedModel === m.modelRef ? 'selected' : '')}
          onClick={() => setSelectedModel(m.modelRef)} disabled={busy}>
          <strong>{m.filename}</strong><small>{m.projectorAttached ? 'GGUF + MMProj' : 'GGUF only'} · {m.available ? 'Available' : 'Missing file'}</small></button>)}</div>
        <button className="subtleButton full" disabled={busy || loaded || !selected || selected.projectorAttached} onClick={attachProjector}>Attach MMProj to selected model</button>
        <div className="settingsActions"><button className="primaryButton" disabled={busy || loaded || !selected?.available} onClick={loadModel}>Load selected model</button>
          <button className="subtleButton" disabled={busy || !loaded} onClick={unloadModel}>Unload current model</button></div>
        <div className="settingsStatus"><span className="statusDot" data-ready={loaded}></span>{activity || (loaded ? 'Ready' : 'No model loaded')}</div>
        <p className="settingsNotice">Vulkan acceleration is requested; device offload requires verification from the engine logs. Model selection never activates weights.</p>
      </div>
    </div>}
  </div>;
}

createRoot(document.getElementById('root')).render(<App />);
