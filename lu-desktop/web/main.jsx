import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { splitModelOutput } from './response_view.mjs';
import { MarkdownView } from './markdown_view.jsx';
import {buildSubmittedPrompt, attachmentSummary, MAX_TEXT_FILES} from './attachment_drafts.mjs';
import {isPotentialImage, normalizedImageFile} from './image_attachment_input.mjs';
import {queuedJob, startedJob, tokenForJob, removeJob} from './generation_jobs.mjs';
import {nearTranscriptBottom, restoredScrollTop, isSendKey, isSearchKey, isNewChatKey, matchesChatTitle} from './chat_ui_behavior.mjs';

function MessageText({text, role='assistant'}) {
  if (role === 'user') return <div className="messageContent userText">{text}</div>;
  return <div className="messageContent">{splitModelOutput(text).map((part,i) =>
    part.kind === 'think' ? <details key={i} className="thinkBlock"><summary>Model reasoning</summary><MarkdownView text={part.text}/></details>
      : <MarkdownView key={i} text={part.text}/>)}</div>;
}
const MAX_TEXT_ATTACHMENT = 64 * 1024;
const MAX_IMAGE_ATTACHMENT = 8 * 1024 * 1024;

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
  const [textFiles, setTextFiles] = useState([]);
  const [jobs, setJobs] = useState({}); // bounded, per-chat host generation state
  const [backgroundPaneOpen,setBackgroundPaneOpen] = useState(false);
  const [backgroundTasks,setBackgroundTasks] = useState([]);
  const [backgroundError,setBackgroundError] = useState('');
  const [backgroundFilter,setBackgroundFilter] = useState('active');
  const [backgroundTerminalNotice,setBackgroundTerminalNotice] = useState(null);
  const [working, setWorking] = useState(false);
  const [modelLoadSeconds, setModelLoadSeconds] = useState(0);
  const [error, setError] = useState('');
  const [activity, setActivity] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [folderDraft, setFolderDraft] = useState(false);
  const [folderName, setFolderName] = useState('');
  const [expandedFolders, setExpandedFolders] = useState([]);
  const [composerHeight, setComposerHeight] = useState(130);
  const [composerCollapsed, setComposerCollapsed] = useState(false);
  const [dragActive, setDragActive] = useState(false);
  const [chatQuery, setChatQuery] = useState('');
  const [atBottom, setAtBottom] = useState(true);
  const transcriptRef = useRef(null);
  const scrollByChat = useRef(new Map());
  const followOutput = useRef(true);
  const searchRef = useRef(null);
  const composerRef = useRef(null);
  const modelMenuRef = useRef(null);
  const requestedChat = useRef(null);
  const dragSize = useRef(null);
  const fileInput = useRef(null);
  const currentChat = useRef(null);
  const pendingDrafts = useRef(new Map());
  const draftText = useRef('');
  const draftImages = useRef([]);
  const draftFiles = useRef([]);
  const previewUrls = useRef(new Set());
  const selected = models.find(m => m.modelRef === selectedModel);
  const loadedModel = models.find(m => m.modelRef === runtime.modelRef);
  const loaded = runtime.phase === 'Ready' || runtime.phase === 'Generating';
  const busy = working || runtime.phase === 'Generating' || Object.keys(jobs).length > 0;
  const activeChat = chats.find(c => c.chatId === chatId);
  const matchingChats = chats.filter(c => matchesChatTitle(c, chatQuery));
  const visibleJob = chatId ? jobs[chatId] : null;
  const visibleStream = visibleJob?.text ?? '';
  const chatBusy = Boolean(visibleJob);
  const activeBackgroundTasks=backgroundTasks.filter(task=>
    ['PENDING','RUNNING','PAUSED'].includes(task.state));
  const shownBackgroundTasks=backgroundFilter==='active'
    ?activeBackgroundTasks:backgroundTasks;
  // An in-flight checkpoint is already visible in the live stream buffer.
  // Do not render that same unfinished assistant text twice after chat switch.
  const displayedHistory = visibleJob ? history.filter((entry,i) =>
    !(i === history.length - 1 && entry.role === 'assistant' && entry.interrupted && !entry.cancelled)) : history;
  useEffect(() => { draftText.current = prompt; draftImages.current = images; draftFiles.current = textFiles; }, [prompt, images, textFiles]);
  useEffect(() => {
    if (!(working && activity.startsWith('Loading'))) { setModelLoadSeconds(0); return; }
    const started = Date.now();
    setModelLoadSeconds(0);
    const timer = setInterval(() => setModelLoadSeconds(Math.floor((Date.now() - started) / 1000)), 1000);
    return () => clearInterval(timer);
  }, [working, activity]);
  useEffect(() => () => { for (const url of previewUrls.current) URL.revokeObjectURL(url); }, []);

  async function refreshCatalog() {
    const [knownModels, status] = await Promise.all([invoke('lu_list_models'), invoke('lu_status')]);
    setModels(knownModels);
    setRuntime(status);
    setSelectedModel(old => old && knownModels.some(m => m.modelRef === old) ? old : status.modelRef || knownModels[0]?.modelRef || '');
  }
  async function startMediaFileScan() {
    setBackgroundError('');
    try {
      // Host-owned OS file dialog and pinned Lu worker, never model inference.
      const taskId=await invoke('lu_background_scan_file',{chatId:chatId||null});
      if (taskId) {
        setBackgroundFilter('active');
        await refreshBackgroundTasks();
      }
    } catch (error) {
      setBackgroundError(String(error));
    }
  }
  async function cancelBackgroundTask(taskId) {
    try {
      await invoke('lu_background_cancel', {taskId});
      await refreshBackgroundTasks();
    } catch (error) {
      // Native dialog denial and races against completion are not success.
      await refreshBackgroundTasks();
      if (!String(error).includes('Cancellation declined')) {
        setBackgroundError(String(error));
      }
    }
  }
  async function refreshBackgroundTasks() {
    try {
      const items=await invoke('lu_background_tasks');
      setBackgroundTasks(Array.isArray(items)?items:[]);
      setBackgroundError('');
    } catch (error) {
      setBackgroundError(String(error));
    }
  }
  useEffect(()=>{
    if (!backgroundPaneOpen) return undefined;
    let alive=true;
    const refresh=()=>{if(alive) void refreshBackgroundTasks();};
    refresh();
    // UI polling queries Lu's local store, not the model or a worker.
    // Future host event pushes can replace this lightweight refresh.
    const timer=setInterval(refresh,2500);
    return ()=>{alive=false;clearInterval(timer);};
  },[backgroundPaneOpen]);

  async function refreshChats() {
    const list = await invoke('lu_chats');
    setChats(list.chats);
    setFolders(list.folders);
    return list.chats;
  }
  async function switchChat(id) {
    if (currentChat.current !== id) {
      // Capture the outgoing reading position before the asynchronous read.
      const el = transcriptRef.current;
      if (el) scrollByChat.current.set(currentChat.current || 'new', {
        top: el.scrollTop, follow: followOutput.current
      });
      pendingDrafts.current.set(currentChat.current || 'new', {
        prompt: draftText.current, images: draftImages.current, files: draftFiles.current
      });
    }
    requestedChat.current = id;
    const entries = await invoke('lu_history', {chatId: id});
    if (requestedChat.current !== id) return;
    currentChat.current = id;
    const draft = pendingDrafts.current.get(id) || {prompt:'',images:[],files:[]};
    draftText.current = draft.prompt;
    draftImages.current = draft.images;
    draftFiles.current = draft.files || [];
    setPrompt(draft.prompt);
    setImages(draft.images);
    setTextFiles(draft.files || []);
    setChatId(id);
    setHistory(entries);
    setError('');
  }
  useEffect(() => {
    let alive = true;
    let stopQueued = () => {}, stopStart = () => {}, stopToken = () => {},
        stopBackground = () => {};
    (async () => {
      try {
        const [list] = await Promise.all([refreshChats(), refreshCatalog()]);
        if (alive && list.length) await switchChat(list[0].chatId);
      } catch (error) { if (alive) setError(String(error)); }
    })();
    listen('lu-generation-queued', e => {
      if (!alive || !e.payload?.chatId) return;
      setJobs(prev => queuedJob(prev,e.payload));
    }).then(fn => { if (alive) stopQueued = fn; else fn(); });
    listen('lu-generation-start', e => {
      if (!alive || !e.payload?.chatId) return;
      setJobs(prev => startedJob(prev,e.payload));
    }).then(fn => { if (alive) stopStart = fn; else fn(); });
    listen('lu-token', e => {
      if (!alive || !e.payload?.chatId) return;
      setJobs(prev => tokenForJob(prev,e.payload));
    }).then(fn => { if (alive) stopToken = fn; else fn(); });
    listen('lu-background-terminal', e => {
      if (!alive || !e.payload?.taskId) return;
      // This event comes from the native host's persisted terminal ledger.
      // No AI generation or permission grant is performed by the WebView.
      setBackgroundTerminalNotice(e.payload);
      if (backgroundPaneOpen) void refreshBackgroundTasks();
    }).then(fn => { if (alive) stopBackground=fn; else fn(); });
    return () => { alive = false; stopQueued(); stopStart(); stopToken(); stopBackground(); };
  }, []);
  // Follow new tokens only if the operator was already reading the end.
  // Remember independent scroll positions when switching conversations.
  function onTranscriptScroll() {
    const el = transcriptRef.current;
    if (!el) return;
    const follow = nearTranscriptBottom(el);
    followOutput.current = follow;
    scrollByChat.current.set(currentChat.current || 'new', {top: el.scrollTop, follow});
    setAtBottom(follow);
  }
  function jumpToLatest() {
    const el = transcriptRef.current;
    if (!el) return;
    followOutput.current = true;
    el.scrollTop = el.scrollHeight;
    scrollByChat.current.set(currentChat.current || 'new', {top: el.scrollTop, follow:true});
    setAtBottom(true);
  }
  useLayoutEffect(() => {
    const el = transcriptRef.current;
    if (!el) return;
    const stored = scrollByChat.current.get(chatId || 'new');
    followOutput.current = stored?.follow ?? true;
    el.scrollTop = restoredScrollTop(stored, el.scrollHeight, el.clientHeight);
    setAtBottom(followOutput.current);
  }, [chatId]);
  useLayoutEffect(() => {
    const el = transcriptRef.current;
    if (!el || !followOutput.current) return;
    el.scrollTop = el.scrollHeight;
  }, [history, visibleStream, chatId]);
  useEffect(() => { if (!settingsOpen) composerRef.current?.focus(); }, [chatId]);
  useEffect(() => {
    function onShortcut(event) {
      if (event.defaultPrevented) return;
      if (isSearchKey(event) && !settingsOpen) {
        event.preventDefault(); setChatQuery(''); searchRef.current?.focus();
      } else if (isNewChatKey(event) && !settingsOpen) {
        event.preventDefault(); void newChat();
      } else if (event.key === 'Escape') {
        if (settingsOpen) setSettingsOpen(false);
        else if (modelMenuOpen) setModelMenuOpen(false);
        else if (document.activeElement === searchRef.current) {
          setChatQuery(''); composerRef.current?.focus();
        }
      }
    }
    document.addEventListener('keydown', onShortcut);
    return () => document.removeEventListener('keydown', onShortcut);
  }, [settingsOpen, modelMenuOpen]);
  useEffect(() => {
    if (!modelMenuOpen) return;
    function onOutside(event) {
      if (modelMenuRef.current && !modelMenuRef.current.contains(event.target))
        setModelMenuOpen(false);
    }
    document.addEventListener('pointerdown', onOutside);
    return () => document.removeEventListener('pointerdown', onOutside);
  }, [modelMenuOpen]);

  async function action(label, call) {
    setError(''); setActivity(label); setWorking(true);
    try { return await call(); }
    catch (error) { setError(String(error)); return undefined; }
    finally { setWorking(false); setActivity(''); }
  }
  async function newChat() {
    try {
      const created = await invoke('lu_new_chat');
      const list = await refreshChats();
      if (list.some(c => c.chatId === created.chatId)) await switchChat(created.chatId);
    } catch (error) { setError(String(error)); }
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
    const image = draftImages.current.find(i => i.imageRef === imageRef);
    if (image?.previewUrl) { URL.revokeObjectURL(image.previewUrl); previewUrls.current.delete(image.previewUrl); }
    draftImages.current = draftImages.current.filter(i => i.imageRef !== imageRef);
    setImages(draftImages.current);
    invoke('lu_release_image', {imageRef}).catch(error => setError(String(error)));
  }
  function cancel() {
    const current = chatId && jobs[chatId];
    if (!current?.generationId || !runtime.sessionId || current.cancelling) return;
    const origin = chatId;
    setJobs(prev => ({...prev,[origin]:{...prev[origin],cancelling:true}}));
    invoke('lu_cancel', {sessionId: runtime.sessionId, generationId: current.generationId})
      .catch(error => {
        setJobs(prev => prev[origin] ? ({...prev,[origin]:{...prev[origin],cancelling:false}}) : prev);
        if (currentChat.current === origin) setError(String(error));
      });
    // Cancellation only targets this conversation's generation ID.
  }
  async function send(e) {
    e.preventDefault();
    if (working || chatBusy || !runtime.sessionId || !prompt.trim()) return;
    const input = prompt.trim();
    let submitted;
    try { submitted = buildSubmittedPrompt(input, textFiles); }
    catch (error) {setError(String(error));return;}
    const submittedImages = [...images];
    const refs = submittedImages.map(i => i.imageRef);
    let activeId = chatId;
    if (!activeId) {
      const created = await invoke('lu_new_chat').catch(error => { setError(String(error)); return null; });
      if (!created) return;
      activeId = created.chatId;
      currentChat.current = activeId;
      setChatId(activeId);
    }
    // Capture the submitted turn immediately; leave the composer available
    // for drafting the next prompt while inference runs.
    draftText.current = '';
    draftImages.current = [];
    draftFiles.current = [];
    pendingDrafts.current.set(activeId, {prompt:'',images:[],files:[]});
    setPrompt(''); setImages([]); setTextFiles([]);
    if (currentChat.current === activeId) {
      setHistory(prev => [...prev, {role:'user',text:input,interrupted:false,cancelled:false}]);
    }
    setJobs(prev => ({...prev,[activeId]: {generationId:null,phase:'submitting',position:0,text:''}}));
    setError('');
    let completed = false;
    let cancelled = false;
    try {
      const output = await invoke('lu_generate', {
        sessionId: runtime.sessionId, chatId: activeId,
        prompt: submitted, imageRefs: refs, maxNewTokens: 512
      });
      if (typeof output !== 'string') throw Error('Lu did not return a text response');
      completed = true;
    } catch (error) {
      cancelled = /Cancelled|Canceled/i.test(String(error));
      if (!cancelled && currentChat.current === activeId) setError(String(error));
    } finally {
      // Keep the streaming bubble until the journal has been read back.
      await refreshChats().catch(error => setError(String(error)));
      // A background turn must never steal focus from another conversation.
      if (currentChat.current === activeId) {
        const entries = await invoke('lu_history', {chatId:activeId})
          .catch(error => {setError(String(error));return null;});
        if (entries && currentChat.current === activeId) setHistory(entries);
      }
      setJobs(prev => removeJob(prev,activeId));
      await refreshCatalog().catch(error => { if (currentChat.current === activeId) setError(String(error)); });
    }
    if (!completed && !cancelled && currentChat.current === activeId && !draftText.current.trim()) {
      // Failed submissions remain editable. Already-journaled user turns are
      // still displayed; the next attempt can be revised by the operator.
      draftText.current = input;
      setPrompt(input);
    }
    for (const image of submittedImages) {
      if (image.previewUrl) {URL.revokeObjectURL(image.previewUrl);previewUrls.current.delete(image.previewUrl);}
      await invoke('lu_release_image', {imageRef:image.imageRef}).catch(() => {});
    }
  }
  async function importAttachments(files) {
    // Capture origin before a FileReader or native image admission can await.
    // A chat switch must not redirect an in-flight attachment to another draft.
    const targetChat = currentChat.current || 'new';
    function storeAttachment(kind, value) {
      const key=targetChat;
      const old=pendingDrafts.current.get(key) || {
        prompt: key === (currentChat.current||'new') ? draftText.current : '',
        images: key === (currentChat.current||'new') ? draftImages.current : [],
        files: key === (currentChat.current||'new') ? draftFiles.current : []
      };
      const next={...old,[kind]:value};
      pendingDrafts.current.set(key,next);
      if (key === (currentChat.current||'new')) {
        if(kind === 'images') {draftImages.current=value;setImages(value);}
        else {draftFiles.current=value;setTextFiles(value);}
      }
    }
    const items = Array.from(files || []).slice(0, 8);
    for (const file of items) {
      if (isPotentialImage(file)) {
        if (file.size <= 0 || file.size > MAX_IMAGE_ATTACHMENT) {
          setError(`Image ${file.name} exceeds the 8 MB limit`); continue;
        }
        if (!loadedModel?.projectorAttached) {
          setError('Load an MMProj-compatible model before attaching images'); continue;
        }
        try {
          const normalized = await normalizedImageFile(file);
          const dataUrl = await new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result);
            reader.onerror = () => reject(Error('Could not read image'));
            reader.readAsDataURL(normalized.blob);
          });
          const image = await invoke('lu_ingest_image_data_url', {dataUrl});
          if (image) {
            const previewUrl = URL.createObjectURL(file);
            const previous=pendingDrafts.current.get(targetChat)?.images ||
              (targetChat === (currentChat.current||'new') ? draftImages.current : []);
            if (previous.some(p => p.imageRef === image.imageRef)) URL.revokeObjectURL(previewUrl);
            else {
              previewUrls.current.add(previewUrl);
              storeAttachment('images',[...previous,{...image, name:file.name, size:file.size, previewUrl, detectedMimeType: normalized.mimeType}]);
            }
          }
        } catch (error) { setError(String(error)); }
      } else if (/\.(txt|md|csv|json|log|py|rs|js|jsx|ts|tsx)$/i.test(file.name)) {
        if (file.size <= 0 || file.size > MAX_TEXT_ATTACHMENT) {
          setError(`Text attachment ${file.name} exceeds the 64 KB limit`);continue;
        }
        try {
          const text = await file.text();
          const nextFile={name:file.name,size:file.size,text};
          const origin=pendingDrafts.current.get(targetChat);
          const previous=origin?.files || (targetChat === (currentChat.current||'new') ? draftFiles.current : []);
          const candidate=[...previous,nextFile];
          if(candidate.length>MAX_TEXT_FILES) {
            setError(`You can attach up to ${MAX_TEXT_FILES} text files per message`); continue;
          }
          try { buildSubmittedPrompt(origin?.prompt || draftText.current || 'Attachment',candidate); }
          catch(error) {setError(String(error));continue;}
          storeAttachment('files',candidate);
        } catch(error) { setError(String(error)); }
      } else {
        setError(`Unsupported attachment ${file.name}. Images and plaintext documents work; PDF/DOCX are not yet supported.`);
      }
    }
  }
  function handlePaste(event) {
    const fileItems = Array.from(event.clipboardData?.items || [])
      .filter(item => item.kind === 'file').map(item => item.getAsFile()).filter(Boolean);
    if (fileItems.length) { event.preventDefault(); void importAttachments(fileItems); }
  }
  function handleDrop(event) {
    event.preventDefault(); setDragActive(false);
    if (!loaded) { setError('Load a model before attaching files'); return; }
    void importAttachments(event.dataTransfer?.files);
  }
  function handleAppDrag(event) {
    if (Array.from(event.dataTransfer?.types || []).includes('Files')) {
      event.preventDefault();
      if (loaded) setDragActive(true);
    }
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
    <button className="chatName" title={chat.title} onClick={() => switchChat(chat.chatId)}>{chat.title}</button>
    <button className={'favoriteAction ' + (chat.favorite ? 'isFavorite' : '')} title={chat.favorite ? 'Remove favorite' : 'Favorite'}
      onClick={() => favoriteChat(chat)} disabled={working} aria-label={chat.favorite ? 'Unfavorite chat' : 'Favorite chat'}>{chat.favorite ? '★' : '☆'}</button>
    <select className="chatMove" title="Move conversation to a folder" disabled={working} value={chat.folderId || ''}
      onChange={e => moveChat(chat,e.target.value || null)} aria-label={'Move ' + chat.title}>
      <option value="">—</option>{folders.map(f => <option key={f.folderId} value={f.folderId}>{f.name}</option>)}
    </select>
  </div>;
  return <div className={'shell' + (dragActive ? ' appDragActive' : '')}
    onDragEnter={handleAppDrag}
    onDragOver={handleAppDrag}
    onDragLeave={e => { if (!e.currentTarget.contains(e.relatedTarget)) setDragActive(false); }}
    onDrop={handleDrop}>
    <aside className="sidebar">
      <div className="brand"><div className="brandMark">L</div><div><strong>Lu</strong><small>Independent desktop</small></div></div>
      <button className="newChatButton" onClick={newChat}><span>＋</span> New Chat</button>
      <div className="chatSearchRow"><span aria-hidden="true">⌕</span><input ref={searchRef} type="search"
        value={chatQuery} onChange={e => setChatQuery(e.target.value)} placeholder="Search chats"
        aria-label="Search conversations" title="Search conversations (Ctrl+K)"/></div>
      <div className="chatSidebarContent">
        {chatQuery.trim() ? <section className="chatSection"><h3>SEARCH RESULTS · {matchingChats.length}</h3>
          {matchingChats.map(renderChat)}
          {!matchingChats.length && <p className="asideHelp">No conversations match your search.</p>}
        </section> : <>
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
        </section></>}
      </div>
      <div className="sidebarFooter">
        <div className="runtimeStatus"><span className="statusDot" data-ready={loaded}></span><strong>{loaded ? 'Model loaded' : working && activity.startsWith('Loading') ? 'Loading model…' : 'No model loaded'}</strong></div>
        <small>{loaded ? loadedModel?.filename : 'Select a model from the chat header'}</small>
        <button className="settingsButton" onClick={() => setBackgroundPaneOpen(true)}>
          ◷ Background Tasks {activeBackgroundTasks.length ? '· '+activeBackgroundTasks.length : ''}
        </button>
        <button className="settingsButton" onClick={() => setSettingsOpen(true)}>⚙ Settings</button>
      </div>
    </aside>
    <main className="workspace">
      <header className="topbar">
        <div className="chatTitle"><h1>{activeChat?.title && activeChat.title !== 'New chat' ? activeChat.title : 'Chat'}</h1>
          <p>{loaded ? 'Ready to chat · local model' : 'Choose a local GGUF to start'}</p></div>
        <div className="headerControls">
          {working && activity.startsWith('Loading') && <span className="busyHint" role="status" aria-live="polite"><span className="spinner" aria-hidden="true"></span> Loading model… {modelLoadSeconds}s elapsed</span>}
          <div className="modelSwitcherWrap" ref={modelMenuRef}>
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
              {loaded && <button className="subtleButton full" disabled={busy} onClick={() => {setModelMenuOpen(false);unloadModel();}}>⏏ Unload active model</button>}
            </div>}
          </div>
          {loaded && <button className="quickEject" disabled={busy} title="Unload active model" onClick={unloadModel}>⏏</button>}
          <button className="quietSettings" title="Settings" onClick={() => setSettingsOpen(true)}>⚙</button>
        </div>
      </header>
      <div className="transcriptWrap"><div className="messages" ref={transcriptRef} onScroll={onTranscriptScroll} role="log" aria-live="polite" aria-relevant="additions">
        {!history.length && <div className="welcome"><div className="welcomeSymbol">L</div>
          <h2>What can I help you build?</h2>
          <p>Local conversations are saved separately. Start a new chat or choose one from your history.</p></div>}
        {displayedHistory.map((entry,i) => <div key={i} className={'message ' + (entry.role === 'user' ? 'user' : 'assistant')}>
          <span className="messageRole">{entry.role === 'user' ? 'You' : 'Lu'}{entry.cancelled ? ' · stopped' : entry.interrupted ? ' · interrupted' : ''}</span>
          <MessageText text={entry.text} role={entry.role}/></div>)}
        {visibleJob && <div className="message assistant streaming"><span className="messageRole">
          {visibleJob.phase === 'queued' ? `Lu · queued${visibleJob.position ? ' (#' + visibleJob.position + ')' : ''}`
           : visibleJob.phase === 'submitting' ? 'Lu · submitting'
           : visibleJob.cancelling ? 'Lu · stopping' : 'Lu · generating'}
          </span>{visibleStream ? <MessageText text={visibleStream}/> : <span className="spinner"/>}</div>}
      </div>
        {!atBottom && <button type="button" className="jumpLatest" onClick={jumpToLatest}
          aria-label="Jump to latest message">↓ Jump to latest</button>}
      </div>
      <div className="composerArea">
        <div className="composerResizer" onPointerDown={beginComposerDrag} role="separator"
          aria-label="Resize composer" aria-orientation="horizontal" tabIndex={0}
          aria-valuemin={100} aria-valuemax={460} aria-valuenow={composerHeight}
          onKeyDown={e => {if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
            e.preventDefault(); setComposerHeight(h => Math.max(100, Math.min(460, h + (e.key === 'ArrowUp' ? 20 : -20))));
          }}}>
          <span className="composerHandle">···</span>
          <button type="button" className="collapseComposer" onPointerDown={e => e.stopPropagation()} onClick={() => setComposerCollapsed(v => !v)}>{composerCollapsed ? 'Expand' : 'Collapse'} composer</button>
        </div>
        {error && <div className="error" role="alert"><span>{error}</span><button onClick={() => setError('')}>Dismiss</button></div>}
        {!composerCollapsed && <form className={'composer' + (dragActive ? ' dragActive' : '')} onSubmit={send}>
          {(images.length > 0 || textFiles.length > 0) && <div className="attachments" aria-label="Draft attachments">
            {images.map(im => <span key={im.imageRef} className="attachment imageAttachment">
              {im.previewUrl ? <img src={im.previewUrl} alt="Attached image preview"/> : <span aria-hidden="true">▧</span>}
              <span title={im.name || im.mimeType}>{im.name || im.mimeType}{im.detectedMimeType && im.name?.toLowerCase().endsWith('.jpg') && im.detectedMimeType === 'image/png' ? ' (PNG data)' : ''}</span>
              <button type="button" onClick={() => removeImage(im.imageRef)} aria-label="Remove image">×</button>
            </span>)}
            {textFiles.map((file,i)=><span key={i} className="attachment textAttachment"><span aria-hidden="true">▤</span>
              <span title={attachmentSummary(file)}>{attachmentSummary(file)}</span>
              <button type="button" onClick={() => {const next=draftFiles.current.filter((_,n)=>n!==i);draftFiles.current=next;setTextFiles(next);}} aria-label={'Remove '+file.name}>×</button>
            </span>)}
          </div>}
          <textarea ref={composerRef} aria-label="Message Lu" value={prompt} onChange={e => {draftText.current=e.target.value;setPrompt(e.target.value);}} style={{height:composerHeight}} maxLength={16384}
            placeholder={loaded ? 'Message Lu… Paste or drop images and text files' : 'Load a GGUF model first…'} disabled={!loaded}
            onPaste={handlePaste}
            onKeyDown={e => { if (isSendKey(e)) { e.preventDefault(); e.currentTarget.form?.requestSubmit(); } }}/>
          <div className="composerFooter"><input ref={fileInput} type="file" className="visuallyHidden" multiple
              accept=".png,.jpg,.jpeg,.txt,.md,.csv,.json,.log,.py,.rs,.js,.jsx,.ts,.tsx" onChange={e => {void importAttachments(e.target.files);e.target.value='';}}/>
            <button type="button" className="subtleButton" disabled={!loaded} onClick={() => fileInput.current?.click()}>+ Attach</button>
            <span className="composerHelp">Paste or drop images and text · Enter to send · Shift+Enter for newline</span>
            {chatBusy ? <button className="dangerButton" type="button" onClick={cancel}
                disabled={!visibleJob.generationId || visibleJob.cancelling}>{visibleJob.cancelling ? 'Stopping…' : visibleJob.phase === 'queued' ? 'Cancel queued' : 'Cancel'}</button>
              : <button className="primaryButton" type="submit" disabled={working || !loaded || !prompt.trim()}>Send</button>}
          </div></form>}
      </div>
    </main>
    {dragActive && <div className="appDropHint" role="status" aria-live="polite">Drop image or text files to attach to this conversation</div>}
    {backgroundPaneOpen && <div className="backgroundPaneBackdrop" role="presentation"
      onClick={()=>setBackgroundPaneOpen(false)}>
      <aside className="backgroundPane" role="dialog" aria-modal="true"
        aria-label="Background Tasks" onClick={e=>e.stopPropagation()}>
        <header className="backgroundPaneHeader">
          <div><h2>Background Tasks</h2><small>Monitored by Lu — no model polling</small></div>
          <button className="quietIcon" aria-label="Close Background Tasks"
            onClick={()=>setBackgroundPaneOpen(false)}>×</button>
        </header>
        <div className="backgroundScanLaunch">
          <button type="button" className="primaryButton"
            onClick={()=>void startMediaFileScan()}>
            Scan media file…
          </button>
          <small>Native worker · file integrity scan (SHA-256), not video content analysis</small>
        </div>
        {backgroundTerminalNotice && <div className="backgroundTerminalBanner" role="status">
          <strong>Task ended: {backgroundTerminalNotice.state}</strong>
          <span>{backgroundTerminalNotice.taskId}</span>
          <button type="button" className="quietIcon"
            aria-label="Dismiss terminal notification"
            onClick={()=>setBackgroundTerminalNotice(null)}>×</button>
        </div>}
        <div className="backgroundPaneToolbar">
          <div className="backgroundPaneFilters" aria-label="Task history filter">
            <button className={backgroundFilter==='active'?'selected':''}
              onClick={()=>setBackgroundFilter('active')}>Active ({activeBackgroundTasks.length})</button>
            <button className={backgroundFilter==='all'?'selected':''}
              onClick={()=>setBackgroundFilter('all')}>Recent</button>
          </div>
          <button className="subtleButton" onClick={()=>void refreshBackgroundTasks()}
            aria-label="Refresh background tasks">Refresh</button>
        </div>
        {backgroundError && <p className="backgroundPaneError" role="alert">{backgroundError}</p>}
        <div className="backgroundPaneList" aria-live="off">
          {shownBackgroundTasks.map(task=><article className="backgroundTask" key={task.taskId}>
            <div className="backgroundTaskHeading">
              <strong title={task.taskId}>{task.taskId}</strong>
              <span className={'backgroundTaskStatus state-'+task.state.toLowerCase()}>{task.state}</span>
            </div>
            {task.workspaceId && <small>Workspace: {task.workspaceId}</small>}
            <p>{task.detail || 'No progress details yet'}</p>
            <div className="backgroundTaskProgress" role="progressbar"
              aria-label={'Progress for '+task.taskId} aria-valuemin={0}
              aria-valuemax={100} aria-valuenow={task.progress}>
              <span style={{width:Math.max(0,Math.min(100,task.progress))+'%'}}/>
            </div>
            <div className="backgroundTaskFoot">
              <span>{task.progress}% recorded</span><span>Updated: {task.updatedAt || 'unknown'}</span>
            </div>
            {['INTERRUPTED','FAILED'].includes(task.state) &&
              <p className="backgroundTaskWarning">Requires review before retrying.</p>}
            {task.canCancel && <button className="backgroundTaskCancel" type="button"
              onClick={()=>void cancelBackgroundTask(task.taskId)}>
              Cancel task…
            </button>}

          </article>)}
          {!shownBackgroundTasks.length && !backgroundError &&
            <p className="backgroundPaneEmpty">
              {backgroundFilter==='active'?'No active background tasks.':'No background task history yet.'}
            </p>}
        </div>
        <footer className="backgroundPaneFooter">
          Cancel is available only for Lu-owned live workers and requires native confirmation.
          Pause and resume remain unavailable until workers support them.
          Ordinary progress updates do not invoke the AI model.
        </footer>
      </aside>
    </div>}
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
